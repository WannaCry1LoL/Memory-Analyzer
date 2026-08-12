use crate::errors::{ElevationError, MemoryReadError, MemoryWriteError};
use crate::ffi::constants::*;
use crate::ffi::ffi::*;
use crate::ffi::types::*;
use std::any::Any;
use std::default::Default;
use std::ffi::c_void;
use std::ops::ControlFlow;
use std::os::windows::prelude::*;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::ptr::{null, null_mut};

unsafe fn adjust_token_privileges(
	token_handle: Handle,
	disable_all_privileges: bool,
	new_state: Option<&TokenPrivileges>,
	buffer_length: u32,
	previous_state: Option<&mut TokenPrivileges>,
	return_length: Option<&mut u32>,
) -> Result<(), NtStatus> { unsafe {
	NtAdjustPrivilegesToken(
		token_handle,
		disable_all_privileges as u8,
		new_state,
		buffer_length,
		previous_state,
		return_length,
	)
		.into_result()
} }

pub unsafe fn open_process(
	desired_access: u32,
	inherit_handle: bool,
	process_id: Pid,
) -> Result<Handle, NtStatus> {
	let mut handle: Handle = Default::default();

	let mut client_id = ClientId {
		unique_process: Handle(process_id.0 as _),
		unique_thread: Default::default(),
	};

	let mut object_attributes = ObjectAttributes {
		length: size_of::<ObjectAttributes>() as u32,
		root_directory: Default::default(),
		object_name: Default::default(),
		attributes: if inherit_handle { OBJ_INHERIT } else { 0 },
		security_descriptor: null_mut(),
		security_quality_of_service: null_mut(),
	};

	unsafe {
		let status = NtOpenProcess(
			&mut handle,
			desired_access,
			&mut object_attributes,
			Some(&mut client_id),
		);

		status.into_result().map(|_| handle)
	}
}

pub unsafe fn open_process_token(
	process_handle: Handle,
	desired_access: u32,
) -> Result<Handle, NtStatus> {
	let mut token_handle: Handle = Default::default();

	unsafe {
		NtOpenProcessToken(process_handle, desired_access, &mut token_handle)
			.into_result()
			.map(|_| token_handle)
	}
}

pub unsafe fn query_full_process_image_name(
	process_handle: Handle,
	buffer: &mut [u16],
) -> Result<u32, NtStatus> {
	let buff_len = buffer.len() * 2;
	let mut return_length = 0;

	unsafe {
		NtQueryInformationProcess(
			process_handle,
			PROCESS_IMAGE_FILENAME_WIN32,
			buffer.as_mut_ptr() as _,
			buff_len as _,
			Some(&mut return_length),
		)
			.into_result()
			.map(|_| return_length)
	}
}

pub unsafe fn read_bytes(
	handle: Handle,
	address: *const u8,
	buffer: *mut u8,
	len: usize,
) -> Result<usize, MemoryReadError> {
	let mut read_bytes: usize = 0;

	let status = unsafe { NtReadVirtualMemory(
		handle,
		address as _,
		buffer as _,
		len,
		Some(&mut read_bytes),
	) };

	if read_bytes == 0 {
		return status
			.into_result()
			.map_err(MemoryReadError::Other)
			.map(|_| 0);
	}

	Ok(read_bytes)
}

pub unsafe fn write_bytes(
	handle: Handle,
	address: *const u8,
	buffer: *const u8,
	len: usize,
) -> Result<(), MemoryWriteError> {
	let mut written_bytes: usize = 0;

	let status = unsafe { NtWriteVirtualMemory(
		handle,
		address as _,
		buffer as _,
		len,
		Some(&mut written_bytes),
	) };

	if written_bytes != len {
		return Err(MemoryWriteError::NotEnoughBytesWritten);
	}

	status.into_result().map_err(MemoryWriteError::Other)
}

pub fn virtual_query_ex(
	process_handle: Handle,
	base_address: *const c_void,
) -> Result<(MemoryBasicInformation, usize), NtStatus> {
	let mut mem_info = Default::default();
	let mut return_length = 0;

	unsafe {
		NtQueryVirtualMemory(
			process_handle,
			base_address,
			0,
			&mut mem_info,
			size_of::<MemoryBasicInformation>(),
			Some(&mut return_length),
		)
			.into_result()
			.map(|_| (mem_info, return_length))
	}
}

