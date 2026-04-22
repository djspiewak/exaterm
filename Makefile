UNAME_S := $(shell uname -s)

ifeq ($(UNAME_S),Darwin)
APP_PACKAGE := exaterm-macos
# Wipe inherited shell-state vars (e.g. __PROFILE_SOURCED) so the login wrapper
# sources ~/.profile cleanly, matching a Finder/Dock launch from launchd.
RUN_ENV := env -i HOME="$$HOME" PATH="$$PATH" USER="$$USER" SHELL="$$SHELL" LOGNAME="$$LOGNAME" TMPDIR="$$TMPDIR" LANG="$$LANG" TERM="$$TERM"
else
APP_PACKAGE := exaterm-gtk
RUN_ENV :=
endif

.PHONY: all build build-app build-gtk build-macos run run-app run-gtk run-macos daemon check test test-workspace core-test core-check daemon-check clean help

all: build

build:
	cargo build -p exaterm-types -p exaterm-core -p exaterm-ui -p $(APP_PACKAGE) -p exatermd

build-app:
	cargo build -p $(APP_PACKAGE) -p exatermd

build-gtk:
	cargo build -p exaterm-gtk -p exatermd

build-macos:
	cargo build -p exaterm-macos -p exatermd

run: run-app

run-app: build-app
	$(RUN_ENV) cargo run -p $(APP_PACKAGE)

run-gtk: build-gtk
	cargo run -p exaterm-gtk

run-macos: build-macos
	$(RUN_ENV) cargo run -p exaterm-macos

daemon:
	cargo run -p exatermd

check:
	cargo check -p exaterm-types -p exaterm-core -p exaterm-ui -p $(APP_PACKAGE) -p exatermd

test:
	cargo test -p exaterm-types -p exaterm-core -p exaterm-ui -p $(APP_PACKAGE) -p exatermd

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
		'make              Build the default app and daemon for this platform' \
		'make build-app    Build the native frontend package for this platform' \
		'make run          Build and run the native frontend package for this platform' \
		'make build-gtk    Build the GTK frontend explicitly' \
		'make run-gtk      Build and run the GTK frontend explicitly' \
		'make build-macos  Build the macOS frontend explicitly' \
		'make run-macos    Build and run the macOS frontend explicitly' \
		'make daemon       Run the daemon directly' \
		'make check        Check the default app and daemon for this platform' \
		'make test         Run the default app, core, UI, and daemon tests' \
		'make core-test    Run core library tests' \
		'make daemon-check Check the daemon package' \
		'make clean        Remove build artifacts'
