use std::cmp::PartialEq;
use std::collections::HashMap;
use anyhow::{bail, Error};
use crate::tabulation::exchange_rate::RateType::{DECLARED, INFERRED};
use crate::util::date::Date;
use crate::util::scalar::Scalar;

#[derive(Debug, Default)]
pub struct ExchangeRates {
    /// Store rates with a tuple of (base, quote) as the key
    rates: HashMap<(String, String), Vec<ExchangeRate>>,
}

impl ExchangeRates {
    pub fn new() -> Self {
        Default::default()
    }

    /// Add a new exchange rate declared via directive. Might fail if there is
    /// an existing declared rate on the same date.
    pub fn declare(&mut self, date: Date, base: String, quote: String, mut rate: Scalar) -> Result<(), Error> {
        if base == quote {
            bail!("cannot exchange a currency for itself")
        }
        if rate <= 0 {
            bail!("exchange rate must be positive")
        }

        // to standardize lookups, base should be alphabetically before quote
        let key = if base < quote {
            (base, quote)
        } else {
            rate = 1 / rate;
            (quote, base)
        };

        if self.get_exact_rate(&key, date, DECLARED).is_some() {
            bail!("cannot declare multiple exchange rates on same date")
        }
        let new_rate = ExchangeRate::new(date, DECLARED, rate);

        // We do not need to check for existing inferred rates, because all
        // directives are calculated first, so one cannot exist.

        self.rates.entry(key.clone()).or_insert_with(Vec::new).push(new_rate);
        self.rates.entry(key).
            and_modify(|e| e.sort_by(|a, b| b.date.cmp(&a.date)));
        Ok(())
    }

    /// Add a new exchange rate inferred from an entry. Might fail if there is
    /// an existing declared rate that is outside tolerance from this new rate.
    /// If there is an existing declared rate at all, this one will definitely
    /// be ignored.
    pub fn infer(&mut self, date: Date, base: String, quote: String, mut rate: Scalar) -> Result<(), Error> {
        if base == quote {
            bail!("cannot exchange a currency for itself")
        }
        if rate <= 0 {
            bail!("exchange rate must be positive");
        }

        // to standardize lookups, base should be alphabetically before quote
        let key = if base < quote {
            (base, quote)
        } else {
            rate = 1 / rate;
            (quote, base)
        };

        if let Some(declared) = self.get_exact_rate(&key, date, DECLARED) {
            // Check if the inferred rate is within 1% of the declared rate. If
            // it is, ignore this inferred rate and use the declared; if not,
            // then the declared rate is too far from reality on this date to be
            // accurate, so we should error to stop tabulation here.
            if !within_tolerance_of(Scalar::new(1, 2), declared, rate) {
                bail!("inferred exchange rate deviates >1% from declared rate")
            }

            return Ok(());
        }

        let new_rate = ExchangeRate::new(date, INFERRED, rate);
        self.rates.entry(key.clone()).or_insert_with(Vec::new).push(new_rate);
        self.rates.entry(key).
            and_modify(|e| e.sort_by(|a, b| b.date.cmp(&a.date)));
        Ok(())
        
    }

    /// Retrieve the most recent rate before a given date, if any
    pub fn get_effective_rate_on(
        &self,
        date: Date,
        base: String,
        quote: String,
    ) -> Option<Scalar> {
        let mut invert_rate = false;
        let key = if base < quote {
            (base, quote)
        } else {
            invert_rate = true;
            (quote, base)
        };

        self.rates.get(&key)
            .and_then(|rates| {
                rates.iter().find(|rate| rate.date <= date)
            })
            .and_then(|r| Some(r.rate))
            .and_then(|found| {
                if invert_rate {
                    Some(1 / found)
                } else {
                    Some(found)
                }
            })
    }

    /// Retrieve the most recent rate available, if any
    pub fn get_latest_rate(
        &self,
        base: String,
        quote: String,
    ) -> Option<Scalar> {
        let mut invert_rate = false;
        let key = if base < quote {
            (base, quote)
        } else {
            invert_rate = true;
            (quote, base)
        };

        self.rates.get(&key)
            .and_then(|rates| rates.first())
            .and_then(|r| Some(r.rate))
            .and_then(|found| {
                if invert_rate {
                    Some(1 / found)
                } else {
                    Some(found)
                }
            })
    }

    /// Returns a rate that already exists for the *exact* passed date, if any.
    fn get_exact_rate(&self, key: &(String, String), date: Date, rate_type: RateType) -> Option<Scalar> {
        self.rates.get(key)
            .and_then(|rates| {
                rates.iter()
                    .find(|rate| rate.date == date && rate.rate_type == rate_type)
            })
            .and_then(|r| Some(r.rate))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RateType {
    /// the user said this is true
    DECLARED,
    /// we inferred this rate from an entry
    INFERRED,
}

#[derive(Clone, Debug)]
struct ExchangeRate {
    date: Date,
    rate_type: RateType,

    rate: Scalar,
}

impl ExchangeRate {
    fn new(date: Date, rate_type: RateType, rate: Scalar) -> Self {
        Self {
            date,
            rate_type,
            rate,
        }
    }
}

/// returns true iff a and b are within percent of each other.
/// Percent should be presented as a decimal form, e.g. 0.01 == 1%.
fn within_tolerance_of(percent: Scalar, a: Scalar, b: Scalar) -> bool {
    (a - b).abs() <= percent * a.abs().max(b.abs())
}
