use crate::errors::{MemoryReadError, MemoryScanError};
use crate::matches::MatchingState;
use crate::mem2disk::{BlockDescriptor, DumpFlags, ScanDiskManager};
use crate::win_helpers::read_bytes;
use windows::Win32::Foundation::HANDLE;

#[derive(Debug, Clone)]
pub enum ValueType {
	Binary,
	Byte,
	TwoBytes,
	FourBytes,
	// EightBytes,
	Float,
	Double,
	ByteArray
}

impl ValueType {
	pub const fn default_alignment(&self) -> usize {
		match self {
			ValueType::Binary | ValueType::Byte | ValueType::ByteArray => 1,
			ValueType::TwoBytes => 2,
			ValueType::FourBytes | ValueType::Float | ValueType::Double => 4,
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

#[derive(Debug, Clone)]
pub struct MemoryScannerOptions {
	pub scan_options: ScanOptions,
	pub dump_options: DumpFlags,
	pub value_type: ValueType
}

impl MemoryScannerOptions {
	const fn get_alignment(&self) -> usize {
		if self.scan_options.use_custom_alignment { self.scan_options.custom_alignment } else { self.value_type.default_alignment() }
	}
}

pub struct MemoryScanner {
	pub file_manager: ScanDiskManager,
	pub handle: HANDLE,
	pub state: MatchingState,
	pub options: MemoryScannerOptions,
}
// TODO: figure out how to represent scan pass
// TODO: write scan pass to disk
// TODO: narrow scans

impl MemoryScanner {
	#[inline]
	unsafe fn get_chunk(&self, block_descriptor: BlockDescriptor) -> Result<Vec<u8>, MemoryReadError> {
		let mut buffer = vec![0u8; block_descriptor.size];

		let res = read_bytes(
			self.handle,
			block_descriptor.virtual_address as _,
			buffer.as_mut_ptr(),
			block_descriptor.size
		);

		res?;
		Ok(buffer)
	}


	pub fn scan(&mut self, ) -> Result<(), MemoryScanError> {
		let res = self.file_manager.dump_memory_blocks().map_err(|e| MemoryScanError::MemoryDumpFailed(e))?;
		let alignment = self.options.get_alignment();
		let mut new_state: MatchingState = MatchingState::new(alignment);

		for desc in res {
			let mem = unsafe { self.get_chunk(desc).unwrap_unchecked() } ; // TODO

		}

		self.state = new_state;
		Ok(())
	}
}