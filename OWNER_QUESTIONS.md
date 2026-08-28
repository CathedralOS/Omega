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

## Q1 — Physical ABI for opaque by-value boundary data

### Context

`InterruptAcknowledgement` and `InterruptMaskGuard` are now public opaque
linear boundary data. Their provider-owned settlement fields are correctly no
longer source-visible. Both can nevertheless cross a boundary by value; for
example, `InterruptEntry::enter` receives an `InterruptAcknowledgement` and is
governed by a source-authored `Calling<C>` policy.

### Problem statement

Calling-policy evaluation needs a target-specific byte size and alignment
before it can validate a by-value placement. Opaque boundary data deliberately
has no ordinary Omega layout, and package review currently records its ABI and
mechanism as `Unbound`. The compiler therefore rejects the interrupt entry as
zero-sized. Restoring public structural fields, treating the value as a ZST, or
hardcoding its former five-`u64` shape would each contradict the opacity and
representation-TCB decisions.

### Proposed direction

Keep the source type opaque, but require the selected provider/installation to
supply a compiler-validated, target-specific representation descriptor before
evaluating any `Calling<C>` policy that passes the value by value. The policy
may inspect only the closed shape descriptor, never provider fields. Review and
eventual admission should replace `Unbound` with the exact ABI and mechanism
commitments and reject when no unique descriptor is selected.

### Alternates

- Acceptable if it matches the intended machine contract: make opaque
  obligations cross this boundary through an explicit reference/handle shape,
  so no by-value representation is promised.
- Tempting but wrong: restore public identity fields merely to recover layout.
- Tempting but wrong: assign a compiler-global magic size or accept zero-sized
  placement without selected representation evidence.

## Q2 — Package selector for a multi-package source

### Context

A fetched Git repository may have a package at its root or a workspace root
whose member paths lead to several packages. Member paths are deliberately not
stable package names. The selected member's own `builder.package("name")`
declaration remains authoritative identity evidence.

### Problem statement

`Source::Git` currently carries only repository and revision. That is
unambiguous for a repository-root package but cannot select one package from a
workspace. The lock cannot be the only selector because a fresh lockless
resolution must be reproducible, and an import alias cannot select because
aliases are local and may be explicitly renamed.

### Proposed direction

Keep the existing `Source::Git` case for an unambiguous repository-root package;
the resolver reads that package's own declaration, so the caller does not
repeat its name. Add ordinary `Source::GitPackage { repository, revision,
package }` data—not grammar—for selecting a package from a workspace Git
source. Treat `package` only as selection intent:
after authenticating the repository root, project its declared member paths and
require exactly one member's own package declaration to match. That fetched
declaration—not the request string—establishes the name joined into
`PackageKey`. Using root-package `Source::Git` on a workspace rejects as
ambiguous.

### Alternates

- Acceptable: add an optional selector to the existing Git source data if Omega
  construction remains concise and omission is permitted only for a
  repository-root package. A separate source case is clearer with the current
  explicit data model.
- Tempting but wrong: require a package-name field for every Git source. A
  repository-root package is already unambiguous, so this makes every ordinary
  package declare the same name twice.
- Tempting but wrong: select by member directory path; repository relocation
  would become package replacement and callers would duplicate workspace
  layout.
- Tempting but wrong: infer selection from the default alias or defer it to
  `omega.lock`; explicit aliases and first resolution make both ambiguous.

## Q3 — Application identity in the package graph

### Context

Applications now declare `builder.application("name")`, may own dependencies,
and form the root of a reconciled package closure. Compiler package handoff
currently identifies graph roots through `PackageKeyIdentity`.

### Problem statement

Giving applications no source-qualified graph identity requires a second root
identity system and weakens provenance across application updates. Treating an
application as an ordinary dependency, however, would erase the role
distinction and permit consumers to import an artifact root as a library.

### Proposed direction

Give an application the same name-plus-source-lineage `PackageKey` used for a
stable reach-unit identity, while retaining `Application` as its role. It may
own dependencies and produce artifacts but cannot satisfy another project's
package dependency. Exact source and artifact evidence remain instance facts.

### Alternates

- Acceptable if a concrete compiler constraint requires it: define a distinct
  source-qualified application-root key with the same lineage and instance
  commitments, then prove the graph handoff cannot confuse it with packages.
