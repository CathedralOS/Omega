# Alpha execution substrate

> **Current design.** Alpha is the 21-opcode tape VM at the bottom of the
> bootstrap chain. It is not an Omega subset, a self-hosted source language,
> or a compiler. The former “Alpha compiler written in Alpha” design has been
> retired.

Canonical architecture and executable details live in:

- [Bootstrap chain](../architecture/bootstrap_chain/bootstrap_chain.md)
- [Alpha rung](../architecture/bootstrap_chain/rungs/alpha.md)
- [`source/alpha/SEMANTICS.md`](../../source/alpha/SEMANTICS.md)

## Responsibility

Alpha supplies raw deterministic computation:

- fixed-width integer/register operations;
- bounded tape and data memory;
- load/store and branches;
- call/return control;
- byte input/output;
- halt and defined traps.

It does not parse source text, check proofs, define higher-language meaning, manage
ownership, allocate objects, or optimize code. The textual assembler is the
Beta rung and lives at `source/beta/compiler/`; its implementation is itself a
raw Alpha tape.

## Auditability constraints

- Keep the opcode set frozen unless a higher-rung construction demonstrates a
  concrete impossibility with the existing substrate.
- Specify every opcode as a small-step transition. Executable conformance tests
  accompany the written semantics but do not replace it.
- Treat memory capacity as an explicit execution parameter with a defined
  exhaustion/fault result. A platform-specific tape hole is an implementation
  capacity, not language meaning.
- Bounds faults, arithmetic faults, malformed opcodes, and boundary failures
  trap deterministically; they must not silently corrupt state.
- Keep platform I/O and image loading narrow and recorded in the native trust
  ledger.
- Keep the native realization small enough to inspect against the semantics.

## Determinism

Identical tape, input, and declared execution parameters produce identical
observable behavior. Determinism supports reproducible artifacts, fixed-point
diagnostics, cache identity, and audit. It is not a correctness proof.

The x64 Windows and arm64 macOS realizations run the same tapes and are checked
against the same semantics and conformance corpus. Their agreement is valuable
platform evidence; multiplicity does not grant semantic authority.

## Resource model

Alpha itself provides no heap or allocator abstraction. Programs may implement
arenas or allocation policies over their explicit memory region. Higher rungs
must define allocation, ownership, and exhaustion before relying on those
facilities semantically.

Current implementation capacities, including the platform-specific tape-hole
sizes, are tracked beside `seed_env.sh` and in
[TASKS_BOOTSTRAP.md](../../TASKS_BOOTSTRAP.md). They should not be copied into a
standing design brief as universal constants.

## Bootstrap role

The selected bootstrap lattice is:

```text
Alpha VM + admitted Beta compiler tape
  -> Beta-written Gamma evaluator
  -> Gamma-written Delta compiler
  -> Delta-written Epsilon compiler
  -> Epsilon-written Omega compiler D
  -> omega0
omega0 + Omega-written compiler C -> omega
```

The native Alpha executor is the sole per-platform binary. Beta is the first
trusted textual language; its compiler tape is admitted at the Alpha boundary
and its Beta source reconstructs that tape byte-identically.

Self-reproduction at any compiler stage establishes deterministic dependency
closure, not correctness. Compiler artifacts become authoritative only when the
exact artifact is checked to refine the canonical meaning of its exact source,
with the check rooted below the compiler being judged.

## Explicitly retired claims

The following ideas from the earlier design are no longer active:

- Alpha as a syntactic subset of Omega;
- an Alpha compiler written in Alpha;
- fixed compiler AST/token/symbol capacities as Alpha language semantics;
- a Rust Alpha compiler as the planned source of the steady-state seed.

Useful fixed-buffer and trap-on-failure instincts survive at the appropriate
rungs, but they no longer define Alpha's architectural role.
