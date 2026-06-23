# Rung: Epsilon — safe systems programming

[Lattice overview](../bootstrap_lattice.md) | Prev: [Delta](delta.md) | Next: [Omega](omega.md)

> **Status: DIRECTION.** Does not exist yet as part of the lattice.
>
> **Naming collision:** the current `compiler/epsilon/` folder is unrelated
> legacy experiment soup (a renamed/parked structured-language compiler), to be
> cleared. Do not conflate it with this rung.

Once the [evidence machinery](delta.md) exists, epsilon introduces real systems
programming — the features that are *checked then erased*, so they do not expand
[Alpha](alpha.md).

## Adds

- mutable memory
- ownership, regions
- effects
- (and toward the top) threads, traits, generic types

These are largely **static discipline**: checked at a higher rung, then erased to
ordinary alpha-level computation. They expand the amount of source and formal
definition *above* alpha; they do not expand the native seed.

## Written in

Delta / Gamma (its checker and reference interpreter sit on the evidence rung).

## Meaning

An epsilon program means what an **epsilon reference interpreter** does; its
safety obligations are discharged as [Delta](delta.md) certificates.

## Must not contain

Full contracts, refinement, and dependent types with automation — those are
[Omega](omega.md). Epsilon is the safe-systems layer beneath the
verification-oriented surface.

## The hardware boundary (honest edge #5)

Most epsilon features are static and never touch alpha. But systems and OS work
need a **runtime and hardware access**: allocator, possibly a collector, atomics,
memory fences, MMIO, interrupt entry. These either reduce to alpha's existing ops
or form a **second native boundary** that grows with hardware targets (Cathedral
already needs atomics-as-real-LOCK). Freeze the computational core of alpha;
deliberately manage this separate hardware-interface surface — it is the one place
"alpha never grows" does not hold.

## Open questions

- Which systems features are pure static discipline (erased) vs which need runtime
  or native support.
- The hardware-interface boundary: its minimal surface, and how it is audited
  (it is part native, so part of the trust ledger).
- Clearing/renaming the legacy `compiler/epsilon/` folder so the name is free.