- Tempting but wrong: key an application by its authored name alone.
- Tempting but wrong: make applications importable packages merely to reuse
  existing graph code.

## Q4 — Scoped build machines as project manifests

### Context

Package identity and dependency projection recognize one canonical free
`machine build(builder: &mut Build)` in `build.omg`. That entry declares the
project role and owns the authoritative dependency projection. Standalone
compiler loading still recognizes both free `build` and scoped
`Owner::build` machines in `build.omg` as privileged build roots. Two positive
provider canaries and three deliberately failing build-authority canaries use
the scoped form.

The rest of the repository is now closed: all 1,338 tracked free build roots
declare an explicit role and both package orchestration and compiler loading
enforce the shared grammar. A corpus canary proves the five exceptions each
remain exactly one scoped root with no competing free root. The unresolved
behavior is therefore isolated to the early scoped-root bypass in compiler
role validation and scoped-name acceptance during build-machine selection;
there is no broader migration dependency hiding behind this question.

### Problem statement

One `build.omg` currently has two incompatible meanings. Package-aware readers
reject scoped build machines because they cannot establish the single canonical
project role/dependency root, while standalone compilation executes them with
build authority. Enforcing roles globally would either reject an intended
composition surface or preserve a second project-manifest model. It would also
mask the authority diagnostics pinned by the malformed scoped canaries unless
their intended status is decided first.

### Proposed direction

Retire scoped machines as project build roots. Require exactly one free build
entry to declare the application, package, or workspace role and own dependency
projection. Component-specific provider configuration remains ordinary Omega
composition selected or called from that root rather than acquiring a second
manifest identity. Migrate the positive scoped canaries to the free entry and
recast the failing canaries so they continue testing their authority violation
under the canonical root.

### Alternates

- Acceptable if scoped ownership is semantically important: formally admit
  exactly one scoped root and specify how it declares project role, owns
  dependencies, receives the `Build` activation, and excludes any competing
  free root. Both compiler and package readers must then share that rule.
- Tempting but wrong: keep standalone acceptance and package-reader rejection;
  the same file would continue to mean different things by caller.
- Tempting but wrong: infer project role from the scoped owner name.
- Tempting but wrong: add a no-op free manifest beside the privileged scoped
  build; that restores duplicate build roots rather than one authoritative
  entry.

## Q5 — Fixed-array element cleanup order

### Context

Literal-length fixed arrays expose one canonical ownership path per element.
Moving one literal-indexed element leaves every unselected sibling obligation
live, and the cleanup plan must later dispose each remaining cleanup-bearing
element exactly once. Records already clean structural fields in recursive
reverse declaration order, but array elements are not declarations and the
language guide assigns them no cleanup order.

### Problem statement

General fixed-array cleanup, including partial arrays with more than one live
element, needs one deterministic semantic order before checked cleanup plans,
fuel, proof traces, and native artifacts can agree. Choosing increasing or
decreasing index order in the compiler would silently add language semantics.
The bounded two-element slice with exactly one moved element and one residual,
and the bounded three-element slice with exactly two moved elements and one
residual, do not expose this choice. Wider-array partitions with multiple live
residuals remain blocked on it.

### Proposed direction

Define literal array construction in increasing index order and structural
cleanup in the reverse order of the live constructed elements: decreasing
index, skipping moved elements. This matches record cleanup's reverse-source
principle, makes partial cleanup a filtered suffix/order rather than a new
schedule, and gives interpretation, fuel, and artifact replay one canonical
sequence.

### Alternates

- Acceptable if iteration semantics should dominate: clean in increasing index
  order, but state why arrays intentionally differ from reverse record-field
  cleanup and pin construction-failure behavior to the same choice.
- Acceptable if element order must be type-directed: require an explicit
  collection-owned cleanup policy, but ordinary fixed arrays then need a
  canonical default before they can contain cleanup-bearing elements.
- Tempting but wrong: let each backend choose an order or treat order as
  unobservable. Cleanup calls can carry effects, requirements, guarantees,
  fuel, and diagnostics, so their sequence is semantic.

## Q6 — Lifetime application on conformance target traits

### Context

