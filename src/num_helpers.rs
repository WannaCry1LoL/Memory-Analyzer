use crate::simd_types::ToSimd;
use crate::utils::MatchMask;
use bitvec::order::Lsb0;
use bitvec::view::BitView;
use num_traits::{Float, Saturating};
use std::cmp::Ordering;
use std::ptr::read_unaligned;
use std::simd::cmp::{SimdPartialEq, SimdPartialOrd};
use std::simd::num::SimdFloat;
use std::simd::prelude::*;

pub trait FromU128 {
	fn from_u128(x: u128) -> Self;
}

macro_rules! impl_from_u128 {
    ($($t:ty),*) => {
        $(
	        impl FromU128 for $t {
		        fn from_u128(x: u128) -> Self { x as Self }
	        }
        )*
    };
}

impl_from_u128!(u8, u16, u32, u64);

pub trait FuzzyEq {
	fn fuzzy_eq(self, other: Self, epsilon: Self) -> bool;
}

impl<T: Float> FuzzyEq for T {
	fn fuzzy_eq(self, other: Self, epsilon: Self) -> bool {
		if self == other { return true; }

		// Because NaN can have many different bit patterns, consider them equal to allow searching
		if self.is_nan() && other.is_nan() { return true; }

		let diff = (self - other).abs();
		if diff < epsilon {
			return true;
		}

		let largest = self.abs().max(other.abs());
		diff < largest * epsilon
	}
}

macro_rules! get {
	($e1:expr, $e2:expr) => { unsafe { $e1.get_unchecked($e2) } };
}

#[derive(Clone, Copy, Debug)]
pub enum ScanCondition<T: Scannable> {
	/// Equality
	Eq(T),
	/// !Eq
	NotEq(T),
	/// Checks if the value is >= lower and <= upper
	Range(T, T),
	/// Checks if the value is >= lower or <= upper
	WrappingRange(T, T),
	/// Checks for fuzzy equality (if relevant)
	FuzzyEq(T, T)
}

impl<T> ScanCondition<T> where T : Scannable {
	pub	fn default_alignment(&self) -> usize {
		<T as Scannable>::DEFAULT_ALIGNMENT
	}
}

pub trait Scannable : PartialEq + PartialOrd + ToSimd + 'static {
	type Vector = <Self as ToSimd>::SimdType;
	const DEFAULT_ALIGNMENT: usize = size_of::<Self>();

	fn load(bytes: &[u8]) -> Self::Vector {
		let ptr = bytes.as_ptr() as _;
		unsafe { read_unaligned(ptr) }
	}
	fn from_bytes(bytes: &[u8]) -> Self {
		let ptr = bytes.as_ptr() as _;
		unsafe { read_unaligned(ptr) }
	}
	fn eval(cond: &ScanCondition<Self>, value: Self::Vector) -> u64;
	fn matches(cond: &ScanCondition<Self>, value: Self) -> bool {
		match *cond {
			ScanCondition::Eq(n) => value == n,
			ScanCondition::NotEq(n) => value != n,
			ScanCondition::Range(lo, hi) => value >= lo && value <= hi,
			ScanCondition::WrappingRange(lo, hi) => value >= lo || value <= hi,
			ScanCondition::FuzzyEq(n, _) => value == n
		}
	}
}

pub fn scan<T: Scannable>(bytes: &[u8], cond: ScanCondition<T>, alignment: usize) -> MatchMask {
	let elem_size = T::SIZE;
	let reg_width = size_of::<T::Vector>();
	let mut res2 = MatchMask::new();
	let alignment = alignment.max(1);

	if bytes.len() < elem_size {
		return res2;
	}

	let mut i = 0;

	let stagger_limit = if alignment < elem_size { elem_size } else { 1 };

	while i + reg_width + stagger_limit - 1 <= bytes.len() {
		for offset in (0..stagger_limit).step_by(alignment) {
			let chunk = get!(bytes, i + offset .. i + offset + reg_width);
			let v = T::load(chunk);
			let bitmask = T::eval(&cond, v);

			res2.extend_from_bitslice(&bitmask.view_bits::<Lsb0>()[..T::LANES]);
			/*
			while bitmask != 0 {
				let lane = bitmask.trailing_zeros() as usize;
				res.push(i + offset + lane * elem_size);
				bitmask &= bitmask - 1;
			}
			*/
		}
		i += reg_width;
	}

	let mut rem_i = i;

	while rem_i + elem_size <= bytes.len() {
		if rem_i % alignment == 0 {
			let value = T::from_bytes(get!(bytes, rem_i .. rem_i + elem_size));
			res2.push(T::matches(&cond, value));
		}
		rem_i += 1;
	}

	res2
}

