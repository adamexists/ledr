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
use crate::gl::ledger::Ledger;
use crate::util::amount::Amount;
use crate::util::date::Date;
use crate::util::scalar::Scalar;
use anyhow::{anyhow, bail, Error};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io;
use std::io::{BufRead, Seek};
use std::path::Path;

#[derive(Debug, Default)]
pub struct ParseResult {
	pub max_precision_by_currency: HashMap<String, u32>, // currency > precision
	pub latest_date: Date, // latest date across entries (not directives)
}

impl ParseResult {
	fn note_precision(&mut self, currency: &str, precision: u32) {
		let entry = self
			.max_precision_by_currency
			.entry(currency.to_owned())
			.or_insert(0);
		*entry = (*entry).max(precision);
	}

	fn note_date(&mut self, date: Date) {
		if self.latest_date < date {
			self.latest_date = date;
		}
	}
}

/// First pass to process only directive lines. Include statements in the file
/// may cause this to be called recursively, so it uses the passed Ledger struct
/// to keep track of files it's traversed before, and block circular inclusion.
fn first_pass(
	path: &str,
	file: &File,
	ledger: &mut Ledger,
	begin: &Date,
	end: &Date,
) -> Result<(), Error> {
	ledger.declare_file(path)?;

	let reader = io::BufReader::new(file);

	for (i, line) in reader.lines().enumerate() {
		let l = line?.trim().to_string();

		// Skip blank lines and comments
		if l.is_empty() || l.starts_with("#") {
			continue;
		}

		// Handle includes, which recursively first_passes when seen
		if l.starts_with("include") {
			let include: Vec<&str> = l.split_whitespace().collect();
			if include.len() != 2 {
				bail!("Invalid include (line {})", i)
			}

			let file = file_from_path(include[1])?;
			first_pass(include[1], &file, ledger, begin, end)?;
			continue;
		}

		let mut directive: VecDeque<&str> = match l.strip_prefix("!") {
			None => continue,
			Some(d) => d.split_whitespace().collect(),
		};

		if directive.len() < 2 {
			bail!("Invalid directive (line {}): {}", i + 1, l);
		}

		let date_str = directive.pop_front().unwrap();
		let date = Date::from_str(date_str.trim())
			.map_err(|e| anyhow!("{} (line {})", e, i))?;

		// Ignore entries from outside the date bounds
		if &date < begin || &date > end {
			continue;
		}

		match directive[0] {
			"account" if directive.len() == 2 => {
				let account = directive[1].to_string();
				ledger.declare_account(account, date).map_err(
					|e| anyhow!("{} (line {})", e, i),
				)?;
			},
			"currency" if directive.len() == 2 => {
				let currency = directive[1].to_string();
				ledger.declare_currency(currency, date)
					.map_err(|e| {
						anyhow!("{} (line {})", e, i)
					})?;
			},
			"rate" if directive.len() == 4 => {
				let from = directive[1].to_string();
				let to = directive[2].to_string();
				let rate = directive[3];
				ledger.exchange_rates
					.declare(
						date,
						from,
						to,
						Scalar::from_str(rate)
							.map_err(|e| {
								anyhow!("{} (line {})", e, i)
							})?,
					)
					.map_err(|e| {
						anyhow!("{} (line {})", e, i)
					})?;
			},
			_ => bail!("Invalid directive (line {}): {}", i + 1, l),
		}
	}

	Ok(())
}

