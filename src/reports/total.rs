use std::collections::HashMap;
use anyhow::Error;
use crate::tabulation::entry::Detail;
use crate::tabulation::money;
use crate::tabulation::money::Money;

// Each total represents one account or segment, one position on the hierarchy,
// that may have a balance. For example, for the ledger
//
// TODO replace with actual report output when it exists
// Assets
//      Cash
//      AR
// Liabilities
//      Short-Term
//      Long-Term
//
// Each of these lines would have a Total object. The Assets and Liabilities
// totals would each have subtotal lists of length 2.
//
// There is a top level total which will always have amount values of 0 in every
// currency, because double-entry accounting, and account string "".
#[derive(Default)]
pub struct Total {
    account: String,
    amounts: HashMap<String, Money>, // currency -> balance held
    subtotals: HashMap<String, Total>, // account name -> next total
    depth: u32, // top level total has depth 0; Assets/Liabilities depth 1, etc.
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
        // TODO: Maybe this is used in the future.
        Ok(())
    }

    pub fn dump_contents(&self) {
        self.dump_contents_recursive(0);
    }

    fn dump_contents_recursive(&self, indent: usize) {
        let indentation = "\t".repeat(indent);
        if !self.account.is_empty() {
            println!("{}{}", indentation, self.account);
            for (currency, amount) in &self.amounts {
                println!(
                    "{}  {:>10} {}",
                    indentation,
                    format!("{:.2}", amount),
                    currency
                )
            }
        }
        for (_, subtotal) in &self.subtotals {
            subtotal.dump_contents_recursive(indent + 1);
        }
    }
}