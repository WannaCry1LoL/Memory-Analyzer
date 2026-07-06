use std::ffi::c_void;
use std::hint::unreachable_unchecked;
use windows::Win32::Foundation::{HANDLE, NTSTATUS, UNICODE_STRING, WIN32_ERROR};


#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum NtStatusType {
	Information = 0,
	Success = 1,
	Warning = 2,
	Error = 3
}

pub trait IntoResult: Sized {
	fn into_result(self) -> Result<(), Self>;
}

impl IntoResult for NTSTATUS {
	fn into_result(self) -> Result<(), Self> {
		if self.is_ok() { Ok(()) } else { Err(self) }
	}
}

impl IntoResult for WIN32_ERROR {
	fn into_result(self) -> Result<(), Self> {
		if self.is_ok() { Ok(()) } else { Err(self) }
	}
}

pub trait NtStatusExt: IntoResult {
	fn get_type(&self) -> NtStatusType;
	fn is_information(&self) -> bool;
	fn is_success(&self) -> bool;
	fn is_warn(&self) -> bool;
}

impl NtStatusExt for NTSTATUS {

	#[inline]
	fn get_type(&self) -> NtStatusType {
		match self.0 as u32 >> 30 {
			0 => NtStatusType::Success,
			1 => NtStatusType::Information,
			2 => NtStatusType::Warning,
			3 => NtStatusType::Error,
			_ => unsafe { unreachable_unchecked() }
		}
	}

	#[inline]
	fn is_information(&self) -> bool { self.get_type() == NtStatusType::Information }
	#[inline]
	fn is_success(&self) -> bool { self.get_type() == NtStatusType::Success }
	#[inline]
	fn is_warn(&self) -> bool { self.get_type() == NtStatusType::Warning }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SystemInformationClass(pub i32);

#[allow(non_upper_case_globals)]
pub const SystemBasicProcessInformation: SystemInformationClass = SystemInformationClass(252i32);

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct SYSTEM_BASICPROCESS_INFORMATION
{
	pub next_entry_offset: u32,
	pub unique_process_id: usize,
	pub inherited_from_unique_process_id: usize,
	pub sequence_number: u64,
	pub image_name: UNICODE_STRING
}

#[link(name = "ntdll")]
unsafe extern "system" {
	fn RtlNtStatusToDosError(status: NTSTATUS) -> u32;
	pub fn NtSuspendProcess(process: HANDLE) -> NTSTATUS;
	pub fn NtResumeProcess(process: HANDLE) -> NTSTATUS;

	pub fn NtQuerySystemInformation(
		system_information_class: SystemInformationClass, 
		system_information: *mut c_void, 
		system_information_length: u32, 
		return_length: *mut u32
	) -> NTSTATUS;

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