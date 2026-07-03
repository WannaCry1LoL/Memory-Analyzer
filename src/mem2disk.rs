use crate::utils::TriState;
use crate::utils::TriState::{DontCare, False, True};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::{fs, slice};
use std::path::PathBuf;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Memory::{VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_IMAGE, MEM_PRIVATE, PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_PROTECTION_FLAGS, PAGE_READWRITE, PAGE_WRITECOPY};

#[derive(Debug, Clone, Copy)]
pub struct DumpFlags {
	pub writable: TriState,
	pub executable: TriState,
	pub copy_on_write: TriState,
	pub start_address: usize,
	pub end_address: usize
}

impl DumpFlags {
	const WRITABLE_FLAGS: u32 = PAGE_READWRITE.0 | PAGE_EXECUTE_READWRITE.0;
	const EXECUTABLE_FLAGS: u32 = PAGE_EXECUTE.0 | PAGE_EXECUTE_READ.0 | PAGE_EXECUTE_READWRITE.0 | PAGE_EXECUTE_WRITECOPY.0;
	const COW_FLAGS: u32 = PAGE_WRITECOPY.0 | PAGE_EXECUTE_WRITECOPY.0;

	fn is_writable(flags: PAGE_PROTECTION_FLAGS) -> bool { flags.0 & Self::WRITABLE_FLAGS != 0 }
	fn is_executable(flags: PAGE_PROTECTION_FLAGS) -> bool { flags.0 & Self::EXECUTABLE_FLAGS != 0 }
	fn is_cow(flags: PAGE_PROTECTION_FLAGS) -> bool { flags.0 & Self::COW_FLAGS != 0 }

	pub fn page_flags_match(&self, flags: PAGE_PROTECTION_FLAGS) -> bool {
		self.writable.matches(Self::is_writable(flags))
			&& self.executable.matches(Self::is_executable(flags))
			&& self.copy_on_write.matches(Self::is_cow(flags))
	}

	pub fn new(writable: TriState, executable: TriState, copy_on_write: TriState, start_address: usize, end_address: usize) -> Self {
		DumpFlags { writable, executable, copy_on_write, start_address, end_address }
	}
}

impl Default for DumpFlags {
	fn default() -> Self {
		DumpFlags {
			writable: True,
			executable: DontCare,
			copy_on_write: False,
			start_address: 0,
			end_address: 0x00007fffffffffff
		}
	}
}

#[repr(C)]
pub struct FileHeader {
	magic: [u8; 4],
	block_count: usize
}

impl FileHeader {
	const MAGIC_BYTES: u32 = 0x44554d50;
	pub fn new(block_count: usize) -> FileHeader {
		FileHeader { magic: Self::MAGIC_BYTES.to_be_bytes(), block_count }
	}
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct BlockDescriptor {
	pub virtual_address: usize,
	pub file_base: usize,
	pub size: usize
}

impl BlockDescriptor {
	pub fn new(virtual_address: usize, size: usize) -> Self {
		BlockDescriptor { virtual_address, file_base: 0, size }
	}
}

pub struct ScanDiskManager {
	handle: HANDLE,
	initial_scan_file: File,
	scan_file: File,
	flags: DumpFlags
}

impl ScanDiskManager {

	fn new(handle: HANDLE, flags: DumpFlags) -> std::io::Result<ScanDiskManager> {
		let mut dir = std::env::temp_dir();
		dir.push("MemScanner");
		fs::create_dir(&dir).unwrap();

		let mut file_1: PathBuf = dir.iter().collect();
		file_1.push("");
		let mut file_2: PathBuf = dir.iter().collect();

		let initial_scan_file =
			OpenOptions::new()
				.read(true)
				.write(true)
				.create_new(true)
				.open(file_1)?;

		let scan_file =
			OpenOptions::new()
				.read(true)
				.write(true)
				.create_new(true)
				.open(file_2)?;

		Ok(ScanDiskManager {
			handle,
			initial_scan_file,
			scan_file,
			flags
		})
	}

	pub fn dump_memory_blocks(&self) -> std::io::Result<Vec<BlockDescriptor>> {
		let max_addr = self.flags.end_address as _;
		let mut raw_blocks = Vec::new();
		let mut base_addr= self.flags.start_address as _;
		let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };

		while base_addr < max_addr {
			unsafe { VirtualQueryEx(
				self.handle,
				Some(base_addr),
				&mut info,
				size_of::<MEMORY_BASIC_INFORMATION>(),
			) };

			if self.flags.page_flags_match(info.Protect)
				&& info.State == MEM_COMMIT
				&& matches!(info.Type, MEM_PRIVATE | MEM_IMAGE) {
				raw_blocks.push(BlockDescriptor::new(base_addr as usize, info.RegionSize));
			}

			base_addr = unsafe { base_addr.byte_add(info.RegionSize) };
		}
		let amount = raw_blocks.len();
		let mut curr_offset = size_of::<FileHeader>() + amount * size_of::<BlockDescriptor>();

		for val in &mut raw_blocks {
			val.file_base = curr_offset;
			curr_offset += val.size;
		}

		let header = FileHeader::new(amount);
		let mut writer = BufWriter::new(&self.initial_scan_file);
		unsafe {
			let header_bytes = slice::from_raw_parts(
				&raw const header as _,
				size_of::<FileHeader>()
			);
			writer.write_all(header_bytes)?;

			if !raw_blocks.is_empty() {
				let block_bytes = slice::from_raw_parts(
					raw_blocks.as_ptr() as _,
					amount * size_of::<BlockDescriptor>()
				);
				writer.write_all(block_bytes)?;
			}
		}
		writer.flush()?;
		Ok(raw_blocks)
	}
}