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
  come before all sells.
- Include instructions are not directives. They have different syntax. It
  is best practice to use fully qualified paths with them, else it'll be
  relative to the current working directory.
- If two or more implicit exchange rates are inferred from the same date,
  the last of the set will be used for all reporting. This is the only time
  where the order of entries matters outside of lot-counting, potentially.
- `ledr` uses integer math for all numbers, and is tested & accurate for all
  values within 10E-9 and 10E+9 to at least 18 decimal places. Its actual
  bounds are higher, but nobody needs more than this.
- The default visible precision of every report is the maximum precision of
  any entry in the ledger for that given currency. You can reduce the visible
  precision using the -p command line flag. Internal math is never rounded;
  just the visible results. Rounding is done using banker's rounding.
- If you want to short something, then that's a lot with a negative cost basis
- Should document the details of what lenient mode does, from making account
  and currency declarations optional to skipping coherence checks on currency
  conversion graphs.