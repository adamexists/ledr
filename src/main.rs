use anyhow::Error;
use tabulation::ledger::Ledger;
use crate::parsing::parser::parse_ledger;

mod parsing;
mod tabulation;
mod reports;

fn main() -> Result<(), Error> {
    let mut ledger = Ledger::new();
    parse_ledger("ledger.txt", &mut ledger)?;

    let totals = ledger.to_totals();
    totals.validate()?;

    totals.dump_contents();
    Ok(())
}