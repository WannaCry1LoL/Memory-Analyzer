use crate::errors::{MemoryReadError, MemoryWriteError};
use crate::externs::{nt_error, NtReadVirtualMemory, NtWriteVirtualMemory};
use std::mem::MaybeUninit;
use std::ptr::addr_of;
use windows::Win32::Foundation::HANDLE;

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