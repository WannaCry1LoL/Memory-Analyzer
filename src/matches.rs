use bitvec::prelude::*;
use crate::mem2disk::BlockDescriptor;
use crate::utils::MatchMask;

#[derive(Clone, Debug)]
pub struct MatchingBlock {
	/// Virtual Address of the Section
	pub base_address: usize,
	/// Bitmask for n byte aligned matches
	pub match_mask: MatchMask
}

impl MatchingBlock {
	pub fn new(base: BlockDescriptor, indices: Vec<usize>, alignment: usize) -> Self {
		debug_assert!(!indices.is_empty());
		debug_assert_ne!(alignment, 0);

		let mut bv = BitVec::repeat(false, indices.last().unwrap() / alignment + 1);

		for i in indices {
			debug_assert_eq!(i % alignment, 0);
			bv.set(i / alignment, true);
		}

		MatchingBlock {
			base_address: base.virtual_address,
			match_mask: bv
		}
	}
}

#[derive(Clone, Debug)]
pub struct MatchingState {
	/// The alignment that was used to generate the scan
	pub alignment: usize,
	pub blocks: Vec<MatchingBlock>,
	/// Ordered data, contains the values of the previous scan.
	pub data: Vec<u8>
}

impl MatchingState {
	pub fn new(alignment: usize) -> Self {
		Self {
			alignment,
			blocks: vec![],
			data: vec![]
		}
	}
}