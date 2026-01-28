use sysinfo::Pid;
use std::convert::{TryFrom, TryInto};
use windows::Win32::Foundation::{GetLastError, ERROR_ACCESS_DENIED, ERROR_INVALID_HANDLE, HANDLE, HWND, NTSTATUS};
use windows::Win32::System::Threading::{GetProcessHandleFromHwnd, GetProcessId, OpenProcess, PROCESS_ALL_ACCESS};
use crate::errors::ProcessError;

type ErrorType = ProcessError;

pub struct Process {
	handle: HANDLE,
	pid: Pid
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
				Ok(
					Process {
						handle,
						pid: Pid::from_u32(pid)
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

	pub fn get_image_name() -> Result<String, NTSTATUS> {
		todo!()
	}

	pub fn get_process_name(&self) -> Result<String, NTSTATUS> {
		todo!()
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
