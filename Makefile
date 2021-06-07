build:
	@cargo build

clean:
	@cargo clean

TESTS = ""
test:
	@cargo test $(TESTS) --offline -- --color=always --test-threads=1 --nocapture

docs: build
	@cargo doc --no-deps

style-check:
	@rustup component add rustfmt 2> /dev/null
	cargo fmt --all -- --check

lint:
	@rustup component add clippy 2> /dev/null
	touch src/**
	cargo clippy --all-targets --all-features -- -D warnings

check:
	@rustup component add rustfmt 2> /dev/null
	cargo check

.PHONY: build test docs style-check lint
