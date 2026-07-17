use std::collections::HashMap;

use bitvec::vec::BitVec;

use crate::utils::TriState::{DontCare, False, True};
use crate::win_helpers::{enum_proc_info, EnumerationState};

pub type MatchMask = BitVec<u64, bitvec::prelude::Lsb0>;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct Pid(pub u32);

unsafe impl Send for Pid {}
unsafe impl Sync for Pid {}

macro_rules! impl_fmt_forward {
    ($ty:ty => $($trait:path),+ $(,)?) => {
        $(
            impl $trait for $ty {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.0.fmt(f) }
            }
        )+
    };
}

impl_fmt_forward!(Pid => std::fmt::Display, std::fmt::UpperHex);

impl From<u32> for Pid {
	fn from(pid: u32) -> Self { Self(pid) }
}

impl From<Pid> for u32 {
	fn from(pid: Pid) -> Self { pid.0 }
}

pub fn build_process_tree() -> HashMap<usize, Vec<usize>> {
	let mut ret: HashMap<usize, Vec<usize>> = HashMap::new();
	enum_proc_info(|info| {
		let ppid = info.inherited_from_unique_process_id;
		let pid = info.unique_process_id;
		ret.entry(ppid).or_default().push(pid);

		EnumerationState::<()>::Continue
	});
	ret
}

pub fn find_proc_from_image_name(name: &str) -> Option<usize> {
	enum_proc_info(|info| {
		if !info.image_name.buffer.is_null()
			&& let res = info.image_name.to_string()
			&& res == name
		{ EnumerationState::Return(info.unique_process_id) } 
		else { EnumerationState::Continue }
	})
}

#[derive(PartialEq, Clone, Copy, Debug)]
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
