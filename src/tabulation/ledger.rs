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

use crate::tabulation::entry::{CostBasis, Detail, Entry};
use crate::tabulation::exchange_rate::ExchangeRates;
use crate::tabulation::ledger::CostBasisAmountType::TotalCost;
use crate::tabulation::lot::Lots;
use crate::tabulation::total::Total;
use crate::util::date::Date;
use crate::util::scalar::Scalar;
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
		amount: String,
		currency: String,
		cost_basis: Option<CostBasisInput>,
	) -> Result<(), Error> {
		self.check_account(&account)?;
		self.check_currency(&currency)?;

		if let Some(cost_basis_currency) = &cost_basis {
			self.check_currency(&cost_basis_currency.currency)?;
		}

		if self.pending_entry.is_none() {
			bail!("Orphaned entry detail")
		}

		if account.is_empty() {
			bail!("Account is empty")
		}

		if amount.is_empty() {
			bail!("Amount is empty")
		}

		let has_valid_prefix = VALID_PREFIXES
			.iter()
			.any(|&prefix| account.starts_with(prefix));
		if !has_valid_prefix {
			bail!("Invalid account prefix: {}", account)
		}

		let money_amt = Scalar::from_str(&amount)?;
		let mut cost_basis_input = None;

		if let Some(mut cb) = cost_basis {
			if cb.amount_type == TotalCost {
				cb.amount /= money_amt;
			}

			// A cost basis has the authority of a declaration in
			// many ways, but in case there are multiple intraday
			// transactions that differ from each other (as day
			// traders etc. experience all the time), we must treat
			// them as inferred rates here.
			self.exchange_rates.infer(
				self.pending_entry_date(),
				currency.clone(),
				cb.currency.clone(),
				cb.amount,
			)?;

			self.lots.add_movement(
				self.pending_entry_date(),
				account.clone(),
				currency.clone(),
				money_amt,
				cb.amount,
				cb.currency.clone(),
			)?;

			cost_basis_input = Some(CostBasis {
				unit_price: cb.amount,
				currency: cb.currency,
				associated_amount: money_amt,
			});
		}

		self.pending_entry.as_mut().unwrap().add_detail(
			account,
			money_amt,
			currency,
			cost_basis_input,
		)
	}

	pub fn set_virtual_detail(
		&mut self,
		account: String,
	) -> Result<(), Error> {
		self.check_account(&account)?;

		if self.pending_entry.is_none() {
			bail!("Orphaned entry detail")
		}

		self.pending_entry
			.as_mut()
			.unwrap()
			.set_virtual_detail(account)
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

	pub fn pending_entry_date(&self) -> Date {
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
	///
	/// Consumes the ledger.
	pub fn finalize(
		mut self,
		overall_max_reso: Option<u32>,
	) -> Result<Total, Error> {
		let mut max_reso_by_currency: HashMap<String, u32> =
			HashMap::new();
		let mut overall_max = 6;
		if let Some(requested_max) = overall_max_reso {
			overall_max = requested_max;
		}

		for detail in self.entries.iter().flat_map(|x| x.get_details())
		{
			let reso = min(detail.amount.resolution(), overall_max);
			let currency = detail.currency();

			// Update max if this detail has higher resolution
			max_reso_by_currency
				.entry(currency.clone())
				.and_modify(|max_reso| {
					if reso > *max_reso {
						*max_reso = reso;
					}
				})
				.or_insert(reso);
		}

		// Standardize all currencies to the highest precision found
		for entry in &mut self.entries {
			for (currency, &reso) in &max_reso_by_currency {
				entry.set_resolution_for_currency(
					currency, reso,
				)?
			}
		}

		// Transform this into a Total, and return that
		let mut total = Total::new();

		let all_details: Vec<Detail> = self
			.entries
			.into_iter()
			.flat_map(|e| e.take_details())
			.collect();

		total.ingest_details(&all_details);
		Ok(total)
	}
}

#[derive(Debug, PartialEq)]
pub enum CostBasisAmountType {
	UnitCost,
	TotalCost,
}

#[derive(Debug)]
pub struct CostBasisInput {
	pub amount: Scalar,
	pub amount_type: CostBasisAmountType,

	pub currency: String,
}
