use core::ffi::c_void;
use std::os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, IntoRawHandle, OwnedHandle, RawHandle};

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum NtStatusType {
	Success = 0,
	Information = 1,
	Warning = 2,
	Error = 3,
}

pub trait IntoResult: Sized {
	fn into_result(self) -> Result<(), Self>;
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Hash)]
pub struct Pid(pub u32);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Hash)]
pub struct Handle(pub usize);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Hwnd(pub isize);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct NtStatus(pub i32);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Win32Error(pub u32);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct PageProtectionFlags(pub u32);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct SystemInformationClass(pub i32);

impl AsRawHandle for Handle {
	fn as_raw_handle(&self) -> RawHandle { self.0 as _ }
}

impl IntoRawHandle for Handle {
	fn into_raw_handle(self) -> RawHandle { self.0 as _ }
}

impl FromRawHandle for Handle {
	unsafe fn from_raw_handle(handle: RawHandle) -> Self { Self(handle as usize) }
}

impl From<OwnedHandle> for Handle {
	fn from(handle: OwnedHandle) -> Self { Self(handle.into_raw_handle() as _) }
}

impl<'a> From<BorrowedHandle<'a>> for Handle {
	fn from(handle: BorrowedHandle<'a>) -> Self { Self(handle.as_raw_handle() as _) }
}

impl IntoResult for NtStatus {
	fn into_result(self) -> Result<(), Self> {
		if self.is_ok() { Ok(()) } else { Err(self) }
	}
}

impl IntoResult for Win32Error {
	fn into_result(self) -> Result<(), Self> {
		if self.is_ok() { Ok(()) } else { Err(self) }
	}
}

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

impl Handle {
	pub const CURRENT_PROCESS: Self = Self(-1isize as _);
	pub const NULL: Self = Self(0);

	#[inline]
	pub fn is_invalid(&self) -> bool { self.0 == Self::NULL.0 }
}

impl Hwnd {
	pub const NULL: Self = Self(0);

	#[inline]
	pub fn is_invalid(&self) -> bool { self.0 == 0 }
}

impl NtStatus {

	#[inline]
	pub const fn get_type(&self) -> NtStatusType {
		match self.0 as u32 >> 30 {
			0 => NtStatusType::Success,
			1 => NtStatusType::Information,
			2 => NtStatusType::Warning,
			_ => NtStatusType::Error
		}
	}

	#[inline]
	pub const fn is_success(&self) -> bool { (self.0 as u32) >> 30 == 0 }
	#[inline]
	pub const fn is_information(&self) -> bool { (self.0 as u32) >> 30 == 1 }
	#[inline]
	pub const fn is_warn(&self) -> bool { (self.0 as u32) >> 30 == 2 }
	#[inline]
	pub const fn is_error(&self) -> bool { (self.0 as u32) >> 30 == 3 }
	#[inline]
	pub const fn is_ok(&self) -> bool { self.is_success() }
}

impl Win32Error {
	#[inline]
	pub const fn is_ok(self) -> bool { self.0 == 0 }
}

impl PageProtectionFlags {
	pub fn is_writable(&self) -> bool {
		self.0 & Self::MEM_WRITABLE.0 != 0
	}

	pub fn is_executable(&self) -> bool {
		self.0 & Self::MEM_EXECUTABLE.0 != 0
	}
	pub fn is_cow(&self) -> bool {
		self.0 & Self::MEM_COW.0 != 0
	}
}

impl std::fmt::Display for UnicodeString {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if self.buffer.is_null() || self.length == 0 {
			return Ok(());
		}

		let len = self.length as usize / 2;

		let slice = unsafe { std::slice::from_raw_parts(self.buffer, len) };

		let string = String::from_utf16_lossy(slice);

		write!(f, "{}", string)
	}
}


#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct ClientId {
	pub unique_process: Handle,
	pub unique_thread: Handle,
}

#[repr(C)]
pub struct ObjectAttributes {
	pub length: u32,
	pub root_directory: Handle,
	pub object_name: *const UnicodeString,
	pub attributes: u32,
	pub security_descriptor: *mut c_void,
	pub security_quality_of_service: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Rect {
	pub left: i32,
	pub top: i32,
	pub right: i32,
	pub bottom: i32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Luid {
	pub low_part: u32,
	pub high_part: i32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct LuidAndAttributes {
	pub luid: Luid,
	pub attributes: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct TokenPrivileges {
	pub privilege_count: u32,
	// This should be a VLA, but VLAs are difficult to express in rust
	// Instead we just declare it as length 1 because we only ever need the first member anyway
	pub privileges: [LuidAndAttributes; 1],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct SystemInfo {
	pub processor_architecture: u16,
	reserved: u16,
	pub page_size: u32,
	pub minimum_application_address: *mut c_void,
	pub maximum_application_address: *mut c_void,
	pub active_processor_mask: usize,
	pub number_of_processors: u32,
	pub processor_type: u32,
	pub allocation_granularity: u32,
	pub processor_level: u16,
	pub processor_revision: u16,
}

#[repr(C)]
pub struct UnicodeString {
	pub length: u16,
	pub maximum_length: u16,
	pub buffer: *mut u16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct MemoryBasicInformation {
	pub base_address: *mut c_void,
	pub allocation_base: *mut c_void,
	pub allocation_protect: PageProtectionFlags,
	pub partition_id: u16,
	pub region_size: usize,
	pub state: u32,
	pub protect: PageProtectionFlags,
	pub typ: u32,
}

#[repr(C)]
pub struct SystemBasicProcessInformation {
	pub next_entry_offset: u32,
	pub unique_process_id: usize,
	pub inherited_from_unique_process_id: usize,
	pub sequence_number: u64,
	pub image_name: UnicodeString,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct ProcessBasicInformation {
	pub exit_status: i32,
	pub peb_base_address: *mut c_void,
	pub affinity_mask: usize,
	pub base_priority: i32,
	pub unique_process_id: usize,
	pub inherited_from_unique_process_id: usize,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct SystemBasicInformation {
	pub reserved: u32,
	pub timer_resolution: u32,
	pub page_size: u32,
	pub number_of_physical_pages: u32,
	pub lowest_physical_page_number: u32,
	pub highest_physical_page_number: u32,
	pub allocation_granularity: u32,
	pub minimum_user_va: usize,
	pub maximum_user_va: usize,
	pub active_processors_affinity_mask: usize,
	pub number_of_processors: u8,
}