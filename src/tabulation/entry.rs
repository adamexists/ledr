use std::collections::{HashMap, HashSet};
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
    is_finalized: bool,

    virtual_detail: Option<String>,
    totals: HashMap<String, Scalar>, // Currency -> Amount
    cost_basis_totals: HashMap<String, Scalar>, // Currency -> Amount
}

impl Entry {
    pub fn new(date: Date, desc: String) -> Self {
        Self {
            date,
            desc,
            details: vec![],
            virtual_detail: None,
            is_finalized: false,
            totals: HashMap::new(),
            cost_basis_totals: HashMap::new(),
        }
    }

    pub fn add_detail(
        &mut self,
        account: String,
        amount: Scalar,
        currency: String,
        cost_basis: Option<CostBasis>,
    ) -> Result<(), Error> {
        if self.is_finalized {
            bail!("entry already finalized")
        }

        if account.len() == 0 {
            bail!("account is blank")
        }

        // details with a cost basis cause an imbalance of the cost basis
        // currency also, because they imply that amount has offset this detail
        *self.totals.entry(currency.clone()).or_insert(scalar::ZERO) += amount;
        if let Some(cb) = &cost_basis {
            *self.cost_basis_totals.entry(
                cb.currency.clone()
            ).or_insert(scalar::ZERO) += cb.unit_price * amount;
        }

        self.details.push(Detail {
            account,
            amount,
            currency,
            cost_basis,
        });


        Ok(())
    }

    /// Used during finalization to insert details that should not impact the
    /// imbalance lists. This is because they are being inserted to correct the
    /// imbalances themselves. TODO Consider a better name for this?
    fn add_balancing_detail(
        &mut self,
        account: String,
        amount: Scalar,
        currency: String,
    ) -> Result<(), Error> {
        if self.is_finalized {
            bail!("entry already finalized")
        }

        if account.len() == 0 {
            bail!("account is blank")
        }

        self.details.push(Detail {
            account,
            amount,
            currency,
            cost_basis: None,
        });


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
        let cost_basis_imbalances = self.get_cost_basis_imbalances();

        let infer_rates = cost_basis_imbalances.len() == 0;

        // TODO: I think the cloning here is not optimal.
        if let Some(vd) = &self.virtual_detail.clone() {
            for (currency, amount) in cost_basis_imbalances {
                self.add_balancing_detail(vd.clone(), -amount, currency.clone())?;
                self.add_balancing_detail(VIRTUAL_CONVERSION_ACCOUNT.to_string(), amount, currency)?
            }
        }

        let mut imbalances = self.get_imbalances();

        match imbalances.len() {
            0 => return Ok(()),
            1 => {
                if infer_rates {
                    let (currency, amount) = imbalances.pop().unwrap();
                    if let Some(vd) = &self.virtual_detail {
                        self.handle_virtual_detail(vd.clone(), currency, amount)
                    }
                } else {
                    // TODO: Need to net the imbalance specifically in the 
                    //  conversions account, because shenanigans. By the end of
                    //  today the logic will be right, but man if this isn't
                    //  confusing.
                    let (currency, amount) = imbalances.pop().unwrap();
                    self.add_balancing_detail(VIRTUAL_CONVERSION_ACCOUNT.to_string(), -amount, currency)?;
                }
            }
            2 => {
                if infer_rates && self.virtual_detail.is_some() {
                    bail!("unbalanced entry")
                }
                
                self.implicit_conversions_check(imbalances, rates, infer_rates)?;
            }
            _ => bail!("unbalanced entry")
        }

        Ok(())
    }

    fn implicit_conversions_check(&mut self, mut imbalances: Vec<(String, Scalar)>, rates: &mut ExchangeRates, infer_rates: bool) -> Result<(), Error> {
        // Assume implicit currency conversion if exactly two currencies are
        // unbalanced and in opposite directions.
        if imbalances.len() == 2 {
            let (currency1, amount1) = imbalances.remove(0);
            let (currency2, amount2) = imbalances.remove(0);

            if infer_rates {
                if (amount1 < 0 && amount2 < 0)
                    || (amount1 > 0 && amount2 > 0) {
                    bail!("Unbalanced entry")
                }
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

            // This implies an exchange rate between the currencies, except in
            // some cases where we've entered reconciling details manually and
            // should not make assumptions here
            if infer_rates {
                rates.infer(
                    self.date,
                    currency2,
                    currency1,
                    (amount1 / amount2).abs(),
                )?;
            }

            self.details.push(virtual_detail1);
            self.details.push(virtual_detail2);
        }

        Ok(())
    }

    fn handle_virtual_detail(&mut self, account: String, currency: String, amount: Scalar) {
        // We consume the virtual detail within, in all logical branches.
        self.virtual_detail = None;

        // This section TODO needs to be properly manpaged etc.
        // What we do here is that, if we have a virtual detail and the
        // only other detail in the entry has a cost basis, then we do
        // a bit of syntactic sugar and assume the user has exchanged
        // the cost basis currency and amount.
        // TODO: I have not thought about how this handles the cases I set up
        //  today.
        if self.details.len() == 1 && self.details.first().unwrap().cost_basis.is_some() {
            let details = self.details.first().unwrap();

            let cb = details.cost_basis.as_ref().unwrap();

            let mut special_entry = -cb.unit_price;
            special_entry *= details.amount;

            self.details.push(Detail {
                account: account.clone(),
                amount: special_entry,
                currency: cb.currency.clone(),
                cost_basis: None,
            });

            // TODO: After this, on this path, the implicit conversions
            //  correction needs to be run again, because we've just created an
            //  imbalanced entry on purpose.
        } else {
            let new_detail = Detail {
                account: account.clone(),
                amount: -amount,
                currency,
                cost_basis: None,
            };

            self.details.push(new_detail);
        };
    }

    /// Find all currencies in the entry that do not sum to zero, with amounts
    fn get_imbalances(&self) -> Vec<(String, Scalar)> {
        // Collect the retained elements into a Vec
        self.totals.iter().filter_map(|(k, &v)| {
            if v != 0 {
                Some((k.clone(), v))
            } else {
                None
            }
        }).collect()
    }

    /// Find all cost bases in the entry that do not sum to zero, with amounts
    fn get_cost_basis_imbalances(&self) -> Vec<(String, Scalar)> {
        // Collect the retained elements into a Vec
        self.cost_basis_totals.iter().filter_map(|(k, &v)| {
            if v != 0 {
                Some((k.clone(), v))
            } else {
                None
            }
        }).collect()
    }
}

#[derive(Clone, Debug)]
pub struct Detail {
    pub account: String,
    pub amount: Scalar,
    currency: String,

    cost_basis: Option<CostBasis>,
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

#[derive(Clone, Debug)]
pub struct CostBasis {
    pub unit_price: Scalar,
    pub currency: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::scalar::Scalar;
    use crate::tabulation::exchange_rate::ExchangeRates;
    use crate::util::date::Date;

    // Helper function to create a sample Date for testing
    fn sample_date(offset: u8) -> Date {
        Date::from_ymd(2024, 1, 1 + offset)
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
