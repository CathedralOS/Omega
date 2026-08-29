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

Last pruned: 2026-08-29.

## Q1 — Lifetime application on conformance target traits

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

## Q2 — Authority-bearing roles for ordinary packages

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

## Q3 — Explicit transport authority for quotient preconditions

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

## Q4 — Source result schema for placed-view establishment

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

## Q5 — Reborrow restoration disposition

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

## Q6 — Nominal result carriers for observing compare-exchange

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

## Q7 — Strict SSH trust and credential authority

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

## Q8 — Suspension as control-flow exit or resumable continuation

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

## Q9 — Cyclic control flow in Terminal Psi

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

## Q10 — Close the Delta v1 semantic contract

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

## Q11 — Select one typed executable Gamma contract

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

## Q12 — Fix Beta block formation and definite-initialization reachability

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

## Q13 — Select the canonical Beta compiler outcome carrier

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

## Q14 — Canonical kernel propositions for exact scalar operations

### Context

Terminal scalar semantics now owns typed `CanonicalScalarGoal` carriers for
every proof-bearing exact integer operation. Nonzero divisors, defined exact
division, and exact shift counts have settled kernel propositions, so the
verifier reconstructs those obligations directly and checks the producer's
serialized derivation without searching its mirrored candidate frontier.

Three goal families intentionally return no kernel proposition: exact-cast
representability, exact shift-left representability, and exact arithmetic
representability for add, subtract, and multiply. The bootstrap verifier still
uses its legacy sufficient-form reducer for those families, so their accepted
proof question can depend on available facts.

### Problem statement

The language must choose the exact proposition vocabulary and coordinates for
these representability obligations. The choice determines certificate identity,
which algebraic facts are premises versus derivation steps, how signed and
unsigned bounds are expressed, and what the independently reconstructing
verifier checks. Selecting whichever sufficient interval or affine consequence
happens to be discoverable would keep acceptance search-dependent; inventing a
compiler-private proposition would make producer and checker semantics drift.

### Proposed direction

Give each remaining typed goal one direct, canonical kernel proposition derived
only from its operation schema and operands:

- exact cast: the source value is representable in the exact target integer
  carrier;
- exact shift-left: the mathematically shifted value is representable in the
  value carrier, separately from the already-settled shift-count obligation;
- exact add, subtract, and multiply: the operation's mathematical expression is
  representable in its result carrier.

Represent carrier membership with one shared canonical proposition family and
retain the exact integer type plus structural scalar expression. Algebraic
rewrites, interval bounds, aliases, and affine decompositions remain producer
proof steps rather than alternate verifier-selected obligations. Once these
mappings are settled, remove the remaining production sufficient-form reducer
and mirrored verifier search tree.

### Alternates

- Acceptable: define distinct closed proposition families per operation if a
  shared representability proposition cannot preserve exact operation identity,
  provided each mapping is deterministic and schema-owned.
- Acceptable as a narrower first release: reject an unsettled exact operation at
  Terminal publication until its canonical kernel proposition is available.
- Tempting but wrong: keep choosing a unique sufficient proposition by searching
  the verifier's available-fact frontier.
- Tempting but wrong: serialize only the producer's chosen goal and trust it
  without independently reconstructing the operation-owned proposition.

## Q15 — Compose the exact Alpha-to-Beta edge within checker capacity

### Context

The first bootstrap edge must establish exact correspondence between the
78,109-byte `beta_compiler.alpha` source and its 20,977-byte Alpha tape. The
generic checker binds both raw subjects and can check a balanced trace, local
assembly grammar, widths, label uniqueness, absolute fixups, and complete
source/tape exhaustion without an assembly-specific kernel rule.

The selected certificate shape required one closed
`VERIFY(source, tape, trace) = ACCEPT` equality discharged wholly by
computation and reflexivity. Compiler-scale prototypes established a hard
implementation conflict. Dynamic balanced cutting accepts 714 canonical leaves
and fails at 715. Structural recursion traverses all 6,467 leaves in 0.704
seconds, but adding local parsing exhausts the arena; even a content-free visit
of every raw source byte fails. Sequential state threading instead exhausts the
generated semantic stack. The checker reclaims normalization scratch only after
each complete equality decision, so a single root conversion retains every
branch temporary.

