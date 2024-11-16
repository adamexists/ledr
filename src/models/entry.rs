use std::collections::HashMap;
use std::string::ToString;
use anyhow::{bail, Error};
use chrono::NaiveDate;
use crate::models::amount::Amount;

const VIRTUAL_CONVERSION_ACCOUNT: &str = "Equity:Conversions";

pub struct Entry {
    date: NaiveDate,
    desc: String,
    details: Vec<Detail>,
    virtual_detail_account: Option<Vec<String>>,
    is_finalized: bool,
}

impl Entry {
    pub fn new(date: NaiveDate, desc: String) -> Self {
        Self {
            date,
            desc,
            details: vec![],
            virtual_detail_account: None,
            is_finalized: false,
        }
    }

    pub fn add_detail(&mut self, account: String, amount: String, ident: u32) -> Result<(), Error> {
        if self.is_finalized {
            bail!("entry already finalized")
        }

        if account.len() == 0 {
            bail!("account is blank")
        }

        if amount.len() == 0 {
            bail!("amount is blank")
        }

        self.details.push(Detail {
            account: account.split(':').map(|s| s.to_string()).collect(),
            amount: Amount::new_from_str(amount, ident)?,
        });

        Ok(())
    }

    pub fn set_virtual_detail(&mut self, account: String) -> Result<(), Error> {
        if self.is_finalized {
            bail!("entry already finalized")
        }

        if self.virtual_detail_account.is_some() {
            bail!("only one line per entry may omit amount and currency")
        }

        if account.len() == 0 {
            bail!("account is blank")
        }

        self.virtual_detail_account =
            Some(account.split(':').map(|s| s.to_string()).collect());
        Ok(())
    }

    pub fn get_date(&self) -> &NaiveDate {
        &self.date
    }

    pub fn take_details(self) -> Vec<Detail> {
        self.details
    }

    pub fn finalize(&mut self) -> Result<(), Error> {
        // Step 1: Sum amounts for each currency
        let mut currency_sums: HashMap<u32, Amount> = HashMap::new();

        for detail in &self.details {
            let entry = currency_sums.entry(detail.amount.ident())
                .or_insert(Amount::new(0, 0, detail.amount.ident()));
            *entry += detail.amount;
        }

        // Step 2: Check if all currencies sum to zero
        let mut unbalanced_currencies: Vec<(u32, Amount)> = currency_sums
            .iter()
            .filter(|(_, &sum)| !sum.is_zero())
            .map(|(&ident, &sum)| (ident, sum))
            .collect();

        if unbalanced_currencies.is_empty() {
            return Ok(());
        }

        // Assume implicit currency conversion if exactly two currencies are
        // unbalanced and in opposite directions.
        if unbalanced_currencies.len() == 2 && self.virtual_detail_account.is_none() {
            let (_, amount1) = unbalanced_currencies.remove(0);
            let (_, amount2) = unbalanced_currencies.remove(0);

            if (amount1.is_neg() && amount2.is_neg())
                || (!amount1.is_neg() && !amount2.is_neg()) {

                bail!("Unbalanced entry")
            }

            let virtual_detail1 = Detail {
                account: VIRTUAL_CONVERSION_ACCOUNT.split(':').map(|s| s.to_string()).collect(),
                amount: -amount1,
            };

            let virtual_detail2 = Detail {
                account: VIRTUAL_CONVERSION_ACCOUNT.split(':').map(|s| s.to_string()).collect(),
                amount: -amount2,
            };

            self.details.push(virtual_detail1);
            self.details.push(virtual_detail2);

            self.is_finalized = true;
            return Ok(());
        }

        // Step 3: Handle the case where there's a virtual detail account
        if let Some(account) = &self.virtual_detail_account {
            return if unbalanced_currencies.len() == 1 {
                let (_, sum) = unbalanced_currencies.pop().unwrap();
                let new_detail = Detail {
                    account: account.clone(),
                    amount: -sum,
                };

                self.details.push(new_detail);
                self.virtual_detail_account = None;
                self.is_finalized = true;
                Ok(())
            } else {
                bail!("Unbalanced entry")
            };
        }

        // No virtual detail account and unbalanced, return an error
        bail!("Unbalanced entry")
    }
}

#[derive(Clone)]
pub struct Detail {
    // "Assets:Cash" -> Vec<String> = {"Assets","Cash"}
    pub account: Vec<String>,
    pub amount: Amount,
}