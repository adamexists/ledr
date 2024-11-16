use std::fs::File;
use std::io;
use std::io::BufRead;
use std::path::Path;
use anyhow::{bail, Error};
use chrono::NaiveDate;
use crate::models::currency::Currency;
use crate::models::ledger::Ledger;

pub fn parse_ledger(file_path: &str, ledger: &mut Ledger) -> Result<(), Error> {
    let path = Path::new(file_path);
    let file = File::open(&path)?;
    let reader = io::BufReader::new(file);

    let mut lines = reader.lines().peekable();

    while let Some(Ok(line)) = lines.next() {
        let line = line.trim_end();

        // Skip blank lines or process them as a signal to finish an entry
        if line.trim().is_empty() {
            ledger.finish_entry()?;
            continue;
        }

        // Handle directive lines starting with a date and '!'
        if let Some((date_str, remainder)) = line.split_once('!') {
            let date = NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d")?;
            let parts: Vec<&str> = remainder.trim().split_whitespace().collect();

            if parts.is_empty() {
                bail!("Invalid directive format: {}", line);
            }

            match parts[0] {
                "currency" if parts.len() == 2 => {
                    let symbol = parts[1].to_string();
                    ledger.declare_currency(symbol, date)?;
                }
                _ => bail!("Unknown directive or invalid arguments: {}", line),
            }
            continue;
        }

        // Handle entry declaration lines with a date and description
        if let Some((date_str, desc)) = line.split_once(' ') {
            if let Ok(date) = NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d") {
                ledger.new_entry(date, desc.trim().to_string())?;
                continue;
            }
        }

        // Handle entry detail lines
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let account = parts[0].to_string();
            let amount = parts[1].to_string();
            let currency_symbol = parts[2].to_string();
            let currency = Currency::new(currency_symbol);

            // Handle optional cost basis
            let cost_basis = if parts.len() > 3 {
                let basis_str = parts[3..].join(" ");
                if let Some((symbol, basis_amount_currency)) = basis_str.split_once(' ') {
                    if symbol == "@" {
                        let basis_parts: Vec<&str> = basis_amount_currency.split_whitespace().collect();
                        if basis_parts.len() == 2 {
                            Some((basis_parts[0].to_string(), basis_parts[1].to_string()))
                        } else {
                            bail!("Invalid cost basis format: {}", line);
                        }
                    } else {
                        bail!("Invalid cost basis format: {}", line);
                    }
                } else {
                    bail!("Invalid cost basis format: {}", line);
                }
            } else {
                None
            };

            ledger.add_detail(account, amount, currency, cost_basis)?;
            continue;
        }

        // Handle virtual entry detail lines (only account)
        if parts.len() == 1 {
            let account = parts[0].to_string();
            ledger.set_virtual_detail(account)?;
            continue;
        }

        bail!("Invalid line format: {}", line);
    }

    // Make sure to finish the last pending entry if the file ends without an empty line
    ledger.finish_entry()?;

    Ok(())
}
