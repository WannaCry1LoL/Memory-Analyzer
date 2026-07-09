use crate::utils::TriState::{DontCare, False, True};
use crate::win_helpers::{enum_proc_info, EnumerationState};
use bitvec::vec::BitVec;
use std::collections::HashMap;
use std::ops::Deref;
use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_NOT_ALL_ASSIGNED, HANDLE, LUID};
use windows::Win32::Security::{AdjustTokenPrivileges, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub type MatchMask = BitVec<u64, bitvec::prelude::Lsb0>;

pub fn build_process_tree() -> HashMap<usize, Vec<usize>> {
	let mut ret: HashMap<usize, Vec<usize>> = HashMap::new();
	enum_proc_info(|info| {
		let ppid = info.inherited_from_unique_process_id;
		let pid = info.unique_process_id;
		ret
			.entry(ppid)
			.or_default()
			.push(pid);
		
		EnumerationState::<()>::Continue
	});
	ret
}

pub fn find_proc_from_image_name(name: &str) -> Option<usize> {
	enum_proc_info(|info| unsafe {
		if !info.image_name.Buffer.is_null()
			&& let Ok(res) = info.image_name.Buffer.to_string()
			&& res == name
		{ EnumerationState::Return(info.unique_process_id) } 
		else { EnumerationState::Continue }
	})
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum TriState {
	False = 0,
	True = 1,
	DontCare = 2,
}

impl TriState {
	pub fn matches(&self, actual: bool) -> bool {
		match self {
			DontCare => true,
			True => actual,
			False => !actual,
		}
	}

	pub fn cycle(self) -> Self {
		match self {
			DontCare => True,
			True => False,
			False => DontCare,
		}
	}

	pub fn label(&self) -> &'static str {
		match self {
			True => "x",
			False => " ",
			DontCare => "-",
		}
	}
}

impl From<bool> for TriState {
	fn from(v: bool) -> TriState { if v { True } else { False } }
}

pub struct HandleGuard<'a> {
	handle: &'a mut HANDLE,
}

impl Drop for HandleGuard<'_> {
	fn drop(&mut self) { let _ = unsafe { CloseHandle(*self.handle) }; }
}

impl Deref for HandleGuard<'_> {
	type Target = HANDLE;
	fn deref(&self) -> &HANDLE { self.handle }
}

pub fn enable_debug_privilege() -> Result<(), ElevationError> {
	unsafe {
		let mut h_token = HANDLE::default();

		OpenProcessToken(
			GetCurrentProcess(),
			TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
			&mut h_token
		)?;

		let handle = HandleGuard { handle: &mut h_token };

		let mut luid = LUID::default();

		LookupPrivilegeValueW(None, w!("SeDebugPrivilege"), &mut luid)?;

		let mut tp = TOKEN_PRIVILEGES::default();
		tp.PrivilegeCount = 1;
		tp.Privileges[0].Luid = luid;
		tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;

		AdjustTokenPrivileges(
			*handle,
			false,
			Some(&tp),
			size_of::<TOKEN_PRIVILEGES>() as _,
			None,
			None,
		)?;

		match GetLastError() {
			ERROR_NOT_ALL_ASSIGNED => Err(ElevationError::PrivilegeNotAssigned),
			_ => Ok(())
		}
	}
}

macro_rules! get {
	($e1:expr, $e2:expr) => { unsafe { $e1.get_unchecked($e2) } };
}

pub(crate) use get;
use crate::errors::ElevationError;
