# Psi/Omega toolchain

[Lattice overview](bootstrap_lattice.md) | [Delta language rung](rungs/delta.md)

Omega is the product language and toolchain, not another Greek bootstrap rung.
Psi owns source processing through terminal portable IR; Omega consumes that IR
and performs target realization, optimization, and native emission. Today the
working implementations are primarily Rust. The hosted destination has one
profile-limited bridge compiler and one production compiler:

```text
Delta → omega-bootstrap (accepts Ωself) → omega (implements full Ω)
```

`omega-bootstrap` is written in Delta and contains only the Psi/Omega input
surface required by the exact production source closure. It is permitted to
reject proofs, dependent/linear types, and any other Omega construct absent from
that closure. Accepted constructs retain exact Omega semantics; this is not a
bootstrap dialect. The production compiler is written in Omega constrained to
that `Ωself` profile and implements the full specification for users.

The bridge binary may run slowly and lower the production compiler
conservatively. It must compile the `Ωself` source that implements the product
optimizer and advanced lowering, but need not duplicate those passes. A further
production self-rebuild can optimize the compiler binary; it is optional
evidence, not a required dependency.

The distinction is architectural:

- Alpha, Beta, Gamma, and Delta form the small language chain used to build the
  bridge compiler from the audited seed. Delta is independent, not an Omega
  subset requirement.
- `Ωself` is a mechanically enforced Omega source profile, not Epsilon or
  another language rung.
- The bridge compiler builds the full optimizing production compiler once from
  the exact `Ωself` source manifest; that compiler's own binary may initially be
  conservative.
- The Psi-aware artifact verifier reconstructs the obligations imposed by an
  exact terminal-Psi module; the [proof kernel](proof_kernel.md) independently
  checks the certificate derivations that discharge those obligations.

## Current repository roles

- `bootstrap/onramps/omega-rust/{psi,omega}/` is the current working Rust
  compiler and executable reference producer;
  `bootstrap/onramps/omega-rust/apps/omega-cli/` is its user-facing executable.
- `compiler/{psi,omega}/` is reserved for the eventual Omega-written product
  source. Those roots are placeholders today, not a compiler source tree.
- `bootstrap/omega-bootstrap/` is the owner for Rust-free meaning,
  Delta-written bridge-compiler slices/profiles, and bootstrap validation. Its
  architectural role is `omega-bootstrap`; the obsolete `omega0` label is not
  a compiler generation or language claim.
- `bootstrap/rungs/delta/` owns the bootstrap language corpus and Delta-written
  compiler; `bootstrap/onramps/delta-rust/` is its disposable Rust producer.
  Together their current gates are growing toward `omega-bootstrap` without
  assigning language ownership to Rust.

All Rust bootstrap/reference producers now live under explicitly suffixed
`bootstrap/onramps/` directories. In particular, the current Psi/Omega crates
do not claim the permanent unsuffixed product roots. See the [repository
structure](repository_structure.md).

Hosting does not by itself prove compiler correctness. A defect in
`omega-bootstrap` can reproduce while it builds production Omega. The value of
this shape is dependency closure: one checked hosted edge replaces a historical
tower of external implementation-language dependencies. Semantic correctness
still comes from the canonical meaning route, reconstructed proof obligations,
derivation checking, and translation validation across that edge.

The exact distinction between Delta's literal specification and `Ωself` is
defined in [`self_hosting_profile.md`](self_hosting_profile.md).

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

Bridge hosting and the one required production compile are tracked in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md). Native refinement evidence
and terminal-ledger migration remain product-assurance work under P3 in
[`TASKS.md`](../../../TASKS.md).
Production optimization remains outside the trusted proof kernel.

The first O0 console canary now has its canonical scalar boundary lane.
Terminal-Psi vocabulary 26 carries ordered scalar parameter types on boundary
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
26 bytes with ordinary `write_byte`, byte-identical to a fixture generated by
the shared codec; it has no private bridge IR or artifact buffer. Every truncated
prefix rejects. A Delta-written backend consumes that module
and emits the canonical 8 KiB Linux x86-64 ELF without an assembler or linker;
its output is byte-identical to the production lowering and malformed input
produces no output. O0 is therefore closed end to end. Generalizing that source
and artifact path into the `Ωself` bridge compiler remains open; general
full-Omega acceptance is not a requirement of that artifact.

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
