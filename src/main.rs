/* Copyright © 2024 Adam House <adam@adamexists.com>
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
use crate::investment::lot::LotStatus;
use crate::investment::portfolio::{LotFilter, Portfolio};
use crate::parsing::parser::ParseResult;
use crate::reports::ledger_reporter::LedgerReporter;
use crate::reports::portfolio_reporter::PortfolioReporter;
use crate::reports::statement_reporter::StatementReporter;
use crate::util::date::Date;
use anyhow::{bail, Error};
use chrono::Local;
use clap::{Parser, ValueEnum};
use gl::ledger::Ledger;
use std::cmp::PartialEq;

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
	/// Ignore entries prior to this date (YYYY-MM-DD)
	#[arg(short, long, required = false)]
	begin: Option<String>,

	/// Ignore entries after this date (YYYY-MM-DD)
	#[arg(short, long, required = false)]
	end: Option<String>,

	/// Specifies the input file
	#[arg(short)]
	file: String,

	/// Only show balances in the given currency, converting when possible
	#[arg(short, long)]
	currency: Option<String>,

	/// Hides equity accounts from reports
	/// TODO: Add test data examples to validate this one.
	#[arg(short = 'E', long)]
	ignore_equity: bool,

	/// Condense accounts nested below this depth
	#[arg(short, long)]
	depth: Option<usize>,

	/// Negates all currency values
	/// TODO: Add test data examples to validate this one.
	#[arg(short, long)]
	invert: bool,

	/// Ignore directives designed to catch and correct bad input data
	/// TODO: Add test data examples to validate this one.
	#[arg(long)]
	lenient: bool,

	/// Maximum amount of decimal places to show for any amounts
	/// TODO: Add test data examples to validate this one.
	#[arg(short, long)]
	precision: Option<u32>,
}

#[derive(ValueEnum, Clone, PartialEq)]
enum Directive {
	Bs, // balance sheet
	Is, // income statement
	Tb, // trial balance

	As, // account summary

	Lots, // open lots report
	Rgl,  // realized gains/losses report
	Ugl,  // unrealized gains/losses report
}

fn main() -> Result<(), Error> {
	let args = Cli::parse();

	let (begin, end) = get_range(&args)?;

	let mut ledger = Ledger::new(args.lenient);

	let mut parser = parsing::parser::Parser::new();
	let parse_result = parser.parse(&args.file, &mut ledger)?;

	// For some filtered reports, the end date is an as-of date, so we "rewind"
	// history if that report is selected by ignoring lot actions after it.
	// Everything before it is always computed. In other cases, the portfolio
	// always sees everything, but other date filters may change what we show
	// about the full up-to-date state.
	let portfolio = finalize_ledger(
		&mut ledger,
		args.precision,
		&args.currency,
		&parse_result,
		&begin,
		&end,
		args.command == Directive::Lots || args.command == Directive::Ugl,
	)?;

	match args.command {
		Directive::Bs => {
			financial_statement(ledger, args, vec!["Assets", "Liabilities"])?
		},
		Directive::Is => {
			financial_statement(ledger, args, vec!["Income, Expenses"])?
		},
		Directive::Tb => financial_statement(
			ledger,
			args,
			vec!["Assets", "Liabilities", "Income", "Expenses"],
		)?,
		Directive::As => {
			// Ensure the search term is provided for the AS command
			if let Some(account) = &args.term {
				let currency = match &args.currency {
					Some(c) => c,
					None => bail!("Currency required (-c)"),
				};

				let entries = LedgerReporter::new(ledger.take_entries());
				entries.account_summary(account, currency)
			} else {
				bail!("No account specified");
			}
		},
		Directive::Lots => {
			let ordered_lots = PortfolioReporter::new(
				portfolio.take_lots(vec![LotFilter::Status(LotStatus::Open)]),
				parse_result.max_precision_by_currency,
				args.precision.unwrap_or(u32::MAX),
			);
			ordered_lots.print_open_lots(&end.min(today()))
		},
		Directive::Rgl => {
			let ordered_lots = PortfolioReporter::new(
				portfolio.take_lots(vec![LotFilter::HasSales(true)]),
				parse_result.max_precision_by_currency,
				args.precision.unwrap_or(u32::MAX),
			);
			ordered_lots.print_realized_gain_loss(&begin, &end.min(today()))
		},
		Directive::Ugl => {
			let ordered_lots = PortfolioReporter::new(
				portfolio.take_lots(vec![LotFilter::Status(LotStatus::Open)]),
				parse_result.max_precision_by_currency,
				args.precision.unwrap_or(u32::MAX),
			);
			ordered_lots.print_unrealized_gain_loss(
				&end.min(today()),
				&ledger.exchange_rates,
			)
		},
	}

	Ok(())
}

/// Performs validation of the ledger, and returns the portfolio representing
/// the state of lots.
fn finalize_ledger(
	ledger: &mut Ledger,
	max_precision: Option<u32>,
	collapse_to: &Option<String>,
	parse_result: &ParseResult,
	begin: &Date,
	end: &Date,
	portfolio_ignore_after_end: bool,
) -> Result<Portfolio, Error> {
	ledger.exchange_rates.finalize(begin, end)?;

	let portfolio_end = match portfolio_ignore_after_end {
		true => end,
		false => &Date::max(),
	};

	let portfolio = ledger.lots.tabulate(portfolio_end)?;

	if let Some(collapse) = collapse_to {
		ledger.collapse_to(collapse.clone());
	}

	ledger.finalize(
		&parse_result.max_precision_by_currency,
		max_precision,
		begin,
		end,
	)?;

	Ok(portfolio)
}

fn financial_statement(
	ledger: Ledger,
	args: Cli,
	top_level_accounts_to_show: Vec<&str>,
) -> Result<(), Error> {
	let mut totals = ledger_to_totals(ledger, args.currency, args.invert)?;

	let mut top_levels = top_level_accounts_to_show;
	if !args.ignore_equity {
		top_levels.push("Equity");
	}
	totals.filter_top_level(top_levels);
	let mut ordered_totals = StatementReporter::from_total(totals);

	ordered_totals.sort_canonical();
	ordered_totals.print_ledger_format(args.depth);
	Ok(())
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

fn get_range(args: &Cli) -> Result<(Date, Date), Error> {
	let begin = Date::from_str(
		args.begin.as_ref().unwrap_or(&Date::min().to_string()),
	)?;
	let end =
		Date::from_str(args.end.as_ref().unwrap_or(&Date::max().to_string()))?;

	Ok((begin, end))
}

fn today() -> Date {
	Date::from_str(&Local::now().date_naive().to_string()).unwrap()
}
