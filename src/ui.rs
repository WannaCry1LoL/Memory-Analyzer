use std::default::Default;
use crate::matches::MatchingState;
use crate::memory_scanner::{MemoryScanner, MemoryScannerOptions, ScanRequest, IntCondition, FloatCondition, ValueType};
use crate::process::Process;
use crate::ui_types::*;
use crate::utils::{string_to_bytes_hex, TriState};

use ratatui::{
	layout::{Alignment, Constraint, Direction, Layout, Rect},
	style::{Color, Modifier, Style},
	text::{Line, Span},
	widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table},
	Frame,
};

use std::sync::{mpsc};
use std::thread;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const ACCENT: Color = Color::Yellow;
const EDIT: Color = Color::Green;
const DIM: Color = Color::DarkGray;
const HEADER_BG: Color = Color::Rgb(30, 30, 40);

pub enum ScanEvent {
	Finished {
		scanner: MemoryScanner,
		result: Result<(), String>,
		was_narrow: bool,
		elapsed: Duration,
	},
}

pub enum WorkerCommand {
	Write { address: usize, data: Vec<u8> },
	AddLock { id: usize, address: usize, data: Vec<u8> },
	RemoveLock { id: usize },
	ReadBatch { addresses: Vec<(usize, usize)> },
	Stop
}

pub enum WorkerEvent {
	WriteResult { id: Option<usize>, success: bool },
	ReadResult { values: HashMap<usize, Vec<u8>> },
	LockFailed { id: usize }
}

pub fn memory_worker_loop(
	process: Process,
	cmd_rx: mpsc::Receiver<WorkerCommand>,
	event_tx: mpsc::Sender<WorkerEvent>,
) {
	let mut locks: HashMap<usize, (usize, Vec<u8>)> = HashMap::new();

	loop {
		let mut should_continue = true;

		match cmd_rx.recv_timeout(Duration::from_millis(16)) {
			Ok(cmd) => should_continue = process_command(cmd, &process, &mut locks, &event_tx),
			Err(mpsc::RecvTimeoutError::Timeout) => {},
			Err(mpsc::RecvTimeoutError::Disconnected) => break,
		}

		if !should_continue { break; }

		while let Ok(cmd) = cmd_rx.try_recv() {
			if !process_command(cmd, &process, &mut locks, &event_tx) {
				should_continue = false;
				break;
			}
		}

		if !should_continue { break; }

		let mut failed_locks = Vec::new();
		for (id, (addr, data)) in &locks {
			if process.write(*addr as _, data).is_err() {
				failed_locks.push(*id);
			}
		}

		for id in failed_locks {
			locks.remove(&id);
			let _ = event_tx.send(WorkerEvent::LockFailed { id });
		}
	}
}

fn process_command(
	cmd: WorkerCommand,
	process: &Process,
	locks: &mut HashMap<usize, (usize, Vec<u8>)>,
	event_tx: &mpsc::Sender<WorkerEvent>
) -> bool {
	match cmd {
		WorkerCommand::Write { address, data } => {
			let success = process.write(address as _, &data).is_ok();
			let _ = event_tx.send(WorkerEvent::WriteResult { id: Some(address), success });
			true
		}
		WorkerCommand::AddLock { id, address, data } => {
			locks.insert(id, (address, data));
			true
		}
		WorkerCommand::RemoveLock { id } => {
			locks.remove(&id);
			true
		}
		WorkerCommand::ReadBatch { addresses } => {
			let mut values = HashMap::new();
			for (addr, size) in addresses {
				if let Ok(data) = process.read(addr as _, size) {
					values.insert(addr, data);
				}
			}
			let _ = event_tx.send(WorkerEvent::ReadResult { values });
			true
		}
		WorkerCommand::Stop => false
	}
}

pub struct App {
	pub screen: Screen,
	pub input_mode: InputMode,
	pub should_quit: bool,

	pub processes: Vec<Process>,
	pub process_filter: String,
	pub process_list_state: usize,

	pub scanner: Option<MemoryScanner>,
	pub has_first_scan: bool,
	pub scan_count: u32,

	pub value_type: UiValueType,
	pub scan_mode: ScanMode,
	pub value_input: String,
	pub value_input2: String,
	pub alignment: String,
	pub range_start: String,
	pub range_end: String,
	pub freeze_process: bool,
	pub writable: TriState,
	pub executable: TriState,
	pub cow: TriState,
	pub rounding_mode: RoundingMode,

	pub focus: Focus,
	pub editing_target: Option<FieldTarget>,
	pub edit_buffer: String,

	pub results_selected: usize,

	pub saved: Vec<SavedEntry>,
	pub saved_selected: usize,

	pub status: String,

	pub is_scanning: bool,
	pub scan_rx: Option<mpsc::Receiver<ScanEvent>>,

	pub worker_rx: Option<mpsc::Receiver<WorkerEvent>>,
	pub worker_tx: Option<mpsc::Sender<WorkerCommand>>,

	pub visible_matches: Vec<MatchEntry>,
	pub visible_start_idx: usize,
	pub visible_end_idx: usize,
	pub scan_just_finished: bool,
	pub last_value_refresh: Instant,
}

