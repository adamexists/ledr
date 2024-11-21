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
use crate::gl::exchange_rate::ExchangeRates;
use crate::util::amount::Amount;
use crate::util::date::Date;
use crate::util::scalar::Scalar;
use anyhow::{bail, Error};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::string::ToString;

pub(crate) const VIRTUAL_CONVERSION_ACCOUNT: &str = "Equity:Conversions";

#[derive(Debug)]
pub struct Entry {
	date: Date,
	desc: String,
	details: Vec<Detail>,

	virtual_detail: Option<String>,
	totals: HashMap<String, Scalar>, // Currency -> Amount
	reference: Option<String>,       // optional string, not inspected
}

impl Entry {
	pub fn new(date: Date, desc: String) -> Self {
		Self {
			date,
			desc,
			details: vec![],
			virtual_detail: None,
			totals: HashMap::new(),
			reference: None,
		}
	}

	pub fn add_detail(
		&mut self,
		account: &str,
		amount: Amount,
	) -> Result<(), Error> {
		if account.is_empty() {
			bail!("Account is empty")
		}

		*self.totals
			.entry(amount.currency.clone())
			.or_insert(Scalar::zero()) += amount.value;

		self.details.push(Detail::new(account.to_owned(), amount));

		Ok(())
	}

	pub fn set_virtual_detail(
		&mut self,
		account: String,
	) -> Result<(), Error> {
		if self.virtual_detail.is_some() {
			bail!("Only one line per entry may omit amount and currency")
		}

		if account.is_empty() {
			bail!("Account is empty")
		}

		self.virtual_detail = Some(account);
		Ok(())
	}

	/// Adds a reference to the entry, to be used for reference purposes.
	/// The system is guaranteed not to analyze it or use it for any reason
	/// or purpose. Some reports and queries will display it, however.
	///
	/// If a note is already present, it appends the two, separated by one
	/// newline character.
	pub fn add_reference(&mut self, reference: String) {
		match &mut self.reference {
			Some(existing_note) => {
				existing_note.push('\n');
				existing_note.push_str(reference.trim());
			},
			None => {
				self.reference =
					Some(reference.trim().to_string());
			},
		}
	}

	pub fn get_desc(&self) -> &String {
		&self.desc
	}

	pub fn get_date(&self) -> Date {
		self.date
	}

	pub fn details(&mut self) -> &mut Vec<Detail> {
		&mut self.details
	}

	pub fn take_details(self) -> Vec<Detail> {
		self.details
	}

	/// Returns the net amount from this entry on the given account, i.e.
	/// the sum of all detail lines related to the account, for a given
	/// currency. Currency match must be exact. Account argument can be
	/// any substring.
	pub fn net_for_account(
		&self,
		account: &String,
		currency: &String,
	) -> Scalar {
		self.details.iter().fold(Scalar::zero(), |mut acc, x| {
			if x.account.contains(account)
				&& x.amount.currency == *currency
			{
				acc += x.amount.value
			}
			acc
		})
	}

	/// Rounds all Details for a certain currency to a certain resolution.
	/// In doing so, precision may be lost, but not gained (because the
	/// extra decimal places will just fill in with zeroes). This is more
	/// about the clean display of currency amounts for reporting.
	pub fn round_for_currency(
		&mut self,
		currency: &String,
		resolution: u32,
	) -> Result<(), Error> {
		for detail in &mut self.details {
			if detail.amount.currency == *currency {
				detail.amount.value.round(resolution)
			}
		}

		Ok(())
	}

	/// Completes an entry. We have to pass the exchange rate set in here,
	/// because this is where exchange rates are inferred in some cases,
	/// i.e. if exactly two currencies are imbalanced.
	///
	/// This method also handles the resolution of cost bases and their
	/// related syntactical magic, particularly decomposing it such that it
	/// has the appropriate effect on balances for the currency it was
	/// exchanged with.
	pub fn finalize(
		&mut self,
		rates: &mut ExchangeRates,
	) -> Result<(), Error> {
		let mut imbalances = self.get_imbalances();

		// Special case if exactly two currencies are unbalanced with no
		// virtual account, in which case we net them against each other
		if imbalances.len() == 2 && self.virtual_detail.is_none() {
			self.multiline_implicit_currency_conversion(
				&mut imbalances,
				rates,
			)?;
			return Ok(());
		}

		// If a virtual detail exists, it can absorb all imbalances.
		// Otherwise, if any remain, we fail the entry as unbalanced.
		while let Some((currency, value)) = imbalances.pop() {
			if let Some(vd) = &self.virtual_detail {
				self.details.push(Detail::new(
					vd.clone(),
					Amount::new(-value, currency),
				));
			} else {
				bail!("Unbalanced entry")
			}
		}

		Ok(())
	}