macro_rules! impl_scannable_numeric {
    ($($t:ty),*) => {
        $(
            impl Scannable for $t {
				fn eval(cond: &ScanCondition<Self>, v: Self::Vector) -> u64 {
					match *cond {
						ScanCondition::Eq(n) => v.simd_eq(Self::Vector::splat(n)).to_bitmask(),
						ScanCondition::NotEq(n) => v.simd_ne(Self::Vector::splat(n)).to_bitmask(),
						ScanCondition::Range(lo, hi) =>
							v.simd_ge(Self::Vector::splat(lo)).to_bitmask()
								& v.simd_le(Self::Vector::splat(hi)).to_bitmask(),
						ScanCondition::WrappingRange(lo, hi) =>
							v.simd_ge(Self::Vector::splat(lo)).to_bitmask()
								| v.simd_le(Self::Vector::splat(hi)).to_bitmask(),
						ScanCondition::FuzzyEq(n, eps) => {
							let (lo, hi) = (n.saturating_sub(eps), n.saturating_add(eps));
							v.simd_ge(Self::Vector::splat(lo)).to_bitmask()
								& v.simd_le(Self::Vector::splat(hi)).to_bitmask()
						}
					}
				}

				fn matches(cond: &ScanCondition<Self>, value: Self) -> bool {
					match *cond {
						ScanCondition::Eq(n) => value == n,
						ScanCondition::NotEq(n) => value != n,
						ScanCondition::Range(lo, hi) => value >= lo && value <= hi,
						ScanCondition::WrappingRange(lo, hi) => value >= lo || value <= hi,
						ScanCondition::FuzzyEq(n, eps) =>
							value >= n.saturating_sub(eps) && value <= n.saturating_add(eps),
					}
				}
			}
        )*
    };
}

macro_rules! impl_scannable_float {
    ($($t:ty),*) => {
        $(
            impl Scannable for $t {
	            fn eval(cond: &ScanCondition<Self>, v: Self::Vector) -> u64 {
	                match *cond {
	                    ScanCondition::Eq(n) => v.simd_eq(Self::Vector::splat(n)).to_bitmask(),
	                    ScanCondition::NotEq(n) => v.simd_ne(Self::Vector::splat(n)).to_bitmask(),
	                    ScanCondition::Range(lo, hi) => {
	                        v.simd_ge(Self::Vector::splat(lo)).to_bitmask()
	                            & v.simd_le(Self::Vector::splat(hi)).to_bitmask()
	                    }
	                    ScanCondition::WrappingRange(lo, hi) => {
		                    v.simd_ge(Self::Vector::splat(lo)).to_bitmask()
		                        | v.simd_le(Self::Vector::splat(hi)).to_bitmask()
	                    }
	                    ScanCondition::FuzzyEq(n, eps) => {
	                        let needle_simd = Self::Vector::splat(n);
	                        let eps_simd = Self::Vector::splat(eps);

	                        let is_eq = v.simd_eq(needle_simd);
	                        let both_nan = v.is_nan() & needle_simd.is_nan();

	                        let diff = (v - needle_simd).abs();
	                        let is_within_eps = diff.simd_lt(eps_simd);

	                        let largest = v.abs().simd_max(needle_simd.abs());
	                        let is_relative_eq = diff.simd_lt(largest * eps_simd);

	                        (is_eq | both_nan | is_within_eps | is_relative_eq).to_bitmask()
	                    }
	                }
	            }

	            fn matches(cond: &ScanCondition<Self>, value: Self) -> bool {
	                match *cond {
	                    ScanCondition::Eq(n) => value == n,
	                    ScanCondition::NotEq(n) => value != n,
	                    ScanCondition::Range(lo, hi) => value >= lo && value <= hi,
	                    ScanCondition::WrappingRange(lo, hi) => value >= lo || value <= hi,
	                    ScanCondition::FuzzyEq(n, eps) => n.fuzzy_eq(value, eps),
	                }
	            }
	        }
        )*
    };
}

impl_scannable_numeric!(u8, i8, u16, i16, u32, i32, u64, i64);
impl_scannable_float!(f32, f64);

pub trait ArrayScanExt {
	fn scan_for<T: Scannable>(&self, cond: ScanCondition<T>, alignment: usize) -> MatchMask;

	fn find_eq<T: Scannable>(&self, needle: T, alignment: usize) -> MatchMask {
		self.scan_for(ScanCondition::Eq(needle), alignment)
	}