A public conformance may have its own lifetime telescope, and package review now
retains lifetime-sensitive type arguments through inherited trait requirements.
The remaining unsupported form is a conformance whose selected trait itself is
lifetime-parameterized. Current conformance representations retain the target
trait's type arguments but no target-trait lifetime application.

### Problem statement

The compiler cannot distinguish, validate, or canonically record which
conformance lifetime supplies each target-trait lifetime. Accepting the form
would erase public interface identity; reconstructing it from names or expected
subject shape would make inference and package review disagree.

### Proposed direction

Treat the target as an ordinary complete trait application. Retain each lifetime
argument in target-trait declaration order and resolve it to an ordinal in the
conformance lifetime telescope. Require explicit arguments unless the existing
language-wide lifetime-elision rule yields exactly one result, and retain that
resolved result identically through typed checking, conformance closure, and
package review.

### Alternates

- Acceptable and simpler: require every target-trait lifetime argument to be
  explicit at a conformance declaration, even where callable/type lifetime
  elision would otherwise be unique.
- Tempting but wrong: infer target lifetimes from the conformance subject or
  trait type arguments without retaining the resolved application.
- Tempting but wrong: erase the target-trait lifetime application because it
  has no runtime layout effect; it remains proof and public-interface identity.

## Q7 — Authority-bearing roles for ordinary packages

### Context

`omega::language::std` is settled as an ordinary optional package rather than a
compiler-mounted namespace. Its filesystem, console, target, and platform
provider declarations nevertheless have compiler-recognized semantic roles:
they drive sandboxed build services, dangerous-authority classification, and
target/provider validation. `PackageCompilationInputs` currently carries exact
package identities, roots, and dependency edges, but no authenticated role
binding.

### Problem statement

Once std and platform providers arrive through the ordinary graph, the compiler
must know which exact package is allowed to supply each compiler-recognized
role. Inferring that authority from a declared package name, requester alias,
source path, repository location, or same-spelled trait/service would let a
lookalike package acquire authority. Initial review also needs to classify and
sandbox a candidate before that candidate has accepted lock evidence, without
pretending candidate designation is admission.

The post-relocation `generated-table -> generated-consumer` canary now exposes
the same boundary from build execution: dependency generated-source custody is
ready, but the producer cannot receive `FilesystemHost` until its exact
ordinary-package role is authenticated. A package name, trait spelling, or old
physical std path must not make that test pass.

### Proposed direction

Have package orchestration pass an explicit, exact role-binding set into the
compiler. Each binding names a closed compiler-owned role and one reachable
`PackageKeyIdentity`; roles that require a particular interface additionally
rejoin exact declaration/schema coordinates after checking. Candidate review
and accepted compilation use distinct provenance: a candidate binding permits
classification and confined evaluation and becomes review evidence, while an
accepted binding must come from the consumer's accepted graph policy. The
compiler validates reachability and exact semantic declarations and passes
their resolved symbols downstream; Psi and package review never rediscover the
role from names.

### Alternates

- Acceptable but less flexible: weld one exact std package lineage into each
  compiler release while still resolving its revisions as ordinary graph
  nodes. This makes alternate standard-service implementations a compiler
  configuration change.
- Acceptable for platform providers: let the root application's accepted build
  policy bind target/provider roles explicitly, provided candidate review keeps
  that choice non-authoritative until admission.
- Tempting but wrong: reserve `omega-language-std`, `omega_language_std`, or a
  source-directory location as authority.
- Tempting but wrong: classify any package that implements a same-spelled
  `FilesystemHost`, `Console`, target, or provider trait.
- Tempting but wrong: keep std toolchain-owned internally while presenting it
  as an ordinary package at the import surface; that preserves two identities
  for one dependency and defeats capability review.

## Q8 — Requested versus source-selected build targets

### Context

The build/package design gives durable `Build` one selected `TargetProfile` and
shows ordinary source assigning `builder.target`. The migration/reference
compiler still discovers target availability through transitional
`target name {}` items and receives the selected target from its invocation.
The Omega-written compiler's `build.omg` consequently declares four selectable
targets while binding one entry root for each. Its checkpoint build prelude has
no `TargetProfile`, target field, or target-selection operation yet.

### Problem statement

