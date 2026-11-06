#![warn(clippy::all, clippy::pedantic)]
// #![allow(unused)]

use anyhow::{Result, anyhow, bail};
use app::App;
use conf::Config;
use crypto::{Cipher, VERIFY_TEXT, derive_key, generate_salt};
use crossterm::{
	cursor,
	event::DisableMouseCapture,
	execute,
	style::Stylize,
	terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use db::Db;
use ratatui::{Terminal, prelude::CrosstermBackend};
use run::run_app;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::{io, path::Path};

mod app;
mod conf;
mod crypto;
mod db;
mod run;
mod ui;

const DB_FILE: &str = "psu.db";
const OLD_CSV_FILE: &str = "psu.csv";
const MAX_ATTEMPTS: u8 = 3;

/// Undoes what `main` set up. Runs on normal exit, on `?` and while unwinding a panic.
struct TerminalGuard;

impl Drop for TerminalGuard {
	fn drop(&mut self) {
		restore_terminal();
	}
}

fn restore_terminal() {
	// Errors ignored: the guard also runs during unwinding, where there is nowhere to report to.
	let _ = disable_raw_mode();
	let _ = execute!(io::stderr(), LeaveAlternateScreen, DisableMouseCapture);
	let _ = execute!(io::stderr(), cursor::Show);
}

fn install_panic_hook() {
	let previous = std::panic::take_hook();

	std::panic::set_hook(Box::new(move |info| {
		restore_terminal();
		previous(info);
	}));
}

fn read_master(prompt: &str) -> Result<SecretString> {
	let password = rpassword::prompt_password(prompt)?;
	if password.is_empty() {
		bail!("Empty master password");
	}
	Ok(SecretString::new(password))
}

fn setup_master(db: &Db) -> Result<Cipher> {
	let master = loop {
		let first = read_master("Create master password: ")?;
		let second = read_master("Confirm master password: ")?;

		if first.expose_secret() == second.expose_secret() {
			break first;
		}
		eprintln!("Passwords do not match, try again.");
	};

	let salt = generate_salt();
	let key = derive_key(&master, &salt)?;
	let cipher = Cipher::new(&key);
	let verify = cipher.encrypt(VERIFY_TEXT)?;
	db.initialize(&salt, &verify)?;

	Ok(cipher)
}

fn unlock(db: &Db) -> Result<Cipher> {
	let salt = db.salt()?.ok_or_else(|| anyhow!("Database is missing salt"))?;

	for _ in 0..MAX_ATTEMPTS {
		let master = read_master("Master password: ")?;
		let key = derive_key(&master, &salt)?;
		let cipher = Cipher::new(&key);

		let verify = db
			.verify_blob()?
			.ok_or_else(|| anyhow!("Database is missing verification blob"))?;

		match cipher.decrypt(&verify) {
			Ok(text) if text.as_str() == VERIFY_TEXT => return Ok(cipher),
			_ => eprintln!("Wrong master password."),
		}
	}

	bail!("Too many failed attempts")
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CsvRow {
	service: String,
	login: String,
	password: String,
}

/// Imports the plaintext CSV from the previous version into the encrypted database.
/// The CSV is renamed to `psu.csv.bak` after a successful import.
fn migrate_csv(db: &Db, cipher: &Cipher, csv_path: &Path) -> Result<()> {
	let mut reader = csv::Reader::from_path(csv_path)?;
	let mut imported = 0usize;
	let mut failed = 0usize;

	for record in reader.deserialize::<CsvRow>() {
		match record {
			Ok(row) => {
				let ciphertext = cipher.encrypt(&row.password)?;
				db.insert(&row.service, &row.login, &ciphertext)?;
				imported += 1;
			}
			Err(_) => failed += 1,
		}
	}

	let backup = csv_path.with_extension("csv.bak");
	std::fs::rename(csv_path, &backup)?;

	if failed > 0 {
		eprintln!(
			"Migration: imported {imported} records, {failed} malformed rows skipped (backup: {}).",
			backup.display()
		);
	} else {
		eprintln!("Migrated {imported} passwords to the encrypted database (backup: {}).", backup.display());
	}
	Ok(())
}

fn main() -> Result<()> {
	let config = Config::load().unwrap_or_else(|e| {
		eprintln!("{}", format!("ERROR: {e}").red());
		Config::default()
	});

	let db = Db::open(&config.path.join(DB_FILE))?;
	let csv_path = config.path.join(OLD_CSV_FILE);

	let cipher = if db.is_initialized() {
		unlock(&db)?
	} else {
		let cipher = setup_master(&db)?;
		if csv_path.exists() {
			migrate_csv(&db, &cipher, &csv_path)?;
		}
		cipher
	};

	let mut app = App::new(db, cipher)?;
	let mut stderr = io::stderr();

	install_panic_hook();

	enable_raw_mode()?;
	execute!(stderr, EnterAlternateScreen)?;
	let _guard = TerminalGuard;

	let backend = CrosstermBackend::new(stderr);
	let mut terminal = Terminal::new(backend)?;

	run_app(&mut terminal, &mut app)?;

	Ok(())
}
