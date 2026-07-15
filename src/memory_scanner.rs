use std::mem::MaybeUninit;
use std::slice;
use rayon::iter::ParallelIterator;
use rayon::prelude::*;
use crate::errors::MemoryScanError;
use crate::matches::MatchingState;
use crate::mem2disk::{BlockDescriptor, DumpFlags, ScanDiskManager};
use crate::num_helpers::{ArrayScanExt, FromU128, ScanCondition, Scannable};
use crate::process::Process;
use crate::simd_types::ToSimd;
use crate::utils::{get, MatchMask};

#[derive(Debug, Clone, Copy)]
pub enum IntCondition {
	Eq(i128),
	NotEq(i128),
	Range(i128, i128),
	GreaterThan(i128),
	LessThan(i128),
	FuzzyEq(i128, i128),
}

#[derive(Debug, Clone, Copy)]
pub enum FloatCondition {
	Eq(f64),
	NotEq(f64),
	Range(f64, f64),
	GreaterThan(f64),
	LessThan(f64),
	FuzzyEq(f64, f64),
}

#[derive(Debug, Clone)]
pub enum ValueType {
	Byte(IntCondition),
	TwoBytes(IntCondition),
	FourBytes(IntCondition),
	EightBytes(IntCondition),
	Float(FloatCondition),
	Double(FloatCondition),
	ByteArray(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct ScanRequest {
	pub value_type: ValueType,
	pub compare_to_first_scan: bool,
	pub freeze_process: bool
}

fn to_bit_pattern<T: Scannable + FromU128>(x: i128) -> T {
	let bits = <T as ToSimd>::SIZE * 8;
	let range_min = -(1i128 << (bits - 1));
	let range_max = (1i128 << bits) - 1;
	T::from_u128(x.clamp(range_min, range_max) as _)
}

impl<T: Scannable + FromU128> From<IntCondition> for ScanCondition<T> {
	fn from(x: IntCondition) -> Self {
		match x {
			IntCondition::Range(lo, hi) => ScanCondition::from_logical_range(lo, hi),
			IntCondition::Eq(n) => ScanCondition::<T>::Eq(to_bit_pattern(n)),
			IntCondition::NotEq(n) => ScanCondition::<T>::NotEq(to_bit_pattern(n)),
			IntCondition::GreaterThan(n) => ScanCondition::<T>::from_greater_than(to_bit_pattern(n)),
			IntCondition::LessThan(n) => ScanCondition::<T>::from_less_than(to_bit_pattern(n)),
			IntCondition::FuzzyEq(n, eps) => ScanCondition::<T>::FuzzyEq(to_bit_pattern(n), to_bit_pattern(eps)),
		}
	}
}

impl From<FloatCondition> for ScanCondition<f32> {
	fn from(x: FloatCondition) -> Self {
		match x {
			FloatCondition::Eq(n) => ScanCondition::Eq(n as f32),
			FloatCondition::NotEq(n) => ScanCondition::NotEq(n as f32),
			FloatCondition::Range(lo, hi) => ScanCondition::Range(lo as f32, hi as f32),
			FloatCondition::LessThan(n) => ScanCondition::<f32>::from_less_than(n as f32),
			FloatCondition::GreaterThan(n) => ScanCondition::<f32>::from_greater_than(n as f32),
			FloatCondition::FuzzyEq(n, eps) => ScanCondition::FuzzyEq(n as f32, eps as f32),
		}
	}
}

impl From<FloatCondition> for ScanCondition<f64> {
	fn from(x: FloatCondition) -> Self {
		match x {
			FloatCondition::Eq(n) => ScanCondition::Eq(n),
			FloatCondition::NotEq(n) => ScanCondition::NotEq(n),
			FloatCondition::Range(lo, hi) => ScanCondition::Range(lo, hi),
			FloatCondition::LessThan(n) => ScanCondition::from_less_than(n),
			FloatCondition::GreaterThan(n) => ScanCondition::from_greater_than(n),
			FloatCondition::FuzzyEq(n, eps) => ScanCondition::FuzzyEq(n, eps),
		}
	}
}

impl ScanRequest {
	pub fn default_alignment(&self) -> usize {
		match &self.value_type {
			ValueType::Byte(_) => 1,
			ValueType::TwoBytes(_) => 2,
			ValueType::FourBytes(_) | ValueType::EightBytes(_) | ValueType::Float(_) | ValueType::Double(_)=> 4,
			ValueType::ByteArray(_) => 1,
		}
	}

	fn element_size(&self) -> usize {
		match &self.value_type {
			ValueType::Byte(_) => size_of::<u8>(),
			ValueType::TwoBytes(_) => size_of::<u16>(),
			ValueType::FourBytes(_) => size_of::<u32>(),
			ValueType::EightBytes(_) => size_of::<u64>(),
			ValueType::Float(_) => size_of::<f32>(),
			ValueType::Double(_) => size_of::<f64>(),
			ValueType::ByteArray(p) => p.len(),
		}
	}

	fn run(&self, bytes: &[u8], alignment: usize) -> MatchMask {
		match &self.value_type {
			ValueType::Byte(c) => bytes.scan_for::<u8>((*c).into(), alignment),
			ValueType::TwoBytes(c) => bytes.scan_for::<u16>((*c).into(), alignment),
			ValueType::FourBytes(c) => bytes.scan_for::<u32>((*c).into(), alignment),
			ValueType::EightBytes(c) => bytes.scan_for::<u64>((*c).into(), alignment),
			ValueType::Float(c) => bytes.scan_for::<f32>((*c).into(), alignment),
			ValueType::Double(c) => bytes.scan_for::<f64>((*c).into(), alignment),
			ValueType::ByteArray(p) => bytes.find_pattern(p, alignment),
		}
	}

	fn test_bytes(&self, bytes: &[u8]) -> bool {
		match &self.value_type {
			ValueType::Byte(c) => u8::matches(&(*c).into(), u8::from_bytes(bytes)),
			ValueType::TwoBytes(c) => u16::matches(&(*c).into(), u16::from_bytes(bytes)),
			ValueType::FourBytes(c) => u32::matches(&(*c).into(), u32::from_bytes(bytes)),
			ValueType::EightBytes(c) => u64::matches(&(*c).into(), u64::from_bytes(bytes)),
			ValueType::Float(c) => f32::matches(&(*c).into(), f32::from_bytes(bytes)),
			ValueType::Double(c) => f64::matches(&(*c).into(), f64::from_bytes(bytes)),
			ValueType::ByteArray(pattern) => bytes == pattern.as_slice(),
		}
	}
}


#[derive(Debug, Clone)]
pub struct ScanOptions {
	pub use_custom_alignment: bool,
	pub custom_alignment: usize
}

impl Default for ScanOptions {
	fn default() -> Self {
		ScanOptions {
			use_custom_alignment: false,
			custom_alignment: 4
		}
	}
}

#[derive(Debug, Clone, Default)]
pub struct MemoryScannerOptions {
	pub scan_options: ScanOptions,
	pub dump_options: DumpFlags
}

impl MemoryScannerOptions {
	pub fn resolve_alignment(&self, or: &ScanRequest) -> usize {
		if self.scan_options.use_custom_alignment { self.scan_options.custom_alignment } else { or.default_alignment() }
	}
}

pub struct MemoryScanner {
	pub file_manager: ScanDiskManager,
	pub process: Process,
	pub state: MatchingState,
	pub options: MemoryScannerOptions,
}

// TODO: narrow scans

impl MemoryScanner {
	pub fn new(mut process: Process, options: MemoryScannerOptions) -> Result<Self, std::io::Error> {
		process
			.init_handle()
			.map_err(|e| std::io::Error::new(std::io::ErrorKind::PermissionDenied, format!("{:?}", e)))?;
		let handle = process.get_handle();

		let file_manager = ScanDiskManager::new(handle, options.dump_options);

		Ok(Self {
			file_manager,
			process,
			state: MatchingState::new(1, 1, true),
			options,
		})
	}

	pub fn scan(&mut self, request: ScanRequest) -> Result<(), MemoryScanError> {
		self.process.init_handle().map_err(|_| MemoryScanError::HandleInitFailed)?;

		let _guard = request.freeze_process.then(|| self.process.suspend_guard().ok()).flatten();

		let alignment = self.options.resolve_alignment(&request);
		let element_size = request.element_size();
		let blocks = self.file_manager.enumerate_blocks();
		let mut new_state = MatchingState::new(alignment, element_size, true);

		let process = &self.process;
		let mut capacity = 0;
		let mut block_offsets = Vec::with_capacity(blocks.len());
		for block in &blocks {
			block_offsets.push(capacity);
			capacity += block.size;
		}

		let mut main_buffer = Vec::<u8>::with_capacity(capacity);
		let mut remaining_buf = main_buffer.spare_capacity_mut();
		let mut chunk_slices = Vec::with_capacity(blocks.len());

		for block in &blocks {
			let (left, right) = remaining_buf.split_at_mut(block.size);
			chunk_slices.push(left);
			remaining_buf = right;
		}

		let results: Vec<(usize, MatchMask, Vec<u8>)> = blocks
			.par_iter()
			.zip(chunk_slices)
			.filter_map(|(desc, buf)| {
				let read = process.get_chunk_into(desc, buf).ok()?;
				let mem = unsafe { buf[..read].assume_init_ref() };

				let mask = request.run(mem, alignment);
				let ones = mask.count_ones();
				if ones == 0 { return None; }

				let mut matched: Vec<u8> = Vec::with_capacity(ones * element_size);
				for i in mask.iter_ones() {
					matched.extend_from_slice(get!(mem, i * alignment .. i * alignment + element_size));
				}

				Some((desc.virtual_address, mask, matched))
			})
			.collect();

		for (addr, mask, matched) in results {
			new_state.push_block(addr, mask, &matched);
		}

		self.state = new_state;
		Ok(())
	}

	pub fn narrow(&mut self, request: ScanRequest) -> Result<(), MemoryScanError> {
		let alignment = self.state.alignment;
		let elem_size = request.element_size();

		if elem_size != self.state.elem_size {
			return Err(MemoryScanError::IncompatibleScanType);
		}

		let _guard = request.freeze_process.then(|| self.process.suspend_guard().ok()).flatten();

		let mut new_state = MatchingState::new(alignment, elem_size, false);

		let process = &self.process;
		let state = &self.state;

		let initial = if state.initial_scan { &state.data } else { &state.initial };

		let mut capacity = 0;
		for block in &state.blocks {
			let mask_len = block.match_mask.len();
			if mask_len > 0 {
				capacity += (mask_len - 1) * alignment + elem_size;
			}
		}

		let mut main_buffer = Vec::<u8>::with_capacity(capacity);
		let mut remaining_buf = main_buffer.spare_capacity_mut();
		let mut chunk_slices = Vec::with_capacity(state.blocks.len());

		for block in &state.blocks {
			let mask_len = block.match_mask.len();
			if mask_len == 0 {
				chunk_slices.push(&mut [] as _);
				continue;
			}
			let (left, right) = remaining_buf.split_at_mut((mask_len - 1) * alignment + elem_size);
			chunk_slices.push(left);
			remaining_buf = right;
		}

		let results: Vec<(usize, MatchMask, Vec<u8>, Vec<u8>)> = state.blocks
			.par_iter()
			.zip(chunk_slices)
			.filter_map(|(block, buf)| {
				let mask_len = block.match_mask.len();
				if mask_len == 0 { return None; }

				let read_size = (mask_len - 1) * alignment + elem_size;
				let desc = BlockDescriptor::new(block.base_address, read_size);

				let read = process.get_chunk_into(&desc, buf).ok()?;
				let mem = unsafe { buf[..read].assume_init_ref() };

				let mut new_mask = MatchMask::repeat(false, mask_len);

				let old_count = block.match_mask.count_ones();
				let mut current_values = Vec::with_capacity(old_count * elem_size);
				let mut initial_values = Vec::with_capacity(old_count * elem_size);

				for (k, i) in block.match_mask.iter_ones().enumerate() {
					let offset = i * alignment;
					if offset + elem_size > mem.len() {
						break;
					}

					let bytes = get!(mem, offset .. offset + elem_size);

					if request.test_bytes(bytes) {
						new_mask.set(i, true);
						current_values.extend_from_slice(bytes);

						let old_off = block.data_offset + k * elem_size;
						initial_values.extend_from_slice(get!(initial, old_off .. old_off + elem_size));
					}
				}

				if new_mask.count_ones() == 0 { return None; }

				Some((block.base_address, new_mask, initial_values, current_values))
			})
			.collect();

		for (addr, mask, initial_values, current_values) in results {
			new_state.push_narrowed_block(addr, mask, &initial_values, &current_values);
		}

		new_state.data.shrink_to_fit();
		new_state.initial.shrink_to_fit();
		self.state = new_state;

		Ok(())
	}
}