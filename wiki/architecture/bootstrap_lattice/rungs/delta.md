# Rung: Delta — compiler-host systems language

[Lattice overview](../bootstrap_lattice.md) | Prev: [Gamma](gamma.md) | Next: —

> **Status: WORKING RUNG.** `bootstrap/rungs/delta/` owns the language corpus,
> Delta-written compiler, and lattice-built artifacts. The disposable Rust
> producer lives separately under `bootstrap/onramps/delta-rust/`. Native,
> self-hosting, and Delta-to-Gamma meaning gates exist today; exact lower-rung
> coverage of the eventual `omega-bootstrap` compiler remains open.

Delta is the terminal small/Greek language rung in the bootstrap spine:

```text
Alpha → Beta → Gamma → Delta
```

Delta supplies a coherent systems/compiler-host surface sufficient to write the
bootstrap Psi/Omega bridge robustly. The current experiments demonstrate
mutable storage, state machines, arenas, effects, and boundary declarations,
but none of those mechanisms is admitted to v1 merely because it exists today.
Delta remains an independently specified, deterministic compiler-host language;
it should resemble Omega where consistency is cheap, but shared spelling does
not make it an Omega subset. Delta compiler artifacts may be slow and
conservatively lowered; those are artifact properties, not language features.

Delta v1 is designed around the complete canonical Delta-compiler and
`omega-bootstrap` source closures plus explicit coherence, safety, robustness,
and maintainability arguments. D0, the sample corpus, and the Rust producer may
reveal useful facilities but cannot admit them. The design floor is a regular
C-class compiler host: scalar and aggregate data, structured control, modules,
deterministic bounded storage or allocation with explicit exhaustion, and a
sealed byte/artifact/diagnostic/exit boundary. Records, payload sums, arrays,
slices, and arena-style storage are ordinary candidates for meeting that floor;
forcing tag-plus-payload records or hand-expanded buffers is not intrinsically
simpler.

The exact arithmetic, representation, call, allocation, and module forms still
resolve from whole-bootstrap cost and lower-rung meaning. Fixed, bump, or paged
backing may implement deterministic allocation without copying Omega's
production allocator model. Broader arithmetic domains and general boundary
traits enter v1 only when they reduce total required-source and assurance cost
or make the language materially safer, more coherent, or less brittle. Omission
is not a goal in itself. Delta is therefore neither a token census of the
required programs nor a disguised subset of Omega.

Its job is to implement `omega-bootstrap`, which accepts the exact Omega
product-compiler source profile `Ωself` and rejects the rest. That bridge
compiles the `Ωself`-constrained production source into the full optimizing
compiler. That compiler's own machine code may initially be conservative.

Delta's own self-hosting compiler and `omega-bootstrap` are distinct artifacts.
The former establishes and exercises the Delta language; the latter is the
Delta program that compiles `Ωself` Omega source. Success of the former proves
compiler-host feasibility, not completeness of the latter.

The completed cold publication of that compiler is lower-rooted: the
Beta-written Delta→Gamma elaborator translates the exact Delta compiler source,
and Gamma's Beta-written interpreter executes it on that same source to emit the
native compiler artifact. The current route already runs the complete compiler
on bounded inputs; complete source coverage and artifact publication remain
open engineering work. This is the concrete `Gamma → Delta` edge. A Rust-built
or Delta-self-built artifact may remain a differential control, but neither may
substitute for this required publication and refinement join.

## Implementation

- `bootstrap/rungs/delta/samples/` is the canonical executable language corpus.
- `bootstrap/rungs/delta/samples/lowermachine.alp` is the self-hosting compiler
  written in Delta.
- `bootstrap/onramps/delta-rust/` is the current disposable Rust producer and
  executable reference. The former `compiler/delta-rs` entry is retired.
- `DELTA_EMIT=gamma` exposes the Rust reference elaborator. The
  `delta-meaning-diamond.sh` gate compares it with native execution; it is useful
  regression evidence, not the final authority.
- `bootstrap/omega-bootstrap/meaning/omega2gamma.beta` is the lower-rung, Rust-free elaborator for
  the shared Delta/Omega machine surface. Gamma's `interp.beta` executes its
  result. Exact coverage of the Delta source eventually used by
  both the Delta compiler and `omega-bootstrap` remains the closure criterion.
- `bootstrap/rungs/delta/build/` contains the checked-in bootstrap binaries
  produced by this work.
