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
use crate::investment::commodity::Commodity;
use crate::investment::sale::Sale;
use crate::util::date::{Date, Duration};
use crate::util::scalar::Scalar;
use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq)]
pub struct Lot {
	pub id: String,

	/// Indicates whether the user specifically named this lot
	pub is_named: bool,

	pub status: LotStatus,
	pub account: String,

	pub commodity: Commodity,
	pub quantity: Scalar, // always in positive terms; can't go negative

	pub acquisition_date: Date,

	pub closed_date: Option<Date>,
	pub sales: Vec<Sale>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LotStatus {
	Open,
	Closed,
}

impl PartialOrd for LotStatus {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for LotStatus {
	fn cmp(&self, other: &Self) -> Ordering {
		match (self, other) {
			(LotStatus::Open, LotStatus::Closed) => Ordering::Less,
			(LotStatus::Closed, LotStatus::Open) => {
				Ordering::Greater
			},
			_ => Ordering::Equal,
		}
	}
}

impl Lot {
	pub fn time_held(&self, as_of: &Date) -> Duration {
		let end = match self.closed_date {
			Some(date) => {
				if as_of < &date {
					as_of
				} else {
					&date.clone()
				}
			},
			None => as_of,
		};
		self.acquisition_date.until(end)
	}

	/// Assembles sales into a semicolon-separated string in the format: "{quantity} on {date}; ..."
	pub fn format_sales(&self) -> String {
		if self.sales.is_empty() {
			return "n/a".to_string();
		}

		self.sales
			.iter()
			.map(|sale| {
				format!("{} on {}", sale.quantity, sale.date)
			})
			.collect::<Vec<String>>()
			.join("; ")
	}
}

impl PartialOrd for Lot {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for Lot {
	// TODO: This should be maximally deterministic, and thus be made to
	//  use every field on the Lot.
	fn cmp(&self, other: &Self) -> Ordering {
		// First: Compare by acquisition_date (ascending)
		let acquisition_cmp =
			self.acquisition_date.cmp(&other.acquisition_date);
		if acquisition_cmp != Ordering::Equal {
			return acquisition_cmp;
		}

		// Second: Compare by status (Open < Closed)
		let status_cmp = self.status.cmp(&other.status);
		if status_cmp != Ordering::Equal {
			return status_cmp;
		}

		// Third: Compare by commodity (lexicographically)
		let commodity_cmp = self.commodity.cmp(&other.commodity);
		if commodity_cmp != Ordering::Equal {
			return commodity_cmp;
		}

		// Fifth: Compare by account (lexicographically)
		let account_cmp = self.account.cmp(&other.account);
		if account_cmp != Ordering::Equal {
			return account_cmp;
		}

		// Sixth: Compare by number of sales (descending)
		let sales_cmp = other.sales.len().cmp(&self.sales.len());
		if sales_cmp != Ordering::Equal {
			return sales_cmp;
		}

		Ordering::Equal
	}
}
