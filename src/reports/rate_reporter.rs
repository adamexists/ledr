use crate::reports::table::Table;
use crate::util::date::Date;
use crate::util::quant::Quant;
use std::collections::BTreeMap;

pub struct RateReporter {
	rates: BTreeMap<(String, String), Vec<(Date, Quant)>>,
}

impl RateReporter {
	pub fn new(
		rates: BTreeMap<(String, String), Vec<(Date, Quant)>>,
	) -> RateReporter {
		Self { rates }
	}

	pub fn print_all_rates(&self) {
		let mut table = Table::new(4);

		table.add_header(vec!["Base", "Quote", "Observed", "Rate"]);
		table.add_separator();

		for ((base, quote), rate_set) in &self.rates {
			// This is a hack to account for the way the multi-day set of rates works. Should be resolved. TODO
			for (date, rate) in rate_set {
				let reported_date = if date == &Date::max() {
					"Inferred".to_string()
				} else {
					date.to_string()
				};

				table.add_row(vec![
					base,
					quote,
					&reported_date,
					&rate.to_string(),
				])
			}
		}

		table.print();
	}
}