	/// This is a special case in which there is no virtual detail, but
	/// there are exactly two lines that we can net against each other if
	/// they are cardinally opposed.
	fn multiline_implicit_currency_conversion(
		&mut self,
		imbalances: &mut Vec<(String, Scalar)>,
		rates: &mut ExchangeRates,
	) -> Result<(), Error> {
		let (currency1, amount1) = imbalances.remove(0);
		let (currency2, amount2) = imbalances.remove(0);

		if (amount1 < 0 && amount2 < 0) || (amount1 > 0 && amount2 > 0)
		{
			bail!("Unbalanced entry")
		}

		self.details.push(Detail::new(
			VIRTUAL_CONVERSION_ACCOUNT.to_string(),
			Amount::new(-amount1, currency1.clone()),
		));
		self.details.push(Detail::new(
			VIRTUAL_CONVERSION_ACCOUNT.to_string(),
			Amount::new(-amount2, currency2.clone()),
		));

		// This implies an exchange rate between the currencies.
		// We use a lame method here to make the underlying integer
		// division nicer.
		if amount1.abs() > amount2.abs() {
			rates.infer(
				self.date,
				&currency2,
				&currency1,
				(amount1 / amount2).abs(),
			)?;
		} else {
			rates.infer(
				self.date,
				&currency1,
				&currency2,
				(amount2 / amount1).abs(),
			)?;
		}

		Ok(())
	}

	/// Find all currencies that don't sum to zero, with amounts
	fn get_imbalances(&self) -> Vec<(String, Scalar)> {
		// Collect the retained elements into a Vec
		self.totals
			.iter()
			.filter_map(|(k, &v)| {
				if v != 0 {
					Some((k.clone(), v))
				} else {
					None
				}
			})
			.collect()
	}
}

impl PartialEq for Entry {
	fn eq(&self, other: &Self) -> bool {
		self.date == other.date && self.desc == other.desc
	}
}

impl Eq for Entry {}

impl PartialOrd for Entry {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for Entry {
	// Sort by date, then desc, both ascending
	fn cmp(&self, other: &Self) -> Ordering {
		match self.date.cmp(&other.date) {
			Ordering::Equal => self.desc.cmp(&other.desc),
			other => other,
		}
	}
}

#[derive(Clone, Debug)]
pub struct Detail {
	account: String,
	amount: Amount,
}

impl Detail {
	pub fn new(account: String, amount: Amount) -> Self {
		Self { account, amount }
	}

	pub fn account(&self) -> &String {
		&self.account
	}

	pub fn currency(&self) -> String {
		self.amount.currency.clone()
	}

	pub fn amount(&self) -> &Amount {
		&self.amount
	}

