/* Copyright (C) 2024 Adam House <adam@adamexists.com>
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
use anyhow::{bail, Error};
use std::collections::HashSet;
use std::fs::File;
use std::path::Path;

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
