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

use crate::gl::entry::Detail;
use crate::gl::ledger::Ledger;
use crate::util::quant::Quant;
use std::collections::HashMap;

/// Each total represents one account or segment, one position on the hierarchy,
/// that may have a balance. For example, for the ledger with hierarchy:
///
/// Assets
///      Cash
///      AR
/// Liabilities
///      Short-Term
///      Long-Term
///
/// Each of these lines would have a Total object. The Assets and Liabilities
/// totals would each have subtotal lists of length 2.
///
/// There is a top level total which will always have amount values of 0 in each
/// currency, because double-entry accounting, and account string "". The only
/// time the top level will be nonzero is after filtering.
#[derive(Default)]
pub struct Total {
	pub account: String,
	pub amounts: HashMap<String, Quant>, // currency -> balance held
	pub subtotals: HashMap<String, Total>, // account name -> next total
	pub depth: u32, // top level total is depth 0; Income/Expenses is 1, etc.
}

impl Total {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn from_ledger(ledger: Ledger) -> Self {
		let mut total = Self::new();

		let all_details: Vec<Detail> = ledger
			.take_entries()
			.into_iter()
			.flat_map(|e| e.take_details())
			.collect();

		total.ingest_details(&all_details);
		total
	}

	pub fn ingest_details(&mut self, details: &Vec<Detail>) {
		for detail in details {
			let mut current = &mut *self;

			for segment in detail.account().split(":").collect::<Vec<&str>>() {
				// Update each total along the hierarchy
				*current
					.amounts
					.entry(detail.currency().to_string())
					.or_insert_with(Quant::zero) += detail.value();

				current = current
					.subtotals
					.entry(segment.to_string())
					.or_insert_with(|| Total {
						account: segment.to_string(),
						amounts: HashMap::new(),
						subtotals: HashMap::new(),
						depth: current.depth + 1,
					});
			}

			// Update the leaf node with the final amount
			*current
				.amounts
				.entry(detail.currency().to_string())
				.or_insert_with(Quant::zero) += detail.value();
		}
	}

	// -------------
	// -- FILTERS --
	// -------------

	/// Drops those subtotals not matching the given strs vec, then sums all
	/// subtotals by currency and updates top-level totals with them.
	/// Designed for filtering to a subset of the VALID_PREFIXES.
	pub fn filter_top_level(&mut self, strs: Vec<&str>) {
		self.subtotals
			.retain(|name, _| strs.contains(&name.as_str()));

		let mut currency_totals: HashMap<String, Quant> = HashMap::new();

		// Sum subtotals; doesn't need to be recursive because we only
		// dropped some top-level branches of the hierarchy; what
		// remains is accurate
		for subtotal in self.subtotals.values_mut() {
			for (currency, amount) in &subtotal.amounts {
				currency_totals
					.entry(currency.clone())
					.and_modify(|e| *e += *amount)
					.or_insert_with(|| *amount);
			}
		}

		self.amounts = currency_totals.into_iter().collect();
	}

	/// Designed for use with collapsed reports, we drop totals not in the
	/// target currency. This is so the collapsed report only shows the
	/// currency requested and not anything that could not be converted to
	/// it.
	pub fn ignore_currencies_except(&mut self, currency: &String) {
		self.amounts.retain(|c, _| c == currency);
		for subtotal in self.subtotals.values_mut() {
			subtotal.ignore_currencies_except(currency);
		}
	}

