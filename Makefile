.PHONY: fmt fmt-check lint build docs test test-isolated check check-isolated

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

test-isolated:
	bash tools/test-isolated.sh

check-isolated:
	bash tools/check-isolated.sh

check: fmt-check lint build docs test
