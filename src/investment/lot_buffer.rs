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

use crate::investment::action::{Action, Direction};
use crate::investment::lot_state::LotState;
use crate::util::date::Date;
use anyhow::Error;
use std::cmp::Ordering;

/// Stores actions related to lots until they are all in, at which time this
/// sorts them and assembles a LotState for inspection. TODO rename this
#[derive(Debug, Default)]
pub struct LotBuffer {
	actions: Vec<Action>, // all actions, unordered
}

impl LotBuffer {
	/// Adds an action to the lot buffer.
	pub fn add_action(&mut self, action: Action) {
		self.actions.push(action);
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
			match &action.direction {
				Direction::Buy => {
					state.buy_lot(action);
				},
				Direction::Sell(proceeds) => {
					state.sell_lot(
						action,
						proceeds.clone(),
					)?;
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