	/// Invert the signs of every Quant in the hierarchy
	pub fn invert(&mut self) {
		for scalar in self.amounts.values_mut() {
			scalar.negate();
		}

		for subtotal in self.subtotals.values_mut() {
			subtotal.invert();
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::gl::entry::Detail;
	use crate::util::amount::Amount;
	use crate::util::quant::Quant;

	#[test]
	fn test_total_initialization() {
		let total = Total::new();
		assert_eq!(total.account, "");
		assert_eq!(total.amounts.len(), 0);
		assert_eq!(total.subtotals.len(), 0);
		assert_eq!(total.depth, 0);
	}

	#[test]
	fn test_ingest_details_single_detail() {
		let mut total = Total::new();
		let detail = Detail::new(
			"Assets:Cash",
			Amount::new(Quant::new(1000, 1), "USD"),
			false,
		);
		total.ingest_details(&vec![detail]);

		assert_eq!(total.subtotals.len(), 1);
		assert!(total.subtotals.contains_key("Assets"));
		assert_eq!(total.subtotals["Assets"].subtotals.len(), 1);
		assert!(total.subtotals["Assets"].subtotals.contains_key("Cash"));

		let cash_total = &total.subtotals["Assets"].subtotals["Cash"];
		assert_eq!(cash_total.amounts.get("USD"), Some(&Quant::new(1000, 1)));
	}

	#[test]
	fn test_ingest_details_multiple_details_same_currency() {
		let mut total = Total::new();
		let details = vec![
			Detail::new(
				"Assets:Cash",
				Amount::new(Quant::new(1000, 1), "USD"),
				false,
			),
			Detail::new(
				"Assets:AR",
				Amount::new(Quant::new(2000, 1), "USD"),
				false,
			),
		];
		total.ingest_details(&details);

		assert_eq!(total.subtotals.len(), 1);
		assert!(total.subtotals.contains_key("Assets"));

		let assets_total = &total.subtotals["Assets"];
		assert_eq!(assets_total.subtotals.len(), 2);
		assert!(assets_total.subtotals.contains_key("Cash"));
		assert!(assets_total.subtotals.contains_key("AR"));

		let cash_total = &assets_total.subtotals["Cash"];
		let ar_total = &assets_total.subtotals["AR"];
		assert_eq!(cash_total.amounts.get("USD"), Some(&Quant::new(1000, 1)));
		assert_eq!(ar_total.amounts.get("USD"), Some(&Quant::new(2000, 1)));
	}

	#[test]
	fn test_ingest_details_hierarchy() {
		let mut total = Total::new();
		let detail = Detail::new(
			"Liabilities:Short-Term:CreditCard",
			Amount::new(Quant::new(500, 1), "EUR"),
			false,
		);
		total.ingest_details(&vec![detail]);

		assert_eq!(total.subtotals.len(), 1);
		assert!(total.subtotals.contains_key("Liabilities"));

		let liabilities_total = &total.subtotals["Liabilities"];
		assert!(liabilities_total.subtotals.contains_key("Short-Term"));

		let short_term_total = &liabilities_total.subtotals["Short-Term"];
		assert!(short_term_total.subtotals.contains_key("CreditCard"));

		let credit_card_total = &short_term_total.subtotals["CreditCard"];
		assert_eq!(
			credit_card_total.amounts.get("EUR"),
			Some(&Quant::new(500, 1))
		);
	}

	#[test]
	fn test_filter_top_level() {
		let mut total = Total::new();
		total.ingest_details(&vec![
			Detail::new(
				"Assets:Cash",
				Amount::new(Quant::new(1000, 1), "USD"),
				false,
			),
			Detail::new(
				"Liabilities:CreditCard",
				Amount::new(Quant::new(500, 1), "USD"),
				false,
			),
		]);

		total.filter_top_level(vec!["Assets"]);
		assert_eq!(total.subtotals.len(), 1);
		assert!(total.subtotals.contains_key("Assets"));
		assert!(!total.subtotals.contains_key("Liabilities"));

		let assets_total = &total.subtotals["Assets"];
		assert_eq!(assets_total.amounts.get("USD"), Some(&Quant::new(1000, 1)));
	}

	#[test]
	fn test_invert() {
		let mut total = Total::new();
		total.ingest_details(&vec![
			Detail::new(
				"Income:Sales",
				Amount::new(Quant::new(3000, 1), "USD"),
				false,
			),
			Detail::new(
				"Expenses:Rent",
				Amount::new(Quant::new(1000, 1), "USD"),
				false,
			),
		]);

		total.invert();

		let sales_total = &total.subtotals["Income"].subtotals["Sales"];
		let rent_total = &total.subtotals["Expenses"].subtotals["Rent"];

		assert_eq!(sales_total.amounts.get("USD"), Some(&Quant::new(-3000, 1)));
		assert_eq!(rent_total.amounts.get("USD"), Some(&Quant::new(-1000, 1)));
	}

	#[test]
	fn test_no_subtotals_in_empty_total() {
		let total = Total::new();
		assert!(total.subtotals.is_empty());
	}

	#[test]
	fn test_filter_top_level_empty_filter() {
		let mut total = Total::new();
		total.ingest_details(&vec![
			Detail::new(
				"Assets:Cash",
				Amount::new(Quant::new(1000, 1), "USD"),
				false,
			),
			Detail::new(
				"Liabilities:CreditCard",
				Amount::new(Quant::new(500, 1), "USD"),
				false,
			),
		]);

		total.filter_top_level(vec![]);
		assert!(total.subtotals.is_empty());
	}
}
