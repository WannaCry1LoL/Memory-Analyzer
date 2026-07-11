use crate::externs::*;
use crate::utils::TriState;
use crate::utils::TriState::{DontCare, False, True};

#[derive(Debug, Clone, Copy)]
pub struct DumpFlags {
	pub writable: TriState,
	pub executable: TriState,
	pub copy_on_write: TriState,
	pub start_address: usize,
	pub end_address: usize
}

impl DumpFlags {
	pub fn page_flags_match(&self, flags: PageProtectionFlags) -> bool {
		self.writable.matches(flags.is_writable())
			&& self.executable.matches(flags.is_executable())
			&& self.copy_on_write.matches(flags.is_cow())
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
	handle: Handle,
	//initial_scan_file: File,
	//scan_file: File,
	flags: DumpFlags
}

impl ScanDiskManager {
	pub fn new(handle: Handle, flags: DumpFlags) -> Self /*std::io::Result<ScanDiskManager>*/ {
		/*
		let mut dir = std::env::temp_dir();
		dir.push("MemScanner");
		match fs::create_dir(&dir) {
			Ok(()) => {}
			Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
			Err(e) => return Err(e),
		}

		let pid = std::process::id();
		let initial_scan_file = OpenOptions::new()
			.read(true).write(true).create(true).truncate(true)
			.open(dir.join(format!("initial_{pid}.dat")))?;

		let scan_file = OpenOptions::new()
			.read(true).write(true).create(true).truncate(true)
			.open(dir.join(format!("scan_{pid}.dat")))?;
		 */

		ScanDiskManager { handle, flags }
	}

	pub fn enumerate_blocks(&self) -> Vec<BlockDescriptor> {
		let max_addr = self.flags.end_address as _;
		let mut blocks: Vec<BlockDescriptor> = Vec::new();
		let mut base_addr = self.flags.start_address as _;

		loop {
			let Ok((info, written)) = virtual_query_ex(self.handle, base_addr) else {
				break
			};

			if written == 0 || base_addr as usize >= max_addr { break; }

			if self.flags.page_flags_match(info.protect)
				&& info.state == MEM_COMMIT
				&& matches!(info.typ, MEM_PRIVATE | MEM_IMAGE)
			{
				let region_start = base_addr as usize;

				match blocks.last_mut() {
					Some(last) if last.virtual_address + last.size == region_start => last.size += info.region_size,
					_ => blocks.push(BlockDescriptor::new(region_start, info.region_size))
				}
			}

			base_addr = unsafe { base_addr.byte_add(info.region_size) };
		}

		let mut curr_offset = size_of::<FileHeader>() + blocks.len() * size_of::<BlockDescriptor>();
		for b in &mut blocks {
			b.file_base = curr_offset;
			curr_offset += b.size;
		}
		blocks
	}
	/*
	pub fn write_header(&mut self, blocks: &[BlockDescriptor]) -> std::io::Result<()> {
		let header = FileHeader::new(blocks.len());
		self.initial_scan_file.seek(SeekFrom::Start(0))?;
		let mut writer = BufWriter::new(&self.initial_scan_file);
		unsafe {
			writer.write_all(slice::from_raw_parts(&raw const header as _, size_of::<FileHeader>()))?;
			if !blocks.is_empty() {
				writer.write_all(slice::from_raw_parts(blocks.as_ptr() as _, blocks.len() * size_of::<BlockDescriptor>()))?;
			}
		}
		writer.flush()
	}

	pub fn write_block(&mut self, desc: &BlockDescriptor, bytes: &[u8]) -> std::io::Result<()> {
		debug_assert_eq!(bytes.len(), desc.size);
		self.initial_scan_file.seek(SeekFrom::Start(desc.file_base as u64))?;
		self.initial_scan_file.write_all(bytes)
	}

	pub fn append_matches(&mut self, bytes: &[u8]) -> std::io::Result<()> {
		self.scan_file.write_all(bytes)
	}
	 */
}

unsafe impl Send for ScanDiskManager {}