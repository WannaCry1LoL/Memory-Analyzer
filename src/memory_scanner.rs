use std::{f16, f128, mem};
use std::cmp::PartialEq;
use std::collections::{HashMap, HashSet};
use std::env::var;
use std::ffi::c_void;
use std::fmt::LowerHex;
use std::iter::Map;
use std::ptr::null_mut;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Memory::{PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_EXECUTE, PAGE_PROTECTION_FLAGS, PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY, PAGE_TARGETS_INVALID, MEMORY_BASIC_INFORMATION, VirtualQueryEx, MEM_COMMIT};
use crate::errors::MemoryReadError;
use crate::matches::{Match, MatchLocation, MatchType, MatchValue, RegionId};
use crate::memory_pattern::MemoryPattern;
use crate::rw_helper::read_bytes;
use crate::utils::{get_sys_info, TriState};
use crate::utils::TriState::{DontCare, False, True};

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
		let mut read = &buffer[..size];
		match &self {
			Self::Wildcard => true,
			Self::Gap(n) => true,
			Self::Literal(val) => read.first().unwrap() == val,
			Self::Literal2(val) => unsafe {
				std::ptr::read_unaligned(read.as_ptr() as *const u16) == *val
			},
			Self::Literal4(val) => unsafe {
				std::ptr::read_unaligned(read.as_ptr() as *const u32) == *val
			},
			Self::Literal8(val) => unsafe {
				std::ptr::read_unaligned(read.as_ptr() as *const u64) == *val
			},
			Self::Literal16(val) => unsafe {
				std::ptr::read_unaligned(read.as_ptr() as *const u128) == *val
			},
			Self::Range { lower, upper } => {
				let tmp = read.first().unwrap();
				tmp >= lower && tmp <= upper
			},
			Self::Half(expected) => {
				let val = f16::from_ne_bytes(<[u8; 2]>::try_from(read).unwrap());
				(val - expected).abs() < 0.05
			},
			Self::Single(expected) => {
				let val = f32::from_ne_bytes(<[u8; 4]>::try_from(read).unwrap());
				(val - expected).abs() < 0.01
			},
			Self::Double(expected) => {
				let val = f64::from_ne_bytes(<[u8; 8]>::try_from(read).unwrap());
				(val - expected).abs() < 0.01
			},
			Self::Quad(expected) => {
				let val = f128::from_ne_bytes(<[u8; 16]>::try_from(read).unwrap());
				(val - expected).abs() < 0.01
			}
			Self::Repeated { pattern, amount } => {
				let mut chunks = read.chunks(pattern.size());
				chunks.len() == *amount && chunks.all(|chunk| pattern.matches(chunk))
			},
			Self::Not(pattern) => !pattern.matches(read),
			Self::Or(patterns) => patterns.iter().any(|pattern| pattern.matches(read)),
			Self::Mask { value, mask } => {
				let byte = read.first().unwrap();
				byte & mask == *value
			},
			Self::List(patterns) => {
				for pattern in patterns {
					if !pattern.matches(read) { return false };
					read = &read[..size + pattern.size()]
				}
				true
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

		})
	}
	fn get_chunk_info(&self, base_addr: *const c_void, info: &mut MEMORY_BASIC_INFORMATION) {
		unsafe {
			VirtualQueryEx(
				self.handle,
				Some(base_addr),
				info,
				size_of::<MEMORY_BASIC_INFORMATION>(),
			);
		}
	}

	fn get_chunk(&self, base_addr: *const c_void, info: &mut MEMORY_BASIC_INFORMATION) -> Result<Vec<u8>, MemoryReadError> {
		self.get_chunk_info(base_addr, info);
		if !(info.State == MEM_COMMIT && self.options.check_page_flags(info.Protect)) {
			return Err(MemoryReadError::UnsuitableMemory);
		}
		let mut buffer: Vec<u8> = vec![0u8; info.RegionSize];


		let res = read_bytes(self.handle, info.BaseAddress as _, buffer.as_mut_ptr(), info.RegionSize);
		res?;
		Ok(buffer)
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

			while base_addr < si.lpMaximumApplicationAddress {
				let res = self.get_chunk(base_addr, &mut info);
				if let Ok(buffer) = res {
					for i in (0..info.RegionSize - pattern.size()).step_by(increment) {
						let slice = &buffer[i..i + pattern.size()];
						if pattern.matches(slice) {
							new_state.results
								.entry(RegionId(base_addr as usize)).or_insert(Default::default())
								.push(Match::new(pattern.match_type, i, slice, slice));
						}
					}
				}

				base_addr = info.BaseAddress.add(info.RegionSize);
			}
		}
		self.state = new_state;
	}

	pub fn update(&mut self) {
		unsafe {
			let mut info: MEMORY_BASIC_INFORMATION = mem::zeroed();
			let old_results = std::mem::take(&mut self.state.results);
			self.state.results = old_results
				.into_iter()
				.filter_map(|(key, mut value)| {
					if let Ok(buffer) = &self.get_chunk(key.0 as _, &mut info) {
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
			let mut info: MEMORY_BASIC_INFORMATION = mem::zeroed();
			self.update();
			let old_results = mem::take(&mut self.state.results);
			self.state.results = old_results
				.into_iter()
				.filter_map(|(key, mut value)| {
					if let Ok(buffer) = &self.get_chunk(key.0 as _, &mut info) {
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
						if new.len() > 0 {
							return Some((key, new));
						}
					}
					None
				}).collect();
		}
	}
}