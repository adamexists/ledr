use std::collections::HashMap;
use anyhow::{bail, Error};
use chrono::NaiveDate;
use crate::models::entry::{Detail, Entry};
use crate::models::total::Total;

#[derive(Default)]
pub struct Ledger {
    entries: Vec<Entry>,

    pending_entry: Option<Entry>, // entry currently being assembled, if any

    // currency -> the earliest date currency is allowed to appear
    declared_currencies: HashMap<String, NaiveDate>,
}

impl Ledger {
    pub fn new() -> Self {
        Default::default()
    }

    // -----------
    // -- INPUT --
    // -----------

    pub fn declare_currency(&mut self, currency: String, date: NaiveDate) -> Result<(), Error> {
        if self.declared_currencies.contains_key(&currency) {
            bail!("currency {} declared twice", currency)
        }

        self.declared_currencies.insert(currency.clone(), date);

        Ok(())
    }

    pub fn new_entry(&mut self, date: NaiveDate, desc: String) -> Result<(), Error> {
        if self.pending_entry.is_some() {
            self.finish_entry()?;
        }

        self.pending_entry = Some(Entry::new(date, desc));
        Ok(())
    }

    pub fn add_detail(&mut self, account: String, amount: String, mut currency: String, cost_basis: Option<(String, String)>) -> Result<(), Error> {
        let declaration_date = match self.declared_currencies.get(&currency) {
            Some(d) => d,
            None => bail!("currency {} used without declaration", currency)
        };

        if self.pending_entry.is_none() {
            bail!("orphaned entry detail")
        }

        if self.pending_entry.as_ref().unwrap().get_date() < declaration_date {
            bail!("currency {} used prior to declaration on {}", currency, declaration_date)
        }

        self.pending_entry.as_mut().unwrap().add_detail(account, amount, currency, cost_basis)
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
                entry.finalize()?;
                self.entries.push(entry);
                Ok(())
            }
        }
    }

    // ----------------
    // -- TABULATING --
    // ----------------

    // Consumes the Ledger, transforming it into a Total.
    pub fn to_totals(self) -> Total {
        let mut total = Total::new();

        let all_details: Vec<Detail> = self.entries
            .into_iter()
            .flat_map(|e| e.take_details())
            .collect();

        total.ingest_details(&all_details);
        total
    }
}
