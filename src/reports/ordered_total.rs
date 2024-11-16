use crate::reports::total::Total;
use crate::tabulation::ledger::VALID_PREFIXES;
use crate::tabulation::money::Money;

// When using this to display something, you should instantiate it from_total(),
// then filter it, then sort it, then display it.
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

    // Sorts top-level by Assets, Liabilities, Equity, Income, Expenses, then
    // recursively sort in the following way:
    //
    // Each ordered_total's amounts should be sorted by currency. Each
    // ordered_total's subtotals beyond the first, which is the special case here,
    // should be sorted in descending order by the absolute value of the sum of
    // its amounts' Money components.
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
            subtotal.sort_recursive();
        }
    }

    // Helper method to sort subtotals recursively
    fn sort_recursive(&mut self) {
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

        // Recursively sort the subtotals
        for (_, subtotal) in &mut self.subtotals {
            subtotal.sort_recursive();
        }
    }

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

    // Prints the contents of the ordered_totals like the classic Ledger does.
    // We only expand the subtotals up to the max_depth, if present.
    pub fn print_ledger_format(&self, max_depth: Option<usize>) {
        let column_width = self.calculate_column_width();

        // Display all entries
        self.print_ledger_format_recursive(0, column_width, max_depth);

        // Display the totals for each currency
        println!("{:>width$}", "--------------------", width = column_width);
        for (currency, amount) in &self.amounts {
            println!(
                "{:>width$}  {}",
                format!("{} {}", currency, amount),
                self.account,
                width = column_width
            );
        }
    }

    fn print_ledger_format_recursive(
        &self,
        indent: usize,
        column_width: usize,
        max_depth: Option<usize>,
    ) {
        let indentation = " ".repeat(indent * 2);

        // Iterate over amounts and print each one (except top-level)
        if indent != 0 {
            for (currency, amount) in &self.amounts {
                println!(
                    "{:>width$}  {}{}",
                    format!("{} {}", currency, amount),
                    indentation,
                    self.account,
                    width = column_width
                );
            }
        }

        if let Some(d) = max_depth {
            if indent == d {
                return;
            }
        }

        // Recursively display each subtotal
        for (_, subtotal) in &self.subtotals {
            subtotal.print_ledger_format_recursive(indent + 1, column_width, max_depth);
        }
    }
}