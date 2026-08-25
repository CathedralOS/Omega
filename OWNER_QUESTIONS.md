# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Last pruned: 2026-08-24.

## Q1 — What visibility do selectable declaration kinds without `pub` have?

Package admission gates every authored declaration selection to the requesting
package or one direct dependency and separately records whether that selection
enters a public package surface. Data, domains, traits, machines, and wire
schemas carry ordinary `pub`, but the parser currently rejects `pub` on several
independently selectable roots, including operators, propositions, invariants,
measures, and constants. The compiler therefore cannot infer from syntax alone
whether another package may name one of those declarations or whether selecting
it from a public signature publishes a semantic dependency.

Choose one coherent visibility rule for those declaration families. It must
specify:

- which declarations may be named directly across a package boundary;
- whether a declaration nested in or semantically owned by a public data,
  domain, trait, machine, or conformance inherits that owner's visibility;
- whether standalone declarations gain ordinary `pub` syntax or remain
  package-private;
- how direct operators, proposition applications, constant references, and
  measures participate in public package-contract identity; and
- how compiler/toolchain intrinsics remain available without making a
  package-authored same-name declaration public.

Recommended direction: use ordinary `pub` for every independently nameable
standalone declaration and inherit visibility only for declarations that have
one exact semantic owner. Trait requirements and conformance rows follow their
already-defined owner visibility; a standalone operator, proposition, measure,
invariant, or constant is package-private unless explicitly `pub`. This keeps
one source-level visibility rule and gives admission evidence an exact lexical
answer.

A narrower acceptable alternative is to keep selected families permanently
package-private and require every cross-package use to pass through a public
owner's typed surface. That is coherent only if the language forbids direct
foreign naming and defines exact owner inheritance for every such family.

Tempting but wrong alternatives are to treat every declaration lacking a
visibility bit as public, infer visibility from whether another package happens
to select it, use reachability as a substitute for source visibility, or let
the package projector guess public exposure from display names after checking.

## Q2 — What semantic subject does artifact proof establish?

The proof kernel checks a finite derivation of `P` from explicit premises, and
the artifact verifier reconstructs the exact obligation from canonical source
and artifact subjects. The remaining soundness bridge must state what it means
for that accepted proposition to be true.

Choose and relate the semantic subjects used by authoritative verification:

- global consequence over every model satisfying a declarative Omega theory;
- consequence in an initial or otherwise intended model; or
- a judgment in one pinned canonical operational transition system.

Different obligation classes may use different subjects only if their join is
explicit and proved. Do not infer that a global completeness theorem applies to
an initial-model or canonical-execution claim, and do not add no-junk,
fixpoint, or model-selection axioms merely to recover that theorem.

This decision gates the bounded matching-logic investigation in
[`wiki/design_briefs/matching_logic_proof_research.md`](wiki/design_briefs/matching_logic_proof_research.md).
The investigation may produce an untrusted proof producer, an independent
semantic diamond, or a proof-import lane; it does not replace the current
kernel by default.
