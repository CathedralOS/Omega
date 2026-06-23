# Rung: Delta — evidence

[Lattice overview](../bootstrap_lattice.md) | Prev: [Gamma](gamma.md) | Next: [Epsilon](epsilon.md)

> **Status: DIRECTION.** Does not exist yet. This is the rung where the whole
> trust-by-checking architecture earns its keep: proof objects first appear here.

Delta introduces a small logical calculus and the **certificate checker** that
validates evidence. It is the second hand-audited trust anchor in the system —
the analogue of Lean's kernel.

## Adds

A small set of inference rules, e.g.:

```text
If A is true, and A implies B, then B is true.
```

…and a checker `check : Proposition × Certificate -> Accept | Reject`.

A separate **proof-search engine** may be arbitrarily sophisticated (and
LLM-driven), but it has **no authority** — it only *generates* certificates for
the small delta checker to validate. A buggy or hostile search engine cannot get
a false proposition past the checker.

## Written in

The **certificate checker is a Gamma program** — small, audited, frozen. The
proof-search engine is unconstrained and untrusted.

## Meaning

The delta checker *is* the definition of "a valid proof": a certificate is valid
iff the checker accepts it. The checker's correctness is therefore established by
**audit** (it is small) and by lattice diamonds — not by proof (that would be
circular), exactly as a proof-kernel's soundness is audited, not derived.

## The bootstrap (reference) route

To use delta foundationally, run the checker through the slow downward route, not
a delta/omega-compiled fast path:

```text
Delta checker (Gamma)
  interpreted by the Gamma interpreter (Beta)
  interpreted by the Beta interpreter (Alpha)
  executed by the native Alpha VM
```

Catastrophically slow, and that does not matter — it is the reference route.
Later, produce a fast native delta checker and reconcile it against the slow
reference (by proof that the fast one refines the slow one, or by per-run
double-execution). **This reconciliation is an honest open cost** — "just certify
fast against slow" hides that you need either a refinement theorem or per-artifact
double-execution.

## The hard part (honest edge #1)

A proof has to be *about* something. The delta checker validates that a
certificate proves a proposition — but connecting "the proposition" to "what the
program actually does per the [Gamma](gamma.md) reference interpreter" is a
**soundness theorem** at the gamma/delta seam (`provable ⟹ true-about-execution`).
That bridge — not the checker itself — is the core of the proof ambition. Without
it, "Omega proves things" rests on "the propositions happen to mean what we
intended."

## Must not contain

No automation authority (search engines are untrusted producers). No systems
features (mutable memory, ownership, effects — those are [Epsilon](epsilon.md)).
The checker stays as small as it can possibly be.

## Open questions

- The term language and inference rule set: how small can the checker be while
  expressing the math we want?
- The certificate format shared with producers (also proof-engine-north-star open
  question #3).
- The soundness bridge to the reference interpreter's operational semantics.
- Reconciling the fast native checker against the slow reference route.
- Relationship to the existing `omega-rs` contract-entailment engine, which today
  *is* the trusted base ("the engine IS the trusted base") and emits no
  certificate — delta is where it stops being self-trusting.
