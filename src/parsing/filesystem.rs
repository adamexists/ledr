/* Copyright © 2024 Adam House <adam@adamexists.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */
use crate::config::config_file::Config;
use anyhow::{anyhow, bail, Error};
use dirs::home_dir;
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

pub struct Filesystem {
	/// Set of file paths that have been inspected.
	/// Used to avoid circular includes.
	included_files: HashSet<String>,
}

impl Filesystem {
	pub fn new() -> Self {
		Self {
			included_files: HashSet::new(),
		}
	}

	pub fn open(&self, file_path: &str) -> Result<File, Error> {
		let path = Path::new(file_path);
		let file = File::open(path)?;
		Ok(file)
	}

	pub fn declare_file(&mut self, file_path: &str) -> Result<(), Error> {
		if self.included_files.contains(file_path) {
			bail!("Circular file includes: {}", file_path)
		}
		self.included_files.insert(file_path.parse()?);
		Ok(())
	}

	/// Fetches the config from the given path, or default path if none.
	pub fn get_config(
		&self,
		custom_config_path: Option<&String>,
	) -> Result<Config, Error> {
		let config_path = match &custom_config_path {
			None => {
				let home_dir = home_dir().unwrap_or_else(|| {
					panic!("Unable to determine home directory")
				});
				home_dir.join(".config/ledr/config.toml")
			},
			Some(p) => PathBuf::from(p),
		};

		// create empty config file if it doesn't exist
		if !config_path.exists() && custom_config_path.is_none() {
			if let Some(parent) = config_path.parent() {
				fs::create_dir_all(parent)?;
			}
			File::create(config_path.clone())?;
		}

		let content = fs::read_to_string(config_path)?;
		let config: Config = toml::from_str(&content)
			.map_err(|e| anyhow!("failed to parse config: {}", e))?;

		Ok(config)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_declare_file() {
		let mut filesystem = Filesystem::new();
		assert!(filesystem.declare_file("path/to/file").is_ok());
		assert!(filesystem.included_files.contains("path/to/file"));
		assert!(filesystem.declare_file("path/to/file").is_err());
	}
}
