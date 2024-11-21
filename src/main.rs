/* Copyright (C) 2024 Adam House <adam@adamexists.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

use crate::gl::total::Total;
use crate::parsing::parser::{parse, ParseResult};
use crate::reports::ordered_entry::OrderedEntry;
use crate::reports::ordered_total::OrderedTotal;
use anyhow::{bail, Error};
use clap::{Parser, ValueEnum};
use gl::ledger::Ledger;

mod gl;
mod investment;
mod parsing;
mod reports;
mod util;

#[derive(Parser)]
#[command(name = "ledr", version = "1.0", about = "Plain text accounting tool")]
struct Cli {
	// ----------------
	// -- POSITIONAL --
	// ----------------
	/// The command to execute
	command: Directive,

	/// The search term for the AS command
	#[arg(required = false)]
	term: Option<String>,

	// -----------
	// -- FLAGS --
	// -----------
	/// Specifies the input file
	#[arg(short)]
	file: String,

	/// Only show balances in the given currency, converting when possible
	#[arg(short, long)]
	currency: Option<String>,

	/// Hides equity accounts from reports
	#[arg(short = 'E', long)]
	ignore_equity: bool,

	/// Condense accounts nested below this depth
	#[arg(short, long)]
	depth: Option<usize>,

	/// Negates all currency values
	#[arg(short, long)]
	invert: bool,

	/// Ignore directives designed to catch and correct bad input data
	#[arg(long)]
	lenient: bool,

	/// Maximum amount of decimal places to show for any amounts
	#[arg(short, long)]
	precision: Option<u32>,
}

#[derive(ValueEnum, Clone)]
enum Directive {
	BS, // balance sheet
	IS, // income statement
	TB, // trial balance

	AS, // account summary

	OpenLots, // TODO: Need other lot reports.
}

fn main() -> Result<(), Error> {
	let args = Cli::parse();

	let mut ledger = Ledger::new(args.lenient);
	let parse_result = parse(&args.file, &mut ledger)?;

	match args.command {
		Directive::BS => {
			ledger.lots.tabulate(&parse_result.latest_date)?;
			finalize_ledger(&args, &mut ledger, parse_result)?;
			let mut totals = ledger_to_totals(
				ledger,
				args.currency,
				args.invert,
			)?;

			let mut top_levels = vec!["Assets", "Liabilities"];
			if !args.ignore_equity {
				top_levels.push("Equity");
			}
			totals.filter_top_level(top_levels);
			let mut ordered_totals =
				OrderedTotal::from_total(totals);

			ordered_totals.sort_canonical();
			ordered_totals.print_ledger_format(args.depth);
		},
		Directive::IS => {
			ledger.lots.tabulate(&parse_result.latest_date)?;
			finalize_ledger(&args, &mut ledger, parse_result)?;
			let mut totals = ledger_to_totals(
				ledger,
				args.currency,
				args.invert,
			)?;

			totals.filter_top_level(vec!["Income", "Expenses"]);
			let mut ordered_totals =
				OrderedTotal::from_total(totals);

			ordered_totals.sort_canonical();
			ordered_totals.print_ledger_format(args.depth);
		},
		Directive::TB => {
			ledger.lots.tabulate(&parse_result.latest_date)?;
			finalize_ledger(&args, &mut ledger, parse_result)?;
			let totals = ledger_to_totals(
				ledger,
				args.currency,
				args.invert,
			)?;

			let mut ordered_totals =
				OrderedTotal::from_total(totals);

			ordered_totals.sort_canonical();
			ordered_totals.print_ledger_format(args.depth);
		},
		Directive::AS => {
			// Ensure the search term is provided for the AS command
			if let Some(account) = &args.term {
				let currency = match &args.currency {
					Some(c) => c,
					None => bail!("Currency required (-c)"),
				};

				ledger.lots
					.tabulate(&parse_result.latest_date)?;
				finalize_ledger(
					&args,
					&mut ledger,
					parse_result,
				)?;
				let entries = OrderedEntry::new(
					ledger.take_entries(),
				);
				entries.account_summary(account, currency)
			} else {
				bail!("No account specified");
			}
		},
		Directive::OpenLots => {
			// TODO: Add customization for this directive.
			// TODO: Need to implement pretty-printing for this.
			//  Right now, I've tested it but it has no output
			//  anymore.
			ledger.lots.tabulate(&parse_result.latest_date)?;
		},
	}

	Ok(())
}

fn finalize_ledger(
	args: &Cli,
	ledger: &mut Ledger,
	parse_result: ParseResult,
) -> Result<(), Error> {
	if let Some(collapse) = &args.currency {
		ledger.collapse_to(collapse.clone());
	}

	ledger.finalize(parse_result.max_precision_by_currency, args.precision)
}

fn ledger_to_totals(
	ledger: Ledger,
	collapse: Option<String>,
	invert: bool,
) -> Result<Total, Error> {
	let mut totals = Total::from_ledger(ledger);

	if let Some(collapse) = &collapse {
		totals.ignore_currencies_except(collapse);
	}

	if invert {
		totals.invert();
	}

	Ok(totals)
}
