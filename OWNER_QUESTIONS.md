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

Last pruned: 2026-08-25.

## Q1 — What semantic subject does artifact proof establish?

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

## Q2 — May authored code invoke the reserved `T::drop` machine?

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

## Q3 — How does composition select and partition component artifacts?

The semantic unit is settled: a component is one selected provider realization
plus the closed code, state, resource, registration, continuation, and version
graph that realization owns. A package is only a source, naming, visibility,
and dependency-reach unit; `pub` does not export a component ABI, and a package
is not automatically one component. Hot-swap points may nest within a package,
while a part needing a different source dependency/reach envelope is a distinct
package. Package-shaped component closure is therefore only a valid first
implementation fence.

What remains undecided is the composition authority and exact partition
surface. Choose one model that specifies:

- how a consuming build or final composer selects an exact provider
  realization for independent emission rather than static fusion;
- how one package may contribute zero, one, or several component roots;
- which satisfied requirement identities become component exports and which
  requirements remain external imports;
- how the compiler closes concrete implementation calls, private helpers,
  constants, mutable state, registrations, continuations, authority, and
  resources beneath each selected root;
- what happens when two proposed closures overlap, distinguishing shareable
  immutable dependencies from state or custody that cannot belong to two
  replaceable eras;
- what stable symbolic identity names the component slot independently of one
  candidate artifact or era; and
- which replacement, coexistence, resource, and admission policies are chosen
  by composition rather than asserted by the provider package.

Recommended direction: keep ordinary source limited to declaring requirements
and realizations. The consuming build/composer names an exact requirement slot
and selected realization application and chooses `fused` or `independent`;
the compiler derives and validates the closure. An independent component
exports only requirement identities selected by composition. Concrete-identity
edges either remain inside the derived closure or reject; they never become an
implicit component ABI. A provider package may publish candidates and their
contracts, but cannot force every deployment to componentize itself or bless
its own admission and replacement policy.

The first implementation may require each independent closure to coincide
with one package and reject overlap. That restriction must be reported as an
implementation fence, preserve the general identities above, and later relax
to multiple roots within one package without changing already accepted source.

Tempting but wrong alternatives are a `component` source block that bakes one
deployment policy into reusable library code, treating every package or every
`pub` declaration as a component/export, allowing a concrete machine identity
to cross a replaceable boundary, discovering roots from reachability without
an authored composition selection, naming slots with strings or ordinals, or
letting two eras silently claim the same mutable state or linear custody.
