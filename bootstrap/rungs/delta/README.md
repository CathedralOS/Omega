# Delta rung

This directory canonically owns the Delta language corpus, the Delta-written
self-hosting compiler, and lattice-built artifacts. Architectural ownership is
independent of the language used to produce an artifact.

- [`samples/`](samples/) contains the executable language corpus.
- [`samples/lowermachine.alp`](samples/lowermachine.alp) is the Delta-written
  Delta-to-ARM64 compiler and self-host fixed point.
- [`build/`](build/) contains the checked-in bootstrap compiler artifacts.
- [`../../onramps/delta-rust/`](../../onramps/delta-rust/) is the disposable
  Rust producer and executable reference. It is not Delta's semantic authority.

The lower-rung Delta-to-Gamma route under [`../../omega0/meaning/`](../../omega0/meaning/)
defines the meaning profile being widened toward the first Omega compiler.
`compiler/delta` and `compiler/delta-rs` remain temporary compatibility paths;
new gates use the `delta` and `delta-rs` roles from `bootstrap/paths.sh`.
