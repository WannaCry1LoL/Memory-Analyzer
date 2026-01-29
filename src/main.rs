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

use std::fmt::Debug;
use std::io::{stdin, Read};
use std::process::exit;
use std::time::Instant;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS};


use crate::matches::MatchType;
use crate::memory_scanner::{MemoryScanner, Pattern, PatternMatcher};
use crate::utils::find_proc_from_image_name;
use sysinfo::{Pid, System};

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
				let str = &mut String::new();
				let mut scanner = MemoryScanner::new(
					handle,
					Default::default(),
				);
				stdin().read_line(str);
				let initial = str::parse::<f32>(str.trim()).unwrap();
				let pattern_tmp = Pattern::new(
					PatternMatcher::Single(initial),
					MatchType::Float
				);
				let mut time = Instant::now();
				scanner.scan(pattern_tmp); // init
				println!("Done with initial scan. Took {} seconds", time.elapsed().as_secs());
				scanner.print_matches();
				str.clear();
				loop {
					let res = stdin().read_line(str);
					if res.is_ok() && let Ok(number) = str::parse::<f32>(str.trim()) {
						time = Instant::now();
						scanner.narrow(Pattern::new(
							PatternMatcher::Single(number),
							MatchType::Float
						), false);
						println!("Done with scan. Took {} seconds", time.elapsed().as_secs());
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