impl App {
	pub fn new() -> Self {
		Self {
			screen: Screen::ProcessSelect,
			input_mode: InputMode::Normal,
			should_quit: false,

			processes: Process::get_valid_windows_processes(),
			process_filter: String::new(),
			process_list_state: 0,

			scanner: None,
			has_first_scan: false,
			scan_count: 0,

			value_type: UiValueType::Dword,
			scan_mode: ScanMode::ExactValue,
			value_input: String::new(),
			value_input2: String::new(),
			alignment: "4".into(),
			range_start: "0".into(),
			range_end: "00007FFFFFFFFFFF".into(),
			freeze_process: false,
			writable: TriState::True,
			executable: TriState::DontCare,
			cow: TriState::False,
			rounding_mode: RoundingMode::Default,

			focus: Focus::ValueType,
			editing_target: None,
			edit_buffer: String::new(),

			results_selected: 0,

			saved: Vec::new(),
			saved_selected: 0,

			status: "Select a process and press Enter to attach".into(),

			is_scanning: false,
			scan_rx: None,

			worker_rx: None,
			worker_tx: None,

			visible_matches: Vec::new(),
			visible_start_idx: 0,
			visible_end_idx: 0,
			scan_just_finished: false,
			last_value_refresh: Instant::now(),
		}
	}

	/* Process Select */

	pub fn visible_processes(&self) -> Vec<&Process> {
		if self.process_filter.trim().is_empty() {
			self.processes.iter().collect()
		} else {
			let f = self.process_filter.to_lowercase();
			self.processes
				.iter()
				.filter(|p| p.format_name().to_lowercase().contains(&f))
				.collect()
		}
	}

	pub fn process_move(&mut self, delta: i32) {
		let len = self.visible_processes().len() as i32;
		if len == 0 {
			self.process_list_state = 0;
			return;
		}
		let idx = (self.process_list_state as i32 + delta).clamp(0, len - 1);
		self.process_list_state = idx as usize;
	}

	pub fn refresh_processes(&mut self) {
		self.processes = Process::get_valid_windows_processes();
		self.process_list_state = 0;
	}

	pub fn attach_selected(&mut self) {
		let chosen = self.visible_processes().get(self.process_list_state).map(|p| (*p).clone());
		if let Some(mut p) = chosen {
			let clone = p.clone();
			let opts = MemoryScannerOptions::default();
			let mut scanner = match MemoryScanner::new(clone, opts) {
				Ok(s) => s,
				Err(e) => {
					self.status = format!("Failed to attach: {}", e);
					return;
				}
			};

			if let Err(e) = p.init_handle() {
				self.status = format!("Failed to attach: {:?}", e);
				return;
			}
			let (tx_to_worker, rx_worker) = mpsc::channel::<WorkerCommand>();
			let (tx_from_worker, rx_self) = mpsc::channel::<WorkerEvent>();

			(self.worker_tx, self.worker_rx) = (Some(tx_to_worker), Some(rx_self));
			thread::spawn(move || memory_worker_loop(p, rx_worker, tx_from_worker));

			self.update_scanner_options(&mut scanner);
			self.scanner = Some(scanner);
			self.status = format!("Attached to {}", self.scanner.as_ref().unwrap().process.format_name());
			self.screen = Screen::Scanner;
			self.focus = Focus::ValueType;
			self.saved.clear();
			self.results_selected = 0;
			self.saved_selected = 0;
			self.has_first_scan = false;
			self.scan_count = 0;
		}
	}

	pub fn detach(&mut self) {
		self.screen = Screen::ProcessSelect;
		self.scanner = None;
		self.status = "Detached. Select a process and press Enter to attach".into();
		if let Some(r) = &self.worker_tx {
			r.send(WorkerCommand::Stop).unwrap();
		}
		self.worker_rx = None;
		self.worker_tx = None;
	}

	/* Focus / Navigation */

	pub fn tab_order(&self) -> Vec<Focus> {
		let mut order = vec![];

		if !self.has_first_scan {
			order.push(Focus::ValueType);
		}

		order.push(Focus::ScanMode);
		if self.scan_mode.needs_value() {
			order.push(Focus::ValueInput);
			if self.scan_mode.needs_second_value() {
				order.push(Focus::ValueInput2);
			}
		}

		if !self.has_first_scan {
			order.push(Focus::Alignment);
			order.push(Focus::RangeStart);
			order.push(Focus::RangeEnd);
		}

		if self.value_type.is_float() {
			order.push(Focus::RoundingMode);
		}

		if !self.has_first_scan {
			order.push(Focus::Writable);
			order.push(Focus::Executable);
			order.push(Focus::CopyOnWrite);
		}

		order.push(Focus::FreezeProcess);
		order.push(Focus::ScanBtn);
		if self.has_first_scan {
			order.push(Focus::NextScanBtn);
		}
		order.push(Focus::ResultsList);
		order.push(Focus::SavedList);
		order
	}

	pub fn next_focus(&mut self) {
		let order = self.tab_order();
		if order.is_empty() { return; }
		let idx = order.iter().position(|f| *f == self.focus).unwrap_or(0);
		self.focus = order[(idx + 1) % order.len()];
	}

	pub fn prev_focus(&mut self) {
		let order = self.tab_order();
		if order.is_empty() { return; }
		let idx = order.iter().position(|f| *f == self.focus).unwrap_or(0);
		self.focus = order[(idx + order.len() - 1) % order.len()];
	}

	pub fn cycle_focused_combo(&mut self, dir: i32) {
		match self.focus {
			Focus::ValueType => {
				self.value_type = self.value_type.cycle(dir);
				self.alignment = self.value_type.natural_alignment().to_string()
			}
			Focus::ScanMode => self.scan_mode = self.scan_mode.cycle(dir),
			Focus::RoundingMode => self.rounding_mode = self.rounding_mode.cycle(dir),
			_ => {}
		}
	}

	pub fn cycle_focused_tristate(&mut self, reverse: bool) {
		match self.focus {
			Focus::Writable => self.writable = self.writable.cycle(reverse),
			Focus::Executable => self.executable = self.executable.cycle(reverse),
			Focus::CopyOnWrite => self.cow = self.cow.cycle(reverse),
			_ => {}
		}
	}

