use crate::errors::{MemoryReadError, ProcessError, ProcessOperationError};
use crate::externs::{IntoResult, NtResumeProcess, NtSuspendProcess};
use crate::mem2disk::BlockDescriptor;
use crate::win_helpers::{get_process_image_path, read_bytes, SendHandle};
use std::convert::TryFrom;
use sysinfo::Pid;
use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, DuplicateHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_INVALID_HANDLE, E_ACCESSDENIED, HANDLE, HWND, LPARAM, RECT};
use windows::Win32::System::Threading::{GetProcessId, OpenProcess, PROCESS_ALL_ACCESS, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowLongW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW};
struct WindowSearchState { found_windows: Vec<HWND>, }

fn is_valid_target_window(hwnd: HWND) -> bool { unsafe {
	if !IsWindowVisible(hwnd).as_bool()
		|| IsIconic(hwnd).as_bool()
		|| (GetWindowLongW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW.0 as i32) != 0
	{
		return false;
	}

	let mut rect = RECT::default();
	if GetWindowRect(hwnd, &mut rect).is_err() { return false; }
	if ( rect.right - rect.left ) <= 0 || ( rect.bottom - rect.top ) <= 0 { return false; }

	let mut buffer = [0u16; 256];
	let len = GetWindowTextW(hwnd, &mut buffer) as usize;
	if len == 0 { return false; }
	let title = String::from_utf16_lossy(&buffer[..len]);
	if title.trim().is_empty() { return false; }

	true
} }

unsafe extern "system" fn enum_windows_filter_callback(hwnd: HWND, l_param: LPARAM) -> BOOL { unsafe {
	let Some(state) = (l_param.0 as *mut WindowSearchState).as_mut() else {
		return BOOL(1)
	};

	if is_valid_target_window(hwnd) { state.found_windows.push(hwnd); }

	BOOL(1)
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
	handle: SendHandle,
	pid: Pid,
	image_name: String,
	image_path: String
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
	pub fn from_pid(pid: Pid) -> Result<Self, ErrorType> {
		Self::from_pid_raw(pid.as_u32())
	}

	fn from_pid_raw(pid: u32) -> Result<Self, ErrorType> {
		if pid == 0 {
			return Err(ErrorType::InvalidPid);
		}
		unsafe {
			let res = OpenProcess(
				PROCESS_QUERY_LIMITED_INFORMATION,
				false,
				pid
			);

			let handle = res.map_err(|x| match x.code() {
				E_ACCESSDENIED => ErrorType::NoAccess,
				_ => ErrorType::Unknown
			})?;

			let path = get_process_image_path(handle).unwrap_or_default();
			let image_name = path.rsplit_once('\\').map(|(_, p)| p).unwrap_or_default().to_string();

			CloseHandle(handle).ok();

			Ok(
				Process {
					handle: Default::default(),
					pid: Pid::from_u32(pid),
					image_path: path,
					image_name
				}
			)
		}
	}

	pub fn from_process_handle(handle: HANDLE) -> Result<Self, ErrorType> {
		if handle.is_invalid() {
			return Err(ErrorType::InvalidHandle);
		}
		unsafe {
			let proc_id = GetProcessId(handle);
			if proc_id == 0 {
				return Err(match GetLastError() {
					ERROR_INVALID_HANDLE => ErrorType::NonProcessHandle,
					ERROR_ACCESS_DENIED => ErrorType::HandleLacksPermissions,
					_ => ErrorType::Unknown
				})
			}
			Self::from_pid_raw(proc_id)
		}
	}

	pub fn get_valid_windows_processes() -> Vec<Process> {
		let mut state = WindowSearchState {
			found_windows: Vec::new(),
		};

		unsafe {
			let l_param = LPARAM(&raw mut state as _);
			let _ = EnumWindows(Some(enum_windows_filter_callback), l_param);
		}

		state.found_windows
			.into_iter()
			.filter_map(|hwnd| Process::try_from(hwnd).ok())
			.collect()
	}

	pub fn init_handle(&mut self) -> Result<(), ErrorType> {
		if self.handle.is_invalid() {
			unsafe {
				let res = OpenProcess(
					PROCESS_ALL_ACCESS,
					false,
					self.pid.as_u32(),
				);

				let res2 = res.map_err(|x| match x.code() {
					E_ACCESSDENIED => ErrorType::NoAccess,
					_ => ErrorType::Unknown
				})?;

				self.handle = res2.into();
			}
		}
		Ok(())
	}

	pub fn get_handle(&self) -> HANDLE {
		*self.handle
	}

	pub fn get_pid(&self) -> Pid {
		self.pid
	}

	#[inline]
	pub fn get_chunk(&self, block_descriptor: &BlockDescriptor) -> Result<Vec<u8>, MemoryReadError> {
		debug_assert!(!self.handle.is_invalid());
		let mut buffer = vec![0u8; block_descriptor.size];

		let read_count = read_bytes(
			*self.handle,
			block_descriptor.virtual_address as _,
			buffer.as_mut_ptr(),
			block_descriptor.size
		)?;

		buffer.truncate(read_count);
		Ok(buffer)
	}

	pub fn suspend(&self) -> Result<(), ProcessOperationError> {
		assert!(!self.handle.is_invalid());

		if self.pid.as_u32() == std::process::id() { return Err(ProcessOperationError::CannotSuspendSelf) }
		let res = unsafe { NtSuspendProcess(*self.handle) };
		res.into_result().map_err(ProcessOperationError::SuspendFailed)
	}

	pub fn resume(&self) -> Result<(), ProcessOperationError> {
		assert!(!self.handle.is_invalid());

		let res = unsafe { NtResumeProcess(*self.handle) };
		res.into_result().map_err(ProcessOperationError::ResumeFailed)
	}

	pub fn suspend_guard(&self) -> Result<SuspendGuard<'_>, ProcessOperationError> {
		self.suspend()?;
		Ok(SuspendGuard { process: self })
	}

	pub fn format_name(&self) -> String {
		format!("{:08X}-{}", self.pid.as_u32(), self.image_name)
	}
}

impl TryFrom<HANDLE> for Process {
	type Error = ErrorType;
	fn try_from(handle: HANDLE) -> Result<Self, Self::Error> { Self::from_process_handle(handle) }
}

impl TryFrom<HWND> for Process {
	type Error = ErrorType;

	fn try_from(value: HWND) -> Result<Self, Self::Error> {
		unsafe {
			if value.is_invalid() {
				return Err(ErrorType::InvalidHWND);
			}
			let mut res = 0u32;
			let _ = GetWindowThreadProcessId(value, Some(&mut res));
			Self::from_pid_raw(res)
		}
	}
}