mod utils;

use std::process::exit;
use windows::Win32::{
	Foundation::CloseHandle,
	System::Threading::{OpenProcess, PROCESS_ALL_ACCESS},
};


use sysinfo::{Pid, System};

fn get_proc_id_by_name(proc_name: &str) -> Option<Pid> {
	let sys = System::new_all();
	for proc in sys.processes_by_name(proc_name.as_ref()) {
		return Some(proc.pid());
	}
	None
}

fn main() {
	const PROC_NAME: &str = "MemTestingProc.exe";
	const TEST_STR: &str = "this is so peak trust me   ";
	let pid: Pid = get_proc_id_by_name(PROC_NAME).unwrap_or_else(|| {
		eprintln!("Unable to find process {}", PROC_NAME);
		exit(-1)
	});

	unsafe {
		let handle = OpenProcess(
			PROCESS_ALL_ACCESS,
			false,
			pid.as_u32()
		).unwrap_or_else(|_| {
			eprintln!("Unable to open process {} ({})", PROC_NAME, pid.as_u32());
			exit(-1)
		});
		
		
		CloseHandle(handle).unwrap_or_else(|_| {
			eprintln!("Unable to close process {} ({})", PROC_NAME, pid.as_u32());
			exit(-1)
		});

	}
}