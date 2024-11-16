use std::collections::HashMap;
use crate::reports::total::Total;
use crate::tabulation::ledger::VALID_PREFIXES;
use crate::tabulation::money::Money;

/// When using this to display something, you should instantiate it, then sort
/// it, then display it. Filters should be handled in the Total struct.
pub struct OrderedTotal {
    account: String,
    amounts: Vec<(String, Money)>, // currency -> balance held
    subtotals: Vec<(String, OrderedTotal)>, // account name -> next total
}

impl OrderedTotal {
    pub fn from_total(t: Total) -> Self {
        Self {
            account: t.account,
            amounts: t.amounts.into_iter().collect(),
            subtotals: t.subtotals
                .into_iter()
                .map(|(k, v)| (k, OrderedTotal::from_total(v)))
                .collect(),
        }
    }

    // -----------
    // -- SORTS --
    // -----------

    /// Sorts top-level by Assets, Liabilities, Equity, Income, Expenses, then
    /// recursively sort in the following way:
    ///
    /// Each ordered_total's amounts are first sorted by currency. Each
    /// ordered_total's subtotals beyond the first, which is the special case,
    /// are then sorted in descending order by the absolute value of the sum of
    /// its amounts' Money components.
    pub fn sort_canonical(&mut self) {
        // Sort amounts by currency
        self.amounts.sort_by(|(a, _), (b, _)| a.cmp(b));

        // Special case: sort the top-level subtotals based on VALID_PREFIXES
        self.subtotals.sort_by_key(|(s, _)| {
            VALID_PREFIXES
                .iter()
                .position(|&prefix| prefix == s)
                .unwrap_or(usize::MAX)
        });

        // Now, sort the rest of the subtotals recursively
        for (_, subtotal) in self.subtotals.iter_mut() {
            subtotal.sort_canonical_recursive();
        }
    }

    fn sort_canonical_recursive(&mut self) {
        // Sort amounts by currency
        self.amounts.sort_by(|(a, _), (b, _)| a.cmp(b));

        // Sort subtotals by the absolute value of the sum of their Money components
        self.subtotals.sort_by(|(_, a), (_, b)| {
            let sum_a: Money = a
                .amounts
                .iter()
                .map(|(_, money)| *money)
                .sum::<Money>().abs();
            let sum_b: Money = b
                .amounts
                .iter()
                .map(|(_, money)| *money)
                .sum::<Money>().abs();
            sum_b.partial_cmp(&sum_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        for (_, subtotal) in &mut self.subtotals {
            subtotal.sort_canonical_recursive();
        }
    }

    // ------------
    // -- PRINTS --
    // ------------

    pub fn calculate_column_width(&self) -> usize {
        let mut max_width = 0;

        // Helper function to determine the width of a formatted currency-amount pair
        let calculate_width = |currency: &String, amount: &Money| {
            format!("{} {}", currency, amount).len()
        };

        // Check the width of all amounts in this OrderedTotal
        for (currency, amount) in &self.amounts {
            let width = calculate_width(currency, amount);
            if width > max_width {
                max_width = width;
            }
        }

        // Recursively check all subtotals
        for (_, subtotal) in &self.subtotals {
            let subtotal_width = subtotal.calculate_column_width();
            if subtotal_width > max_width {
                max_width = subtotal_width;
            }
        }

        max_width + 1
    }

    /// Prints the contents of the ordered_totals like the classic Ledger does.
    /// We only expand the subtotals up to the max_depth, if present.
    pub fn print_ledger_format(&self, max_depth: Option<usize>) {
        let column_width = self.calculate_column_width();

        // Display all entries
        self.ledger_fmt_recursive(0, column_width, max_depth);

        // Display the totals for each currency
        println!("{:>width$}", "------------------", width = column_width);
        for (currency, amount) in &self.amounts {
            println!(
                "{:>width$}  {}",
                format!("{} {}", currency, amount),
                self.account,
                width = column_width
            );
        }
    }

    fn ledger_fmt_recursive(
        &self,
        indent: usize,
        col_width: usize,
        max_depth: Option<usize>,
    ) {
        let indentation = " ".repeat(indent * 2);

        // Iterate over amounts and print each one (except top-level)
        if indent != 0 {
            let amts = &mut self.amounts.iter().peekable();

            let mut has_printed_acct = false;
            while let Some((currency, amount)) = amts.next() {
                // how to print the account name differs massively
                let acct = match (has_printed_acct, amts.peek().is_some()) {
                    (false, false) => &*{
                        format!(" {}", &self.account)
                    },
                    (false, true) => &*{
                        format!(" {}", &self.account)
                    },
                    (true, true) => {
                        " ↩"
                    }
                    (true, false) => {
                        " ↩"
                    }
                };

                println!(
                    "{:>width$} {}{}",
                    format!("{} {}", currency, amount),
                    indentation,
                    acct,
                    width = col_width
                );

                has_printed_acct = true;
            }
        }

        if let Some(d) = max_depth {
            if indent == d {
                return;
            }
        }

        // Recursively display each subtotal
        for (_, subtotal) in &self.subtotals {
            subtotal.ledger_fmt_recursive(indent + 1, col_width, max_depth);
        }
    }
}