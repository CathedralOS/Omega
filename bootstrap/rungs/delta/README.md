# Delta rung

This directory canonically owns the Delta language corpus, the Delta-written
self-hosting compiler, and lattice-built artifacts. Architectural ownership is
independent of the language used to produce an artifact.

Delta is an independent compiler-host language, not an Omega subset. Its open
literal-v1 contract and its role in building `omega-bootstrap` are tracked in
[`../../../TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

- [`samples/`](samples/) contains the executable language corpus.
- [`samples/lowermachine.alp`](samples/lowermachine.alp) is the Delta-written
  Delta-to-ARM64 compiler and self-host fixed point.
- [`build/`](build/) contains the checked-in bootstrap compiler artifacts.
- [`../../onramps/delta-rust/`](../../onramps/delta-rust/) is the disposable
  Rust producer and executable reference. It is not Delta's semantic authority.

The lower-rung Delta-to-Gamma route under [`../../omega-bootstrap/meaning/`](../../omega-bootstrap/meaning/)
defines the meaning profile being widened toward `omega-bootstrap`.
`compiler/delta` and `compiler/delta-rs` remain historical compatibility paths;
new gates use the `delta` and `delta-rs` roles from `bootstrap/paths.sh`.
