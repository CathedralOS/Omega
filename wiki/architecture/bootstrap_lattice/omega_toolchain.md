# Psi/Omega toolchain

[Lattice overview](bootstrap_lattice.md) | [Delta language rung](rungs/delta.md)

Omega is the product language and toolchain, not another Greek bootstrap rung. Psi owns source
processing through terminal portable IR; Omega consumes that IR and performs
target realization, optimization, and native emission. Today the working
implementations are primarily Rust. The bootstrap destination has two compiler
artifacts for the same Omega language:

```text
Delta → Omega (simple, Delta-built) → Omega (optimized, Omega-built)
```

The first compiler contains the Psi source/semantic path and enough target
realization to compile conforming Omega programs. It deliberately omits advanced
optimization and may compile slowly or emit slow code. The second is the full
production compiler written in Omega and built by the first.

The distinction is architectural:

- Alpha, Beta, Gamma, and Delta form the small language chain used to build the
  first Omega compiler from the audited seed.
- The Delta-built Omega compiler is a valid self-sufficient endpoint.
- That compiler then builds the optimized Omega compiler from Omega source. The
  repeated Omega is a self-host edge, not another language rung.
- The Psi-aware artifact verifier reconstructs the obligations imposed by an
  exact terminal-Psi module; the [proof kernel](proof_kernel.md) independently
  checks the certificate derivations that discharge those obligations.

## Current repository roles

- `bootstrap/onramps/omega-rust/{psi,omega}/` is the current working Rust
  compiler and executable reference producer;
  `bootstrap/onramps/omega-rust/apps/omega-cli/` is its user-facing executable.
- `compiler/{psi,omega}/` is reserved for the eventual Omega-written product
  source. Those roots are placeholders today, not a compiler source tree.
- `bootstrap/omega0/` owns Rust-free meaning, current Delta-written compiler
  slices/profiles, and bootstrap validation; it has no alias at the product root.
- `bootstrap/rungs/delta/` owns the bootstrap language corpus and Delta-written
  compiler; `bootstrap/onramps/delta-rust/` is its disposable Rust producer.
  Together their current gates are growing toward the simple bootstrap Omega
  compiler without assigning language ownership to Rust.

All Rust bootstrap/reference producers now live under explicitly suffixed
`bootstrap/onramps/` directories. In particular, the current Psi/Omega crates
do not claim the permanent unsuffixed product roots. See the [repository
structure](repository_structure.md).

Self-hosting does not by itself prove compiler correctness. A defect in bootstrap
Omega can reproduce while it builds production Omega. The value of this shape is
dependency closure: one checked self-host edge replaces a historical tower of
external implementation-language dependencies. Semantic correctness still comes
from the canonical meaning route, reconstructed proof obligations, derivation
checking, and translation validation across that edge.

The current Rust `psi-terminal-verifier` demonstrates the artifact-aware half:
it validates canonical terminal Psi, reconstructs its exact obligation set,
rejects missing or extra evidence, and produces `VerifiedTerminalModule`. It is
not interchangeable with the generic proof kernel and remains an explicit
trusted migration dependency. The final hosted architecture uses one total low-
rung semantic-ledger definition over canonical bytes. Direct evaluation or a
checked derivation of that definition establishes every deployed artifact's
ledger; Rust agreement grants no authority. Local operation denotations and
canonical goals come from restricted declarative schemas, while algebraic
reduction is untrusted and must emit a checked proof of the unchanged goal.

Bootstrap-Omega hosting and the Omega self-build are tracked in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md). Native refinement evidence
and terminal-ledger migration remain product-assurance work under P3 in
[`TASKS.md`](../../../TASKS.md).
Production optimization remains outside the trusted proof kernel.

The first O0 console canary now has its canonical scalar boundary lane.
Terminal-Psi vocabulary 25 carries ordered scalar parameter types on boundary
declarations and ordered scalar values on `BoundaryCall`; the checked producer,
codec, semantic schema, verifier, interpreter, and Omega abstract consumer all
preserve that lane. This closed an implementation seam, not a language ruling.

The import-free Linux `exit_process(i32)` realization now consumes that scalar
through `exit_group` on x86-64 and AArch64, records its exact native settlement
interval, and traps if the nominally nonreturning syscall returns. Darwin and
Windows stay fail-closed pending validated import and relocation evidence.

The literal `write_line` console boundary now crosses the same canonical route.
Psi owns exact raw bytes through syntax, resolved, typed, checked, and terminal
forms. Omega retains the literal place and bytes through abstract, target,
assigned, machine, object, image, and installation custody. Linux x86-64 and
AArch64 use an import-free short-write loop over the exact bytes plus one
newline; Darwin, Windows, nonliteral forwarding, and in-module literal calls
remain fail-closed.

O0 now retains `Main { console: Console }` honestly as an attached machine with
one relevant erased provider field and exact provider roots for the two bodyless
Console requirements. The Delta frontend streams the same canonical vocabulary
25 bytes with ordinary `write_byte`, byte-identical to a fixture generated by
the shared codec; it has no private Omega0 IR or artifact buffer. Every truncated
prefix rejects. A Delta-written backend consumes that module
and emits the canonical 8 KiB Linux x86-64 ELF without an assembler or linker;
its output is byte-identical to the production lowering and malformed input
produces no output. O0 is therefore closed end to end. Generalizing that source
and artifact path into the first spec-compliant Omega compiler remains open.

O1 is the first variable rather than fixture-shaped step: one bounded
statement table accepts 0–16 literal `write_line` operations followed by one
final literal `exit_process`, and one loop emits and lowers the resulting
variable operation sequence. Delta-emitted terminal modules and direct x86-64
images match the product pipeline for 0/1/2/16 writes, while table/text overflow
and malformed order fail before artifact publication. This required no new Psi
vocabulary or language ruling. The complete backend now also runs through the
Beta-written Omega-to-Gamma elaborator and canonical Gamma interpreter: exact
canonical and variant images agree with native Delta execution, while malformed
and exhausted inputs agree on status and empty output. This closes O1's used
Delta meaning profile; it does not generalize O1 into the production compiler.
