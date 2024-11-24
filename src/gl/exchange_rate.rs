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

use crate::util::amount::Amount;
use crate::util::date::Date;
use crate::util::graph::Graph;
use crate::util::scalar::Scalar;
use anyhow::{bail, Error};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct ExchangeRates {
	/// Stores a set of Graphs, one per date
	rate_graphs: HashMap<Date, Graph>,

	/// Preprocessed data for constant-time lookups, only available after
	/// finalize() has been called on this
	resolved_rates: HashMap<(String, String), Vec<(Date, Scalar)>>,
}

impl ExchangeRates {
	/// Adds a new exchange rate declared via directive. Might fail if
	/// there's already a declared rate on the same date, or if the input
	/// is incoherent.
	pub fn declare(
		&mut self,
		date: Date,
		base: String,
		quote: String,
		rate: Scalar,
	) -> Result<(), Error> {
		if base == quote {
			bail!("Cannot exchange a currency for itself")
		}

		// Rates of 0 or Inf are conceptually permissible, but we can't
		// work with it. Instead, avoid putting in such a rate so we
		// never try to convert to or from something worthless.
		if rate == 0 {
			return Ok(());
		}

		let b_amt = Amount::new(Scalar::from_i128(1), &base);
		let q_amt = Amount::new(rate, &quote);

		// We do not need to check for existing inferred rates, because
		// all directives are handled first, so one cannot exist.

		let entry = self.rate_graphs.entry(date).or_insert_with(Graph::new);

		if entry.get_direct_rate(&base, &quote, true).is_some() {
			bail!("Cannot declare multiple rates on same date")
		}

		entry.add_rate(&b_amt, &q_amt, false)?;

		Ok(())
	}

	/// Adds a new exchange rate inferred from an entry. Might fail if there
	/// is an existing declared rate that is outside tolerance from this new
	/// rate. If there is an existing declared rate at all, this one will
	/// definitely be ignored.
	pub fn infer(
		&mut self,
		date: Date,
		base: &String,
		quote: &String,
		rate: Scalar,
	) -> Result<(), Error> {
		if base == quote {
			bail!("Cannot exchange a currency for itself")
		}

		// Rates of 0 or Inf are conceptually permissible, but we can't
		// work with it. Instead, avoid putting in such a rate so we
		// never try to convert to or from something worthless.
		if rate == 0 {
			return Ok(());
		}

		let b_amt = Amount::new(Scalar::from_i128(1), base);
		let q_amt = Amount::new(rate, quote);

		// We do not need to check for existing inferred rates, because
		// all directives are handled first, so one cannot exist.

		let entry = self.rate_graphs.entry(date).or_insert_with(Graph::new);

		if let Some(existing_rate) = entry.get_direct_rate(base, quote, true) {
			// Check if the inferred rate is within 1% of the
			// declared rate. If it is, ignore this inferred rate
			// and use the declared; if not, then the declared rate
			// is too far from reality on this date to be accurate,
			// so we should error to stop tabulation here.
			if !within_tolerance_of(Scalar::new(1, 2), existing_rate, rate) {
				bail!("Inferred exchange rate deviates >1% from declared rate")
			}

			return Ok(());
		}

		entry.add_rate(&b_amt, &q_amt, true)?;

		Ok(())
	}

	pub fn infer_equal_amts(
		&mut self,
		date: Date,
		a: Amount,
		b: Amount,
	) -> Result<(), Error> {
		let entry = self.rate_graphs.entry(date).or_insert_with(Graph::new);

		if let Some(existing_rate) =
			entry.get_direct_rate(&a.currency, &b.currency, true)
		{
			let rate = b.value / a.value;
			// Check if the inferred rate is within 1% of the
			// declared rate. If it is, ignore this inferred rate
			// and use the declared; if not, then the declared rate
			// is too far from reality on this date to be accurate,
			// so we should error to stop tabulation here.
			if !within_tolerance_of(Scalar::new(1, 2), existing_rate, rate) {
				bail!("Inferred exchange rate deviates >1% from declared rate")
			}

			return Ok(());
		}

		entry.add_rate(&a, &b, true)?;

		Ok(())
	}

	/// Finalizes the rates into a resolved form for efficient lookups,
	/// after which the methods to retrieve rates from here will work.
	/// Prior to that, they will not work. Finalization can fail if any
	/// of the underlying Graphs are incoherent.
	///
	/// Ignores and drops all data outside the bounds defined by the
	/// relevant arguments.
	pub fn finalize(
		&mut self,
		drop_before: &Date,
		drop_after: &Date,
	) -> Result<(), Error> {
		let mut resolved = HashMap::new();

		self.rate_graphs
			.retain(|date, _| date >= drop_before && date <= drop_after);

		for (date, graph) in &self.rate_graphs {
			if graph.has_inconsistent_cycle() {
				bail!("Exchange rates on {} are incoherent", date)
			}
			for (base, quote, rate) in graph.get_all_rates() {
				resolved
					.entry((base.clone(), quote.clone()))
					.or_insert_with(Vec::new)
					.push((*date, rate));
			}
		}

		// Sort rates for each currency pair by date in descending order
		for rates in resolved.values_mut() {
			rates.sort_by(|a, b| b.0.cmp(&a.0));
		}

		self.resolved_rates = resolved;
		Ok(())
	}