Replacing those four declarations with one ordinary assignment is not a
mechanical syntax migration. A singular source-chosen target would collapse the
compiler package's cross-target availability, while `Target::Host` or an
implicitly preinitialized field would make ambient invocation state select
semantic build output without a specified Omega value or retained authored
acceptance. Conversely, treating `Build.target` as a set would contradict the
settled singular durable projection. The language does not yet say how an
external requested target enters the build machine, how source accepts or
rejects it, or which fact establishes the one normalized selected target.

### Proposed direction

Separate invocation request from durable selection. Give the build activation
an immutable, exact requested `TargetProfile`, and require ordinary build source
to accept that value into the singular durable `Build.target` (directly or
through one ordinary operation). Source may constrain the closed profiles it
supports with normal Omega control flow; the compiler product can therefore
accept its four supported profiles without declaring four competing selected
outputs. Normalize and retain both request identity and authored acceptance,
and reject omission, substitution, or multiple selections. Resolve a `Host`
convenience before semantic build evaluation so ambient host identity never
enters the durable artifact implicitly.

### Alternates

- Acceptable if target support is entirely expressed by root/provider
  availability: initialize an immutable selected target from the invocation
  and remove source-level target selection, but revise the durable `Build`
  model and explain how source rejects unsupported targets.
- Acceptable if source must enumerate support independently: add an ordinary
  supported-target collection plus a separate singular selection operation;
  keep their identities distinct and require the requested target to belong to
  the authored collection.
- Tempting but wrong: replace the four product declarations with the current
  development host's concrete profile.
- Tempting but wrong: add a magical `Target::Host` case whose meaning changes
  after source evaluation or is omitted from normalized build identity.

## Q9 — Explicit transport authority for quotient preconditions

### Context

Quotient representative checking now admits exact substituted public `Q` facts
and strict integer `ProofFact::Expression` entailment. The arithmetic rung uses
the complete ordered public-`Q` premise roster, exact side-specific symbol,
static, and literal substitution, and a deterministic `Proven` judgment with
canonical replay evidence. Quotient-domain membership facts and opaque
proposition families are intentionally outside that engine language.

### Problem statement

A carrier-facing representative precondition `P` may follow from a
quotient-facing public condition `Q` through a transport or weakening theorem,
but the language does not say which declaration owns that theorem or how one is
selected for each left and right representative application. Inferring the
authority from ambient domain links, visibility search, same-spelled
propositions, or an opaque solver verdict would make acceptance depend on
context rather than authored relation identity. It would also leave no stable
identity or premise/application record for Terminal replay.

### Proposed direction

Require one explicit authored selection at a settled declaration locus in the
quotient or law-bearing relation surface. Resolve the selected transport or
weakening theorem to canonical identity and apply it independently to each
representative side with exact `Q` premises and `P` goal coordinates. Retain
the theorem identity, ordered premises, side, substitutions, and resulting
application evidence so validation and Terminal replay consume the same proof
object. Omission, ambiguity, inapplicability, or identity drift must fail
closed.

### Alternates

- Acceptable if proposition families themselves own transport: require an
  explicit canonical transport declaration on each family and have the
  quotient select that declaration by identity rather than visibility.
- Acceptable if relations own transport: add explicit left/right theorem
  selections to the relation declaration, even when both sides select the same
  theorem.
- Tempting but wrong: treat a quotient-to-carrier domain link as proof of every
  membership or proposition implication between those domains.
- Tempting but wrong: search visible theorems and choose the unique theorem
  that happens to type-check at the use site.
- Tempting but wrong: retain only a solver `Proven` verdict without the selected
  theorem identity, ordered premises, per-side application, and replay data.

## Q10 — Selecting an overloaded boundary-operator provider family

### Context

Each boundary-operator overload already has an exact package-qualified
provider-plan slot and may have checked or external provider candidates. A
unique covering candidate can be selected without source policy. The ordinary
build override is currently `builder.select_provider<Service, Provider>()`,
where `Service` resolves to one boundary-trait declaration. A boundary operator
instead has a descriptive path plus an overload coordinate.

### Problem statement

`CheckedMath::offset_zero` identifies an operator family, not necessarily one
overload. The existing static path has no position for parameter/result
dispatch identity, so treating it as one exact slot is unsound when several
boundary operators share that path. Silently applying the override to whichever
symbol resolution encounters first, to only the provider's matching subset, or
to every overload without an atomic completeness rule would make selection
context- or declaration-order-dependent. Inventing a stringified signature
would duplicate compiler identity in source.

