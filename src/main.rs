#![feature(portable_simd)]
#![feature(associated_type_defaults)]
extern crate core;

mod utils;
mod matches;
mod win_helpers;
mod process;
mod memory_scanner;
mod errors;
mod num_helpers;
mod mem2disk;
mod simd_types;
mod ui;
mod ui_types;
mod externs;

use std::io;
use std::io::ErrorKind;
use std::time::{Duration, Instant};

use crossterm::{
	event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use ui::{draw, App};
use ui_types::{FieldTarget, Focus, InputMode, Screen};
use crate::win_helpers::enable_debug_privilege;

fn main() -> io::Result<()> {
	enable_debug_privilege().expect("Process must be run as administrator");
	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
	let backend = CrosstermBackend::new(stdout);
	let mut terminal = Terminal::new(backend)?;

	let mut app = App::new();
	let result = run_app(&mut terminal, &mut app);

	disable_raw_mode()?;
	execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
	terminal.show_cursor()?;

	result
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
	let tick_rate = Duration::from_millis(250);
	let mut last_tick = Instant::now();

	loop {
		// 1. Poll background thread for completed scans
		app.poll_scan_events();

		// 2. Draw UI
		if terminal.draw(|f| draw(f, app)).is_err() {
			return Err(io::Error::new(ErrorKind::Other, "Terminal draw error"));
		}

		// 3. Handle input
		let timeout = tick_rate.saturating_sub(last_tick.elapsed());
		if event::poll(timeout)? {
			if let Event::Key(key) = event::read()? {
				if key.kind == KeyEventKind::Press {
					handle_key(app, key.code, key.modifiers);
				}
			}
		}

		if last_tick.elapsed() >= tick_rate {
			last_tick = Instant::now();
		}

		if app.should_quit {
			return Ok(());
		}
	}
}

fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
	if app.input_mode == InputMode::Editing {
		match code {
			KeyCode::Enter => app.commit_edit(),
			KeyCode::Esc => app.cancel_edit(),
			KeyCode::Backspace => {
				app.edit_buffer.pop();
			}
			KeyCode::Char(c) => app.edit_buffer.push(c),
			_ => {}
		}
		return;
	}

	match code {
		KeyCode::Char('q') => {
			app.should_quit = true;
			return;
		}
		KeyCode::Esc => {
			if app.screen == Screen::Scanner {
				app.detach();
			} else {
				app.should_quit = true;
			}
			return;
		}
		KeyCode::Tab | KeyCode::BackTab => {
			if modifiers.contains(KeyModifiers::SHIFT) || code == KeyCode::BackTab {
				app.prev_focus();
			} else {
				app.next_focus();
			}
			return;
		}
		_ => {}
	}

	match app.screen {
		Screen::ProcessSelect => handle_key_process_select(app, code),
		Screen::Scanner => handle_key_scanner(app, code),
	}
}

fn handle_key_process_select(app: &mut App, code: KeyCode) {
	match code {
		KeyCode::Up | KeyCode::Char('k') => app.process_move(-1),
		KeyCode::Down | KeyCode::Char('j') => app.process_move(1),
		KeyCode::Char('/') => app.start_edit(FieldTarget::ProcessFilter),
		KeyCode::Enter => app.attach_selected(),
		KeyCode::Char('r') => app.refresh_processes(),
		_ => {}
	}
}

fn handle_key_scanner(app: &mut App, code: KeyCode) {
	match app.focus {
		Focus::ValueType | Focus::ScanMode | Focus::RoundingMode => match code {
			KeyCode::Left | KeyCode::Char('h') => app.cycle_focused_combo(-1),
			KeyCode::Right | KeyCode::Char('l') => app.cycle_focused_combo(1),
			KeyCode::Enter => app.cycle_focused_combo(1),
			_ => {}
		},
		Focus::FreezeProcess => match code {
			KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('l') | KeyCode::Right => app.freeze_process = !app.freeze_process,
			_ => {}
		},
		Focus::ValueInput => {
			if code == KeyCode::Enter {
				app.start_edit(FieldTarget::Value);
			}
		}
		Focus::ValueInput2 => {
			if code == KeyCode::Enter {
				app.start_edit(FieldTarget::Value2);
			}
		}
		Focus::Alignment => {
			if code == KeyCode::Enter {
				app.start_edit(FieldTarget::Alignment);
			}
		}
		Focus::RangeStart => {
			if code == KeyCode::Enter {
				app.start_edit(FieldTarget::RangeStart);
			}
		}
		Focus::RangeEnd => {
			if code == KeyCode::Enter {
				app.start_edit(FieldTarget::RangeEnd);
			}
		}
		Focus::Writable | Focus::Executable | Focus::CopyOnWrite => match code {
			KeyCode::Enter | KeyCode::Char(' ') => app.cycle_focused_tristate(false),
			KeyCode::Left => app.cycle_focused_tristate(true),
			_ => {}
		},
		Focus::ScanBtn => {
			if code == KeyCode::Enter {
				app.trigger_scan();
			}
		}
		Focus::NextScanBtn => {
			if code == KeyCode::Enter {
				app.trigger_next_scan();
			}
		}
		Focus::ResultsList => match code {
			KeyCode::Up | KeyCode::Char('k') => app.results_move(-1),
			KeyCode::Down | KeyCode::Char('j') => app.results_move(1),
			KeyCode::Enter | KeyCode::Char('a') => app.add_selected_to_saved(),
			_ => {}
		},
		Focus::SavedList => match code {
			KeyCode::Up | KeyCode::Char('k') => app.saved_move(-1),
			KeyCode::Down | KeyCode::Char('j') => app.saved_move(1),
			KeyCode::Char(' ') => app.toggle_lock_selected_saved(),
			KeyCode::Enter => app.start_edit(FieldTarget::SavedValue),
			KeyCode::Char('r') => app.start_edit(FieldTarget::SavedDescription),
			KeyCode::Char('d') | KeyCode::Delete => app.remove_selected_saved(),
			_ => {}
		},
	}
}