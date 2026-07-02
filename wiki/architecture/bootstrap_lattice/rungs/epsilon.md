# Rung: Epsilon — safe systems programming

> **Status: ABSORBED INTO OMEGA (2026-07-02, decision D7 in [decisions.md](../decisions.md)).**
> Epsilon is no longer a separate rung. Its surface is the **Omega kernel subset**; its Rust-free
> meaning translator lives on as `compiler/omega/omega2gamma.beta`; its kept gates (the kernel
> diamond, the Rust-free proof-carrying convergence) moved to `compiler/omega/`. `compiler/epsilon-rs/`
> keeps its historical name as the disposable Rust producer for the kernel subset. The ladder is now
> α → β/bc → γ → δ → ω. The text below is retained as the design record of what the kernel subset adds.

[Lattice overview](../bootstrap_lattice.md) | Prev: [Delta](delta.md) | Next: [Omega](omega.md)

> **Status: DIRECTION.** The on-ramp (`compiler/epsilon-rs/`, throwaway Rust) and
> its self-hosting backend (`lowermachine.alp`) exist; the rung's *meaning* (a
> reference interpreter in Delta/Gamma) is approximated by the epsilon-meaning
> diamond (`EPS_EMIT=gamma` → `interp.beta`). (The old misfiled `compiler/epsilon/`
> alpha-in-alpha experiment has been pruned, freeing the name.)

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
