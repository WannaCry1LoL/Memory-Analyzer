use crate::externs::{nt_success, NtGetNextProcess, NtGetNextThread, SystemBasicProcessInformation, SYSTEM_BASICPROCESS_INFORMATION};
use std::ptr::null_mut;
use std::{
	collections::{HashMap, HashSet},
	ffi::c_void,
	mem,
	ptr,
};
use windows::{
	Wdk::{
		System::SystemInformation::NtQuerySystemInformation,
		System::Threading::{NtQueryInformationProcess, ProcessBasicInformation}
	},
	Win32::{
		Foundation::{CloseHandle, HANDLE, STATUS_INFO_LENGTH_MISMATCH},
		System::{
			Diagnostics::Debug::ReadProcessMemory,
			Memory::{
				VirtualAlloc,
				VirtualFree,
				VirtualQueryEx,
				MEMORY_BASIC_INFORMATION,
				MEM_COMMIT,
				MEM_RELEASE,
				PAGE_READWRITE
			},
			SystemInformation::{GetSystemInfo, SYSTEM_INFO},
			SystemServices::MAXIMUM_ALLOWED,
			Threading::{GetThreadId, PROCESS_BASIC_INFORMATION},
		}
	},
};


#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn get_sys_info() -> SYSTEM_INFO {
	let mut sys_info: SYSTEM_INFO = mem::zeroed();
	GetSystemInfo(&mut sys_info);
	sys_info
}

#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn iterate_mem_with_func<F: Fn(u8, &[u8], usize, usize) -> bool>(handle: HANDLE, callback: F) -> HashSet<usize> {
	let mut addresses: HashSet<usize> = HashSet::new();
	let si = get_sys_info();

	let mut info: MEMORY_BASIC_INFORMATION = mem::zeroed();
	let mut base_addr: *mut c_void = null_mut();

	while base_addr < si.lpMaximumApplicationAddress {
		VirtualQueryEx(
			handle,
			Some(base_addr),
			&mut info,
			size_of::<MEMORY_BASIC_INFORMATION>(),
		);

		if (info.State == MEM_COMMIT && info.Protect == PAGE_READWRITE) {
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

		base_addr = info.BaseAddress.add(info.RegionSize);
	}
	addresses
}

#[allow(unused)]
pub fn get_proc_threads(handle: HANDLE) -> Vec<usize> {
	let mut ret: Vec<usize> = Vec::new();
	unsafe {
		let mut thread: HANDLE = HANDLE::default();
		let mut prev_thread: HANDLE = HANDLE::default();
		while nt_success(NtGetNextThread(
			handle,
			prev_thread,
			MAXIMUM_ALLOWED,
			0,
			0,
			&mut thread as *mut HANDLE,
		))
		{
			ret.push(GetThreadId(thread) as usize);
			if !prev_thread.is_invalid() {
				CloseHandle(prev_thread);
			}
			prev_thread = thread;
		}
		if !prev_thread.is_invalid() {
			CloseHandle(prev_thread);
		}
	}
	ret
}

pub fn build_process_tree_new() -> HashMap<usize, Vec<usize>> {
	let mut ret: HashMap<usize, Vec<usize>> = HashMap::new();

	unsafe {
		let mut bytes: u32 = 0;
		let mut bytes_written: u32 = 0;
		if NtQuerySystemInformation(
			SystemBasicProcessInformation,
			null_mut(),
			0,
			&mut bytes,
		) == STATUS_INFO_LENGTH_MISMATCH {
			let sys_info = VirtualAlloc(None, bytes as usize, MEM_COMMIT, PAGE_READWRITE);
			let mut ptr = sys_info as *mut SYSTEM_BASICPROCESS_INFORMATION;
			if nt_success(NtQuerySystemInformation(
				SystemBasicProcessInformation,
				sys_info,
				bytes,
				&mut bytes_written
			)) {
				let mut current_sys_info: SYSTEM_BASICPROCESS_INFORMATION;
				loop {
					current_sys_info = ptr::read_unaligned(ptr);
					let ppid = current_sys_info.InheritedFromUniqueProcessId;
					let pid = current_sys_info.UniqueProcessId;
					ret
						.entry(ppid)
						.or_default()
						.push(pid);
					if current_sys_info.NextEntryOffset == 0 { break; }

					ptr = ptr.byte_add(current_sys_info.NextEntryOffset as usize);
				}
			}
			VirtualFree(sys_info, 0, MEM_RELEASE);
		}
	}

	ret
}

pub fn build_process_tree() -> HashMap<usize, Vec<usize>> {
	let mut map: HashMap<usize, Vec<usize>> = HashMap::new();
	unsafe {
		let mut proc: HANDLE = HANDLE::default();
		let mut prev_proc: HANDLE = HANDLE::default();
		let mut proc_info: PROCESS_BASIC_INFORMATION = mem::zeroed();
		while nt_success(NtGetNextProcess(
			prev_proc,
			MAXIMUM_ALLOWED,
			0,
			0,
			&mut proc as *mut HANDLE,
		))
		{
			let mut bytes: u32 = 0;
			if nt_success(NtQueryInformationProcess(
				proc,
				ProcessBasicInformation,
				&mut proc_info as *mut _ as *mut c_void,
				size_of::<PROCESS_BASIC_INFORMATION>() as u32,
				&mut bytes
			)) {
				map
					.entry(proc_info.InheritedFromUniqueProcessId)
					.or_default()
					.push(proc_info.UniqueProcessId);
			}
			if !prev_proc.is_invalid() {
				CloseHandle(prev_proc);
			}
			prev_proc = proc;
		}
		if !prev_proc.is_invalid() {
			CloseHandle(prev_proc);
		}
	}
	map
}

// todo: build some abstraction over process
pub fn find_proc_from_image_name(name: &str) -> Option<usize> {
	unsafe {
		let mut bytes: u32 = 0;
		let mut bytes_written: u32 = 0;
		if NtQuerySystemInformation(
			SystemBasicProcessInformation,
			null_mut(),
			0,
			&mut bytes,
		) == STATUS_INFO_LENGTH_MISMATCH {
			let sys_info = VirtualAlloc(None, bytes as usize, MEM_COMMIT, PAGE_READWRITE);
			let mut ptr = sys_info as *mut SYSTEM_BASICPROCESS_INFORMATION;
			if nt_success(NtQuerySystemInformation(
				SystemBasicProcessInformation,
				sys_info,
				bytes,
				&mut bytes_written
			)) {
				let mut current_sys_info: SYSTEM_BASICPROCESS_INFORMATION;

				loop {
					current_sys_info = ptr::read_unaligned(ptr);

					if !current_sys_info.ImageName.Buffer.is_null()
						&& let Ok(res) = current_sys_info.ImageName.Buffer.to_string()
						&& res == name
					{
						return Some(current_sys_info.UniqueProcessId);
					}

					if current_sys_info.NextEntryOffset == 0 { break; }

					ptr = ptr.byte_add(current_sys_info.NextEntryOffset as usize);
				}
			}
			VirtualFree(sys_info, 0, MEM_RELEASE);

		}
	}

	None
}

#[derive(PartialEq, Clone, Copy)]
pub enum TriState {
	DontCare = 2,
	True = 1,
	False = 0,
}
