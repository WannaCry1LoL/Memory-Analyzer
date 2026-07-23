use crate::ffi::types::{NtStatus, PageProtectionFlags, SystemInformationClass, Win32Error};

pub const MEM_COMMIT: u32 = 0x1000;
pub const MEM_PRIVATE: u32 = 0x20000;
pub const MEM_IMAGE: u32 = 0x1000000;

pub const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;
pub const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
pub const TOKEN_QUERY: u32 = 0x0008;

pub const PROCESS_NAME_FORMAT_WIN32: u32 = 0;
pub const PROCESS_ALL_ACCESS: u32 = 0x001F0FFF;
pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

pub const GWL_EXSTYLE: i32 = -20;
pub const WS_EX_TOOLWINDOW: isize = 0x00000080;

pub const OBJ_INHERIT: u32 = 0x00000002;

pub const PROCESS_IMAGE_FILENAME_WIN32: u32 = 43;

impl NtStatus {
	pub const SUCCESS: Self = Self(0);
	pub const STATUS_INFO_LENGTH_MISMATCH: Self = Self(0xC0000004_u32 as _);
	pub const STATUS_ACCESS_DENIED: Self = Self(0xC0000022_u32 as _);
	pub const STATUS_INVALID_HANDLE: Self = Self(0xC0000008_u32 as _);
}

impl Win32Error {
	pub const ERROR_ACCESS_DENIED: Win32Error = Win32Error(5);
	pub const ERROR_INVALID_HANDLE: Win32Error = Win32Error(6);
	pub const ERROR_NOT_ALL_ASSIGNED: Win32Error = Win32Error(1300);
}

impl PageProtectionFlags {
	pub const PAGE_READWRITE: Self = Self(0x04);
	pub const PAGE_WRITECOPY: Self = Self(0x08);
	pub const PAGE_EXECUTE: Self = Self(0x10);
	pub const PAGE_EXECUTE_READ: Self = Self(0x20);
	pub const PAGE_EXECUTE_READWRITE: Self = Self(0x40);
	pub const PAGE_EXECUTE_WRITECOPY: Self = Self(0x80);
	
	pub const MEM_WRITABLE: Self = Self(Self::PAGE_READWRITE.0 | Self::PAGE_EXECUTE_READWRITE.0);
	pub const MEM_EXECUTABLE: Self = Self(
		Self::PAGE_EXECUTE.0
			| Self::PAGE_EXECUTE_READ.0
			| Self::PAGE_EXECUTE_READWRITE.0
			| Self::PAGE_EXECUTE_WRITECOPY.0,
	);
	pub const MEM_COW: Self = Self(Self::PAGE_WRITECOPY.0 | Self::PAGE_EXECUTE_WRITECOPY.0);
}

impl SystemInformationClass {
	pub const SYSTEM_BASIC_PROCESS_INFORMATION: Self = Self(252i32);
}

