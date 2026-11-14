# Static binder contract dependencies

Ratified 2026-11-07. A helper that takes a callback should publish "whatever
the callback does, plus my own part" on every contract axis, not only on reach,
suspension and blocking. Today a wrapper that admits a trapping callback must
publish a flat `crashes Trap` for every caller, and a wrapper cannot say "I
terminate when my callback does". Both are the same missing row of one
mechanism: check the generic once, keep a term per axis with the callback
binders as variables, publish the terms, and substitute each application's
selected published contracts.

The normative owners are the
[per-axis table](../../spec/language/effects.md#static-binder-contract-dependencies),
[guarded crashes](../../spec/language/effects.md#guarded-crashes),
[published termination guarantees](../../spec/language/termination.md#published-guarantees-and-private-witnesses)
and [requirement omission](../../spec/language/generics.md#requirements-and-conformance-evidence).
Track implementation under `STATIC-BINDER-CONTRACT-DEPENDENCIES` in
[TASKS.md](../../../TASKS.md). Delete this record when the implementation and
acceptance are complete.

## Decisions

- One mechanism. Every operational axis publishes a normalized term over exact
  binder projections plus the body's fixed contribution. Applications
  substitute selected published contracts. Nothing reads a selected callback's
  private body, observed behavior or the current consumer set.
- One omission rule. Omission derives the answer where derivation is safe.
  Suspension and blocking have call-site markers checked against a known bound,
  so published surfaces and requirements author them. Termination is a promise,
  so it stays authored. Reach and crashes infer on bodies and defer on
  requirements.
- Crash row (amended 2026-11-08). Omitted `crashes` on a public checked
  machine means `crashes none`; a private body infers; a non-boundary
  requirement defers to the selection. A trap needs no authority and no marker,
  so public surfaces author it like `suspends` and `blocks`, while reach stays
  inferred because a body cannot exceed held authority. Routes compose per
  cause and then use the existing path-fact disproof.
- Termination row. Authored `terminates` on a generic whose requirement
  publishes no guarantee carries a binder premise. A selection that publishes
  `terminates` discharges it; a silent selection stays admissible with no
  guarantee. Omission on the generic still publishes nothing.
- Spelling. Explicit empty values are `reaches none`, `crashes none`,
  `suspends false` and `blocks false`: clause then the empty literal of its
  type. The keyword with no value is malformed in the spec; both parsers still
  accept it until the retirement sweep of `tests/fixtures` and `samples`
  lands. The library, Build prelude, guide and spec were swept in the
  ratifying change.
- Identity. Terms, bounds and premises are part of contract identity. A body
  edit that changes one is an interface change even with an unchanged header.
  Compatibility target: existing admitted closed applications keep their
  guarantees; generic recipes and their published identities may change.
- Result relations (ratified 2026-11-08). A static binder carries a proof-only
  relation: the selected callback's published normal-return guarantee under the
  requirement. A checked normal return establishes it for the exact arguments
  and result. Generic checking treats it as opaque. Wrappers publish it only by
  an authored `ensures` claim, which the checker discharges or rejects; no
  derived relation is exported. Specialization substitutes the selected
  published relation. First scope: immutable arguments and normal returns.
  Spelling (ratified 2026-11-08): `F::ensures(a1, ..., an, r)`, parameters in
  declaration order and the result subject last, an ordinary proof-only
  proposition on the static binder. No unapplied value, partial application or
  `F::requires` is implied.

## Current code

- The parser accepts `reaches none` and `crashes none` beside the bare keyword
  (`omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/contracts_and_parameters.rs`);
  the Omega-written parser accepts both in `source/psi/parse/clause.omg`.
  `crashes none` currently publishes no contract, which equals omission on
  today's published surfaces; private-body and requirement
  enforcement arrive with the crash row.

## Implementation

1. Shared mechanism, built on suspension and blocking first. One term
   representation over exact binder identities; derive it during the single
   parametric body check; normalize OR/union associatively, commutatively and
   idempotently; publish it in the machine contract rows and Psi encoding;
   import it; forward through renamed binders; substitute at applications;
   converge recursive propagation; retain authored ceilings and residual
   admission obligations through publication and import; admit no selected
   private body fact into a public result.
2. Reach onto the same representation. Keep the deferred/explicit/finite
   distinctions and installation-bound rows unchanged.
3. Crash row. Public omission means none, as the compiler already enforces;
   infer private bodies; defer requirements; enforce `crashes none` on private
   bodies and requirements; compose routes per cause (a Boolean per
   cause is sufficient for the first cut, guard mapping through call-site
   actuals is the precision step); apply disproof after substitution.
4. Carry row on the shared suspension term, as already specified.
5. Termination row. Binder premises on authored `terminates` with the own
   progress proof; application discharge from selected published promises;
   the termination identity retains binder premises.
6. Retire the bare keyword in both parsers and sweep `tests/fixtures`,
   `samples`, `tests/psi/parser` and `tests/psi/resolver` to `reaches none`.
7. Result relations: retain the opaque per-invocation relation fact through
   generic checking, discharge authored claims from it with ordinary logical
   evidence, substitute the selected published relation at specialization, and
   publish the claim under the usual dependency identity. This extends the
   dependency-term machinery; it is not a separate proof system.

Milestone for step 1: a wrapper publishes an unresolved dependency; another
module imports that contract and forwards a differently named binder; quiet and
non-quiet concrete callbacks produce the correct instantiated contracts;
recursive propagation converges; authored ceilings and residual obligations
survive export and import; no selected private body fact enters the public
result.

## Fixtures

Combinators `forward<F>(x)`, `compose<F, G>(x)` and `choose<F, G>(c, x)` across
reach, suspends, blocks, crashes, terminates and carry, each with a positive and
a negative twin. Boundary cases:

- `F`'s requirement promises a positive result and `G` traps only on zero:
  existing composition discharges the route (regression, not new capability).
- Only the selected implementation has the stronger positivity guarantee: it
  does not enter a wrapper checked under the weaker requirement.
- A selected callback with a narrower published envelope specializes the
  application while ordinary call and resource obligations remain.
- A wrapper over a requirement that omits `crashes`, selected with a crash-free
  callback, publishes no route; with a trapping callback it publishes `Trap`.
- A wrapper authoring `terminates` over a silent requirement is admissible with
  a non-terminating selection and receives no guarantee; with a promising
  selection it receives one.

Result-relation fixtures, immutable arguments:

- Direct forwarding preserves the projected relation for the actual returned
  value.
- A selected callback with a stronger published guarantee than its requirement
  specializes the wrapper's authored claim to that guarantee.
- Two calls with a non-transitive relation compose through the intermediate
  value instead of collapsing to one application. The fixture must show the
  authored claim as real source, written with the existing witness machinery
  or a named mathematical definition packaging it, since Omega has no
  existential syntax; if that claim is unpleasant to write, the ergonomic gap
  is in the mathematical surface and is reported as its own item.
- Two calls where the intermediate may violate the second call's input
  requirement reject unless the wrapper establishes closure.
- A discarded callback result and an unrelated wrapper result: the relation
  does not attach to the constant; the completed call's existential consequence
  remains available.
- Forwarding through another generic wrapper preserves the relation across
  layers.
- A callback that may crash or diverge: the relation is evidence on the
  normal-return path only and implies no progress.

Transformation pairs, stating what stays invariant: adding a transparent
forwarding wrapper keeps the relevant guarantees; extracting a private helper
keeps the delegated frame and invariant debt; replacing a callback with one
satisfying a stronger published contract keeps composition sound and may use the
stronger guarantee; changing a branch from unreachable to reachable makes its
contributions relevant; replacing a terminating path with divergence fails a
required guarantee.

## Completion

An author predicts what a wrapper publishes from its declared requirements and
the permitted published dependencies, without reading selected bodies or knowing
the consumer set. A dependency survives publication, import, binder forwarding
and final substitution. The three earlier decisions become rows with no change
in meaning except crash omission.
