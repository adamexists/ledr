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

use crate::tabulation::amount::Amount;
use crate::tabulation::entry::Entry;
use crate::tabulation::exchange_rate::ExchangeRates;
use crate::tabulation::lot::Lots;
use crate::util::date::Date;
use anyhow::{bail, Error};
use std::cmp::min;
use std::collections::{HashMap, HashSet};

/// The only valid top-level account names. This is an accounting system, after
/// all! Society has rules! Granted, there is no functional reason to have this
/// requirement other than sorting guarantees when presenting reports.
///
/// If you are reading this and want a variant of this for a language other than
/// English, email me with the right terms to use for each category, and I will
/// implement a parallel one for your language.
pub const VALID_PREFIXES: [&str; 5] =
	["Assets", "Liabilities", "Equity", "Income", "Expenses"];

#[derive(Debug)]
pub struct Ledger {
	entries: Vec<Entry>,
	/// entry currently being assembled, if any
	pending_entry: Option<Entry>,

	/// Ignore currency and account directives
	lenient_mode: bool,

	/// currency -> the earliest date currency is allowed to appear
	declared_currencies: HashMap<String, Date>,
	/// account -> the earliest date account is allowed to appear
	declared_accounts: HashMap<String, Date>,
	/// set of file paths that have been passed to this ledger, used to
	/// avoid circular includes
	included_files: HashSet<String>,

	// other modules the ledger must populate or access
	pub exchange_rates: ExchangeRates,
	pub lots: Lots,
}

impl Ledger {
	pub fn new(lenient: bool) -> Self {
		Self {
			entries: vec![],
			pending_entry: None,
			lenient_mode: lenient,
			declared_currencies: Default::default(),
			declared_accounts: Default::default(),
			included_files: Default::default(),
			exchange_rates: Default::default(),
			lots: Default::default(),
		}
	}

	// -----------
	// -- INPUT --
	// -----------

	pub fn declare_file(&mut self, file_path: &str) -> Result<(), Error> {
		if self.included_files.contains(file_path) {
			bail!("Circular file includes: {}", file_path)
		}
		self.included_files.insert(file_path.parse()?);
		Ok(())
	}

	pub fn declare_currency(
		&mut self,
		currency: String,
		date: Date,
	) -> Result<(), Error> {
		if self.lenient_mode {
			return Ok(());
		}

		if self.declared_currencies.contains_key(&currency) {
			bail!("Currency {} declared twice", currency)
		}

		self.declared_currencies.insert(currency.clone(), date);

		Ok(())
	}

	pub fn declare_account(
		&mut self,
		account: String,
		date: Date,
	) -> Result<(), Error> {
		if self.lenient_mode {
			return Ok(());
		}

		if self.declared_accounts.contains_key(&account) {
			bail!("Account {} declared twice", account)
		}

		self.declared_accounts.insert(account.clone(), date);

		Ok(())
	}

	pub fn new_entry(
		&mut self,
		date: Date,
		desc: String,
	) -> Result<(), Error> {
		if self.pending_entry.is_some() {
			self.finish_entry()?;
		}

		self.pending_entry = Some(Entry::new(date, desc));
		Ok(())
	}

	pub fn add_detail(
		&mut self,
		account: String,
		amount: Amount,
	) -> Result<(), Error> {
		if self.pending_entry.is_none() {
			bail!("Orphaned entry detail")
		}

		if !self.lenient_mode {
			self.check_account(&account)?;
			self.check_currency(&amount.currency)?;
			if let Some(cost_basis_currency) = amount.cost_basis() {
				self.check_currency(
					&cost_basis_currency.currency,
				)?;
			}
		}

		if account.is_empty() {
			bail!("Account is empty")
		}

		let has_valid_prefix = VALID_PREFIXES
			.iter()
			.any(|&prefix| account.starts_with(prefix));
		if !has_valid_prefix {
			bail!("Invalid account prefix: {}", account)
		}

		if let Some(cb) = amount.cost_basis() {
			// A cost basis has the authority of a declaration in
			// many ways, but in case there are multiple intraday
			// transactions that differ from each other (as day
			// traders etc. experience all the time), we must treat
			// them as inferred rates here.
			self.exchange_rates.infer(
				self.pending_entry_date(),
				amount.currency.clone(),
				cb.currency.clone(),
				cb.unit_price,
			)?;

			self.lots.add_movement(
				self.pending_entry_date(),
				account.clone(),
				amount.currency.clone(),
				amount.value,
				cb.unit_price,
				cb.currency.clone(),
			)?;
		}

		self.pending_entry
			.as_mut()
			.unwrap()
			.add_detail(account, amount)
	}

	pub fn set_virtual_detail(
		&mut self,
		account: String,
	) -> Result<(), Error> {
		if !self.lenient_mode {
			self.check_account(&account)?;
		}

		if self.pending_entry.is_none() {
			bail!("Orphaned entry detail")
		}

		self.pending_entry
			.as_mut()
			.unwrap()
			.set_virtual_detail(account)
	}

