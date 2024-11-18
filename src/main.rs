use anyhow::Error;
use clap::{Parser, ValueEnum};
use tabulation::ledger::Ledger;
use crate::parsing::parser::parse_ledger;
use crate::reports::ordered_total::OrderedTotal;
use crate::tabulation::total::Total;
use crate::util::date::Date;

mod parsing;
mod tabulation;
mod reports;
mod util;

// TODO: Do a sweep for the usefulness of errors.
// TODO: Should add a .build.yml for tests, too.
// TODO: Sweep "pub" stuff that can actually be "pub(crate)".

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

    let mut ledger = Ledger::new();
    parse_ledger(&args.file, &mut ledger)?;

    match args.command {
        Directive::BS => {
            let mut totals = financial_statement(&args, ledger)?;

            let mut top_levels = vec!["Assets", "Liabilities"];
            if !args.ignore_equity {
                top_levels.push("Equity");
            }
            totals.filter_top_level(top_levels);
            let mut ordered_totals = OrderedTotal::from_total(totals);

            ordered_totals.sort_canonical();
            ordered_totals.print_ledger_format(args.depth);
        },
        Directive::IS => {
            let mut totals = financial_statement(&args, ledger)?;

            totals.filter_top_level(vec!["Income", "Expenses"]);
            let mut ordered_totals = OrderedTotal::from_total(totals);

            ordered_totals.sort_canonical();
            ordered_totals.print_ledger_format(args.depth);
        }
        Directive::TB => {
            let totals = financial_statement(&args, ledger)?;
            let mut ordered_totals = OrderedTotal::from_total(totals);

            ordered_totals.sort_canonical();
            ordered_totals.print_ledger_format(args.depth);
        }
        Directive::OpenLots => {
            // TODO: Add customization for this directive.
            // TODO: Need to implement pretty-printing for this. Right now,
            //  I've tested it but it has no output anymore.
            ledger.lots.tabulate(&Date::today())?
        }
    }

    Ok(())
}

fn financial_statement(args: &Cli, mut ledger: Ledger) -> Result<Total, Error> {
    ledger.remove_cost_basis();
    if let Some(collapse) = &args.collapse {
        ledger.collapse_to(collapse.clone());
    }
    ledger.finalize(args.precision)?;

    let mut totals = ledger.to_totals()?;

    if args.invert {
        totals.invert();
    }

    totals.validate()?;

    Ok(totals)
}
