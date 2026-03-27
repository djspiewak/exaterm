.PHONY: all build run daemon check test test-workspace core-test core-check daemon-check probe clean help

all: build

build:
	cargo build -p exaterm -p exatermd

run:
	cargo run -p exaterm

daemon:
	cargo run -p exatermd

check:
	cargo check -p exaterm

test:
	cargo test -p exaterm

test-workspace:
	cargo test --workspace

core-test:
	cargo test -p exaterm-core

core-check:
	cargo check -p exaterm-core

daemon-check:
	cargo check -p exatermd

probe:
	cargo run -p exaterm-core --bin beachhead_probe

clean:
	cargo clean

help:
	@printf '%s\n' \
		'make            Build the GTK app (exaterm)' \
		'make run        Run the GTK app' \
		'make daemon     Run the daemon directly' \
		'make check      cargo check for the GTK app' \
		'make test       Run app-package tests' \
		'make test-workspace  Run the full workspace test suite' \
		'make core-test  Run core library tests' \
		'make daemon-check    Check the daemon package' \
		'make probe      Run the beachhead raw-path probe' \
		'make clean      Remove build artifacts'
