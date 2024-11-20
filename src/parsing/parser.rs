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
use crate::tabulation::ledger::{CostBasisAmountType, CostBasisInput, Ledger};
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
}

impl ParseResult {
	fn note_precision(&mut self, currency: String, precision: u32) {
		let entry = self
			.max_precision_by_currency
			.entry(currency)
			.or_insert(0);
		*entry = (*entry).max(precision);
	}
}

/// First pass to process only directive lines. Include statements in the file
/// may cause this to be called recursively, so it uses the passed Ledger struct
/// to keep track of files it's traversed before, and block circular inclusion.
fn first_pass(
	path: &str,
	file: &File,
	ledger: &mut Ledger,
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
		// TODO: Document how includes work. They are not directives
		//  because they do not include a date. The file path is
		//  relative to the working directory, so the best practice is
		//  to use fully qualified paths for include directives.
		if l.starts_with("include") {
			let include: Vec<&str> = l.split_whitespace().collect();
			if include.len() != 2 {
				bail!("Invalid include (line {})", i)
			}

			let file = file_from_path(include[1])?;
			first_pass(include[1], &file, ledger)?;
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
) -> Result<(), Error> {
	let reader = io::BufReader::new(file);

	let mut lines = reader.lines();
	let mut i = 0;

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
			second_pass(&file, ledger, parse_result)?;
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
		}

		// Handle entry declaration lines with a date and description
		if let Some((date_str, desc)) = l.split_once(' ') {
			if let Ok(date) = Date::from_str(date_str.trim()) {
				ledger.new_entry(date, desc.trim().to_string())
					.map_err(|e| {
						anyhow!("{} (line {})", e, i)
					})?;
				continue;
			}
		}

		// Make sure the line is not a date by itself
		if Date::from_str(l).is_ok() {
			bail!("Orphaned date (line {}): {}", i, l);
		}

		// Handle entry detail lines
		let parts: Vec<&str> = l.split_whitespace().collect();

		// Handle virtual entry detail lines (account name only)
		if parts.len() == 1 {
			let account = parts[0].to_string();
			ledger.set_virtual_detail(account)
				.map_err(|e| anyhow!("{} (line {})", e, i))?;
			continue;
		} else if parts.len() < 3 {
			bail!("Invalid line format (line {}): {}", i, l);
		}

		let account = parts[0].to_string();
		let amount = Scalar::from_str(&parts[1])?;
		let currency = parts[2].to_string();

		// if exactly three parts, no cost basis
		if parts.len() == 3 {
			parse_result.note_precision(
				currency.clone(),
				amount.resolution(),
			);
			ledger.add_detail(account, amount, currency, None)
				.map_err(|e| anyhow!("{} (line {})", e, i))?;
			continue;
		}

		// If we get here, we know we have a cost basis
		let basis_str = parts[3..].join(" ");
		let basis_parts = basis_str.split_once(' ');

		if basis_parts.is_none() {
			bail!("Invalid cost basis (line {}): {}", i, l);
		}

		let (operator, basis) = basis_parts.unwrap();

		let b_parts: Vec<&str> = basis.split_whitespace().collect();
		if b_parts.len() != 2 {
			bail!("Invalid cost basis (line {}): {}", i, l);
		}

		let amount_type = match operator {
			"@" => CostBasisAmountType::UnitCost,
			"@@" => CostBasisAmountType::TotalCost,
			_ => bail!("Invalid cost basis (line {}): {}", i, l),
		};

		let cb_amount = Scalar::from_str(b_parts[0]).map_err(|_| {
			anyhow!("Invalid scalar value (line {}): {}", i, l)
		})?;
		let cb_currency = b_parts[1].to_string();

		parse_result.note_precision(
			cb_currency.clone(),
			cb_amount.resolution(),
		);

		ledger.add_detail(
			account,
			amount,
			currency,
			Some(CostBasisInput {
				amount: cb_amount,
				amount_type,
				currency: cb_currency,
			}),
		)
		.map_err(|e| anyhow!("{} (line {})", e, i))?;
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
/// reporting will use the latest one processed in the file. TODO: Manpage this.
pub fn parse(path: &str, ledger: &mut Ledger) -> Result<ParseResult, Error> {
	let mut file = file_from_path(path)?;

	first_pass(&path, &file, ledger)?;
	file.rewind()?;

	// Second pass is responsible for assembling the ParseResult object,
	// which we pass in this way so it can be passed recursively within.
	let mut output: ParseResult = Default::default();
	second_pass(&file, ledger, &mut output)?;

	Ok(output)
}

fn file_from_path(file_path: &str) -> Result<File, Error> {
	let path = Path::new(file_path);
	let file = File::open(path)?;
	Ok(file)
}
