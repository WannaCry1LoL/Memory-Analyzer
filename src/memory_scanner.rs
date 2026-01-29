use rayon::iter::ParallelIterator;
use crate::errors::MemoryReadError;
use crate::matches::{Match, MatchType, RegionId};
use crate::rw_helper::read_bytes;
use crate::utils::TriState::{DontCare, False, True};
use crate::utils::{get_sys_info, TriState};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::{null_mut, read_unaligned};
use std::{f128, f16, mem, slice};
use std::sync::Arc;
use rayon::prelude::IntoParallelIterator;
use winapi::um::memoryapi::{GetProcessWorkingSetSizeEx, VirtualFree};
use windows::core::s;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Memory::{VirtualAlloc, VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_RELEASE, PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_PROTECTION_FLAGS, PAGE_READWRITE, PAGE_WRITECOPY, SETPROCESSWORKINGSETSIZEEX_FLAGS};
use crate::externs::{NtResumeProcess, NtSuspendProcess};

#[derive(Clone)]
pub enum PatternMatcher {
	None,
	Literal(u8),
	Literal2(u16),
	Literal4(u32),
	Literal8(u64),
	Literal16(u128),
	Wildcard,
	Repeated { pattern: Box<PatternMatcher>, amount: usize },
	Range {lower: u8, upper: u8},
	/// Short form of Pattern::Repeated(Pattern::Wildcard, amount)
	Gap(usize),
	Mask { value: u8, mask: u8 },
	Or(Vec<PatternMatcher>),
	Not(Box<PatternMatcher>),
	Half(f16),
	Single(f32),
	Double(f64),
	Quad(f128),
	List(Vec<PatternMatcher>),
}

macro_rules! buffer_to_type {
    ($t:ty, $expression:ident) => {
	    <$t>::from_ne_bytes(<[u8; std::mem::size_of::<$t>()]>::try_from($expression).unwrap())
    };
}

impl PatternMatcher {
	pub fn size(&self) -> usize {
		match &self {
			Self::None => 0,
			Self::Wildcard | Self::Literal(_) | Self::Range { .. } | Self::Mask { .. } => 1,
			Self::Repeated { pattern, amount } => pattern.size() * amount,
			Self::Gap(n) => *n,
			Self::Not(pattern) => pattern.size(),
			Self::Or(patterns) => patterns.first().unwrap_or(&Self::None).size(),
			Self::Half(_) | Self::Literal2(_) => 2,
			Self::Single(_) | Self::Literal4(_) => 4,
			Self::Double(_) | Self::Literal8(_) => 8,
			Self::Quad(_) | Self::Literal16(_) => 16,
			Self::List(patterns) => patterns.iter().map(|p| p.size()).sum(),
		}
	}
	pub fn matches(&self, buffer: &[u8]) -> bool {
		let size = self.size();
		if size == 0 {
			return true;
		}
		if buffer.len() < size {
			return false;
		}
		let read = &buffer[..size];
		match &self {
			Self::Wildcard => true,
			Self::Gap(n) => true,
			Self::Literal(val) => read.first().unwrap() == val,
			Self::Literal2(expected) => {
				let actual = buffer_to_type!(u16, buffer);
				actual == *expected
			},
			Self::Literal4(expected) => {
				let actual = buffer_to_type!(u32, buffer);
				actual == *expected
			},
			Self::Literal8(expected) => {
				let actual = buffer_to_type!(u64, buffer);
				actual == *expected
			},
			Self::Literal16(expected) => {
				let actual = buffer_to_type!(u128, buffer);
				actual == *expected
			},
			Self::Range { lower, upper } => {
				let tmp = read.first().unwrap();
				tmp >= lower && tmp <= upper
			},
			Self::Half(expected) => {
				let val = buffer_to_type!(f16, read);
				(val - expected).abs() < 0.05
			},
			Self::Single(expected) => unsafe {
				let val = read_unaligned::<f32>(read.as_ptr() as _);
				(val - expected).abs() < 0.05
			},
			Self::Double(expected) => {
				let val = buffer_to_type!(f64, read);
				(val - expected).abs() < 0.01
			},
			Self::Quad(expected) => {
				let val = buffer_to_type!(f128, read);
				(val - expected).abs() < 0.01
			}
			Self::Repeated { pattern, amount } => {
				read.len() == pattern.size() * amount
				&& read.chunks_exact(pattern.size()).all(|c| pattern.matches(c))
			},
			Self::Not(pattern) => !pattern.matches(read),
			Self::Or(patterns) => patterns.iter().any(|pattern| pattern.matches(read)),
			Self::Mask { value, mask } => {
				let byte = read.first().unwrap();
				byte & mask == *value
			},
			Self::List(patterns) => {
				let mut new_read = read;
				patterns.iter().all(|p| {
					let res = p.matches(&new_read[..p.size()]);
					new_read = &new_read[p.size()..];
					res
				})
			},
			Self::None => unreachable!(),
		}
	}
}