pub fn get_process_id(process_handle: Handle) -> Result<Pid, NtStatus> {
	let mut pbi = ProcessBasicInformation::default();
	let mut return_length = 0;

	unsafe {
		NtQueryInformationProcess(
			process_handle,
			0,
			&raw mut pbi as _,
			size_of::<ProcessBasicInformation>() as u32,
			Some(&mut return_length),
		)
			.into_result()
			.map(|_| (pbi.unique_process_id as u32).into())
	}
}

pub fn enable_debug_privilege() -> Result<(), ElevationError> { unsafe {
	let token = open_process_token(
		Handle::CURRENT_PROCESS,
		TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
	)
		.map_err(|_| ElevationError::Unknown)?;

	let guard = OwnedHandle::from_raw_handle(token.as_raw_handle());

	let mut luid = Luid::default();
	let priv_name: Vec<u16> = "SeDebugPrivilege\0".encode_utf16().collect();

	if LookupPrivilegeValueW(null(), priv_name.as_ptr(), &mut luid) == 0 {
		return Err(ElevationError::PrivilegeLookupFailed)
	};

	let mut tp = TokenPrivileges::default();
	tp.privilege_count = 1;
	tp.privileges[0].luid = luid;
	tp.privileges[0].attributes = SE_PRIVILEGE_ENABLED;

	adjust_token_privileges(
		guard.as_handle().into(),
		false,
		Some(&tp),
		size_of::<TokenPrivileges>() as _,
		None,
		None,
	)
		.map_err(|_| ElevationError::PrivilegeAdjustmentFailed)?;

	match RtlGetLastNtStatus() {
		NtStatus::STATUS_NOT_ALL_ASSIGNED => Err(ElevationError::PrivilegeNotAssigned),
		_ => Ok(())
	}
} }

pub fn close(handle: Handle) -> Result<(), NtStatus> { unsafe {
	NtClose(handle).into_result()
} }

pub fn suspend_process(handle: Handle) -> Result<(), NtStatus> { unsafe {
	NtSuspendProcess(handle).into_result()
} }

pub fn resume_process(handle: Handle) -> Result<(), NtStatus> { unsafe {
	NtResumeProcess(handle).into_result()
} }

pub fn get_process_id_from_window(hwnd: Hwnd) -> Result<Pid, Win32Error> { unsafe {
	let mut pid = Pid::default();
	let res = GetWindowThreadProcessId(hwnd, Some(&mut pid));
	if res == 0 {
		return RtlGetLastWin32Error()
			.into_result()
			.map(|_| Pid::default());
	}
	Ok(pid)
} }

pub fn is_valid_target_window(hwnd: Hwnd) -> bool { unsafe {
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

struct EnumContext<'a, F, B> {
	callback: &'a mut F,
	result: ControlFlow<B>,
	panic: Option<Box<dyn Any + Send + 'static>>,
}

unsafe extern "system" fn trampoline<F, B>(hwnd: Hwnd, lparam: isize) -> i32
where
	F: FnMut(Hwnd) -> ControlFlow<B>,
{
	let ctx = unsafe { (lparam as *mut EnumContext<F, B>).as_mut().unwrap() };
	let res = catch_unwind(AssertUnwindSafe(|| { (ctx.callback)(hwnd) }));

	match res {
		Ok(ControlFlow::Continue(())) => 1,
		Ok(ControlFlow::Break(value)) => {
			ctx.result = ControlFlow::Break(value);
			0
		}
		Err(payload) => {
			ctx.panic = Some(payload);
			0
		}
	}
}

pub fn enum_windows<F: FnMut(Hwnd) -> ControlFlow<B>, B>(mut callback: F) -> ControlFlow<B> {
	let mut ctx = EnumContext {
		callback: &mut callback,
		result: ControlFlow::Continue(()),
		panic: None,
	};

	unsafe {
		let l_param = &raw mut ctx as isize;
		let _ = EnumWindows(Some(trampoline::<F, B>), l_param);
	}

	if let Some(payload) = ctx.panic {
		resume_unwind(payload);
	}

	ctx.result
}