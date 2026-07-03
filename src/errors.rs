use windows::Win32::Foundation::{NTSTATUS, WIN32_ERROR};

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

#[derive(Debug)]
pub enum MemoryScanError {
	/// The memory dump failed because an error occured while dumping process memory
	MemoryDumpFailed(std::io::Error),
}

#[derive(Debug)]
pub enum MemoryReadError {
	/// NtReadVirtualMemory returned less bytes read than requested
	NotEnoughBytesRead,

	/// The passed buffer was too small
	BufferTooSmall(usize),

	/// The memory is not valid for the selected settings (ie PAGE_EXECUTE_READWRITE with execute set to False)
	UnsuitableMemory,

	Other(NTSTATUS),
}

#[derive(Debug)]
pub enum MemoryWriteError {
	/// NtWriteVirtualMemory returned less bytes written than requested
	NotEnoughBytesWritten,
	Other(NTSTATUS),
}

#[derive(Debug)]
pub enum ProcessImagePathError {
	/// Windows returned invalid UTF 16
	InvalidUtf16ImagePath,
	Unknown(WIN32_ERROR)
}