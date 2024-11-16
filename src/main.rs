use anyhow::Error;
use crate::models::ledger::Ledger;
use crate::parser::parse_ledger;

mod models;
mod parser;

fn main() -> Result<(), Error> {
    let mut ledger = Ledger::new();
    parse_ledger("ledger.txt", &mut ledger)?;

    let totals = ledger.to_totals();
    totals.validate()?;

    totals.dump_contents();
    Ok(())
}