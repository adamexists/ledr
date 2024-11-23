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
use crate::investment::commodity::Commodity;
use crate::investment::lot::{Lot, LotStatus, Sale};
use crate::investment::lot_buffer::Action;
use crate::util::date::Date;
use anyhow::{bail, Error};
use std::collections::HashMap;

/// TODO rename this and write a proper description
pub struct LotState {
	state: HashMap<Commodity, Vec<Lot>>, // commodity -> its lots
}

impl LotState {
	pub fn new() -> Self {
		Self {
			state: Default::default(),
		}
	}

	pub fn buy_lot(&mut self, action: &Action) {
		self.state
			.entry(action.commodity.clone())
			.or_default()
			.push(Lot {
				status: LotStatus::Open,
				account: action.account.clone(),
				commodity: action.commodity.clone(),
				quantity: action.quantity,
				acquisition_date: action.date,
				closed_date: None,
				sales: vec![],
			})
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
	pub fn sell_lot(&mut self, action: &Action) -> Result<(), Error> {
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

			// Handle any remaining quantity that couldn't be matched
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

	/// Flattens the set of lots into one Vec, and returns it.
	/// Consumes this.
	pub fn take_lots(
		self,
		ignore_after: Option<Date>,
		only_open: bool,
	) -> Vec<Lot> {
		let lots_iter = self.state.into_values().flatten();
		if only_open {
			lots_iter
				.filter(|lot| lot.status == LotStatus::Open)
				.collect()
		} else {
			lots_iter.collect()
		}
	}
}
