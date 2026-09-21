# INDUCTIVE-CARRIER-CERTIFICATE — verify + record (z142)

Bare stub — the name appears on TASKS.md only as a citation inside
PROOF-INTERCHANGE-IMPORT's verified row (:6720, "the carrier certificate
lives in the proof-admission crate"). Mines the internal induction
certificate: one inductive carrier's finite base/step/decrease
certificate, per `wiki/drafts/matching_logic.md:45`.

## Verified at `e7c0099cb2` (linux x86-64, cargo nextest)

**Landed.** `proof-admission/src/admission/recursion.rs` owns
`verify_recursive_component[_with_machine_parameters]` — the internal
carrier certificate:

- `RecursiveComponentObligation` (members, optional ranking relation,
  well-foundedness certificate obligation, per-edge decrease
  obligations) is owned by artifact/source reconstruction — a proof
  bundle cannot add members, remove edges, or select another ranking.
- Check: shape validation → ranking-relation match
  (`RankingRelationMismatch`/`MissingRankingRelation`) →
  well-foundedness evidence verified through the admission kernel →
  canonical ordered certificate edges (`NonCanonicalCertificateEdges`)
  → every obligation edge's decrease verified
  (`MissingDecreaseEvidence`/`Decrease`) → no stray evidence
  (`UnknownDecreaseEvidence`).
- `verify_recursive_component_with_machine_parameters` keeps
  reconstruction scalar parameters exact — loop-local values never
  acquire parameter-only proof rules.

Landing chain: `883097ab1bf11` (grouped proof recursion certificates),
`922b065b4b80b` (slice-ranked graphs retain component comparison
evidence), `6cd98ce220b3b` (terminal verifier replays a Natural
certificate, rejects a wrong arrival), `d6bdae1b7c908` (value-returning
ranked machine lowers with its Natural certificate).

End-to-end witness: `terminal-codec/tests/theorem_certificate.rs` —
`indexCorrect` (the indexed family's index-soundness theorem, proved by
the derived `iindW` eliminator) transports through decode and re-verify
with an exact `certificate_assumption_closure`.

## Fresh witness at `e7c0099cb2`

- `nextest run -p proof-admission --lib`: **295/295 PASS**, incl.
  `recursive_component_preserves_exact_machine_parameter_scope` and the
  `recursion.rs` unit suite.
- `nextest run -p terminal-codec theorem_certificate`: **5/5 PASS**
  (wire decode + re-verify, assumption transport, polymorphic arity).

## Live fences

- `PROOF-KERNEL-CORE` (zergling-177, exp ~09:49Z) holds the whole
  `omega-rust/psi/semantics/proof-admission` tree — any extension of the
  certificate surface belongs to that lane.
- The *external* matching-logic certificate leg (the board-cited
  residual) has no importer — gated on MATCHING-LOGIC-BOUNDED-SLICE's
  bounded comparison + an independently checked translation, per
  MATCHING-LOGIC-EXTERNAL-PROOF-IMPORT's verified row. Not this item's
  scope.

## Verdict

**Resolved / record-only.** The internal inductive-carrier certificate
is landed and green at HEAD; no unclaimed slice exists under this name.
