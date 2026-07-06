use crate::utils::TriState::{DontCare, False, True};
use crate::win_helpers::{enum_proc_info, EnumerationState};
use bitvec::vec::BitVec;
use std::collections::HashMap;


pub type MatchMask = BitVec<u64, bitvec::prelude::Lsb0>;

pub fn build_process_tree() -> HashMap<usize, Vec<usize>> {
	let mut ret: HashMap<usize, Vec<usize>> = HashMap::new();
	enum_proc_info(|info| {
		let ppid = info.inherited_from_unique_process_id;
		let pid = info.unique_process_id;
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
		if !info.image_name.Buffer.is_null()
			&& let Ok(res) = info.image_name.Buffer.to_string()
			&& res == name
		{ EnumerationState::Return(info.unique_process_id) } 
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

macro_rules! get {
	($e1:expr, $e2:expr) => { unsafe { $e1.get_unchecked($e2) } };
}

pub(crate) use get;
