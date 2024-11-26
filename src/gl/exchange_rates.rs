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

use crate::gl::observed_rate::ObservedRate;
use crate::util::amount::Amount;
use crate::util::date::Date;
use crate::util::graph::Graph;
use crate::util::quant::Quant;
use anyhow::{bail, Error};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct ExchangeRates {
	/// Stores a set of Graphs, one per date
	daily_graphs: BTreeMap<Date, Graph>,

	primary_graph: Graph,

	/// Preprocessed data for constant-time lookups, only available after
	/// finalize() has been called on this.
	resolved_rates: BTreeMap<(String, String), Vec<ObservedRate>>,
}

impl ExchangeRates {
	pub fn new() -> Self {
		Self {
			daily_graphs: Default::default(),
			resolved_rates: Default::default(),
			primary_graph: Default::default(),
		}
	}

	/// Adds a new exchange rate declared via directive. Might fail if
	/// there's already a declared rate on the same date, or if the input
	/// is incoherent.
	pub fn declare(
		&mut self,
		date: Date,
		base: String,
		quote: String,
		rate: Quant,
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

		let b_amt = Amount::new(Quant::from_i128(1), &base);
		let q_amt = Amount::new(rate, &quote);

		// We do not need to check for existing inferred rates, because
		// all directives are handled first, so one cannot exist.

		let entry = self.daily_graphs.entry(date).or_default();

		if entry.get_direct_rate(&base, &quote, true).is_some() {
			bail!("Cannot declare multiple rates on same date")
		}

		entry.add_rate(&date, &b_amt, &q_amt, false)?;

		self.primary_graph
			.overwrite_rate_if_newer(&date, &b_amt, &q_amt, false)?;

		Ok(())
	}

	/// Adds a new exchange rate inferred from an entry. Might fail if there
	/// is already a declared rate that is outside tolerance from this new
	/// rate. If there is already a declared rate at all, this one will
	/// definitely be ignored.
	pub fn infer(
		&mut self,
		date: Date,
		base: &String,
		quote: &String,
		rate: Quant,
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

		let b_amt = Amount::new(Quant::from_i128(1), base);
		let q_amt = Amount::new(rate, quote);

		// We do not need to check for existing inferred rates, because
		// all directives are handled first, so one cannot exist.

		let entry = self.daily_graphs.entry(date).or_default();

		if let Some(existing_rate) = entry.get_direct_rate(base, quote, true) {
			// Check if the inferred rate is within 1% of the
			// declared rate. If it is, ignore this inferred rate
			// and use the declared; if not, then the declared rate
			// is too far from reality on this date to be accurate,
			// so we should error to stop tabulation here.
			if !within_tolerance_of(Quant::new(1, 2), existing_rate, rate) {
				bail!("Inferred exchange rate deviates >1% from declared rate")
			}

			return Ok(());
		}

		entry.add_rate(&date, &b_amt, &q_amt, true)?;

		self.primary_graph
			.overwrite_rate_if_newer(&date, &b_amt, &q_amt, true)?;

		Ok(())
	}

	pub fn infer_from_equal_amounts(
		&mut self,
		date: Date,
		a: Amount,
		b: Amount,
	) -> Result<(), Error> {
		let entry = self.daily_graphs.entry(date).or_default();

		// TODO: Sweep this whole project for nondeterministic things.

		if let Some(existing_rate) =
			entry.get_direct_rate(&a.currency, &b.currency, true)
		{
			let rate = b.value / a.value;
			// Check if the inferred rate is within 1% of the
			// declared rate. If it is, ignore this inferred rate
			// and use the declared; if not, then the declared rate
			// is too far from reality on this date to be accurate,
			// so we should error to stop tabulation here.
			if !within_tolerance_of(Quant::new(1, 2), existing_rate, rate) {
				bail!("Inferred exchange rate deviates >1% from declared rate")
			}

			return Ok(());
		}

		entry.add_rate(&date, &a, &b, true)?;

		self.primary_graph
			.overwrite_rate_if_newer(&date, &a, &b, true)?;

		Ok(())
	}

	/// Finalizes the rates into a resolved form for efficient lookups,
	/// after which the methods to retrieve rates from here will work.
	/// Prior to that, they will not work. Finalization can fail if any
	/// of the underlying Graphs are incoherent.
	pub fn finalize(
		&mut self,
		max_precision_by_currency: &BTreeMap<String, u32>,
		emit_warnings: bool,
	) -> Result<(), Error> {
		let mut resolved = BTreeMap::new();

		for (date, graph) in &self.daily_graphs {
			if emit_warnings && graph.has_inconsistent_cycle() {
				println!("Warning: currency conversion graph on {} is not internally consistent\n", date);
			}

			// TODO: Clarify: We still need the daily graphs because of some
			//  reports' need to conjure a historical exchange rate as of a
			//  given day. Just... think about it when I'm not so tired!

			// Make sure exchange rates inherit desired precision from user
			for (base, quote, mut observation) in graph.get_all_rates() {
				match (
					max_precision_by_currency.get(&base),
					max_precision_by_currency.get(&quote),
				) {
					(Some(bmp), Some(qmp)) => observation
						.rate
						.set_render_precision(*bmp.max(qmp), true),
					(Some(bmp), None) => {
						observation.rate.set_render_precision(*bmp, true)
					},
					(None, Some(qmp)) => {
						observation.rate.set_render_precision(*qmp, true)
					},
					(None, None) => {},
				}

				// Attach date to these observations, because the underlying
				// graph has no way to know it TODO that could be improved
				observation.date = Some(*date);

				resolved
					.entry((base.clone(), quote.clone()))
					.or_insert_with(Vec::new)
					.push(observation);
			}
		}

		// Sort rates for each currency pair by date in descending order
		for rates in resolved.values_mut() {
			rates.sort_by(|a, b| {
				b.date.cmp(&a.date).then_with(|| a.rate.cmp(&b.rate))
			});
		}

		for (base, quote, mut observation) in self.primary_graph.get_all_rates()
		{
			match (
				max_precision_by_currency.get(&base),
				max_precision_by_currency.get(&quote),
			) {
				(Some(bmp), Some(qmp)) => {
					observation.rate.set_render_precision(*bmp.max(qmp), false)
				},
				(Some(bmp), None) => {
					observation.rate.set_render_precision(*bmp, false)
				},
				(None, Some(qmp)) => {
					observation.rate.set_render_precision(*qmp, false)
				},
				(None, None) => {},
			}

			// Do not use the primary graph, which goes across dates, unless
			// there is no other rate for a pair.
			if resolved.contains_key(&(base.clone(), quote.clone())) {
				continue;
			}

			// We set date to max here just so the rates will be used last when sorted,
			// and as a hacky signal to certain reports that they are not from a specific
			// date.
			resolved
				.entry((base.clone(), quote.clone()))
				.or_insert_with(Vec::new)
				.push(observation);
		}

		self.resolved_rates = resolved;

		Ok(())
	}

