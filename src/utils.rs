use std::ptr::read_unaligned;
use bitvec::vec::BitVec;

use crate::errors::ProcessImagePathError::{self, InvalidUtf16ImagePath};
use crate::utils::TriState::{DontCare, False, True};
use crate::ffi::wrappers::query_full_process_image_name;
use crate::ffi::types::{Handle, UnicodeString};

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