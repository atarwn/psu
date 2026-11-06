use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
	key TEXT PRIMARY KEY,
	value BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS passwords (
	id INTEGER PRIMARY KEY,
	service TEXT NOT NULL,
	login TEXT NOT NULL,
	password BLOB NOT NULL
);";

pub struct StoredPassword {
	pub id: i64,
	pub service: String,
	pub login: String,
	pub password: Vec<u8>,
}

pub struct Db {
	conn: Connection,
}

impl Db {
	pub fn open(path: &Path) -> Result<Self> {
		if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
			std::fs::create_dir_all(parent).context("Failed to create data directory")?;
		}

		let conn = Connection::open(path).context("Failed to open database")?;
		conn.pragma_update(None, "journal_mode", "WAL").context("Failed to set journal_mode")?;
		conn.pragma_update(None, "busy_timeout", 5000).context("Failed to set busy_timeout")?;
		conn.execute_batch(SCHEMA).context("Failed to create schema")?;

		Ok(Self { conn })
	}

	pub fn is_initialized(&self) -> bool {
		self.meta("salt").ok().flatten().is_some()
	}

	fn meta(&self, key: &str) -> Result<Option<Vec<u8>>> {
		self.conn
			.query_row("SELECT value FROM meta WHERE key = ?1", params![key], |row| row.get(0))
			.optional()
			.with_context(|| format!("Failed to read meta key {key:?}"))
	}

	pub fn salt(&self) -> Result<Option<Vec<u8>>> {
		self.meta("salt")
	}

	pub fn verify_blob(&self) -> Result<Option<Vec<u8>>> {
		self.meta("verify")
	}

	pub fn initialize(&self, salt: &[u8], verify: &[u8]) -> Result<()> {
		self.conn
			.execute("INSERT INTO meta (key, value) VALUES ('salt', ?1)", params![salt])
			.context("Failed to store salt")?;
		self.conn
			.execute("INSERT INTO meta (key, value) VALUES ('verify', ?1)", params![verify])
			.context("Failed to store verification blob")?;
		Ok(())
	}

	pub fn load(&self) -> Result<Vec<StoredPassword>> {
		let mut stmt = self
			.conn
			.prepare("SELECT id, service, login, password FROM passwords ORDER BY id")
			.context("Failed to prepare load query")?;

		let rows = stmt
			.query_map([], |row| {
				Ok(StoredPassword {
					id: row.get(0)?,
					service: row.get(1)?,
					login: row.get(2)?,
					password: row.get(3)?,
				})
			})
			.context("Failed to query passwords")?;

		rows.collect::<std::result::Result<Vec<_>, _>>()
			.context("Failed to read password rows")
	}

	pub fn insert(&self, service: &str, login: &str, password: &[u8]) -> Result<i64> {
		self.conn
			.execute(
				"INSERT INTO passwords (service, login, password) VALUES (?1, ?2, ?3)",
				params![service, login, password],
			)
			.context("Failed to insert password")?;
		Ok(self.conn.last_insert_rowid())
	}

	pub fn update(&self, id: i64, service: &str, login: &str, password: &[u8]) -> Result<()> {
		let changed = self
			.conn
			.execute(
				"UPDATE passwords SET service = ?2, login = ?3, password = ?4 WHERE id = ?1",
				params![id, service, login, password],
			)
			.context("Failed to update password")?;

		if changed == 0 {
			bail!("No password with id {id}");
		}
		Ok(())
	}

	pub fn delete(&self, id: i64) -> Result<()> {
		let changed = self
			.conn
			.execute("DELETE FROM passwords WHERE id = ?1", params![id])
			.context("Failed to delete password")?;

		if changed == 0 {
			bail!("No password with id {id}");
		}
		Ok(())
	}
}
