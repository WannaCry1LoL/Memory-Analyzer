#![feature(portable_simd)]
#![feature(associated_type_defaults)]
extern crate core;

mod utils;
mod externs;
mod matches;
mod win_helpers;
mod process;
mod memory_scanner;
mod errors;
mod num_helpers;
mod mem2disk;
mod simd_types;
mod ui;
mod ui_types;

use sysinfo::{Pid, System};

fn get_proc_id_by_name(proc_name: &str) -> Option<Pid> {
	let sys = System::new_all();
	sys.processes_by_name(proc_name.as_ref()).next().map(|a| a.pid())
}

fn main() {}