use crate::errors::{MemoryReadError, MemoryWriteError, ProcessError, ProcessOperationError};
use crate::ffi::constants::{PROCESS_ALL_ACCESS, PROCESS_QUERY_LIMITED_INFORMATION};
use crate::ffi::types::{Handle, Hwnd, NtStatus, Pid};
use crate::ffi::wrappers::*;
use crate::mem2disk::BlockDescriptor;
use crate::utils::get_process_image_path;
use std::convert::TryFrom;
use std::mem::MaybeUninit;
use std::ops::ControlFlow;

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
	fn from_pid_raw(pid: Pid) -> Result<Self, ErrorType> {
		if pid.0 == 0 { return Err(ErrorType::InvalidPid) }

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

		close(handle).ok();

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
		let proc_id = get_process_id(handle).map_err(|x| match x {
			NtStatus::STATUS_ACCESS_DENIED => ErrorType::HandleLacksPermissions,
			NtStatus::STATUS_INVALID_HANDLE => ErrorType::InvalidHandle,
			_ => ErrorType::Unknown,
		})?;

		Self::from_pid_raw(proc_id)
	}

	pub fn get_valid_windows_processes() -> Vec<Process> {
		let mut found_windows = Vec::new();

		let _ = enum_windows(|hwnd| {
			if is_valid_target_window(hwnd) { found_windows.push(hwnd); };
			ControlFlow::<()>::Continue(())
		});

		found_windows
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

	pub fn get_handle(&self) -> Handle { self.handle }

	pub fn get_pid(&self) -> Pid { self.pid }

	pub fn get_chunk_into(
		&self,
		block_descriptor: &BlockDescriptor,
		buffer: &mut [MaybeUninit<u8>],
	) -> Result<usize, MemoryReadError> {
		debug_assert!(!self.handle.is_invalid());
		debug_assert!(buffer.len() >= block_descriptor.size);

		let read_count = unsafe { read_bytes(
			self.handle,
			block_descriptor.virtual_address as _,
			buffer.as_mut_ptr() as _,
			block_descriptor.size,
		)? };

		Ok(read_count)
	}

	pub fn write(&self, addr: *mut u8, bytes: &[u8]) -> Result<(), MemoryWriteError> {
		unsafe { write_bytes(self.handle, addr, bytes.as_ptr(), bytes.len()) }
	}

	pub fn read(&self, addr: *const u8, length: usize) -> Result<Vec<u8>, MemoryReadError> {
		let mut buf = vec![0u8; length];
		unsafe { read_bytes(self.handle, addr, buf.as_mut_ptr(), length)? };
		Ok(buf)
	}

	pub fn suspend(&self) -> Result<(), ProcessOperationError> {
		assert!(!self.handle.is_invalid());

		if self.pid == std::process::id().into() {
			return Err(ProcessOperationError::CannotSuspendSelf);
		}

		suspend_process(self.handle).map_err(ProcessOperationError::SuspendFailed)
	}

	pub fn resume(&self) -> Result<(), ProcessOperationError> {
		assert!(!self.handle.is_invalid());

		resume_process(self.handle).map_err(ProcessOperationError::ResumeFailed)
	}

	pub fn suspend_guard(&self) -> Result<SuspendGuard<'_>, ProcessOperationError> {
		self.suspend()?;
		Ok(SuspendGuard { process: self })
	}

	pub fn format_name(&self) -> String { format!("{:08X}-{}", self.pid, self.image_name) }
}

impl TryFrom<Handle> for Process {
	type Error = ErrorType;
	fn try_from(handle: Handle) -> Result<Self, Self::Error> { Self::from_process_handle(handle) }
}

impl TryFrom<Hwnd> for Process {
	type Error = ErrorType;

	fn try_from(value: Hwnd) -> Result<Self, Self::Error> {
		if value.is_invalid() {
			return Err(ErrorType::InvalidHWND);
		}
		let res = get_process_id_from_window(value)
			.map_err(|_| ErrorType::Unknown)?;
		Self::from_pid_raw(res)
	}
}

impl Drop for Process {
	fn drop(&mut self) {
		if !self.handle.is_invalid() {
			close(self.handle).ok();
		}
	}
}