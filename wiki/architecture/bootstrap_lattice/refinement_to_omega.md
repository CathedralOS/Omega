# Refinement to production Omega

[Lattice overview](bootstrap_lattice.md) · [Decisions](decisions.md) ·
[Proof kernel](proof_kernel.md)

The bootstrap chain does not become trustworthy merely because each compiler
was built by the previous one. Each source-to-artifact edge must be checked
against an independently fixed semantic subject.

```text
Delta compiler source
  └─ lower-rung publication/refinement ─▶ Delta compiler artifact

exact ordinary-Omega compiler source C
  └─ checked Delta-produced compilation ─▶ omega₀

the same C
  └─ checked omega₀ compilation ─────────▶ omega
```

There is no separately owned bridge or bridge refinement. The first product
edge is simply the Delta-produced compiler compiling `C`.

## What transfers from the lower refinement work

- Reconstruct obligations from the source and produced artifact; do not accept
  the producer's description of its own question.
- Bind every claim to exact source, artifact, semantics, observation profile,
  target semantics, and certificate identity.
- Use negative controls that perturb source, artifacts, or derivations and must
  reject.
- Refuse unsupported shapes loudly. Missing coverage is not permission to skip
  an obligation.
- Keep proof search and compilation untrusted. The small checker validates the
  resulting derivation.

## New work at the Omega edge

Omega adds data-dependent control, compound values, effects, target
realization, and a much larger obligation set. Those require richer meanings
and certificate production, but not another compiler identity. The Rust
implementation under `source/omega-rust/` may help compare behavior while the
Omega-written path is completed; agreement with it grants no authority.

`source/delta/meaning/` retains the useful Rust-free elaboration pieces from the
removed experiment. They now serve Delta publication directly.

## Required joins

The proof kernel checks derivations; it does not choose their semantic subject.
Every accepted edge therefore records:

- the exact source closure;
- the exact produced artifact;
- the canonical semantics and observation profile;
- target-semantics dependencies where realization occurs;
- reconstructed obligations and their certificates;
- any irreducible admissions, transitively disclosed.

The live order and remaining closure work are tracked only in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).
