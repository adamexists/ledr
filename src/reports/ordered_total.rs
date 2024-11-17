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
    // -- CHECKS --
    // ------------

    /// Returns true iff this OrderedTotal has the same balance as all subtotals
    /// below it, and there is only one subtotal on each level of depth. In
    /// other words, if true, then it should be intuitive to represent this
    /// balance on a single line, with the account name segments below it all
    /// condensed into one line.
    ///
    /// e.g. Instead of
    ///     USD -3,000.00    Liabilities
    ///       USD -900.00      CreditCards
    ///       USD -900.00        Card
    ///       USD -400.00          Nested
    ///       USD -400.00            SuperFar
    ///       USD -400.00              Down
    ///
    /// We would get
    ///     USD -3,000.00    Liabilities
    ///       USD -900.00      CreditCards
    ///       USD -900.00        Card
    ///       USD -400.00          Nested:SuperFar:Down
    fn can_condense_with_all_below(&self) -> bool {
        if self.subtotals.len() > 1 {
            return false;
        }

        // Store the expected currency amounts from the current node
        let expected_amounts = &self.amounts;

        for (_, ot) in &self.subtotals {
            // Check if the path is still linear.
            if !ot.can_condense_with_all_below() {
                return false;
            }

            // Validate that the amounts in the current node match the expected amounts
            if !OrderedTotal::amounts_match(expected_amounts, &ot.amounts) {
                return false;
            }
        }

        true
    }

    fn amounts_match(a: &Vec<(String, Money)>, b: &Vec<(String, Money)>) -> bool {
        if a.len() != b.len() {
            return false;
        }

        for (currency, amount) in a {
            if let Some((_, other_amount)) = b.iter().find(|(c, _)| c == currency) {
                if amount != other_amount {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    /// Performs the condensation of name discussed and shown above, on the
    /// comment for OrderedTotal::can_condense_with_all_below().
    fn condensed_name(&self) -> String {
        // Check if there are no subtotals; return the current node's name.
        if self.subtotals.is_empty() {
            return self.account.clone();
        }

        // There should be only one subtotal at this point because of the has_single_subtotal_to_leaf check.
        let (_, subtotal) = self.subtotals.iter().next().unwrap();

        // Recursively get the name from the next node and concatenate with a colon.
        format!("{}:{}", self.account, subtotal.condensed_name())
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
        let can_condense = self.can_condense_with_all_below();

        // Iterate over amounts and print each one (except top-level)
        if indent != 0 {
            let amts = &mut self.amounts.iter().peekable();


            let account_name = if can_condense {
                &self.condensed_name()
            } else {
                &self.account
            };

            let mut has_printed_acct = false;
            while let Some((currency, amount)) = amts.next() {
                // we avoid repeating the same account name on subsequent lines
                // when multicurrency balances would otherwise cause that
                let acct = match (has_printed_acct, amts.peek().is_some()) {
                    (true, _) => " ↩",
                    _ => &*format!(" {}", account_name)
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

        if !can_condense {
            // Recursively display each subtotal
            for (_, subtotal) in &self.subtotals {
                subtotal.ledger_fmt_recursive(indent + 1, col_width, max_depth);
            }
        }
    }
}