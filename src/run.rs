use super::{
	app::{App, CurrentScreen, Data, InputMode, Kind},
	ui::ui,
};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{Terminal, prelude::Backend};
use tui_input::backend::crossterm::EventHandler;

pub fn run_app<B: Backend + 'static>(terminal: &mut Terminal<B>, app: &mut App) -> anyhow::Result<()>
where
	B::Error: std::error::Error + Send + Sync + 'static,
{
	loop {
		terminal.draw(|frame| ui(frame, app))?;

		let event = event::read()?;
		if let Event::Key(key) = event {
			if key.kind == event::KeyEventKind::Release {
				continue;
			}

			match app.current_screen {
				CurrentScreen::Main => match key.code {
					KeyCode::Esc => return Ok(()),
					KeyCode::Char('P') => app.clip_row().unwrap_or_else(|e| app.show_error(e)),
					KeyCode::Char('c') => app.clip_column().unwrap_or_else(|e| app.show_error(e)),
					KeyCode::Char('p') => app.clip_password().unwrap_or_else(|e| app.show_error(e)),
					KeyCode::Char('j') | KeyCode::Down => app.next_row(),
					KeyCode::Char('k') | KeyCode::Up => app.previous_row(),
					KeyCode::Char('l') | KeyCode::Right => app.next_column(),
					KeyCode::Char('h') | KeyCode::Left => app.previous_column(),
					KeyCode::Char('d' | 'D') => app.delete().unwrap_or_else(|e| app.show_error(e)),
					KeyCode::Char('?') => app.current_screen = CurrentScreen::Help,
					KeyCode::Char('n' | 'N') => app.current_screen = CurrentScreen::Popup(Kind::Add),
					KeyCode::Char('m' | 'M') => {
						app.modify();
						app.current_screen = CurrentScreen::Popup(Kind::Edit);
					}
					_ => {}
				},
				CurrentScreen::Popup(ref kind) => match key.code {
					KeyCode::Esc => {
						if kind == &Kind::Edit {
							app.input.reset_data();
						}
						app.current_screen = CurrentScreen::Main;
					}
					KeyCode::Enter => {
						// Skip print if fields are empty
						if app.input.login().is_empty()
							&& app.input.password().is_empty()
							&& app.input.service().is_empty()
						{
							continue;
						}

						let result = if kind == &Kind::Edit { app.commit_edit() } else { app.commit_add() };

						match result {
							Ok(()) => {
								app.input.reset_data();
								app.input_mode = InputMode::default();
								app.current_screen = CurrentScreen::Main;
							}
							Err(error) => app.show_error(error),
						}
					}
					// Switch fields
					KeyCode::Down | KeyCode::Tab => app.next_input_mode(),
					KeyCode::Up => app.prev_input_mode(),
					_ => {
						// Switch inputs for fields
						match app.input_mode {
							InputMode::Login => app.input.login.handle_event(&event),
							InputMode::Password => app.input.password.handle_event(&event),
							InputMode::Service => app.input.service.handle_event(&event),
						};
					}
				},

				CurrentScreen::Help | CurrentScreen::Error => match key.code {
					KeyCode::Esc => app.current_screen = CurrentScreen::Main,
					KeyCode::Char('j') | KeyCode::Down => {
						if let num = app.error_scroll.saturating_add(1)
							&& (num as usize) < app.error_message.lines().count()
						{
							app.error_scroll = num;
						}
					}
					KeyCode::Char('k') | KeyCode::Up => {
						if let num = app.error_scroll.saturating_sub(1)
							&& (num as usize) < app.error_message.lines().count()
						{
							app.error_scroll = num;
						}
					}
					_ => {}
				},
			}
		}
	}
}
