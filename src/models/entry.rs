use std::collections::HashMap;
use anyhow::{anyhow, Error};
use crate::models::currency::Currency;

struct Entry {
    date: chrono::NaiveDate,
    desc: String,
    details: Vec<Detail>,
    virtual_detail_account: Option<Vec<String>>,
}

impl Entry {
    pub fn balance(&mut self) -> Result<(), Error> {
        // Step 1: Sum amounts for each currency
        let mut currency_sums: HashMap<u32, Currency> = HashMap::new();

        for detail in &self.details {
            let entry = currency_sums.entry(detail.amount.ident())
                .or_insert(Currency::new(0, 0, detail.amount.ident()));
            *entry += detail.amount;
        }

        // Step 2: Check if all currencies sum to zero
        let mut unbalanced_currencies: Vec<(u32, Currency)> = currency_sums
            .iter()
            .filter(|&(_, &sum)| !sum.is_zero())
            .collect();

        if unbalanced_currencies.is_empty() {
            // Entry is already balanced
            return Ok(());
        }

        // Step 3: Handle the case where there's a virtual detail account
        if let Some(account) = &self.virtual_detail_account {
            return if unbalanced_currencies.len() == 1 {
                // Only one currency is unbalanced, let's balance it
                let (_, sum) = unbalanced_currencies.pop().unwrap();
                let new_detail = Detail {
                    account: account.clone(),
                    amount: -sum, // TODO Confirm this works.
                };

                self.details.push(new_detail);
                self.virtual_detail_account = None;
                Ok(())
            } else {
                // More than one currency is unbalanced, cannot balance
                Err(anyhow!("Unbalanced entry"))
            }
        }

        // No virtual detail account and unbalanced, return an error
        Err(anyhow!("Unbalanced entry"))
    }
}

struct Detail {
    // "Assets:Cash" -> Vec<String> = {"Assets","Cash"}
    account: Vec<String>,
    amount: Currency,
}