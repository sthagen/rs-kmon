mod app;
mod event;
mod kernel;
mod style;
mod util;
use app::{App, Blocks, InputMode, ScrollDirection};
use enum_unitary::{Bounded, EnumUnitary};
use event::{Event, Events};
use kernel::cmd::ModuleCommand;
use kernel::info::KernelInfo;
use kernel::lkm::KernelModules;
use kernel::log::KernelLogs;
use std::io::stdout;
use termion::event::Key;
use termion::input::MouseTerminal;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::Terminal;
use unicode_width::UnicodeWidthStr;

const VERSION: &str = "0.1.0"; /* Version */
const REFRESH_RATE: &str = "250"; /* Default refresh rate of the terminal */

/**
 * Create a terminal instance with using termion as backend.
 *
 * @param  ArgMatches
 * @return Result
 */
fn create_term(args: &clap::ArgMatches) -> Result<(), failure::Error> {
	/* Configure the terminal. */
	let stdout = stdout().into_raw_mode()?;
	let stdout = MouseTerminal::from(stdout);
	let stdout = AlternateScreen::from(stdout);
	let backend = TermionBackend::new(stdout);
	let mut terminal = Terminal::new(backend)?;
	terminal.hide_cursor()?;
	/* Set required items for the terminal widgets. */
	let mut app = App::new(Blocks::ModuleTable);
	let mut kernel_logs = KernelLogs::default();
	let mut kernel_info = KernelInfo::new();
	let mut kernel_modules = KernelModules::new(args);
	/* Create terminal events. */
	let events = Events::new(
		args.value_of("rate")
			.unwrap_or(REFRESH_RATE)
			.parse::<u64>()
			.unwrap(),
		&kernel_logs,
	);
	/* Draw terminal and render the widgets. */
	loop {
		terminal.draw(|mut f| {
			let chunks = Layout::default()
				.direction(Direction::Vertical)
				.constraints(
					[Constraint::Percentage(75), Constraint::Percentage(25)]
						.as_ref(),
				)
				.split(f.size());
			{
				let chunks = Layout::default()
					.direction(Direction::Horizontal)
					.constraints(
						[Constraint::Percentage(60), Constraint::Percentage(40)]
							.as_ref(),
					)
					.split(chunks[0]);
				{
					let chunks = Layout::default()
						.direction(Direction::Vertical)
						.constraints(
							[Constraint::Length(3), Constraint::Percentage(100)]
								.as_ref(),
						)
						.split(chunks[0]);
					{
						let chunks = Layout::default()
							.direction(Direction::Horizontal)
							.constraints(
								[
									Constraint::Percentage(60),
									Constraint::Percentage(40),
								]
								.as_ref(),
							)
							.split(chunks[0]);
						app.draw_user_input(&mut f, chunks[0], &events.tx);
						app.draw_kernel_info(
							&mut f,
							chunks[1],
							&kernel_info.current_info,
						)
					}
					app.draw_kernel_modules(&mut f, chunks[1], &mut kernel_modules);
				}
				app.draw_module_info(&mut f, chunks[1], &mut kernel_modules);
			}
			app.draw_kernel_activities(&mut f, chunks[1], &mut kernel_logs);
		})?;
		/* Set cursor position if the input mode flag is set. */
		if !app.input_mode.is_none() {
			util::set_cursor_pos(
				terminal.backend_mut(),
				2 + app.input_query.width() as u16,
				2,
			)?;
		}
		/* Handle terminal events. */
		match events.rx.recv()? {
			/* Key input events. */
			Event::Input(input) => {
				if app.input_mode.is_none() {
					/* Default input mode. */
					match input {
						/* Quit. */
						Key::Char('q')
						| Key::Char('Q')
						| Key::Ctrl('c')
						| Key::Ctrl('d')
						| Key::Esc => {
							break;
						}
						/* Refresh. */
						Key::Char('r') | Key::Char('R') | Key::F(5) => {
							app = App::new(Blocks::ModuleTable);
							kernel_logs.index = 0;
							kernel_info = KernelInfo::new();
							kernel_modules = KernelModules::new(args);
						}
						/* Show help message. */
						Key::Char('?') | Key::F(1) => {
							app.selected_block = Blocks::ModuleInfo;
							kernel_modules.current_name = String::from("!Help");
							kernel_modules
								.current_info
								.set_raw_text(String::from("(TODO)\nHelp Message"));
						}
						/* Scroll the selected block up. */
						Key::Up | Key::Char('k') | Key::Char('K') => {
							match app.selected_block {
								Blocks::ModuleTable => {
									kernel_modules.scroll_list(ScrollDirection::Up)
								}
								Blocks::ModuleInfo => kernel_modules
									.scroll_mod_info(ScrollDirection::Up),
								Blocks::Activities => {
									kernel_logs.scroll(ScrollDirection::Up);
								}
								_ => {}
							}
						}
						/* Scroll the selected block down. */
						Key::Down | Key::Char('j') | Key::Char('J') => {
							match app.selected_block {
								Blocks::ModuleTable => {
									kernel_modules.scroll_list(ScrollDirection::Down)
								}
								Blocks::ModuleInfo => kernel_modules
									.scroll_mod_info(ScrollDirection::Down),
								Blocks::Activities => {
									kernel_logs.scroll(ScrollDirection::Down);
								}
								_ => {}
							}
						}
						/* Select the next terminal block. */
						Key::Left
						| Key::Char('h')
						| Key::Char('H')
						| Key::Ctrl('h') => {
							app.selected_block =
								match app.selected_block.prev_variant() {
									Some(v) => v,
									None => Blocks::max_value(),
								}
						}
						/* Select the previous terminal block. */
						Key::Right
						| Key::Char('l')
						| Key::Char('L')
						| Key::Ctrl('l') => {
							app.selected_block =
								match app.selected_block.next_variant() {
									Some(v) => v,
									None => Blocks::min_value(),
								}
						}
						/* Scroll to the top of the module list. */
						Key::Char('t') | Key::Char('T') | Key::Home => {
							app.selected_block = Blocks::ModuleTable;
							kernel_modules.scroll_list(ScrollDirection::Top)
						}
						/* Scroll to the bottom of the module list. */
						Key::Char('b') | Key::Char('B') | Key::End => {
							app.selected_block = Blocks::ModuleTable;
							kernel_modules.scroll_list(ScrollDirection::Bottom)
						}
						/* Scroll kernel activities up. */
						Key::PageUp => {
							app.selected_block = Blocks::Activities;
							kernel_logs.scroll(ScrollDirection::Up);
						}
						/* Scroll kernel activities down. */
						Key::PageDown => {
							app.selected_block = Blocks::Activities;
							kernel_logs.scroll(ScrollDirection::Down);
						}
						/* Scroll module information up. */
						Key::Char('<') | Key::Alt(' ') => {
							app.selected_block = Blocks::ModuleInfo;
							kernel_modules.scroll_mod_info(ScrollDirection::Up)
						}
						/* Scroll module information down. */
						Key::Char('>') | Key::Char(' ') => {
							app.selected_block = Blocks::ModuleInfo;
							kernel_modules.scroll_mod_info(ScrollDirection::Down)
						}
						/* Show the next kernel information. */
						Key::Char('\t') | Key::BackTab => {
							kernel_info.next();
						}
						/* Unload kernel module. */
						Key::Char('u')
						| Key::Char('U')
						| Key::Char('-')
						| Key::Backspace => {
							kernel_modules
								.set_current_command(ModuleCommand::Unload);
						}
						/* Blacklist kernel module. */
						Key::Char('x')
						| Key::Char('X')
						| Key::Ctrl('b')
						| Key::Delete => {
							kernel_modules
								.set_current_command(ModuleCommand::Blacklist);
						}
						/* Execute the current command. */
						Key::Char('y') | Key::Char('Y') => {
							if kernel_modules.exec_current_command() {
								events
									.tx
									.send(Event::Input(Key::Char('r')))
									.unwrap();
							}
						}
						/* Cancel the execution of current command. */
						Key::Char('n') | Key::Char('N') => {
							if !kernel_modules.command.is_none() {
								app.selected_block = Blocks::ModuleTable;
								kernel_modules.command = ModuleCommand::None;
								if kernel_modules.index != 0 {
									kernel_modules.index -= 1;
									kernel_modules
										.scroll_list(ScrollDirection::Down);
								} else {
									kernel_modules.index += 1;
									kernel_modules.scroll_list(ScrollDirection::Up);
								}
							}
						}
						/* User input mode. */
						Key::Char('\n')
						| Key::Char('s')
						| Key::Char('S')
						| Key::Char('m')
						| Key::Char('M')
						| Key::Char('i')
						| Key::Char('I')
						| Key::Char('+')
						| Key::Char('/')
						| Key::Insert => {
							app.selected_block = Blocks::UserInput;
							match input {
								Key::Char('m')
								| Key::Char('M')
								| Key::Char('i')
								| Key::Char('I')
								| Key::Char('+') => app.input_mode = InputMode::Load,
								_ => app.input_mode = InputMode::Search,
							}
							if input != Key::Char('\n') {
								app.input_query = String::new();
							}
							util::set_cursor_pos(
								terminal.backend_mut(),
								2 + app.input_query.width() as u16,
								2,
							)?;
							terminal.show_cursor()?;
						}
						/* Other character input. */
						Key::Char(v) => {
							/* Check if input is a number except zero. */
							let index = v.to_digit(10).unwrap_or(0);
							/* Show the used module info at given index. */
							if index != 0 {
								kernel_modules
									.show_used_module_info(index as usize - 1);
							}
						}
						_ => {}
					}
				} else {
					/* User input mode. */
					match input {
						/* Quit with ctrl+key combinations and ESC. */
						Key::Ctrl('c') | Key::Ctrl('d') | Key::Esc => {
							break;
						}
						/* Switch to the previous input mode. */
						Key::Up => {
							app.input_mode = match app.input_mode.prev_variant() {
								Some(v) => v,
								None => InputMode::max_value(),
							};
							if app.input_mode.is_none() {
								app.input_mode = InputMode::max_value();
							}
							app.input_query = String::new();
						}
						/* Switch to the next input mode. */
						Key::Down => {
							app.input_mode = match app.input_mode.next_variant() {
								Some(v) => v,
								None => {
									InputMode::min_value().next_variant().unwrap()
								}
							};
							app.input_query = String::new();
						}
						/* Exit user input mode. */
						Key::Char('\n')
						| Key::Char('\t')
						| Key::Right
						| Key::Ctrl('l')
						| Key::Left
						| Key::Ctrl('h') => {
							/* Select the next eligible block for action. */
							app.selected_block = match input {
								Key::Left | Key::Ctrl('h') => {
									match app.selected_block.prev_variant() {
										Some(v) => v,
										None => Blocks::max_value(),
									}
								}
								Key::Char('\n') => match app.input_mode {
									InputMode::Load => Blocks::ModuleInfo,
									_ => Blocks::ModuleTable,
								},
								_ => Blocks::ModuleTable,
							};
							/* Show the first modules information if the search mode is set. */
							if app.input_mode == InputMode::Search
								&& kernel_modules.index == 0
							{
								kernel_modules.scroll_list(ScrollDirection::Top);
							/* Load kernel module. */
							} else if app.input_mode == InputMode::Load
								&& !app.input_query.is_empty()
							{
								kernel_modules.current_name = app.input_query;
								kernel_modules
									.set_current_command(ModuleCommand::Load);
								app.input_query = String::new();
							}
							/* Hide terminal cursor and set the input mode flag. */
							terminal.hide_cursor()?;
							app.input_mode = InputMode::None;
						}
						/* Append character to input query. */
						Key::Char(c) => {
							app.input_query.push(c);
							kernel_modules.index = 0;
						}
						/* Delete the last character from input query. */
						Key::Backspace | Key::Delete => {
							app.input_query.pop();
							kernel_modules.index = 0;
						}
						_ => {}
					}
				}
			}
			/* Kernel events. */
			Event::Kernel(logs) => {
				kernel_logs.output = logs;
			}
			_ => {}
		}
	}
	Ok(())
}

/**
 * Entry point.
 */
fn main() {
	create_term(&util::parse_args(VERSION)).expect("failed to create terminal");
}
