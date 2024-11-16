use std::fmt;
use anyhow::Error;
use crate::models::money::Money;

#[derive(Clone, Hash)]
pub struct Amount {
    scalar: Money,
    currency: String,
}

impl Amount {
    pub fn new(scalar: Money, currency: String) -> Self {
        Self {
            scalar,
            currency,
        }
    }

    pub fn new_from_str(amount: String, currency: String) -> Result<Self, Error> {
        let scalar = Money::new(&*amount)?;
        Ok(Self {
            scalar,
            currency,
        })
    }

    pub fn currency(&self) -> String {
        self.currency.clone()
    }

    pub fn scalar(&self) -> Money {
        self.scalar
    }

    pub fn add_cost_basis(&mut self, amount: String, currency: String) {
        self.currency = format!("{} @ {} {}", self.currency, amount, currency);
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.scalar, self.currency)
    }
}