pub struct Pattern {
	pattern_type: PatternMatcher,
	pub match_type: MatchType,
}

impl Pattern {
	pub fn new(pattern_type: PatternMatcher, match_type: MatchType) -> Self {
		Self {
			pattern_type,
			match_type
		}
	}

	pub fn size(&self) -> usize {
		self.pattern_type.size()
	}

	pub fn matches(&self, buffer: &[u8]) -> bool {
		self.pattern_type.matches(buffer)
	}
}


pub struct MemoryScannerOptions {
	pub respect_alignment: bool,
	pub align_size: usize,
	pub writable: TriState,
	pub executable: TriState,
	pub copy_on_write: TriState
}

impl Default for MemoryScannerOptions {
	fn default() -> Self {
		Self {
			respect_alignment: true,
			align_size: 4,
			writable: True,
			executable: DontCare,
			copy_on_write: False,
		}
	}
}

impl MemoryScannerOptions {
	pub fn new(respect_alignment: bool, align_size: usize, writable: TriState, executable: TriState, cow: TriState) -> Self {
		Self { respect_alignment, align_size, writable, executable, copy_on_write: cow }
	}

	fn is_writable(flags: PAGE_PROTECTION_FLAGS) -> bool { flags.contains(PAGE_READWRITE) || flags.contains(PAGE_EXECUTE_READWRITE) }

	fn is_executable(flags: PAGE_PROTECTION_FLAGS) -> bool {
		flags.contains(PAGE_EXECUTE) || flags.contains(PAGE_EXECUTE_READ) || flags.contains(PAGE_EXECUTE_READWRITE) || flags.contains(PAGE_EXECUTE_WRITECOPY)
	}

	fn is_cow(flags: PAGE_PROTECTION_FLAGS) -> bool { flags.contains(PAGE_WRITECOPY) || flags.contains(PAGE_EXECUTE_WRITECOPY) }

	fn check_tristate(value: TriState, actual: bool) -> bool {
		match value {
			DontCare => true,
			True => actual,
			False => !actual,
		}
	}
	pub fn check_page_flags(&self, flags: PAGE_PROTECTION_FLAGS) -> bool {
		Self::check_tristate(self.writable, Self::is_writable(flags))
			&& Self::check_tristate(self.executable, Self::is_executable(flags))
			&& Self::check_tristate(self.copy_on_write, Self::is_cow(flags))
	}
}

pub struct ScanState {
	pub results: HashMap<RegionId, Vec<Match>>,
}

impl Default for ScanState {
	fn default() -> Self {
		Self {
			results: HashMap::new(),
		}
	}
}

pub struct MemoryScanner {
	handle: HANDLE,
	state: ScanState,
	options: MemoryScannerOptions,
}

impl Default for MemoryScanner {
	fn default() -> Self {
		Self {
			handle: Default::default(),
			state: Default::default(),
			options: Default::default(),
		}
	}
}

impl MemoryScanner {

	pub fn print_matches(&self) {
		self.state.results.iter().for_each(|(region, results)| {
			println!("Address: {}", results.iter().map(|x| format!("{:X}", region.0 + x.location.offset)).collect::<Vec<_>>().join(", "));
			// after some fiddling i gave up just did it like this
		})
	}
	fn get_chunk_info(handle: HANDLE, base_addr: *const c_void, info: &mut MEMORY_BASIC_INFORMATION) {
		unsafe {
			VirtualQueryEx(
				handle,
				Some(base_addr),
				info,
				size_of::<MEMORY_BASIC_INFORMATION>(),
			);
		}
	}

	#[inline]
	unsafe fn get_chunk(handle: HANDLE, options: &MemoryScannerOptions, base_addr: *const c_void, info: &mut MEMORY_BASIC_INFORMATION) -> Result<Vec<u8>, MemoryReadError> {
		Self::get_chunk_info(handle, base_addr, info);
		if !(info.State == MEM_COMMIT && options.check_page_flags(info.Protect)) {
			return Err(MemoryReadError::UnsuitableMemory);
		}

		let mut buffer = vec![0u8; info.RegionSize];

		let res = read_bytes(handle, info.BaseAddress as _, buffer.as_mut_ptr(), info.RegionSize);
		res?;
		Ok(buffer)
	}

