ledr(1)

# NAME

ledr - plain text accounting tool

# DESCRIPTION

*ledr* is a plain text accounting tool. This document, though, is not ready,
and right now it just contains some bullet points of things it *should* talk
about when the time comes to write it. That'll be after *ledr* is complete
and its features, syntax, etc. are relatively stable.

Here's what this doc should contain in the future:

- If multiple lots are reported at the same date, all buys are assumed to
  come before all sells. Logic with lots is FIFO by default. You can
  control this by indicating specific lot IDs to match with, using quotes
  in the lot syntax.
- Include instructions are not directives. They have different syntax. It
  is best practice to use fully qualified paths with them, else it'll be
  relative to the current working directory.
- If two or more implicit exchange rates are inferred from the same date,
  the last of the set will be used for all reporting. This is the only time
  where the order of entries matters outside of lot-counting, potentially.
- `ledr` uses integer math for all numbers, and is tested & accurate for all
  values within 10E-18 and 10E+18 to at least 18 decimal places. Its actual
  bounds are higher, but nobody needs more than this.
- The default visible precision of every report is the maximum precision of
  any entry in the ledger for that given currency. You can reduce the visible
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