A checker-native carrier control split the same source into 112 named equality
decisions, visited every byte, and composed their checked propositions with
`use`; it accepted in 1.192 seconds. This establishes that per-equality scratch
reclamation is viable. It does not establish the required exact boundary chain
or any assembly semantics.

### Problem statement

Choose how exact compositional work becomes the one admitted root edge without
weakening subject identity, partition/exhaustion, grammar, label, fixup, or tape
equality. This is an architecture choice: further trace compression or another
local parser cannot change the lifetime of temporaries inside one equality.

### Proposed direction

Permit one fixed artifact-owned proof to check bounded, subject-bound chunk
equalities by reflexivity, then derive the single root edge equality through the
existing checked equality congruence/`eqelim` rules. Every chunk must expose
exact source and tape boundary states; composition must prove adjacency, order,
unique ownership, root start/end, and full exhaustion. Chunk goals, theory,
subjects, and the final proposition remain owner-fixed. No host result, status
code, hash, generated receipt, or producer assertion becomes a premise.

This changes proof composition, not the trusted calculus or assembly meaning.
It also uses the checker's existing sound scratch-reclamation boundary instead
of adding an assembly-specific evaluator path.

### Alternates

- Acceptable if kept fully generic: implement sound branch-local reclamation or
  garbage collection in every checker implementation, prove that live normal
  forms cannot reference reclaimed nodes, and retain the single-reflexivity
  certificate shape.
- Tempting but wrong: raise or bypass an undocumented memory bound until this
  one certificate happens to fit.
- Tempting but wrong: split the source into independent local claims without
  checked boundary-state composition and call their conjunction the edge.
- Tempting but wrong: restore the deleted status ledger, add an assembly
  primitive, trust a producer receipt, compare hashes, or weaken exact total
  partitioning.

## Q16 — Own the ranked native-fuel sponsor entry

### Context

The exact ranked-`u32` countdown now reaches directly metered final-image,
format-43 installation, and source-free native-artifact custody on Linux x86-64
and AArch64. Transfer-runtime encoding and replay already retain exact activation
slots, interrupted/saved/restored state, transfer/resume bytes, sponsor-stack
demand, relocations, and full unrelocated/final text fingerprints. Ranked
transfer admission also requires the activation record to save the actual ABI
rank carrier (`rdi` or `x0`).

The runtime binder requires the sponsor symbol to be an existing nonempty text
function in the metered object. The admitted ranked artifact deliberately owns
exactly one semantic function: the countdown itself. Naming it as sponsor would
make exhaustion call the countdown under an unrelated sponsor ABI and is not a
valid execution model. Appending an unowned compiler helper would contradict
the exact one-function artifact and hide a new authority edge.

### Problem statement

Choose which owner supplies the sponsor entry and how that ownership joins the
ranked image without turning runtime scaffolding into a second semantic source
tree. This blocks honest native rank 0, 1, and 3 execution/schedule comparison;
it does not block direct metered publication.

### Proposed direction

Bind the transfer runtime to an admitted installed sponsor route owned outside
the ranked semantic object. The installation/external-root join should name the
exact sponsor artifact, calling contract, target, and provision, while the
compiler-owned transfer stub remains the only appended runtime text. Preserve
the one-function ranked semantic identity and require source-free replay to
prove the final call target is exactly that admitted sponsor entry.

### Alternates

- Acceptable: define one compiler-owned sponsor body as an explicit, separately
  identified runtime artifact with a closed ABI and proof/replay contract, then
  compose it with the ranked image rather than laundering it into the semantic
  object.
- Acceptable for the first measurement only: use an already admitted fixed
  sponsor fixture as differential-test scaffolding, provided no result is
  reported as production installation authority.
- Tempting but wrong: use the countdown entry itself as sponsor.
- Tempting but wrong: append an anonymous helper, magic host callback, script,
  or test-only trampoline and treat successful execution as chain evidence.
