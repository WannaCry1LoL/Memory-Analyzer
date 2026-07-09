#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
	ProcessSelect,
	Scanner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
	Normal,
	Editing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
	ValueType,
	ScanMode,
	ValueInput,
	ValueInput2,
	Alignment,
	RangeStart,
	RangeEnd,
	Writable,
	Executable,
	CopyOnWrite,
	ScanBtn,
	NextScanBtn,
	ResultsList,
	SavedList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldTarget {
	Value,
	Value2,
	Alignment,
	RangeStart,
	RangeEnd,
	ProcessFilter,
	SavedValue,
	SavedDescription,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
	Byte,
	Word,
	Dword,
	Qword,
	Float,
	Double,
	ArrayOfByte,
}

impl ValueType {
	pub const ALL: [ValueType; 7] = [
		ValueType::Byte,
		ValueType::Word,
		ValueType::Dword,
		ValueType::Qword,
		ValueType::Float,
		ValueType::Double,
		ValueType::ArrayOfByte,
	];

	pub fn label(self) -> &'static str {
		match self {
			ValueType::Byte => "Byte",
			ValueType::Word => "2 Bytes",
			ValueType::Dword => "4 Bytes",
			ValueType::Qword => "8 Bytes",
			ValueType::Float => "Float",
			ValueType::Double => "Double",
			ValueType::ArrayOfByte => "Array of byte",
		}
	}

	pub fn is_numeric(self) -> bool {
		self != ValueType::ArrayOfByte
	}

	pub fn is_float(self) -> bool {
		matches!(self, ValueType::Float | ValueType::Double)
	}

	pub fn cycle(self, dir: i32) -> ValueType {
		let all = Self::ALL;
		let idx = all.iter().position(|v| *v == self).unwrap_or(0) as i32;
		let len = all.len() as i32;
		all[((idx + dir).rem_euclid(len)) as usize]
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
	ExactValue,
	BiggerThan,
	SmallerThan,
	ValueBetween
}

impl ScanMode {
	pub const ALL: [ScanMode; 4] = [
		ScanMode::ExactValue,
		ScanMode::BiggerThan,
		ScanMode::SmallerThan,
		ScanMode::ValueBetween
	];

	pub fn label(self) -> &'static str {
		match self {
			ScanMode::ExactValue => "Exact Value",
			ScanMode::BiggerThan => "Bigger than",
			ScanMode::SmallerThan => "Smaller than",
			ScanMode::ValueBetween => "Value between"
		}
	}

	pub fn needs_value(self) -> bool {
		matches!(
            self,
            ScanMode::ExactValue | ScanMode::BiggerThan | ScanMode::SmallerThan | ScanMode::ValueBetween
        )
	}

	pub fn needs_second_value(self) -> bool {
		matches!(self, ScanMode::ValueBetween)
	}

	pub fn cycle(self, dir: i32) -> ScanMode {
		let all = Self::ALL;
		let idx = all.iter().position(|v| *v == self).unwrap_or(0) as i32;
		let len = all.len() as i32;
		all[((idx + dir).rem_euclid(len)) as usize]
	}
}

#[derive(Debug, Clone)]
pub struct MatchEntry {
	pub address: usize,
	pub value: String,
	pub previous: String,
}

#[derive(Debug, Clone)]
pub struct SavedEntry {
	pub description: String,
	pub address: usize,
	pub value_type: ValueType,
	pub value: String,
	pub locked: bool,
}
