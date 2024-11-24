/* Copyright (C) 2024 Adam House <adam@adamexists.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */
use crate::investment::action::Action;
use crate::investment::commodity::Commodity;
use crate::investment::lot::{Lot, LotStatus};
use crate::investment::sale::Sale;
use crate::util::amount::Amount;
use anyhow::{bail, Error};
use std::collections::HashMap;

/// TODO rename this and write a proper description
pub struct LotState {
	state: HashMap<Commodity, Vec<Lot>>, // commodity -> its lots
	/// The ID number that will be assigned to the next lot
	next_id: u64,
}

impl LotState {
	pub fn new() -> Self {
		Self {
			state: Default::default(),
			next_id: 1,
		}
	}

	pub fn buy_lot(&mut self, action: &Action) {
		self.state
			.entry(action.commodity.clone())
			.or_default()
			.push(Lot {
				id: self.next_id, // TODO: Make certain this is deterministic
				status: LotStatus::Open,
				account: action.account.clone(),
				commodity: action.commodity.clone(),
				quantity: action.quantity,
				acquisition_date: action.date,
				closed_date: None,
				sales: vec![],
			});

		self.next_id += 1;
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
	// TODO: Rework the below loop to be less... wide.
	pub fn sell_lot(
		&mut self,
		action: &Action,
		unit_proceeds: Option<Amount>,
	) -> Result<(), Error> {
		let lots = self.state.get_mut(&action.commodity);
		if let Some(lots) = lots {
			let mut remaining_quantity = action.quantity;

			for lot in lots.iter_mut() {
				if lot.commodity != action.commodity
					|| lot.status == LotStatus::Closed
				{
					continue;
				}

				// Determine how much can be sold from this lot
				let sell_quantity =
					remaining_quantity.min(lot.quantity);
				lot.quantity -= sell_quantity;
				remaining_quantity -= sell_quantity;

				// Register the sale against the lot
				lot.sales.push(Sale {
					date: action.date,
					quantity: sell_quantity,
					unit_proceeds: unit_proceeds.clone(),
				});

				// If the lot is fully sold, mark it as closed
				if lot.quantity == 0 {
					lot.status = LotStatus::Closed;
					lot.closed_date = Some(action.date);
				}

				// Break if we've sold everything needed
				if remaining_quantity == 0 {
					break;
				}
			}

			if remaining_quantity > 0 {
				bail!("No remaining lots for {} of {} (cost basis {})", remaining_quantity, action.commodity.symbol(), action.commodity.cost_basis())
			}

			Ok(())
		} else {
			// No lots available to sell
			bail!(
				"No matching lots to sell {} (cost basis {})",
				action.commodity.symbol(),
				action.commodity.cost_basis()
			);
		}
	}

	/// Flattens the set of lots into one Vec, applies filters, and returns it.
	/// Consumes this.
	pub fn take_lots(
		self,
		filters: impl IntoIterator<Item = LotFilter>,
	) -> Vec<Lot> {
		// First assign IDs to all lots

		let mut lots_iter: Box<dyn Iterator<Item = Lot>> =
			Box::new(self.state.into_values().flatten());

		for filter in filters {
			lots_iter = match filter {
				LotFilter::Status(status) => {
					Box::new(lots_iter.filter(move |lot| {
						lot.status == status
					}))
				},
				LotFilter::HasSales(has_sales) => {
					Box::new(lots_iter.filter(move |lot| {
						lot.sales.is_empty()
							!= has_sales
					}))
				},
			};
		}

		lots_iter.collect()
	}
}

pub enum LotFilter {
	Status(LotStatus),
	HasSales(bool),
}
