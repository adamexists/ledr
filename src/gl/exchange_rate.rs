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

use crate::gl::exchange_rate::RateType::{Declared, Inferred};
use crate::util::date::Date;
use crate::util::scalar::Scalar;
use anyhow::{bail, Error};
use std::cmp::PartialEq;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct ExchangeRates {
	/// Stores rates with a tuple of (base, quote) as the key
	rates: HashMap<(String, String), Vec<ExchangeRate>>,
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
		mut rate: Scalar,
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

		// to standardize lookups, put base alphabetically before quote
		let key = if base < quote {
			(base, quote)
		} else {
			rate = 1 / rate;
			(quote, base)
		};

		if self.get_exact_rate(&key, date, Declared).is_some() {
			bail!("Cannot declare multiple rates on same date")
		}
		let new_rate = ExchangeRate::new(date, Declared, rate);

		// We do not need to check for existing inferred rates, because
		// all directives are handled first, so one cannot exist.

		self.rates.entry(key.clone()).or_default().push(new_rate);
		self.rates
			.entry(key)
			.and_modify(|e| e.sort_by(|a, b| b.date.cmp(&a.date)));
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
		mut rate: Scalar,
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

		// To standardize lookups, put base alphabetically before quote
		let key = if base < quote {
			(base.clone(), quote.clone())
		} else {
			rate = 1 / rate;
			(quote.clone(), base.clone())
		};

		if let Some(declared) =
			self.get_exact_rate(&key, date, Declared)
		{
			// Check if the inferred rate is within 1% of the
			// declared rate. If it is, ignore this inferred rate
			// and use the declared; if not, then the declared rate
			// is too far from reality on this date to be accurate,
			// so we should error to stop tabulation here.
			if !within_tolerance_of(
				Scalar::new(1, 2),
				declared,
				rate,
			) {
				bail!("Inferred exchange rate deviates >1% from declared rate")
			}

			return Ok(());
		}

		let new_rate = ExchangeRate::new(date, Inferred, rate);
		self.rates.entry(key.clone()).or_default().push(new_rate);
		self.rates
			.entry(key)
			.and_modify(|e| e.sort_by(|a, b| b.date.cmp(&a.date)));
		Ok(())
	}

	/// Retrieves the most recent rate before a given date, if any
	pub fn get_effective_rate_on(
		&self,
		date: Date,
		base: String,
		quote: String,
	) -> Option<Scalar> {
		let mut invert_rate = false;
		let key = if base < quote {
			(base, quote)
		} else {
			invert_rate = true;
			(quote, base)
		};

		self.rates
			.get(&key)
			.and_then(|rates| {
				rates.iter().find(|rate| rate.date <= date)
			})
			.map(|r| r.rate)
			.map(
				|found| {
					if invert_rate {
						1 / found
					} else {
						found
					}
				},
			)
	}

	/// Retrieves the most recent rate available, if any
	pub fn get_latest_rate(
		&self,
		base: String,
		quote: String,
	) -> Option<Scalar> {
		let mut invert_rate = false;
		let key = if base < quote {
			(base, quote)
		} else {
			invert_rate = true;
			(quote, base)
		};

		self.rates
			.get(&key)
			.and_then(|rates| rates.first())
			.map(|r| r.rate)
			.map(
				|found| {
					if invert_rate {
						1 / found
					} else {
						found
					}
				},
			)
	}

	/// Returns a rate that exists for the *exact* passed date, if any.
	fn get_exact_rate(
		&self,
		key: &(String, String),
		date: Date,
		rate_type: RateType,
	) -> Option<Scalar> {
		self.rates
			.get(key)
			.and_then(|rates| {
				rates.iter().find(|rate| {
					rate.date == date
						&& rate.rate_type == rate_type
				})
			})
			.map(|r| r.rate)
	}
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RateType {
	/// i.e. The user said this is true
	Declared,
	/// i.e. We inferred this rate from an entry or detail
	Inferred,
}

#[derive(Clone, Debug)]
struct ExchangeRate {
	date: Date,
	rate_type: RateType,

