use std::convert::TryFrom;
use std::mem::MaybeUninit;

use crate::errors::{MemoryReadError, MemoryWriteError, ProcessError, ProcessOperationError};
use crate::externs::*;
use crate::mem2disk::BlockDescriptor;
use crate::utils::get_process_image_path;

struct WindowSearchState {
	found_windows: Vec<Hwnd>,
}

fn is_valid_target_window(hwnd: Hwnd) -> bool { unsafe {
	if IsWindowVisible(hwnd) == 0
		|| IsIconic(hwnd) != 0
		|| (GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW) != 0
	{ return false; }

	let mut rect = Rect::default();
	if GetWindowRect(hwnd, &mut rect) == 0 { return false; }
	if (rect.right - rect.left) <= 0 || (rect.bottom - rect.top) <= 0 { return false; }

	let mut buffer = [0u16; 256];
	let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) as usize;
	if len == 0 { return false; }

	let title = String::from_utf16_lossy(&buffer[..len]);
	!title.trim().is_empty()
} }

unsafe extern "system" fn enum_windows_filter_callback(hwnd: Hwnd, l_param: isize) -> i32 { unsafe {
	let Some(state) = (l_param as *mut WindowSearchState).as_mut() else {
		return 1
	};

	if is_valid_target_window(hwnd) { state.found_windows.push(hwnd); }

	1
} }

pub struct SuspendGuard<'a> {
	process: &'a Process,
}

impl Drop for SuspendGuard<'_> {
	fn drop(&mut self) {
		let _ = self.process.resume();
	}
}

type ErrorType = ProcessError;

pub struct Process {
	handle: Handle,
	pid: Pid,
	image_name: String,
	image_path: String,
}

impl Clone for Process {
	fn clone(&self) -> Self {
		Self {
			handle: Default::default(),
			pid: self.pid,
			image_name: self.image_name.clone(),
			image_path: self.image_path.clone(),
		}
	}
}

impl Process {
	fn from_pid_raw(pid: u32) -> Result<Self, ErrorType> {
		if pid == 0 { return Err(ErrorType::InvalidPid) }

		let res = unsafe { open_process(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };

		let handle = res.map_err(|x| match x {
			NtStatus::STATUS_ACCESS_DENIED => ErrorType::NoAccess,
			_ => ErrorType::Unknown,
		})?;

		let path = get_process_image_path(handle).unwrap_or_default();
		let image_name = path
			.rsplit_once('\\')
			.map(|(_, p)| p)
			.unwrap_or_default()
			.to_string();

		unsafe { NtClose(handle) };

		Ok(Process {
			handle: Default::default(),
			pid: pid.into(),
			image_path: path,
			image_name,
		})
	}

	pub fn from_process_handle(handle: Handle) -> Result<Self, ErrorType> {
		if handle.is_invalid() {
			return Err(ErrorType::InvalidHandle);
		}
		unsafe {
			let proc_id = get_process_id(handle).map_err(|x| match x {
				NtStatus::STATUS_ACCESS_DENIED => ErrorType::HandleLacksPermissions,
				NtStatus::STATUS_INVALID_HANDLE => ErrorType::InvalidHandle,
				_ => ErrorType::Unknown,
			})?;

			Self::from_pid_raw(proc_id.0)
		}
	}

	pub fn get_valid_windows_processes() -> Vec<Process> {
		let mut state = WindowSearchState {
			found_windows: Vec::new(),
		};

		unsafe {
			let l_param = &raw mut state as _;
			let _ = EnumWindows(Some(enum_windows_filter_callback), l_param);
		}

		state.found_windows
			.into_iter()
			.filter_map(|hwnd| Process::try_from(hwnd).ok())
			.collect()
	}

	pub fn init_handle(&mut self) -> Result<(), ErrorType> {
		if self.handle.is_invalid() {
			let res = unsafe {
				open_process(PROCESS_ALL_ACCESS, false, self.pid.into())
			};

			self.handle = res.map_err(|x| match x {
				NtStatus::STATUS_ACCESS_DENIED => ErrorType::NoAccess,
				_ => ErrorType::Unknown,
			})?.into();
		}
		Ok(())
	}

	pub fn get_handle(&self) -> Handle {
		self.handle
	}

	pub fn get_pid(&self) -> Pid {
		self.pid
	}

	#[inline]
	pub fn get_chunk(&self, block_descriptor: &BlockDescriptor) -> Result<Vec<u8>, MemoryReadError> {
		let mut buffer = Vec::with_capacity(block_descriptor.size);
		let spare = buffer.spare_capacity_mut();
		self.get_chunk_into(block_descriptor, spare).map(|x| {
			unsafe { buffer.set_len(x); }
			buffer
		})
	}

	pub fn get_chunk_into(
		&self,
		block_descriptor: &BlockDescriptor,
		buffer: &mut [MaybeUninit<u8>],
	) -> Result<usize, MemoryReadError> {
		debug_assert!(!self.handle.is_invalid());
		debug_assert!(buffer.len() >= block_descriptor.size);

		let read_count = read_bytes(
			self.handle,
			block_descriptor.virtual_address as _,
			buffer.as_mut_ptr() as _,
			block_descriptor.size,
		)?;

		Ok(read_count)
	}

	pub fn write(&self, addr: *mut u8, bytes: &[u8]) -> Result<(), MemoryWriteError> {
		write_bytes(self.handle, addr, bytes.as_ptr(), bytes.len())
	}

	pub fn read(&self, addr: *const u8, length: usize) -> Result<Vec<u8>, MemoryReadError> {
		let mut buf = vec![0u8; length];
		read_bytes(self.handle, addr, buf.as_mut_ptr(), length)?;
		Ok(buf)
	}

	pub fn suspend(&self) -> Result<(), ProcessOperationError> {
		assert!(!self.handle.is_invalid());

		if self.pid == std::process::id().into() {
			return Err(ProcessOperationError::CannotSuspendSelf);
		}

		unsafe {
			NtSuspendProcess(self.handle)
				.into_result()
				.map_err(ProcessOperationError::SuspendFailed)
		}
	}

	pub fn resume(&self) -> Result<(), ProcessOperationError> {
		assert!(!self.handle.is_invalid());

		unsafe {
			NtResumeProcess(self.handle)
				.into_result()
				.map_err(ProcessOperationError::ResumeFailed)
		}
	}

	pub fn suspend_guard(&self) -> Result<SuspendGuard<'_>, ProcessOperationError> {
		self.suspend()?;
		Ok(SuspendGuard { process: self })
	}

	pub fn format_name(&self) -> String {
		format!("{:08X}-{}", self.pid, self.image_name)
	}
}

impl TryFrom<Handle> for Process {
	type Error = ErrorType;
	fn try_from(handle: Handle) -> Result<Self, Self::Error> { Self::from_process_handle(handle) }
}

impl TryFrom<Hwnd> for Process {
	type Error = ErrorType;

	fn try_from(value: Hwnd) -> Result<Self, Self::Error> {
		unsafe {
			if value.is_invalid() {
				return Err(ErrorType::InvalidHWND);
			}
			let mut res = 0u32;
			let _ = GetWindowThreadProcessId(value, &mut res);
			Self::from_pid_raw(res)
		}
	}
}

impl Drop for Process {
	fn drop(&mut self) {
		if !self.handle.is_invalid() {
			unsafe { NtClose(self.handle); }
		}
	}
}