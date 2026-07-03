use crate::win_helpers::{enum_proc_info, EnumerationState};
use std::collections::HashMap;
use bitvec::vec::BitVec;
use crate::utils::TriState::{DontCare, False, True};

pub type MatchMask = BitVec<u64, bitvec::prelude::Lsb0>;

pub fn build_process_tree() -> HashMap<usize, Vec<usize>> {
	let mut ret: HashMap<usize, Vec<usize>> = HashMap::new();
	enum_proc_info(|info| unsafe {
		let ppid = info.InheritedFromUniqueProcessId;
		let pid = info.UniqueProcessId;
		ret
			.entry(ppid)
			.or_default()
			.push(pid);
		
		EnumerationState::<()>::Continue
	});
	ret
}

pub fn find_proc_from_image_name(name: &str) -> Option<usize> {
	enum_proc_info(|info| unsafe {
		if !info.ImageName.Buffer.is_null()
			&& let Ok(res) = info.ImageName.Buffer.to_string()
			&& res == name
		{ EnumerationState::Return(info.UniqueProcessId) } 
		else { EnumerationState::Continue }
	})
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum TriState {
	DontCare = 2,
	True = 1,
	False = 0,
}

impl TriState {
	pub fn matches(&self, actual: bool) -> bool {
		match self {
			DontCare => true,
			True => actual,
			False => !actual,
		}
	}
}

impl From<bool> for TriState {
	fn from(v: bool) -> TriState { if v { True } else { False } }
}