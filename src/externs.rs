use std::os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, IntoRawHandle, OwnedHandle, RawHandle};
use core::ffi::c_void;
use std::hint::unreachable_unchecked;
use std::ptr::{null, null_mut};
use crate::utils::Pid;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Handle(pub *mut c_void);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
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
	pub const NULL: Self = Self(null_mut());

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
	pub const STATUS_INFO_LENGTH_MISMATCH: Self = Self(0xC0000004_u32 as _);
	pub const STATUS_ACCESS_DENIED: Self = Self(0xC0000022_u32 as _);
	pub const STATUS_INVALID_HANDLE: Self = Self(0xC0000008_u32 as _);
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
	pub const ERROR_INVALID_HANDLE: Win32Error = Win32Error(6);
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

	pub fn is_writable(&self) -> bool { self.0 & Self::MEM_WRITABLE.0 != 0 }
	pub fn is_executable(&self) -> bool { self.0 & Self::MEM_EXECUTABLE.0 != 0 }
	pub fn is_cow(&self) -> bool { self.0 & Self::MEM_COW.0 != 0 }
}

impl SystemInformationClass {
	pub const SYSTEM_BASIC_PROCESS_INFORMATION: Self = Self(252i32);
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

pub const OBJ_INHERIT: u32 = 0x00000002;

pub const PROCESS_IMAGE_FILENAME_WIN32: u32 = 43;

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct ClientId {
	pub unique_process: Handle,
	pub unique_thread: Handle
}

#[repr(C)]
pub struct ObjectAttributes {
	pub length: u32,
	pub root_directory: Handle,
	pub object_name: *const UnicodeString,
	pub attributes: u32,
	pub security_descriptor: *mut c_void,
	pub security_quality_of_service: *mut c_void
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
	pub left: i32,
	pub top: i32,
	pub right: i32,
	pub bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Luid {
	pub low_part: u32,
	pub high_part: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LuidAndAttributes {
	pub luid: Luid,
	pub attributes: u32,
}

#[repr(C)]
#[derive(Default)]
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

#[repr(C)]
pub struct SystemBasicProcessInformation
{
	pub next_entry_offset: u32,
	pub unique_process_id: usize,
	pub inherited_from_unique_process_id: usize,
	pub sequence_number: u64,
	pub image_name: UnicodeString
}

#[repr(C)]
pub struct ProcessBasicInformation {
	pub exit_status: i32,
	pub peb_base_address: *mut c_void,
	pub affinity_mask: usize,
	pub base_priority: i32,
	pub unique_process_id: usize,
	pub inherited_from_unique_process_id: usize,
}

#[repr(C)]
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

pub unsafe fn open_process(
	desired_access: u32,
	inherit_handle: bool,
	process_id: u32,
) -> Result<Handle, NtStatus> {
	let mut handle: Handle = Default::default();

	let mut client_id = ClientId {
		unique_process: Handle(process_id as _),
		unique_thread: Default::default(),
	};

	let mut object_attributes = ObjectAttributes {
		length: size_of::<ObjectAttributes>() as u32,
		root_directory: Default::default(),
		object_name: Default::default(),
		attributes: if inherit_handle { OBJ_INHERIT } else { 0 },
		security_descriptor: null_mut(),
		security_quality_of_service: null_mut(),
	};

	unsafe {
		let status = NtOpenProcess(
			&mut handle,
			desired_access,
			&mut object_attributes,
			&mut client_id,
		);

		status.into_result().map(|_| handle)
	}
}

pub unsafe fn open_process_token(
	process_handle: Handle,
	desired_access: u32,
) -> Result<Handle, NtStatus> {
	let mut token_handle: Handle = Default::default();

	unsafe {
		NtOpenProcessToken(
			process_handle,
			desired_access,
			&mut token_handle,
		).into_result().map(|_| token_handle)
	}
}

pub unsafe fn adjust_token_privileges(
	token_handle: Handle,
	disable_all_privileges: bool,
	new_state: Option<&TokenPrivileges>,
	buffer_length: u32,
	previous_state: Option<&mut TokenPrivileges>,
	return_length: Option<&mut u32>,
) -> Result<(), NtStatus> {
	let new_state_ptr = new_state
		.map_or(null(), |r| r as *const TokenPrivileges);

	let previous_state_ptr = previous_state
		.map_or(null_mut(), |r| r as *mut TokenPrivileges);

	let return_length_ptr = return_length
		.map_or(null_mut(), |r| r as *mut u32);

	unsafe {
		NtAdjustPrivilegesToken(
			token_handle,
			disable_all_privileges as u8,
			new_state_ptr,
			buffer_length,
			previous_state_ptr,
			return_length_ptr,
		).into_result()
	}
}

pub fn virtual_query_ex(
	process_handle: Handle,
	base_address: *const c_void,
) -> Result<(MemoryBasicInformation, usize), NtStatus> {

	let mut mem_info = unsafe { std::mem::zeroed() };
	let mut return_length = 0;

	let status = unsafe {
		NtQueryVirtualMemory(
			process_handle,
			base_address,
			0,
			&raw mut mem_info as _,
			size_of::<MemoryBasicInformation>(),
			&mut return_length,
		)
	};

	status.into_result().map(|_| (mem_info, return_length))
}

pub unsafe fn get_process_id(process_handle: Handle) -> Result<Pid, NtStatus> { unsafe {
	let mut pbi = std::mem::zeroed::<ProcessBasicInformation>();
	let mut return_length = 0;

	let status = NtQueryInformationProcess(
		process_handle,
		0,
		&raw mut pbi as _,
		size_of::<ProcessBasicInformation>() as u32,
		&mut return_length,
	);

	status.into_result().map(|_| (pbi.unique_process_id as u32).into())
} }

pub unsafe fn query_full_process_image_name(
	process_handle: Handle,
	buffer: &mut [u16],
) -> Result<u32, NtStatus> { unsafe {
	let buff_len = buffer.len() * 2;
	let mut return_length = 0;

	let status = NtQueryInformationProcess(
		process_handle,
		PROCESS_IMAGE_FILENAME_WIN32,
		buffer.as_mut_ptr() as _,
		buff_len as _,
		&mut return_length,
	);

	status.into_result().map(|_| return_length)
} }

pub unsafe fn get_system_info() -> Result<SystemBasicInformation, NtStatus> { unsafe {
	let mut sbi = std::mem::zeroed::<SystemBasicInformation>();
	let mut return_length = 0;

	NtQuerySystemInformation(
		SystemInformationClass(0),
		&raw mut sbi as _,
		size_of::<SystemBasicInformation>() as u32,
		&mut return_length,
	).into_result().map(|_| sbi)
} }


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
	pub fn LookupPrivilegeValueW(
		system_name: *const u16,
		name: *const u16,
		luid: *mut Luid
	) -> i32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
	pub fn RtlGetLastWin32Error() -> Win32Error;
	pub fn RtlGetLastNtStatus() -> NtStatus;
	pub fn RtlNtStatusToDosError(status: NtStatus) -> Win32Error;
	pub fn NtSuspendProcess(process: Handle) -> NtStatus;
	pub fn NtResumeProcess(process: Handle) -> NtStatus;

	pub fn NtQuerySystemInformation(
		system_information_class: SystemInformationClass,
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

	pub fn NtOpenProcess(
		process_handle: *mut Handle,
		desired_access: u32,
		object_attributes: *mut ObjectAttributes,
		client_id: *mut ClientId,
	) -> NtStatus;

	pub fn NtOpenProcessToken(
		process_handle: Handle,
		desired_access: u32,
		token_handle: *mut Handle
	) -> NtStatus;

	pub fn NtAdjustPrivilegesToken(
		token_handle: Handle,
		disable_all_privileges: u8,
		new_state: *const TokenPrivileges,
		buffer_length: u32,
		previous_state: *mut TokenPrivileges,
		return_length: *mut u32,
	) -> NtStatus;

	pub fn NtQueryVirtualMemory(
		process_handle: Handle,
		base_address: *const c_void,
		memory_information_class: u32,
		memory_information: *mut c_void,
		memory_information_length: usize,
		return_length: *mut usize,
	) -> NtStatus;

	pub fn NtClose(handle: Handle) -> NtStatus;

	pub fn NtQueryInformationProcess(
		process_handle: Handle,
		process_information_class: u32,
		process_information: *mut c_void,
		process_information_length: u32,
		return_length: *mut u32,
	) -> NtStatus;
}