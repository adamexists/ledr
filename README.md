# Ledr

#### Plain text accounting tool

[![builds.sr.ht status](https://builds.sr.ht/~house/ledr.svg)](https://builds.sr.ht/~house/ledr?)

Ledr is a [plain text accounting](https://plaintextaccounting.org) tool,
written in Rust, and designed for complex use cases. It includes a parser, a
reporting engine, and absolutely no floating point arithmetic.

Ledr is a work in progress, but it already has a robust syntax and is ready
for routine use. The primary focuses of the project at the moment are to
expand its feature set related to investments, particularly lot tracking,
and to complete its documentation.

This document will be expanded as the project expands.

## Getting Started

Check out the `tests` directory for a large number of examples of specific
entries and what they can contain. The tests cover substantially all the
syntax of the project. This is only a stopgap recommendation until better
documentation is written.

Compiling Ledr requires a working Rust toolchain. From there, it's as simple
as cloning the repository, running `cargo build --release`, and doing what you
will!

## Contributions

Ledr is a passion project. I use it every day for my own finances and
accounting, and I am currently putting substantial time into adding features
to the project.

The most important type of contribution you can provide is an example of a
ledger that is being processed incorrectly or unexpectedly, is unintuitive to
you, or is unable to represent a legitimate financial situation that you or
someone is in.

## Roadmap

The current targeted features are:
- Adding support for associating arbitrary text notes to entries
- Completing functionality related to lot management and capital gains/losses
- Thereafter, expanding the set of reports available, including at least:
  - Lots, capital gains / losses, assignment strategies, holdings, & net worth
  - Transaction-based reports, views, & searching
  - Arbitrary filtering of financial statement reports (fuzzy account matching)
  - Prices and rates, declared and inferred, over time 

Once the project is feature-complete, I will do a documentation push, which
will signal the end of the first phase of development. I am targeting Q2 2025
for this.

The second phase of development is looser, but will likely consist of an API
server capable of reporting on the contents of the file and performing updates
to it. After that, the goal would be a UI. The goal is a world-class accounting
system, ready for mainstream use by any individual or corporation, backed by a
collection of text files and released for free under the GPL.

## Copyright & License

Copyright © Adam House 2024 <adam@adamexists.com>

Ledr is licensed under the GPLv3.
