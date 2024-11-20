.PHONY: all build test fmt clean install

all: build

build:
	cargo build --release

test: fmt
	cargo test -- --test-threads=1

fmt:
	cargo fmt

clean: fmt
	cargo clean

install:
	install -m 755 target/release/ledr /usr/local/bin/ledr