	pub fn add_reference(
		&mut self,
		reference: String,
	) -> Result<(), Error> {
		match &mut self.pending_entry {
			Some(e) => {
				e.add_reference(reference);
				Ok(())
			},
			None => bail!("Orphaned reference"),
		}
	}

	pub fn finish_entry(&mut self) -> Result<(), Error> {
		match self.pending_entry.take() {
			None => Ok(()),
			Some(mut entry) => {
				entry.finalize(&mut self.exchange_rates)?;
				self.entries.push(entry);
				Ok(())
			},
		}
	}

	fn pending_entry_date(&self) -> Date {
		match &self.pending_entry {
			Some(e) => *e.get_date(),
			None => panic!("pending_entry_date has no entry"),
		}
	}

	/// Checks whether a currency has been declared for use, and checks the
	/// pending entry to make sure the declaration date is not ahead of the
	/// pending entry where the currency appears.
	fn check_currency(&self, currency: &String) -> Result<(), Error> {
		let declaration_date =
			match self.declared_currencies.get(currency) {
				Some(d) => d,
				None => bail!(
					"Currency {} used without declaration",
					currency
				),
			};

		if self.pending_entry.as_ref().unwrap().get_date()
			< declaration_date
		{
			bail!(
				"Currency {} used prior to declaration on {}",
				currency,
				declaration_date
			)
		}

		Ok(())
	}

	/// Checks whether an account has been declared for use, and checks the
	/// pending entry to make sure the declaration date is not ahead of the
	/// pending entry where the account appears.
	fn check_account(&self, account: &String) -> Result<(), Error> {
		let declaration_date = match self.declared_accounts.get(account)
		{
			Some(d) => d,
			None => bail!(
				"Account {} used without declaration",
				account
			),
		};

		if self.pending_entry.as_ref().unwrap().get_date()
			< declaration_date
		{
			bail!(
				"Account {} used prior to declaration on {}",
				account,
				declaration_date
			)
		}

		Ok(())
	}

	// ----------------
	// -- TABULATING --
	// ----------------

	/// Converts all possible balances to the currency provided, if exchange
	/// rates are available. If a rate is not available for the given pair,
	/// then we skip. There is no graph traversal: a direct rate must have
	/// been observed.
	pub fn collapse_to(&mut self, currency: String) {
		self.entries.iter_mut().flat_map(|e| e.details()).for_each(
			|d| {
				if let Some(rate) =
					self.exchange_rates.get_latest_rate(
						d.currency(),
						currency.clone(),
					) {
					d.convert_to(&currency, rate)
				}
			},
		)
	}

	/// Finalizes the entire ledger by standardizing the visible precision
	/// of each currency, marking the ledger as finalized, and reporting
	/// totals from it.
	pub fn finalize(
		&mut self,
		max_reso_by_currency: HashMap<String, u32>,
		overall_max_reso: Option<u32>,
	) -> Result<(), Error> {
		let max_reso = overall_max_reso.unwrap_or(99);

		for entry in &mut self.entries {
			for (currency, &reso) in &max_reso_by_currency {
				entry.round_for_currency(
					currency,
					min(reso, max_reso),
				)?
			}
		}

		Ok(())
	}

