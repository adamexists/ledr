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

use crate::tabulation::exchange_rate::ExchangeRates;
use crate::util::date::Date;
use crate::util::scalar;
use crate::util::scalar::Scalar;
use anyhow::{bail, Error};
use std::collections::HashMap;
use std::string::ToString;

const VIRTUAL_CONVERSION_ACCOUNT: &str = "Equity:Conversions";

#[derive(Debug)]
pub struct Entry {
    date: Date,
    desc: String,
    details: Vec<Detail>,
    is_finalized: bool,

    virtual_detail: Option<String>,
    totals: HashMap<String, Scalar>, // Currency -> Amount
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

        if account.is_empty() {
            bail!("account is blank")
        }

        *self.totals.entry(currency.clone()).or_insert(scalar::ZERO) += amount;
        self.details.push(Detail {
            account,
            amount,
            currency,
            cost_basis,
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

        if account.is_empty() {
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
        &mut self,
        currency: &String,
        resolution: u32,
    ) -> Result<(), Error> {
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
    /// This method also handles the resolution of cost bases and their related
    /// syntactical magic, particularly decomposing it such that it has the
    /// appropriate effect on balances for the currency it was exchanged with.
    pub fn finalize(&mut self, rates: &mut ExchangeRates) -> Result<(), Error> {
        let cost_basis_details = self.get_cost_basis_details();

        let infer_rates = cost_basis_details.is_empty();

        if let Some(vd) = &self.virtual_detail.clone() {
            for mut d in cost_basis_details {
                let cbd = d.cost_basis.take().unwrap();

                // Move cost basis (after @ sign) component to virtual detail
                // account, offsetting with conversion account. Effectively, we
                // are manually creating an imbalance, but in the cost basis
                // currency. The virtual account will then absorb it in the
                // correct currency, alongside any other imbalances.
                self.add_detail(
                    vd.clone(),
                    -cbd.unit_price * cbd.associated_amount,
                    cbd.currency.clone(),
                    None,
                )?;
                self.add_detail(
                    VIRTUAL_CONVERSION_ACCOUNT.to_string(),
                    cbd.unit_price * cbd.associated_amount,
                    cbd.currency.clone(),
                    None,
                )?;
                // then net out the currency with the cost basis
                self.add_detail(
                    VIRTUAL_CONVERSION_ACCOUNT.to_string(),
                    -d.amount,
                    d.currency.clone(),
                    None,
                )?;
            }
        }

        let mut imbalances = self.get_imbalances();

        // Special case if exactly two currencies are unbalanced with no virtual
        // account, in which case we net them against each other.
        if imbalances.len() == 2 && self.virtual_detail.is_none() {
            self.multiline_implicit_currency_conversion(&mut imbalances, rates, infer_rates)?;
            self.is_finalized = true;
            return Ok(());
        }

        while let Some((currency, amount)) = imbalances.pop() {
            if let Some(vd) = &self.virtual_detail {
                let new_detail = Detail {
                    account: vd.clone(),
                    amount: -amount,
                    currency,
                    cost_basis: None,
                };

                self.details.push(new_detail);
            } else {
                bail!("unbalanced entry")
            }
        }

        self.is_finalized = true;
        Ok(())
    }

    /// This is a special case in which there is no virtual detail, but there
    /// are exactly two lines that we can net against each other if they are
    /// cardinally opposed.
    fn multiline_implicit_currency_conversion(
        &mut self,
        imbalances: &mut Vec<(String, Scalar)>,
        rates: &mut ExchangeRates,
        can_infer_rates: bool,
    ) -> Result<(), Error> {
        let (currency1, amount1) = imbalances.remove(0);
        let (currency2, amount2) = imbalances.remove(0);

        if (amount1 < 0 && amount2 < 0) || (amount1 > 0 && amount2 > 0) {
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

        // This implies an exchange rate between the currencies, except in
        // some cases related to cost basis processing where we've entered
        // reconciling details manually and should not make assumptions here.
        //
        // We use a quick hack to make the underlying integer division nicer.
        if can_infer_rates {
            if amount1.abs() > amount2.abs() {
                rates.infer(self.date, currency2, currency1, (amount1 / amount2).abs())?;
            } else {
                rates.infer(self.date, currency1, currency2, (amount2 / amount1).abs())?;
            }
        }

        self.details.push(virtual_detail1);
        self.details.push(virtual_detail2);

        Ok(())
    }

    /// Find all currencies in the entry that do not sum to zero, with amounts
    fn get_imbalances(&self) -> Vec<(String, Scalar)> {
        // Collect the retained elements into a Vec
        self.totals
            .iter()
            .filter_map(|(k, &v)| if v != 0 { Some((k.clone(), v)) } else { None })
            .collect()
    }

    /// Find all Details with cost bases in the entry.
    fn get_cost_basis_details(&self) -> Vec<Detail> {
        // Collect the retained elements into a Vec
        self.details
            .iter()
            .filter_map(|d| {
                if d.cost_basis.is_some() {
                    Some(d.clone())
                } else {
                    None
                }
            })
            .collect()
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
        self.amount *= rate;
    }

    pub fn remove_cost_basis(&mut self) {
        self.cost_basis = None
    }
}

#[derive(Clone, Debug)]
pub struct CostBasis {
    pub unit_price: Scalar,
    pub currency: String,

    pub associated_amount: Scalar,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabulation::exchange_rate::ExchangeRates;
    use crate::util::date::Date;
    use crate::util::scalar::Scalar;

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

        let mut rates = ExchangeRates::default();
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

        let mut rates = ExchangeRates::default();
        let result = entry.finalize(&mut rates);

        // Expect success since the entry is balanced
        assert!(result.is_ok());
        assert!(entry.is_finalized);
    }

    // Test for setting virtual detail
    #[test]
    fn test_set_virtual_detail() {
        let mut entry = create_entry(0);
        let result = entry.set_virtual_detail("Assets:Virtual".to_string());

        assert!(result.is_ok());
        assert!(entry.virtual_detail.is_some());
        assert_eq!(entry.virtual_detail.unwrap(), "Assets:Virtual")
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
