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

## Q3 — May authored code invoke the reserved `T::drop` machine?

Chapter 17 defines `T::drop(&mut self)` as the ordinary reserved machine shape
selected by compiler-planned automatic cleanup. It also says the body receives
one whole valid value and returns it valid before structural field cleanup.
Neither the chapter nor the checker currently says whether source code may call
that machine directly. Today `value.drop();` checks as an ordinary mutable-
receiver call, leaves `value` live, and may therefore be followed by the
compiler invoking the same `drop` again on the return edge. The package
selection ledger has an unused `ExplicitCleanupCall` kind, but classification
cannot repair the undefined ownership event.

Choose one rule:

- whether reserved `drop` is compiler-only or source-callable;
- if source-callable, whether the call consumes the whole place, how that
  follows from a declaration whose receiver is `&mut self`, and how the
  frontier suppresses later automatic cleanup;
- whether early cleanup may target fields or only a whole valid root;
- which automatic-cleanup preconditions and control restrictions apply; and
- whether this is a dedicated authored cleanup selection or an ordinary call.

Recommended direction: make reserved `T::drop` compiler-only. Reject authored
calls to it during checking. Early protocol completion or abandonment remains
an ordinary explicitly named consuming machine such as `close`, `finish`, or
`abandon`; that machine consumes ownership according to its ordinary signature
and is already captured by the authored call ledger. Automatic `drop` remains
a carried semantic dependency and grants no source authority. Under this rule,
remove `ExplicitCleanupCall` from the authored-selection vocabulary rather than
pretending an unsupported operation exists.

An acceptable alternative is to define authored `drop` as a special whole-place
consuming operation. It must consume exactly once, establish every cleanup
premise at that site, suppress edge cleanup for the consumed root, and receive
explicit source and Psi semantics despite the declaration's mutable receiver.

Tempting but wrong alternatives are to keep treating `value.drop()` as an
ordinary `&mut` call, infer consumption from the spelling only after checking,
allow both authored and automatic invocation on the same live place, or record
an `ExplicitCleanupCall` package row without first defining its ownership
semantics.
