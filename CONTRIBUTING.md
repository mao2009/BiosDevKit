# Contributing

Please open an Issue before substantive changes. Follow **one focused Issue per PR** and open changes as Draft pull requests.

## Rust checks

Run:
```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo run --locked -- --help
```

Core components must stay architecture-neutral. CPU, BIOS ABI and emulator-specific logic belongs in platform adapters.

Use synthetic test fixtures. Do not commit proprietary BIOS dumps or commercial ROM images. Do not claim compatibility from a passing unit test alone.
