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
pub enum UiValueType {
	Byte,
	Word,
	Dword,
	Qword,
	Float,
	Double,
	ArrayOfByte,
}

impl UiValueType {
	pub const ALL: [UiValueType; 7] = [
		UiValueType::Byte,
		UiValueType::Word,
		UiValueType::Dword,
		UiValueType::Qword,
		UiValueType::Float,
		UiValueType::Double,
		UiValueType::ArrayOfByte,
	];

	pub fn label(self) -> &'static str {
		match self {
			UiValueType::Byte => "Byte",
			UiValueType::Word => "2 Bytes",
			UiValueType::Dword => "4 Bytes",
			UiValueType::Qword => "8 Bytes",
			UiValueType::Float => "Float",
			UiValueType::Double => "Double",
			UiValueType::ArrayOfByte => "Array of byte",
		}
	}

	pub fn is_numeric(self) -> bool {
		self != UiValueType::ArrayOfByte
	}

	pub fn is_float(self) -> bool {
		matches!(self, UiValueType::Float | UiValueType::Double)
	}

	pub fn cycle(self, dir: i32) -> UiValueType {
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
	pub value_type: UiValueType,
	pub value: String,
	pub locked: bool,
}