	/* Text Editing */

	pub fn start_edit(&mut self, target: FieldTarget) {
		let current = match target {
			FieldTarget::Value => self.value_input.clone(),
			FieldTarget::Value2 => self.value_input2.clone(),
			FieldTarget::Alignment => self.alignment.clone(),
			FieldTarget::RangeStart => self.range_start.clone(),
			FieldTarget::RangeEnd => self.range_end.clone(),
			FieldTarget::ProcessFilter => self.process_filter.clone(),
			FieldTarget::SavedValue => self.saved.get(self.saved_selected).map(|e| e.value.clone()).unwrap_or_default(),
			FieldTarget::SavedDescription => self.saved.get(self.saved_selected).map(|e| e.description.clone()).unwrap_or_default(),
		};
		self.edit_buffer = current;
		self.editing_target = Some(target);
		self.input_mode = InputMode::Editing;
	}

	pub fn commit_edit(&mut self) {
		if let Some(target) = self.editing_target.take() {
			match target {
				FieldTarget::Value => self.value_input = self.edit_buffer.clone(),
				FieldTarget::Value2 => self.value_input2 = self.edit_buffer.clone(),
				FieldTarget::Alignment => self.alignment = self.edit_buffer.clone(),
				FieldTarget::RangeStart => self.range_start = self.edit_buffer.clone(),
				FieldTarget::RangeEnd => self.range_end = self.edit_buffer.clone(),
				FieldTarget::ProcessFilter => {
					self.process_filter = self.edit_buffer.clone();
					self.process_list_state = 0;
				}
				FieldTarget::SavedValue => {
					if self.saved_selected < self.saved.len() {
						let value_type = self.saved[self.saved_selected].value_type;
						let address = self.saved[self.saved_selected].address;
						let locked = self.saved[self.saved_selected].locked;

						match parse_value_bytes(value_type, &self.edit_buffer) {
							Ok(data) => {
								self.saved[self.saved_selected].value = format_bytes(&data, value_type);
								if let Some(tx) = &self.worker_tx {
									if locked {
										let _ = tx.send(WorkerCommand::AddLock { id: address, address, data });
									} else {
										let _ = tx.send(WorkerCommand::Write { address, data });
									}
								}
								self.status = "Value written".into();
							}
							Err(err) => {
								self.status = err;
							}
						}
					}
				}
				FieldTarget::SavedDescription => {
					if let Some(e) = self.saved.get_mut(self.saved_selected) {
						e.description = self.edit_buffer.clone();
					}
				}
			}
		}
		self.edit_buffer.clear();
		self.input_mode = InputMode::Normal;
	}

	pub fn cancel_edit(&mut self) {
		self.editing_target = None;
		self.edit_buffer.clear();
		self.input_mode = InputMode::Normal;
	}

	/* Results / Saved List */

	pub fn results_move(&mut self, delta: i32) {
		let total = self.scanner.as_ref().map(|s| s.state.total_matches()).unwrap_or(0);
		if total == 0 { return; }
		let len = total as i32;
		let idx = (self.results_selected as i32 + delta).clamp(0, len - 1);
		self.results_selected = idx as usize;
	}

	pub fn add_selected_to_saved(&mut self) {
		if let Some(scanner) = &self.scanner {
			if let Some((addr, bytes, _)) = scanner.state.get_match(self.results_selected) {
				let value = format_bytes(bytes, self.value_type);
				let n = self.saved.len() + 1;
				self.saved.push(SavedEntry {
					description: format!("Entry {n}"),
					address: addr,
					value_type: self.value_type,
					value,
					locked: false,
				});
				self.status = "Added selected match to saved list".into();
			}
		}
	}

	pub fn saved_move(&mut self, delta: i32) {
		if self.saved.is_empty() { return; }
		let len = self.saved.len() as i32;
		let idx = (self.saved_selected as i32 + delta).clamp(0, len - 1);
		self.saved_selected = idx as usize;
	}

	pub fn toggle_lock_selected_saved(&mut self) {
		if self.saved_selected >= self.saved.len() { return; }

		let now_locked = !self.saved[self.saved_selected].locked;
		let address = self.saved[self.saved_selected].address;
		let value_type = self.saved[self.saved_selected].value_type;
		let value_str = self.saved[self.saved_selected].value.clone();

		if now_locked {
			match parse_value_bytes(value_type, &value_str) {
				Ok(data) => {
					self.saved[self.saved_selected].locked = true;
					if let Some(tx) = &self.worker_tx {
						let _ = tx.send(WorkerCommand::AddLock { id: address, address, data });
					}
					self.status = "Locked - value will be frozen".into();
				}
				Err(err) => {
					self.status = format!("Cannot lock: {}", err);
				}
			}
		} else {
			self.saved[self.saved_selected].locked = false;
			if let Some(tx) = &self.worker_tx {
				let _ = tx.send(WorkerCommand::RemoveLock { id: address });
			}
			self.status = "Unlocked".into();
		}
	}

	pub fn remove_selected_saved(&mut self) {
		if self.saved_selected < self.saved.len() {
			let removed = self.saved.remove(self.saved_selected);
			if removed.locked {
				if let Some(tx) = &self.worker_tx {
					let _ = tx.send(WorkerCommand::RemoveLock { id: removed.address });
				}
			}
			if self.saved_selected > 0 && self.saved_selected >= self.saved.len() {
				self.saved_selected -= 1;
			}
			self.status = "Removed from saved list".into();
		}
	}

	// Scanning Logic

