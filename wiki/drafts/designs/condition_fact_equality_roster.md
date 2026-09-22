# Condition-fact equality roster — projection design

Status: design for the open asymptotic centre of the C2L conjunct-lowering
cliff. The measurement record is `wiki/drafts/measurements/c2l_conjunct_lowering_cliff.md`;
the owning row's acceptance is unchanged — the unreduced fixture terminates
with a verdict under an ordinary test timeout with **no obligation abandoned**.
Delete this design once the projection lands and the cliff record's hotspot 2
is closed.

## What is being rebuilt today

`terminal-verifier/src/verification/reconstruction/path_facts/conditions.rs`
(`transport_certified`, currently lines 71-84) re-decides a fixed-shape
`ValueEqualityTransport` certificate for every reconstructed condition fact,
twice per conditional (positive and negative arms). Each call:

1. Scans the whole axiom roster newest-first and clones every
   `Equal(Value, _)` axiom into a `Vec<ProofNode>` — O(roster) construction.
2. Emits those nodes as certificate children — `Action::Children(equalities)`
   checks each `SemanticAxiom{index}` node — O(roster) acceptances.
3. `check_value_equality_denotation` re-validates every supplied equation
   (`budget.proposition` + `context.validate`) and rebuilds `ValueEqualities`
   by linear first-wins scan — O(roster) again, per arm.

A chain of N short-circuiting `&&`s has O(N) conditionals over an O(N)
roster: O(N^2) kernel work and the ~20,000 certificate acceptances the cliff
record attributes to this site.

## The invariant that makes projection safe

Inside `predicate_denotation/value_equalities.rs`:

- `ValueEqualities` consults an equation iff a `ScalarTerm::Value { id }`
  leaf whose `id` heads that equation occurs in the term being transported,
  or occurs inside the right-hand side of a definition reached transitively.
  `scalar()` is a pure lookup over `definitions`; all other constructors just
  recurse.
- Definitions are first-wins per `ValueId`: later equations for the same id
  never enter `definitions` and cannot fire.

Therefore the transport result for premise `P` and conclusion `C` depends
only on equations whose `ValueId` lies in the **closure**: the Value leaves of
`P` and `C`, expanded through first-wins definitions until fixpoint. Every
other roster equation is dead weight — it is validated, cited, and traversed
but can never be looked up. Supplying only the closure leaves both
denotations bit-identical, so the `original == transported` verdict is
unchanged and the certificate's acceptance set drops to the closure.

Two consequences the implementation must honor:

- `SemanticAxiom{index}` cites positions in the ambient axiom roster, so the
  projection must retain original indices — the certificate children are a
  sparse subset of the roster, not a renumbered copy. `check_certificate`
  receives the full `axioms` unchanged; only the `equalities` vec shrinks.
- `check_value_equality_denotation` rejects an empty equation list
  (`InvalidValueEquality`), and `normalize()` runs regardless of equations —
  so an empty closure is not a skip. Rule: when the closure is empty, retain
  the roster's newest equality as a sentinel. The sentinel can never fire
  (its head id is not in the closure, so not in either term), the checker
  still sees a real cited equation, and the verdict matches the unprojected
  run exactly.

Dropped-equation validation has no observable side effect: roster axioms are
emitted by reconstruction itself under the same `PropositionContext`, so a
malformed entry cannot exist; skipping a check that would have succeeded is
neutral, and nothing else consumes the validated form.

## Design

Per-path **incremental equality index**, built once and shared by every
condition fact on that path:

```
struct EqualityRosterIndex {
    // Roster order retained for citation; parallel to the axiom roster.
    newest_first: Vec<(ValueId, /* axiom index */ usize)>,
    // ValueId -> position in newest_first of its first-wins equation.
    first_win: HashMap<ValueId, usize>,
}
```

