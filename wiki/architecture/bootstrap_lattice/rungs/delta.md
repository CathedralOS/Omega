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

Delta supplies only the systems machinery the bootstrap Psi/Omega bridge
actually needs. The current experiments demonstrate mutable storage, state
machines, arenas, effects, and boundary declarations, but none of those
mechanisms is admitted to v1 merely because it exists today. Delta remains an
independent, deterministic compiler-host language; it should resemble Omega
where consistency is cheap, but it need not be an Omega subset and may be slow
and conservatively lowered.

Delta v1 is designed around the complete `omega-bootstrap` source closure. D0,
the sample corpus, and the Rust producer may reveal useful facilities but cannot
admit them. Exact arithmetic, ordinary fixed backing, explicit tags, and a
sealed byte-I/O host surface are simpler candidates to measure first. Broader
arithmetic domains, allocation machinery, payload sums, or general boundary
traits enter v1 only when they reduce total bridge-source and assurance cost or
make the compiler-host language materially safer, more coherent, or less
brittle; their omission is not a goal in itself.

Its job is to implement `omega-bootstrap`, which accepts the exact Omega
product-compiler source profile `Ωself` and rejects the rest. That bridge
compiles the `Ωself`-constrained production source into the full optimizing
compiler. That compiler's own machine code may initially be conservative.

Delta's own self-hosting compiler and `omega-bootstrap` are distinct artifacts.
The former establishes and exercises the Delta language; the latter is the
Delta program that compiles `Ωself` Omega source. Success of the former proves
compiler-host feasibility, not completeness of the latter.

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
  `omega-bootstrap` remains
  the closure criterion.
- `bootstrap/rungs/delta/build/` contains the checked-in bootstrap binaries
  produced by this work.
- `bootstrap/rungs/delta/samples/bootstrap-storage.alp` is the first fixed-backing
  storage profile canary. It uses checked integer-offset reservations and bulk
  reset without adding pointers or a general heap to Delta.
- `bootstrap/onramps/delta-rust/delta-storage-meaning.sh` evaluates that canary and a
  perturbation through `omega2gamma.beta` and Gamma's `interp.beta`, without the
  Rust Gamma emitter defining the result.
- `bootstrap/omega-bootstrap/compiler/omega-bootstrap-frontend.alp` is the first actual bridge
  compiler slice written in Delta. It decodes the complete bounded canonical
  bundle, retains unit provenance, validates each unit independently, and admits
  one O0/O1 program-bearing unit plus trivia-only auxiliaries without cross-unit
  token fusion. It then performs exact name/type/count checks and retains a
  checked table of console-boundary operands. Its focused gate also recompiles it
  through Delta-written `lowermachine`; the lower-rung meaning gate executes the
  complete frontend through `omega2gamma.beta` and Gamma. The old Delta-sample
  path is compatibility plumbing.
- `bootstrap/omega-bootstrap/compiler/BOOTSTRAP_PROFILES.md` freezes the current Delta D0
  implementation profile and Omega O0/O1 vertical-canary input profiles; the
  production `Ωself` profile remains source-derived.
- `bootstrap/omega-bootstrap/compiler/omega-bootstrap-terminal-to-elf.alp` is the first direct
  artifact backend. It emits exact O0/O1 Linux x86-64 images without a host
  assembler or linker; general Omega lowering remains open.

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
Delta → omega-bootstrap (accepts Ωself)
      → omega (implements full Ω)
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
complete bridge source demonstrates another unavoidable host operation, it must
be specified and added to the trust ledger explicitly.

## Open work

- Complete the Rust-free Delta implementation and keep its self-host fixed point.
- Maintain Delta's provisional feature ledger while implementing the bridge,
  then remove accidental producer/corpus behavior and freeze a coherent,
  robust literal specification containing the complete `omega-bootstrap`
  source closure. Preserve Omega spelling and ordinary meaning for retained
  shared constructs without forcing subset compatibility.
- Build `omega-bootstrap` with exact `Ωself` acceptance and enough conservative
  lowering to compile the source that implements the production optimizer and
  advanced lowering; do not duplicate those product passes in Delta.
- Use it once to build and validate the full-spec production compiler from
  `Ωself`-constrained Omega source.
- Continue widening the Delta-to-Gamma meaning route beyond the now-gated O1
  frontend. Native, Delta-self-hosted, and Beta-to-Gamma execution agree on its
  canonical retained-operand digest; the lower-rung route also pins zero/two-
  write observations and semantic rejection independently of the Rust producer.
- Keep the now-logical `lowermachine` table/source arenas within the checked D0
  backing convention as compiler capacity grows. Compiler tables use integer
  offsets in one typed backing extent; source storage grows in an explicit byte
  backing and rejects exhaustion rather than truncating input.