	pub fn update_scanner_options(&self, scanner: &mut MemoryScanner) {
		scanner.options.dump_options.writable = self.writable;
		scanner.options.dump_options.executable = self.executable;
		scanner.options.dump_options.copy_on_write = self.cow;

		if let Ok(start) = usize::from_str_radix(&self.range_start, 16) {
			scanner.options.dump_options.start_address = start;
		}
		if let Ok(end) = usize::from_str_radix(&self.range_end, 16) {
			scanner.options.dump_options.end_address = end;
		}

		if !self.alignment.is_empty() {
			if let Ok(align) = self.alignment.parse::<usize>() {
				scanner.options.scan_options.use_custom_alignment = true;
				scanner.options.scan_options.custom_alignment = align;
			}
		} else {
			scanner.options.scan_options.use_custom_alignment = false;
		}
	}

	fn build_scan_request(&self) -> Result<ScanRequest, String> {
		let parse_int = |s: &str| s.parse::<i128>().map_err(|_| format!("Invalid integer: {}", s));
		let parse_float = |s: &str| s.parse::<f64>().map_err(|_| format!("Invalid float: {}", s));

		match self.value_type {
			UiValueType::Float | UiValueType::Double => {
				let first = parse_float(&self.value_input)?;
				let cond = match self.scan_mode {
					ScanMode::ExactValue => match self.rounding_mode {
						RoundingMode::Default => FloatCondition::Range(first - 0.5, (first + 0.5).next_down()),
						RoundingMode::Extreme => FloatCondition::Range(first - 1.0, (first + 1.0).next_down()),
						RoundingMode::Truncate if first >= 0.0 => FloatCondition::Range(first, (first + 1.0).next_down()),
						RoundingMode::Truncate => FloatCondition::Range((first - 1.0).next_up(), first)
					},
					ScanMode::BiggerThan => FloatCondition::GreaterThan(first),
					ScanMode::SmallerThan => FloatCondition::LessThan(first),
					ScanMode::ValueBetween => FloatCondition::Range(first, parse_float(&self.value_input2)?),
				};
				let value_type = match self.value_type {
					UiValueType::Double => ValueType::Double(cond),
					UiValueType::Float => ValueType::Float(cond),
					_ => unreachable!()
				};
				Ok(ScanRequest { value_type, compare_to_first_scan: self.has_first_scan, freeze_process: self.freeze_process })
			}
			UiValueType::ArrayOfByte => {
				let b_arr = string_to_bytes_hex(&self.value_input).map_err(|_| "Invalid Byte Array".to_string())?;
				Ok(ScanRequest { value_type: ValueType::ByteArray(b_arr), compare_to_first_scan: self.has_first_scan, freeze_process: self.freeze_process })
			}
			_ => {
				let cond = match self.scan_mode {
					ScanMode::ExactValue => IntCondition::Eq(parse_int(&self.value_input)?),
					ScanMode::BiggerThan => IntCondition::GreaterThan(parse_int(&self.value_input)?),
					ScanMode::SmallerThan => IntCondition::LessThan(parse_int(&self.value_input)?),
					ScanMode::ValueBetween => IntCondition::Range(parse_int(&self.value_input)?, parse_int(&self.value_input2)?),
				};
				let value_type = match self.value_type {
					UiValueType::Byte => ValueType::Byte(cond),
					UiValueType::Word => ValueType::TwoBytes(cond),
					UiValueType::Dword => ValueType::FourBytes(cond),
					UiValueType::Qword => ValueType::EightBytes(cond),
					_ => unreachable!()
				};
				Ok(ScanRequest { value_type, compare_to_first_scan: self.has_first_scan, freeze_process: self.freeze_process })
			}
		}
	}

	pub fn trigger_scan(&mut self) {
		if self.is_scanning {
			self.status = "Scan already in progress...".into();
			return;
		}

		if self.has_first_scan {
			self.reset_scan_state();
		} else {
			self.start_scan(false);
		}
	}

	pub fn trigger_next_scan(&mut self) {
		if !self.has_first_scan {
			self.status = "No first scan to narrow yet".into();
			return;
		}
		self.start_scan(true);
	}

	fn reset_scan_state(&mut self) {
		if let Some(scanner) = self.scanner.as_mut() {
			scanner.state = MatchingState::new(1, 1, true);
		}
		self.has_first_scan = false;
		self.scan_count = 0;
		self.results_selected = 0;
		self.focus = Focus::ScanBtn;
		self.status = "Scan reset. Enter a value and press First Scan".into();
		self.scan_just_finished = true;
	}

	fn start_scan(&mut self, narrow: bool) {
		if self.is_scanning {
			self.status = "Scan already in progress...".into();
			return;
		}

		let req = match self.build_scan_request() {
			Ok(req) => req,
			Err(e) => { self.status = e; return; }
		};

		if let Some(mut scanner) = self.scanner.take() {
			self.update_scanner_options(&mut scanner);
			self.is_scanning = true;
			self.status = if narrow { "Narrowing scan...".into() } else { "Scanning...".into() };

			let (tx, rx) = mpsc::channel();
			self.scan_rx = Some(rx);

			thread::spawn(move || {
				let started = Instant::now();
				let res = if narrow { scanner.narrow(req) } else { scanner.scan(req) };
				let elapsed = started.elapsed();
				let result = res.map_err(|e| format!("{:?}", e));
				let _ = tx.send(ScanEvent::Finished { scanner, result, was_narrow: narrow, elapsed });
			});
		}
	}

