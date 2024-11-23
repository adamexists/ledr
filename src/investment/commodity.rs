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
use crate::util::cost_basis::CostBasis;
use crate::util::scalar::Scalar;
use std::fmt::Formatter;

// TODO: Resolve problems with these derivations, if any.
#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct Commodity {
	symbol: String,
	cost_basis: CostBasis,
}

impl Commodity {
	pub fn new(symbol: String, cost_basis: CostBasis) -> Self {
		Self { symbol, cost_basis }
	}

	pub fn symbol(&self) -> &str {
		&self.symbol
	}

	pub fn cost_basis(&self) -> &CostBasis {
		&self.cost_basis
	}

	pub fn unit_cost(&self) -> Scalar {
		self.cost_basis.unit_cost
	}
}

impl std::fmt::Display for Commodity {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{} {{ {} }}", self.symbol, self.cost_basis)
	}
}