### Proposed direction

Keep the concise existing form and define an operator path in
`select_provider` as an atomic family selection. Resolve the exact
package-qualified path, enumerate every applicable boundary-operator overload
coordinate in that family for the selected target, and require the selected
provider type to contribute exactly one complete candidate for every member.
Select all of those plans together or reject the complete declaration. A
project requiring different providers should use distinct descriptive operator
paths rather than splitting one overload family through hidden signature
syntax.

### Alternates

- Acceptable if per-overload selection is genuinely required: introduce one
  ordinary typed declaration-reference value whose canonical meaning already
  includes the overload coordinate, then let `select_provider` consume it. Do
  not add package-manager-only signature syntax.
- Acceptable as a stricter first release: forbid authored overrides for
  boundary operators and require target policy or one unique covering candidate
  for every exact slot.
- Tempting but wrong: select the first same-path operator symbol or use return
  type/display spelling to break the tie.
- Tempting but wrong: encode the normalized signature as an authored string.
- Tempting but wrong: apply a family override only to overloads the provider
  happens to implement and leave the rest on unrelated defaults.

## Q11 — Provider selection for a top-level boundary requirement

### Context

Bounded installation-reach rows can now remain symbolic until an installed
root joins them to one exact selected provider plan. Trait and operator
requirements already have authored `satisfies Trait::requirement` selection,
but core also owns bodyless top-level boundary requirements. Interrupt entry
and acknowledgement completion need distinct exact operation rows: PIC
completion resolves to `PortIo`, while LAPIC/x2APIC completion resolves to
`MachineControl`.

### Problem statement

No approved source form selects the realization of one exact top-level
bodyless boundary requirement. Reusing trait `satisfies` would invent an owner
that the requirement does not have; inferring a provider from equal resolved
reach rows would erase the distinct entry/completion operations and their token
lineage. Until this is settled, provider selection cannot close the top-level
completion dependency and the public interrupt completion contract cannot
replace its conservative fixed reach with a separately resolved bound.

### Proposed direction

Add one explicit nominal provider-binding form for a canonical top-level
requirement path. The binding must name the selected realization directly,
retain the requirement and operation identity independently from its bounded
reach row, and flow through selected-provider, installed-root, acknowledgement
policy, invocation, and token-lineage evidence. Missing, ambiguous, foreign,
or duplicate bindings reject; row equality grants neither selection nor
settlement authority.

### Alternates

- Acceptable: place the explicit selection in target/build provider policy if
  it still names the exact top-level requirement and realization and survives
  into installed evidence.
- Acceptable: give top-level boundary requirements a nominal owner solely for
  provider selection, provided that ownership is ordinary declared language
  structure rather than a compiler-synthesized trait.
- Tempting but wrong: choose the unique visible provider whose concrete reach
  happens to refine the same bound.
- Tempting but wrong: keep one hardcoded `PortIo` completion row and treat it as
  proof of PIC/LAPIC provider coherence.

## Q12 — Source result schema for placed-view establishment

### Context

The placement model settles three distinct operations over an exact borrow or
owned split of `Extent in Granted`: `view` interprets existing content,
`initialize` encodes new content into `Vacant` Stable storage, and `validate`
checks existing Stable content. Their successful views retain the source loan
or owned extent, every unconditional non-runtime `Type` input has an explicit
per-outcome disposition, and proof results remain in the separate `;` lane.
The guide illustrates a conceptual `PlacementResult<View, Returned>` and says
the compiler derives the Type-only `Returned` row for each instantiation.

The core source surface currently declares only `Placement::plan`.
`Placed<P, T>`, `Vacant`, `Resident<P, T>`, `PlacementError`, the three
establishment operations, and their result families have no declarations from
which source typing or checked identity can be derived.

### Problem statement

An owned placement request may fail dynamic range, alignment, revision, or
content checks, so its source result cannot be invented as an infallible
`Placed<P, T>`. The language does not yet define the nominal identity of the
compiler-derived returned-input row, how that row participates in one ordinary
closed result sum, or the exact operation signatures that return the owned
extent/resident custody on rejection. `validate` must additionally retain the
selected validator's declared content-error sum without erasing it into a
generic code. Choosing any of these shapes in the compiler would create a
public core ABI and pattern-matching vocabulary that the language has not
specified.

