.PHONY: fmt fmt-check lint build docs test check

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

build:
	cargo build --all-targets --all-features

docs:
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

test:
	cargo test --all-targets --all-features

check: fmt-check lint build docs test