Axioms on a selected successor's roster are append-only along the walk, so
the index is maintained incrementally: appending an `Equal(Value{id}, rhs)`
with `left != right` pushes the entry and inserts `id` only if absent —
O(1) per new axiom, no per-fact scans. Where the walk clones an existing
roster prefix, clone the index with it (the `Vec` is already needed; the
map is small relative to a per-fact rebuild).

Per condition fact:

1. Seed the closure with every `Value` leaf in the premise
   `condition == Boolean(positive)` and in the emitted proposition.
2. Expand: for each id in the closure with a `first_win` entry, add the
   `Value` leaves of that equation's right-hand side; repeat to fixpoint —
   O(closure) via the map, not O(roster).
3. Emit `ProofNode`s for exactly the closure ids' first-wins equations —
   `SemanticAxiom` carrying the retained original roster index, in
   newest-first roster order (preserves the transport's documented
   first-match discipline). Plus the sentinel rule above when the closure
   is empty.
4. Pass the unchanged full `axioms` to `check_certificate` as today.

The projected roster replaces the second per-arm rebuild too: positive and
negative arms differ only in the premise's polarity, not the roster, so one
closure computation feeds both `transport_certified` calls — the two
certificates differ but share the equality subset.

New work per fact is O(closure); typical `&&`-chain facts walk a short
definition spine, so closure ≈ spine depth and the stage's certificate count
collapses from O(N^2) toward O(N·spine).

## What this does not touch

- `infer_type` recursion depth inside individual certificates (cliff record
  centre 3) — orthogonal; a projected roster does not shorten any single
  certificate's term.
- The emitted proposition (`condition_proposition`) is already computed
  before `transport_certified`; this design changes only which equations
  back the certificate, never which fact is emitted or which arm selects it.
- No bound on the search: 192/192 obligations still prove; nothing here
  abandons or weakens a certificate. The change is a sparse projection of
  the same cited evidence, not a new judgment.
- `certified` stays the test-visible classification. Failures still fall to
  the `fact:branch-condition` licensed introductions; a projection-induced
  verdict change would itself be a bug per the invariant above.

## Verification

- Replay: on the cliff fixture and a canary sweep, the `certified` flag and
  every emitted proposition must be identical before/after — the projection
  is observably semantics-preserving by construction, and the corpus
  exercises it (the flag exists precisely so tests read it back).
- Cost witness: the 73-conjunct terminating reduction (45s at the record)
  should collapse its certificate-acceptance count roughly by the
  roster/closure ratio; measure acceptances before/after, not just seconds.
- Roster-growth edge cases in `conditions/tests.rs`: shadowed same-id
  equations (first-wins), a closure that is empty (sentinel), and a cycle of
  forward definitions (the existing `expanded.contains` rejection is
  untouched).

## Anchor re-verification

Re-checked against the live code at `f9efadbb493e` (linux x86-64 working
tree); every load-bearing anchor holds:

- `conditions.rs` still clones the whole `Equal(Value, _)` roster into both
  per-arm certificates (`transport_certified`, the
  `axioms.iter().enumerate().rev().filter_map` block at ~:71-84), and the
  `check_certificate` call passes the full ambient `axioms` slice unchanged —
  the sparse-projection contract (retain original `SemanticAxiom` indices,
  shrink only the `equalities` vec) is what the call shape requires.
- `predicate_denotation.rs:189-190` rejects an empty equation list with
  `InvalidValueEquality` — the sentinel rule is mandatory, not optional.
- `value_equalities.rs:28` keeps first-wins per `ValueId`
  (`!definitions.iter().any(...)` gate), and `scalar()` (:82-100) is a pure
  head-id lookup recursing through definitions — so the transitive closure
  over first-wins right-hand sides is exactly the consulted equation set;
  `active.contains` → `CyclicValueEquality` is the untouched cycle
  rejection, and `condition_proposition`'s own `expanded.contains` guards
  sit at conditions.rs :112-119 and :228-233.
- `budget.proposition` charges per validated equation inside the same
  per-certificate budget (`check_value_equality_denotation` :178-184), so
  dropping dead equations can only shrink consumption — no verdict can flip
  from Ok to Err.
