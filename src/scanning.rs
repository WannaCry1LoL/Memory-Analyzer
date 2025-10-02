use std::collections::HashSet;
use std::ptr;
use windows::Win32::Foundation::HANDLE;
use crate::utils::{iterate_mem_with_func, ToBytes};

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
pub fn find_number<T: ToBytes>(handle: HANDLE, num: T) -> HashSet<usize> {
	find_bytes(handle, &num.to_ne_bytes_vec())
}

pub fn find_single(handle: HANDLE, single: f32) -> HashSet<usize> {
	unsafe {
		iterate_mem_with_func(handle, |_, buffer, idx, bytes_read| {
			if bytes_read - idx < 4 { return false; }
			let ptr = buffer[idx..].as_ptr() as *const f32;
			let single_res = ptr::read_unaligned(ptr);
			return (single - single_res).abs() < f32::EPSILON;
		})
	}
}

pub fn find_double(handle: HANDLE, double: f64) -> HashSet<usize> {
	unsafe {
		iterate_mem_with_func(handle, |_, buffer, idx, bytes_read| {
			if bytes_read - idx < 8 { return false; }
			let ptr = buffer[idx..].as_ptr() as *const f64;
			let double_res = ptr::read_unaligned(ptr);
			return (double - double_res).abs() < f64::EPSILON; // probably should adjust this i think? arent floats dynamic or is this always the smallest difference between floating points of any scale?
		})
	}
}