- `bootstrap/rungs/delta/samples/bootstrap-storage.alp` is the first fixed-backing
  storage profile canary. It uses checked integer-offset reservations and bulk
  reset without adding pointers or a general heap to Delta.
- `bootstrap/onramps/delta-rust/delta-storage-meaning.sh` evaluates that canary and a
  perturbation through `omega2gamma.beta` and Gamma's `interp.beta`, without the
  Rust Gamma emitter defining the result.
- `bootstrap/omega-bootstrap/compiler/omega-bootstrap-frontend.alp` is the
  historical O0/O1 and bounded-scalar bridge regression slice written in Delta.
  It decodes the complete bounded canonical
  bundle, retains unit provenance, validates each unit independently, and admits
  one program-bearing unit plus trivia-only auxiliaries without cross-unit token
  fusion. It then performs exact O0/O1 console checks or bounded scalar-call
  symbol/signature/value/graph checks. Its focused gates also recompile it
  through Delta-written `lowermachine`; the lower-rung meaning gate executes the
  complete frontend through `omega2gamma.beta` and Gamma. The old Delta-sample
  path is compatibility plumbing.
- `bootstrap/omega-bootstrap/compiler/BOOTSTRAP_PROFILES.md` freezes the current Delta D0
  implementation profile, Omega O0/O1 vertical-canary input profiles, and the
  profile-neutral scalar-call conformance slice; the production `Ωself` profile
  remains source-derived.
- `bootstrap/omega-bootstrap/compiler/omega-bootstrap-terminal-to-elf.alp` is
  the matching historical direct-artifact backend. It emits exact O0/O1 and
  bounded scalar-call Linux x86-64 images without a host assembler or linker;
  general Omega lowering remains open.
- `bootstrap/omega-bootstrap/compiler/omega-bootstrap-source-custody-check.alp`
  is the first checkpoint-driven general frontend cost probe. Its corresponding
  artifact task has selected a private versioned checked IR and direct
  conservative backend rather than a Terminal-Psi widening. That choice does
  not freeze Delta v1 or admit its Omega source facilities to `Ωself`.

## Relationship to Psi and Omega

Delta is a bootstrap language, not the product language. Psi owns the front end
and terminal portable IR; Omega consumes terminal Psi and performs target
realization and code generation. Delta hosts only the `Ωself` input path needed
by the exact production compiler source. Unsupported Omega features must reject
rather than acquire approximate bootstrap semantics.

The resulting bridge binary may itself run slowly and lower the product
compiler conservatively. It must compile the `Ωself` source that implements the
product optimizer and advanced lowering; it need not duplicate those passes:

```text
Delta compiler source ──[Delta→Gamma + Gamma execution]──▶ delta compiler
Delta bridge source ──[delta compiler]───────────────────▶ omega-bootstrap
Ωself product source ──[omega-bootstrap]──────────────▶ omega (implements full Ω)
Ωself product source ──[optional omega rebuild]───────▶ omega (same compiler; optimized binary)
```

See [`../compiler_source_profile.md`](../compiler_source_profile.md) for the separate
Delta and `Ωself` feature budgets.

## Proofs

Delta programs may emit proof certificates, but proof checking is not a Delta
language feature or a fifth rung. The cross-cutting [proof kernel](../proof_kernel.md)
checks certificates using independent Beta and Gamma implementations.
Their cross-check is regression evidence; artifact authority comes from the
kernel's soundness and the independently reconstructed obligation, not from
implementation agreement.

## Trust boundary

Most candidate Delta facilities erase into lower-rung computation. The
provisional bridge host surface is only source-byte input, artifact-byte output,
diagnostic-byte output, and process termination. Target configuration is
explicit input; filesystem, environment, clock, network, process-spawn, atomics,
MMIO, interrupt, and general foreign-call authority are not presumed. If the
complete canonical-compiler or bridge source demonstrates another unavoidable
host operation, it must be specified and added to the trust ledger explicitly.

## Closure criteria

Delta closes only when its complete deterministic canonical-compiler and
`omega-bootstrap` source closures are valid under one versioned, independently
specified Delta contract; the lattice-built compiler accepts all conforming
programs within published bounds; the exact Delta compiler artifact can be
published by the Delta→Gamma/Gamma route without Rust; and native, self-built,
and lower-rung routes agree at their declared observations. The resulting
bridge must then accept exactly frozen `Ωself` and perform the one required
hosted production build. Exact execution order and current bridge capabilities
live only in
[`TASKS_BOOTSTRAP.md`](../../../../TASKS_BOOTSTRAP.md); this rung definition is
not a second task queue.