	pub fn convert_to(&mut self, currency: &str, rate: Scalar) {
		self.amount.convert_to(currency, rate);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::gl::exchange_rate::ExchangeRates;
	use crate::util::date::Date;
	use crate::util::scalar::Scalar;

	// Helper function to create a sample Date for testing
	fn sample_date(offset: u8) -> Date {
		Date::new(2024, 1, 1 + offset)
	}

	// Helper function to set up an Entry with a date and description
	fn create_entry(offset: u8) -> Entry {
		Entry::new(sample_date(offset), "Sample Entry".to_string())
	}

	#[test]
	fn test_entry_creation() {
		let entry = create_entry(0);
		assert_eq!(entry.get_date(), sample_date(0));
		assert!(entry.details.is_empty());
	}

	#[test]
	fn test_add_detail() {
		let mut entry = create_entry(0);
		let result = entry.add_detail(
			&"Assets:Cash".to_string(),
			Amount::new(Scalar::new(1000, 1), "USD".to_string()),
		);

		assert!(result.is_ok());
		assert_eq!(entry.details.len(), 1);

		let detail = &entry.details[0];
		assert_eq!(detail.account, "Assets:Cash");
		assert_eq!(detail.amount.value, Scalar::new(1000, 1));
		assert_eq!(detail.amount.currency, "USD");
	}

	#[test]
	fn test_add_detail_empty_account() {
		let mut entry = create_entry(0);
		let result = entry.add_detail(
			&"".to_string(),
			Amount::new(Scalar::new(1000, 1), "USD".to_string()),
		);

		assert!(result.is_err());
	}

	#[test]
	fn test_add_detail_multiple_same_currency() {
		let mut entry = create_entry(0);
		entry.add_detail(
			&"Assets:Cash".to_string(),
			Amount::new(Scalar::new(1000, 1), "USD".to_string()),
		)
		.unwrap();
		entry.add_detail(
			&"Assets:Savings".to_string(),
			Amount::new(Scalar::new(500, 1), "USD".to_string()),
		)
		.unwrap();

		assert_eq!(
			entry.totals.get("USD"),
			Some(&Scalar::new(1500, 1))
		);
	}

	#[test]
	fn test_finalize_unbalanced_entry() {
		let mut entry = create_entry(0);
		entry.add_detail(
			&"Assets:Cash".to_string(),
			Amount::new(Scalar::new(1000, 1), "USD".to_string()),
		)
		.unwrap();
		entry.add_detail(
			&"Expenses:Food".to_string(),
			Amount::new(Scalar::new(-500, 1), "USD".to_string()),
		)
		.unwrap();

		let mut rates = ExchangeRates::default();
		let result = entry.finalize(&mut rates);

		// Expect an error since the entry is unbalanced
		assert!(result.is_err());
	}

	#[test]
	fn test_finalize_balanced_entry() {
		let mut entry = create_entry(0);
		entry.add_detail(
			&"Assets:Cash".to_string(),
			Amount::new(Scalar::new(1000, 1), "USD".to_string()),
		)
		.unwrap();
		entry.add_detail(
			&"Expenses:Food".to_string(),
			Amount::new(Scalar::new(-1000, 1), "USD".to_string()),
		)
		.unwrap();

		let mut rates = ExchangeRates::default();
		let result = entry.finalize(&mut rates);

		// Expect success since the entry is balanced
		assert!(result.is_ok());
	}

	#[test]
	fn test_set_virtual_detail() {
		let mut entry = create_entry(0);
		let result =
			entry.set_virtual_detail("Assets:Virtual".to_string());

		assert!(result.is_ok());
		assert!(entry.virtual_detail.is_some());
		assert_eq!(entry.virtual_detail.unwrap(), "Assets:Virtual")
	}

	#[test]
	fn test_set_virtual_detail_twice() {
		let mut entry = create_entry(0);
		entry.set_virtual_detail("Assets:Virtual".to_string())
			.unwrap();
		let result =
			entry.set_virtual_detail("Assets:Another".to_string());

		assert!(result.is_err());
	}

	#[test]
	fn test_set_virtual_detail_empty_account() {
		let mut entry = create_entry(0);
		let result = entry.set_virtual_detail("".to_string());

		assert!(result.is_err());
	}

	#[test]
	fn test_set_resolution_for_currency() {
		let mut entry = create_entry(0);
		entry.add_detail(
			&"Assets:Cash".to_string(),
			Amount::new(Scalar::new(1234567, 4), "USD".to_string()),
		)
		.unwrap();

		let result = entry.round_for_currency(&"USD".to_string(), 2);
		assert!(result.is_ok());

		let detail = &entry.details[0];
		assert_eq!(detail.amount.value.amount(), 12346); // rounding check
		assert_eq!(detail.amount.value.resolution(), 2);
	}

	#[test]
	fn test_set_resolution_for_currency_different_currency() {
		let mut entry = create_entry(0);
		entry.add_detail(
			&"Assets:Cash".to_string(),
			Amount::new(Scalar::new(1234567, 4), "USD".to_string()),
		)
		.unwrap();
		entry.add_detail(
			&"Assets:Cash".to_string(),
			Amount::new(Scalar::new(9876543, 4), "EUR".to_string()),
		)
		.unwrap();

		let result = entry.round_for_currency(&"USD".to_string(), 2);
		assert!(result.is_ok());

		let usd_detail = &entry.details[0];
		assert_eq!(usd_detail.amount.value.amount(), 12346);
		assert_eq!(usd_detail.amount.value.resolution(), 2);

		let eur_detail = &entry.details[1];
		assert_eq!(eur_detail.amount.value.amount(), 9876543);
		assert_eq!(eur_detail.amount.value.resolution(), 4);
	}

	#[test]
	fn test_multiline_implicit_currency_conversion() {
		let mut entry = create_entry(0);
		entry.add_detail(
			&"Assets:Cash".to_string(),
			Amount::new(Scalar::new(1000, 1), "USD".to_string()),
		)
		.unwrap();
		entry.add_detail(
			&"Assets:Bank".to_string(),
			Amount::new(Scalar::new(-2000, 1), "EUR".to_string()),
		)
		.unwrap();

		let mut rates = ExchangeRates::default();
		let mut imbalances = entry.get_imbalances();
		let result = entry.multiline_implicit_currency_conversion(
			&mut imbalances,
			&mut rates,
		);

		assert!(result.is_ok());
		assert_eq!(entry.details.len(), 4);
	}

	#[test]
	fn test_multiline_implicit_currency_conversion_error() {
		let mut entry = create_entry(0);
		entry.add_detail(
			&"Assets:Cash".to_string(),
			Amount::new(Scalar::new(1000, 1), "USD".to_string()),
		)
		.unwrap();
		entry.add_detail(
			&"Assets:Bank".to_string(),
			Amount::new(Scalar::new(2000, 1), "EUR".to_string()),
		)
		.unwrap(); // Both amounts are positive

		let mut rates = ExchangeRates::default();
		let mut imbalances = entry.get_imbalances();
		let result = entry.multiline_implicit_currency_conversion(
			&mut imbalances,
			&mut rates,
		);

		// Both imbalances are in the same direction, which is bad
		assert!(result.is_err());
	}
}
