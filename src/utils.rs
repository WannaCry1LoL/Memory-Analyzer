use::lazy_static::lazy_static;
use std::{
	collections::HashSet,
	ffi::c_void,
	mem,
	ptr,
};

use windows::Win32::{
	Foundation::HANDLE,
	System::{
		Diagnostics::Debug::ReadProcessMemory,
		Memory::{
			VirtualQueryEx,
			MEMORY_BASIC_INFORMATION,
			MEM_COMMIT,
			PAGE_READWRITE,
		},
		SystemInformation::{GetSystemInfo, SYSTEM_INFO}
	},
};

enum Endianness {
	Little,
	Big
}


lazy_static!{
	static ref ENDIAN: Endianness = unsafe {
		match ptr::read_unaligned([0xDEu8, 0xADu8, 0xBEu8, 0xEFu8].as_ptr() as *const u32) {
			0xDEADBEEF => Endianness::Big,
			_ => Endianness::Little,
		}
	};
}


pub trait ToBytes {
	fn to_ne_bytes_vec(&self) -> Vec<u8>;
}



macro_rules! impl_to_bytes {
    ($($t:ty),*) => {
        $(
            impl ToBytes for $t {
                fn to_ne_bytes_vec(&self) -> Vec<u8> {
                    self.to_ne_bytes().to_vec()

                }
            }
        )*
    };
}

impl_to_bytes!(u16, u32, u64, i16, i32, i64, u128, i128);

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn get_sys_info() -> SYSTEM_INFO {
	let mut sys_info: SYSTEM_INFO = mem::zeroed();
	GetSystemInfo(&mut sys_info);
	sys_info
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn iterate_mem_with_func<F: Fn(u8, &[u8], usize, usize) -> bool>(handle: HANDLE, callback: F) -> HashSet<usize> {
	let mut addresses: HashSet<usize> = HashSet::new();
	let si = get_sys_info();

	let mut info: MEMORY_BASIC_INFORMATION = mem::zeroed();
	let mut base_addr: *mut c_void = ptr::null_mut();

	while base_addr < si.lpMaximumApplicationAddress {
		VirtualQueryEx(
			handle,
			Some(base_addr),
			&mut info,
			size_of::<MEMORY_BASIC_INFORMATION>(),
		);

		if info.State == MEM_COMMIT && info.Protect == PAGE_READWRITE {
			let mut buffer: Vec<u8> = vec![0u8; info.RegionSize];
			let mut read_bytes: usize = 0;
			let res = ReadProcessMemory(
				handle,
				base_addr,
				buffer.as_mut_ptr() as *mut c_void,
				info.RegionSize,
				Some(&mut read_bytes),
			);

			// TODO: Handle error

			for i in 0..read_bytes {
				if callback(buffer[i], &buffer, i, read_bytes) {
					addresses.insert(base_addr as usize + i);
				}
			}
		}
		base_addr = (info.BaseAddress as usize + info.RegionSize) as *mut _;
	}

	addresses
}

#[allow(unused)]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn find_byte(handle: HANDLE, byte: u8) -> HashSet<usize> {
	iterate_mem_with_func(handle, |current, _, _, _| {
		return current == byte;
	})
}

#[allow(unused)]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn find_bytes(handle: HANDLE, bytes: &[u8]) -> HashSet<usize> {
	let first = bytes[0];
	let bytes_len = bytes.len();
	iterate_mem_with_func(handle, |_, buffer, idx, bytes_read| {
		buffer[idx] == first && bytes_read - idx >= bytes_len && buffer[idx..bytes_len + idx] == bytes[..]
	})
}

#[allow(unused)]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn find_string(handle: HANDLE, string: &str) -> HashSet<usize> {
	find_bytes(handle, string.as_bytes()) // TODO: Handle 2 bpc strings
}

#[allow(unused)]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn find_number<T: ToBytes>(handle: HANDLE, num: T) -> HashSet<usize> {
	find_bytes(handle, &num.to_ne_bytes_vec())
}

#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn find_single(handle: HANDLE, single: f32) -> HashSet<usize> {
	iterate_mem_with_func(handle, |_, buffer, idx, _| {
		let ptr = buffer[idx..].as_ptr() as *const f32;
		let single_res = ptr::read_unaligned(ptr);
		return (single - single_res).abs() < f32::EPSILON;
	})
}

#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn find_double(handle: HANDLE, double: f64) -> HashSet<usize> {
	iterate_mem_with_func(handle, |_, buffer, idx, _| {
		let ptr = buffer[idx..].as_ptr() as *const f64;
		let double_res = ptr::read_unaligned(ptr);
		return (double - double_res).abs() < f64::EPSILON;
	})
}