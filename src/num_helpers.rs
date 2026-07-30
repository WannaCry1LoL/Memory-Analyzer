#![allow(unexpected_cfgs)]

use std::cmp::Ordering;
use std::marker::PhantomData;
use std::ptr::read_unaligned;
use std::simd::cmp::{SimdPartialEq, SimdPartialOrd};
use std::simd::prelude::*;

use bitvec::order::Lsb0;
use bitvec::view::BitView;
use num_traits::Float;

use crate::simd_types::ToSimd;
use crate::utils::{compute_layout, deposit, extract, get, BitLayout, MatchMask};

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
	FuzzyEq(T, T),
}

impl<T: Scannable> ScanCondition<T> {

	pub const fn from_greater_than(value: T) -> Self {
		Self::Range(value, T::MAX)
	}

	pub const fn from_less_than(value: T) -> Self {
		Self::Range(T::MIN, value)
	}
}

impl<T: Scannable + FromU128> ScanCondition<T> {
	pub fn from_logical_range(lo: i128, hi: i128) -> ScanCondition<T> {
		let bits: usize = <T as ToSimd>::SIZE * 8;

		let range_min = -(1i128 << (bits - 1));
		let range_max = (1i128 << bits) - 1;

		let clamped_lo = lo.clamp(range_min, range_max) as _;
		let clamped_hi = hi.clamp(range_min, range_max) as _;

		let thing_lo: T = T::from_u128(clamped_lo);
		let thing_hi: T = T::from_u128(clamped_hi);

		match clamped_lo.cmp(&clamped_hi) {
			Ordering::Equal => ScanCondition::Eq(thing_lo),
			Ordering::Less => ScanCondition::Range(thing_lo, thing_hi),
			Ordering::Greater => ScanCondition::WrappingRange(thing_lo, thing_hi),
		}
	}
}

pub trait Scannable: PartialEq + PartialOrd + ToSimd + 'static {
	type Vector = <Self as ToSimd>::SimdType;
	const DEFAULT_ALIGNMENT: usize = size_of::<Self>();
	const MAX: Self;
	const MIN: Self;

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
			ScanCondition::FuzzyEq(n, _) => value == n,
		}
	}
}

struct ScanHelper<T, const N: usize> {
	_marker: PhantomData<T>,
}

impl<T: Scannable, const N: usize> ScanHelper<T, N> {
	pub const COMPRESS_RATIO: usize = if N > T::SIZE { N / T::SIZE } else { 2 };
	pub const DEPOSIT_RATIO: usize = if T::SIZE > N { T::SIZE / N } else { 2 };

	pub const COMPRESS_LAYOUT: BitLayout = compute_layout(Self::COMPRESS_RATIO);
	pub const DEPOSIT_LAYOUT: BitLayout = compute_layout(Self::DEPOSIT_RATIO);
}

#[allow(unused)]
#[inline(always)]
fn deposit_bits(value: u64, mask: u64, layout: BitLayout) -> u64 {
	#[cfg(fast_bmi2)]
	unsafe { ::core::arch::x86_64::_pdep_u64(value, mask) }
	#[cfg(not(fast_bmi2))]
	deposit(value, layout)
}

#[allow(unused)]
#[inline(always)]
fn extract_bits(value: u64, mask: u64, layout: BitLayout) -> u64 {
	#[cfg(fast_bmi2)]
	unsafe { ::core::arch::x86_64::_pext_u64(value, mask) }
	#[cfg(not(fast_bmi2))]
	extract(value, layout)
}

#[inline(always)]
fn spacing_mask(ratio: usize) -> u64 {
	let mut mask = 0;
	for i in 0..(64 / ratio) {
		mask |= 1 << (i * ratio);
	}
	mask
}

