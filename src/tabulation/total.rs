use std::collections::HashMap;
use anyhow::Error;
use crate::tabulation::entry::Detail;
use crate::tabulation::money;
use crate::tabulation::money::Money;

/// Each total represents one account or segment, one position on the hierarchy,
/// that may have a balance. For example, for the ledger with hierarchy
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
    pub amounts: HashMap<String, Money>, // currency -> balance held
    pub subtotals: HashMap<String, Total>, // account name -> next total
    pub depth: u32, // top level total is depth 0; Income/Expenses is 1, etc.
}

impl Total {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn ingest_details(&mut self, details: &Vec<Detail>) {
        for detail in details {
            let mut current_total = &mut *self;

            for segment in &detail.account {
                // Update each total along the hierarchy
                *current_total.amounts
                    .entry(detail.currency())
                    .or_insert_with(|| money::ZERO) += detail.amount;

                current_total = current_total.subtotals.entry(segment.clone())
                    .or_insert_with(|| Total {
                        account: segment.clone(),
                        amounts: HashMap::new(),
                        subtotals: HashMap::new(),
                        depth: current_total.depth + 1,
                    });
            }

            // Update the leaf node with the final amount
            *current_total.amounts
                .entry(detail.currency())
                .or_insert_with(|| money::ZERO) += detail.amount;
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        // TODO: Maybe this will be used in the future.
        Ok(())
    }

    // -------------
    // -- FILTERS --
    // -------------

    /// Drops those subtotals not matching the given strs vec. Designed to be
    /// used for filtering to a subset of the VALID_PREFIXES.
    pub fn filter_top_level(&mut self, strs: Vec<&str>) {
        self.subtotals.retain(|name, _| strs.contains(&name.as_str()));
        self.recompute_top_level();
    }

    /// Invert the signs of every Money in the hierarchy
    pub fn invert(&mut self) {
        for (_, money) in &mut self.amounts {
            money.negate();
        }

        for (_, subtotal) in &mut self.subtotals {
            subtotal.invert();
        }
    }

    /// Sums all subtotals by currency and updates top-level totals with them
    fn recompute_top_level(&mut self) {
        let mut currency_totals: HashMap<String, Money> = HashMap::new();

        // Sum subtotals; doesn't need to be recursive because we only dropped
        // some top-level branches of the hierarchy; what remains is accurate
        for (_, subtotal) in &mut self.subtotals {
            for (currency, amount) in &subtotal.amounts {
                currency_totals
                    .entry(currency.clone())
                    .and_modify(|e| *e += *amount)
                    .or_insert_with(|| amount.clone());
            }
        }

        self.amounts = currency_totals.into_iter().collect();
    }
}