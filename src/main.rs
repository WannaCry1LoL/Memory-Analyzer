#![feature(f16)]
#![feature(f128)]

mod utils;
mod scanning;
mod externs;
mod memory_pattern; 
mod matches;
mod rw_helper;
mod process;
mod memory_scanner;
mod errors;

use std::collections::HashSet;
use std::fmt::Debug;
use std::io::{stdin, Read};
use std::process::exit;
use windows::Win32::{
	Foundation::CloseHandle,
	System::Threading::{OpenProcess, PROCESS_ALL_ACCESS},
};


use sysinfo::{Pid, System};
use windows::Win32::System::Threading::{CloseThreadpool, OpenThread, THREAD_GET_CONTEXT};
use crate::matches::MatchType;
use crate::memory_scanner::{MemoryScanner, MemoryScannerOptions, Pattern, PatternMatcher};
use crate::process::Process;
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
	const PROC_NAME: &str = "untitled.exe";
	let pid: Pid = get_proc_id_by_name(PROC_NAME).unwrap_or_else(|| {
		eprintln!("Unable to find process {}", PROC_NAME);
		exit(-1)
	});

	unsafe {
		//utils::get_proc_threads(handle).iter().for_each(|pid| {println!("{}", pid)});
		//scanning::find_pattern(handle, TEST_STR).iter().for_each(|val| { println!("{:#X}", val); });

		if let Some(pid) = find_proc_from_image_name(PROC_NAME) {
			if let Ok(handle) = OpenProcess(PROCESS_ALL_ACCESS, false, pid as u32) {
				let str = &mut String::new();
				let mut scanner = MemoryScanner::new(
					handle,
					Default::default(),
				);
				stdin().read_line(str);
				let initial = u128::from_str_radix(str.trim(), 16).unwrap();
				str.clear();
				let pattern_tmp = Pattern::new(
					PatternMatcher::Literal16(initial),
					MatchType::GenericBytes(16)
				);
				scanner.scan(pattern_tmp); // init
				scanner.print_matches();
				loop {
					let mut num = 0u128;
					let res = stdin().read_line(str);
					if res.is_ok() && let Ok(number) = u128::from_str_radix(str.trim(), 16) {
						scanner.narrow(Pattern::new(
							PatternMatcher::Literal16(number),
							MatchType::GenericBytes(16)
						), false);
						scanner.print_matches();
					} else {
						println!("Please enter a valid number");
						continue;
					}
					str.clear();
				}

			}
		}

	}
}