### Proposed direction

Declare opaque core `Placed<P, T>`, `Vacant`, and invariant
`Resident<P, T>` identities together with distinct generic `view`,
`initialize`, and `validate` operations. Give each instantiated operation one
compiler-derived nominal outcome type whose `Ready` case contains the exact
view and whose `Rejected` case contains a closed operation-specific reason sum
plus the canonical Type-only row of inputs marked `returned`. Derive that row
from the operation identity, `P`, `T`, and canonical declaration paths—not the
call site—and make its nominal identity and field order available to source
patterns, checked Psi, and artifact replay. Keep validator-specific errors as a
named nested case/payload selected from the validator contract, and keep proof
outputs outside the runtime result.

### Alternates

- Acceptable if compiler-derived nominal sums are too broad for the first
  release: declare separate core result families for `view`, `initialize`, and
  `validate`, with a canonical compiler-generated returned-row argument and
  exact validator-error argument.
- Acceptable as a narrower first rung: admit only borrowed `view` when all
  dynamic rejection cases are statically disproved, while still reserving the
  settled general result family before owned establishment becomes source
  visible.
- Tempting but wrong: expose the Rust bootstrap admission receipt or occurrence
  identifiers as forgeable source values.
- Tempting but wrong: return an opaque error code or discard moved inputs on
  rejection.
- Tempting but wrong: derive result identity from source spelling, call-site
  order, accessor names, parameter ordinals, or compact plan fingerprints.

## Q13 — Declaring a direct native-parameter callback destination

### Context

The settled outbound callback model maps one nominal static-machine binder to
one compiler-issued `NativePlace`. A nested destination comes from an
authoritative layout's explicitly cited `PrivateCallbackSlot<Requirement>`
demand. A registrar whose native ABI already has a direct callback parameter
instead needs `NativePlace::Parameter` naming that existing runtime parameter;
it must not append a hidden argument or expose a raw code address.

The compiler internally derives a nominal native-parameter identity for each
runtime parameter, but the source `BoundarySignature` publishes only semantic
shape roots, callback binders, and layout-derived private demands. No source
declaration associates one exact runtime parameter with a callback requirement
or makes its compiler-issued native-parameter identity available to the calling
policy.

### Problem statement

The language does not say where a registrar declares that an already-authored
runtime parameter is the destination for one exact callback requirement.
Inferring the association from parameter order, type shape, binder order, or a
unique pointer-sized placement is unstable and loses nominal requirement
identity. Letting policy source author a numeric `NativeParameterId` would turn
an internal coordinate into forgeable ABI authority. Adding a hidden callback
argument would change the public requirement rather than describe its existing
native ABI.

### Proposed direction

Add one declaration relationship on the registrar requirement's existing
runtime parameter that explicitly names the nominal static-machine binder (or
its exact requirement) as that parameter's private callback destination. The
compiler resolves both parameter and binder by symbol, publishes their paired
compiler-issued identities in the target-closed `BoundarySignature`, and lets
the policy select only that declared `NativePlace::Parameter`. Require exactly
one compatible supply per cited binder and reject duplicate destinations,
wrong requirements, non-runtime parameters, inferred order, and declarations
that would add or reorder ABI parameters.

### Alternates

- Acceptable: publish a compiler-derived native-parameter catalog plus a
  separate explicit requirement-level mapping operation, provided source names
  parameters and binders nominally rather than indexing either catalog.
- Acceptable for a narrower first release: support only layout-field callback
  destinations and reject direct native callback parameters until the
  declaration relationship is ratified.
- Tempting but wrong: reuse the semantic shape identity in
  `BoundarySignature.parameters` as native-parameter identity.
- Tempting but wrong: select the first pointer-sized argument, the binder with
  the same ordinal, or the only parameter whose target placement happens to
  fit a function pointer.
- Tempting but wrong: expose a raw parameter ordinal, physical register/stack
  location, or callback code address to source policy.

## Q14 — Reborrow restoration disposition

### Context

