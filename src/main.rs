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
use crate::reports::rate_reporter::RateReporter;
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
	#[arg(short = 'E', long)]
	ignore_equity: bool,

	/// Condense accounts nested below this depth
	#[arg(short, long)]
	depth: Option<usize>,

	/// Enable warning messages for potential data integrity problems
	#[arg(long)]
	emit_warnings: bool,

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

#[derive(ValueEnum, Clone, PartialEq)]
enum Directive {
	Bs, // balance sheet
	Is, // income statement
	Tb, // trial balance

	As, // account summary
	Er, // exchange rates

	Lots, // open lots report
	Rgl,  // realized gains/losses report
	Ugl,  // unrealized gains/losses report
}

fn main() -> Result<(), Error> {
	let args = Cli::parse();

	let (begin, end) = get_range(&args)?;

	let mut ledger = Ledger::new(args.lenient);

	let mut parser = parsing::parser::Parser::new();
	let parse_result = parser.parse(&args.file, &mut ledger, &end)?;

	let portfolio = finalize_ledger(
		&mut ledger,
		args.precision,
		&parse_result,
		&begin,
		args.emit_warnings,
	)?;

	match args.command {
		Directive::Bs => financial_statement(
			ledger,
			args,
			true,
			vec!["Assets", "Liabilities"],
		)?,
		Directive::Is => financial_statement(
			ledger,
			args,
			false,
			vec!["Income", "Expenses"],
		)?,
		Directive::Tb => financial_statement(
			ledger,
			args,
			true,
			vec!["Assets", "Liabilities", "Income", "Expenses"],
		)?,
		Directive::As => {
			// Ensure the search term is provided for the AS command
			if let Some(account) = &args.term {
				let entries = LedgerReporter::new(ledger.take_entries());
				entries.account_summary(account, args.currency)
			} else {
				bail!("No account specified");
			}
		},
		Directive::Er => {
			let rates = ledger.exchange_rates.take_all_rates();
			let reporter = RateReporter::new(rates);
			reporter.print_all_rates();
		},
		Directive::Lots => {
			let reporter = PortfolioReporter::new(
				portfolio.take_lots(vec![LotFilter::Status(LotStatus::Open)]),
				parse_result.max_precision_by_currency,
				args.precision.unwrap_or(u32::MAX),
			);
			reporter.print_open_lots(&end.min(today()))
		},
		Directive::Rgl => {
			let reporter = PortfolioReporter::new(
				portfolio.take_lots(vec![LotFilter::HasSales(true)]),
				parse_result.max_precision_by_currency,
				args.precision.unwrap_or(u32::MAX),
			);
			reporter.print_realized_gain_loss(
				&begin,
				&end.min(today()),
				&ledger.exchange_rates,
			)
		},
		Directive::Ugl => {
			let reporter = PortfolioReporter::new(
				portfolio.take_lots(vec![LotFilter::Status(LotStatus::Open)]),
				parse_result.max_precision_by_currency,
				args.precision.unwrap_or(u32::MAX),
			);
			reporter.print_unrealized_gain_loss(
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
	parse_result: &ParseResult,
	begin: &Date,
	emit_warnings: bool,
) -> Result<Portfolio, Error> {
	ledger
		.exchange_rates
		.finalize(&parse_result.max_precision_by_currency, emit_warnings)?;

	let portfolio = ledger.lots.tabulate()?;

	ledger.finalize(
		&parse_result.max_precision_by_currency,
		max_precision,
		begin,
	)?;

	Ok(portfolio)
}

fn financial_statement(
	ledger: Ledger,
	args: Cli,
	include_equity_by_default: bool,
	top_level_accounts_to_show: Vec<&str>,
) -> Result<(), Error> {
	let mut totals = ledger_to_totals(ledger, args.currency, args.invert)?;
	if let Some(p) = args.precision {
		totals.set_max_precision(p);
	}

	let mut top_levels = top_level_accounts_to_show;
	if include_equity_by_default && !args.ignore_equity {
		top_levels.push("Equity");
	}
	totals.filter_top_level(top_levels);
	let mut reporter = StatementReporter::from_total(totals);

	reporter.sort_canonical();
	reporter.print_ledger_format(args.depth);
	Ok(())
}

fn ledger_to_totals(
	mut ledger: Ledger,
	collapse: Option<String>,
	invert: bool,
) -> Result<Total, Error> {
	let mut totals = Total::from_ledger(&ledger);

	if let Some(collapse) = &collapse {
		totals.collapse_to(collapse, &mut ledger.exchange_rates, false);
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
