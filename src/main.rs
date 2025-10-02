#![feature(let_chains)]

mod utils;
mod scanning;
mod externs;
mod MemoryPattern;

use std::fmt::Debug;
use std::process::exit;
use windows::Win32::{
	Foundation::CloseHandle,
	System::Threading::{OpenProcess, PROCESS_ALL_ACCESS},
};


use sysinfo::{Pid, System};
use windows::Win32::System::Threading::{CloseThreadpool, OpenThread, THREAD_GET_CONTEXT};
use crate::utils::{find_proc_from_image_name};

fn get_proc_id_by_name(proc_name: &str) -> Option<Pid> {
	let sys = System::new_all();
	for proc in sys.processes_by_name(proc_name.as_ref()) {
		return Some(proc.pid());
	}
	None
}

fn main() {
	const PROC_NAME: &str = "MemTestingProc.exe";
	const TEST_STR: &str = "abcdef";
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
		//utils::get_proc_threads(handle).iter().for_each(|pid| {println!("{}", pid)});
		//scanning::find_string(handle, TEST_STR).iter().for_each(|val| { println!("{:#X}", val); });

		&utils::build_process_tree_new().iter().for_each(|pid| {
			println!("{}: {}", pid.0, pid.1.iter().map(| x | { x.to_string() } ).collect::<Vec<_>>().join(", "));
		});

		println!("{}", find_proc_from_image_name(PROC_NAME).unwrap());

		//scanning::find_string(handle, TEST_STR).iter().for_each(|x| { println!("{x:#X}") });
		CloseHandle(handle).unwrap_or_else(|_| {
			eprintln!("Unable to close process {} ({})", PROC_NAME, pid.as_u32());
			exit(-1)
		});

	}
}