	pub fn poll_scan_events(&mut self) {
		if let Some(rx) = &self.scan_rx {
			if let Ok(event) = rx.try_recv() {
				match event {
					ScanEvent::Finished { scanner, result, was_narrow, elapsed } => {
						self.is_scanning = false;
						if self.screen == Screen::Scanner {
							self.scanner = Some(scanner);
							match result {
								Ok(()) => {
									self.has_first_scan = true;
									self.scan_count = if was_narrow { self.scan_count + 1 } else { 1 };
									self.results_selected = 0;
									let total = self.scanner.as_ref().map(|s| s.state.total_matches()).unwrap_or(0);
									self.status = format!("Found {} matches in {:.2?}.", total, elapsed);
									self.focus = Focus::ResultsList;
									self.scan_just_finished = true;
								}
								Err(e) => self.status = format!("Scan error: {}", e),
							}
						} else {
							self.status = "Scan finished, but process was detached.".into();
						}
					}
				}
				self.scan_rx = None;
			}
		}
	}

	/* Worker stuff */

	pub fn poll_worker_events(&mut self) {
		if let Some(rx) = &self.worker_rx {
			while let Ok(event) = rx.try_recv() {
				match event {
					WorkerEvent::WriteResult { id, success } => {
						if !success {
							self.status = match id {
								Some(addr) => format!("Failed to write value at 0x{:X}", addr),
								None => "Failed to write value".into(),
							};
						}
					}
					WorkerEvent::ReadResult { values } => {
						for (i, e) in self.saved.iter_mut().enumerate() {
							if self.editing_target == Some(FieldTarget::SavedValue) && self.saved_selected == i {
								continue;
							}
							if let Some(bytes) = values.get(&e.address) {
								e.value = format_bytes(bytes, e.value_type);
							}
						}

						for entry in &mut self.visible_matches {
							if let Some(bytes) = values.get(&entry.address) {
								entry.value = format_bytes(bytes, self.value_type);
							}
						}
					}
					WorkerEvent::LockFailed { id } => {
						for e in self.saved.iter_mut() {
							if e.address == id {
								e.locked = false;
							}
						}
						self.status = format!("Lock on 0x{:X} failed - address may be invalid", id);
					}
				}
			}
		}
	}

	pub fn refresh_saved_values(&mut self) {
		if self.saved.is_empty() { return; }
		if let Some(tx) = &self.worker_tx {
			let addresses: Vec<(usize, usize)> = self.saved.iter().map(|e| (e.address, saved_entry_size(e))).collect();
			let _ = tx.send(WorkerCommand::ReadBatch { addresses });
		}
	}

	pub fn update_visible_matches(&mut self, visible_rows: usize) {
		let total_matches = self.scanner.as_ref().map(|s| s.state.total_matches()).unwrap_or(0);

		let mut start_idx = 0;
		if total_matches > visible_rows {
			if self.results_selected >= visible_rows / 2 {
				start_idx = self.results_selected - visible_rows / 2;
				if start_idx + visible_rows > total_matches {
					start_idx = total_matches.saturating_sub(visible_rows);
				}
			}
		}
		let end_idx = (start_idx + visible_rows).min(total_matches);

		let needs_rebuild = self.visible_start_idx != start_idx
			|| self.visible_end_idx != end_idx
			|| self.scan_just_finished;

		if needs_rebuild {
			self.visible_start_idx = start_idx;
			self.visible_end_idx = end_idx;
			self.scan_just_finished = false;

			self.visible_matches.clear();
			if let Some(scanner) = &self.scanner {
				for i in start_idx..end_idx {
					if let Some((addr, bytes, initial)) = scanner.state.get_match(i) {
						self.visible_matches.push(MatchEntry {
							original_index: i,
							address: addr,
							size: bytes.len(),
							value: format_bytes(bytes, self.value_type),
							previous: format_bytes(initial, self.value_type),
						});
					}
				}
			}

			self.refresh_visible_matches();
		}
	}

	pub fn refresh_visible_matches(&mut self) {
		if self.visible_matches.is_empty() { return; }
		if let Some(tx) = &self.worker_tx {
			let addresses: Vec<(usize, usize)> = self.visible_matches.iter().map(|e| (e.address, e.size)).collect();
			let _ = tx.send(WorkerCommand::ReadBatch { addresses });
		}
	}

	pub fn tick(&mut self) {
		if self.screen == Screen::Scanner && self.scanner.is_some() {
			if self.last_value_refresh.elapsed() >= Duration::from_millis(500) {
				self.refresh_visible_matches();
				self.refresh_saved_values();
				self.last_value_refresh = Instant::now();
			}
		}
	}
}

pub fn format_bytes(bytes: &[u8], value_type: UiValueType) -> String {
	match value_type {
		UiValueType::Byte if bytes.len() >= 1 => format!("{}", u8::from_le_bytes([bytes[0]])),
		UiValueType::Word if bytes.len() >= 2 => format!("{}", u16::from_le_bytes(bytes[..2].try_into().unwrap())),
		UiValueType::Dword if bytes.len() >= 4 => format!("{}", u32::from_le_bytes(bytes[..4].try_into().unwrap())),
		UiValueType::Qword if bytes.len() >= 8 => format!("{}", u64::from_le_bytes(bytes[..8].try_into().unwrap())),
		UiValueType::Float if bytes.len() >= 4 => format!("{}", f32::from_le_bytes(bytes[..4].try_into().unwrap())),
		UiValueType::Double if bytes.len() >= 8 => format!("{}", f64::from_le_bytes(bytes[..8].try_into().unwrap())),
		UiValueType::ArrayOfByte => bytes.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "),
		_ => "?".to_string(),
	}
}

