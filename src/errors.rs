use crate::externs::NtStatus;
#[derive(Debug)]
pub enum ProcessError {
	/// The handle is invalid
	InvalidHandle,
	/// The HWND is invalid
	InvalidHWND,
	/// The PID is invalid
	InvalidPid,
	/// The Handle is not a process handle
	NonProcessHandle,
	/// The handle does not have PROCESS_QUERY_LIMITED_INFORMATION
	HandleLacksPermissions,
	/// Failed due to a process permission error
	NoAccess,
	/// Failed due to an unknown error
	Unknown,
}

pub enum ProcessOperationError {
	/// Suspend failed because the own process may not be suspended
	CannotSuspendSelf,
	SuspendFailed(NtStatus),
	ResumeFailed(NtStatus)
}

#[derive(Debug)]
pub enum MemoryScanError {
	/// The memory dump failed because an error occurred while dumping process memory
	MemoryDumpFailed(std::io::Error),
	MemoryReadFailed(MemoryReadError),
	HandleInitFailed,
	IncompatibleScanType,
}

#[derive(Debug)]
pub enum MemoryReadError {
	/// NtReadVirtualMemory returned fewer bytes read than requested
	NotEnoughBytesRead,

	/// The passed buffer was too small
	BufferTooSmall(usize),

	/// The memory is not valid for the selected settings (ie PAGE_EXECUTE_READWRITE with execute set to False)
	UnsuitableMemory,

	Other(NtStatus),
}

#[derive(Debug)]
pub enum MemoryWriteError {
	/// NtWriteVirtualMemory returned fewer bytes written than requested
	NotEnoughBytesWritten,
	Other(NtStatus),
}

#[derive(Debug)]
pub enum ProcessImagePathError {
	/// Windows returned invalid UTF 16
	InvalidUtf16ImagePath,
	Unknown(NtStatus),
}

#[derive(Debug)]
pub enum ElevationError {
	/// Failed to lookup LUID for SeDebugPrivilege
	PrivilegeLookupFailed,
	/// Windows reports that the privilege was not assigned.
	PrivilegeNotAssigned,
	/// NtAdjustPrivilegesToken returned a non-success ntstatus
	PrivilegeAdjustmentFailed,
	Unknown
}