	pub fn take_entries(self) -> Vec<Entry> {
		self.entries
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::util::date::Date;
	use crate::util::scalar::Scalar;

	#[test]
	fn test_ledger_initialization() {
		let ledger = Ledger::new(true);
		assert!(ledger.entries.is_empty());
		assert!(ledger.pending_entry.is_none());
		assert!(ledger.declared_currencies.is_empty());
		assert!(ledger.declared_accounts.is_empty());
		assert!(ledger.included_files.is_empty());
		assert!(ledger.lenient_mode);
	}

	#[test]
	fn test_declare_file() {
		let mut ledger = Ledger::new(false);
		assert!(ledger.declare_file("path/to/file").is_ok());
		assert!(ledger.included_files.contains("path/to/file"));
		assert!(ledger.declare_file("path/to/file").is_err());
	}

	#[test]
	fn test_declare_currency() {
		let mut ledger = Ledger::new(false);
		let date = Date::new(2024, 1, 1);

		assert!(ledger
			.declare_currency("USD".to_string(), date)
			.is_ok());
		assert!(ledger.declared_currencies.contains_key("USD"));

		assert!(ledger
			.declare_currency("USD".to_string(), date)
			.is_err());
	}

	#[test]
	fn test_declare_account() {
		let mut ledger = Ledger::new(false);
		let date = Date::new(2024, 1, 1);

		assert!(ledger
			.declare_account("Assets:Cash".to_string(), date)
			.is_ok());
		assert!(ledger.declared_accounts.contains_key("Assets:Cash"));

		assert!(ledger
			.declare_account("Assets:Cash".to_string(), date)
			.is_err());
	}

	#[test]
	fn test_new_entry() {
		let mut ledger = Ledger::new(false);
		let date = Date::new(2024, 1, 1);

		assert!(ledger
			.new_entry(date, "Test Entry".to_string())
			.is_ok());
		assert!(ledger.pending_entry.is_some());
		assert_eq!(ledger.pending_entry_date(), date);
	}

	#[test]
	fn test_add_detail_valid() {
		let mut ledger = Ledger::new(false);
		let date = Date::new(2024, 1, 1);
		ledger.declare_currency("USD".to_string(), date).unwrap();
		ledger.declare_account("Assets:Cash".to_string(), date)
			.unwrap();

		ledger.new_entry(date, "Test Entry".to_string()).unwrap();

		assert!(ledger
			.add_detail(
				"Assets:Cash".to_string(),
				Amount::new(
					Scalar::new(1000, 1),
					"USD".to_string(),
				)
			)
			.is_ok());
	}

	#[test]
	fn test_add_detail_invalid_currency() {
		let mut ledger = Ledger::new(false);
		let date = Date::new(2024, 1, 1);
		ledger.declare_account("Assets:Cash".to_string(), date)
			.unwrap();

		ledger.new_entry(date, "Test Entry".to_string()).unwrap();

		assert!(ledger
			.add_detail(
				"Assets:Cash".to_string(),
				Amount::new(
					Scalar::new(1000, 1),
					"EUR".to_string(),
				)
			)
			.is_err());
	}

	#[test]
	fn test_add_detail_invalid_account() {
		let mut ledger = Ledger::new(false);
		let date = Date::new(2024, 1, 1);
		ledger.declare_currency("USD".to_string(), date).unwrap();

		ledger.new_entry(date, "Test Entry".to_string()).unwrap();

		assert!(ledger
			.add_detail(
				"Liabilities:Loan".to_string(),
				Amount::new(
					Scalar::new(1000, 1),
					"USD".to_string(),
				)
			)
			.is_err());
	}

	#[test]
	fn test_add_detail_orphaned_entry() {
		let mut ledger = Ledger::new(false);

		assert!(ledger
			.add_detail(
				"Assets:Cash".to_string(),
				Amount::new(
					Scalar::new(1000, 1),
					"USD".to_string(),
				)
			)
			.is_err());
	}

	#[test]
	fn test_finish_entry() {
		let mut ledger = Ledger::new(false);
		let date = Date::new(2024, 1, 1);
		ledger.new_entry(date, "Test Entry".to_string()).unwrap();
		assert!(ledger.finish_entry().is_ok());
		assert!(ledger.pending_entry.is_none());
		assert_eq!(ledger.entries.len(), 1);
	}

	#[test]
	fn test_check_currency_before_declaration() {
		let mut ledger = Ledger::new(false);
		let date = Date::new(2024, 1, 1);
		ledger.declare_currency(
			"USD".to_string(),
			Date::new(2024, 1, 2),
		)
		.unwrap();

		ledger.new_entry(date, "Test Entry".to_string()).unwrap();
		let result = ledger.check_currency(&"USD".to_string());

		assert!(result.is_err());
	}

	#[test]
	fn test_check_account_before_declaration() {
		let mut ledger = Ledger::new(false);
		let date = Date::new(2024, 1, 1);
		ledger.declare_account(
			"Assets:Cash".to_string(),
			Date::new(2024, 1, 2),
		)
		.unwrap();

		ledger.new_entry(date, "Test Entry".to_string()).unwrap();
		let result = ledger.check_account(&"Assets:Cash".to_string());

		assert!(result.is_err());
	}

	#[test]
	fn test_add_detail_without_currency_declaration_in_lenient_mode() {
		let mut ledger = Ledger::new(true);
		let date = Date::new(2024, 1, 1);

		ledger.new_entry(date, "Lenient Test Entry".to_string())
			.unwrap();

		assert!(ledger
			.add_detail(
				"Assets:Cash".to_string(),
				Amount::new(
					Scalar::new(500, 1),
					"EUR".to_string(),
				)
			)
			.is_ok());
	}

	#[test]
	fn test_add_detail_without_account_declaration_in_lenient_mode() {
		let mut ledger = Ledger::new(true);
		let date = Date::new(2024, 1, 1);

		ledger.new_entry(date, "Lenient Test Entry".to_string())
			.unwrap();

		assert!(ledger
			.add_detail(
				"Liabilities:Loan".to_string(),
				Amount::new(
					Scalar::new(1000, 1),
					"USD".to_string(),
				)
			)
			.is_ok());
	}

	#[test]
	fn test_set_virtual_detail_without_account_declaration_in_lenient_mode()
	{
		let mut ledger = Ledger::new(true);
		let date = Date::new(2024, 1, 1);

		ledger.new_entry(
			date,
			"Lenient Virtual Detail Test".to_string(),
		)
		.unwrap();

		assert!(ledger
			.set_virtual_detail("Equity:OpeningBalance".to_string())
			.is_ok());
	}
}
