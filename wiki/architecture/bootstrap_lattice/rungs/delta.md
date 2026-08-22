# Rung: Delta — compiler-host systems language

[Lattice overview](../bootstrap_lattice.md) | Prev: [Gamma](gamma.md) | Next: —

> **Status: WORKING ON-RAMP.** `compiler/delta-rs/` is the disposable Rust
> implementation. Its native corpus, self-hosting compiler, and Delta-to-Gamma
> meaning diamond exist today. The implementation is still being moved fully
> onto the audited bootstrap lineage.

Delta is the terminal small/Greek language rung in the bootstrap spine:

```text
Alpha → Beta → Gamma → Delta
```

It adds the systems machinery needed to build the bootstrap Psi/Omega toolchain:
mutable storage, state machines, ownership and regions, effects, boundary
operations, and the compiler-scale data structures built from them. Delta may be
slow and conservatively lowered. Its job is to build a simple, spec-compliant
Omega compiler from the audited seed without Rust or another external compiler.
That compiler then builds the full optimizing Omega compiler from Omega source.

## Implementation

- `compiler/delta-rs/` is the current Rust on-ramp and executable specification.
- `compiler/delta-rs/samples/lowermachine.alp` is the self-hosting compiler
  written in the language.
- `DELTA_EMIT=gamma` exposes the Rust reference elaborator. The
  `delta-meaning-diamond.sh` gate compares it with native execution; it is useful
  regression evidence, not the final authority.
- `compiler/omega/omega2gamma.beta` is the lower-rung, Rust-free elaborator for
  the shared Delta/Omega machine surface. Gamma's `interp.beta` executes its
  result. Exact coverage of the Delta source eventually used by Omega0 remains
  the closure criterion.
- `compiler/delta/` contains the checked-in bootstrap binaries produced by this
  work.
- `compiler/delta-rs/samples/bootstrap-storage.alp` is the first fixed-backing
  storage profile canary. It uses checked integer-offset reservations and bulk
  reset without adding pointers or a general heap to Delta.
- `compiler/delta-rs/delta-storage-meaning.sh` evaluates that canary and a
  perturbation through `omega2gamma.beta` and Gamma's `interp.beta`, without the
  Rust Gamma emitter defining the result.
- `compiler/delta-rs/samples/omega0-frontend.alp` is the first actual Omega0
  compiler slice written in Delta. It decodes a canonical single-source bundle,
  lexes and parses the frozen O0 shape, performs its exact name/type/count
  checks, and retains the two console-boundary operands. Its focused gate also
  recompiles it through Delta-written `lowermachine`.
- `compiler/omega/BOOTSTRAP_PROFILES.md` freezes the Delta D0 implementation
  profile and Omega O0 vertical-canary input profile in their transitional home.

## Relationship to Psi and Omega

Delta is a bootstrap language, not the product language. Psi owns the front end
and terminal portable IR; Omega consumes terminal Psi and performs target
realization and code generation. Delta first hosts the smallest conforming
Psi/Omega path capable of compiling Omega source. Advanced lowering and
optimization are intentionally unnecessary at this edge.

The result is a bootstrap Omega compiler. It is a valid stopping point, although
it may compile slowly and emit minimally optimized code. It then compiles the
full production compiler written in Omega:

```text
Delta → Omega (Delta-built bootstrap compiler)
      → Omega (Omega-built production compiler)
```

## Proofs

Delta programs may emit proof certificates, but proof checking is not a Delta
language feature or a fifth rung. The cross-cutting [proof kernel](../proof_kernel.md)
checks certificates using independent Beta and Gamma implementations.
Their cross-check is regression evidence; artifact authority comes from the
kernel's soundness and the independently reconstructed obligation, not from
implementation agreement.

## Trust boundary

Most Delta facilities erase into lower-rung computation. Native hardware
operations—atomics, fences, MMIO, interrupt entry, and platform runtime calls—are
explicit boundary surfaces and remain in the platform trust ledger.

## Open work

- Complete the Rust-free Delta implementation and keep its self-host fixed point.
- Make the rung sufficient to build the spec-compliant bootstrap Omega compiler.
- Use bootstrap Omega to build and validate the full Omega-source production
  compiler.
- Continue widening the Delta-to-Gamma meaning route and its differential gates.
- Extend that lower-rung route to the O0 frontend; native and Delta-self-hosted
  execution agree today, while the current Beta-to-Gamma path rejects the
  canonical frontend run and therefore does not yet grant semantic authority.
- Keep the now-logical `lowermachine` table/source arenas within the checked D0
  backing convention as compiler capacity grows. Compiler tables use integer
  offsets in one typed backing extent; source storage grows in an explicit byte
  backing and rejects exhaustion rather than truncating input.
