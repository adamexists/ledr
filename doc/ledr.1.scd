ledr(1)

# NAME

ledr - plain text accounting tool

# SYNOPSIS

*ledr* -f FILE _command_

# DESCRIPTION

*ledr* is a plain text accounting tool. Its primary function is to interpret
and summarize text files containing records of movements of money between
accounts. These movements may be actual (i.e. the purchase of goods) or
abstract (i.e. the appropriation of funds for a specific purpose). In other
words, it is a reporting tool from a text-based *general ledger*.

In addition to this primary function, *ledr* contains an importer that can
pull transactions from external financial systems into a specified text file
in the proper format. At time of writing, the only import target is Mercury,
the financial services firm in the United States. This makes *ledr* highly
suited to those individuals and companies whose primary banking is done there.
Additional importers may exist in the future.

*ledr* contains a number of modules for different purposes. The three
categories of modules are (1) financial statements; (2) lot reports, and (3)
auxiliary functions. Each is described below in turn. All of these expect
an input file in the _ledr_(5) format. A number of configuration options also
exist to modify the display of reports.

With the exception of the _import_ command, *ledr* is solely a read-only tool,
and does not modify files. The _import_ command is only capable of appending
to the end of the file it targets.

# FINANCIAL STATEMENTS

## ledr bs

Balance sheet. Displays a summary of accounts in the Assets, Liabilities and
Equity categories. A point-in-time report that takes into account all entries
prior to the specified end date, to show a complete balance as of that date.

## ledr is

Income statement. Displays a summary of accounts in the Income and Expenses
categories. This report accepts a time-range, to indicate the total income and
expenses during a certain period.

## ledr tb

Trial balance. Displays a summary of all accounts across all five categories.
Generally used to verify that

# ACCOUNTING PRINCIPLES

*ledr*, like all prominent plain text accounting tools, uses double-entry
accounting principles in its design. This means that for every debit (i.e.
increase in the balance of an account) there must be a corresponding, equal
and opposite credit (i.e. decrease in the balance of an account). A general
description of accounting principles is beyond the scope of this manual, but
*ledr* will not be of much interest to you if you are not familiar with the
general principles of double-entry accounting.

## Rounding

*ledr* uses a rational number representation for all numerical values; in
other words, it represents all quantities as a fraction of two whole numbers.
This means it can handle decimal amounts with no loss of precision, so long as
the number of decimal places is reasonable and does not exceed the limits of
the data type. In practice, *ledr* has been tested with all fractional values
with numerator and denominator between 10E-19 and 10E+19. This comfortably
allows management of numbers in the quadrillions or higher, down to decimal
precision beyond the requirements of any human system.

Of course, in all accounting systems, there is a need for rounding. Suppose
you or your firm trades in foreign currency, keeps cryptocurrencies with
very small denominations, or tracks inventories of physical objects with a
need for many decimal places of precision. This does not mean you will want
the numbers on all reports to reflect that level of precision.

*ledr* uses a simple philosophy on rounding: no numbers are ever rounded off
until it comes time to show the results of its calculations to you, the user.
At that time, *ledr* will round quantities using "banker's rounding", i.e.
it rounds normally to the nearest whole number, but if a quantity falls
precisely between two whole numbers (e.g. $3.50 or $4.50 rounded to the
dollar), it will always round to the closest even number (e.g. both the above
example amounts would round to $4.00, not $3.00 or $5.00). This keeps rounding
error more stable over time.

All rounding introduces error. Whenever rounding error is introduced in a
manner that would affect the viewable report, *ledr* inserts an entry to the
_Equity:Rounding_ account to show you this. The full precision of all numbers
is always optionally available to see.

# LOT MANAGEMENT

*ledr* includes support for the tracking of lots. Those familiar with the
surrounding concepts can freely skip the *Background* section below.

## Background

*ledr* includes support for the tracking of lots. A _lot_ is a certain quantity
of a certain asset or commodity that was purchased at a specific time for a
specific amount. An investor, for example, will hold many lots, and in general,
one can say that the sum of one's lots that relate to a certain asset or
commodity would be that investor's _position_.

In most jurisdictions, lots are the basis of capital gain and loss calculation.
To determine how much one gained or lost in a given year through investment
activity, one looks at each of the lots that investor bought and, more
crucially, each of the lots that investor sold during that year. A lot's price
of acquisition is called that lot's _cost basis_, and the price that a lot sold
for is called the lot's _proceeds_. Other terms exist for these as well; here
we are just using common ones. In most jurisdictions, the tax liability for the
capital gain of a lot is computed by subtracting the total cost basis of lot
sales from the total proceeds of lot sales in a given year. This, in
combination with the existence of different tax rates based on the length of
time that one held a particular lot before selling it, necessitates detailed
tracking of not only the purchase and sale prices of lots, but the timing of
when any _specific_ lot was purchased in relation to the time of its sale.

