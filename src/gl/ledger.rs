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

use crate::gl::entry::{
	Entry, VIRTUAL_CONVERSION_ACCOUNT, VIRTUAL_ROUNDING_ERROR_ACCOUNT,
};
use crate::gl::exchange_rates::ExchangeRates;
use crate::investment::action::Action;
use crate::investment::action_buffer::ActionBuffer;
use crate::util::amount::Amount;
use crate::util::date::Date;
use anyhow::{bail, Error};
use std::cmp::min;
use std::collections::HashMap;

/// The only valid top-level account names. This is an accounting system, after
/// all! Society has rules! Granted, there is no functional reason to have this
/// requirement other than sorting guarantees when presenting reports.
///
/// If you are reading this and want a variant of this for a language other than
/// English, email me with the right terms to use for each category, and I will
/// implement a parallel one for your language.
pub const VALID_PREFIXES: [&str; 5] =
	["Assets", "Liabilities", "Equity", "Income", "Expenses"];

/// The central data structure of this system that takes input from the parser
/// and assembles it into accounting journal entries. Entries have detail lines
/// and details contain or imply various metadata, like exchange rates, lot
/// activity, and so on.
///
/// The Ledger is chiefly responsible for assembling this data according to
/// input. In general, the Ledger will pass one or more of its data sets to
/// another data structure for further refinement and reporting.
#[derive(Debug)]
pub struct Ledger {
	entries: Vec<Entry>,
	/// Entry currently being assembled, if any
	pending_entry: Option<Entry>,

	/// Skip currency and account validation steps
	lenient_mode: bool,

	/// currency -> the earliest date currency is allowed to appear
	declared_currencies: HashMap<String, Date>,
	/// account -> the earliest date account is allowed to appear
	declared_accounts: HashMap<String, Date>,

	// conceptually distinct modules the ledger must populate or access
	pub exchange_rates: ExchangeRates,
	pub lots: ActionBuffer,
}

impl Ledger {
	pub fn new(lenient: bool) -> Self {
		Self {
			entries: vec![],
			pending_entry: None,
			lenient_mode: lenient,
			declared_currencies: Default::default(),
			declared_accounts: Default::default(),
			exchange_rates: ExchangeRates::new(),
			lots: Default::default(),
		}
	}

	// -----------
	// -- INPUT --
	// -----------