pub fn parse_value_bytes(value_type: UiValueType, s: &str) -> Result<Vec<u8>, String> {
	let s = s.trim();
	match value_type {
		UiValueType::Byte => s.parse::<i128>().map(|n| vec![n as u8]).map_err(|_| format!("Invalid byte value: {}", s)),
		UiValueType::Word => s.parse::<i128>().map(|n| (n as u16).to_le_bytes().to_vec()).map_err(|_| format!("Invalid value: {}", s)),
		UiValueType::Dword => s.parse::<i128>().map(|n| (n as u32).to_le_bytes().to_vec()).map_err(|_| format!("Invalid value: {}", s)),
		UiValueType::Qword => s.parse::<i128>().map(|n| (n as u64).to_le_bytes().to_vec()).map_err(|_| format!("Invalid value: {}", s)),
		UiValueType::Float => s.parse::<f32>().map(|n| n.to_le_bytes().to_vec()).map_err(|_| format!("Invalid float value: {}", s)),
		UiValueType::Double => s.parse::<f64>().map(|n| n.to_le_bytes().to_vec()).map_err(|_| format!("Invalid double value: {}", s)),
		UiValueType::ArrayOfByte => string_to_bytes_hex(s).map_err(|_| "Invalid byte array".to_string()),
	}
}

fn saved_entry_size(entry: &SavedEntry) -> usize {
	match entry.value_type {
		UiValueType::Byte => 1,
		UiValueType::Word => 2,
		UiValueType::Dword => 4,
		UiValueType::Qword => 8,
		UiValueType::Float => 4,
		UiValueType::Double => 8,
		UiValueType::ArrayOfByte => entry.value.split_whitespace().count().max(1),
	}
}

/* Drawing */

pub fn draw(f: &mut Frame, app: &mut App) {
	app.tick();
	match app.screen {
		Screen::ProcessSelect => draw_process_select(f, app),
		Screen::Scanner => draw_scanner(f, app),
	}
}

fn box_style(app: &App, focus: Focus) -> Style {
	let is_locked = app.has_first_scan && matches!(focus,
		Focus::ValueType | Focus::Alignment | Focus::RangeStart |
		Focus::RangeEnd | Focus::Writable | Focus::Executable |
		Focus::CopyOnWrite
	);
	if is_locked {
		return Style::default().fg(DIM);
	}

	if app.focus == focus {
		if app.input_mode == InputMode::Editing {
			Style::default().fg(EDIT).add_modifier(Modifier::BOLD)
		} else {
			Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
		}
	} else {
		Style::default().fg(Color::Gray)
	}
}

fn field_text(app: &App, target: FieldTarget, stored: &str) -> String {
	if app.editing_target == Some(target) {
		format!("{}\u{2588}", app.edit_buffer) // blocks cursor
	} else {
		stored.to_string()
	}
}

fn draw_process_select(f: &mut Frame, app: &App) {
	let area = f.area();
	let chunks = Layout::default()
		.direction(Direction::Vertical)
		.constraints([
			Constraint::Length(3),
			Constraint::Length(3),
			Constraint::Min(5),
			Constraint::Length(3),
		])
		.split(area);

	let title = Paragraph::new("  Process List  ")
		.style(Style::default().fg(Color::White).bg(HEADER_BG).add_modifier(Modifier::BOLD))
		.block(Block::default().borders(Borders::ALL).title(" MemScan "));
	f.render_widget(title, chunks[0]);

	let filter_text = field_text(app, FieldTarget::ProcessFilter, &app.process_filter);
	let filter_style = if app.editing_target == Some(FieldTarget::ProcessFilter) {
		Style::default().fg(EDIT)
	} else {
		Style::default().fg(Color::White)
	};
	let filter = Paragraph::new(Line::from(vec![
		Span::styled("Filter: ", Style::default().fg(DIM)),
		Span::styled(filter_text, filter_style),
	]))
		.block(Block::default().borders(Borders::ALL).title(" Search ('/' to edit) "));
	f.render_widget(filter, chunks[1]);

	let procs = app.visible_processes();
	let rows: Vec<Row> = procs
		.iter()
		.enumerate()
		.map(|(i, p)| {
			let style = if i == app.process_list_state {
				Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)
			} else {
				Style::default().fg(Color::White)
			};
			Row::new(vec![
				Cell::from(p.get_pid().to_string()),
				Cell::from(p.format_name()),
			]).style(style)
		}).collect();

	let table = Table::new(
		rows,
		[Constraint::Length(10), Constraint::Length(40)],
	)
		.header(
			Row::new(vec!["PID", "Process Name"]).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
		)
		.block(Block::default().borders(Borders::ALL).title(format!(" Processes ({}) ", procs.len())));
	f.render_widget(table, chunks[2]);

	let help = Paragraph::new("↑/↓ or j/k: move   Enter: attach   /: filter   r: refresh   q: quit")
		.style(Style::default().fg(DIM))
		.block(Block::default().borders(Borders::ALL).title(" Help "));
	f.render_widget(help, chunks[3]);
}

fn draw_results(f: &mut Frame, app: &mut App, area: Rect) {
	let border_style = box_style(app, Focus::ResultsList);
	let header = Row::new(vec!["Address", "Value", "Previous"])
		.style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

	let total_matches = app.scanner.as_ref().map(|s| s.state.total_matches()).unwrap_or(0);
	let visible_rows = area.height.saturating_sub(3) as usize;

	app.update_visible_matches(visible_rows);

	let mut rows: Vec<Row> = Vec::with_capacity(app.visible_matches.len());
	for entry in &app.visible_matches {
		let is_selected = entry.original_index == app.results_selected && app.focus == Focus::ResultsList;
		let style = if is_selected {
			Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)
		} else {
			Style::default().fg(Color::White)
		};

		rows.push(Row::new(vec![
			format!("0x{:X}", entry.address),
			entry.value.clone(),
			entry.previous.clone()
		]).style(style));
	}

	let title = format!(
		" Found Addresses ({}){} ",
		total_matches,
		if app.has_first_scan { format!(" - scan #{}", app.scan_count) } else { String::new() }
	);

	let table = Table::new(rows, [Constraint::Length(18), Constraint::Length(18), Constraint::Length(18)])
		.header(header)
		.block(Block::default().borders(Borders::ALL).title(title).border_style(border_style));
	f.render_widget(table, area);
}

