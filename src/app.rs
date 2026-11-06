use crate::{crypto::Cipher, db::Db, ui::TableColors};
use anyhow::{Context, Result};
use ratatui::{
	style::palette::tailwind,
	widgets::{ScrollbarState, TableState},
};
use secrecy::{ExposeSecret, SecretString};
use std::{
	borrow::Borrow,
	fmt::Display,
};

pub const ITEM_HEIGHT: u16 = 3;

#[derive(Debug, Default, PartialEq)]
pub enum CurrentScreen {
	#[default]
	Main,
	Popup(Kind),
	Error,
	Help,
}

#[derive(Debug, Default, PartialEq)]
pub enum Kind {
	#[default]
	Add,
	Edit,
}

#[derive(Debug, Default, PartialEq)]
pub enum InputMode {
	#[default]
	Service,
	Login,
	Password,
}

pub struct PasswordTable {
	// Table
	pub table_state: TableState,
	// Scroll state table
	pub scroll_state: ScrollbarState,
	// Pallette for table
	pub colors: TableColors,
}

pub struct App {
	// User form
	pub input: UserInput,
	// Vector with passwords
	pub items: Vec<Password>,
	// Current field
	pub input_mode: InputMode,
	// Save errors
	pub error_message: String,
	// Scroll
	pub error_scroll: u16,
	// Scroll state errors
	pub error_scrollbar: ScrollbarState,
	// Table app
	pub table: PasswordTable,
	// Current input mode
	pub current_screen: CurrentScreen,
	db: Db,
	cipher: Cipher,
}

impl App {
	pub fn new(db: Db, cipher: Cipher) -> Result<Self> {
		let stored = db.load()?;
		let mut items = Vec::with_capacity(stored.len());

		for row in stored {
			let password = cipher.decrypt(&row.password)?;
			items.push(Password {
				id: Some(row.id),
				service: row.service,
				login: row.login,
				password: SecretString::new(password.to_string()),
			});
		}

		let item_count = items.len();
		let selected = (item_count > 0).then_some(0);

		Ok(Self {
			input: UserInput::default(),
			items,
			input_mode: InputMode::default(),
			error_message: String::new(),
			error_scroll: 0,
			error_scrollbar: ScrollbarState::default(),
			table: PasswordTable {
				table_state: TableState::default().with_selected(selected),
				scroll_state: ScrollbarState::new(item_count.saturating_sub(1)),
				colors: TableColors::new(&tailwind::GRAY),
			},
			current_screen: CurrentScreen::default(),
			db,
			cipher,
		})
	}

	pub fn show_error(&mut self, error: impl Display) {
		self.error_message = error.to_string();
		self.error_scroll = 0;
		self.error_scrollbar = ScrollbarState::new(self.error_message.lines().count().max(1)).position(0);
		self.current_screen = CurrentScreen::Error;
	}

	pub fn next_row(&mut self) {
		if self.items.is_empty() {
			self.table.table_state.select(None);
			return;
		}

		let i = match self.table.table_state.selected() {
			Some(i) if i + 1 < self.items.len() => i + 1,
			_ => 0,
		};
		self.select_row(i);
	}

	pub fn previous_row(&mut self) {
		if self.items.is_empty() {
			self.table.table_state.select(None);
			return;
		}

		let i = match self.table.table_state.selected() {
			Some(0) => self.items.len() - 1,
			Some(i) => i - 1,
			None => 0,
		};
		self.select_row(i);
	}

	fn select_row(&mut self, index: usize) {
		self.table.table_state.select(Some(index));
		self.table.scroll_state = self.table.scroll_state.position(index * usize::from(ITEM_HEIGHT));
	}

	fn refresh_scroll(&mut self) {
		let position = self.table.table_state.selected().unwrap_or(0) * usize::from(ITEM_HEIGHT);
		self.table.scroll_state = self
			.table
			.scroll_state
			.content_length(self.items.len().saturating_sub(1))
			.position(position);
	}

	pub fn next_input_mode(&mut self) {
		self.input_mode = match self.input_mode {
			InputMode::Login => InputMode::Password,
			InputMode::Password => InputMode::Service,
			InputMode::Service => InputMode::Login,
		};
	}

	pub fn prev_input_mode(&mut self) {
		self.input_mode = match self.input_mode {
			InputMode::Login => InputMode::Service,
			InputMode::Password => InputMode::Login,
			InputMode::Service => InputMode::Password,
		};
	}

	pub fn next_column(&mut self) {
		self.table.table_state.select_next_column();
	}

	pub fn previous_column(&mut self) {
		self.table.table_state.select_previous_column();
	}

	pub fn commit_add(&mut self) -> Result<()> {
		let [service, login, password] = self.input.ref_array();
		let ciphertext = self.cipher.encrypt(&password)?;
		let id = self.db.insert(&service, &login, &ciphertext)?;

		self.items.push(Password::with_id(Some(id), service, login, password));
		self.refresh_scroll();
		Ok(())
	}

	pub fn commit_edit(&mut self) -> Result<()> {
		let index = self.table.table_state.selected().context("No row selected")?;
		let id = self.items.get(index).and_then(|p| p.id).context("Row has no database id")?;

		let [service, login, password] = self.input.ref_array();
		let ciphertext = self.cipher.encrypt(&password)?;
		self.db.update(id, &service, &login, &ciphertext)?;

		let data = self.items.get_mut(index).context("Selected row out of bounds")?;
		data.service = service;
		data.login = login;
		data.password = SecretString::new(password);

		Ok(())
	}

