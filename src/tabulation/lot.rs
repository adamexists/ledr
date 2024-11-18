use std::cmp::Ordering;
use std::collections::HashMap;
use anyhow::{bail, Error};
use crate::tabulation::entry::CostBasis;
use crate::util::scalar::Scalar;
use crate::util::date::{Date, Duration};

// TODO: Reorganize this whole file. It needs to be cleaned up a lot.

#[derive(Debug, Default)]
pub struct Lots {
    state: HashMap<String, Vec<Lot>>, // currency -> all lots of that currency
    movements: Vec<Movement>, // all movements, unordered
}

impl Lots {
    pub fn add_movement(
        &mut self,
        date: Date,
        account: String,
        commodity: String,
        quantity: Scalar,
        cost_basis: CostBasis,
    ) -> Result<(), Error> {

        let movement = Movement {
            action: if quantity > 0 { LotAction::Buy } else { LotAction::Sell },
            date,
            account,
            commodity,
            quantity: if quantity > 0 { quantity } else { -quantity },
            unit_price: cost_basis.unit_price,
            currency: cost_basis.currency,
        };

        self.movements.push(movement);
        Ok(())
    }

    pub fn tabulate(&mut self, as_of: &Date) -> Result<(), Error> {
        self.movements.sort_by(
            |a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal)
        );
        self.movements.retain(|m| &m.date <= as_of);

        for movement in &self.movements {
            match movement.action {
                LotAction::Buy => {
                    let lot = Lot {
                        status: LotStatus::Open,
                        account: movement.account.clone(),
                        commodity: movement.commodity.clone(),
                        quantity: movement.quantity,
                        acquisition_date: movement.date,
                        acquisition_unit_cost: movement.unit_price,
                        acquisition_currency: movement.currency.clone(),
                        closed_date: None,
                        sales: Vec::new(),
                    };

                    self.state
                        .entry(movement.commodity.clone())
                        .or_default()
                        .push(lot);
                }
                // TODO: Currently we use FIFO only; we could expand this to
                //  point to specific shares based on minimizing spread in
                //  consideration of specific capital gains tax thresholds,
                //  but that would probably require a config file and a number
                //  of disclaimers. For now, we know we're doing FIFO because
                //  we iterate chronologically through dates, first with all
                //  buys and then with all sells on that date. So by the time
                //  we process a sell, all the buys are in, in chronological
                //  order.
                LotAction::Sell => {
                    let lots = self.state.get_mut(&movement.commodity);
                    if let Some(lots) = lots {
                        let mut remaining_quantity = movement.quantity;

                        for lot in lots.iter_mut() {
                            if lot.acquisition_currency != movement.currency {
                                // TODO: Right now we cannot match lots if they
                                //  were bought & sold in different currencies,
                                //  but it's a matter of calculating the gains
                                //  and losses, so it might come when there is a
                                //  beautiful exchange rates graph data
                                //  structure... :)
                                continue;
                            }

                            if lot.status == LotStatus::Closed {
                                continue;
                            }

                            // Determine how much can be sold from this lot
                            let sell_quantity = remaining_quantity.min(lot.quantity);
                            lot.quantity -= sell_quantity;
                            remaining_quantity -= sell_quantity;

                            // Register the sale against the lot
                            lot.sales.push(Sale {
                                date: movement.date,
                                quantity: sell_quantity,
                                unit_price: movement.unit_price,
                            });

                            // If the lot is fully sold, mark it as closed
                            if lot.quantity == 0 {
                                lot.status = LotStatus::Closed;
                                lot.closed_date = Some(movement.date);
                            }

                            // Break if we've sold everything needed
                            if remaining_quantity == 0 {
                                break;
                            }
                        }

                        // Handle any remaining quantity that couldn't be matched
                        if remaining_quantity > 0 {
                            bail!(
                                "not enough lots to sell remaining {} of {} on {}",
                                remaining_quantity, movement.commodity,
                                movement.date,
                            );
                        }
                    } else {
                        // No lots available to sell
                        bail!(
                            "no lots found for commodity {} to sell",
                            movement.commodity
                        );
                    }
                }
            }
        }

        println!("report coming soon!");
        Ok(())
    }
}

#[derive(Debug)]
struct Movement {
    action: LotAction,
    date: Date,

    account: String,
    commodity: String,
    quantity: Scalar,
    unit_price: Scalar, // amount exchanged per unit of this lot
    currency: String, // currency in which cost is denominated
}

impl PartialEq for Movement {
    fn eq(&self, other: &Self) -> bool {
        self.date == other.date &&
            self.action == other.action &&
            self.commodity == other.commodity
    }
}

impl PartialOrd for Movement {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Compare by date first
        match self.date.partial_cmp(&other.date) {
            Some(Ordering::Equal) => {
                // If dates are equal, sort buys before sells TODO: manpage this
                match (&self.action, &other.action) {
                    (LotAction::Buy, LotAction::Sell) => Some(Ordering::Less),
                    (LotAction::Sell, LotAction::Buy) => Some(Ordering::Greater),
                    _ => {
                        // If actions are the same, compare by commodity string
                        self.commodity.partial_cmp(&other.commodity)
                    }
                }
            }
            non_equal => non_equal,
        }
    }
}

#[derive(Debug)]
pub struct Lot {
    status: LotStatus,
    account: String,

    commodity: String,
    quantity: Scalar, // always in positive terms, even for sales

    acquisition_date: Date,
    acquisition_unit_cost: Scalar,
    acquisition_currency: String,

    closed_date: Option<Date>,
    sales: Vec<Sale>,
}

#[derive(Debug, PartialEq)]
enum LotAction {
    Buy,
    Sell,
}

#[derive(Debug, PartialEq)]
pub enum LotStatus {
    Open,
    Closed,
}

impl Lot {
    pub fn cost_basis(&self) -> Scalar {
        self.acquisition_unit_cost * self.quantity
    }

    pub fn time_held(&self, as_of: &Date) -> Duration {
        let end = match self.closed_date {
            Some(date) => if as_of < &date { as_of } else { &date.clone() },
            None => as_of
        };
        self.acquisition_date.until(end)
    }
}

#[derive(Debug)]
struct Sale {
    date: Date,
    quantity: Scalar,
    unit_price: Scalar,
}