fn draw_scanner(f: &mut Frame, app: &mut App) {
	let area = f.area();
	let outer = Layout::default()
		.direction(Direction::Vertical)
		.constraints([Constraint::Length(3), Constraint::Min(10), Constraint::Length(3)])
		.split(area);

	draw_top_bar(f, app, outer[0]);

	let body = Layout::default()
		.direction(Direction::Horizontal)
		.constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
		.split(outer[1]);

	draw_scan_config(f, app, body[0]);

	let right = Layout::default()
		.direction(Direction::Vertical)
		.constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
		.split(body[1]);

	draw_results(f, app, right[0]);
	draw_saved(f, app, right[1]);

	draw_status_bar(f, app, outer[2]);
}

fn draw_top_bar(f: &mut Frame, app: &App, area: Rect) {
	let name = app
		.scanner
		.as_ref()
		.map(|s| s.process.format_name())
		.unwrap_or_else(|| "<none>".to_string());
	let text = Line::from(vec![
		Span::styled(" Attached: ", Style::default().fg(DIM)),
		Span::styled(name, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
		Span::raw("    "),
		Span::styled("Esc", Style::default().fg(ACCENT)),
		Span::styled(": detach   ", Style::default().fg(DIM)),
		Span::styled("Tab/Shift+Tab", Style::default().fg(ACCENT)),
		Span::styled(": move focus   ", Style::default().fg(DIM)),
		Span::styled("q", Style::default().fg(ACCENT)),
		Span::styled(": quit", Style::default().fg(DIM)),
	]);
	let bar = Paragraph::new(text)
		.style(Style::default().bg(HEADER_BG))
		.block(Block::default().borders(Borders::ALL).title(" CE-TUI Scanner "));
	f.render_widget(bar, area);
}

fn draw_scan_config(f: &mut Frame, app: &App, area: Rect) {
	let block = Block::default().borders(Borders::ALL).title(" Scan Settings ");
	let inner = block.inner(area);
	f.render_widget(block, area);

	let needs_v2 = app.scan_mode.needs_second_value();
	let mut constraints = vec![
		Constraint::Length(3),                                 // Value type
	];
	if app.value_type.is_numeric() {
		constraints.push(Constraint::Length(3));
	}
	if app.scan_mode.needs_value() {
		constraints.push(Constraint::Length(3));        // Value
		if needs_v2 {
			constraints.push(Constraint::Length(3));    // Value2
		}
	}
	constraints.push(Constraint::Length(3));            // Alignment
	constraints.push(Constraint::Length(3));            // Range
	if app.value_type.is_float() {
		constraints.push(Constraint::Length(3));        // Float rounding mode
	}
	constraints.push(Constraint::Length(3));            // Protection flags
	constraints.push(Constraint::Length(3));            // Freeze process
	constraints.push(Constraint::Length(3));            // Scan buttons
	constraints.push(Constraint::Min(1));               // Filler

	let rows = Layout::default().direction(Direction::Vertical).constraints(constraints).split(inner);
	let mut idx = 0;

	render_combo(f, rows[idx], "Value Type", app.value_type.label(), box_style(app, Focus::ValueType));
	idx += 1;

	if app.value_type.is_numeric() {
		render_combo(f, rows[idx], "Scan Type", app.scan_mode.label(), box_style(app, Focus::ScanMode));
		idx += 1;
	}

	if app.scan_mode.needs_value() {
		let label = if needs_v2 { "Value (min)" } else { "Value" };
		render_text_field(f, rows[idx], label, &field_text(app, FieldTarget::Value, &app.value_input), box_style(app, Focus::ValueInput));
		idx += 1;
		if needs_v2 {
			render_text_field(f, rows[idx], "Value (max)", &field_text(app, FieldTarget::Value2, &app.value_input2), box_style(app, Focus::ValueInput2));
			idx += 1;
		}
	}

	render_text_field(f, rows[idx], "Alignment", &field_text(app, FieldTarget::Alignment, &app.alignment), box_style(app, Focus::Alignment));
	idx += 1;

	let range_cols = Layout::default()
		.direction(Direction::Horizontal)
		.constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
		.split(rows[idx]);
	render_text_field(f, range_cols[0], "Range Start", &field_text(app, FieldTarget::RangeStart, &app.range_start), box_style(app, Focus::RangeStart));
	render_text_field(f, range_cols[1], "Range End", &field_text(app, FieldTarget::RangeEnd, &app.range_end), box_style(app, Focus::RangeEnd));
	idx += 1;

	if app.value_type.is_float() {
		render_combo(f, rows[idx], "Float Rounding Mode", app.rounding_mode.label(), box_style(app, Focus::RoundingMode));
		idx += 1;
	}
	let flag_cols = Layout::default()
		.direction(Direction::Horizontal)
		.constraints([Constraint::Percentage(34), Constraint::Percentage(33), Constraint::Percentage(33)])
		.split(rows[idx]);
	render_tristate(f, flag_cols[0], "Writable", app.writable, box_style(app, Focus::Writable));
	render_tristate(f, flag_cols[1], "Executable", app.executable, box_style(app, Focus::Executable));
	render_tristate(f, flag_cols[2], "Copy-on-Write", app.cow, box_style(app, Focus::CopyOnWrite));
	idx += 1;

	render_tristate(f, rows[idx], "Freeze Process", app.freeze_process.into(), box_style(app, Focus::FreezeProcess));
	idx += 1;

	let btn_cols = Layout::default()
		.direction(Direction::Horizontal)
		.constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
		.split(rows[idx]);

	let scan_label = if app.has_first_scan { "New Scan" } else { "First Scan" };
	render_button(f, btn_cols[0], scan_label, box_style(app, Focus::ScanBtn));

	let next_style = if app.has_first_scan {
		box_style(app, Focus::NextScanBtn)
	} else {
		Style::default().fg(DIM)
	};
	render_button(f, btn_cols[1], "Next Scan", next_style);
}

fn render_combo(f: &mut Frame, area: Rect, title: &str, value: &str, style: Style) {
	let text = Line::from(vec![
		Span::raw("\u{25c0} "),
		Span::styled(value, Style::default().add_modifier(Modifier::BOLD)),
		Span::raw(" \u{25b6}"),
	]);
	let p = Paragraph::new(text)
		.alignment(Alignment::Center)
		.block(Block::default().borders(Borders::ALL).title(format!(" {title} ")).border_style(style));
	f.render_widget(p, area);
}

fn render_text_field(f: &mut Frame, area: Rect, title: &str, value: &str, style: Style) {
	let p = Paragraph::new(value.to_string())
		.block(Block::default().borders(Borders::ALL).title(format!(" {title} ")).border_style(style));
	f.render_widget(p, area);
}

fn render_tristate(f: &mut Frame, area: Rect, title: &str, value: TriState, style: Style) {
	let color = match value {
		TriState::DontCare => Color::Gray,
		TriState::True => Color::Green,
		TriState::False => Color::Red,
	};
	let p = Paragraph::new(value.label())
		.alignment(Alignment::Center)
		.style(Style::default().fg(color).add_modifier(Modifier::BOLD))
		.block(Block::default().borders(Borders::ALL).title(format!(" {title} ")).border_style(style));
	f.render_widget(p, area);
}

fn render_button(f: &mut Frame, area: Rect, label: &str, style: Style) {
	let p = Paragraph::new(label)
		.alignment(Alignment::Center)
		.style(style.add_modifier(Modifier::BOLD))
		.block(Block::default().borders(Borders::ALL).border_style(style));
	f.render_widget(p, area);
}

fn draw_saved(f: &mut Frame, app: &App, area: Rect) {
	let border_style = box_style(app, Focus::SavedList);

	if app.saved.is_empty() {
		let p = Paragraph::new("(empty - select a result and press 'a' to add it here)")
			.style(Style::default().fg(DIM))
			.block(
				Block::default()
					.borders(Borders::ALL)
					.title(" Saved Addresses (Cheat Table) ")
					.border_style(border_style),
			);
		f.render_widget(p, area);
		return;
	}

	let items: Vec<ListItem> = app
		.saved
		.iter()
		.enumerate()
		.map(|(i, e)| {
			let selected = i == app.saved_selected && app.focus == Focus::SavedList;
			let lock_editing = app.editing_target == Some(FieldTarget::SavedDescription) && selected;
			let desc = if lock_editing { field_text(app, FieldTarget::SavedDescription, &e.description) } else { e.description.clone() };

			let value_editing = app.editing_target == Some(FieldTarget::SavedValue) && selected;
			let value = if value_editing { field_text(app, FieldTarget::SavedValue, &e.value) } else { e.value.clone() };

			let lock_span = if e.locked {
				Span::styled("[LOCKED]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
			} else {
				Span::styled("[ ]", Style::default().fg(DIM))
			};

			let line = Line::from(vec![
				lock_span,
				Span::raw(" "),
				Span::styled(format!("{desc:<18}"), Style::default().add_modifier(Modifier::BOLD)),
				Span::styled(format!(" 0x{:<12X}", e.address), Style::default().fg(Color::Cyan)),
				Span::styled(format!(" {:<12} ", e.value_type.label()), Style::default().fg(Color::Magenta)),
				Span::styled(value, Style::default().fg(Color::Green)),
			]);

			let style = if selected {
				Style::default().bg(Color::Rgb(50, 50, 20))
			} else {
				Style::default()
			};
			ListItem::new(line).style(style)
		})
		.collect();

	let list = List::new(items).block(
		Block::default()
			.borders(Borders::ALL)
			.title(format!(" Saved Addresses ({}) ", app.saved.len()))
			.border_style(border_style),
	);
	f.render_widget(list, area);
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
	let help = match app.focus {
		Focus::ValueType | Focus::ScanMode => "\u{2190}/\u{2192}: change   Tab: next field",
		Focus::ValueInput | Focus::ValueInput2 | Focus::Alignment | Focus::RangeStart | Focus::RangeEnd | Focus::RoundingMode | Focus::FreezeProcess => {
			"Enter: edit   Tab: next field"
		}
		Focus::Writable | Focus::Executable | Focus::CopyOnWrite => "Enter/\u{2190}\u{2192}: toggle   Tab: next field",
		Focus::ScanBtn => "Enter: run scan",
		Focus::NextScanBtn => "Enter: run next scan",
		Focus::ResultsList => "\u{2191}/\u{2193}: select   a/Enter: add to saved list",
		Focus::SavedList => "\u{2191}/\u{2193}: select   space: lock   Enter: edit value   r: rename   d: delete",
	};
	let text = Line::from(vec![
		Span::styled(format!(" {} ", app.status), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
		Span::raw("   "),
		Span::styled(help, Style::default().fg(DIM)),
	]);
	let bar = Paragraph::new(text).block(Block::default().borders(Borders::ALL));
	f.render_widget(bar, area);
}