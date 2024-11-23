use crate::investment::lot::Lot;
use crate::reports::table::Table;
use crate::util::date::Date;

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
	pub fn print_open_lots(&self, as_of: &Date) {
		if self.lots.is_empty() {
			println!("No open lots");
			return;
		}

		let mut table = Table::new(7);
		table.right_align(0);
		table.right_align(1);
		table.right_align(3);
		table.right_align(4);

		table.add_row(vec![
			"Opened",
			"Held",
			"Asset",
			"Qty",
			"Basis",
			"Account",
			"Dispositions",
		]);

		table.add_separator();
		for l in self.lots.iter() {
			table.add_row(vec![
				&l.acquisition_date.to_string(),
				&l.time_held(as_of).to_string(),
				l.commodity.symbol(),
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
			&format!("{} {}", self.lots.len(), bottom_line),
			None,
			&"".to_string(),
		)
	}
}
