use crate::errors::ProcessError;
use crate::win_helpers::get_process_image_path;
use std::convert::{TryFrom, TryInto};
use sysinfo::Pid;
use windows::Win32::Foundation::{GetLastError, ERROR_ACCESS_DENIED, ERROR_INVALID_HANDLE, HANDLE, HWND};
use windows::Win32::System::Threading::{
	GetProcessHandleFromHwnd, GetProcessId, OpenProcess, PROCESS_ALL_ACCESS
};

type ErrorType = ProcessError;

pub struct Process {
	handle: HANDLE,
	pid: Pid,
	image_name: String,
	image_path: String,
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
			let new_handle = OpenProcess(
				PROCESS_ALL_ACCESS,
				false,
				pid
			);

			if let Ok(handle) = new_handle {
				let path = get_process_image_path(handle).unwrap_or_default();
				let image_name = path.rsplit_once('/').map(|(_, p)| p).unwrap_or_default().to_string();
				Ok(
					Process {
						handle,
						pid: Pid::from_u32(pid),
						image_path: path,
						image_name,
					}
				)
			} else {
				Err(match GetLastError() {
					ERROR_ACCESS_DENIED => ErrorType::NoAccess,
					_ => ErrorType::Unknown
				})
			}
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

	pub fn get_handle(&self) -> HANDLE {
		self.handle
	}

	pub fn get_pid(&self) -> Pid {
		self.pid
	}
}

impl TryFrom<HANDLE> for Process {
	type Error = ErrorType;
	fn try_from(handle: HANDLE) -> Result<Self, Self::Error> {
		Process::from_process_handle(handle)
	}
}

impl TryFrom<HWND> for Process {
	type Error = ErrorType;

	fn try_from(value: HWND) -> Result<Self, Self::Error> {
		unsafe {
			if value.is_invalid() {
				return Err(ErrorType::InvalidHWND);
			}
			let handle = GetProcessHandleFromHwnd(value);
			Process::try_from(handle)
		}
	}
}
//
// impl TryFrom<Pid> for Process {
//
// }
//
// impl TryFrom<&str> for Process {
//
// }
//