	unsafe fn get_chunk_buffer(handle: HANDLE, options: &MemoryScannerOptions, base_addr: *const c_void, info: &mut MEMORY_BASIC_INFORMATION, buffer: *mut u8) -> Result<(), MemoryReadError> {
		Self::get_chunk_info(handle, base_addr, info);
		if !(info.State == MEM_COMMIT && options.check_page_flags(info.Protect)) {
			return Err(MemoryReadError::UnsuitableMemory);
		}

		let res = read_bytes(handle, info.BaseAddress as _, buffer, info.RegionSize);
		res?;
		Ok(())
	}

	pub fn new(handle: HANDLE, options: MemoryScannerOptions) -> Self {
		Self { handle, state: Default::default(), options }
	}

	pub fn scan(&mut self, pattern: Pattern) {
		let mut new_state: ScanState = Default::default();
		let increment = if self.options.respect_alignment { self.options.align_size } else { 1 };
		unsafe {
			let si = get_sys_info();

			let mut info: MEMORY_BASIC_INFORMATION = mem::zeroed();
			let mut base_addr: *mut c_void = null_mut();

			let options = &self.options;
			let handle = self.handle;

			/*

			let mut flags: u32 = mem::zeroed();
			let mut min_size: usize = 0;
			let mut max_size: usize = 0;

			GetProcessWorkingSetSizeEx(
				handle.0,
				&mut min_size,
				&mut max_size,
				&mut flags,
			);

			let buffer = VirtualAlloc(
				None,
				max_size / 1024,
				MEM_COMMIT,
				PAGE_READWRITE
			) as *mut u8;
			let buffer_as_slice = slice::from_raw_parts(buffer, (max_size / 1024) as _);
			*/
			while base_addr < si.lpMaximumApplicationAddress {
				let res = Self::get_chunk(handle, options, base_addr, &mut info);//Self::get_chunk_buffer(handle, options, base_addr, &mut info, buffer);
				if let Ok(buff) = res {
					for i in (0..info.RegionSize).step_by(increment) {
						let slice = &buff[i..];//&buffer_as_slice[i..];
						if pattern.matches(slice) {
							new_state.results
								.entry(RegionId(base_addr as usize)).or_insert(Default::default())
								.push(Match::new(pattern.match_type, i, slice, slice));
						}
					}
				}

				base_addr = info.BaseAddress.add(info.RegionSize);
			}
			//VirtualFree(buffer as _, max_size, MEM_RELEASE.0);
		}
		self.state = new_state;
	}

	pub fn update(&mut self) {
		unsafe {
			let mut info: MEMORY_BASIC_INFORMATION = mem::zeroed();

			let handle = self.handle;
			let options = &self.options;
			let old_results = std::mem::take(&mut self.state.results);
			self.state.results = old_results
				.into_iter()
				.filter_map(|(key, mut value)| {
					if let Ok(buffer) = Self::get_chunk(handle, options, key.0 as _, &mut info) {
						for val in &mut value {
							let base = val.location.offset;
							let tmp_val = &buffer[base..base + val.initial_value.value.len()];
							val.value.value.copy_from_slice(tmp_val);
						}
						return Some((key, value))
					}
					None
				}).collect();
		}
	}

	pub fn narrow(&mut self, pattern: Pattern, compare_to_initial: bool) {
		unsafe {
			let handle_tmp = self.handle.0 as usize;
			self.update();
			let old_results = mem::take(&mut self.state.results);
			self.state.results = old_results
				.into_par_iter()
				.filter_map(|(key, value)| {
					let mut info: MEMORY_BASIC_INFORMATION = mem::zeroed();
					if let Ok(buffer) = Self::get_chunk(mem::transmute::<usize, HANDLE>(handle_tmp), &self.options, key.0 as _, &mut info) {
						let new = value
							.into_iter()
							.filter(|val| {
								if compare_to_initial {
									pattern.matches(&val.initial_value.value)
								} else {
									pattern.matches(&val.value.value)
								}
							})
							.collect::<Vec<_>>();
						if !new.is_empty() {
							return Some((key, new));
						}
					}
					None
				}).collect();
		}
	}
}