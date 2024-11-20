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

use crate::reports::table::Table;
use crate::tabulation::entry::Entry;
use crate::util::scalar;

// TODO: Build a collection of test cases for this one.
pub struct OrderedEntry {
	entries: Vec<Entry>,
}

impl OrderedEntry {
	pub fn new(mut entries: Vec<Entry>) -> Self {
		entries.sort();

		Self { entries }
	}

	pub fn account_summary(&self, account: &String, currency: &String) {
		let mut table = Table::new(4);
		table.right_align(2);

		let mut total = scalar::ZERO; // TODO move away from this syntax
		for entry in &self.entries {
			let net = entry.net_for_account(account, currency);
			total += net;
			if net != 0 {
				table.add_row(vec![
					entry.get_date().to_string(),
					entry.get_desc().clone(),
					net.to_string(),
					currency.to_string(),
				]);
			}
		}

		table.print(2, &total.to_string(), 3, currency)
	}
}
