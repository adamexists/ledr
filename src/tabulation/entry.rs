use std::collections::HashMap;
use std::string::ToString;
use anyhow::{bail, Error};
use crate::tabulation::exchange_rate::ExchangeRates;
use crate::util::scalar;
use crate::util::scalar::Scalar;
use crate::util::date::Date;

const VIRTUAL_CONVERSION_ACCOUNT: &str = "Equity:Conversions";

#[derive(Debug)]
pub struct Entry {
    date: Date,
    desc: String,
    details: Vec<Detail>,
    virtual_detail: Option<String>,
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
        amount: Scalar,
        currency: String,
        cost_basis: Option<(String, String)>,
    ) -> Result<(), Error> {
        if self.is_finalized {
            bail!("entry already finalized")
        }

        if account.len() == 0 {
            bail!("account is blank")
        }

        if amount == 0 {
            bail!("amount is blank: {:?}", amount)
        }

        let new_detail = Detail {
            account,
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

        self.virtual_detail = Some(account);
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
                    let mut special_entry = -Scalar::from_str(&*cb_amt.clone())?;
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
            self.is_finalized = true;
            return Ok(());
        }

        // No virtual detail account and unbalanced, return an error
        bail!("Unbalanced entry")
    }

    fn implicit_conversions_check(&mut self, rates: &mut ExchangeRates) -> Result<(), Error> {
        let mut unbalanced_currencies = self.get_unbalanced_currencies();

        if unbalanced_currencies.is_empty() {
            self.is_finalized = true;
            return Ok(());
        }

        // Assume implicit currency conversion if exactly two currencies are
        // unbalanced and in opposite directions.
        if unbalanced_currencies.len() == 2 && self.virtual_detail.is_none() {
            let (currency1, amount1) = unbalanced_currencies.remove(0);
            let (currency2, amount2) = unbalanced_currencies.remove(0);

            if (amount1 < 0 && amount2 < 0)
                || (amount1 > 0 && amount2 > 0) {
                bail!("Unbalanced entry")
            }

            let virtual_detail1 = Detail {
                account: VIRTUAL_CONVERSION_ACCOUNT.to_string(),
                amount: -amount1,
                currency: currency1.clone(),
                cost_basis: None,
            };

            let virtual_detail2 = Detail {
                account: VIRTUAL_CONVERSION_ACCOUNT.to_string(),
                amount: -amount2,
                currency: currency2.clone(),
                cost_basis: None,
            };

            // a quick hack to keep the scalar division more efficient
            if amount1 < amount2 {
                rates.infer(
                    self.date,
                    currency2,
                    currency1,
                    (amount1 / amount2).abs(),
                )?;
            } else {
                rates.infer(
                    self.date,
                    currency1,
                    currency2,
                    (amount2 / amount1).abs(),
                )?;
            }

            self.details.push(virtual_detail1);
            self.details.push(virtual_detail2);

            self.is_finalized = true;
            return Ok(());
        }

        self.is_finalized = true;
        Ok(())
    }

    fn get_unbalanced_currencies(&self) -> Vec<(String, Scalar)> {
        // Step 1: Sum amounts for each currency
        let mut currency_sums: HashMap<String, Scalar> = HashMap::new();

        for detail in &self.details {
            let entry = currency_sums.entry(detail.currency())
                .or_insert(scalar::ZERO);
            *entry += detail.amount;
        }

        // Step 2: Check if all currencies sum to zero
        currency_sums
            .into_iter()
            .filter(|(_, amount)| *amount != 0)
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct Detail {
    pub account: String,
    pub amount: Scalar,
    currency: String,

    cost_basis: Option<(String, String)>,
}

impl Detail {
    pub fn currency(&self) -> String {
        self.currency.clone()
    }

    pub fn convert_to(&mut self, currency: &String, rate: Scalar) {
        if &self.currency == currency {
            return;
        }

        self.currency = currency.clone();
        self.amount = rate;
    }

    pub fn remove_cost_basis(&mut self) {
        self.cost_basis = None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::scalar::Scalar;
    use crate::tabulation::exchange_rate::ExchangeRates;
    use crate::util::date::Date;

    // Helper function to create a sample Date for testing
    fn sample_date(offset: u8) -> Date {
        Date::from_ymd(2024, 1, 1+offset)
    }

    // Helper function to set up an Entry with a date and description
    fn create_entry(offset: u8) -> Entry {
        Entry::new(sample_date(offset), "Sample Entry".to_string())
    }

    #[test]
    fn test_entry_creation() {
        let entry = create_entry(0);
        assert_eq!(entry.get_date(), &sample_date(0));
        assert!(entry.get_details().is_empty());
        assert!(!entry.is_finalized);
    }

    #[test]
    fn test_add_detail() {
        let mut entry = create_entry(0);
        let result = entry.add_detail(
            "Assets:Cash".to_string(),
            Scalar::new(1000, 1),
            "USD".to_string(),
            None,
        );

        assert!(result.is_ok());
        assert_eq!(entry.get_details().len(), 1);

        let detail = &entry.get_details()[0];
        assert_eq!(detail.account, "Assets:Cash");
        assert_eq!(detail.amount, Scalar::new(1000, 1));
        assert_eq!(detail.currency, "USD");
    }

    #[test]
    fn test_finalize_unbalanced_entry() {
        let mut entry = create_entry(0);
        entry
            .add_detail(
                "Assets:Cash".to_string(),
                Scalar::new(1000, 1),
                "USD".to_string(),
                None,
            )
            .unwrap();
        entry
            .add_detail(
                "Expenses:Food".to_string(),
                Scalar::new(-500, 1),
                "USD".to_string(),
                None,
            )
            .unwrap();

        let mut rates = ExchangeRates::new(); // Assuming ExchangeRates has a new method
        let result = entry.finalize(&mut rates);

        // Expect an error since the entry is unbalanced
        assert!(result.is_err());
    }

    #[test]
    fn test_finalize_balanced_entry() {
        let mut entry = create_entry(0);
        entry
            .add_detail(
                "Assets:Cash".to_string(),
                Scalar::new(1000, 1),
                "USD".to_string(),
                None,
            )
            .unwrap();
        entry
            .add_detail(
                "Expenses:Food".to_string(),
                Scalar::new(-1000, 1),
                "USD".to_string(),
                None,
            )
            .unwrap();

        let mut rates = ExchangeRates::new();
        let result = entry.finalize(&mut rates);

        // Expect success since the entry is balanced
        assert!(result.is_ok());
        assert!(entry.is_finalized);
    }

    // Additional helper function to set up ExchangeRates if needed
    fn setup_exchange_rates() -> ExchangeRates {
        ExchangeRates::new() // Assuming an appropriate constructor
    }

    // Test for setting virtual detail
    #[test]
    fn test_set_virtual_detail() {
        let mut entry = create_entry(0);
        let result = entry.set_virtual_detail("Assets:Virtual".to_string());

        assert!(result.is_ok());
        assert!(entry.virtual_detail.is_some());
        assert_eq!(entry.virtual_detail.unwrap(), "Assets")
    }

    // Placeholder for future tests, e.g., testing resolution adjustments
    #[test]
    fn test_set_resolution_for_currency() {
        let mut entry = create_entry(0);
        entry
            .add_detail(
                "Assets:Cash".to_string(),
                Scalar::new(1234567, 4),
                "USD".to_string(),
                None,
            )
            .unwrap();

        let result = entry.set_resolution_for_currency(&"USD".to_string(), 2);
        assert!(result.is_ok());

        let detail = &entry.get_details()[0];
        assert_eq!(detail.amount.amount(), 12345); // Check for truncation
        assert_eq!(detail.amount.resolution(), 2);
    }
}