	fn find_neq<T: Scannable>(&self, needle: T, alignment: usize) -> MatchMask {
		self.scan_for(ScanCondition::NotEq(needle), alignment)
	}

	fn find_range<T: Scannable>(&self, lo: T, hi: T, alignment: usize) -> MatchMask {
		self.scan_for(ScanCondition::Range(lo, hi), alignment)
	}

	fn find_range_dual<T: Scannable + FromU128>(&self, lo: i128, hi: i128, alignment: usize) -> MatchMask {
		let bits: usize = <T as ToSimd>::SIZE * 8;

		let range_min = -(1 << (bits - 1));
		let range_max = (1 << bits) - 1;

		let clamped_lo = lo.clamp(range_min, range_max) as _;
		let clamped_hi = hi.clamp(range_min, range_max) as _;

		let thing_lo: T = T::from_u128(clamped_lo);
		let thing_hi: T = T::from_u128(clamped_hi);

		match clamped_lo.cmp(&clamped_hi) {
			Ordering::Equal   => self.scan_for(ScanCondition::Eq(thing_lo), alignment),
			Ordering::Less    => self.scan_for(ScanCondition::Range(thing_lo, thing_hi), alignment),
			Ordering::Greater => self.scan_for(ScanCondition::WrappingRange(thing_lo, thing_hi), alignment),
		}
	}

	fn find_pattern(&self, needle: &[u8], alignment: usize) -> MatchMask;
}

pub trait ArrayByteExt {
	fn cmp_signed_be(&self, b: &[u8]) -> Ordering;
	fn cmp_signed_le(&self, b: &[u8]) -> Ordering;
	fn cmp_le(&self, b: &[u8]) -> Ordering;
}

impl ArrayByteExt for [u8] {
	fn cmp_signed_be(&self, b: &[u8]) -> Ordering {
		debug_assert_eq!(self.len(), b.len());
		if self.is_empty() { return Ordering::Equal; }

		let a_msb_flipped = self[0] ^ 0x80;
		let b_msb_flipped = b[0] ^ 0x80;

		match a_msb_flipped.cmp(&b_msb_flipped) {
			Ordering::Equal => get!(self, 1..).cmp(get!(&b, 1..)),
			other => other,
		}
	}

	fn cmp_signed_le(&self, b: &[u8]) -> Ordering {
		debug_assert_eq!(self.len(), b.len());
		let n = self.len();
		if n == 0 { return Ordering::Equal; }

		let a_msb_flipped = get!(self, n - 1) ^ 0x80;
		let b_msb_flipped = get!(b, n - 1) ^ 0x80;

		match a_msb_flipped.cmp(&b_msb_flipped) {
			Ordering::Equal => get!(self, ..n - 1).cmp_le(get!(&b, ..n - 1)),
			other => other,
		}
	}

	fn cmp_le(&self, b: &[u8]) -> Ordering {
		debug_assert_eq!(self.len(), b.len());
		let n = self.len();
		if n == 0 { return Ordering::Equal; }

		let mut i = n;

		while i >= 64 {
			i -= 64;
			let val_a = u8x64::from_slice(get!(&self, i..i + 64));
			let val_b = u8x64::from_slice(get!(&b, i..i + 64));

			if val_a != val_b {
				let bitmask = val_a.simd_ne(val_b).to_bitmask();
				let highest_differing_lane = 63 - bitmask.leading_zeros() as usize;
				return get!(self, i + highest_differing_lane).cmp(get!(&b, i + highest_differing_lane));
			}
		}

		while i > 0 {
			i -= 1;
			let (a, b) = (get!(self, i), get!(b, i));
			if a != b {
				return a.cmp(b);
			}
		}

		Ordering::Equal
	}
}

impl ArrayScanExt for [u8] {
	fn scan_for<T: Scannable>(&self, cond: ScanCondition<T>, alignment: usize) -> MatchMask {
		scan(self, cond, alignment)
	}

	fn find_pattern(&self, needle: &[u8], alignment: usize) -> MatchMask {
		let n = needle.len();
		let alignment = alignment.max(1);

		if n == 0 || self.len() < n {
			return MatchMask::new();
		}

		let last_valid_start = self.len() - n + 1;
		let search_region = get!(self, ..last_valid_start);

		let mut candidates = search_region.find_eq(*get!(needle, 0), alignment);

		candidates
			.clone()
			.iter_ones()
			.filter(|&i| get!(self, i..i+n) != needle)
			.for_each(|i| candidates.set(i, false));

		candidates
	}
}