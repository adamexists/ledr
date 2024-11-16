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

    pub fn symbol(&self) -> &String {
        &self.symbol
    }

    pub fn is_commodity(&self) -> bool {
        self.cost_basis.is_some()
    }
}
