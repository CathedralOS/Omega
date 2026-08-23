# Rust Alpha-assembler on-ramp

This disposable/reference producer reads Alpha assembly text and emits the
21-opcode bytecode tape consumed by the Alpha VM. Its produced artifact makes it
an **Alpha assembler** on-ramp. It does not parse Beta source, produce Alpha
assembly, or define the Beta language; the historical `beta-rs` name predates
the repository's role-based ownership.

The canonical assembler is
[`bootstrap/rungs/alpha/assembler/assembler.alpha`](../../rungs/alpha/assembler/assembler.alpha),
written in Alpha and run by the audited seed. That lattice-built implementation
owns normal bootstrap assembly and semantics. This Rust crate is untrusted,
optional cold-start/reference tooling: matching its output is useful bug-finding
evidence, not DDC and not source-to-artifact authority.

The binary accepts mnemonic or numeric Alpha assembly on standard input and
writes a raw tape on standard output. Optional input/output paths retain the
same behavior. `--num` rewrites mnemonic input into the numeric source form used
by the earliest assembler cold start.

Run the focused ownership/behavior gate from the repository root:

```sh
sh bootstrap/onramps/alpha-assembler-rust/test.sh
```

It compares the Rust output with the lattice-built assembler over the
self-hosting assembler and example corpus, checks the Rust-only historical
numeric transport and file
arguments, and pins fail-closed malformed-input behavior. `compiler/beta-rs` is
a temporary compatibility symlink to this directory.
