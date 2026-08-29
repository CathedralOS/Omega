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

Last pruned: 2026-08-28.

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

## Q8 — Explicit transport authority for quotient preconditions

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

## Q9 — Source result schema for placed-view establishment

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

## Q10 — Reborrow restoration disposition

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

## Q11 — Nominal result carriers for observing compare-exchange

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

## Q12 — Strict SSH trust and credential authority

### Context

Package source requests admit HTTPS, SSH URLs, and SCP-like SSH locators. The
resolver seals Git configuration, selects and hashes one exact SSH client, uses
batch mode, disables user SSH configuration, and requires strict host-key
checking. It still consumes the invoking user's default known-host and key
files. The strict resolver contract requires explicit host-trust evidence and a
closed credential-provider class before an accepted source receipt can claim
that ambient authority was excluded.

### Problem statement

No trusted command/resolver input currently supplies SSH host trust or
credentials. Treating the user's default files or agent as implicit authority
would make resolution depend on ambient mutable state that is absent from the
source question and receipt. Letting `build.omg` or dependency source choose
those values would grant untrusted package code transport and secret authority.
Persisting private key material in `omega.lock` would expose secrets while still
failing to define which process may use them.

### Proposed direction

Require trusted command infrastructure to provide one explicit resolver-owned
SSH authority input. It binds the requested host to exact known-host evidence
and selects one closed credential-provider class, such as a specifically opened
key capability or an explicitly designated credential broker. The fetch helper
receives only those capabilities; home-directory discovery and an ambient agent
remain disabled. The resolver receipt records commitments to the host evidence,
provider class, and effective endpoint, never secret bytes. This authority is
deployment input rather than package source, dependency identity, or a portable
producer claim.

### Alternates

- Acceptable for the first strict release: admit only HTTPS to accepted
  resolution while SSH remains available solely through the clearly diagnostic
  resolver path.
- Acceptable: support an explicitly selected SSH agent or platform credential
  broker as a distinct provider class, provided its identity and authority are
  bounded and receipt-visible rather than inherited.
- Tempting but wrong: inherit `~/.ssh`, a default agent, or system Git/SSH
  configuration and call strict host-key checking sufficient custody.
- Tempting but wrong: let a package, dependency declaration, repository, or
  `build.omg` select host trust or credential material.
- Tempting but wrong: serialize private keys, tokens, or reusable credentials in
  `omega.lock` or source-resolution evidence.

## Q13 — Suspension as control-flow exit or resumable continuation

### Context

Terminal Psi currently retains whether a machine or call may suspend as
declaration and effect knowledge. Its executable control-flow vocabulary has
normal returns and crash exits, but no suspension terminator, resume edge, or
continuation identity. Omega's optimizer can therefore preserve conservative
`MaySuspend` knowledge, but it cannot construct the explicit suspension exits
required by post-dominance, liveness, ownership, provenance, and fuel analyses.

### Problem statement

Treating suspension as an ordinary no-successor exit would discard the state
that must survive resumption. Treating it as an ordinary successor would imply
that resumption is synchronous local control flow. Inferring either model from
a declaration-level `suspends` flag would silently choose language semantics
and could make post-dominance or cleanup reasoning unsound.

### Proposed direction

Represent suspension explicitly in Terminal Psi as a semantic transfer with a
stable continuation/resume identity. Define which values, places, claims,
cleanup obligations, provider effects, and logical-fuel sites cross that
transfer. Omega may then model the suspension event as an observable exit from
the current activation while separately retaining the authorized resume edge
and its custody.

### Alternates

- Acceptable if suspension never resumes the same activation: define it as a
  terminal outcome distinct from normal return and crash, with complete exit
  ownership and fuel semantics.
- Acceptable if suspension is call-only: keep local CFGs free of suspension
  terminators, but define an explicit interprocedural call outcome and clarify
  that block post-dominance deliberately excludes it.
- Tempting but wrong: classify every `MaySuspend` call as a local CFG exit
  without retaining its continuation and outcome-specific state.

## Q14 — Cyclic control flow in Terminal Psi

### Context

Omega's analysis vocabulary includes block SCCs, reducible/irreducible loop
classification, dominators, and post-dominators. Those algorithms are tested on
synthetic graphs, but total optimizer-unit validation currently rejects every
`ControlCycle`. Executable repetition is otherwise expressed through machines,
state transitions, and calls.

### Problem statement

