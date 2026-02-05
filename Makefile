# Veracity Makefile - all builds go to target/release/

.PHONY: build run test clean install

build:
	cargo build --release -j 6

run:
	cargo run --release -j 6

test:
	cargo test --release -j 6

clean:
	cargo clean

install:
	cargo install --path . --force
