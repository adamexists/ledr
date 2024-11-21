use crate::util::scalar::Scalar;

/// A scalar value with a currency, and potentially with a cost basis.
#[derive(Clone, Debug)]
pub struct Amount {
	pub value: Scalar,
	pub currency: String,

	cost_basis: Option<CostBasis>,
	/// Denotes whether to treat the cost basis as a lot.
	is_lot: bool,
}

#[derive(Clone, Debug)]
pub struct CostBasis {
	pub unit_price: Scalar,
	pub currency: String,
}

impl Amount {
	pub fn new(value: Scalar, currency: String) -> Self {
		Self {
			value,
			currency,
			cost_basis: None,
			is_lot: false,
		}
	}

	pub fn add_cost_basis(&mut self, cb: CostBasis) {
		self.cost_basis = Some(cb)
	}

	pub fn has_cost_basis(&self) -> bool {
		self.cost_basis.is_some()
	}

	pub fn cost_basis(&self) -> Option<&CostBasis> {
		self.cost_basis.as_ref()
	}

	pub fn take_cost_basis(self) -> Option<CostBasis> {
		self.cost_basis
	}
}
