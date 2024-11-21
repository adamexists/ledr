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
use crate::util::scalar::Scalar;
use std::fmt;

/// A scalar value with a currency, and potentially with a cost basis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Amount {
	pub value: Scalar,
	pub currency: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct CostBasis {
	pub unit_cost: Scalar,
	pub currency: String,
}

impl Amount {
	pub fn new(value: Scalar, currency: String) -> Self {
		Self { value, currency }
	}

	pub fn convert_to(&mut self, currency: &str, rate: Scalar) {
		self.currency = currency.to_owned();
		self.value *= rate;
	}
}

impl fmt::Display for CostBasis {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{} {}", self.unit_cost, self.currency)
	}
}
