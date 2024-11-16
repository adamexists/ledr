use anyhow::Error;
use crate::models::amount::Amount;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Currency {
    symbol: String,
    cost_basis: Option<Amount>, // signifies this is a commodity if present
}

impl Currency {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            cost_basis: None,
        }
    }

    pub fn add_cost_basis(&mut self, amount: String, ident: u32) -> Result<(), Error> {
        self.cost_basis = Some(Amount::new_from_str(amount, ident)?);
        Ok(())
    }

    pub fn cost_basis(&self) -> Option<&Amount> {
        match &self.cost_basis {
            Some(a) => Some(a),
            None => None,
        }
    }

    pub fn print_cost_basis(&self, symbol: &String) -> Option<String> {
        match &self.cost_basis {
            Some(a) => {
                Some(format!("@ {} {}", a, symbol))
            }
            None => None,
        }
    }

    pub fn symbol(&self) -> &String {
        &self.symbol
    }
}