	/// Retrieves the most recent rate, if any, at or before the given date
	pub fn get_rate_as_of(
		&self,
		base: &str,
		quote: &str,
		as_of: &Date,
	) -> Option<Quant> {
		// TODO: Refactor this expression.
		self.resolved_rates
			.get(&(base.to_string(), quote.to_string()))
			.and_then(|rates| {
				rates
					.iter()
					.find(|o| {
						o.date.is_none() || o.date.as_ref().unwrap() <= as_of
					})
					.map(|o| o.rate)
			})
	}

	/// Retrieves the most recent rate available, if any
	pub fn get_latest_rate(&self, base: &str, quote: &str) -> Option<Quant> {
		self.resolved_rates
			.get(&(base.to_string(), quote.to_string()))
			.and_then(|rates| rates.first().map(|o| o.rate))
	}

	/// Returns the final map of resolved rates. Consumes this.
	pub fn take_all_rates(
		self,
	) -> BTreeMap<(String, String), Vec<ObservedRate>> {
		self.resolved_rates
	}
}

/// Returns true iff a and b are within the given tolerance of each other.
/// The given tolerance should be in the form of a percent, i.e. 1% == 0.01.
fn within_tolerance_of(tolerance: Quant, a: Quant, b: Quant) -> bool {
	(a - b).abs() <= tolerance * a.abs().max(b.abs())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::util::date::Date;
	use crate::util::quant::Quant;

	fn setup_exchange_rates() -> ExchangeRates {
		ExchangeRates::default()
	}

	#[test]
	fn test_declare_valid_rate() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::from_str("2024-1-1").unwrap();
		let base = "USD".to_string();
		let quote = "EUR".to_string();
		let rate = Quant::new(11, 1);

		assert!(exchange_rates
			.declare(date, base.clone(), quote.clone(), rate)
			.is_ok());

		let date2 = Date::from_str("2024-11-2").unwrap();
		assert!(exchange_rates
			.declare(date2, base, quote, Quant::new(12, 1))
			.is_ok());
	}

	#[test]
	fn test_declare_self_exchange() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::from_str("2024-1-1").unwrap();
		let base = "USD".to_string();
		let rate = Quant::new(11, 1);

		assert!(exchange_rates
			.declare(date, base.clone(), base.clone(), rate)
			.is_err());

		let date2 = Date::from_str("2024-11-2").unwrap();
		assert!(exchange_rates
			.declare(date2, base.clone(), base, Quant::new(9, 1))
			.is_err());
	}

	#[test]
	fn test_declare_non_positive_rate() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::from_str("2024-11-01").unwrap();
		let base = "USD".to_string();
		let quote = "EUR".to_string();

		assert!(exchange_rates
			.declare(date, base.clone(), quote.clone(), Quant::new(0, 0))
			.is_ok());
		assert!(exchange_rates
			.declare(date, base, quote, Quant::new(-1, 1))
			.is_ok());
	}

	#[test]
	fn test_infer_rate_within_tolerance() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::from_str("2024-11-01").unwrap();
		let base = "USD".to_string();
		let quote = "EUR".to_string();
		let declared_rate = Quant::new(11, 1);

		exchange_rates
			.declare(date, base.clone(), quote.clone(), declared_rate)
			.unwrap();

		let inferred_rate = Quant::new(1099, 3);
		assert!(exchange_rates
			.infer(date, &base, &quote, inferred_rate)
			.is_ok());

		let date2 = Date::from_str("2024-11-02").unwrap();
		assert!(exchange_rates
			.infer(date2, &base, &quote, Quant::new(111, 2))
			.is_ok());
	}

	#[test]
	fn test_infer_rate_outside_tolerance() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::from_str("2024-11-1").unwrap();
		let base = "USD".to_string();
		let quote = "EUR".to_string();
		let declared_rate = Quant::new(11, 1);

		exchange_rates
			.declare(date, base.clone(), quote.clone(), declared_rate)
			.unwrap();

		let inferred_rate = Quant::new(112, 2);
		assert!(exchange_rates
			.infer(date, &base, &quote, inferred_rate)
			.is_err());

		assert!(exchange_rates
			.infer(date, &base, &quote, Quant::new(97, 2))
			.is_err());
	}
}