The optimizer cannot exercise loop transforms or production loop-forest
consumers until the semantic handoff either admits cyclic block graphs or
declares that Terminal Psi is intentionally acyclic. Admitting cycles requires
defined SSA edge bindings, loop-carried ownership and cleanup frontiers,
progress/termination evidence, observation boundaries, and logical-fuel
accounting. Inventing those only inside the optimizer would create a second
language semantics.

### Proposed direction

Decide explicitly whether Terminal Psi may contain cyclic block control flow.
If yes, add a versioned cyclic vocabulary and validator contract that retains
loop-carried values, ownership/frontier state, progress evidence, provenance,
and fuel before Omega accepts the first real cycle. If no, make acyclicity a
durable language/IR invariant and redefine loop optimization over the actual
machine/state-transition representation rather than maintaining a decorative
block-loop API.

### Alternates

- Acceptable: admit only structured reducible loops first, leaving irreducible
  cycles rejected until their ownership and progress contracts are explicit.
- Acceptable: keep Terminal block CFGs acyclic and expose a separate verified
  state-machine cycle graph for loop-like optimization.
- Tempting but wrong: permit cyclic `Jump`/`Conditional` graphs by weakening
  validation before loop-carried SSA, ownership, cleanup, and fuel semantics
  exist.

## Q15 — Close the Delta v1 semantic contract

### Context

Delta is the independent C-like compiler-host language accepted by the
Gamma-written Delta compiler and used to author the first full Omega compiler
implementation `D`. `source/delta/LANGUAGE.md` now separates Delta from the
deleted Beta-written Delta-to-Gamma route, but a corpus audit found choices that
change source validity or observable meaning. Implementing them ad hoc inside
`delta_compiler.gamma` would make the compiler, rather than the language
contract, define Delta.

The deleted native compiler prototype demonstrated the intended plain core:
records, fixed arrays, `i32`/`u8`, receiver machines, states, arithmetic,
strings, and Console calls. It does not require packages, attributes,
contracts, proof syntax, range types, or general domains. The broader test
corpus does exercise several of those unsettled forms.

### Problem statement

The following decisions must be closed together:

1. `Incomplete` is currently listed as a `DeltaV1` result and later called not
   a Delta result. Decide whether it is a language observation, an
   execution-profile outcome, or an outer compiler/checker status. Enumerate
   every exact reject code/offset and trap kind so failure is deterministic.
2. Decide whether keywords are reserved or contextual. The grammar reserves
   `state`, `transition`, `machine`, and others, while
   `contextual-state-identifiers.delta` uses them as identifiers. The reserved
   set also currently omits `use`, `requires`, `ensures`, and `assert`.
3. Either define or remove v1 `use`, attributes, domains, `requires`,
   `ensures`, and `terminates by`. Existing tests additionally use field
   domains, a special `result` binding, and `i32 in 0..N` range types that the
   grammar does not admit. Define whether `min`/`max` are reserved builtins,
   shadowable builtins, or ordinary declarations.
4. Define the sealed boundary ABI: whether bare `read_byte`/`write_byte` are
   sugar for `self.console`, the type/lifetime of decoded string literals, and
   their conversion to `&[u8]` for `write_line`.
5. Define the outcome of a scalar transition with no matching arm and no `_`.
6. Reconcile one Delta translation unit with the package-resolved closure `D`:
   select either one canonical packed/resolved unit supplied as compiler input
   or a real Delta module/import model with an exact closure owner.

### Proposed direction

Keep Delta v1 deliberately small and sufficient for `D`:

- make `Incomplete` an explicit verifier/compiler resource outcome outside
  source execution, while `Exit`, `Reject`, `Trap`, and `Diverges` are Delta
  observations; bind private-capacity failures to `Incomplete` without partial
  artifact bytes;
- reserve the complete keyword set and rewrite tests that pin the deleted
  translator's contextual-keyword behavior;
- omit `use`, attributes, range types, proof-oriented contracts, and
  `terminates by` from v1 unless a concrete `D` implementation need is shown;
  retain only arithmetic-domain placements that receive complete rules;
- define string literals as immutable call-scoped byte views, and define bare
  byte I/O as exact sugar for the single threaded Console capability;
- trap deterministically on a nonexhaustive scalar transition; and
- give the Delta compiler one already resolved, canonically packed translation
  unit, leaving package resolution outside Delta semantics but inside exact
  source custody.

This direction minimizes the Gamma compiler and keeps package/proof semantics
out of Delta without weakening its ability to host a robust compiler.

### Alternates

- Acceptable: retain contracts and arithmetic domains if their complete static,
  dynamic, failure, and result-binding rules are fixed now and needed by `D`.
