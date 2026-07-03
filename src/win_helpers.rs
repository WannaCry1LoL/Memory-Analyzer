use crate::errors::ProcessImagePathError::InvalidUtf16ImagePath;
use crate::errors::{MemoryReadError, MemoryWriteError, ProcessImagePathError};
use crate::externs::{nt_error, nt_success, NtReadVirtualMemory, NtWriteVirtualMemory, SystemBasicProcessInformation, SYSTEM_BASICPROCESS_INFORMATION};
use std::mem::MaybeUninit;
use std::ptr::{addr_of, null_mut};
use std::{mem, ptr};
use windows::Wdk::System::SystemInformation::NtQuerySystemInformation;
use windows::Win32::Foundation::{GetLastError, HANDLE, MAX_PATH, STATUS_INFO_LENGTH_MISMATCH};
use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
use windows::Win32::System::Threading::{QueryFullProcessImageNameW, PROCESS_NAME_FORMAT};

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

	if nt_error(status) {
		Err(MemoryReadError::Other(status))
	} else {
		Ok(())
	}
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
	if nt_error(status) {
		Err(MemoryWriteError::Other(status))
	} else {
		Ok(())
	}
}

pub fn read_generic<T: Sized>(handle: HANDLE, address: *const u8) -> Result<T, MemoryReadError> {
	let mut mem = MaybeUninit::<T>::uninit();
	let res = read_bytes(handle, address, mem.as_mut_ptr() as _, size_of::<T>());
	res?;
	Ok(unsafe { mem.assume_init() })
}

pub fn write_generic<T: Sized>(handle: HANDLE, address: *const u8, value: T) -> Result<(), MemoryWriteError> {
	let ptr = addr_of!(value);
	let res = write_bytes(handle, address, ptr as *const _, size_of::<T>());
	res?;
	Ok(())
}

pub fn get_process_image_path(handle: HANDLE) -> Result<String, ProcessImagePathError> {
	let mut buffer = vec![0u16; MAX_PATH as _];
	let mut size = buffer.len() as u32;

	unsafe {
		let result = QueryFullProcessImageNameW(
			handle,
			PROCESS_NAME_FORMAT(0),
			windows::core::PWSTR(buffer.as_mut_ptr()),
			&mut size,
		);
		let status = GetLastError();

		if result.is_ok() {
			String::from_utf16(&buffer[0..size as usize]).map_err(|_| InvalidUtf16ImagePath)
		} else {
			Err(ProcessImagePathError::Unknown(status))
		}
	}
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

		if nt_success(NtQuerySystemInformation(
			SystemBasicProcessInformation,
			sys_info.as_mut_ptr() as _,
			bytes,
			&mut bytes_written
		)) {
			let mut current_sys_info: SYSTEM_BASICPROCESS_INFORMATION;

			loop {
				let ptr = base_ptr as *const SYSTEM_BASICPROCESS_INFORMATION;
				current_sys_info = ptr::read_unaligned(ptr);

				match callback(&current_sys_info) {
					EnumerationState::Return(value) => return Some(value),
					EnumerationState::Continue => {},
					EnumerationState::Break => break,
				}

				if current_sys_info.NextEntryOffset == 0 { break; }
				base_ptr = base_ptr.add(current_sys_info.NextEntryOffset as _);
			}
		}

	}

	None
}
