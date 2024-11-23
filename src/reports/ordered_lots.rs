use crate::investment::lot::Lot;
use crate::reports::table::Table;

/// Struct for handling and displaying an ordered list of lots, for reports
pub struct OrderedLots {
	lots: Vec<Lot>,
}

impl OrderedLots {
	pub fn new(mut lots: Vec<Lot>) -> Self {
		lots.sort();
		Self { lots }
	}

	/// Prints an abbreviated table format, meant to contain open lots only.
	pub fn print_open_lots(&self) {
		if self.lots.len() == 0 {
			println!("No open lots");
			return;
		}

		let mut table = Table::new(6);
		table.right_align(0);
		table.right_align(3);

		table.add_row(vec![
			"Purchased",
			"Commodity",
			"Open Qty",
			"Cost Basis",
			"Account",
			"Sales",
		]);
		table.add_separator();
		for l in self.lots.iter() {
			table.add_row(vec![
				&l.acquisition_date.to_string(),
				&l.commodity.symbol().to_string(),
				&l.quantity.to_string(),
				&l.commodity.cost_basis().to_string(),
				&l.account.to_string(),
				&l.format_sales(),
			])
		}

		let bottom_line =
			if self.lots.len() == 1 { "Lot" } else { "Lots" };

		// total just shows lot count
		table.print(
			5,
			&format!(
				"{} {}",
				self.lots.len().to_string(),
				bottom_line
			),
			None,
			&"".to_string(),
		)
	}
}
