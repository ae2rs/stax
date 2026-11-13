# The local gate: formatting, lints, tests.
check: fmt clippy test

fmt:
    cargo fmt --check

clippy:
    cargo clippy --all-targets -- -D warnings

test:
    cargo nextest run --status-level fail --final-status-level fail

install:
    cargo install --path . --locked
