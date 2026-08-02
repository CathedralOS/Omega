# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-08-02.

## 1. How are content-conservation theorems authored in contracts?

The n-ary law and its closed algebras are settled, and checked claim outcome
maps already identify which input claim feeds each result path. The design
briefs say that ordinary postconditions relate projections, but their
`content(result)` and `content(old(buffer))` examples are schematic. Core
declares neither operation, typed proof expressions have no distinguished
pre-state snapshot, and no source form identifies an authorized retirement as
the remainder of the same separated equation. Inferring equality from field
names, constructor shape, or the outcome map alone would silently authorize
content duplication.

Decide:

- the proof-only source expression that applies the owner-selected
  `Content<A>` projection to a qualified claim, including how an author selects
  one exact qualification when a carrier has multiple independent claims;
- the spelling and binding rules for pre-state content of consumed or mutated
  parameters, and whether snapshots apply to arbitrary values or only
  compiler-normalized proof projections;
- the source representation of partial separated composition, exact equality,
  and an authorized-retirement term in one n-to-m theorem;
- how result field/case/index paths and input paths in the checked outcome map
  bind to theorem subjects without relying on parameter order or presentation
  names;
- which unambiguous transformations the compiler may infer directly and which
  require an authored postcondition, especially direct constructors, one-to-one
  returns, splits, merges, and consuming failure outcomes; and
- how independently conserved algebras produce distinct witnesses while a
  joint correspondence algebra prevents an author from splitting related
  authority into unrelated equations.

Recommendation: add compiler-resolved proof intrinsics for exact-qualified
`content(value)` and its entry snapshot, plus one closed `separate(...)`
relation whose terms are output claims or route-authorized retirement. Require
an explicit qualification selector whenever projection choice is not unique.
Permit inference only when normalized input and output projections are
definitionally identical after the checked outcome-map substitution; require
an authored theorem for every other n-to-m transformation. Erase the intrinsics
after checking while retaining the normalized equation and its proof result in
checked/debug artifacts.

## 2. How does a domain authorize an admitted inbound parameter?

The exact-route rule is settled for results: a domain names one owner-approved
checked or boundary requirement, and only that requirement's qualified result
may establish membership. Interrupt acknowledgement `Pending` evidence has the
opposite direction. Hardware enters a selected `Calling<C>` requirement with a
linear acknowledgement parameter. The selected schema retains an exact strict
`accepts` row, and the installed-root invocation receipt binds that row to the
concrete acknowledgement subject, but the domain route model has no parameter
position. Leaving `Pending` empty permits vacuous explicit qualification;
allowing every boundary parameter to originate it would discard domain-owner
authorization. Core also cannot enumerate target-owned timer/interrupt traits.

Decide:

- the source form by which a domain authorizes an exact inbound parameter
  position rather than a result;
- whether target roots must inherit one core-owned entry requirement so the
  domain can name a stable requirement symbol without a reverse dependency;
- how overload identity and parameter position survive trait inheritance,
  selected provider schema normalization, and `Calling<C>` specialization;
- how checked direct calls retain propagated caller evidence while only an
  installed external-root entry may consume the invocation receipt;
- which checked/backend fact represents the per-invocation receipt requirement
  without pretending a static provider-plan fingerprint is the dynamic receipt;
  and
- how explicit qualification, look-alike requirements, uninstalled entry
  paths, parameter substitution, and receipt replay reject.

Recommendation: add an exact boundary-requirement-parameter establishment
route keyed by stable requirement symbol and non-self parameter index. Require
interrupt roots to inherit a core-owned acknowledgement-entry requirement so
the domain owner authorizes one stable symbol; let `Calling<C>` and the target
root refine its plan without replacing that identity. The checked external
entry specialization may then assume `Pending` only when backend lowering
consumes the matching installed-root invocation qualification. Ordinary direct
calls continue to propagate evidence supplied by their caller and never mint
it.
