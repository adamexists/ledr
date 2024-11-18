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

use crate::parsing::parser::parse_ledger;
use crate::reports::ordered_total::OrderedTotal;
use crate::tabulation::total::Total;
use crate::util::date::Date;
use anyhow::Error;
use clap::{Parser, ValueEnum};
use tabulation::ledger::Ledger;

mod parsing;
mod reports;
mod tabulation;
mod util;

// TODO: scdoc and man page!

#[derive(Parser)]
#[command(name = "ledr", version = "1.0", about = "Plain text accounting tool")]
struct Cli {
	// ----------------
	// -- POSITIONAL --
	// ----------------
	/// The command to execute
	command: Directive,

	// -----------
	// -- FLAGS --
	// -----------
	/// Specifies the input file
	#[arg(short)]
	file: String,

	/// Convert all possible currencies in the output to the given currency
	#[arg(short, long)]
	collapse: Option<String>,

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
	BS,
	IS,
	TB,
	OpenLots, // TODO: Need other lot reports.
}

fn main() -> Result<(), Error> {
	let args = Cli::parse();

	let mut ledger = Ledger::new(args.lenient);
	parse_ledger(&args.file, &mut ledger)?;

	match args.command {
		Directive::BS => {
			let mut totals = financial_statement(&args, ledger)?;

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
			let mut totals = financial_statement(&args, ledger)?;

			totals.filter_top_level(vec!["Income", "Expenses"]);
			let mut ordered_totals =
				OrderedTotal::from_total(totals);

			ordered_totals.sort_canonical();
			ordered_totals.print_ledger_format(args.depth);
		},
		Directive::TB => {
			let totals = financial_statement(&args, ledger)?;
			let mut ordered_totals =
				OrderedTotal::from_total(totals);

			ordered_totals.sort_canonical();
			ordered_totals.print_ledger_format(args.depth);
		},
		Directive::OpenLots => {
			// TODO: Add customization for this directive.
			// TODO: Need to implement pretty-printing for this. Right now,
			//  I've tested it but it has no output anymore.
			ledger.lots.tabulate(&Date::today())?
		},
	}

	Ok(())
}

fn financial_statement(args: &Cli, mut ledger: Ledger) -> Result<Total, Error> {
	ledger.remove_cost_basis();
	if let Some(collapse) = &args.collapse {
		ledger.collapse_to(collapse.clone());
	}
	let mut totals = ledger.finalize(args.precision)?;

	if args.invert {
		totals.invert();
	}

	Ok(totals)
}