pub fn scan<T: Scannable, const N: usize>(bytes: &[u8], cond: ScanCondition<T>) -> MatchMask {
	let elem_size = T::SIZE;
	let reg_width = size_of::<T::Vector>();
	let elements = 1 + (bytes.len().saturating_sub(elem_size)) / N;
	let mut res2 = MatchMask::with_capacity(elements);

	if bytes.len() < elem_size {
		return res2;
	}

	let mut i = 0;

	if N == elem_size {
		while i + reg_width <= bytes.len() {
			let chunk = get!(bytes, i, reg_width);
			let v = T::load(chunk);
			let mask = T::eval(&cond, v);
			let view = &mask.view_bits::<Lsb0>()[..T::LANES];
			res2.extend_from_bitslice(view);
			i += reg_width;
		}
	} else if N > elem_size {
		debug_assert_eq!(N % elem_size, 0);
		let layout = ScanHelper::<T, N>::COMPRESS_LAYOUT;
		let ratio = ScanHelper::<T, N>::COMPRESS_RATIO;
		let extract_mask = spacing_mask(ratio);
		while i + reg_width <= bytes.len() {
			let chunk = get!(bytes, i, reg_width);
			let v = T::load(chunk);
			let mask = T::eval(&cond, v);
			let compressed = extract_bits(mask, extract_mask, layout);
			let view = &compressed.view_bits::<Lsb0>()[..T::LANES / ratio];
			res2.extend_from_bitslice(view);
			i += reg_width;
		}
	} else {
		debug_assert_eq!(elem_size % N, 0);
		let layout = ScanHelper::<T, N>::DEPOSIT_LAYOUT;
		let ratio = ScanHelper::<T, N>::DEPOSIT_RATIO;
		let deposit_mask = spacing_mask(ratio);
		while i + reg_width + elem_size - 1 <= bytes.len() {
			let mut chunk_mask = 0u64;
			for offset in (0..elem_size).step_by(N) {
				let chunk = get!(bytes, i + offset, reg_width);
				let v = T::load(chunk);
				let bitmask = T::eval(&cond, v);
				let spaced_bits = deposit_bits(bitmask, deposit_mask, layout);
				chunk_mask |= spaced_bits << (offset / N);
			}
			let view = &chunk_mask.view_bits::<Lsb0>()[..T::LANES * ratio];
			res2.extend_from_bitslice(view);
			i += reg_width;
		}
	}

	let mut rem_i = i.div_ceil(N) * N;

	while rem_i + elem_size <= bytes.len() {
		let value = T::from_bytes(get!(bytes, rem_i, elem_size));
		res2.push(T::matches(&cond, value));
		rem_i += N;
	}

	res2
}

macro_rules! impl_scannable_numeric {
    ($($t:ty),*) => {
        $(
            impl Scannable for $t {
	            const MIN: Self = <$t>::MIN;
	            const MAX: Self = <$t>::MAX;
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
	            const MIN: Self = <$t>::MIN;
	            const MAX: Self = <$t>::MAX;
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
			Ordering::Equal => get!(self, 1..).cmp(get!(b, 1..)),
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
			Ordering::Equal => get!(self, ..n - 1).cmp_le(get!(b, ..n - 1)),
			other => other,
		}
	}

	fn cmp_le(&self, b: &[u8]) -> Ordering {
		debug_assert_eq!(self.len(), b.len());
		let n = self.len();
		if n == 0 {
			return Ordering::Equal;
		}

		let mut i = n;

		while i >= 64 {
			i -= 64;
			let val_a = u8x64::from_slice(get!(self, i, 64));
			let val_b = u8x64::from_slice(get!(b, i, 64));

			if val_a != val_b {
				let bitmask = val_a.simd_ne(val_b).to_bitmask();
				let highest_differing_lane = 63 - bitmask.leading_zeros() as usize;
				return get!(self, i + highest_differing_lane)
					.cmp(get!(b, i + highest_differing_lane));
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
		match alignment {
			1 => scan::<T, 1>(self, cond),
			2 => scan::<T, 2>(self, cond),
			4 => scan::<T, 4>(self, cond),
			8 => scan::<T, 8>(self, cond),
			16 => scan::<T, 16>(self, cond),
			32 => scan::<T, 32>(self, cond),
			_ => unreachable!(),
		}
	}

	fn find_pattern(&self, needle: &[u8], alignment: usize) -> MatchMask {
		let n = needle.len();

		if n == 0 || self.len() < n {
			return MatchMask::new();
		}

		let last_valid_start = self.len() - n + 1;
		let search_region = get!(self, ..last_valid_start);

		let mut candidates = search_region.find_eq(*get!(needle, 0), alignment);

		candidates
			.clone()
			.iter_ones()
			.filter(|&i| get!(self, i, n) != needle)
			.for_each(|i| candidates.set(i, false));

		candidates
	}
}
