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

I welcome constructive feedback and patchsets via mailing list onSourcehut,
where this project is hosted. Please do not email me directly with patchsets,
but feel free to email me for any other reason!

## Copyright & License

Copyright © Adam House 2024 <adam@adamexists.com>

Ledr is licensed under the GPLv3.