/// Second pass to process everything else other than directives. Include
/// statements may cause this method to call itself recursively, but it does
/// not need to keep track of where it is to avoid circular include statements
/// because first_pass has already done that.
fn second_pass(
	file: &File,
	ledger: &mut Ledger,
	parse_result: &mut ParseResult,
	begin: &Date,
	end: &Date,
) -> Result<(), Error> {
	let reader = io::BufReader::new(file);

	let mut lines = reader.lines();
	let mut i = 0;

	let mut ignore_until_next_entry = false;

	while let Some(Ok(line)) = lines.next() {
		i += 1;
		let l = line.trim();

		// If a line is blank, this entry is over (or we are not in one)
		if l.is_empty() {
			ledger.finish_entry()
				.map_err(|e| anyhow!("{} (line {})", e, i))?;
			continue;
		}

		// Handle includes, which recursively second_passes when seen.
		// No need to check the structure of the include because the
		// first pass would've failed by now if it were invalid.
		if line.starts_with("include") {
			let include: Vec<&str> =
				line.split_whitespace().collect();

			let file = file_from_path(include[1])?;
			second_pass(&file, ledger, parse_result, begin, end)?;
			continue;
		}

		// ignore comment lines and directives
		if l.starts_with("#") || l.starts_with('!') {
			continue;
		}

		// Lines that start with two slashes are reference lines.
		// Empty references are fine; they just do nothing
		if l.starts_with("//") && l.len() > 2 {
			let content = l[2..].trim();
			if !content.is_empty() {
				ledger.add_reference(content.to_string())?;
			}
			continue;
		}

		// Handle entry declaration lines with a date and description
		if let Some((date_str, desc)) = l.split_once(' ') {
			if let Ok(date) = Date::from_str(date_str.trim()) {
				if &date < begin || &date > end {
					ignore_until_next_entry = true;
					continue;
				}

				ignore_until_next_entry = false;

				ledger.new_entry(date, desc.trim().to_string())
					.map_err(|e| {
						anyhow!("{} (line {})", e, i)
					})?;

				parse_result.note_date(date);
				continue;
			}
		}

		// Make sure the line is not a date by itself
		if Date::from_str(l).is_ok() {
			bail!("Orphaned date (line {}): {}", i, l);
		}

		if ignore_until_next_entry {
			continue;
		}

		// Handle entry detail lines
		let parts: Vec<&str> = l.split_whitespace().collect();

		// The rest of the things this line can be all have different
		// numbers of terms
		if parts.len() == 1 {
			let account = parts[0].to_string();
			ledger.set_virtual_detail(account)
				.map_err(|e| anyhow!("{} (line {})", e, i))?;
			continue;
		}

		let account = parts[0].to_string();
		let amount = Amount::new(
			Scalar::from_str(parts[1])?,
			parts[2].to_string(),
		);
		parse_result.note_precision(
			&amount.currency,
			amount.value.resolution(),
		);

		match parts.len() {
			// no inline conversion
			3 => ledger
				.add_detail(account, amount, None, None)
				.map_err(|e| anyhow!("{} (line {})", e, i))?,
			6 => {
				// inline conversion, i.e. `@ 20.00 USD`
				let is_total_cost = match parts[3] {
					"@" => false,
					"@@" => true,
					_ => bail!(
						"Invalid format (line {})",
						i
					),
				};

				let mut ic_amount = Scalar::from_str(parts[4])
					.map_err(|_| {
						anyhow!("Invalid value (line {})", i)
					})?;
				let ic_currency = parts[5].to_string();

				parse_result.note_precision(
					&ic_currency,
					ic_amount.resolution(),
				);

				if is_total_cost {
					ic_amount /= amount.value
				};

				ledger.add_detail(
					account,
					amount,
					Some(Amount::new(
						ic_amount,
						ic_currency,
					)),
					None,
				)
				.map_err(|e| anyhow!("{} (line {})", e, i))?
			},
			7 => {
				// lot declaration, i.e. `{ 20.00 USD }`
				if parts[3] != "{" || parts[6] != "}" {
					bail!("Invalid format (line {})", i);
				}

				// Grab cost basis
				let cb_amount = Scalar::from_str(parts[4])
					.map_err(|_| {
						anyhow!("Invalid value (line {})", i)
					})?;
				let cb_currency = parts[5].to_string();

				parse_result.note_precision(
					&cb_currency,
					cb_amount.resolution(),
				);

				ledger.add_detail(
					account,
					amount,
					Some(Amount::new(
						cb_amount,
						cb_currency.clone(),
					)),
					Some(Amount {
						value: cb_amount,
						currency: cb_currency,
					}),
				)
				.map_err(|e| anyhow!("{} (line {})", e, i))?
			},
			_ => bail!("Invalid format (line {})", i),
		}
	}

	// Make sure to finish the last entry if the file ends without an empty line
	ledger.finish_entry()
		.map_err(|e| anyhow!("{} (line eof)", e))?;

	Ok(())
}

/// Opens and parses the file at file_path into the passed Ledger. We make two
/// passes through the file: the first processes directives, and the second
/// processes everything else. This means we are agnostic to the order of any
/// contents of the file. The only exception is that when multiple implicit
/// currency conversions occur in the same day between the same currencies, all
/// reporting will use the latest one processed in the file.
///
/// TODO: Create test case to test for begin and end directives working.
pub fn parse(
	file_path: &str,
	begin: &Date,
	end: &Date,
	ledger: &mut Ledger,
) -> Result<ParseResult, Error> {
	let mut file = file_from_path(file_path)?;

	first_pass(file_path, &file, ledger, begin, end)?;
	file.rewind()?;

	// Second pass is responsible for assembling the ParseResult object,
	// which we pass in this way so it can be passed recursively within.
	let mut output: ParseResult = Default::default();
	second_pass(&file, ledger, &mut output, begin, end)?;

	Ok(output)
}

fn file_from_path(file_path: &str) -> Result<File, Error> {
	let path = Path::new(file_path);
	let file = File::open(path)?;
	Ok(file)
}