Checked borrow replay now retains each explicit direct reborrow's exact parent
resource, suspension boundary, parent/child weakening order, and a semantic-
phase lifecycle disposition. The non-authorizing disposition can identify a
still-live parent, an ordered cascade through parents that retired while
suspended, or a same-boundary `RetireOrDiscard` outcome. These rows reconstruct
the current lexical facts without treating flat constraint presence or arena
order as authority.

### Problem statement

The language does not yet define which child-ending event restores usable
authority to a live parent, whether a parent and child ending at the same
semantic boundary retires or discards the pending authority, or how a cascade
through projected retired parents transfers custody to its final parent or
direct-root lifetime. State exit further needs an exact rule for whether root
custody is returned, cleaned up, or consumed. Promoting the checked
classification to Terminal authority without these rules would let a compiler
invent post-reborrow use and cleanup semantics.

### Proposed direction

Define one path-sensitive reborrow restoration judgment over exact checked
resource identities. It should distinguish reactivation of a live parent,
cascade through an ordered retired-parent path, retirement, and discard; name
the first event at which usable authority is restored; and specify projected
place composition plus state-exit direct-root custody. Terminal publication
must retain the full disposition path and independently replay the applicable
judgment. Until this is settled, checked rows remain non-authorizing and no
post-return use, cleanup, or Terminal resource claim may be derived from them.

### Alternates

- Acceptable as a narrower first release: allow Terminal restoration only for
  a child whose exact immediate parent remains live, and reject cascades and
  same-boundary endings until their disposition rules are settled.
- Acceptable: define retirement and discard as one terminal outcome if the
  resulting root-custody and cleanup behavior is observationally identical and
  explicit in the rule.
- Tempting but wrong: call every `LivePastChild` parent reactivated merely
  because its lexical constraint remains present.
- Tempting but wrong: use weakening-arena insertion order to choose the owner
  when parent and child end at the same semantic boundary.
- Tempting but wrong: skip retired projected parents and return authority
  directly to a root without retaining and validating the complete path.

## Q15 — Nominal result carriers for observing compare-exchange

### Context

The atomic design settles two observing compare-exchange requirements and
their exact closed outcomes. Decisive `AtomicCompareExchange<T>` reports
`Exchanged | Mismatched(observed: T)`. Single-attempt
`AtomicCompareExchangeOnce<T>` additionally reports
`Uncommitted(observed: T)` when the comparison matched but the attempt did not
commit. Both require copyable `T`, success uses the success ordering, and both
failure arms use the read-compatible failure ordering.

Those names currently identify public operation requirements, not value types.
Omega source locals require an explicit nominal type, ordinary case patterns
qualify cases through that type, and the core library declares no observing-CAS
result data family. The implemented decisive source intrinsic instead exposes
the instruction-observed prior scalar; the single-attempt source intrinsic is
correctly fenced because that carrier cannot distinguish `Uncommitted` from a
mismatch.

### Problem statement

The language does not name the nominal closed result type or types that own
`Exchanged`, `Mismatched`, and `Uncommitted`. It therefore does not settle
whether decisive and single-attempt results use two distinct generic families,
one larger shared family with an impossible decisive case, or some other
nominal relationship; nor does it settle the public case-qualification paths.
Ordinary Omega sums assign tag zero and the home representation to the first
declared case, but the atomic result table specifies a set of outcomes rather
than their declaration order. Choosing `Exchanged` first would make all-zero
storage look successful; choosing a payload-bearing failure first also needs an
explicit rule for the generic `observed: T` home value.
Choosing names such as `AtomicCompareExchangeResult<T>` and
`AtomicCompareExchangeOnceResult<T>` in the compiler or core library would
create a public core ABI and pattern-matching vocabulary not specified by the
requirement table. Reusing the requirement names as value types would silently
conflate two distinct language identities.

### Proposed direction

Declare two distinct ordinary generic core result sums, one for each observing
axis, with owner-approved nominal names. The decisive sum has exactly
`Exchanged` and `Mismatched(observed: T)`; the single-attempt sum has exactly
those cases plus `Uncommitted(observed: T)`. Constrain both to copyable `T` and
make their cases available through the ordinary nominal case namespace.
Explicitly choose their canonical case order and whether uninitialized/home
storage is permitted to denote any operation outcome; do not inherit a
success-looking zero representation accidentally. Change the existing decisive
source operation to return its closed sum rather than the legacy prior scalar,
and give the single-attempt operation the distinct three-case sum. Keep the
operation requirement identities separate from these value-type identities.