	pub fn declare_currency(
		&mut self,
		currency: &str,
		date: Date,
	) -> Result<(), Error> {
		if self.lenient_mode {
			return Ok(());
		}

		if self.declared_currencies.contains_key(currency) {
			bail!("Currency {} declared twice", currency)
		}

		self.declared_currencies.insert(currency.to_string(), date);

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

	pub fn new_entry(&mut self, date: Date, desc: String) -> Result<(), Error> {
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
		inline_conversion: Option<Amount>,
		cost_basis: Option<Amount>,
		lot_name: Option<String>,
	) -> Result<(), Error> {
		if self.pending_entry.is_none() {
			bail!("Orphaned entry detail")
		}

		if !self.lenient_mode {
			self.check_account(&account)?;
			self.check_currency(&amount.currency)?;
			if let Some(cb) = &cost_basis {
				self.check_currency(&cb.currency)?;
			}
			if let Some(ica) = &inline_conversion {
				self.check_currency(&ica.currency)?;
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

		let pending_entry = self.pending_entry.as_mut().unwrap();
		pending_entry.add_detail(&account, amount.clone())?;

		if let Some(ica) = inline_conversion {
			// An inline conversion has the authority of a
			// declaration in many ways, but in case there are
			// multiple intraday transactions that differ from each
			// other (as day traders etc. experience all the time),
			// we must treat them as inferred rates here.
			self.exchange_rates.infer(
				*pending_entry.get_date(),
				&amount.currency.clone(),
				&ica.currency.clone(),
				ica.value,
			)?;

			// Move the imbalance to the cost basis currency via
			// the virtual conversion account, if this is not a lot
			if cost_basis.is_none() {
				let conversion = VIRTUAL_CONVERSION_ACCOUNT.to_string();
				pending_entry.add_system_detail(
					&conversion,
					Amount::new(ica.value * amount.value, &ica.currency),
				)?;
				pending_entry
					.add_system_detail(&conversion, -amount.clone())?;
			}
		}

		if let Some(cb) = cost_basis {
			pending_entry.add_action(Action::new(
				*pending_entry.get_date(),
				account.clone(),
				amount,
				cb,
				lot_name,
			)?);
		}

		Ok(())
	}

	/// Sets the account name of the entry with no accompanying balance or
	/// currency. This account is then assumed to be the counterparty to all
	/// other detail lines in the entry that create any imbalance.
	///
	/// In general, setting this forces an entry to balance, one way or
	/// another.
	pub fn set_virtual_detail(&mut self, account: String) -> Result<(), Error> {
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

	/// Adds a reference line, which does nothing except appear on some reports
	pub fn add_reference(&mut self, reference: String) -> Result<(), Error> {
		match &mut self.pending_entry {
			Some(e) => {
				e.add_reference(reference);
				Ok(())
			},
			None => bail!("Orphaned reference"),
		}
	}

	/// Takes the pending entry, finalizes it, processes its lots, and adds
	/// it to the set of entries on this. No-ops if nothing is pending.
	pub fn finish_entry(&mut self) -> Result<(), Error> {
		match self.pending_entry.take() {
			None => Ok(()),
			Some(mut entry) => {
				let actions = entry.finalize(&mut self.exchange_rates)?;
				for action in actions {
					self.lots.add_action(action);
				}

				self.entries.push(entry);
				Ok(())
			},
		}
	}

	/// Checks whether a currency has been declared for use, and checks the
	/// pending entry to make sure the declaration date is not ahead of the
	/// pending entry where the currency appears.
	fn check_currency(&self, currency: &str) -> Result<(), Error> {
		let declaration_date = match self.declared_currencies.get(currency) {
			Some(d) => d,
			None => bail!("Currency {} used without declaration", currency),
		};

		if self.pending_entry.as_ref().unwrap().get_date() < declaration_date {
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
		let declaration_date = match self.declared_accounts.get(account) {
			Some(d) => d,
			None => bail!("Account {} used without declaration", account),
		};

		if self.pending_entry.as_ref().unwrap().get_date() < declaration_date {
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
	/// rates are available. The underlying exchange rate system assembles a
	/// graph data structure for indirect conversions, so if there is any path
	/// to an exchange rate, the exchange will be done. Note that all rates
	/// must have been observed on the same day for this graph traversal to
	/// take place.
	pub fn collapse_to(&mut self, currency: String) {
		self.entries
			.iter_mut()
			.flat_map(|e| e.details())
			.for_each(|d| {
				if let Some(rate) =
					self.exchange_rates.get_latest_rate(d.currency(), &currency)
				{
					d.convert_to(&currency, rate)
				}
			});

		self.entries.iter_mut().for_each(|e| {
			e.force_balance(VIRTUAL_CONVERSION_ACCOUNT);
		})
	}

	/// Finalizes the entire ledger by standardizing the visible precision of
	/// each currency, and dropping entries outside the passed date range.
	pub fn finalize(
		&mut self,
		max_reso_by_currency: &HashMap<String, u32>,
		overall_max_reso: Option<u32>,
		drop_before: &Date,
		drop_after: &Date,
	) -> Result<(), Error> {
		let max_reso = overall_max_reso.unwrap_or(u32::MAX);

		self.entries.retain(|e| {
			e.get_date() >= drop_before && e.get_date() <= drop_after
		});

		self.entries.sort();
		for entry in &mut self.entries {
			for (currency, &reso) in max_reso_by_currency {
				entry.round_for_currency(currency, min(reso, max_reso))?;

				entry.force_balance(VIRTUAL_ROUNDING_ERROR_ACCOUNT);
				entry.reduce(vec![
					VIRTUAL_CONVERSION_ACCOUNT,
					VIRTUAL_ROUNDING_ERROR_ACCOUNT,
				]);
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
	use crate::util::quant::Quant;

	#[test]
	fn test_ledger_initialization() {
		let ledger = Ledger::new(true);
		assert!(ledger.entries.is_empty());
		assert!(ledger.pending_entry.is_none());
		assert!(ledger.declared_currencies.is_empty());
		assert!(ledger.declared_accounts.is_empty());
		assert!(ledger.lenient_mode);
	}

	#[test]
	fn test_declare_currency() {
		let mut ledger = Ledger::new(false);
		let date = Date::from_str("2024-1-1").unwrap();

		assert!(ledger.declare_currency("USD", date).is_ok());
		assert!(ledger.declared_currencies.contains_key("USD"));

		assert!(ledger.declare_currency("USD", date).is_err());
	}

	#[test]
	fn test_declare_account() {
		let mut ledger = Ledger::new(false);
		let date = Date::from_str("2024-01-01").unwrap();

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
		let date = Date::from_str("2024-01-01").unwrap();

		assert!(ledger.new_entry(date, "Test Entry".to_string()).is_ok());
		assert!(ledger.pending_entry.is_some());
		assert_eq!(ledger.pending_entry.unwrap().get_date(), &date);
	}

	#[test]
	fn test_add_detail_valid() {
		let mut ledger = Ledger::new(false);
		let date = Date::from_str("2024-01-01").unwrap();
		ledger.declare_currency("USD", date).unwrap();
		ledger
			.declare_account("Assets:Cash".to_string(), date)
			.unwrap();

		ledger.new_entry(date, "Test Entry".to_string()).unwrap();

		assert!(ledger
			.add_detail(
				"Assets:Cash".to_string(),
				Amount::new(Quant::new(1000, 1), "USD",),
				None,
				None,
				None,
			)
			.is_ok());
	}

	#[test]
	fn test_add_detail_invalid_currency() {
		let mut ledger = Ledger::new(false);
		let date = Date::from_str("2024-01-01").unwrap();
		ledger
			.declare_account("Assets:Cash".to_string(), date)
			.unwrap();

		ledger.new_entry(date, "Test Entry".to_string()).unwrap();

		assert!(ledger
			.add_detail(
				"Assets:Cash".to_string(),
				Amount::new(Quant::new(1000, 1), "EUR",),
				None,
				None,
				None
			)
			.is_err());
	}

	#[test]
	fn test_add_detail_invalid_account() {
		let mut ledger = Ledger::new(false);
		let date = Date::from_str("2024-01-01").unwrap();
		ledger.declare_currency("USD", date).unwrap();

		ledger.new_entry(date, "Test Entry".to_string()).unwrap();

		assert!(ledger
			.add_detail(
				"Liabilities:Loan".to_string(),
				Amount::new(Quant::new(1000, 1), "USD",),
				None,
				None,
				None
			)
			.is_err());
	}

	#[test]
	fn test_add_detail_orphaned_entry() {
		let mut ledger = Ledger::new(false);

		assert!(ledger
			.add_detail(
				"Assets:Cash".to_string(),
				Amount::new(Quant::new(1000, 1), "USD",),
				None,
				None,
				None
			)
			.is_err());
	}

	#[test]
	fn test_finish_entry() {
		let mut ledger = Ledger::new(false);
		let date = Date::from_str("2024-01-01").unwrap();
		ledger.new_entry(date, "Test Entry".to_string()).unwrap();
		assert!(ledger.finish_entry().is_ok());
		assert!(ledger.pending_entry.is_none());
		assert_eq!(ledger.entries.len(), 1);
	}

	#[test]
	fn test_check_currency_before_declaration() {
		let mut ledger = Ledger::new(false);
		let date = Date::from_str("2024-01-01").unwrap();
		ledger
			.declare_currency("USD", Date::from_str("2024-1-2").unwrap())
			.unwrap();

		ledger.new_entry(date, "Test Entry".to_string()).unwrap();
		let result = ledger.check_currency("USD");

		assert!(result.is_err());
	}

	#[test]
	fn test_check_account_before_declaration() {
		let mut ledger = Ledger::new(false);
		let date = Date::from_str("2024-01-01").unwrap();
		ledger
			.declare_account(
				"Assets:Cash".to_string(),
				Date::from_str("2024-01-02").unwrap(),
			)
			.unwrap();

		ledger.new_entry(date, "Test Entry".to_string()).unwrap();
		let result = ledger.check_account(&"Assets:Cash".to_string());

		assert!(result.is_err());
	}

	#[test]
	fn test_add_detail_without_currency_declaration_in_lenient_mode() {
		let mut ledger = Ledger::new(true);
		let date = Date::from_str("2024-01-01").unwrap();

		ledger
			.new_entry(date, "Lenient Test Entry".to_string())
			.unwrap();

		assert!(ledger
			.add_detail(
				"Assets:Cash".to_string(),
				Amount::new(Quant::new(500, 1), "EUR",),
				None,
				None,
				None
			)
			.is_ok());
	}

	#[test]
	fn test_add_detail_without_account_declaration_in_lenient_mode() {
		let mut ledger = Ledger::new(true);
		let date = Date::from_str("2024-01-01").unwrap();

		ledger
			.new_entry(date, "Lenient Test Entry".to_string())
			.unwrap();

		assert!(ledger
			.add_detail(
				"Liabilities:Loan".to_string(),
				Amount::new(Quant::new(1000, 1), "USD",),
				None,
				None,
				None
			)
			.is_ok());
	}

	#[test]
	fn test_set_virtual_detail_without_account_declaration_in_lenient_mode() {
		let mut ledger = Ledger::new(true);
		let date = Date::from_str("2024-01-01").unwrap();

		ledger
			.new_entry(date, "Lenient Virtual Detail Test".to_string())
			.unwrap();

		assert!(ledger
			.set_virtual_detail("Equity:OpeningBalance".to_string())
			.is_ok());
	}
}