	/// Retrieves the most recent rate available, if any
	pub fn get_latest_rate(&self, base: &str, quote: &str) -> Option<Scalar> {
		self.resolved_rates
			.get(&(base.to_string(), quote.to_string()))
			.and_then(|rates| rates.first().map(|(_, rate)| *rate))
	}
}

/// Returns true iff a and b are within the given tolerance of each other.
/// The given tolerance should be in the form of a percent, i.e. 1% == 0.01.
fn within_tolerance_of(tolerance: Scalar, a: Scalar, b: Scalar) -> bool {
	(a - b).abs() <= tolerance * a.abs().max(b.abs())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::util::date::Date;
	use crate::util::scalar::Scalar;

	fn setup_exchange_rates() -> ExchangeRates {
		ExchangeRates::default()
	}

	#[test]
	fn test_declare_valid_rate() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::from_str("2024-1-1").unwrap();
		let base = "USD".to_string();
		let quote = "EUR".to_string();
		let rate = Scalar::new(11, 1);

		assert!(exchange_rates
			.declare(date, base.clone(), quote.clone(), rate)
			.is_ok());

		let date2 = Date::from_str("2024-11-2").unwrap();
		assert!(exchange_rates
			.declare(date2, base, quote, Scalar::new(12, 1))
			.is_ok());
	}

	#[test]
	fn test_declare_self_exchange() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::from_str("2024-1-1").unwrap();
		let base = "USD".to_string();
		let rate = Scalar::new(11, 1);

		assert!(exchange_rates
			.declare(date, base.clone(), base.clone(), rate)
			.is_err());

		let date2 = Date::from_str("2024-11-2").unwrap();
		assert!(exchange_rates
			.declare(date2, base.clone(), base, Scalar::new(9, 1))
			.is_err());
	}

	#[test]
	fn test_declare_non_positive_rate() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::from_str("2024-11-01").unwrap();
		let base = "USD".to_string();
		let quote = "EUR".to_string();

		assert!(exchange_rates
			.declare(date, base.clone(), quote.clone(), Scalar::new(0, 0))
			.is_ok());
		assert!(exchange_rates
			.declare(date, base, quote, Scalar::new(-1, 1))
			.is_ok());
	}

	#[test]
	fn test_infer_rate_within_tolerance() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::from_str("2024-11-01").unwrap();
		let base = "USD".to_string();
		let quote = "EUR".to_string();
		let declared_rate = Scalar::new(11, 1);

		exchange_rates
			.declare(date, base.clone(), quote.clone(), declared_rate)
			.unwrap();

		let inferred_rate = Scalar::new(1099, 3);
		assert!(exchange_rates
			.infer(date, &base, &quote, inferred_rate)
			.is_ok());

		let date2 = Date::from_str("2024-11-02").unwrap();
		assert!(exchange_rates
			.infer(date2, &base, &quote, Scalar::new(111, 2))
			.is_ok());
	}

	#[test]
	fn test_infer_rate_outside_tolerance() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::from_str("2024-11-1").unwrap();
		let base = "USD".to_string();
		let quote = "EUR".to_string();
		let declared_rate = Scalar::new(11, 1);

		exchange_rates
			.declare(date, base.clone(), quote.clone(), declared_rate)
			.unwrap();

		let inferred_rate = Scalar::new(112, 2);
		assert!(exchange_rates
			.infer(date, &base, &quote, inferred_rate)
			.is_err());

		assert!(exchange_rates
			.infer(date, &base, &quote, Scalar::new(97, 2))
			.is_err());
	}

	#[test]
	fn test_get_latest_rate() {
		let mut exchange_rates = setup_exchange_rates();
		let date1 = Date::from_str("2024-11-01").unwrap();
		let date2 = Date::from_str("2024-11-02").unwrap();
		let base = "USD".to_string();
		let quote = "EUR".to_string();
		let rate1 = Scalar::new(11, 1);
		let rate2 = Scalar::new(12, 1);

		exchange_rates
			.declare(date1, base.clone(), quote.clone(), rate1)
			.unwrap();
		exchange_rates
			.declare(date2, base.clone(), quote.clone(), rate2)
			.unwrap();

		exchange_rates
			.finalize(&Date::min(), &Date::max())
			.expect("finalize failed");

		assert_eq!(exchange_rates.get_latest_rate(&base, &quote), Some(rate2));

		let date3 = Date::from_str("2024-11-3").unwrap();
		let rate3 = Scalar::new(115, 2);
		exchange_rates
			.declare(date3, base.clone(), quote.clone(), rate3)
			.unwrap();

		exchange_rates
			.finalize(&Date::min(), &Date::max())
			.expect("finalize failed");

		assert_eq!(exchange_rates.get_latest_rate(&base, &quote), Some(rate3));
	}
}
