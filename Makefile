.PHONY: all build test clean install

all: build test

build:
	cargo build --release

test:
	cargo test -- --test-threads=1

clean:
	cargo clean

install:
	install -m 755 target/release/ledr /usr/local/bin/ledr
