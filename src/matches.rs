use crate::utils::MatchMask;

#[derive(Clone, Debug)]
pub struct MatchingBlock {
	/// Virtual Address of the Section
	pub base_address: usize,
	/// Bitmask for n byte aligned matches
	pub match_mask: MatchMask,
	/// Offset into the data vec
	pub data_offset: usize,
}

#[derive(Clone, Debug)]
pub struct MatchingState {
	/// The alignment that was used to generate the scan
	pub alignment: usize,
	pub elem_size: usize,
	pub initial_scan: bool,
	pub blocks: Vec<MatchingBlock>,
	/// Ordered data, contains the initial values. Same size as data.
	pub initial: Vec<u8>,
	/// Ordered data, contains the values of the previous scan.
	pub data: Vec<u8>,
}

impl MatchingState {
	pub fn new(alignment: usize, elem_size: usize, initial_scan: bool) -> Self {
		Self {
			alignment,
			elem_size,
			initial_scan,
			initial: vec![],
			blocks: vec![],
			data: vec![],
		}
	}
	
	pub fn get_initial(&self) -> &Vec<u8> { 
		self.initial_scan
			.then_some(&self.data)
			.unwrap_or(&self.initial)
	}

	pub fn push_block(&mut self, base_address: usize, mask: MatchMask, values: &[u8]) {
		debug_assert_eq!(values.len(), mask.count_ones() * self.elem_size);
		let offset = self.data.len();
		self.data.extend_from_slice(values);
		self.blocks.push(MatchingBlock {
			base_address,
			match_mask: mask,
			data_offset: offset,
		});
	}

	pub fn push_narrowed_block(
		&mut self,
		base_address: usize,
		mask: MatchMask,
		initial_values: &[u8],
		current_values: &[u8],
	) {
		debug_assert_eq!(current_values.len(), mask.count_ones() * self.elem_size);
		debug_assert_eq!(initial_values.len(), current_values.len());
		debug_assert_eq!(self.data.len(), self.initial.len());
		let offset = self.data.len();
		self.data.extend_from_slice(current_values);
		self.initial.extend_from_slice(initial_values);
		self.blocks.push(MatchingBlock {
			base_address,
			match_mask: mask,
			data_offset: offset,
		});
	}

	pub fn iter_matches(&self) -> impl Iterator<Item = (usize, &[u8])> + '_ {
		let (elem, align) = (self.elem_size, self.alignment);

		self.blocks.iter().flat_map(move |block| {
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

impl MatchingState {
	pub fn total_matches(&self) -> usize {
		self.blocks.iter().map(|b| b.match_mask.count_ones()).sum()
	}

	pub fn get_match(&self, index: usize) -> Option<(usize, &[u8], &[u8])> {
		let mut remaining = index;
		let initial_data = self.get_initial();

		for block in &self.blocks {
			let count = block.match_mask.count_ones();
			if remaining < count {
				let mut local_idx = 0;
				for i in block.match_mask.iter_ones() {
					if local_idx == remaining {
						let addr = block.base_address + i * self.alignment;
						let slice_start = block.data_offset + local_idx * self.elem_size;
						let slice_end = slice_start + self.elem_size;

						return Some((
							addr,
							&self.data[slice_start..slice_end],
							&initial_data[slice_start..slice_end],
						));
					}
					local_idx += 1;
				}
			}
			remaining -= count;
		}
		None
	}
}