use std::fs::File;
use std::io;
use std::io::BufRead;
use std::path::Path;
use anyhow::{bail, Error};
use chrono::NaiveDate;
use crate::models::currency::Currency;
use crate::models::ledger::Ledger;

// TODO: In general, the parser is quick and dirty and could use a lot of work
//  to become reliable. It's not nearly good enough at sanitizing the input yet.
pub fn parse_ledger(file_path: &str, ledger: &mut Ledger) -> Result<(), Error> {
    let path = Path::new(file_path);
    let file = File::open(&path)?;
    let reader = io::BufReader::new(file);

    let mut lines = reader.lines().peekable();

    while let Some(Ok(line)) = lines.next() {
        let line = line.trim_end();

        // Skip blank lines or lines containing only whitespace, or process them as a signal to finish an entry
        if line.trim().is_empty() {
            ledger.finish_entry()?;
            continue;
        }

        if line.starts_with(|c: char| c.is_whitespace()) && !line.starts_with("  ") && !line.starts_with("\t") {
            // Line starts with whitespace but not enough for an entry detail - invalid format
            bail!("Invalid line format: {}", line);
        }

        if line.split_whitespace().nth(1) == Some("currency") {
            // Currency declaration
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 3 {
                bail!("Invalid currency declaration format: {}", line);
            }
            let date = NaiveDate::parse_from_str(parts[0], "%Y-%m-%d")?;
            let symbol = parts[2].to_string();
            ledger.declare_currency(symbol, date)?;
        } else if !line.starts_with("  ") && !line.starts_with("\t") {
            // Entry declaration line
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() != 2 {
                bail!("Invalid entry declaration format: {}", line);
            }
            let date = NaiveDate::parse_from_str(parts[0], "%Y-%m-%d")?;
            let desc = parts[1].to_string();
            ledger.new_entry(date, desc)?;
        } else {
            // Entry detail line
            let parts: Vec<&str> = line.trim().splitn(2, |c| c == ' ' || c == '\t').collect();
            if parts.len() < 1 {
                bail!("Invalid entry detail format: {}", line);
            }
            let account = parts[0].to_string();
            let amount_currency = if parts.len() == 2 { parts[1].split_whitespace().collect::<Vec<&str>>() } else { vec![] };

            if amount_currency.is_empty() {
                // Virtual detail case (no amount, only account)
                ledger.set_virtual_detail(account)?;
            } else {
                // Regular detail with amount and currency
                let amount = amount_currency[0].to_string();
                let currency_symbol = amount_currency[1].to_string();
                let currency = Currency::new(currency_symbol);
                ledger.add_detail(account, amount, currency)?;
            }
        }
    }

    // Make sure to finish the last pending entry if the file ends without an empty line
    ledger.finish_entry()?;

    Ok(())
}
