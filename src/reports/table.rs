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

/// Standard table printer for those reports, such as account summaries, that
/// report a potentially large number of single-line objects.
///
/// Not for use with financial statements that require complex nesting, sorting
/// and spacing.
pub struct Table {
	column_count: usize,
	rows: Vec<Vec<String>>,
	right_align: Vec<bool>,
}

impl Table {
	pub fn new(column_count: usize) -> Self {
		Self {
			column_count,
			rows: Vec::new(),
			right_align: vec![false; column_count],
		}
	}

	pub fn add_row(&mut self, row: Vec<String>) {
		if row.len() != self.column_count {
			panic!("Inconsistent column count");
		}

		self.rows.push(row);
	}

	pub fn right_align(&mut self, col: usize) {
		self.right_align[col] = true;
	}

	pub fn print(
		&self,
		total_column_index: usize,
		total: &String,
		currency_index: usize,
		currency: &String,
	) {
		let mut max_widths = vec![0; self.column_count];

		for row in &self.rows {
			for (i, value) in row.iter().enumerate() {
				let width = value.len();
				if width > max_widths[i] {
					max_widths[i] = width;
				}
			}
		}

		for row in &self.rows {
			for (i, value) in row.iter().enumerate() {
				if self.right_align[i] {
					print!(
						"{:>width$}",
						value,
						width = max_widths[i]
					);
				} else {
					print!(
						"{:<width$}",
						value,
						width = max_widths[i]
					);
				}
				if i < row.len() - 1 {
					print!("  ");
				}
			}
			println!();
		}

		for (_, width) in
			max_widths.iter().enumerate().take(total_column_index)
		{
			print!("{:width$}  ", "", width = width);
		}

		let separator_width = max_widths[total_column_index]
			+ max_widths[currency_index]
			+ 2; // +1 for space between columns
		println!(
			"{:->separator_width$}",
			"",
			separator_width = separator_width
		);

		for (i, width) in
			max_widths.iter().enumerate().take(self.column_count)
		{
			if i == total_column_index {
				if self.right_align[i] {
					print!(
						"{:>width$}",
						total,
						width = width
					);
				} else {
					print!(
						"{:<width$}",
						total,
						width = width
					);
				}
			} else if i == currency_index {
				print!("{:<width$}", currency, width = width);
			} else {
				print!("{:width$}", "", width = width);
			}

			if i < self.column_count - 1 {
				print!("  ");
			}
		}
		println!();
	}
}
