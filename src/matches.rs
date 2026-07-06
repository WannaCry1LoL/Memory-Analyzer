use crate::mem2disk::BlockDescriptor;
use crate::utils::MatchMask;

#[derive(Clone, Debug)]
pub struct MatchingBlock {
	/// Virtual Address of the Section
	pub base_address: usize,
	/// Bitmask for n byte aligned matches
	pub match_mask: MatchMask,
	/// Offset into the data vec
	pub data_offset: usize
}

impl MatchingBlock {
	pub fn new(base: BlockDescriptor, match_mask: MatchMask, data_offset: usize) -> Self {
		Self {
			base_address: base.virtual_address,
			match_mask,
			data_offset
		}
	}
}

#[derive(Clone, Debug)]
pub struct MatchingState {
	/// The alignment that was used to generate the scan
	pub alignment: usize,
	pub elem_size: usize,
	pub blocks: Vec<MatchingBlock>,
	/// Ordered data, contains the initial values. Same size as data.
	pub initial: Vec<u8>,
	/// Ordered data, contains the values of the previous scan.
	pub data: Vec<u8>
}

impl MatchingState {
	pub fn new(alignment: usize, elem_size: usize) -> Self {
		Self {
			alignment,
			elem_size,
			initial: vec![],
			blocks: vec![],
			data: vec![]
		}
	}

	pub fn push_block(&mut self, base_address: usize, mask: MatchMask, values: &[u8]) {
		debug_assert_eq!(values.len(), mask.count_ones() * self.elem_size);
		let offset = self.data.len();
		self.data.extend_from_slice(values);
		self.blocks.push(MatchingBlock { base_address, match_mask: mask, data_offset: offset });
	}

	pub fn push_narrowed_block(&mut self, base_address: usize, mask: MatchMask, initial_values: &[u8], current_values: &[u8]) {
		debug_assert_eq!(current_values.len(), mask.count_ones() * self.elem_size);
		debug_assert_eq!(initial_values.len(), current_values.len());
		debug_assert_eq!(self.data.len(), self.initial.len());
		let offset = self.data.len();
		self.data.extend_from_slice(current_values);
		self.initial.extend_from_slice(initial_values);
		self.blocks.push(MatchingBlock { base_address, match_mask: mask, data_offset: offset });
	}

	pub fn iter_matches(&self) -> impl Iterator<Item = (usize, &[u8])> + '_ {
		let (elem, align) = (self.elem_size, self.alignment);

		self.blocks
			.iter()
			.flat_map(move |block| {
				let start = block.data_offset;
				block.match_mask.iter_ones().enumerate().map(move |(k, i)| {
					let addr = block.base_address + i * align;
					let slice_start = start + k * elem;
					let slice_end = slice_start + elem;
					(addr, &self.data[slice_start..slice_end])
				})
			})
	}
}

