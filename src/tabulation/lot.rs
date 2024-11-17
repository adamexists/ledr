use std::cmp::Ordering;
use std::collections::HashMap;
use anyhow::{bail, Error};
use crate::util::scalar::Scalar;
use crate::util::date::{Date, Duration};

// TODO: Reorganize this whole file. It needs to be cleaned up a lot.

#[derive(Default)]
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
        cost_basis: (String, String),
    ) -> Result<(), Error> {
        let amt = Scalar::new(&*cost_basis.0)?;

        let movement = Movement {
            action: if quantity > 0f64 { LotAction::BUY } else { LotAction::SELL },
            date,
            account,
            commodity,
            quantity: if quantity > 0f64 { quantity } else { -quantity },
            unit_price: amt,
            currency: cost_basis.1,
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
                LotAction::BUY => {
                    let lot = Lot {
                        status: LotStatus::OPEN,
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
                        .or_insert_with(Vec::new)
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
                LotAction::SELL => {
                    let lots = self.state.get_mut(&movement.commodity);
                    if let Some(lots) = lots {
                        let mut remaining_quantity = movement.quantity.clone();

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

                            if lot.status == LotStatus::CLOSED {
                                continue;
                            }

                            // Determine how much can be sold from this lot
                            let sell_quantity = remaining_quantity.min(lot.quantity.clone());
                            lot.quantity -= sell_quantity.clone();
                            remaining_quantity -= sell_quantity;

                            // Register the sale against the lot
                            lot.sales.push(Sale {
                                date: movement.date,
                                quantity: sell_quantity,
                                unit_price: movement.unit_price,
                            });

                            // If the lot is fully sold, mark it as closed
                            if lot.quantity == 0f64 {
                                lot.status = LotStatus::CLOSED;
                                lot.closed_date = Some(movement.date);
                            }

                            // Break if we've sold everything needed
                            if remaining_quantity == 0f64 {
                                break;
                            }
                        }

                        // Handle any remaining quantity that couldn't be matched
                        if remaining_quantity > 0f64 {
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
                    (LotAction::BUY, LotAction::SELL) => Some(Ordering::Less),
                    (LotAction::SELL, LotAction::BUY) => Some(Ordering::Greater),
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

#[derive(PartialEq)]
enum LotAction {
    BUY,
    SELL,
}

#[derive(PartialEq)]
pub enum LotStatus {
    OPEN,
    CLOSED,
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
        self.acquisition_date.until(&end)
    }
}

struct Sale {
    date: Date,
    quantity: Scalar,
    unit_price: Scalar,
}