### Alternates

- Acceptable: choose different explicit public names or a containing namespace,
  provided decisive code cannot observe or construct an `Uncommitted` outcome
  and the two requirement identities remain distinct.
- Acceptable: make the result carrier construction-only or give it an explicit
  non-outcome home state, if ordinary zero initialization cannot provide a
  sound failure-first representation for every admitted `T`.
- Acceptable as a migration aid: diagnose the legacy scalar annotation with a
  targeted replacement message, without retaining scalar-return semantics as
  an overload.
- Tempting but wrong: infer an anonymous sum from the local initializer; Omega
  case construction and matching are nominal, and the public pattern paths
  would still be undefined.
- Tempting but wrong: model both requirements with the observed-prior scalar or
  a success Boolean; either representation erases the specified closed cases.
- Tempting but wrong: expose `Uncommitted` on the decisive result merely because
  one larger runtime layout would be convenient.

## Q16 — Authoritative Delta v1 language and resource semantics

### Context

The bootstrap decisions require Delta to be an independent, robust
compiler-host language whose nonoptimizing lower-rung meaning elaborates to
Gamma. The canonical compiler source currently lives in
`source/delta/compiler/main.alp`, and the Rust-free implementation of the
meaning route lives in `source/delta/meaning/delta2gamma.beta`. Publication can
already bind exact source and repeated emitted assembly, but its V1 receipt
deliberately does not claim that the translator implements all Delta semantics.

No authoritative Delta language document currently fixes the compositional
grammar, static and dynamic judgments, allocation/exhaustion behavior, sealed
byte I/O, process termination, or observation profile. Historical comments in
the translator instead describe an “Omega kernel subset” and “on-ramp,” which
conflicts with the ratified decision that Delta is not an Omega subset. An
implementation and its current accepted corpus cannot select their own semantic
subject for checked refinement.

### Problem statement

The canonical Delta compiler artifact cannot acquire source-to-artifact
authority until a checker can reconstruct the intended Delta meaning from an
independently fixed contract. It is presently ambiguous which fixed storage
limits are Delta semantics versus replaceable implementation bounds, which
allocation facilities a robust compiler host guarantees, and which source
forms are compositional language facilities rather than accidents of the
single canonical source. Treating `delta2gamma.beta`, the sample corpus, or the
artifact's behavior as the specification would make the refinement circular.

### Proposed direction

Ratify one `source/delta/LANGUAGE.md` contract for a small C-like deterministic
compiler host, independent of Omega spelling. Specify the source grammar and
name/type/control judgments used by the complete canonical compiler; finite
records, sums, arrays and bounded views; checked arithmetic; deterministic
storage/allocation with explicit exhaustion; sealed byte input, artifact and
diagnostic output, and termination; exact resource parameters; and the
observable result used by refinement. Classify each fixed capacity in the
current implementation as either a contract input/bound or private storage,
and require unsupported forms and exhaustion to reject deterministically.
Then validate the Beta-written Delta-to-Gamma elaboration against that contract
and keep optimization outside the meaning edge.

This decision need not make Delta valid Omega, add Omega's proof or package
surface, or prescribe the native compiler's internal representation. Assembly
reproducibility, strict target-dialect validation, executable reconstruction,
and ordinary-Omega frontend implementation may continue while the semantic
contract is settled; only checked Delta source-to-artifact authority is blocked.

### Alternates

- Acceptable: use a smaller explicitly parameterized allocation model backed by
  one fixed arena, provided exhaustion and lifetime behavior are compositional
  and sufficient for the complete compiler source.
- Acceptable: admit additional C-like modules, allocation, or aggregate
  facilities when they materially reduce total compiler and assurance cost,
  provided their semantics are fixed independently of the implementation.
- Tempting but wrong: declare the current translator or canonical source corpus
  to be the language specification.
- Tempting but wrong: revive Delta as a named subset of Omega or infer its
  semantics from similar Omega spelling.
- Tempting but wrong: publish compiler authority from two byte-identical
  executions without checking the artifact against an independently selected
  source meaning.
