
#[derive(Clone, Copy, Debug)]
pub enum MatchType {
	Generic1Byte,
	Generic2Byte,
	Generic4Byte,
	Generic8Byte,
	Generic16Byte,
	Float,
	Double,
	Boolean,
	GenericBytes(usize),
	StringUTF8(usize),
	StringUTF16(usize),
}

impl MatchType {
	pub fn size(self) -> usize {
		match self {
			MatchType::Generic1Byte => 1,
			MatchType::Generic2Byte => 2,
			MatchType::Generic4Byte => 4,
			MatchType::Generic8Byte => 8,
			MatchType::Generic16Byte => 16,
			MatchType::Float => 4,
			MatchType::Double => 8,
			MatchType::Boolean => 1,
			MatchType::GenericBytes(size) => size,
			MatchType::StringUTF8(size) => size,
			MatchType::StringUTF16(size) => size,
		}
	}
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct RegionId(pub usize);

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct MatchLocation {
	pub offset: usize,
}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct MatchValue {
	pub value: Vec<u8>
}

pub struct Match {
	pub match_type: MatchType,
	pub location: MatchLocation,

	pub value: MatchValue,
	pub initial_value: MatchValue
}

impl Match {
	pub fn new(match_type: MatchType, location: usize, value: &[u8], x: &[u8]) -> Self {
		Self {
			match_type,
			location: MatchLocation { offset: location },
			value: MatchValue { value: value.to_vec() } ,
			initial_value: MatchValue { value: value.to_vec() }
		}
	}
}