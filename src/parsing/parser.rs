use std::fs::File;
use std::io;
use std::io::{BufRead, Seek};
use std::path::Path;
use anyhow::{bail, Error};
use crate::tabulation::ledger::Ledger;
use crate::util::scalar::Scalar;
use crate::util::date::Date;

// TODO: Implement strict account declaration.

// First pass to process only directive lines
fn first_pass(file: &File, ledger: &mut Ledger) -> Result<(), Error> {
    let reader = io::BufReader::new(file);

    for (i, line) in reader.lines().enumerate() {
        let line = line?.trim().to_string();

        // Skip blank lines and comments
        if line.is_empty() || line.starts_with("#") {
            continue;
        }

        // Handle directive lines starting with a date and '!'
        if let Some((date_str, remainder)) = line.split_once('!') {
            let date = Date::from_str(date_str.trim())?;
            let parts: Vec<&str> = remainder
                .trim()
                .split_whitespace()
                .collect();

            if parts.is_empty() {
                bail!("Invalid directive format (line {}): {}", i+1, line);
            }

            match parts[0] {
                "currency" if parts.len() == 2 => {
                    let currency = parts[1].to_string();
                    ledger.declare_currency(currency, date)?;
                }
                "rate" if parts.len() == 4 => {
                    let from = parts[1].to_string();
                    let to = parts[2].to_string();
                    let rate = parts[3];
                    ledger.exchange_rates.declare(
                        date, from, to, Scalar::new(rate)?.to_f64(),
                    )?
                }
                _ => bail!("Invalid directive or arguments (line {}): {}",
                    i+1,
                    line,
                ),
            }
        }
    }

    Ok(())
}

// Second pass to process everything else
fn second_pass(file: &File, ledger: &mut Ledger) -> Result<(), Error> {
    let reader = io::BufReader::new(file);

    let mut lines = reader.lines();
    let mut i = 0;

    while let Some(Ok(line)) = lines.next() {
        i += 1;
        let line = line.trim();

        // ignore comment lines completely
        if line.starts_with("#") {
            continue;
        }

        // Skip blank lines or process them as a signal to finish an entry
        if line.is_empty() {
            ledger.finish_entry()?;
            continue;
        }

        // Skip directive lines
        if line.contains('!') {
            continue;
        }

        // Handle entry declaration lines with a date and description
        if let Some((date_str, desc)) = line.split_once(' ') {
            if let Ok(date) = Date::from_str(date_str.trim()) {
                ledger.new_entry(date, desc.trim().to_string())?;
                continue;
            }
        }

        // Handle entry detail lines
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let account = parts[0].to_string();
            let amount = parts[1].to_string();
            let currency = parts[2].to_string();

            // Handle optional cost basis
            let cost_basis = if parts.len() > 3 {
                let basis_str = parts[3..].join(" ");
                if let Some((operator, basis)) = basis_str.split_once(' ') {
                    let b_parts: Vec<&str> = basis.split_whitespace().collect();
                    if b_parts.len() != 2 {
                        bail!("Invalid cost basis (line {}): {}", i, line);
                    }


                    let is_total_cost = match operator {
                        "@" => false,
                        "@@" => true,
                        _ => bail!("Invalid cost basis (line {}): {}", i, line)
                    };

                    Some((b_parts[0].to_string(), b_parts[1].to_string(), is_total_cost))
                } else {
                    bail!("Invalid cost basis (line {}): {}", i, line);
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

        bail!("Invalid line format (line {}): {}", i, line);
    }

    // Make sure to finish the last entry if the file ends without an empty line
    ledger.finish_entry()?;

    Ok(())
}

/// Opens and parses the file at file_path into the passed Ledger. We make two
/// passes through the file: the first processes directives, and the second
/// processes everything else. This means we are agnostic to the order of any
/// contents of the file. The only exception is that when multiple implicit
/// currency conversions occur in the same day between the same currencies, all
/// reporting will use the latest one processed in the file. TODO: Manpage this.
pub fn parse_ledger(file_path: &str, ledger: &mut Ledger) -> Result<(), Error> {
    let path = Path::new(file_path);
    let mut file = File::open(&path)?;

    first_pass(&file, ledger)?;
    file.rewind()?;
    second_pass(&file, ledger)?;

    Ok(())
}
