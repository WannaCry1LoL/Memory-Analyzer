use std::ffi::{c_ulong, c_void};
use windows::Win32::Foundation::{NTSTATUS, HANDLE, UNICODE_STRING, WIN32_ERROR};
use windows::Win32::System::Threading::{PROCESS_INFORMATION_CLASS, THREAD_INFORMATION_CLASS};
use windows::Wdk::System::SystemInformation::{SYSTEM_INFORMATION_CLASS};


#[inline(always)]
pub fn nt_success(status: NTSTATUS) -> bool {
	status.0 >= 0
}

#[inline(always)]
pub fn nt_information(status: NTSTATUS) -> bool {
	(status.0 >> 30) == 1
}

#[inline(always)]
pub fn nt_warning(status: NTSTATUS) -> bool {
	(status.0 >> 30) == 2
}

#[inline(always)]
pub fn nt_error(status: NTSTATUS) -> bool {
	(status.0 >> 30) == 3
}

pub const SystemBasicProcessInformation: SYSTEM_INFORMATION_CLASS = SYSTEM_INFORMATION_CLASS(252i32);
#[repr(C)]
pub struct SYSTEM_BASICPROCESS_INFORMATION
{
	pub NextEntryOffset: u32,
	pub UniqueProcessId: usize,
	pub InheritedFromUniqueProcessId: usize,
	pub SequenceNumber: u64,
	pub ImageName: UNICODE_STRING
}

#[link(name = "ntdll")]
unsafe extern "system" {
	fn RtlNtStatusToDosErrorRaw(status: NTSTATUS) -> u32;
	fn NtSuspendProcess(process: HANDLE) -> NTSTATUS;
	fn NtResumeProcess(process: HANDLE) -> NTSTATUS;

	pub fn NtGetNextThread(
		process: HANDLE,
		thread: HANDLE,
		access_mask: u32,
		attributes: u32,
		flags: u32,
		new_handle: *mut HANDLE
	) -> NTSTATUS;

	pub fn NtGetNextProcess(
		process: HANDLE,
		access_mask: u32,
		attributes: u32,
		flags: u32,
		new_handle: *mut HANDLE
	) -> NTSTATUS;

	pub fn NtReadVirtualMemory(
		process: HANDLE,
		address: *const c_void,
		buffer: *mut c_void,
		num_to_read: usize,
		num_read: *mut usize,
	) -> NTSTATUS;

	pub fn NtWriteVirtualMemory(
		process: HANDLE,
		address: *const c_void,
		buffer: *const c_void,
		num_to_write: usize,
		num_written: *mut usize
	) -> NTSTATUS;
}