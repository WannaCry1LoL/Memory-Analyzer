use std::ffi::c_void;
use crate::ffi::types::*;
#[link(name = "user32")]
unsafe extern "system" {
	pub fn EnumWindows(
		enum_func: Option<unsafe extern "system" fn(Hwnd, isize) -> i32>,
		param: isize,
	) -> i32;
	pub fn GetWindowLongPtrW(window: Hwnd, index: i32) -> isize;
	pub fn GetWindowRect(window: Hwnd, rect: *mut Rect) -> i32;
	pub fn GetWindowTextW(window: Hwnd, string: *mut u16, max_count: i32) -> i32;
	pub fn GetWindowThreadProcessId(window: Hwnd, process_id: *mut u32) -> u32;
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
	pub fn NtSuspendProcess(process: Handle) -> NtStatus;
	pub fn NtResumeProcess(process: Handle) -> NtStatus;
	pub fn NtClose(handle: Handle) -> NtStatus;

	pub fn NtOpenProcessToken(
		process_handle: Handle,
		desired_access: u32,
		token_handle: *mut Handle,
	) -> NtStatus;

	pub fn NtOpenProcess(
		process_handle: *mut Handle,
		desired_access: u32,
		object_attributes: *mut ObjectAttributes,
		client_id: *mut ClientId,
	) -> NtStatus;

	pub fn NtQuerySystemInformation(
		system_information_class: SystemInformationClass,
		system_information: *mut c_void,
		system_information_length: u32,
		return_length: *mut u32,
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
		num_written: *mut usize,
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

	pub fn NtQueryInformationProcess(
		process_handle: Handle,
		process_information_class: u32,
		process_information: *mut c_void,
		process_information_length: u32,
		return_length: *mut u32,
	) -> NtStatus;
}