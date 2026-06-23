# Rung: Omega — the full language

[Lattice overview](../bootstrap_lattice.md) | Prev: [Epsilon](epsilon.md) | Next: —

> **Status: DIRECTION** as a *lattice-built* rung. The language itself is real and
> documented; today it is realized by `omega-rs`, the throwaway Rust compiler.

Omega is the full verification-oriented language: everything below, plus the proof
surface. As a lattice rung it adds no new native machinery — it expands the source
and formal definition running above [Alpha](alpha.md).

## Adds

- contracts (`requires`/`ensures`)
- refinement and dependent types
- proof automation as an **untrusted front line** that emits certificates the
  [Delta](delta.md) checker validates

The intended synthesis (see
[`proof_engine_north_star.md`](../../../design_briefs/proof_engine_north_star.md)):
automation discharges the easy 95% with zero hand-proving, a tiny kernel checks
the hard 5%, and SMT-class procedures emit kernel-checkable certificates — strictly
better than Lean (less hand-proving) and than SPARK/pure-SMT (a smaller trusted
base, and it can do the hard cases at all).

## Written in

Epsilon, in the lattice end-state. **Self-hosting Omega-in-Omega** is permitted
only as an *accelerator*: its output stays on the untrusted side and is checked by
the lower kernel. Self-hosting adds **zero** trust — that is the point. It is
dogfooding, not a security milestone.

## Meaning

Omega means what its **written semantics + reference interpreter** define. The
production compiler (whether `omega-rs` today or a self-hosted Omega later) is an
acceleration whose output is checked against that meaning.

## Must not contain

Nothing is "above" omega to exclude — but the discipline is: keep the
**trusted** part (the delta checker, the semantics) small, and keep all the large
machinery (parser, elaborator, SMT, optimizer, the whole `omega-rs` pipeline) on
the **untrusted, checked** side. Size and cleverness are fine there; authority is
not.

## For Cathedral

In a single-address-space OS, the compiler *is* the isolation boundary, so
"output is checked, not trusted" means the safety of the entire OS reduces to {the
delta checker is correct, the chip obeys its manual}. An operating system whose
security rests on a tiny audited checker plus one hardware assumption. See
[`cathedral_alignment.md`](../../../cathedral_alignment.md).

## Current repo reality

`omega-rs` is the real compiler today: an 11-stage multi-IR pipeline
(source → … → machine bytes) with a contract-entailment engine and a differential
interpreter oracle as the interim correctness mitigation. In this architecture it
is the **fast untrusted producer** and the current executable reference for the
language — progressively superseded by lattice-built rungs, and in the end-state
its output is checked rather than trusted.

## Open questions

- The certificate bridge: making the entailment engine emit kernel-checkable
  witnesses so it stops being self-trusting.
- Quantifiers and the logic surface (parse errors today) — the long pole.
- `Real`/continuous-math semantics.
- Sequencing the kernel relative to self-hosting (the kernel is the trust
  milestone; self-hosting is not — pin this ordering).
