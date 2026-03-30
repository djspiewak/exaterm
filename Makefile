UNAME_S := $(shell uname -s)

ifeq ($(UNAME_S),Darwin)
APP_PACKAGE := exaterm-macos
else
APP_PACKAGE := exaterm-gtk
endif

.PHONY: all build build-app build-gtk build-macos run run-app run-gtk run-macos daemon check test test-workspace core-test core-check daemon-check clean help

all: build

build:
	cargo build

build-app:
	cargo build -p $(APP_PACKAGE)

build-gtk:
	cargo build -p exaterm-gtk

build-macos:
	cargo build -p exaterm-macos

run: run-app

run-app: build-app
	cargo run -p $(APP_PACKAGE)

run-gtk: build-gtk
	cargo run -p exaterm-gtk

run-macos: build-macos
	cargo run -p exaterm-macos

daemon:
	cargo run -p exatermd

check:
	cargo check

test:
	cargo test --workspace

test-workspace: test

core-test:
	cargo test -p exaterm-core

core-check:
	cargo check -p exaterm-core

daemon-check:
	cargo check -p exatermd

clean:
	cargo clean

help:
	@printf '%s\n' \
		'make              Build the default workspace for this platform' \
		'make build-app    Build the native frontend package for this platform' \
		'make run          Build and run the native frontend package for this platform' \
		'make build-gtk    Build the GTK frontend explicitly' \
		'make run-gtk      Build and run the GTK frontend explicitly' \
		'make build-macos  Build the macOS frontend explicitly' \
		'make run-macos    Build and run the macOS frontend explicitly' \
		'make daemon       Run the daemon directly' \
		'make check        Run cargo check for the default workspace' \
		'make test         Run the full workspace test suite' \
		'make core-test    Run core library tests' \
		'make daemon-check Check the daemon package' \
		'make clean        Remove build artifacts'
