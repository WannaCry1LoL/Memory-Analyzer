#![feature(f16)]
#![feature(f128)]

mod utils;
mod scanning;
mod externs;
mod memory_pattern; 
mod matches;
mod rw_helper;
mod Process;
mod memory_scanner;
mod errors;

use std::collections::HashSet;
use std::fmt::Debug;
use std::process::exit;
use windows::Win32::{
	Foundation::CloseHandle,
	System::Threading::{OpenProcess, PROCESS_ALL_ACCESS},
};


use sysinfo::{Pid, System};
use windows::Win32::System::Threading::{CloseThreadpool, OpenThread, THREAD_GET_CONTEXT};
use crate::scanning::{find_pattern, find_string};
use crate::utils::{build_process_tree, find_proc_from_image_name};

fn get_proc_id_by_name(proc_name: &str) -> Option<Pid> {
	let sys = System::new_all();
	for proc in sys.processes_by_name(proc_name.as_ref()) {
		return Some(proc.pid());
	}
	None
}

fn main() {
	const PROC_NAME: &str = "Blood West.exe";
	let pid: Pid = get_proc_id_by_name(PROC_NAME).unwrap_or_else(|| {
		eprintln!("Unable to find process {}", PROC_NAME);
		exit(-1)
	});

	unsafe {
		//utils::get_proc_threads(handle).iter().for_each(|pid| {println!("{}", pid)});
		//scanning::find_pattern(handle, TEST_STR).iter().for_each(|val| { println!("{:#X}", val); });

		if let Some(pid) = find_proc_from_image_name(PROC_NAME) {
			if let Ok(handle) = OpenProcess(PROCESS_ALL_ACCESS, false, pid as u32) {
				find_pattern(handle, 3090f32).iter().for_each(|s| { println!("{}", s); });
			}
		}

	}
}