- Acceptable: use contextual keywords if every grammar position has an
  unambiguous deterministic resolution rule and the complexity is justified.
- Acceptable: make `Incomplete` part of a larger profiled evaluation judgment,
  provided it is no longer simultaneously denied as a Delta result and its
  relation to divergence/exhaustion is exact.
- Tempting but wrong: implement whatever the 75 historical positive files happen
  to accept and call that the Delta specification.
- Tempting but wrong: retain the old translator's private capacities, exit
  codes, or Darwin output behavior as language rules.

## Q16 — Select one typed executable Gamma contract

### Context

Gamma is the safe definitional rung used to write the Delta compiler. The
current repository implements two disconnected surfaces:

- `interp.beta` accepts untyped `def* EXPR` programs and evaluates a final
  expression; and
- `typeck.beta` accepts typed `data* def*` programs, checks every definition,
  and has no executable entry or final expression.

The intended Delta compiler must be typed and executable. Gamma is otherwise
pure and currently has no source-level byte-I/O effects. A Beta-written
Gamma-to-Alpha compiler therefore cannot choose its entry, byte-stream ABI,
outcome mapping, or fuel meaning without adding language semantics.

### Problem statement

Fix one canonical contract for:

1. typed executable grammar and erasure/evaluation after type checking;
2. a unique entry declaration;
3. a pure compiler ABI mapping sealed Alpha stdin bytes into a Gamma value and
   the accepted Gamma result into exact tape/stdout bytes;
4. malformed source, type error, trap, private exhaustion, fuel exhaustion,
   divergence, and the no-partial-artifact rule; and
5. whether the current 50,000,000-call evaluator fuel is language meaning or a
   verifier-selected resource-profile parameter.

The contract must support arbitrary constructor arity and realistic functions
with more than Beta's four register arguments. Those are implementation ABI
requirements, not reasons to extend Beta or Alpha.

### Proposed direction

Use one typed `data* def*` source language with a distinguished declaration of
type equivalent to:

```text
main : Bytes -> CompileOutcome
```

`Bytes` and `CompileOutcome` are ordinary closed Gamma data types fixed by the
compiler-entry profile. The generated Alpha runtime alone reads sealed stdin,
constructs `Bytes`, invokes pure Gamma `main`, and serializes the selected
outcome. Successful artifact bytes are exact; rejection, trap, or private
resource exhaustion publishes no partial tape. Call fuel and heap/tape limits
are explicit resource-profile parameters and cannot change Gamma meaning.

The Beta-written compiler then type-checks before emission, erases types into a
defined runtime representation, uses a custom arbitrary-arity Gamma frame ABI,
preserves proper tail calls, and emits Alpha tape directly. `interp.beta` and
`typeck.beta` remain semantic oracles/components only while they expose distinct
failures.

### Alternates

- Acceptable: retain a final typed expression rather than a named `main`, if
  its type and byte-stream adapter are equally unique and explicit.
- Acceptable: define a different closed `Bytes`/outcome carrier, provided the
  mapping to sealed input, exact output, and failure status is canonical.
- Tempting but wrong: compile the untyped interpreter language while describing
  Gamma as the typed safety rung.
- Tempting but wrong: publish an interpreter plus serialized AST as the final
  compiler architecture; it duplicates the evaluator in every tape and leaves
  the compiler dependent on interpreter capacities and dispatch cost.
- Tempting but wrong: make Alpha I/O effects directly callable from arbitrary
  Gamma source merely to avoid defining the compiler-entry adapter.

## Q17 — Fix Beta block formation and definite-initialization reachability

### Context

Beta procedures have one entry block and an ordered state-machine CFG. A call
binds only parameters; every other local becomes initialized when its `let` is
executed. The Alpha-written compiler currently proves only that a referenced
slot was declared earlier in source order. A jump may skip that initializing
store, so source-order lookup is not the required every-path property.

The implementation audit produced a bounded forward must-analysis over the
entry plus at most 1,024 state blocks. It records per-block reads-before-write,
writes, reachability, fallthrough, and per-transition write prefixes, then
intersects initialization at joins. That machinery fits below the existing
1 MiB compiler source buffer and needs no new Beta feature.

### Problem statement

Two language-formation choices remain unstated:

1. `LANGUAGE.md` recursively includes `state` as a statement in any block,
   while `SEMANTICS.md` describes a flat entry block followed by ordered state
   blocks. Decide whether nested states and loose ordinary statements after the
   first state are valid and, if so, exactly how they become blocks and acquire
   fallthrough edges.
