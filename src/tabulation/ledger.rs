use std::cmp::min;
use std::collections::HashMap;
use anyhow::{bail, Error};
use crate::tabulation::total::Total;
use crate::tabulation::entry::{CostBasis, Detail, Entry};
use crate::tabulation::exchange_rate::ExchangeRates;
use crate::tabulation::ledger::CostBasisAmountType::TotalCost;
use crate::tabulation::lot::Lots;
use crate::util::scalar::Scalar;
use crate::util::date::Date;

pub const VALID_PREFIXES: [&'static str; 5] =
    ["Assets", "Liabilities", "Equity", "Income", "Expenses"];

#[derive(Debug, Default)]
pub struct Ledger {
    entries: Vec<Entry>,
    /// entry currently being assembled, if any
    pending_entry: Option<Entry>,
    is_finalized: bool,

    /// currency -> the earliest date currency is allowed to appear
    declared_currencies: HashMap<String, Date>,

    pub exchange_rates: ExchangeRates,
    pub lots: Lots,
}

impl Ledger {
    pub fn new() -> Self {
        Default::default()
    }

    // -----------
    // -- INPUT --
    // -----------

    pub fn declare_currency(
        &mut self,
        currency: String,
        date: Date,
    ) -> Result<(), Error> {
        if self.declared_currencies.contains_key(&currency) {
            bail!("currency {} declared twice", currency)
        }

        self.declared_currencies.insert(currency.clone(), date);

        Ok(())
    }

    pub fn new_entry(
        &mut self,
        date: Date,
        desc: String,
    ) -> Result<(), Error> {
        if self.pending_entry.is_some() {
            self.finish_entry()?;
        }

        self.pending_entry = Some(Entry::new(date, desc));
        Ok(())
    }

    pub fn add_detail(
        &mut self,
        account: String,
        amount: String,
        currency: String,
        cost_basis: Option<CostBasisInput>,
    ) -> Result<(), Error> {
        self.check_currency(&currency)?;
        if let Some(cost_basis_currency) = &cost_basis {
            self.check_currency(&cost_basis_currency.currency)?;
        }

        if self.pending_entry.is_none() {
            bail!("orphaned entry detail")
        }

        if account.is_empty() {
            bail!("invalid account: empty")
        }

        if amount.is_empty() {
            bail!("invalid amount: empty")
        }

        let has_valid_prefix = VALID_PREFIXES
            .iter()
            .any(|&prefix| account.starts_with(prefix));
        if !has_valid_prefix {
            bail!("invalid account prefix: {}", account)
        }

        let money_amt = Scalar::from_str(&*amount)?;
        let mut cost_basis_input = None;

        // TODO: Could still use another refactor eventually.
        if let Some(mut cb) = cost_basis {
            if cb.amount_type == TotalCost {
                cb.amount /= money_amt;
            }
            // TODO: Investigate whether this is necessary anymore, now that
            //  finalizing an entry has been refactored.
            self.exchange_rates.infer(
                self.pending_entry_date(),
                currency.clone(),
                cb.currency.clone(),
                cb.amount,
            )?;

            self.lots.add_movement(
                self.pending_entry_date(),
                account.clone(),
                currency.clone(),
                money_amt,
                CostBasis {
                    currency: cb.currency.clone(),
                    unit_price: cb.amount,
                    associated_amount: money_amt, // TODO: Silly this is here
                    associated_currency: currency.clone(), // TODO: Silly this is here
                },
            )?;

            cost_basis_input = Some(CostBasis {
                unit_price: cb.amount,
                currency: cb.currency,
                associated_amount: money_amt,
                associated_currency: currency.clone(),
            });
        }

        self.pending_entry
            .as_mut()
            .unwrap()
            .add_detail(account, money_amt.clone(), currency, cost_basis_input)
    }

    pub fn set_virtual_detail(&mut self, account: String) -> Result<(), Error> {
        if self.pending_entry.is_none() {
            bail!("orphaned entry detail")
        }

        self.pending_entry.as_mut().unwrap().set_virtual_detail(account)
    }

    pub fn finish_entry(&mut self) -> Result<(), Error> {
        match self.pending_entry.take() {
            None => Ok(()),
            Some(mut entry) => {
                entry.finalize(&mut self.exchange_rates)?;
                self.entries.push(entry);
                Ok(())
            }
        }
    }

    pub fn pending_entry_date(&self) -> Date {
        match &self.pending_entry {
            Some(e) => {
                e.get_date().clone()
            }
            None => panic!("pending_entry_date called without pending entry"),
        }
    }

    fn check_currency(&self, currency: &String) -> Result<(), Error> {
        let declaration_date = match self.declared_currencies.get(currency) {
            Some(d) => d,
            None => bail!("currency {} used without declaration", currency)
        };

        if self.pending_entry.as_ref().unwrap().get_date() < declaration_date {
            bail!("currency {} used prior to declaration on {}",
                currency,
                declaration_date)
        }

        Ok(())
    }

    // ----------------
    // -- TABULATING --
    // ----------------

    /// Converts all possible balances to the currency provided, if exchange
    /// rates are available. If a rate is not available for the given pair, then
    /// we skip. There is no graph traversal: a direct rate must have been
    /// observed.
    ///
    /// TODO: Graph traversal is feasible in the future.
    pub fn collapse_to(&mut self, currency: String) {
        self.entries.iter_mut()
            .flat_map(|e| e.details())
            .for_each(|d| {
                if let Some(rate) = self.exchange_rates.get_latest_rate(
                    d.currency(), currency.clone(),
                ) {
                    d.convert_to(&currency, rate)
                }
            })
    }

    /// Removes cost basis from currencies. This is done for most reports that
    /// do not specifically care about it.
    pub fn remove_cost_basis(&mut self) {
        self.entries.iter_mut()
            .flat_map(|e| e.details())
            .for_each(|d| {
                d.remove_cost_basis();
            })
    }

    pub fn finalize(&mut self, overall_max_reso: Option<u32>) -> Result<(), Error> {
        let mut max_reso_by_currency: HashMap<String, u32> = HashMap::new();
        let mut overall_max = 6;
        if let Some(requested_max) = overall_max_reso {
            overall_max = requested_max;
        }

        // Iterate over each detail to determine the highest resolution per currency
        for detail in self.entries.iter().flat_map(|x| x.get_details()) {
            let reso = min(detail.amount.resolution(), overall_max);
            let currency = detail.currency();

            // Update max resolution if this detail has higher resolution
            max_reso_by_currency
                .entry(currency.clone())
                .and_modify(|max_reso| {
                    if reso > *max_reso {
                        *max_reso = reso;
                    }
                })
                .or_insert(reso);
        }

        // Standardize all currencies to the highest precision found among them
        for entry in &mut self.entries {
            for (currency, &reso) in &max_reso_by_currency {
                entry.set_resolution_for_currency(currency, reso)?
            }
        }

        self.is_finalized = true;
        Ok(())
    }

    /// Consumes the Ledger, transforming it into a Total.
    pub fn to_totals(self) -> Result<Total, Error> {
        if !self.is_finalized {
            bail!("ledger not marked as finalized")
        }

        let mut total = Total::new();

        let all_details: Vec<Detail> = self.entries
            .into_iter()
            .flat_map(|e| e.take_details())
            .collect();

        total.ingest_details(&all_details);
        Ok(total)
    }
}

#[derive(Debug, PartialEq)]
pub enum CostBasisAmountType {
    UnitCost,
    TotalCost,
}

#[derive(Debug)]
pub struct CostBasisInput {
    pub(crate) amount: Scalar,
    pub(crate) amount_type: CostBasisAmountType,

    pub(crate) currency: String,
}
