use std::ptr::read_unaligned;
use bitvec::vec::BitVec;

use crate::errors::ProcessImagePathError::{self, InvalidUtf16ImagePath};
use crate::ffi::wrappers::query_full_process_image_name;
use crate::ffi::types::{Handle, UnicodeString};
use Alignment::*;
use TriState::*;

pub type MatchMask = BitVec<u64, bitvec::prelude::Lsb0>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TriState {
	False = 0,
	True = 1,
	DontCare = 2,
}

impl TriState {
	pub fn matches(&self, actual: bool) -> bool {
		match self {
			DontCare => true,
			True => actual,
			False => !actual,
		}
	}

	pub fn cycle(self, rev: bool) -> Self {
		match self {
			DontCare if rev => False,
			False if rev => True,
			True if rev => DontCare,
			DontCare => True,
			True => False,
			False => DontCare,
		}
	}

	pub fn label(&self) -> &'static str {
		match self {
			True => "Y",
			False => "N",
			DontCare => "*",
		}
	}
}

impl From<bool> for TriState {
	fn from(v: bool) -> TriState { if v { True } else { False } }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Alignment {
	OneByte = 1,
	TwoByte = 2,
	FourByte = 4,
	EightByte = 8,
	SixteenByte = 16,
	ThirtyTwoByte = 32
}

impl Alignment {
	pub const ALL: [Self; 6] = [
		OneByte,
		TwoByte,
		FourByte,
		EightByte,
		SixteenByte,
		ThirtyTwoByte
	];

	pub fn cycle(self, dir: i32) -> Self {
		let all = Self::ALL;
		let idx = all.iter().position(|v| *v == self).unwrap_or(0) as i32;
		let len = all.len() as i32;
		all[(idx + dir).rem_euclid(len) as usize]
	}

	pub fn label(&self) -> &'static str {
		match self {
			OneByte => "1 Byte",
			TwoByte => "2 Bytes",
			FourByte => "4 Bytes",
			EightByte => "8 Bytes",
			SixteenByte => "16 Bytes",
			ThirtyTwoByte => "32 Bytes",
		}
	}

	pub const fn value(&self) -> usize { *self as _ }
}

pub fn get_process_image_path(handle: Handle) -> Result<String, ProcessImagePathError> {
	let mut buffer = vec![0u16; 1024];
	let bytes_written = unsafe {
		query_full_process_image_name(handle, &mut buffer[..])
			.map_err(ProcessImagePathError::Unknown)?
	} as usize;

	let unicode_struct_size = size_of::<UnicodeString>();
	if bytes_written < unicode_struct_size {
		return Err(InvalidUtf16ImagePath);
	}

	let unicode_str = unsafe { read_unaligned(buffer.as_ptr() as *const UnicodeString) };
	let char_len = unicode_str.length as usize / 2;
	let start_idx = unicode_struct_size / 2;

	if start_idx + char_len > buffer.len() {
		return Err(InvalidUtf16ImagePath);
	}

	String::from_utf16(&buffer[start_idx..start_idx + char_len])
		.map_err(|_| InvalidUtf16ImagePath)
}

pub fn string_to_bytes_radix(s: &str, radix: Option<u32>) -> Result<Vec<u8>, ()> {
	let radix = radix.unwrap_or(10);
	if radix < 2 || radix > 36 { return Err(()) }
	let stripped = s.trim().trim_start_matches('0');

	if stripped.is_empty() { return Ok(vec![0]); }

	let mut bytes = Vec::new();

	for c in stripped.chars() {
		let mut carry = c.to_digit(radix).ok_or(())?;

		for byte in bytes.iter_mut() {
			let current = (*byte as u32) * radix + carry;
			*byte = (current & 0xFF) as u8;
			carry = current >> 8;
		}

		while carry > 0 {
			bytes.push((carry & 0xFF) as u8);
			carry >>= 8;
		}
	}

	Ok(bytes.into_iter().rev().collect())
}

pub fn string_to_bytes_dec(s: &str) -> Result<Vec<u8>, ()> { string_to_bytes_radix(s, None) }
pub fn string_to_bytes_hex(s: &str) -> Result<Vec<u8>, ()> { string_to_bytes_radix(s, Some(16)) }

#[cfg(not(debug_assertions))]
macro_rules! get {
	($arr:expr, $range:expr) => {
		unsafe { $arr.get_unchecked($range) }
	};
	($arr:expr, $start:expr, $len:expr) => {
		get!($arr, $start..$start + $len)
	};
}

#[cfg(debug_assertions)]
macro_rules! get {
	($arr:expr, $range:expr) => {
		&$arr[$range]
	};
	($arr:expr, $start:expr, $len:expr) => {
		get!($arr, $start..$start + $len)
	};
}

pub(crate) use get;

#[derive(Clone, Copy)]
pub struct BitLayout {
	init_mask: u64,
	steps: [(u32, u64); 6],
	num_steps: usize,
}

#[allow(non_snake_case)]
#[inline(always)]
pub const fn compute_layout(N: usize) -> BitLayout {
	assert!(N.is_power_of_two() && N >= 2 && N <= 64, "N must be a power of two 2 < N < 64");

	let mut init_mask = 0u64;
	let mut i = 0;
	while i < 64 {
		init_mask |= 1u64 << i;
		i += N;
	}

	let mut steps = [(0u32, 0u64); 6];
	let mut num_steps = 0;

	let mut shift = (N - 1) as u32;
	let mut block_size = 1;

	while shift < 64 {
		let group_bits = block_size * 2;
		let period = N * group_bits;
		if period > 64 {
			break;
		}

		let group_mask = if group_bits == 64 {
			u64::MAX
		} else {
			(1u64 << group_bits) - 1
		};

		let mut step_mask = 0u64;
		let mut p = 0;
		while p < 64 {
			step_mask |= group_mask << p;
			p += period;
		}

		steps[num_steps] = (shift, step_mask);
		num_steps += 1;

		shift <<= 1;
		block_size <<= 1;
	}

	BitLayout { init_mask, steps, num_steps }
}

#[inline]
pub fn extract(val: u64, layout: BitLayout) -> u64 {
	let mut x = val & layout.init_mask;
	let mut i = 0;
	while i < layout.num_steps {
		let (shift, mask) = layout.steps[i];
		x = (x | (x >> shift)) & mask;
		i += 1;
	}
	x
}

#[inline]
pub fn deposit(val: u64, layout: BitLayout) -> u64 {

	let start_mask = layout.steps[layout.num_steps - 1].1;
	let mut x = val & start_mask;

	let mut i = layout.num_steps;
	while i > 0 {
		i -= 1;
		let shift = layout.steps[i].0;
		let mask = if i == 0 {
			layout.init_mask
		} else {
			layout.steps[i - 1].1
		};
		x = (x | (x << shift)) & mask;
	}
	x
}