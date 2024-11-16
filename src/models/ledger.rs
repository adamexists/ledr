use std::collections::HashMap;
use anyhow::{bail, Error};
use chrono::NaiveDate;
use crate::models::currency::Currency;
use crate::models::entry::{Detail, Entry};
use crate::models::total::Total;

#[derive(Default)]
pub struct Ledger {
    entries: Vec<Entry>,

    pending_entry: Option<Entry>, // entry currently being assembled, if any

    // symbol -> the earliest date currency is allowed to appear
    declared_currencies: HashMap<String, NaiveDate>,

    currency_lookup: HashMap<u32, Currency>, // ident -> currency
    ident_lookup: HashMap<Currency, u32>, // currency -> ident
    next_ident: u32,
}

impl Ledger {
    pub fn new() -> Self {
        Default::default()
    }

    // -----------
    // -- INPUT --
    // -----------

    pub fn declare_currency(&mut self, symbol: String, date: NaiveDate) -> Result<(), Error> {
        if self.declared_currencies.contains_key(&symbol) {
            bail!("currency {} declared twice", symbol)
        }

        self.declared_currencies.insert(symbol.clone(), date);
        self.currency_lookup.insert(self.next_ident, Currency::new(symbol.clone()));
        self.ident_lookup.insert(Currency::new(symbol), self.next_ident);
        self.next_ident += 1;

        Ok(())
    }

    pub fn new_entry(&mut self, date: NaiveDate, desc: String) -> Result<(), Error> {
        if self.pending_entry.is_some() {
            self.finish_entry()?;
        }

        self.pending_entry = Some(Entry::new(date, desc));
        Ok(())
    }

    pub fn add_detail(&mut self, account: String, amount: String, currency: Currency) -> Result<(), Error> {
        let declaration_date = match self.declared_currencies.get(currency.symbol()) {
            Some(d) => d,
            None => bail!("currency {} used without declaration", currency.symbol())
        };

        if self.pending_entry.is_none() {
            bail!("orphaned entry detail")
        }

        if self.pending_entry.as_ref().unwrap().get_date() < declaration_date {
            bail!("currency {} used prior to declaration on {}", currency.symbol(), declaration_date)
        }

        let ident = match self.ident_lookup.get(&currency) {
            None => {
                // first time we've seen this currency with this cost basis; add
                self.currency_lookup.insert(self.next_ident, currency.clone());
                self.ident_lookup.insert(currency, self.next_ident);
                self.next_ident += 1;

                self.next_ident - 1
            }
            Some(&i) => i
        };

        self.pending_entry.as_mut().unwrap().add_detail(account, amount, ident)
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

        total.ingest_details(&self.currency_lookup, &all_details);
        total
    }
}
