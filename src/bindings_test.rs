use std::os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, IntoRawHandle, OwnedHandle, RawHandle};
use core::ffi::c_void;
use std::hint::unreachable_unchecked;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum NtStatusType {
	Success = 0,
	Information = 1,
	Warning = 2,
	Error = 3
}

pub trait IntoResult: Sized {
	fn into_result(self) -> Result<(), Self>;
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle(pub *mut c_void);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hwnd(pub isize);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NtStatus(pub i32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Win32Error(pub u32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageProtectionFlags(pub u32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemInformationClass(pub i32);

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}
unsafe impl Send for Hwnd {}
unsafe impl Sync for Hwnd {}

impl AsRawHandle for Handle {
	fn as_raw_handle(&self) -> RawHandle {
		self.0.cast()
	}
}

impl IntoRawHandle for Handle {
	fn into_raw_handle(self) -> RawHandle { self.0.cast() }
}

impl FromRawHandle for Handle {
	unsafe fn from_raw_handle(handle: RawHandle) -> Self { Self(handle.cast()) }
}

impl From<OwnedHandle> for Handle {
	fn from(handle: OwnedHandle) -> Self { Self(handle.into_raw_handle().cast()) }
}

impl<'a> From<BorrowedHandle<'a>> for Handle {
	fn from(handle: BorrowedHandle<'a>) -> Self { Self(handle.as_raw_handle().cast()) }
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

impl Handle {
	pub const INVALID: Self = Self(-1isize as *mut c_void);
	pub const CURRENT_PROCESS: Self = Self::INVALID;
	pub const NULL: Self = Self(core::ptr::null_mut());

	#[inline]
	pub fn is_invalid(&self) -> bool { self.0.is_null() || self.0 == Self::INVALID.0 }
}

impl Hwnd {
	pub const NULL: Self = Self(0);

	#[inline]
	pub fn is_invalid(&self) -> bool {
		self.0 == 0
	}
}

impl NtStatus {
	pub const SUCCESS: Self = Self(0);
	pub const STATUS_INFO_LENGTH_MISMATCH: Self = Self(0xC0000004_u32 as i32);
	#[inline]
	pub const fn get_type(&self) -> NtStatusType {
		match self.0 as u32 >> 30 {
			0 => NtStatusType::Success,
			1 => NtStatusType::Information,
			2 => NtStatusType::Warning,
			3 => NtStatusType::Error,
			_ => unsafe { unreachable_unchecked() }
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
	pub const ERROR_ACCESS_DENIED: Win32Error = Win32Error(5);
	pub const ERROR_INVALID_Handle: Win32Error = Win32Error(6);
	pub const ERROR_NOT_ALL_ASSIGNED: Win32Error = Win32Error(1300);
	#[inline]
	pub const fn is_ok(self) -> bool { self.0 == 0 }
}


impl PageProtectionFlags {
	pub const PAGE_READWRITE: Self = Self(0x04);
	pub const PAGE_WRITECOPY: Self = Self(0x08);
	pub const PAGE_EXECUTE: Self = Self(0x10);
	pub const PAGE_EXECUTE_READ: Self = Self(0x20);
	pub const PAGE_EXECUTE_READWRITE: Self = Self(0x40);
	pub const PAGE_EXECUTE_WRITECOPY: Self = Self(0x80);

	pub const MEM_WRITABLE: Self = Self(Self::PAGE_READWRITE.0 | Self::PAGE_EXECUTE_READWRITE.0);
	pub const MEM_EXECUTABLE: Self = Self(Self::PAGE_EXECUTE.0 | Self::PAGE_EXECUTE_READ.0 | Self::PAGE_EXECUTE_READWRITE.0 | Self::PAGE_EXECUTE_WRITECOPY.0);
	pub const MEM_COW: Self = Self(Self::PAGE_WRITECOPY.0 | Self::PAGE_EXECUTE_WRITECOPY.0);
}

impl SystemInformationClass {
	pub const SYSTEM_BASIC_PROCESS_INFORMATION: Self = Self(252i32);
}

pub const E_ACCESSDENIED: i32 = -2147024891; // 0x80070005 as i32

pub const MEM_COMMIT: u32 = 0x1000;
pub const MEM_PRIVATE: u32 = 0x20000;
pub const MEM_IMAGE: u32 = 0x1000000;

pub const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;
pub const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
pub const TOKEN_QUERY: u32 = 0x0008;

pub const PROCESS_NAME_FORMAT_WIN32: u32 = 0;
pub const PROCESS_ALL_ACCESS: u32 = 0x001F0FFF;
pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

pub const GWL_EXSTYLE: i32 = -20;
pub const WS_EX_TOOLWINDOW: isize = 0x00000080;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
	pub left: i32,
	pub top: i32,
	pub right: i32,
	pub bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Luid {
	pub low_part: u32,
	pub high_part: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct LuidAndAttributes {
	pub luid: Luid,
	pub attributes: u32,
}

#[repr(C)]
pub struct TokenPrivileges {
	pub privilege_count: u32,
	pub privileges: [LuidAndAttributes; 1],
}

#[repr(C)]
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

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct SystemBasicProcessInformation
{
	pub next_entry_offset: u32,
	pub unique_process_id: usize,
	pub inherited_from_unique_process_id: usize,
	pub sequence_number: u64,
	pub image_name: UnicodeString
}


#[link(name = "kernel32")]
unsafe extern "system" {
	pub fn CloseHandle(handle: Handle) -> i32;
	pub fn DuplicateHandle(
		source_process_handle: Handle,
		source_handle: Handle,
		target_process_handle: Handle,
		target_handle: *mut Handle,
		desired_access: u32,
		inherit_handle: i32,
		options: u32
	) -> i32;
	pub fn GetCurrentProcess() -> Handle;
	pub fn GetLastError() -> Win32Error;
	pub fn GetProcessId(process: Handle) -> u32;
	pub fn GetSystemInfo(system_info: *mut SystemInfo);
	pub fn OpenProcess(
		desired_access: u32,
		inherit_handle: i32,
		process_id: u32
	) -> Handle;
	pub fn QueryFullProcessImageNameW(
		process: Handle,
		flags: u32,
		classname: *mut u16,
		pcch: *mut u32
	) -> i32;
	pub fn VirtualQueryEx(
		process: Handle,
		address: *const c_void,
		buffer: *mut MemoryBasicInformation,
		length: usize
	) -> usize;
}

#[link(name = "user32")]
unsafe extern "system" {
	pub fn EnumWindows(
		enum_func: Option<unsafe extern "system" fn(Hwnd, isize) -> i32>,
		param: isize
	) -> i32;
	pub fn GetWindowLongPtrW(
		window: Hwnd,
		index: i32
	) -> isize;
	pub fn GetWindowRect(
		window: Hwnd,
		rect: *mut Rect
	) -> i32;
	pub fn GetWindowTextW(
		window: Hwnd,
		string: *mut u16,
		max_count: i32
	) -> i32;
	pub fn GetWindowThreadProcessId(
		window: Hwnd,
		process_id: *mut u32
	) -> u32;
	pub fn IsIconic(window: Hwnd) -> i32;
	pub fn IsWindowVisible(window: Hwnd) -> i32;
}

#[link(name = "advapi32")]
unsafe extern "system" {
	pub fn AdjustTokenPrivileges(
		token_handle: Handle,
		disable_all_privileges: i32,
		new_state: *const TokenPrivileges,
		buffer_length: u32,
		previous_state: *mut TokenPrivileges,
		return_length: *mut u32
	) -> i32;
	pub fn LookupPrivilegeValueW(
		system_name: *const u16,
		name: *const u16,
		luid: *mut Luid
	) -> i32;
	pub fn OpenProcessToken(
		process_handle: Handle,
		desired_access: u32,
		token_handle: *mut Handle
	) -> i32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
	pub fn RtlNtStatusToDosError(status: NtStatus) -> u32;
	pub fn NtSuspendProcess(process: Handle) -> NtStatus;
	pub fn NtResumeProcess(process: Handle) -> NtStatus;

	pub fn NtQuerySystemInformation(
		system_information_class: crate::externs::SystemInformationClass,
		system_information: *mut c_void,
		system_information_length: u32,
		return_length: *mut u32
	) -> NtStatus;

	pub fn NtGetNextThread(
		process: Handle,
		thread: Handle,
		access_mask: u32,
		attributes: u32,
		flags: u32,
		new_handle: *mut Handle
	) -> NtStatus;

	pub fn NtGetNextProcess(
		process: Handle,
		access_mask: u32,
		attributes: u32,
		flags: u32,
		new_handle: *mut Handle
	) -> NtStatus;

	pub fn NtReadVirtualMemory(
		process: Handle,
		address: *const c_void,
		buffer: *mut c_void,
		num_to_read: usize,
		num_read: *mut usize,
	) -> NtStatus;

	pub fn NtWriteVirtualMemory(
		process: Handle,
		address: *const c_void,
		buffer: *const c_void,
		num_to_write: usize,
		num_written: *mut usize
	) -> NtStatus;
}