	rate: Scalar,
}

impl ExchangeRate {
	fn new(date: Date, rate_type: RateType, rate: Scalar) -> Self {
		Self {
			date,
			rate_type,
			rate,
		}
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
		let date = Date::new(2024, 11, 1);
		let base = "USD".to_string();
		let quote = "EUR".to_string();
		let rate = Scalar::new(11, 1);

		assert!(exchange_rates
			.declare(date, base.clone(), quote.clone(), rate)
			.is_ok());

		let date2 = Date::new(2024, 11, 2);
		assert!(exchange_rates
			.declare(date2, base, quote, Scalar::new(12, 1))
			.is_ok());
	}

	#[test]
	fn test_declare_self_exchange() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::new(2024, 11, 1);
		let base = "USD".to_string();
		let rate = Scalar::new(11, 1);

		assert!(exchange_rates
			.declare(date, base.clone(), base.clone(), rate)
			.is_err());

		let date2 = Date::new(2024, 11, 2);
		assert!(exchange_rates
			.declare(date2, base.clone(), base, Scalar::new(9, 1))
			.is_err());
	}

	#[test]
	fn test_declare_non_positive_rate() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::new(2024, 11, 1);
		let base = "USD".to_string();
		let quote = "EUR".to_string();

		assert!(exchange_rates
			.declare(
				date,
				base.clone(),
				quote.clone(),
				Scalar::new(0, 0)
			)
			.is_ok());
		assert!(exchange_rates
			.declare(date, base, quote, Scalar::new(-1, 1))
			.is_ok());
	}

	#[test]
	fn test_infer_rate_within_tolerance() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::new(2024, 11, 1);
		let base = "USD".to_string();
		let quote = "EUR".to_string();
		let declared_rate = Scalar::new(11, 1);

		exchange_rates
			.declare(
				date,
				base.clone(),
				quote.clone(),
				declared_rate,
			)
			.unwrap();

		let inferred_rate = Scalar::new(1099, 3);
		assert!(exchange_rates
			.infer(date, &base, &quote, inferred_rate)
			.is_ok());

		let date2 = Date::new(2024, 11, 2);
		assert!(exchange_rates
			.infer(date2, &base, &quote, Scalar::new(111, 2))
			.is_ok());
	}

	#[test]
	fn test_infer_rate_outside_tolerance() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::new(2024, 11, 1);
		let base = "USD".to_string();
		let quote = "EUR".to_string();
		let declared_rate = Scalar::new(11, 1);

		exchange_rates
			.declare(
				date,
				base.clone(),
				quote.clone(),
				declared_rate,
			)
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
	fn test_get_effective_rate_on() {
		let mut exchange_rates = setup_exchange_rates();
		let date = Date::new(2024, 11, 1);
		let base = "USD".to_string();
		let quote = "EUR".to_string();
		let rate = Scalar::new(11, 1);

		exchange_rates
			.declare(date, base.clone(), quote.clone(), rate)
			.unwrap();

		assert_eq!(
			exchange_rates.get_effective_rate_on(
				date,
				base.clone(),
				quote.clone()
			),
			Some(rate)
		);

		let earlier_date = Date::new(2024, 10, 31);
		assert_eq!(
			exchange_rates.get_effective_rate_on(
				earlier_date,
				base,
				quote
			),
			None
		);
	}

	#[test]
	fn test_get_latest_rate() {
		let mut exchange_rates = setup_exchange_rates();
		let date1 = Date::new(2024, 11, 1);
		let date2 = Date::new(2024, 11, 2);
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

		assert_eq!(
			exchange_rates
				.get_latest_rate(base.clone(), quote.clone()),
			Some(rate2)
		);

		let date3 = Date::new(2024, 11, 3);
		let rate3 = Scalar::new(115, 2);
		exchange_rates
			.declare(date3, base.clone(), quote.clone(), rate3)
			.unwrap();
		assert_eq!(
			exchange_rates.get_latest_rate(base, quote),
			Some(rate3)
		);
	}
}
