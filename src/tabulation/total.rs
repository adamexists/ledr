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

use crate::tabulation::entry::Detail;
use crate::util::scalar;
use crate::util::scalar::Scalar;
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
    pub amounts: HashMap<String, Scalar>, // currency -> balance held
    pub subtotals: HashMap<String, Total>, // account name -> next total
    pub depth: u32,                       // top level total is depth 0; Income/Expenses is 1, etc.
}

impl Total {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn ingest_details(&mut self, details: &Vec<Detail>) {
        for detail in details {
            let mut current = &mut *self;

            for segment in &detail.account.split(":").collect::<Vec<&str>>() {
                // Update each total along the hierarchy
                *current
                    .amounts
                    .entry(detail.currency())
                    .or_insert_with(|| scalar::ZERO) += detail.amount;

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
                .entry(detail.currency())
                .or_insert_with(|| scalar::ZERO) += detail.amount;
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

        let mut currency_totals: HashMap<String, Scalar> = HashMap::new();

        // Sum subtotals; doesn't need to be recursive because we only dropped
        // some top-level branches of the hierarchy; what remains is accurate
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

    /// Invert the signs of every Scalar in the hierarchy
    pub fn invert(&mut self) {
        for scalar in self.amounts.values_mut() {
            scalar.negate();
        }

        for subtotal in self.subtotals.values_mut() {
            subtotal.invert();
        }
    }
}
