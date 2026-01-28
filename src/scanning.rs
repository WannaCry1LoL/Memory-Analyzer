use std::collections::HashSet;
use std::ptr;
use windows::Win32::Foundation::HANDLE;
use crate::memory_pattern::MemoryPattern;
use crate::utils::{iterate_mem_with_func};

#[allow(unused)]
pub fn find_byte(handle: HANDLE, byte: u8) -> HashSet<usize> {
	unsafe {
		iterate_mem_with_func(handle, |current, _, _, _| {
			return current == byte;
		})
	}
}

#[allow(unused)]
pub fn find_bytes(handle: HANDLE, bytes: &[u8]) -> HashSet<usize> {
	let first = bytes[0];
	let bytes_len = bytes.len();
	unsafe {
		iterate_mem_with_func(handle, |_, buffer, idx, bytes_read| {
			buffer[idx] == first && bytes_read - idx >= bytes_len && buffer[idx..bytes_len + idx] == bytes[..]
		})
	}
}

#[allow(unused)]
pub fn find_string(handle: HANDLE, string: &str) -> HashSet<usize> {
	find_bytes(handle, string.as_bytes())
		.union(
			&find_bytes(
				handle,
				&string.encode_utf16().flat_map(|c| c.to_ne_bytes()).collect::<Vec<u8>>()[..]
			)
		)
		.copied()
		.collect()
}

#[allow(unused)]
pub fn find_pattern<T: MemoryPattern>(handle: HANDLE, value: T) -> HashSet<usize> {
	unsafe {
		iterate_mem_with_func(handle, |_, buffer, idx, bytes_read| {
			if bytes_read - idx < T::size() {
				return false;
			}
			value.matches(buffer[idx..].as_ptr())
		})
	}
}