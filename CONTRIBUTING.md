# Contributing

Use Rust 1.90 or later and synthetic test data only. Never use real customer
licenses in tests or include private state directories in commits.

```sh
cargo +1.90.0 fmt --check
cargo +1.90.0 clippy --all-features --all-targets --locked -- -D warnings
cargo +1.90.0 test --all-features --locked
cargo +1.90.0 package --locked
```

The normal suite uses local synthetic signatures and HTTP fixtures. The ignored
backend test requires KDPika's isolated PHP/MySQL acceptance fixture; it must not
be pointed at a customer account or production license.

Preserve fail-closed access decisions, stable installation identity, exact signed
bytes, bounded offline policy and safe logging. Add a focused regression test for
behavior changes. Open a pull request describing the change and checks run.
Report vulnerabilities privately as described in SECURITY.md.