	pub fn delete(&mut self) -> Result<()> {
		if self.items.is_empty() {
			return Ok(());
		}

		let index = self.table.table_state.selected().context("No row selected")?;
		let id = self.items.get(index).and_then(|p| p.id).context("Row has no database id")?;
		self.db.delete(id)?;

		self.items.remove(index);
		if self.items.is_empty() {
			self.table.table_state.select(None);
		} else if index >= self.items.len() {
			self.table.table_state.select(Some(self.items.len() - 1));
		}
		self.refresh_scroll();
		Ok(())
	}

	pub fn modify(&mut self) {
		if let Some(index) = self.table.table_state.selected()
			&& let Some(data) = self.items.get(index)
		{
			self.input = data.into();
		}
	}

	pub fn clip_row(&self) -> Result<()> {
		if let Some(index) = self.table.table_state.selected() {
			let password = self.items.get(index).context("Selected row out of bounds")?;
			copy_to_clipboard(&password.to_string())?;
		}
		Ok(())
	}

	pub fn clip_password(&self) -> Result<()> {
		if let Some(index) = self.table.table_state.selected() {
			let password = self.items.get(index).context("Selected row out of bounds")?;
			copy_to_clipboard(password.password())?;
		}
		Ok(())
	}

	pub fn clip_column(&self) -> Result<()> {
		if let Some(index) = self.table.table_state.selected() {
			let password = self.items.get(index).context("Selected row out of bounds")?;

			let data = match self.table.table_state.selected_column() {
				Some(1) => password.login(),
				Some(0) => password.service(),
				_ => password.password(),
			};
			copy_to_clipboard(data)?;
		}
		Ok(())
	}
}

fn copy_to_clipboard(text: &str) -> Result<()> {
	arboard::Clipboard::new()
		.context("Failed to access clipboard")?
		.set_text(text.to_owned())
		.context("Failed to copy to clipboard")
}

pub trait Data {
	fn login(&self) -> &str;
	fn password(&self) -> &str;
	fn service(&self) -> &str;
}

#[derive(Default)]
pub struct UserInput {
	pub service: tui_input::Input,
	pub login: tui_input::Input,
	pub password: tui_input::Input,
}

impl UserInput {
	pub fn reset_data(&mut self) {
		self.login.reset();
		self.password.reset();
		self.service.reset();
	}

	pub fn ref_array(&self) -> [String; 3] {
		[self.service().into(), self.login().into(), self.password().into()]
	}

	#[allow(unused)]
	pub fn from_array<T: AsRef<str>>(value: impl Borrow<[T; 3]>) -> Self {
		let value = value.borrow();
		Self {
			service: value[0].as_ref().into(),
			login: value[1].as_ref().into(),
			password: value[2].as_ref().into(),
		}
	}
}

impl Data for UserInput {
	fn login(&self) -> &str {
		self.login.value()
	}

	fn password(&self) -> &str {
		self.password.value()
	}

	fn service(&self) -> &str {
		self.service.value()
	}
}

#[derive(Clone, Debug)]
#[allow(clippy::struct_field_names)]
pub struct Password {
	pub id: Option<i64>,
	pub service: String,
	pub login: String,
	pub password: SecretString,
}

impl Password {
	#[cfg(test)]
	pub fn new<T: AsRef<str>>(service: T, login: T, password: T) -> Self {
		Self::with_id(None, service.as_ref().into(), login.as_ref().into(), password.as_ref().into())
	}

	pub fn with_id(id: Option<i64>, service: String, login: String, password: String) -> Self {
		Self {
			id,
			service,
			login,
			password: SecretString::new(password),
		}
	}

	pub fn ref_array(&self) -> [String; 3] {
		[self.service().into(), self.login().into(), self.password().into()]
	}
}

impl Display for Password {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}\t{}\t{}", self.service, self.login, self.password.expose_secret())
	}
}

impl Data for Password {
	fn login(&self) -> &str {
		&self.login
	}
	fn password(&self) -> &str {
		self.password.expose_secret()
	}

	fn service(&self) -> &str {
		&self.service
	}
}

impl<T: Borrow<Password>> From<T> for UserInput {
	fn from(value: T) -> Self {
		let value = value.borrow();
		Self {
			service: value.service().into(),
			login: value.login().into(),
			password: value.password().into(),
		}
	}
}

#[cfg(test)]
mod test_app {
	use super::{Password, UserInput};

	#[test]
	fn user_input_ref_array() {
		let user_input = UserInput::from_array(["a", "b", "c"]);
		assert_eq!(user_input.ref_array(), ["a", "b", "c"]);
	}

	#[test]
	fn password_ref_array() {
		let user_input = Password::new("a", "b", "c");
		assert_eq!(user_input.ref_array(), ["a", "b", "c"]);
	}

	#[test]
	fn reset_input() {
		let mut user_input = UserInput::from_array(["a", "b", "c"]);
		user_input.reset_data();
		assert_eq!(user_input.ref_array(), ["", "", ""]);
	}
}