2. Define the static reachability criterion for initialization. In particular,
   decide whether every guarded `to S when e` contributes both its target and
   false-continuation successors even when `e` appears constant, and whether
   reads in syntactically unreachable blocks are checked.

These choices change which Beta programs are well formed. They cannot be
selected solely inside `beta_compiler.alpha`.

### Proposed direction

Keep Beta's state graph flat and mechanically auditable:

- a procedure body consists of ordinary entry statements followed by zero or
  more sibling `state` declarations;
- a state body contains ordinary statements but no nested `state` declaration;
- no loose ordinary statement follows the first sibling state declaration;
- sibling state blocks fall through in source order, while `return` and an
  unconditional `to` terminate the remaining path in their block;
- a guarded `to` always contributes both target and false-continuation edges
  for definite initialization, without constant folding or call reasoning; and
- only blocks reachable from that procedure's entry under those syntactic
  edges are checked for reads-before-initialization. Every procedure is analyzed
  independently, including one with no callers.

This gives one deterministic, terminating byte-vector must-analysis and keeps
semantic acceptance independent of optimizer sophistication.

### Alternates

- Acceptable: permit nested states if flattening, name scope, source order, and
  fallthrough are specified without depending on compiler traversal accidents.
- Acceptable: require every syntactic block, including unreachable blocks, to
  be initialization-safe; this is simpler but deliberately rejects more dead
  source and must be stated as formation rather than runtime meaning.
- Tempting but wrong: infer a constant guard or callee result in the bootstrap
  compiler to recover initialization, making well-formedness depend on an
  increasingly sophisticated evaluator.
- Tempting but wrong: zero-initialize generated frame slots and call the gap
  closed; that changes Beta's written local semantics and hides skipped stores.

## Q18 — Select the canonical Beta compiler outcome carrier

### Context

The canonical Alpha-written Beta compiler now publishes its complete Alpha tape
only after two checked passes and fixup resolution. Every current failure leaves
artifact stdout empty, but the boundary still identifies failure with numeric
Alpha halt values: malformed-source paths expose parser phase numbers, source
capacity uses another number, and internal replay/fixup failures use another.
`TASKS_BOOTSTRAP.md` requires typed `Reject` and private-budget `Incomplete`
outcomes rather than treating host process status as the semantic contract.

This question concerns compilation itself. Status 250 for a generated Beta
program's data-stack exhaustion and status 251 for its invalid raw-memory access
are runtime containment outcomes and must not be conflated with compiler
failure.

### Problem statement

Select one closed compiler-boundary result and its exact Alpha realization:

1. Define the cases, at least successful artifact, malformed/invalid source,
   private producer exhaustion, and compiler invariant failure.
2. Decide which rejection reason, source offset, resource kind, limit, and
   requested amount are observable, and how they are represented within
   Alpha's sealed stdin/stdout/halt observation model.
3. Preserve raw tape bytes as the exact successful artifact while ensuring a
   failed run cannot be mistaken for a partial tape. No shell wrapper or host
   script may supply the missing type distinction.
4. Classify identifier, syntax-nesting, procedure/state/edge/call/slot tables,
   internal labels/fixups, private tape extent, and source extent consistently
   as language rejection, profiled `Incomplete`, or internal failure.

### Proposed direction

Use a proof-level closed sum such as:

```text
BetaCompileOutcome =
    Complete(Bytes)
  | Reject(phase, source_offset, reason)
  | Incomplete(resource, limit, requested)
  | InternalFailure(code)
```

`Complete` alone publishes the raw Alpha tape. Every other case publishes no
artifact bytes. Bind an exact Alpha-level encoding of the selected case and
fields, then make gates decode that encoding into the sum; a Unix shell's
truncated exit status is only a realization detail, never the definition.
Malformed or statically invalid Beta maps to `Reject`, checked private ceilings
map to `Incomplete`, and disagreement between the two compiler passes or an
impossible fixup/table condition maps to `InternalFailure`.

### Alternates

- Acceptable: publish a canonical tagged diagnostic byte sequence on failure,
  provided it is unambiguously not an artifact and no partial tape precedes it.
- Acceptable: keep failure stdout empty and use a compact halt-word encoding if
  every required field and the host-realization projection are exact.
- Tempting but wrong: assign a few undocumented process exit numbers and call
  them typed outcomes.
- Tempting but wrong: prepend a success tag to Alpha tape and thereby change the
  canonical artifact bytes or require a stripping stage.
