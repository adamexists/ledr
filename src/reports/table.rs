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
	rows: Vec<Row>,
	right_align: Vec<bool>,
}

pub enum Row {
	Data(Vec<String>),
	Separator,
	// TODO: Total rows and separators should be a row type.
}

impl Table {
	pub fn new(column_count: usize) -> Self {
		Self {
			column_count,
			rows: Vec::new(),
			right_align: vec![false; column_count],
		}
	}

	pub fn add_row(&mut self, row: Vec<&str>) {
		if row.len() != self.column_count {
			panic!("Inconsistent column count");
		}

		self.rows.push(Row::Data(
			row.into_iter().map(|s| s.to_string()).collect(),
		));
	}

	pub fn add_separator(&mut self) {
		self.rows.push(Row::Separator);
	}

	pub fn right_align(&mut self, col: usize) {
		self.right_align[col] = true;
	}

	// TODO: This could also use a refactor.
	pub fn print(
		&self,
		total_col: Option<usize>,
		total: Option<String>,
		cur_index: Option<usize>, // TODO: Refactor.
		currency: Option<String>,
	) {
		println!();
		let mut max_widths = vec![0; self.column_count];

		// Determine maximum widths for each column
		for row in &self.rows {
			if let Row::Data(data_row) = row {
				for (i, value) in data_row.iter().enumerate() {
					let width = value.len();
					if width > max_widths[i] {
						max_widths[i] = width;
					}
				}
			}
		}

		// Print each row
		for row in &self.rows {
			match row {
				Row::Data(data_row) => {
					for (i, value) in
						data_row.iter().enumerate()
					{
						if self.right_align[i] {
							print!("{:>width$}", value, width = max_widths[i]);
						} else {
							print!("{:<width$}", value, width = max_widths[i]);
						}
						if i < data_row.len() - 1 {
							print!("  ");
						}
					}
					println!();
				},
				Row::Separator => {
					let total_width: usize = max_widths
						.iter()
						.sum::<usize>()
						+ (2 * (self.column_count - 1));
					println!(
						"{:-<total_width$}",
						"",
						total_width = total_width
					);
				},
			}
		}

		if total_col.is_none() || total.is_none() {
			return;
		}

		let total_col_index = total_col.unwrap();
		let total_val = total.unwrap();

		// Print the footer
		for (_, width) in
			max_widths.iter().enumerate().take(total_col_index)
		{
			print!("{:width$}  ", "", width = width);
		}

		let mut separator_width = match cur_index {
			Some(index) => {
				max_widths[total_col_index]
					+ max_widths[index] + 2
			},
			None => max_widths[total_col_index] + 2,
		};
		if total_col_index + 1 == self.column_count {
			separator_width -= 2;
		}

		println!(
			"{:->separator_width$}",
			"",
			separator_width = separator_width
		);

		for (i, width) in
			max_widths.iter().enumerate().take(self.column_count)
		{
			if i == total_col_index {
				if self.right_align[i] {
					print!(
						"{:>width$}",
						total_val,
						width = width
					);
				} else {
					print!(
						"{:<width$}",
						total_val,
						width = width
					);
				}
			} else if cur_index.is_some() && i == cur_index.unwrap()
			{
				print!(
					"{:<width$}",
					currency.clone()
						.unwrap_or("".to_string()),
					width = width
				);
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
