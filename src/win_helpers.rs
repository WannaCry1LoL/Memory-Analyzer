use crate::externs::*;
use crate::errors::ProcessImagePathError::InvalidUtf16ImagePath;
use crate::errors::{ElevationError, MemoryReadError, MemoryWriteError, ProcessImagePathError};
use std::mem::MaybeUninit;
use std::os::windows::io::OwnedHandle;
use std::os::windows::prelude::FromRawHandle;
use std::ptr::null;
use std::{ptr, ptr::null_mut};

#[derive(PartialEq)]
pub enum EnumerationState<T> {
	Continue,
	Break,
	Return(T)
}

pub fn read_bytes(handle: Handle, address: *const u8, buffer: *mut u8, len: usize) -> Result<usize, MemoryReadError> {
	let mut read_bytes: usize = 0;
	let status = unsafe { NtReadVirtualMemory(
		handle,
		address as _,
		buffer as _,
		len,
		&mut read_bytes,
	) };

	if read_bytes == 0 {
		return status.into_result().map_err(MemoryReadError::Other).map(|_| 0);
	}

	Ok(read_bytes)
}

pub fn write_bytes(handle: Handle, address: *const u8, buffer: *const u8, len: usize) -> Result<(), MemoryWriteError> {
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

pub fn read_generic<T: Sized>(handle: Handle, address: *const u8) -> Result<T, MemoryReadError> {
	let mut mem = MaybeUninit::<T>::uninit();
	let res = read_bytes(handle, address, mem.as_mut_ptr() as _, size_of::<T>());
	let read = res?;
	if read != size_of::<T>() {
		return Err(MemoryReadError::NotEnoughBytesRead);
	}
	Ok(unsafe { mem.assume_init() })
}

pub fn write_generic<T: Sized>(handle: Handle, address: *const u8, value: T) -> Result<(), MemoryWriteError> {
	let ptr = &raw const value;
	let res = write_bytes(handle, address, ptr as _, size_of::<T>());
	res?;
	Ok(())
}

pub fn get_process_image_path(handle: Handle) -> Result<String, ProcessImagePathError> {
	let mut buffer = vec![0u16; 1024];
	let size = unsafe {
		query_full_process_image_name(
			handle,
			&mut buffer[..]
		).map_err(ProcessImagePathError::Unknown)?
	};

	String::from_utf16(&buffer[0..size as usize])
		.map_err(|_| InvalidUtf16ImagePath)
}

pub fn enum_proc_info<T>(mut callback: impl FnMut(&SystemBasicProcessInformation) -> EnumerationState<T>) -> Option<T> {
	unsafe {
		let mut bytes: u32 = 0;
		let mut bytes_written: u32 = 0;

		if NtQuerySystemInformation(
			SystemInformationClass::SYSTEM_BASIC_PROCESS_INFORMATION,
			null_mut(),
			0,
			&mut bytes,
		) != NtStatus::STATUS_INFO_LENGTH_MISMATCH {
			return None;
		}

		let mut sys_info = vec![0u8; bytes as usize];
		let mut base_ptr = sys_info.as_ptr();

		if NtQuerySystemInformation(
			SystemInformationClass::SYSTEM_BASIC_PROCESS_INFORMATION,
			sys_info.as_mut_ptr() as _,
			bytes,
			&mut bytes_written
		).is_ok() {
			let mut current_sys_info: SystemBasicProcessInformation;

			loop {
				let ptr = base_ptr as *const SystemBasicProcessInformation;
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

pub fn enable_debug_privilege() -> Result<(), ElevationError> { unsafe {
	let token = open_process_token(
		Handle::CURRENT_PROCESS,
		TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY
	).map_err(|_| ElevationError::Unknown)?;

	let guard = OwnedHandle::from_raw_handle(token.0);

	let mut luid = Luid::default();
	let priv_name: Vec<u16> = "SeDebugPrivilege\0".encode_utf16().collect();

	if LookupPrivilegeValueW(null(), priv_name.as_ptr(), &mut luid) == 0 {
		return Err(ElevationError::PrivilegeLookupFailed)
	};

	let mut tp = TokenPrivileges::default();
	tp.privilege_count = 1;
	tp.privileges[0].luid = luid;
	tp.privileges[0].attributes = SE_PRIVILEGE_ENABLED;

	adjust_token_privileges(
		guard.into(),
		false,
		Some(&tp),
		size_of::<TokenPrivileges>() as _,
		None,
		None
	).map_err(|_| ElevationError::PrivilegeAdjustmentFailed)?;

	match RtlGetLastWin32Error() {
		Win32Error::ERROR_NOT_ALL_ASSIGNED => Err(ElevationError::PrivilegeNotAssigned),
		_ => Ok(())
	}
} }
