use crate::errors::ProcessImagePathError::InvalidUtf16ImagePath;
use crate::errors::{ElevationError, MemoryReadError, MemoryWriteError, ProcessImagePathError};
use crate::externs::*;
use std::mem::MaybeUninit;
use std::{mem, ptr, ptr::null_mut};
use std::ops::Deref;
use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_NOT_ALL_ASSIGNED, HANDLE, LUID, STATUS_INFO_LENGTH_MISMATCH};
use windows::Win32::Security::{AdjustTokenPrivileges, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY};
use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT};

#[derive(PartialEq)]
pub enum EnumerationState<T> {
	Continue,
	Break,
	Return(T)
}

pub fn get_sys_info() -> SYSTEM_INFO {
	unsafe {
		let mut sys_info: SYSTEM_INFO = mem::zeroed();
		GetSystemInfo(&mut sys_info);
		sys_info
	}
}

pub fn read_bytes(handle: HANDLE, address: *const u8, buffer: *mut u8, len: usize) -> Result<(), MemoryReadError> {
	let mut read_bytes: usize = 0;
	let status = unsafe { NtReadVirtualMemory(
		handle,
		address as _,
		buffer as _,
		len,
		&mut read_bytes,
	) };

	if read_bytes != len {
		return Err(MemoryReadError::NotEnoughBytesRead)
	}
	status.into_result().map_err(MemoryReadError::Other)
}

pub fn write_bytes(handle: HANDLE, address: *const u8, buffer: *const u8, len: usize) -> Result<(), MemoryWriteError> {
	let mut written_bytes: usize = 0;
	let status = unsafe { NtWriteVirtualMemory(
		handle,
		address as _,
		buffer as _,
		len,
		&mut written_bytes,
	) };

	if written_bytes != len {
		return Err(MemoryWriteError::NotEnoughBytesWritten)
	}

	status.into_result().map_err(MemoryWriteError::Other)
}

pub fn read_generic<T: Sized>(handle: HANDLE, address: *const u8) -> Result<T, MemoryReadError> {
	let mut mem = MaybeUninit::<T>::uninit();
	let res = read_bytes(handle, address, mem.as_mut_ptr() as _, size_of::<T>());
	res?;
	Ok(unsafe { mem.assume_init() })
}

pub fn write_generic<T: Sized>(handle: HANDLE, address: *const u8, value: T) -> Result<(), MemoryWriteError> {
	let ptr = &raw const value;
	let res = write_bytes(handle, address, ptr as _, size_of::<T>());
	res?;
	Ok(())
}

pub fn get_process_image_path(handle: HANDLE) -> Result<String, ProcessImagePathError> {
	let mut buffer = vec![0u16; 1024];
	let mut size = buffer.len() as u32;

	unsafe {
		QueryFullProcessImageNameW(
			handle,
			PROCESS_NAME_FORMAT(0),
			windows::core::PWSTR(buffer.as_mut_ptr()),
			&mut size,
		)
			.map_err(|_| ProcessImagePathError::Unknown(GetLastError()))?;
	}
	String::from_utf16(&buffer[0..size as usize])
		.map_err(|_| InvalidUtf16ImagePath)
}

pub fn enum_proc_info<T>(mut callback: impl FnMut(&SYSTEM_BASICPROCESS_INFORMATION) -> EnumerationState<T>) -> Option<T> {
	unsafe {
		let mut bytes: u32 = 0;
		let mut bytes_written: u32 = 0;

		if NtQuerySystemInformation(
			SystemBasicProcessInformation,
			null_mut(),
			0,
			&mut bytes,
		) != STATUS_INFO_LENGTH_MISMATCH {
			return None;
		}

		let mut sys_info = vec![0u8; bytes as usize];
		let mut base_ptr = sys_info.as_ptr();

		if NtQuerySystemInformation(
			SystemBasicProcessInformation,
			sys_info.as_mut_ptr() as _,
			bytes,
			&mut bytes_written
		).is_ok() {
			let mut current_sys_info: SYSTEM_BASICPROCESS_INFORMATION;

			loop {
				let ptr = base_ptr as *const SYSTEM_BASICPROCESS_INFORMATION;
				current_sys_info = ptr::read_unaligned(ptr);

				match callback(&current_sys_info) {
					EnumerationState::Return(value) => return Some(value),
					EnumerationState::Continue => {},
					EnumerationState::Break => break,
				}

				if current_sys_info.next_entry_offset == 0 { break; }
				base_ptr = base_ptr.add(current_sys_info.next_entry_offset as _);
			}
		}

	}

	None
}


pub struct HandleGuard<'a> {
	handle: &'a mut HANDLE,
}

impl Drop for HandleGuard<'_> {
	fn drop(&mut self) { let _ = unsafe { CloseHandle(*self.handle) }; }
}

impl Deref for HandleGuard<'_> {
	type Target = HANDLE;
	fn deref(&self) -> &HANDLE { self.handle }
}

pub fn enable_debug_privilege() -> Result<(), ElevationError> {
	unsafe {
		let mut h_token = HANDLE::default();

		OpenProcessToken(
			GetCurrentProcess(),
			TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
			&mut h_token
		)?;

		let handle = HandleGuard { handle: &mut h_token };

		let mut luid = LUID::default();

		LookupPrivilegeValueW(None, w!("SeDebugPrivilege"), &mut luid)?;

		let mut tp = TOKEN_PRIVILEGES::default();
		tp.PrivilegeCount = 1;
		tp.Privileges[0].Luid = luid;
		tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;

		AdjustTokenPrivileges(
			*handle,
			false,
			Some(&tp),
			size_of::<TOKEN_PRIVILEGES>() as _,
			None,
			None,
		)?;

		match GetLastError() {
			ERROR_NOT_ALL_ASSIGNED => Err(ElevationError::PrivilegeNotAssigned),
			_ => Ok(())
		}
	}
}
