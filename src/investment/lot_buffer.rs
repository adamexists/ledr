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
use crate::investment::lot_state::LotState;
use crate::util::amount::Amount;
use crate::util::cost_basis::CostBasis;
use crate::util::date::Date;
use crate::util::scalar::Scalar;
use anyhow::{bail, Error};
use std::cmp::Ordering;

/// Stores actions related to lots until they are all in, at which time this
/// sorts them and assembles a LotState for inspection. TODO rename this
#[derive(Debug, Default)]
pub struct LotBuffer {
	actions: Vec<Action>, // all actions, unordered
}

impl LotBuffer {
	/// Adds an action to the lot buffer. Should only be called if
	/// cost_basis on the passed Amount is Some, or this will panic.
	pub fn add_action(
		&mut self,
		date: Date,
		account: String,
		amount: Amount,
		cost_basis: CostBasis,
	) -> Result<(), Error> {
		if amount.value == 0 {
			bail!("Lot cannot have zero quantity")
		}

		let (direction, quantity) = if amount.value > 0 {
			(Direction::Buy, amount.value)
		} else {
			(Direction::Sell, -amount.value)
		};

		self.actions.push(Action {
			direction,
			date,
			account,
			quantity,
			commodity: Commodity::new(amount.currency, cost_basis),
		});
		Ok(())
	}

	/// Aggregates all actions into lots, in chronological order by date.
	/// On the same date, all buys come before all sells, to account for
	/// order of appearance not being guaranteed. Fails if a Sell action
	/// has no corresponding lot from which to sell, else succeeds and
	/// results in a set of lots.
	pub fn tabulate(&mut self, as_of: &Date) -> Result<LotState, Error> {
		self.sort_actions(as_of);

		let mut state = LotState::new();

		for action in &mut self.actions {
			match action.direction {
				Direction::Buy => {
					state.buy_lot(action);
				},
				Direction::Sell => {
					state.sell_lot(action)?;
				},
			}
		}
		Ok(state)
	}

	fn sort_actions(&mut self, as_of: &Date) {
		self.actions.sort_by(|a, b| {
			a.partial_cmp(b).unwrap_or(Ordering::Equal)
		});
		self.actions.retain(|m| &m.date <= as_of);
	}
}

/// Represents a buy or sell that was recorded by the user. Aggregated into a
/// series of lots. We gather all actions before tabulating them into lots,
/// because we do not require ledger input to be in order.
#[derive(Debug)]
pub struct Action {
	pub direction: Direction,
	pub date: Date,

	pub account: String,
	pub commodity: Commodity,
	pub quantity: Scalar,
}

impl PartialEq for Action {
	fn eq(&self, other: &Self) -> bool {
		self.date == other.date
			&& self.direction == other.direction
			&& self.commodity == other.commodity
	}
}

impl PartialOrd for Action {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		// Compare by date first
		match self.date.partial_cmp(&other.date) {
			Some(Ordering::Equal) => {
				// If dates are equal, sort buys before sells
				match (&self.direction, &other.direction) {
					(Direction::Buy, Direction::Sell) => {
						Some(Ordering::Less)
					},
					(Direction::Sell, Direction::Buy) => {
						Some(Ordering::Greater)
					},
					_ => {
						// Lastly, use commodity string
						self.commodity.partial_cmp(
							&other.commodity,
						)
					},
				}
			},
			non_equal => non_equal,
		}
	}
}

#[derive(Debug, PartialEq)]
pub enum Direction {
	Buy,
	Sell,
}
