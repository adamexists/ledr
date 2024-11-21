use crate::util::scalar::Scalar;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct CostBasis {
	pub unit_cost: Scalar,
	pub currency: String,
}

impl fmt::Display for CostBasis {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{} {}", self.unit_cost, self.currency)
	}
}
