use anyhow::Error;
use tabulation::ledger::Ledger;
use crate::parsing::parser::parse_ledger;
use crate::reports::ordered_total::OrderedTotal;

mod parsing;
mod tabulation;
mod reports;
mod util;

fn main() -> Result<(), Error> {
    let mut ledger = Ledger::new();
    parse_ledger("ledger.txt", &mut ledger)?;
    ledger.finalize()?;

    let mut totals = ledger.to_totals()?;

    totals.filter_top_level(vec!["Income", "Expenses"]);
    totals.invert();

    totals.validate()?;

    let mut ordered_totals = OrderedTotal::from_total(totals);

    ordered_totals.sort_canonical();
    ordered_totals.print_ledger_format(None);

    Ok(())
}