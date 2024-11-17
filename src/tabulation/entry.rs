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
        &mut self,
        account: String,
        amount: Money,
        currency: String,
        cost_basis: Option<(String, String)>,
    ) -> Result<(), Error> {
        if self.is_finalized {
            bail!("entry already finalized")
        }

        if account.len() == 0 {
            bail!("account is blank")
        }

        if amount == 0f64 {
            bail!("amount is blank")
        }

        let new_detail = Detail {
            account: account.split(':').map(|s| s.to_string()).collect(),
            amount,
            currency,
            cost_basis,
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

    /// Adjusts all Details for a certain currency to a certain resolution. In
    /// doing so, precision may be lost, but not gained (because the extra
    /// decimal places will just fill in with zeroes). This is more about the
    /// clean display of currency amounts for reporting.
    pub fn set_resolution_for_currency(
        &mut self, currency: &String, resolution: u32) -> Result<(), Error> {
        for detail in &mut self.details {
            if &detail.currency == currency {
                detail.amount.set_resolution(resolution)
            }
        }

        Ok(())
    }

    /// Completes an entry. We have to pass the exchange rate set in here,
    /// because this is where exchange rates are inferred in some cases, i.e. if
    /// exactly two currencies are imbalanced.
    ///
    /// TODO: This method is in dire need of cleanup.
    pub fn finalize(&mut self, rates: &mut ExchangeRates) -> Result<(), Error> {
        self.implicit_conversions_check(rates)?;

        let mut unbalanced_currencies = self.get_unbalanced_currencies();

        // Step 3: Handle the case where there's a virtual detail account
        if let Some(account) = &self.virtual_detail {
            if unbalanced_currencies.len() == 1 {

                // This section TODO needs to be properly manpaged etc.
                // What we do here is that, if we have a virtual detail and the
                // only other detail in the entry has a cost basis, then we do
                // a bit of syntactic sugar and assume the user has exchanged
                // the cost basis currency and amount.
                if self.details.len() == 1 && self.details.first().unwrap().cost_basis.is_some() {
                    let details = self.details.first().unwrap();

                    let (cb_amt, cb_cur) = details.cost_basis.as_ref().unwrap();

                    // TODO: I thought about making sure the syntactic sugar
                    // entry has the resolution of the cost basis currency, as
                    // displayed, after the multiplication, but it caused some
                    // precision errors. Gotta reconsider. In general there are
                    // some clear precision & rounding errors causing weirdness
                    // on the periphery. Thus this project needs attention.
                    let mut special_entry = -Money::new(&*cb_amt.clone())?;
                    let res = special_entry.resolution();
                    special_entry *= details.amount;
                    // special_entry.set_resolution(res);

                    self.details.push(Detail {
                        account: account.clone(),
                        amount: special_entry,
                        currency: cb_cur.clone(),
                        cost_basis: None,
                    });
                } else {
                    let (cur, sum) = unbalanced_currencies.pop().unwrap();
                    let new_detail = Detail {
                        account: account.clone(),
                        amount: -sum,
                        currency: cur,
                        cost_basis: None,
                    };

                    self.details.push(new_detail);
                };

                self.virtual_detail = None;
            };
        }

        self.implicit_conversions_check(rates)?;
        unbalanced_currencies = self.get_unbalanced_currencies();
        if unbalanced_currencies.is_empty() {
            return Ok(());
        }

        // No virtual detail account and unbalanced, return an error
        bail!("Unbalanced entry")
    }

    fn implicit_conversions_check(&mut self, rates: &mut ExchangeRates) -> Result<(), Error> {
        let mut unbalanced_currencies = self.get_unbalanced_currencies();

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
                cost_basis: None,
            };

            let virtual_detail2 = Detail {
                account: VIRTUAL_CONVERSION_ACCOUNT
                    .split(':')
                    .map(|s| s.to_string())
                    .collect(),
                amount: -amount2,
                currency: currency2.clone(),
                cost_basis: None,
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

        Ok(())
    }

    fn get_unbalanced_currencies(&self) -> Vec<(String, Money)> {
        // Step 1: Sum amounts for each currency
        let mut currency_sums: HashMap<String, Money> = HashMap::new();

        for detail in &self.details {
            let entry = currency_sums.entry(detail.currency())
                .or_insert(money::ZERO);
            *entry += detail.amount;
        }

        // Step 2: Check if all currencies sum to zero
        currency_sums
            .into_iter()
            .filter(|(_, amount)| amount != 0f64)
            .collect()
    }
}

#[derive(Clone)]
pub struct Detail {
    // "Assets:Cash" -> Vec<String> = {"Assets","Cash"}
    pub account: Vec<String>,
    pub amount: Money,
    currency: String,

    cost_basis: Option<(String, String)>,
}

impl Detail {
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

    pub fn remove_cost_basis(&mut self) {
        self.cost_basis = None
    }
}