use std::collections::HashMap;
use anyhow::{bail, Error};
use crate::models::amount::Amount;
use crate::models::currency::Currency;
use crate::models::entry::Detail;

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
    amounts: HashMap<Currency, Amount>,
    subtotals: HashMap<String, Total>, // account name -> next total
    depth: u32, // top level total has depth 0; Assets/Liabilities depth 1, etc.
}

impl Total {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn ingest_details(&mut self, currency_lookup: &HashMap<u32, Currency>, details: &Vec<Detail>) {
        for detail in details {
            let mut current_total = &mut *self;

            // Traverse or create the hierarchy based on the detail's account vector
            for (_, segment) in detail.account.iter().enumerate() {
                // Update each total along the hierarchy
                let amount_entry = current_total.amounts
                    .entry(currency_lookup.get(&detail.amount.ident()).unwrap().clone())
                    .or_insert_with(|| Amount::new(0, 0, detail.amount.ident()));
                *amount_entry += detail.amount;

                // Traverse to the next subtotal or create a new one if it doesn't exist
                current_total = current_total.subtotals.entry(segment.clone())
                    .or_insert_with(|| Total {
                        account: segment.clone(),
                        amounts: HashMap::new(),
                        subtotals: HashMap::new(),
                        depth: current_total.depth + 1,
                    });
            }

            // Add the detail's amount to the final current total's amounts map
            let amount_entry = current_total.amounts
                .entry(currency_lookup.get(&detail.amount.ident()).unwrap().clone())
                .or_insert_with(|| Amount::new(0, 0, detail.amount.ident()));
            *amount_entry += detail.amount;
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        for (currency, amount) in &self.amounts {
            if !self.subtotals.is_empty() {
                let subtotal_sum = self.subtotals.values()
                    .filter_map(|subtotal| subtotal.amounts.get(currency))
                    .fold(Amount::new(0, 0, amount.ident()), |acc, subtotal_amount| acc + *subtotal_amount);

                if *amount != subtotal_sum {
                    // TODO: This rule might be stupid and contradict a lot of common cases.
                    //  But it makes reports easier, at least in the very short-term.
                    bail!("account that has its own balance ({}) cannot have subaccounts", self.account);
                }
            }
        }

        for subtotal in self.subtotals.values() {
            subtotal.validate()?;
        }

        Ok(())
    }

    pub fn dump_contents(&self) {
        self.dump_contents_recursive(0);
    }

    fn dump_contents_recursive(&self, indent: usize) {
        let indentation = "\t".repeat(indent);
        if !self.account.is_empty() {
            println!("{}{}", indentation, self.account);
            if self.subtotals.len() == 0 {
                for (currency, amount) in &self.amounts {
                    println!("{}  {:>10} {}", indentation, format!("{:.2}", amount), currency.symbol());
                }
            }
        }
        for (_, subtotal) in &self.subtotals {
            subtotal.dump_contents_recursive(indent + 1);
        }
    }
}