This brings us to the concept of _lot assignment strategies_. The fact is that
you make certain purchases and then certain sales, when investing; the purpose
of a lot assignment strategy is to associate these purchases and sales together
in a suitable manner.

In practice, this may be done as described: on a detailed, lot-by-lot basis,
in which case the investor will keep track of specific lots, when they were
purchased and sold, and assign certain sales to certain specific purchases.
Operating this way is more work, but can yield tax benefits for the savvy
investor. For those who trade too many lots for this to be practical, or such
entities as gigantic corporations who only have so much to gain on their tax
bill relative to the level of effort required to specifically assign lots,
a shortcut strategy may be used. These vary throughout the world, but by far
the most common one is FIFO (first-in-first-out), meaning that one assumes,
at the time of a lot sale, that one is first selling the oldest applicable
lots still held. This maximizes the time a certain lot is considered to have
been held, which is often the point: avoiding long-term capital gains rates.

## Implementation

*ledr* supports FIFO by default, and also supports giving lots specific names
at the time of purchase and then referencing those at the time of sale, which
forces the assignment of a specific lot to a given sale. From this information,
provided good practices and good data hygiene up-front, *ledr* can compute
realized and unrealized capital gains reports to assist in the estimation of
capital gains and losses over a given interval, or one's current financial
position. (On this matter, please see the Disclaimer at the bottom of this
document.)

For further details on the syntax required to declare and assign a name to a
lot, consult _ledr_(5).

## Currency Conversion

*ledr* is a multicurrency system, and supports multicurrency transactions.
In general, all currency amounts must independently balance. *ledr* handles
this by inserting entries to the _Equity:Conversions_ account whenever a
currency conversion takes place. Further discussion of how to handle currency
conversion is available in _ledr_(7).

# SEE ALSO

_ledr_(5) - general ledger file format

_ledr_(7) - data integrity guidance, further discussion of lot management and
currency conversion

# DISCLAIMER

*ledr*'s maintainers are not financial experts and *ledr* is not a substitute
for professional financial advice. *ledr* does not provide financial advice.
*ledr* does not provide tax advice. *ledr*'s maintainers encourage you to
consult qualified financial professionals for such advice.

# AUTHORS

Maintained by *Adam House <adam@adamexists.com>*. Up-to-date source code can be
found at *github.com/adamexists/ledr* and contributions are accepted via GitHub.

# FIN

Here's what this doc should contain in the future:

- If multiple lots are reported at the same date, all buys are assumed to
  come before all sells. Logic with lots is FIFO by default. You can
  control this by indicating specific lot IDs to match with, using quotes
  in the lot syntax.
- Include instructions are not directives. They have different syntax. It
  is best practice to use fully qualified paths with them, else it'll be
  relative to the current working directory. Document all directives too.
- If two or more implicit exchange rates are inferred from the same date,
  the last of the set will be used for all reporting. This is the only time
  where the order of entries matters outside of lot-counting, potentially.
- The default visible precision of every report is the maximum precision of
  any entry in the ledger for that given currency. You can adjust the visible
  precision using the -p command line flag. Internal math is never rounded;
  just the visible results. Rounding is done using banker's rounding.
- If you want to short something, then that's a lot with a negative cost basis
- Should document the details of what lenient mode does, from making account
  and currency declarations optional to skipping coherence checks on currency
  conversion graphs.
- We allow for entries that may not make sense at first glance, like if there
  are two lines in different currencies that are both positive. This actually
  implies a negative exchange rate between the two currencies, as ledr will
  force the two lines to balance. This is useless for most people, but it
  makes sense in cases like shorting a stock (keeping track of your shorted
  share counts), so we allow it.
- If you are using ledr to track lots, you are highly encouraged to record
  sale transactions individually, so the proceeds from them can be
  automatically tracked for reports. If you do not isolate these with just
  one line item (the proceeds from sale), then some lot reports will not
  contain complete information.
- Generally establish that directives bind input, i.e. that a rate directive,
  for example, will make ledr reject input that deviates too far from it on
  that date.
- Entries are processed in the order they are written in the file, taking
  account of directives (all processed first) and include statements (which
  halt in the current file and process the next). Describe the control flow.
- Describe how highly different currency conversions on the same day can yield
  unexpected results and significant conversion error. Also, describe rounding
  error in similar form.
- I should take care to document the meaning of the headers and row contents
  of each report, in proper detail befitting an accounting system's audience.