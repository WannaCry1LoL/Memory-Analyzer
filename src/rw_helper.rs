use std::{
	collections::{HashSet, HashMap},
	ffi::c_void,
	mem,
	ptr,
};
use std::mem::MaybeUninit;
use std::ptr::{addr_of, null, null_mut};
use windows::{
	Win32::{
		Foundation::{CloseHandle, HANDLE, MAX_PATH, STATUS_INFO_LENGTH_MISMATCH},
		System::{
			Diagnostics::Debug::ReadProcessMemory,
			Memory::{
				VirtualQueryEx,
				VirtualAlloc,
				VirtualFree,
				MEMORY_BASIC_INFORMATION,
				MEM_COMMIT,
				PAGE_READWRITE,
				MEM_RELEASE
			},
			SystemInformation::{GetSystemInfo, SYSTEM_INFO},
			SystemServices::MAXIMUM_ALLOWED,
			Threading::{GetThreadId, PROCESS_BASIC_INFORMATION},
		}
	},
	Wdk::{
		System::SystemInformation::{
			NtQuerySystemInformation,
		},
		System::Threading::{NtQueryInformationProcess, ProcessBasicInformation}
	},
};
use windows::Win32::Foundation::{GetLastError, NTSTATUS, WIN32_ERROR};
use crate::errors::{MemoryReadError, MemoryWriteError};
use crate::externs::{NtGetNextThread, nt_success, NtGetNextProcess, SYSTEM_BASICPROCESS_INFORMATION, SystemBasicProcessInformation, NtReadVirtualMemory, nt_error, NtWriteVirtualMemory};

pub fn read_bytes(handle: HANDLE, address: *const u8, buffer: *mut u8, len: usize) -> Result<(), MemoryReadError> {
	unsafe {
		let mut read_bytes: usize = 0;
		let status = NtReadVirtualMemory(
			handle,
			address as _,
			buffer as _,
			len,
			&mut read_bytes,
		);

		if read_bytes != len {
			return Err(MemoryReadError::NotEnoughBytesRead)
		}
		
		if nt_error(status) {
			Err(MemoryReadError::Other(status))
		} else {
			Ok(())
		}

	}
}

pub fn write_bytes(handle: HANDLE, address: *const u8, buffer: *const u8, len: usize) -> Result<(), MemoryWriteError> {
	unsafe {
		let mut written_bytes: usize = 0;
		let status = NtWriteVirtualMemory(
			handle,
			address as _,
			buffer as _,
			len,
			&mut written_bytes,
		);

		if written_bytes != len {
			return Err(MemoryWriteError::NotEnoughBytesWritten)
		}
		if nt_error(status) {
			Err(MemoryWriteError::Other(status))
		} else {
			Ok(())
		}

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