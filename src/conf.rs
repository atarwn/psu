use anyhow::{Context, Result};
use config::{Config as ConfigBuilder, ConfigError, File, FileFormat};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const CONFIG_FILE: &str = "config.toml";
const APP_NAME: &str = "psu";

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
	pub path: PathBuf,
}

impl Config {
	pub fn gen_config() -> Result<()> {
		let dir = Self::get_app_data_dir().context("Failed to obtain config directory.")?;

		if !dir.exists() {
			fs::create_dir_all(&dir).context("Failed create config dirs")?;
		}

		let conf = toml::to_string_pretty(&Config::default()).context("Failed to parse configuration.")?;
		fs::write(dir.join(CONFIG_FILE), conf).context("Failed write config.")?;

		Ok(())
	}

	pub fn load() -> anyhow::Result<Self> {
		let Some(dir) = Self::get_app_data_dir() else {
			return Ok(Self::default());
		};

		let path = dir.join(CONFIG_FILE);
		if !path.exists() {
			Self::gen_config()?;
		}

		Ok(Self::build(path)?)
	}

	fn replace_tilde(&mut self) {
		if let Some(home) = dirs::home_dir() {
			self.path = self.path.to_string_lossy().replace('~', &home.to_string_lossy()).into();
		}
	}

	fn get_app_data_dir() -> Option<PathBuf> {
		Some(dirs::config_dir()?.join(APP_NAME))
	}

	fn build(path: PathBuf) -> Result<Self, ConfigError> {
		let mut build: Self = ConfigBuilder::builder()
			.add_source(File::from(path).format(FileFormat::Toml))
			.build()?
			.try_deserialize()?;

		build.replace_tilde();
		Ok(build)
	}
}

impl Default for Config {
	fn default() -> Self {
		Self {
			path: dirs::home_dir().unwrap_or_default(),
		}
	}
}

#[cfg(test)]
mod test_config {
	use super::Config;

	#[test]
	fn rep_tilde() {
		if let Some(home) = dirs::home_dir() {
			let mut conf = Config {
				path: "~/Downloads".into(),
			};
			conf.replace_tilde();
			// println!("{}, {}", conf.path.display(), home.join("Downloads").display());
			assert_eq!(conf.path, home.join("Downloads"));
		}
	}
}
