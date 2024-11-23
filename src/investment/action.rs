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
use crate::util::amount::Amount;
use crate::util::date::Date;
use crate::util::scalar::Scalar;
use anyhow::{bail, Error};
use std::cmp::Ordering;

/// Represents a buy or sell that was recorded by the user. Aggregated into a
/// series of lots. We gather all actions before tabulating them into lots,
/// because we do not require ledger input to be in order.
#[derive(Clone, Debug)]
pub struct Action {
	pub direction: Direction,
	pub date: Date,

	pub account: String,
	pub commodity: Commodity,
	pub quantity: Scalar,
}

impl Action {
	pub fn new(
		date: Date,
		account: String,
		amount: Amount,
		cost_basis: Amount,
	) -> Result<Self, Error> {
		if amount.value == 0 {
			bail!("Action cannot have zero quantity")
		}

		let (direction, quantity) = if amount.value > 0 {
			(Direction::Buy, amount.value)
		} else {
			(Direction::Sell(None), -amount.value)
		};

		Ok(Self {
			direction,
			date,
			account,
			quantity,
			commodity: Commodity::new(amount.currency, cost_basis),
		})
	}

	/// Adds unit proceeds to this if it is a sell. Panics if this is not
	/// a sell or if unit proceeds have already been added to it.
	pub fn add_unit_proceeds(&mut self, unit_proceeds: Amount) {
		if self.direction != Direction::Sell(None) {
			panic!("Buy action cannot have unit proceeds")
		}

		self.direction = Direction::Sell(Some(unit_proceeds));
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Direction {
	Buy,

	/// Contains unit proceeds for the sale, if known, which can then be
	/// used on reports to calculate profit & loss, etc., against a lot.
	Sell(Option<Amount>),
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
					(
						Direction::Buy,
						Direction::Sell(_),
					) => Some(Ordering::Less),
					(
						Direction::Sell(_),
						Direction::Buy,
					) => Some(Ordering::Greater),
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
