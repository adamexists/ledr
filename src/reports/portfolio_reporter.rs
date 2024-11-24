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
use crate::gl::exchange_rate::ExchangeRates;
use crate::investment::lot::Lot;
use crate::reports::table::Table;
use crate::util::amount::Amount;
use crate::util::date::Date;
use crate::util::scalar::Scalar;
use std::collections::HashMap;

/// Struct for handling and displaying an ordered list of lots, for reports
pub struct PortfolioReporter {
	lots: Vec<Lot>,
}

impl PortfolioReporter {
	/// When this inits, it will sanitize its inputs with the passed
	/// parameters, and then store rounded lots for pretty reporting.
	pub fn new(
		mut lots: Vec<Lot>,
		max_precision_by_currency: HashMap<String, u32>,
		max_precision_allowed: u32,
	) -> Self {
		lots.sort();

		for lot in &mut lots {
			round_to_precision(
				&mut lot.quantity,
				lot.commodity.symbol(),
				&max_precision_by_currency,
				max_precision_allowed,
			);

			for sale in &mut lot.sales {
				round_to_precision(
					&mut sale.quantity,
					lot.commodity.symbol(),
					&max_precision_by_currency,
					max_precision_allowed,
				);

				if let Some(ref mut proceeds) = sale.unit_proceeds {
					round_to_precision(
						&mut proceeds.value,
						&proceeds.currency,
						&max_precision_by_currency,
						max_precision_allowed,
					);
				}
			}
		}

		Self { lots }
	}

	/// Prints an abbreviated table format, meant to contain open lots only.
	pub fn print_open_lots(&self, as_of: &Date) {
		if self.lots.is_empty() {
			println!("No open lots");
			return;
		}

		let mut table = Table::new(7);
		table.right_align(vec![0, 1, 2, 4, 5]);

		table.add_header(vec![
			"ID",
			"Opened",
			"Held",
			"Asset",
			"Qty",
			"Cost Basis",
			"Account",
		]);

		table.add_separator();
		for l in self.lots.iter() {
			table.add_row(vec![
				&l.id,
				&l.acquisition_date.to_string(),
				&l.time_held(as_of).to_string(),
				l.commodity.symbol(),
				&l.quantity.to_string(),
				&l.commodity.cost_basis().to_string(),
				&l.account.to_string(),
			])
		}

		let bottom_line = if self.lots.len() == 1 {
			"Open Lot"
		} else {
			"Open Lots"
		};

		table.add_partial_separator(vec![1]);

		// total just shows lot count
		table.add_row(vec![
			"",
			&format!("{} {}", self.lots.len(), bottom_line),
			"",
			"",
			"",
			"",
			"",
		]);
		table.print();
	}

	/// Prints a realized gain/loss report.
	pub fn print_realized_gain_loss(&self, begin: &Date, end: &Date) {
		if self.lots.is_empty() {
			println!("No applicable lots");
			return;
		}

		let mut table = Table::new(10);
		table.right_align(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

		table.add_header(vec![
			"ID",
			"Opened",
			"Closed",
			"Held",
			"Asset",
			"Qty",
			"Cost",
			"Proceeds",
			"Unit G/L",
			"Total G/L",
		]);

		table.add_separator();

		// currency -> total g/l
		let mut totals: HashMap<String, Amount> = HashMap::new();
		let mut has_any_unknown_gl = false;

		for l in &self.lots {
			for s in &l.sales {
				if &s.date < begin || &s.date > end {
					continue;
				}

				let cb = l.commodity.cost_basis();

				let (pr, unit_gl, total_gl) = if let Some(pr) = &s.unit_proceeds
				{
					if cb.currency == pr.currency {
						let unit_gl =
							Amount::new(pr.value - cb.value, &cb.currency);

						let total_gl = Amount::new(
							unit_gl.value * s.quantity,
							&cb.currency,
						);

						totals
							.entry(unit_gl.clone().currency)
							.or_insert(Amount::zero(&total_gl.currency))
							.value += total_gl.value;

						(
							pr.to_string(),
							unit_gl.to_string(),
							total_gl.to_string(),
						)
					} else {
						has_any_unknown_gl = true;
						// TODO: Could reduce the unknowns by pulling currency conversions in here.
						(pr.to_string(), "UNK".to_string(), "UNK".to_string())
					}
				} else {
					has_any_unknown_gl = true;
					("UNK".to_string(), "UNK".to_string(), "UNK".to_string())
				};

				table.add_row(vec![
					&l.id,
					&l.acquisition_date.to_string(),
					&s.date.to_string(),
					&s.time_held(&l.acquisition_date).to_string(),
					l.commodity.symbol(),
					&s.quantity.to_string(),
					&l.commodity.cost_basis().to_string(),
					&pr,
					&unit_gl,
					&total_gl,
				])
			}
		}

		table.add_partial_separator(vec![9]);

		// One line of totals per currency. TODO needs to be deterministically sorted.
		for total_gl in totals.values() {
			let final_total_gl = if has_any_unknown_gl {
				"UNK".to_string()
			} else {
				total_gl.to_string()
			};

			table.add_row(vec![
				"",
				"",
				"",
				"",
				"",
				"",
				"",
				"",
				"",
				&final_total_gl,
			]);
		}

		table.print()
	}

	/// Prints an unrealized gain/loss report.
	pub fn print_unrealized_gain_loss(
		&self,
		as_of: &Date,
		exchange_rates: &ExchangeRates,
	) {
		if self.lots.is_empty() {
			println!("No applicable lots");
			return;
		}

		let mut table = Table::new(9);
		table.right_align(vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);

		table.add_header(vec![
			"ID",
			"Opened",
			"Held",
			"Asset",
			"Qty",
			"Cost",
			"Latest",
			"Unit UG/L",
			"Total UG/L",
		]);

		// TODO: Rework this heinously nested thing.
		table.add_separator();

		// currency -> total g/l
		let mut totals: HashMap<String, Amount> = HashMap::new();

		for l in &self.lots {
			let cb = l.commodity.cost_basis();
			totals
				.entry(cb.clone().currency)
				.or_insert(Amount::zero(&cb.currency))
				.value += cb.value;

			let current = match exchange_rates
				.get_latest_rate(l.commodity.symbol(), &cb.currency)
			{
				Some(r) => Amount::new(r, &cb.currency),
				None => cb.clone(),
			};

			let unit_gl = Amount::new(current.value - cb.value, &cb.currency);

			let total_gl =
				Amount::new(unit_gl.value * l.quantity, &cb.currency);

			table.add_row(vec![
				&l.id,
				&l.acquisition_date.to_string(),
				&l.time_held(as_of).to_string(),
				l.commodity.symbol(),
				&l.quantity.to_string(),
				&l.commodity.cost_basis().to_string(),
				&current.to_string(),
				&unit_gl.to_string(),
				&total_gl.to_string(),
			])
		}

		table.add_partial_separator(vec![8]);

		// One line of totals per currency. TODO needs to be deterministically sorted.
		for (_, total_gl) in totals {
			table.add_row(vec![
				"",
				"",
				"",
				"",
				"",
				"",
				"",
				"",
				&total_gl.to_string(),
			]);
		}

		table.print()
	}
}

fn round_to_precision(
	value: &mut Scalar,
	symbol: &str,
	max_precision_by_currency: &HashMap<String, u32>,
	max_precision: u32,
) {
	if let Some(precision) = max_precision_by_currency.get(symbol) {
		value.round(*precision.min(&max_precision));
	} else {
		panic!("Missing symbol for lot precision");
	}
}
