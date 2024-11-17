use std::collections::HashMap;
use std::string::ToString;
use anyhow::{bail, Error};
use crate::tabulation::exchange_rate::ExchangeRates;
use crate::tabulation::money;
use crate::tabulation::money::Money;
use crate::util::date::Date;

const VIRTUAL_CONVERSION_ACCOUNT: &str = "Equity:Conversions";

pub struct Entry {
    date: Date,
    desc: String,
    details: Vec<Detail>,
    virtual_detail: Option<Vec<String>>,
    is_finalized: bool,
}

impl Entry {
    pub fn new(date: Date, desc: String) -> Self {
        Self {
            date,
            desc,
            details: vec![],
            virtual_detail: None,
            is_finalized: false,
        }
    }

    pub fn add_detail(
        &mut self, account:
        String, amount: String,
        currency: String,
        cost_basis: Option<(String, String)>,
    ) -> Result<(), Error> {
        if self.is_finalized {
            bail!("entry already finalized")
        }

        if account.len() == 0 {
            bail!("account is blank")
        }

        if amount.len() == 0 {
            bail!("amount is blank")
        }

        let mut new_detail = Detail {
            account: account.split(':').map(|s| s.to_string()).collect(),
            amount: Money::new(&*amount)?,
            currency,
        };
        if let Some((amt, sym)) = cost_basis {
            new_detail.add_cost_basis(amt, sym)
        };

        self.details.push(new_detail);
        Ok(())
    }

    pub fn set_virtual_detail(&mut self, account: String) -> Result<(), Error> {
        if self.is_finalized {
            bail!("entry already finalized")
        }

        if self.virtual_detail.is_some() {
            bail!("only one line per entry may omit amount and currency")
        }

        if account.len() == 0 {
            bail!("account is blank")
        }

        self.virtual_detail =
            Some(account.split(':').map(|s| s.to_string()).collect());
        Ok(())
    }

    pub fn get_date(&self) -> &Date {
        &self.date
    }

    pub fn details(&mut self) -> &mut Vec<Detail> {
        &mut self.details
    }

    pub fn get_details(&self) -> &Vec<Detail> {
        &self.details
    }

    pub fn take_details(self) -> Vec<Detail> {
        self.details
    }

    pub fn set_resolution_for_currency(&mut self, currency: &String, resolution: u32) {
        for detail in &mut self.details {
            if &detail.currency == currency {
                detail.amount.set_resolution(resolution)
            }
        }
    }

    /// Completes an entry. We have to pass the exchange rate set in here,
    /// because this is where exchange rates are inferred in some cases, i.e. if
    /// exactly two currencies are imbalanced.
    pub fn finalize(&mut self, rates: &mut ExchangeRates) -> Result<(), Error> {
        // Step 1: Sum amounts for each currency
        let mut currency_sums: HashMap<String, Money> = HashMap::new();

        for detail in &self.details {
            let entry = currency_sums.entry(detail.currency())
                .or_insert(money::ZERO);
            *entry += detail.amount;
        }

        // Step 2: Check if all currencies sum to zero
        let mut unbalanced_currencies: Vec<(String, Money)> = currency_sums
            .into_iter()
            .filter(|(_, amount)| amount != 0f64)
            .collect();

        if unbalanced_currencies.is_empty() {
            return Ok(());
        }

        // Assume implicit currency conversion if exactly two currencies are
        // unbalanced and in opposite directions.
        if unbalanced_currencies.len() == 2 && self.virtual_detail.is_none() {
            let (currency1, amount1) = unbalanced_currencies.remove(0);
            let (currency2, amount2) = unbalanced_currencies.remove(0);

            if (amount1 < 0f64 && amount2 < 0f64)
                || (amount1 > 0f64 && amount2 > 0f64) {
                bail!("Unbalanced entry")
            }

            let virtual_detail1 = Detail {
                account: VIRTUAL_CONVERSION_ACCOUNT
                    .split(':')
                    .map(|s| s.to_string())
                    .collect(),
                amount: -amount1,
                currency: currency1.clone(),
            };

            let virtual_detail2 = Detail {
                account: VIRTUAL_CONVERSION_ACCOUNT
                    .split(':')
                    .map(|s| s.to_string())
                    .collect(),
                amount: -amount2,
                currency: currency2.clone(),
            };

            rates.infer(
                self.date,
                currency1,
                currency2,
                (amount2.to_f64() / amount1.to_f64()).abs(),
            )?;

            self.details.push(virtual_detail1);
            self.details.push(virtual_detail2);

            self.is_finalized = true;
            return Ok(());
        }

        // Step 3: Handle the case where there's a virtual detail account
        if let Some(account) = &self.virtual_detail {
            return if unbalanced_currencies.len() == 1 {
                let (cur, sum) = unbalanced_currencies.pop().unwrap();
                let new_detail = Detail {
                    account: account.clone(),
                    amount: -sum,
                    currency: cur,
                };

                self.details.push(new_detail);
                self.virtual_detail = None;
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
    pub amount: Money,
    currency: String,
}

impl Detail {
    pub fn add_cost_basis(&mut self, amount: String, currency: String) {
        self.currency = format!("{} @ {} {}", self.currency, amount, currency);
    }

    pub fn currency(&self) -> String {
        self.currency.clone()
    }

    pub fn convert_to(&mut self, currency: &String, rate: f64) {
        if &self.currency == currency {
            return;
        }

        self.currency = currency.clone();
        self.amount = Money::new_from_f64(
            rate * self.amount.to_f64(),
            self.amount.resolution(),
        );
    }

    /// Removes cost basis from the currency string.
    pub fn remove_cost_basis(&mut self) {
        if let Some(index) = self.currency.find(' ') {
            self.currency.truncate(index);
        }
    }
}