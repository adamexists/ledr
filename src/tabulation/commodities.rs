use crate::tabulation::money::Money;
use crate::util::date::Date;

pub struct Lot {
    status: LotStatus,
    account: Vec<String>,

    commodity: String,
    quantity: Money,

    acquisition_date: Date,
    acquisition_unit_cost: Money,
    acquisition_currency: String,

    closed_date: Option<Date>,
    latest_unit_value: Money,
    latest_unit_value_date: Date,
}

pub enum LotStatus {
    OPEN,
    CLOSED,
}

impl Lot {
    pub fn cost_basis(&self) -> Money {
        self.acquisition_unit_cost * self.quantity
    }

    // pub fn time_held(&self) ->
}