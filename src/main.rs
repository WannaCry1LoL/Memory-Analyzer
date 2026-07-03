#![feature(portable_simd)]
#![feature(associated_type_defaults)]
#![feature(integer_casts)]
extern crate core;

mod utils;
mod externs;
mod matches;
mod win_helpers;
mod process;
mod memory_scanner;
mod errors;
pub mod num_helpers;
pub mod mem2disk;
mod simd_types;

use std::process::exit;

use sysinfo::{Pid, System};

fn get_proc_id_by_name(proc_name: &str) -> Option<Pid> {
	let sys = System::new_all();
	for proc in sys.processes_by_name(proc_name.as_ref()) {
		return Some(proc.pid());
	}
	None
}

fn main() {}