# Build and package material awaiting consolidation

> **Needs porting.** This remaining material still mixes contracts and
> implementation details. See the [migration index](../README.md).

[Build declarations](../../spec/build/declarations.md) and
[package acceptance](../../spec/packages/acceptance.md) now own project roles,
identity, graph discovery, and acceptance boundaries. The
[package guide](../../language_guide/packages.md) explains install/update.
The [build execution contract](../../spec/build/execution.md) owns admission,
generated-source visibility, and dependency handoff. The
[standalone request](../../spec/build/compiler_request.md) and
[source selection](../../spec/packages/sources.md) own compiler input and
resolved graph construction. [Configuration](../../spec/build/configuration.md)
owns activation roots, durable Build output, and target selection;
[macOS publication](../../spec/build/macos_application.md) owns GUI packaging.
This file does not redefine those subjects.

[Target slots and entry roots](../../spec/build/entry_roots.md) now owns arrival
bridges, root selection, and runtime ceilings. Installed introduction/accounting
lives in [content custody](../../spec/resources/content_custody.md).

[Provider selection](../../spec/build/provider_selection.md) and
[opaque representations](../../spec/build/opaque_representations.md) own build
choices; [boundary realization](../../spec/terminal-psi/boundary_calls.md) owns
application/physical evidence, and [component publication](../../spec/build/component_publication.md)
owns independent closure and installation.

[Build observations](../../spec/build/observations.md) owns replay and source
rebuildability. The Rust evaluator's [custody and replay notes](../../../omega-rust/omega/build/build-evaluation/README.md)
own supported operation sequences and private provisions.

## Package admission projection

After successful checking, the compiler derives the candidate's package report
from the earliest semantically complete representations. `PackageAdmissionProjection`
and `CompilerIssuedPackageReview` name existing internal interfaces, not a
second source of trust. The report binds the exact source graph and target to
public API contracts, declared and inferred reach, reachable implementation
authority, selected providers, opaque external supplies, accepted assumptions,
and relevant build/generated-source facts.

The report must be complete for each supported candidate. Unknown authority,
unbound boundary identities, omitted transitive effects, and unresolved proof
obligations reject rather than appearing as an empty capability set. Generic
obligations remain explicit until the compiler can check their substitutions.
Public API ceilings and actual selected-program reach remain distinguishable:
an unused dangerous API is still relevant to reviewing the package.

The manager compares that report with the project's accepted baseline and asks
for decisions on the exact blocking changes. Dependency-owned acceptance never
supplies the consuming project's decision. When checking and conflict resolution
succeed, the transaction records the selected graph and accepted baseline.
There is no intermediate requirement to certify a package artifact, seal an
instance, or promote a review through several evidence types before writing a
lock.

Current code retains `OrdinaryPackageObligationLedger`,
`CanonicalSourceClosureSubject`, `CanonicalPackageReconstructionQuestion`,
and `AcceptedOrdinaryClosureEvidence`. These are existing implementation
structures, not mandated stages in the new transaction. Reuse the source,
graph, target, obligation, and decision consistency checks that serve the
workflow. Consolidate redundant wrappers and repeated same-process checks whose
only purpose is manufacturing acceptance authority. This documentation change
does not claim that the implementation or install/update commands already
perform that simplification.

Compiler proof validation remains mandatory where the language requires it.
A genuine certificate proves its particular checked proposition; a project may
explicitly accept a disclosed boundary assumption but cannot approve an invalid
proof. Native artifact verification, including D29/D32/D41 realization
relationships, remains a compiler responsibility for emitted-code guarantees.
It does not gate source installation when the necessary capability facts are
available before emission. Compiling the installed package may still fail for
unsupported native behavior.

Review consumes the resolved immutable source graph and compiler-checked
package identities. Aliases are requester-local edges; package names alone
cannot authorize imports or impersonate trusted boundaries. Source changes
must change the review's input identity even if normalized capability rows do
not change. Recheck acquired content and detect project/candidate changes before
transaction publication.

Consumer-scoped semantic bindings still resolve exact package-owned
declarations, requirements, and selected providers. A preliminary compiler
pass may discover those bindings, followed by final checking with them. Only
the final findings enter comparison. This preliminary/final split is justified
by resolving actual inputs, not by declaring one same-process result more
trusted than another. Dangerous findings still require project policy.

Build-generated source must be included in the checked candidate and retained
through the operation that uses it. Native publication may reuse that checked
candidate, but installation does not have to produce or publish an executable.
Compiler/build replay transcripts and certificate caches remain outside the
ordinary accepted lock unless a separate artifact workflow consumes them.

Ratified 2026-08-26: the implementation should read each fact from the earliest
coherent compiler-owned representation in which its semantics are established.
Exact structural identity may come from private pre-Psi typed or resolved
state, while checked acceptance, effects, proofs, and realization come from the
stage that establishes them. The projector joins those facts only after
successful checking. Different rows may therefore come from different internal
representations; the final projection must be total, but no single intermediate
representation must contain every row. This may couple the checker to unstable
compiler-private representations: the checker is part of the compiler and
moves with them. That coupling does not make an internal representation a
package format or public compatibility surface. Unchecked syntax, diagnostics,
and convenient-but-unsettled shapes remain inadmissible as evidence.

This projection is not another public IR stage and does not warrant a nominal
Chi stage merely for collection or format stability. It has no execution
semantics or transformation pipeline of its own. A future shared stage is
warranted only if implementation discovers a genuine reusable semantic
invariant boundary. Additional consumers or transformations may reveal such a
boundary; stability, layer purity, or local simplification alone do not. Psi
may repeat the same invariant as a downstream backstop without becoming the
mandatory reconstruction source for a
fact already complete in an earlier compiler-owned representation.
Conversely, discovery may place more rows in an existing coherent
representation such as `Exact` when that simplifies the compiler without
erasing meaning.

Canonical rows may carry compiler-issued explanatory source coordinates without
making those coordinates semantic identity. Paths are canonical UTF-8 and
relative to their package or toolchain source owner; spans are exact byte
offsets into compiler-consumed source. The coordinates remain outside canonical
capability bytes, but changed-row conflicts bind the exact old/new coordinates
shown to the reviewer. Dangerous-authority rows include the toolchain authority
declaration and package exposure declarations. Generated symbols follow their
authored derivation origin, while compiler-derived rows state a closed reason.
Ordinary projection retains the exact declaration symbol beside each semantic
row and sorts the pair; dangerous-authority projection retains the exact service
declaration and exact exposing callables while deriving the row. No later source
join reconstructs those coordinates from reduced nominal identity.
Provider candidate derivation captures a compiler-internal sidecar beside each
semantic plan: exact boundary-schema and optional nominal-provider symbols, and
the exact requirement plus realizing machine for every external or
checked-adapter row. Review v41 encodes those schema, provider, requirement, and
realization declarations as package-qualified nominal identities; readable
plan and overload strings remain operational/audit data. Projection verifies
each declaration against the selected plan's exact package owner, or against an
exact authored toolchain-source identity when the plan carries no package
owner. Package-less user source, unresolved/source-free ownership, and owner
drift reject. Explanatory source custody records each exact requirement
declaration separately from its realizing machine, preventing a provider row
from retaining only its implementation anchor. Selection
and canonical sorting keep that pair intact, adding exact authored build/target-
default sites or a closed reason for an implicit unique choice. The resulting
selected-provider row may mix authored coordinates and compiler-derived reasons
without reconstructing provenance from reduced names, schemas, or fingerprints.
Nested use sites remain incremental provenance carriers in existing Psi stages
or compiler-internal sidecars, not a reason to create nominal Chi.
Public-trait composition is the first such carrier: canonical sorting keeps
each typed parent identifier's exact authored span with its trait row under a
closed `trait_parent` role. Syntax, resolved, and typed contracts retain the
exact authored clause-keyword span independently from semantic facts. Direct
machine, public trait/top-level-requirement, and public-operator contracts carry it under
`contract_clause`; projected declaration families recursively collect the same
anchor from structural static-machine parameter contracts. This uniformly covers expression, membership, proposition,
named-evidence, and outcome-group forms. Accepted-claim rows share the callable
source sidecar and therefore point to the trusted `ensures` clause. These
coordinates remain outside semantic row bytes,
while checked body calls add exact `body_call` anchors by joining checked-flow
coordinates to typed statement, expression, and named-transition sites during
checked lowering, before provider settlement may rewrite typed call identity.
Statement and transition sites retain explicit authored call-selection
occurrences; expression sites reuse their existing attached occurrences. The
join verifies checked target, receiver, receiver shape, and operational
acknowledgement at capture. A legitimate late-bound target does not invalidate
source custody: the span proves where a call was authored, not that target
finalization already occurred. Missing, duplicate, unknown, or contradictory
provenance rejects, while compiler-generated calls produce no invented source
location. Authored `invokes` targets are retained as one typed record binding
their exact parameter-symbol/ordinal or boundary-trait symbol to the target-name
span. Invocation inference consumes that target rather than reselecting by
spelling. Callable, public trait/top-level-requirement, and recursively structural
machine-parameter review rows carry the span under `synchronous_invocation`,
with top-level rows joined exactly to checked invocation facts. Those facts
retain the exact symbolic published and inferred targets before provider
settlement; package review does not re-infer effects from the transformed typed
tree.
Authored `reaches` clauses retain every keyword and target occurrence through
syntax, resolution, typed lowering, copying, and specialization. Resolution
binds targets once to exact boundary-trait symbols. Projection rederives the
parent-closed semantic row from those targets plus invocation-contributed
services and joins it exactly to typed and checked facts. A private memberless
authored clause is a published empty ceiling, not omitted private inference.
Review carries authored member spans—or the keyword span for an empty row—under
`service_reach`, without inventing locations for inferred, invocation-only, or
parent-closure entries. Recovery envelope v6 and conflict fingerprint
v9/rendering V8 bind that reach-source schema. Authored `suspends` and `blocks`
keyword occurrences now survive syntax, resolution, typed lowering, trait-
default synthesis, copying, and specialization. Callable, public trait/top-
level-requirement, and recursively structural machine-parameter rows carry distinct
`suspension` and `blocking` roles. Projection requires retained keywords,
authored booleans, and checked published/internal interfaces to agree exactly;
omission and inference receive no invented location. A public or otherwise
contract-supplied machine's checked operational fact remains its published may-
ceiling, not an observation that the current body exercised that permission.
Review v75/row v33, recovery v13, conflict fingerprint v16, and renderer V15 bind
the current source schema. External executable leaves retain the exact authored
`via` keyword beside the normalized binding identity on the same conformance.
Projection requires binding/span parity and carries that occurrence under
`external_binding` for public and private trait, operator, or top-level
requirement supply. Semantic
row bytes remain unchanged; missing, source-free, or contradictory custody
rejects. Public const declarations additionally retain the exact parsed
initializer-expression span through symbol resolution and typed lowering,
before substitution erases the value tree. `PublicConst` rows carry it as
`const_initializer` beside the declaration-name anchor. Relocation changes the
explanatory coordinates but not the semantic row bytes. Every authored proof
fact retains its full semantic-token
extent under `proof_fact` through syntax, resolution, typed lowering, generic
synthesis, and checked specialization. Public domain/data facts require that
custody, as does every fact beneath an authored public contract clause;
source-free compiler synthesis receives no invented coordinate. Absent
late-stage spans must be retained by their earlier owner, not reconstructed
from source text.
Public trait rows additionally retain every exact machine-requirement
declaration under `trait_requirement`; public data rows retain fields, sum
cases, and payload fields under `data_member`. These roles consume the existing
typed declaration symbols. Direct declarations use their authored spans;
generated declarations expose only their real derivation origin.
Reviewed package callables, public operators, and public trait/top-level requirements
likewise retain every value-parameter declaration under `callable_parameter`.
The same compiler-owned walk covers value parameters nested in structural
static-machine contracts. These coordinates bind what review displays without
changing semantic row identity.

Contract and bundle projection follows this cross-representation rule. Typed
expressions own structural subjects and arguments; checked facts own proof
acceptance, exact witness identity, validity, and admitted assumptions. The
projector joins them without publishing compiler-private representations.
Rendered diagnostic strings cannot be parsed back into semantic identity.
The mathematical-proof migration must preserve these joins for the replacement
contract language and bundle interfaces; unsupported forms fail closed.

Package-visible structural identity follows the same rule: every non-binder
nominal in a public type is qualified by exact package ownership, an exact
source-backed toolchain commitment, or an unresolved marker, while generic
binders are alpha-normalized without an invented owner. The compiler joins a
toolchain nominal through private `SourceId` state, but only the canonical
source commitment enters review bytes; a missing join rejects exact review
rather than degrading to the generic toolchain marker available to weaker
compiler-local identity. The compiler's 22 exact root builtin-type slots instead
encode closed compiler atoms, selected by root position and `BuiltinType` kind
rather than name. Package-authored lookalikes and source-free generated symbols
remain unresolved. Carry permissions use closed enum atoms in a non-nominal
tagged lane. Value domains, layout atoms, and other source-free compiler
semantics require their own closed structural carriers before they can enter
package evidence. Arithmetic domains and aggregate carry policy are already
closed enums; rendering their compiler-owned labels is not an authority hole.
Typed domain constraints now distinguish declared, carry, closed value-domain,
and `OmegaLayout` subjects. Layout retains a closed grammar and an exact
structural schema type with its declaration symbol. Symbol-backed declarations
remain declared regardless of diagnostic spelling. Review v41 encodes the
compiler variants structurally and rejects legacy/unclassified layouts,
malformed subjects, residual const calls, unsupported index forms, and missing,
duplicate, or incomplete checked index selections.
Review v41 also parses the compiler-reserved canonical-const transport atom
back into a closed type-and-encoding term and excludes its diagnostic display.
Decimal const leaves become numeric terms. Both forms are legal only under an
exact declared const parameter; fixed-array and open-expression binders must
reconcile uniquely to the exact alpha-normalized telescope. Residual const
declarations and unrelated source-spelled leaves reject. The transport atom is
never itself package identity.
Generic proof arguments require complete structural identity. A concrete type
argument is a structural type identity, so compiler builtins use closed atoms
and authored nominals require exact package/toolchain-source ownership. A
machine argument remains an exact nominal declaration. Unresolved ownership is
diagnostic state inside ordinary compiler identity only: exact review type
projection returns no identity, and the canonical encoder rejects any unresolved
nominal that reaches it.
Public data projects its supply, generic shape, properties,
fields/variants/payloads, relevance, and stable numbered and retired identities.
Those numbered ordinary-data identities are
the wire contract; the retired standalone `wire data` form is not projected as
a duplicate API. Public data has a closed ordinary/quotient form. A quotient
row binds its exact carrier-family type identity and package-qualified public
relation after package review independently reruns formation. Its normalized
logical expression, binders, and dependencies determine mathematical identity;
the chosen equivalence proof does not redefine that identity. The proof's exact
assumptions and selected conformance remain admission evidence. General
mathematical binders and bundle rows must migrate with the source semantics;
unsupported forms fail closed rather than becoming partial canonical rows.
Public domain rows likewise retain exact declaring-package identity,
alpha-normalized type/const binders, carrier type, and index arguments.
Synthesized semantic paths retain an authored provenance span without replacing
their canonical spelling with the source substring. Transparent aliases
recursively flatten to sorted, deduplicated package-qualified atoms. Authored
toolchain nominals bind a canonical toolchain-relative source path plus exact
source-byte commitment in review evidence; this records their semantic origin
without treating the running executable pathname as review identity. Compiler
carry aliases
expand to closed `CarryPermission` atoms rather than invented nominal owners;
valid package declarations cannot enter that lane merely through a resembling
diagnostic path. Exact compiler/toolchain artifact custody remains separate
and applies only when that artifact is itself a bootstrap or deployment
subject.
Predicate-body presence and
currently representable structural expression/membership facts retain the
domain carrier and exact
package-qualified member/domain identities. A typed fact is admissible only
when it has exactly one checked definition row, one fact-keyed ownership
record, and exact checked dependency places for nested member paths. Public
domain semantic contributions are retained from the exact typed role record as
closed compiler-owned tags. Every retained role must name the declaration's own
typed semantic identity; canonical evidence stores the package-qualified domain
and role rather than the private semantic-domain ID or a name inference. Public
domain operators remain separate exact `PublicOperator` rows. Unsupported
callable forms continue to fail closed until their authority and exact rows are
settled.
Closed compiler-owned classifications and authorized establishment routes
retain exact route kind plus package-qualified trait and requirement identity;
alternative routes are canonically sorted and deduplicated.

Package-owned public traits retain exact identity, boundary status,
alpha-normalized lifetime/type/const binders, ordered package-qualified parent
applications, and ordered machine/operator requirement signatures. Requirement
rows retain lifetime arity, parameter names and modes, package-qualified
lifetime-sensitive signature types, and fixed operator spelling plus exact
declared service reach, installation-bound status,
synchronous invocations as exact non-`self` parameter ordinals or
package-qualified services, suspension, blocking, and termination. Progress
premises retain package-qualified public profile identity, receiver/non-`self`
parameter roots, and package-qualified field projections. Parent applications
retain exact alpha-normalized lifetime-binder arguments;
renaming a binder is stable while changing a borrow relationship changes
evidence. Generic conformance requirements retain an optional alpha-normalized
evidence-binder ordinal, exact subject ordinal, package-qualified public trait
identity, and structural type arguments. Binder-free `where T satisfies Trait`
does not fabricate evidence. Selected conformances retain exact
package-qualified declaration, complete alpha-normalized application,
instantiated subject, and underlying public-trait application; the semantic
declaration owns exact conformance, subject, and trait symbols. Public trait
requirements retain their complete
`requires` and `ensures` through structural fact/expression identities joined
to their checked owner. Bundle interfaces retain exact law subjects, witnesses,
and dependencies, not local diagnostic aliases. Abstract published crash
ceilings come from the checked capsule keyed by trait and requirement and
retain canonical causes and guards; they do not fabricate body sites or calls.
Unsupported arguments or expressions reject rather than producing partial
canonical rows. Trailing `boundary host` / `boundary Name` and trait
`invariant` clauses are retired rather than awaiting package rows.
Requirements also retain whether their checked declaration supplies a default
realization; implementation bodies remain checked source subject to universal
update triage rather than entering the evidence format as compiler-private IR.

Terminal Psi evidence remains a separate evidence class for checked
final-realization claims: Omega-emitted executable code, asserted properties of
native or externally supplied code, lowering- or ABI-bound guarantees, fixed
native resource claims, and hardened profiles that explicitly require
final-code replay. Opaque executable supply may remain an explicit trust/TCB
row making no Terminal claim. Ordinary package reach, authority, provider,
proof-status, and build-contract admission does not require complete Terminal
coverage. A row without Terminal evidence makes no Terminal claim; a generic
partial/completeness bit must not blur the distinction.

Every package-owned bodyless external realization, including a private
implementation leaf, therefore projects as a separate blocking
executable-supply trust row, not as callable API, reach, boundary
representation, accepted proof, or Terminal evidence. The row binds the exact
package-qualified callable and tagged requirement application—trait
conformance, operator overload coordinate, or top-level boundary-requirement
overload—to one closed mechanism: import library and symbol, syscall number,
compiler intrinsic, vtable slot, vtable field, or table-function field.
Projection cross-checks the machine supply mode, satisfies
binding, and external-binding table and rejects
missing, duplicate, mismatched, or unsupported state. It makes no claim that
the supplied executable was audited or that its implementation satisfies the
callable contract.

The compiler reads each component from the earliest coherent private
representation in which it is semantically settled. Structural external-
binding identity may come from pre-Terminal state and join the checked callable
and requirement identity only after successful compilation. This checker may move
with compiler internals; only the versioned canonical row is durable. Psi may
repeat the consistency invariant as a downstream backstop, but no package
format or public IR depends on it. Nominal Chi is unwarranted unless later work
discovers an independently useful semantic boundary; an existing coherent
stage such as Exact should be reused when it carries the same facts more
simply.

Package review v70/canonical row v28 implements this lane. Each external leaf
must have exactly one conformance application and a bodyless supply mode whose
binding, mechanism, conformance reference, and structural table identity agree.
Malformed import/syscall/vtable/table payloads and table fields without one
exact attached data declaration reject. The callable plus complete conformance
application is the row key and the structural binding is its value, so a
binding-only update changes one `OpaqueBlocking` supply row while leaving
callable API bytes stable. Private leaves receive the trust row without being
promoted into public callable rows. Canonical recovery, source accounting, and
conflict rendering carry the row; none of them asserts an audit or Terminal
verification.

Package review v72/canonical row v30 generalizes that same row's key from a
trait-only conformance to a tagged exact requirement: either the complete trait
conformance application or one existing package-qualified operator overload
coordinate. The first operator lane accepts bodyless external supply for a
public, named, nongeneric boundary operator. Public realization machines also
retain the coordinate in their callable row; private leaves remain absent from
public callable API while retaining their opaque supply row. Selected-provider
evidence remains separate and is cross-checked against the exact operator,
realization symbol, package, normalized machine identity, and structural
binding. Thus disclosure never implies selection. Compiler-known intrinsics
are the first executable mechanism; ordinary or private operators, aliases,
generic/lifetime applications, and fixed-token boundary operators reject.
Generic external boundary-operator projection is not deferred package-review
work. Provider planning admits selected boundary-operator execution only
through a checked adapter or a compiler-owned migrated intrinsic, and the
closed migrated-intrinsic catalog contains no generic operator. Generic
type/const operator uses instead produce D29 exact application demands; a
future generic intrinsic must first extend that compiler-owned execution
catalog and its application replay rather than pre-publishing an unusable
opaque-supply template.

Package review v71/canonical row v29 binds each supported checked ordinary
operator realization into its public callable value, whether the declaration
has a fixed token or only its named call surface. Checked lowering retains the exact
machine/operator symbols, conformance/admission form, normalized overload
shape plus exact lifetime-bearing type nodes, both complete canonical contract sets, and exact typed semantic
snapshots of their contract graphs in full. The compiler requires exact
equality with a fresh derivation, reruns signature-directed selection and the
equality/`&&` `requires`/`ensures` contract judgment, then records the selected
public, nongeneric, lifetime-free operator's existing package-qualified
overload coordinate. Post-check redirection and coordinated typed-contract
mutation both reject. Changing only a valid selected declaration changes only
the callable row. Private, generic/lifetime-parameterized, aliased, and bodyless
checked realizations reject. Operator-bound external supply uses the distinct
v72 trust-bearing association rather than borrowing a trait-conformance shape.
A fixed-token checked realization uses the same exact declaration
coordinate as its named call surface. Its public-operator row already owns the
closed compiler spelling, so the realization edge neither repeats that spelling
nor introduces another identity form. Checked-body boundary realizations use
the same edge for contract satisfaction. Active choice remains exclusively in
the existing selected-provider set, whose exact requirement and realizing-
machine declarations join back to the operator and callable rows. Projection
repeats that exact symbol, slot, checked-adapter binding, package, and machine
join. A positive named-boundary canary covers the unique-candidate route.
Package review v94/canonical row v52 first admitted the selected, unaliased,
nongeneric, lifetime-free checked-adapter realization of fixed-token binary
arithmetic/comparison and indexing declarations. Checked execution requires the
exact selected use, token, operand shape, compact plan coordinate, and strong
plan commitment before redirecting to the checked body. The later D29
type/const declaration and exact-application lanes reuse this same join for
supported local generic checked bodies. Range, unsupported arities, aliases,
bodyless/externalized realizations, lifetimes, and evidence drift remain fail-
closed. This is checked/package evidence only; it does not claim Terminal or
native realization. Authored selection across a same-path overloaded family
must use the atomic family rule above. Operators with outcome-specific
contracts reject until their refinement rules exist.

Package review v90/canonical row v48 admits checked operator crash refinement.
For each provider crash cause, the compiler substitutes operator parameters
with realization parameters and requires every provider route to be an exact
member of the operator route set. An unconditional operator route admits any
provider route for that cause; an unconditional provider route requires an
unconditional operator route. Omitted provider causes narrow the contract.
Undeclared causes and stronger routes reject. Ordinary checked-crash
validation still proves that the realization body stays within its own
published routes, and projection reruns the operator-refinement judgment
before retaining the existing complete operator, callable, and checked-crash
rows. This is deliberately structural containment, not an unimplemented
logical-implication prover.

Package review v91/canonical row v49 admits exact nominal-member selection from
a computed contract receiver when that receiver is already representable by
the closed structural expression vocabulary. Projection recursively retains
the receiver and requires one finalized public-interface member-token
selection joined to the typed member declaration; it derives case identity
from that selected declaration. Missing, duplicate, redirected, or typed-
symbol-mismatched custody rejects. This reuses the existing member row and does
not broaden admission to arbitrary computed or aggregate expressions.

Package review v92/canonical row v50 admits the compiler-owned collection-view
call family in public contracts: shared slice, mutable slice, text view, and
bytes. The structural call row retains the receiver and exact operation.
Projection requires one public-interface call selection and one retained
checked intrinsic fact, freshly rederives the operation from the final typed
receiver and checked owner environments, and requires all three identities to
agree. Package-authored lookalikes remain nominal. Missing, duplicate,
redirected, or stale call custody rejects; recovery remains v14. This does not
widen the call compositions accepted by checking; it retains views only in
public facts that already pass the compiler's denotational-call rules.

Package review v93/canonical row v51 admits a named public integer const in a
checked contract-call const slot. Resolution retains the authored path once as
an exact static-argument selection to the const declaration. Projection
rejoins that symbol to exactly one checked public const and decodes its closed
canonical value into the const-argument row. Review v99/canonical row v57 adds
Boolean values with a distinct atom and requires the exact compiler-owned
`bool` declaration carrier. A changed value changes review identity; private
declarations, missing or malformed canonical values, carrier disagreement,
other value families, and selection drift reject. Source names and diagnostic
displays are not value identity, and recovery remains v14.

Package review v102/canonical row v60 admits selected public named constants in
contract-call static slots when the carrier is an acyclic, monomorphic checked
record or pure sum recursively composed from exact integer/Boolean primitives,
literal fixed arrays, and the same checked-data cohort. Projection reruns the
compiler's syntax-free canonical-value validator against the exact resolved
declared carrier, requires exact authored static-argument selection custody,
and emits the carrier beside the display-free canonical encoding. Encoded
field, case, and type names are consistency claims under resolved carrier
replay, not type authority. Malformed encoding, embedded-name spoofing,
carrier substitution, private selection, top-level arrays, and broader generic
or recursive carriers reject. Canonical-row recovery remains v15.

Package review v113/canonical row v71 admits the atomic Mach-O dylib
install-name and dyld symbol locator case. Canonical-row recovery remains v15.

Package review v112/canonical row v70 admits conformance-bound weakening for
top-level external requirement supply. Every provider demand must match one
distinct requirement demand after alpha normalization; requirement-only
demands may be omitted because the provider asks no more of its caller.
Matching ignores each telescope's locally renumbered evidence-binder ordinal
while preserving binder presence, subject ordinal, public trait, selected
conformance application, lifetime arguments, and structural arguments exactly.
Added, changed, or duplicated provider demands reject. Both exact telescopes
remain in the opaque supply key, so weakening neither fabricates conformance
evidence nor grants selection, installation, or execution authority. Canonical-
row recovery remains v15.

Package review v111/canonical row v69 admits static-machine parameters in
external callable signatures. Structural contracts retain their complete
recursively alpha-normalized static/value signature, contracts, crash ceiling,
service and invocation envelope, suspension/blocking flags, and termination
guarantee; nominal contracts retain exact public trait and requirement
identity. Review reruns the compiler's binder-positional recursive refinement
before projecting the independently exact requirement and provider telescopes.
Post-check kind substitution and inapplicable type-property bounds reject.
Canonical-row recovery remains v15.

Package review v110/canonical row v68 retains each external callable's ordered
structural conformance-bound telescope. Top-level requirement supply
independently projects both telescopes and currently admits only exact
equality; a checked provider that weakens or otherwise changes the bounds
rejects until the compiler owns a real conformance-bound subsumption judgment.
Bound evidence names alpha-normalize to ordinals, while subject ordinals,
public trait identity, selected conformance applications, and structural
arguments remain exact key material. Canonical-row recovery remains v15.

Package review v109/canonical row v67 admits const parameters in external
realization and top-level requirement signatures. Each side retains the exact
const carrier; authored binder names alpha-normalize out of callable carrier
identity, while positional const use in fixed-array lengths is revalidated
against the exact requirement-to-provider binder relation. Post-check carrier
substitution and inapplicable type-property bounds reject. Static-machine
and conformance-bound external forms remain fail-closed.
Canonical-row recovery remains v15.

Package review v108/canonical row v66 replaces the external realization's
static-parameter count with an ordered structural telescope and retains the
independently projected top-level requirement signature in the same supply
key. Each currently admitted ordinary type parameter retains its exact
multiplicity and carry-property bounds on both sides. Bounded realizations are
revalidated against the requirement from retained typed custody; a provider
that gains a stronger bound rejects, while an accepted weakening still
preserves both exact telescopes and changes external-supply identity. Const,
machine and conformance-bound static forms remain fail-closed.
Canonical-row recovery remains v15.

Package review v107/canonical row v65 admits unselected external executable-
supply disclosure for a public top-level requirement and realization with
compiler-validated ordinary type parameters carrying default properties and
lifetime telescopes.
Review reruns the exact requirement-realization judgment from retained typed
custody, and all retained carriers alpha-normalize binder spelling. The opaque
blocking row grants no provider selection, installation, execution, or audit
claim. Selected generic provider plans, richer static telescopes, aliases, and
uncatalogued compiler-intrinsic execution remain fail-closed. Canonical-row
recovery remains v15.

Package review v106/canonical row v64 makes each external executable-supply key
self-contained for its callable shape: lifetime and supported static telescope
counts, exact value-parameter modes and types, and the exact return carrier.
Private external leaves therefore do not depend on an absent public callable
row for semantic identity. Richer static kinds, bounds, and conformance
telescopes remain fail-closed. Canonical-row recovery remains v15.

Package review v105/canonical row v63 admits selected public named scalar and
structured const values in the same exact closed-conformance contract-argument
lane. Checked PSI retains the exact parameter and declaration carriers,
display-free canonical value identity as one structural argument. Review then
independently rejoins the declaration and authored selection, replays the
encoding against the exact resolved carrier, and compares the complete
reclosed application with the exact checked call occurrence. Const names and
diagnostic display are not value identity.
Missing, malformed, private, substituted, or post-check-drifted values reject.
Canonical-row recovery remains v15.

Package review v104/canonical row v62 admits a caller const binder in the same
exact closed-conformance contract-argument lane. The selected argument must
rejoin the owning callable telescope and is encoded by alpha-normalized binder
ordinal. Projection still recloses the complete authored conformance
application and compares it with the exact checked call-occurrence row, so
renaming the binder preserves review identity while selecting another binder
changes it and retained-occurrence substitution rejects. Named or structured
const values, and lifetime or machine parameters remain
fail-closed in this lane. Canonical-row recovery remains v15.

Package review v103/canonical row v61 extends the checked closed-conformance
contract-argument lane to a declaration telescope containing exact type
arguments and parser-canonical integer-literal const arguments. The row retains
the consts in declaration order, so `FieldOrder<Card, 7>` and
`FieldOrder<Card, 8>` cannot alias. Forwarded const binders, named or structured
consts, and lifetime or machine parameters remain fail-closed.
The target-trait telescope remains type-only; const target arguments cannot be
relabelled as type identities. Canonical-row recovery remains v15.

Package review v101/canonical row v59 introduced the first checked non-data
nested static application in a contract-expression call: one lifetime-free,
type-only, public closed conformance supplied to an explicit conformance-binder
slot. The checked proof facts retain the exact owner, fact, call occurrence,
static ordinal, and complete `ClosedConformanceApplication`. Review requires
one matching occurrence row, recloses the authored application, compares the
complete checked structure, rejoins the exact public-interface conformance
selection retained from the authored call, and projects its package-qualified
declaration, ordered type arguments, instantiated subject, and target trait
application. In v101, both the conformance declaration telescope and
target-trait telescope had to contain only type parameters with exact arity; a
machine or const target-trait parameter rejected rather than
being mislabeled as a type identity. Missing, duplicate, redirected,
substituted,
or source-selection custody rejects. The compact
report fingerprint and commitment remain local joins, not portable package
identity. Evidence projections into executable calls and nested machine
applications remain rejected by the language;
review does not invent rows for them. Canonical-row recovery remains v15.

Package review v95/canonical row v53 extends the opaque external executable-
supply key with a third exact requirement tag for public top-level boundary
requirements. A supported leaf is bodyless, nongeneric, lifetime-free, and
payload-bearing. Projection retains the normalized requirement-overload
identity and independently rejoins the exact satisfies symbols, binding,
provider type, selected plan when present, and realization declaration.
Disclosure of an unselected leaf still implies no selection or audit.
Package review v97/canonical row v55 closes the first compiler-intrinsic catalog
identity for the exact selected Linux `Console::exit_process(i32) -> Unit`
requirement and realization. Its target-library declaration is a bodyless
target-scoped `boundary machine ... satisfies ...;`; the candidate compiler-
intrinsic binding is derived from exact accepted package/toolchain custody,
realization, signature, and retained selected-target origin, never an authored
payload-free `via`. A targetless Linux-host check may retain that inferred row
with an empty plan target, but cannot close its physical execution. Non-Linux,
wrong-origin, wrong-symbol, wrong-signature, legacy-authored-via, and sibling rows
remain physically closed; further intrinsic entries still require their own catalog
settlement. Canonical recovery remains v14. This is
physical execution identity, not the D39 semantic external-termination row:
Unit and a provider-known nonreturning realization do not establish successful
termination. That authority remains closed until one explicit checked
terminal-effect completion identity survives from the boundary contract through
Terminal and the selected target realization.

Package review v126/canonical row v84 adds the second distinct catalog
execution identity for exact selected Linux
`Console::write_byte(i32) -> Unit`. The same package/toolchain, declaration,
signature, conformance, selected-plan, and target-origin joins apply; the row
does not share the exit identity or its ProcessTermination authority. Native
artifact replay additionally binds one exact runtime scalar source and its
materialization/syscall span. Direct constants and exact preceding internal
Unit-call homes are the current bounded sources; incoming parameters remain
fenced until installation can replay their scalar ABI ledger. This closes the
bounded Linux physical implementation with terminal-authority policy v3
ProcessOutput permission, not a sibling Console row, hosted import, or wider
external-binding mechanism.

For a selected payload-bearing top-level external satisfier, provider planning
extracts the foreign calling row from the same exact selected plan. The ABI
join resolves one canonical top-level requirement and normalized overload; it
does not reinterpret the row as a trait. The declaration's semantic `self`
maps to the satisfier's first explicit carrier parameter and remains in the
foreign signature. Missing, duplicate, mismatched, or fingerprint-spoofed
custody rejects before publication. This closes executable-row extraction, not
installed invocation or component-era replay.

The association is a retained compiler-private
checked baseline, not a persisted package row, hash, or defense against a
trusted component rewriting typed state and checked facts; it is not a reason
for nominal Chi.

## Dependencies and the lock artifact

Code imports package-local aliases. `build.omg` records source requests and
update selectors. It does not assert the fetched package's name, capability
manifest, or resolved immutable identity. Fully qualified paths do not bypass
the declared alias/reach set.

Dependency-source projection is hermetic even when later build staging is not.
Source rows cannot depend on build-host observations, generated files, imported
code, or dependency build outputs. Resolution fetches a source, extracts its
own package declaration and dependency projection, and closes the source graph
before downloaded build code receives any provider.

The lock records the project's selected and accepted state:

- source-qualified package names/keys, the root's explicit role, and exact
  immutable commit/tree/content and workspace-member selections;
- source requests and requester-local dependency/alias edges;
- the normalized accepted capability, public API, and explicit assumption
  baseline, scoped to each actually reviewed target where necessary; and
- the project's explicit acceptance decisions.

Use a bounded, deterministic, diffable encoding with explicit format and
baseline schema versions. Full normalized baselines are required for comparison;
a fingerprint alone cannot explain a change. Cache paths, proof certificates,
native artifacts, build replay transcripts, audit receipts, and compiler-private
handles are not part of this basic payload.

The compiler projects complete normalized policy without compiler-private handles,
proof certificates, or replay transcripts. The manager joins that policy to
exact source pins and historical project decisions. Component details live
with [policy evidence](../../../omega-rust/omega/packages/review/evidence/POLICY_BASELINE.md);
the [lock owner](../../../omega-rust/omega/packages/manager/src/lock/README.md)
defines framing, decision history, version handling, and resource limits.
Loading these records requires neither an old checkout nor a compiler run.

The project normally commits the lock and trusts whoever lands its changes.
Recovery validates format and graph consistency, checks acquired sources against
pins, and treats recorded acceptance as project policy. It does not reconstruct
a certificate proving that the lock deserves trust. A mismatch between current
analysis and the recorded baseline is reported; it never disappears because
the lock says the package is accepted.

The root `omega.lock` and `omega.admissions` of a mutable local package are
project control state, not source input. Local capture excludes them, including
ASCII case variants, so publishing policy does not change the source identity
recorded inside the lock. Symlinks into those excluded root control files reject.
Nested policy-named files
remain ordinary source; exact materialized repository trees retain their full
content, including lock files. The manager still loads the accepted lock
separately and compares its policy with fresh compiler findings. This source
exclusion is not permission to ignore edits to the accepted baseline.

Locked compilation never silently re-resolves a mutable selector. Missing cached
source may be acquired at the exact pin when network use is allowed; offline
failure leaves the graph unchanged. Unsupported lock formats fail with recovery
guidance. Recomputing stale compiler analysis does not silently upgrade pins or
claim that a new audit occurred. If baseline meaning cannot be recovered,
require fresh comparison/decisions rather than treating unknown data as empty.

The codec and install/update transaction can be implemented for the complete
supported source-package surface now. There is no prerequisite for a
certificate-bearing `PackageInstance`, every native artifact class, or a
generic evidence-promotion framework.

Projection is target-independent: it extracts one complete flat unconditional
dependency-request set from each fetched package without fetching that
package's dependencies. A dependency's own request set remains unknown until
its source is resolved. The resulting immutable source graph is the same for
every target; target identity scopes the exact compiler invocation, review, and
evidence subject rather than selecting dependency edges.

One workspace lock may share that immutable source closure across independently
accepted evidence sections for exact caller-requested targets. It contains no
support-profile columns and makes no claim to have discovered or checked
"all" targets. In locked mode, missing evidence for a requested target fails
without network access; one explicit multi-target operation may populate the
caller-supplied set by sharing target-independent stages and forming each exact
target child at target-sensitive stages. A retained section for another target
grants no authority to the current child. Git edges cannot be "resolved but not
fetched": verifying a commit, tree, workspace declaration, and selected member
already exercises host-routed acquisition and resolver-owned source validation.

The maintained compiler applies the same boundary to native realization. Each
child first forms its own canonical Terminal artifact. It may share the
target-neutral verified abstract input only when the complete Terminal artifact
identity, exact proof-admission profile, and optimized/unoptimized entrance are
equal. Target, entry and calling plans, provider and foreign settlements,
authority policy, physical evidence, and machine/image lowering remain local to
the child. The sharing is merely compiler work reuse and creates no additional
package, audit, or security claim.

Target values are checked against the trusted toolchain target catalog at each
exact child boundary. The catalog validates supplied identities; it does not
provide an `all` deployment set because it may also contain abstract and local
modes. Package source does not declare a support matrix or a dependency
condition schema. Locks retain each target's semantic identity, never a string,
ordinal, or temporary Rust enum case.

The first implementation performs no semantic-version solving. Requests for
one `PackageKey` that resolve to one immutable source instance deduplicate even
when their authored selectors differ. Different immutable resolutions fail with
every conflicting dependency path; there is no undefined "compatible request"
relation. Multiple simultaneous instances per key are unsupported. Supporting
them would require nominal types, conformances, provider selections, and evidence
rows to use package-instance identity, not merely another alias.

Selective candidate resolution preserves accepted pins for unchanged Git
locator/revision requests. Alias changes and selecting another member do not
refresh the repository. New or changed requests resolve normally; they must
still reconcile with the complete graph. Selecting an accepted package for
update refreshes its Git repository lineage, not unrelated transitive
repositories. Workspace members share one immutable repository revision, so
their reachable members and relative Path edges move together and receive the
same complete-graph review. This does not add unused workspace members.

`GitDependencyPins` borrows the accepted source subject and exact selected keys,
rejecting unknown or duplicate selections before acquisition. Empty selection
preserves existing requests during installation; ordinary unpinned resolution
updates all selectors. A missing preserved revision either fails offline or is
fetched at the exact recorded commit, according to caller options. That option
controls preserved pins only, not network access for explicitly refreshed or new
requests. Freshly acquired content must match the recorded resolution. There
is no fallback from an unavailable old pin to a newer selector result.

`GitResolutionOptions::offline` is the separate whole-invocation restriction:
it overrides preserved-pin fetch permission and rejects every new or refreshed
Git request, including requests discovered transitively. The command's
`--offline` uses this policy and exact-pin offline recovery for locked
compilation, inspection, and install/update resume. Local roots remain editable
and local-only graphs need no lock. Resume uses the proposal's exact pins, not
the current branch tip. Historical-source diagnostics also obey the option;
an unavailable old checkout does not prevent comparison against lock policy.
The flag changes no lock/proposal schema and does not disable compilation,
scoped build outputs, project decisions, or pending-publication recovery.

## Package reach boundary

The package is the dependency-reach boundary:

- `pub` says what the package offers;
- `build.omg` says what packages/services it may reach;
- undeclared aliases are not nameable; and
- a subsystem requiring a meaningfully different reach set is a separate
  package rather than a hidden nested manifest.

Direct dependency rows authorize authored selection of declarations owned by
the named package; they are not required merely because that package's nominal
type flows through an already-declared dependency's API. Such a value may be
moved, borrowed, stored, returned, passed back through the declaring surface,
and checked for multiplicity without granting access to its owner's methods,
fields, cases, operators, conformances, or ordinary explicit consuming
machines. The reserved owner-attached `T::drop` hook is compiler-only and
authored selection rejects. An authored `omega::core::drop(value)` is instead
an ordinary consuming call; the concrete cleanup plan it triggers remains
carried semantics. Compiler-planned layout and automatic cleanup are carried
type semantics rather than authored declaration selection.

The transitive closure still retains the type owner's exact package instance.
Artifact dependency evidence retains the exact foreign declaration when the
checked program uses it: private flow affects rebuild/content identity, while a
public-signature occurrence also affects semantic API compatibility. A coarse
whole-package edge is a sound over-rejecting implementation until exact
declaration edges land. `pub` exposes only declarations owned by the current
package; there is no `export` item that relabels a dependency declaration.

Implementation uses a package-agnostic authored-selection ledger captured while
resolution still owns exact source spans and public/private syntactic position.
It is finalized only after successful checking, where late-bound receiver
calls, overloads, operators, and inferred conformances have exact selected
declarations. Static rows may be complete earlier; each row is joined from the
earliest coherent owner of its facts. Missing, ambiguous, or unjoinable rows
reject. The ledger is a compiler-internal sidecar and does not justify nominal
Chi.

Package-aware preliminary and final checking may retain a late-bound row only
when its exact retained source origin is compiler-owned `Toolchain`. Those rows
remain TCB input: package declaration admission skips them, package evidence
does not attribute them to the requesting package, and every late-bound row
from ordinary package source still rejects before build authority or evidence
is issued. This exception does not bypass declaration visibility. A package
that resolves a private toolchain declaration still rejects and the intended
core API must be made explicitly public.

Expression custody follows the declaration that publishes it. Public machine
contracts, public data/domain predicates, and public trait contracts use
public-interface exposure; executable state/body expressions and
`terminates by` ranking witnesses remain private. The public termination
promise is `terminates`, not the measure used to prove it. Membership facts
retain their selected domain path as a declaration row, while their
parameter/local value roots remain lexical places. Independently nameable
declarations own ordinary `pub`, including carrier-qualified declarations;
only genuine members with one exact semantic owner inherit visibility. A
public-interface selection of a private declaration rejects.

Generic conformance bounds follow the same authored-authority rule. A subject
parameter and optional evidence binder are lexical. The right-hand trait is an
exact trait selection; a qualified `Carrier::Evidence` bound selects both the
carrier declaration and the package-scoped conformance. Machine and trait
bounds inherit their enclosing declaration's exposure. This does not decide
whether every selected declaration family is independently publishable.

Trait composition is likewise authored authority. Header parents and body
`requires` clauses normalize to one semantic edge, while every source-backed
edge retains the exact resolved trait as a type-reference selection under the
enclosing trait's public/private exposure. The direct-dependency gate consumes
that row; the separate `trait_parent` source coordinate explains where the edge
was authored but grants no authority by itself.

An attached declaration head such as `machine Data::operation` independently
selects the exact carrier declaration. The compiler retains that coordinate as
a type-reference row under the machine's interface exposure, including
exported boundary supply even without `pub`. Qualification neither relabels
the carrier nor admits an owner available only transitively.

Quotient declarations retain every authored formation coordinate: carrier,
right-hand relation path, repeated static-`where` relation subject, sealed
`Equivalence` trait and arguments, and named proof conformance. Relation and
trait rows inherit the data declaration's exposure. Only the selected proof
conformance is private formation custody and absent from quotient API identity;
it remains subject to ordinary visibility and direct-dependency admission.

The direct-dependency gate consumes only finalized authored-selection rows.
Checked carried nominals, automatic cleanup, layout, and move/copy facts feed a
separate exact semantic-dependency set with private/public disposition. They
affect artifact and compatibility identity without widening source
nameability. Compilation must admit the selections applicable at a stage before
executing selected package or build-time code. Final execution consumes the
finalized ledger. Earlier effect-free execution consumes exact early targets or
fails closed unless the complete compiler-derived candidate set is confined to
admitted owners; an implementation whose current order cannot establish that
must reorder or split the work rather than weaken the gate.

The initial exact carrier is a checked-flow sidecar assembled only after
checking succeeds. It derives machine-head and exact checked call-result types,
joins ownership-place types, promotes public-interface exposure, and retains an
automatic cleanup machine only when its exact attached nominal declaration
matches. A same-spelled cleanup attached elsewhere cannot satisfy that edge.
For an owned erased value, the compiler-built descriptor carries the same exact
movement and cleanup plan with payload custody; this is lifecycle metadata, not
trait evidence or a source selection. Borrowed erased views never acquire
cleanup ownership for their referents.
This sidecar is package-neutral and compiler-private. The compiler's review
projector qualifies its consumer and dependency declarations by exact package,
emits versioned blocking rows for each dependency kind and exposure, and keeps
exact source anchors for both sides. It is comparison evidence, not an accepted
lock schema or a reason for nominal Chi; total admission coverage remains a
separate requirement.

Earlier effect-free compiler evaluation uses that split directly. Const-
generic calls, fixed-array const calls, const-domain facts, laid/placed layout
policies, wire policies, and calling policies retain exact invocation custody
and consult a package-neutral authority backed by the reconciled direct graph
before executing. The authority walks each concrete build-time call closure and
checks both caller-to-callee edges and the reachable bodies' authored selection
occurrences. A shared policy is reusable only after every authored application
site is admitted. Exact resolved symbols are authoritative; facts awaiting
later checked resolution fail closed unless the compiler can prove the whole
candidate set remains within toolchain, self, or direct dependencies. Operator
fallback uses the checked layer's conservative intrinsic judgment, never
spelling. Const value substitution retains only a declaration-provenance symbol
and occurrence, preserving package custody without carrying const semantics
into typed or runtime trees.

These checks belong at the earliest coherent private typed/probe representation
owned by the compiler. Coupling to that non-public representation is acceptable
because checker and representation evolve together. This does not introduce
nominal Chi; such a stage would require an actual semantic boundary rather than
the desire for a stable report shape.

Explicit nominal type selections are retained during symbol-resolved-to-typed
lowering. At that point the selected symbol is exact and the enclosing
declaration still determines whether the occurrence belongs to a public
interface or private implementation. Public data, domain, machine-head,
trait, and wire type positions are public; private declarations, internal state
signatures, local type annotations, casts, and public-machine owned storage are private. Generic bases
and named dynamic-trait conformances retain exact custody. Primitive types,
binders, locals, and source-free compiler nodes do not acquire fictional package
provenance. These authored rows enforce direct source authority; they do not
replace the separate carried-type dependency rows needed for artifact and API
compatibility identity.

Conformance custody follows the same earliest-coherent-owner rule. Resolution
records each exact source-backed static conformance argument at its own token.
Checked operator selection records the exact trait conformance selected for an
authored operator token. When an unbound generic conformance requirement is
satisfied by exactly one declaration, specialization validation retains that
declaration as an inferred conformance argument and fingerprints its package-
qualified identity; checked finalization attaches it to the authored call
occurrence. Explicit evidence arguments and inferred selections remain separate
subsets so an explicit argument is not fabricated again at the call token.
Package admission therefore rejects an inferred transitive-only conformance
even when the caller never names the conformance alias.

The same custody applies to statement calls whose result is Unit or explicitly
discarded. The resolver records the source call before rebuilding statement
trees into tables, preserving exact targets and explicit static conformance
arguments. If receiver typing or specialization must settle the target later,
the checked flow call finalizes that exact source obligation and contributes any
uniquely inferred conformance. Compiler-owned build markers and lowered
assembly operations use closed intrinsic ledger variants rather than synthetic
package declarations. A statement call to ordinary package code remains an
authored selection and requires direct authority.

Static call arguments do not form a separate dependency loophole. Every
source-backed declaration path is retained recursively at its own span.
Conformance paths keep their evidence-specific kind; type, static-machine, and
forwarded-binder paths use one static-argument kind because their exact symbol
and the selected callable telescope carry the category. Integer literals name
no declaration. A named const static argument retains the exact const
declaration in that static-argument row and rejoins its compiler-canonical
value. Ordinary expression-level named const reduction preserves the exact
declaration in its substitution-provenance row. An unresolved static path
remains a late obligation and package admission fails closed if no exact
declaration is available.

The same authoritative build surface owns concrete channel/store compatibility
demands. `builder.require_wire_compatibility<Edge, Lineage, Local, Peer, ...>();`
requests only the directional wire facts named after the first four type
arguments. The compiler evaluates those requests against published schema,
codec, unknown-member, canonicalization, and `FormatMigration` evidence; it
reports every fact and rejects unmet requested facts. This is edge/deployment
policy, not intrinsic version metadata on `Local` or `Peer`.

Packages normally compose statically and may optimize across package edges.
They are not ABI or replacement boundaries merely because they are packages.
A build may select a provider realization for independent deployment; the
component is that realization plus its compiler-validated owned closure.

An initial composition also authorizes the runtime replacement envelope for an
independent slot: permitted imports and authority, compatibility and
observation profile, target semantics, resource ceilings, accepted execution
modalities, admission policy, and continuity constraints. A candidate that
fits this frozen envelope may be accepted by the runtime verifier without
re-running the entire build program. A candidate that widens it requires a new
owner-controlled build/composition transaction. Provider code and downloaded
artifacts never authorize their own widening.

The first implementation may accept only closures coinciding with one package.
That is an implementation restriction, not the semantic definition of
component. A concrete-machine call crossing a selected replaceable closure
rejects; a replaceable crossing names an ordinary requirement. The same
requirement may be statically selected and inlined in another build. No
hot-swap call syntax or `slot` keyword is implied.

## Target-dependent public identity

A target-neutral package may export a symbolic constant or type application
that depends on the sealed target-semantic capsule or on one selected target
realization. Its public manifest retains the exact observation and realization
applications rather than only the eventual folded value. Independently closed
artifacts compose only when those applications are compatible.

Adding, removing, or changing such a dependency in a public signature is a
breaking semantic-API revision even when every currently supported target
happens to produce the same scalar. A dependency confined to a private body or
plan changes content/target artifact identity and forces rebuilding or relinking
without changing the public contract. Compatibility diagnostics retain the
producer and consumer closures plus an origin chain through aliases, constants,
generic applications, and selected plans.

Target selection chooses exact declarations and realizations; it never splices
fields or cases into an existing nominal type. Different native field sets use
different ABI-specific nominal schemas behind one stable portable requirement.
Different sizes, offsets, padding, and alignment of one stable schema remain
ordinary target-dependent layout-plan facts.

## Authority evidence and admission

Runtime authority uses ordinary data layout plus domain evidence. The eventual
compiler-issued package-admission artifact must derive evidence from the
fetched, checked candidate; callers and packages cannot author or patch it. It
must record each owner-authorized boundary establishment, checked resource
transformation, provider/backing requirement, admitted claim, and reachable
authority. Public data shape and domain trust policy enter contract/component
compatibility identity; private implementation bodies and proof evidence affect
content identity while remaining outside public contract identity.

For every public callable and the build machine, evidence retains both the
declared service-reach ceiling and, when an actual checked body exists, the
exact inferred transitive reach plus its preselection concrete-transitive base.
The concrete base excludes authority contributed only by unresolved
installation-selected upper bounds and is not final-provider evidence. Bodyless boundary, accepted, requirement, and
external supply instead retain an explicit no-checked-body disposition; their
published ceilings are not relabeled as realized facts. An
impossible combination rejects: checked supply requires a retained body, while
accepted, requirement, and external supply forbid one; boundary supply permits
either an adapter body or a bodyless declaration. An
underdeclared implementation rejects. An overdeclared ceiling remains visible
as contract slack. Compiler-classified dangerous slack emits a separate
audit-recommended row keyed by exact callable and service identity, with both
source coordinates; bodyless supply and package-authored lookalikes emit none.
A later transition
from unused to used authority changes realized evidence even when the public
ceiling is unchanged. Capability-flow, provider, trust, proof, installation,
operational, and executable-TCB rows must retain exact package-qualified
provenance. Provider-plan and provider-trust rows now retain package identity
for the realizing machine, provider type, selected service schema, and
requirement owner, but binding/selection and remaining artifact joins are
unfinished. Risk classes must come from compiler-owned metadata on admitted
nominal identities, never from package-controlled names.

The same callable row retains the exact checked-body, boundary, or accepted
supply tier and canonical entry signature. A bodyless boundary guarantee is an
explicit trust-bearing accepted claim; a claim-free boundary symbol is not.
The compiler also emits one distinct blocking accepted-claim row carrying that
callable's complete published envelope and exact declaration provenance.
Initial or newly introduced trust requires exact root-policy resolution;
unchanged accepted evidence does not become a recurring blanket prompt.

The older standalone trust-lock lane cannot supply package claim admission.
Domain names and unmatched strings are rejected rather than converted into FNV
receipts or bare accepted-fact rows, and domains are absent from trust reports.
Exact selected-provider grants remain valid. Exact accepted-machine grants are
retained only as temporary standalone compatibility; package-aware compilation
rejects them because admitting one selector is not admitting the complete exact
accepted-claim inventory.
Review v89 and canonical row v47 retain each authored selected-provider grant
on the exact selected provider row as its selector kind plus the
collision-resistant `ProviderPlanDigest`. The compiler carries the selecting
build-machine symbol and exact `build.omg` source span from typed build
evaluation through provider settlement; projection rejects a missing, foreign,
or orphan grant and records that span as `ProviderGrant` explanatory custody.
The selector string is not persisted as authority: plan-name grants rejoin the
retained exact plan, slot grants rejoin its exact schema declaration, and both
bind the same complete selected plan and strong digest. Canonical-row recovery
v14 adds the source role.
The signature includes lifetime arity, alpha-normalized type/const/static-
machine binders,
ordered parameter names and modes,
package-qualified lifetime-sensitive parameter types, and result type. This is
contract evidence, not merely ABI layout. Binder renames are stable, while a
changed generic bound, parameter/result type, mode, or borrow relationship
changes evidence. Review v43 and canonical row v3 retain a structural static-
machine contract's recursively alpha-normalized telescope, complete value
signature, proof/crash contracts, reach, invocation, suspension, blocking, and
termination envelope. A nominal contract retains its exact public trait and
requirement identities. Checked proof/crash rows are keyed to each structural
binder, including nested binders; missing custody, excessive nesting, and
private nominal contracts reject. General mathematical arguments in selected
conformance applications remain fail-closed. Selected type, const, and machine arguments use the same
categorized static-argument vocabulary.
Review v44 and canonical row v4 extend contract-call rows with static-machine
arguments. Each retains either the exact caller machine-binder ordinal or the
exact concrete machine entry identity.
Review v45 and canonical row v5 rejoin each contract call to exactly one
selected callee static telescope. Supported arguments retain their category as
a direct concrete type identity, parser-canonical integer const literal, caller
machine-binder ordinal, or exact concrete machine entry identity. Nested static
applications, forwarded or symbolic type/const binders, general mathematical
static arguments, quotient calls, compiler intrinsics, and malformed or
ambiguous joins remain fail-closed.
Review v46 and canonical row v6 add bounded recursive generic data-type static
arguments in contract calls. Each application base rejoins exactly one checked
data declaration, whose telescope is recursively classified; changing a nested
type changes canonical evidence. This rung admits zero-lifetime generic data
applications only. Lifetime-bearing applications, generic machine/conformance
applications, unresolved forwarded type/const binders, general mathematical
static arguments, quotient calls, and compiler intrinsics remain fail-closed.
Review v47 and canonical row v7 admit lifetime-bearing recursive generic data
static arguments in contract calls after an exact data-declaration lifetime-
arity join. Lifetime arguments retain alpha-normalized caller lifetime-binder
ordinals: renames are stable, while selecting a different lifetime changes
canonical evidence. Generic machine/conformance applications, unresolved
forwarded type/const binders, general mathematical static arguments, quotient
calls, and compiler intrinsics remain fail-closed.
Review v48 and canonical row v8 admit contract-call forwarding of caller type
and const binders. Each argument is validated against the exact caller and
selected-callee telescope categories and encoded by its alpha-normalized caller
static-telescope ordinal: binder renames are stable, while selecting a different
binder changes canonical evidence. The frontend now resolves const-parameter
carrier types on machines and traits. Symbolic const declarations or
expressions, general mathematical static arguments, true nested
machine/conformance applications, quotient calls, and compiler intrinsics
remain fail-closed.
General mathematical predicate parameters and contract expressions need exact
binder and subject identity through source checking and review. Renaming a
bound variable is not a semantic change; changing the selected subject or
relation is. The contract/bundle migration must define and recheck these rows
rather than reusing a presentation label as proof or inventing a partial
interface for an unsupported binder.
Review v51 and canonical row v11 admit the four compiler-owned byte-sequence
predicate calls in public contract facts. The checked authored-selection row
now retains the exact closed predicate instead of one undifferentiated
intrinsic tag; projection cross-checks that identity against an unresolved,
receiver-free call before encoding it. Changing the predicate changes
canonical evidence, while a package declaration with the same spelling remains
an ordinary package-qualified callable.
Review v77 and canonical row v35 admit compiler-installed builtin-function calls
in public contract expressions. The projector rejoins the exact checked call
selection to the same fixed root slot and symbol kind, and encodes the stable
closed builtin ordinal rather than its spelling. Same-spelled late-root, nested,
package-authored, and generated symbols remain non-builtin; static arguments or
target-symbol custody disagreement reject.
Review v78 and canonical row v36 extend that closed identity to selected
boundary-operator provider execution. Compiler settlement retains the exact
builtin function beside, but distinct from, the authored realization machine;
projection rederives it from the checked overload and fixed builtin root slot.
Missing, mismatched, or non-intrinsic spoofed state rejects. Primitive-expression
intrinsics remain fail-closed until they receive their own closed atoms.
Review v79 and canonical row v37 add the first such primitive-expression atom:
named-float negation retains the exact checked `f32` or `f64` format. The atom
is selected by compiler dispatch from the exact checked boundary overload and
external realization join, never parsed from the authored realization-machine
name; that machine remains a separate package-qualified nominal. Projection
rederives and cross-checks the atom, while absent, cross-format, non-intrinsic,
and otherwise spoofed state rejects.
Review v80 and canonical row v38 close named-float conversion as one atom
containing the exact checked numeric source type, numeric target type, and
arithmetic domain. This distinguishes float-width conversion from every
float-to-integer width and signedness and distinguishes `Exact`, `Saturating`,
and `Trapping` integer results. The compiler derives all three coordinates from
the exact checked overload; changing or omitting any coordinate rejects during
review reconciliation rather than degrading to an authored name.
Review v82 and canonical row v40 retain the v81 primitive float binary atoms
and add exact atomic boundary-operator family-selection evidence. Each family
row binds the exact package-owned family path, nominal provider, selected
target, selection authority, complete-declaration coverage, and canonical
coordinate-to-plan mapping. Projection rejoins every coordinate against the
selected plan and its retained declaration provenance; absent, duplicate,
cross-family, or provider-drifting mappings reject. Generic/exact-application
coverage remains fail-closed until it receives a distinct compiler-owned
carrier.
Review v83 and canonical row v41 admit width-landed float literals in public
contract expressions, including logical expressions whose comparison
operand is a typed named parameter and callable contracts whose comparison root
is `result`. Result landing comes from the exact return type of the owning
state, operator, or trait requirement. The row contains the checked `f32` or
`f64` format and its exact IEEE bits; decimal source spelling never enters
identity. Equivalent spellings therefore compare equally, while format or bit
changes alter the package contract. A float literal that reaches review without
one exact checked width landing remains fail-closed.
Review v84 and canonical row v42 admit explicit denotational reference
formation in public contract expressions. The row retains shared, mutable, or
write-only access plus the recursively projected target; runtime loan identity,
lifetime spelling, and diagnostic text remain absent. Proposition applications
recheck an explicit reference argument against the exact declared parameter
type before projection, so access or referee disagreement introduced after
checking rejects. Omega's implicit shared lending remains a plain argument
whose receiving parameter already carries shared-reference identity; review
does not invent syntax the typed expression does not contain. Operator-law
conformance and package rederivation compare reference access as well as the
borrowed target, preventing shared/mutable drift from satisfying the same law.
At the same v84/row-v42 schema, named operator calls in public contracts reuse
the structural call row. Projection invokes typed Psi's exact named-operator
resolver, rejoins its symbol with the authored call-selection occurrence, and
encodes the package-qualified operator target. A static namespace such as
`Token` in `Token::ordered(left, right)` is path qualification, not a value
receiver. Target drift and explicit reference arguments inconsistent with the
selected callable telescope reject. No new canonical discriminant is needed.
Review v85 and canonical row v43 admit exact atomic-load expressions in public
contracts. The row binds the recursively projected loaded value and one closed
load-valid ordering: `NoOrdering`, `Receive`, or `GlobalOrder`. Projection
requires an invalid result handle because loads have no secondary result
carrier. Store, read-modify-write, swap, compare-exchange, publish-bearing load
ordering, missing value, and post-check result-carrier drift reject rather than
being generalized into a package claim.
Review v75 and canonical row v33 likewise admit the compiler-owned collection-
length projection in public contract expressions. Checked proof-static member
resolution derives the receiver type from its retained declaration symbol,
prefers an actual package field, and selects `CollectionLength` only for `len`
on a fixed array or slice. Projection requires that exact public-interface
selection occurrence and encodes the structural receiver without inventing a
package owner. A package field named `len` remains nominal. Other compiler
intrinsics remain fail-closed.
Authored `!` and `~` likewise retain the exact operator token through checked
selection custody, including when nested in a public contract expression.
Review requires that public-interface occurrence to finalize as the closed
builtin-operator meaning before projecting the existing structural unary
operator. That custody-only change did not alter the then-current v76/row v34
bytes; it closes a source-custody join rather than adding a semantic
discriminant.
Review v61 and canonical row v19 admit exact raw byte-sequence literals in
public contract expressions. The projector uses typed Psi's decoded octets
directly and assigns them no text encoding. Escape-equivalent source spellings
therefore have identical canonical identity, while changing any octet changes
the reviewed contract. At that revision aggregate and advanced call forms
remained fail-closed; v67 and v68 below close every typed aggregate-literal
node through ordered arrays and exact nominal record/case constructors.
Review v62 and canonical row v20 admit inherited requirement surfaces for
lifetime-generic public conformances when the selected trait has no lifetime
telescope of its own. Requirement rows apply the complete inherited type
substitution before deriving alpha-normalized lifetime topology. Renaming
binders or changing private realization bodies is stable; selecting another
lifetime ordinal changes canonical identity. Review v86 and canonical row v44
extend that identity to lifetime-parameterized target traits. The conformance
header supplies every target-trait lifetime explicitly;
each resolves to an alpha-normalized declaration-order ordinal in the
conformance telescope and is retained beside the target type arguments through
checked closure, inherited requirement substitution, public review, and
canonical encoding. Binder renames are stable and another ordinal is a different
public conformance. Package review consumes the already-resolved mapping and
never repeats application-site inference.

D55 extends the same explicit source application and checking judgment to one
machine's exact `satisfies Trait<...>::requirement` edge, but deliberately does
not reuse the conformance's public identity normalization. The compiler retains
raw ordinals into the realizing machine telescope for signature and contract
substitution. The reviewed edge instead first-occurrence-normalizes those
ordinals in target-trait parameter order, publishing only the equality partition
among trait lifetimes. Reordered, renamed, or newly inserted unused realizer
binders are therefore stable, while `[0,0]` and `[0,1]` remain distinct borrow
contracts. Checked and external supply share this edge identity; opaque supply
does not become proof of the external implementation.
Review v87 and canonical row v45 introduced a selected boundary-operator
family application field using a non-generic atom or normalized arity/string
applications. D35 retires that field in review v97 / row v55 because its
provider assertion was not independently reconstructible coverage. Current-
version recovery rejects the older vocabulary; no current compatibility row or
legacy parser reconstructs it. Future application evidence must consume D29's
compiler-derived tagged type/const demand and role-specific realization
commitment rather than reinterpret the v87 row.
Review v63 and canonical row v21 admit selected generic-conformance
applications in public generic bounds. The row retains the exact
package-qualified conformance declaration, alpha-normalized lifetime
arguments, categorized type/const/machine arguments, instantiated subject, and
the exact public trait with its instantiated type arguments. Checked closure
first validates the complete declaration telescope; review independently
rejoins its semantic declarations and never uses display strings as identity.
Binder renames are stable, while changing any selected application argument
changes canonical evidence. General mathematical arguments and non-public
selections remain fail-closed.
Review v64 and canonical row v22 admit the proof-only representation
observation `zero_value<T>()` in public contracts. Canonical identity retains
the exact package-qualified, alpha-normalized observed type rather than layout
bytes, source spelling, or a checker verdict. Logical type binders
receive exact symbols before typed lowering; binder renames are stable, while a
different observed type changes the row. Quotient targets remain rejected by
the settled representation-observer fence before package review.
Expression-owned type positions retain their exact authored public/private
disposition through symbol resolution. Cast targets, cast domain indices, and
`zero_value<T>()` therefore lower nominal selections under their real contract
position rather than a private default; proof-expression casts resolve those
types through the same exact symbol path as machine casts. Checked visibility
and direct-dependency admission reject private or transitive-only targets.
Outcome-specific `ensures` must not collapse into unconditional postconditions.
Each row retains the exact result-data and case identities and canonical fact,
joined to the checked guarded-guarantee owner. Missing, duplicate, or mismatched
custody rejects. Source row ordering is irrelevant; moving a fact to another
case changes its meaning. The contract/bundle migration preserves these rules.
Review v66 and canonical row v24 admit public-operator crash ceilings. Checked
lowering issues one exact operator-symbol-keyed crash-contract row for every
root and domain-homed operator, including crash-free declarations, and package
review requires the retained table to equal compiler rederivation. Cause
buckets retain unconditional truth or canonical structural guard expressions;
the latter use package-qualified call, member, and selected-overload identity
rather than textual runtime-predicate fallbacks. Clause/guard ordering and
duplicates are stable; a changed cause or guard changes the operator row.
Review v67 and canonical row v25 admit ordered array literals in public
contracts. Elements recursively use the existing structural contract
vocabulary and depth/byte limits. Nested arrays are therefore exact and
ordered; changing or reordering an element changes identity, while an
unsupported child rejects the complete row rather than becoming opaque.
Review v68 and canonical row v26 admit nominal record and sum-case constructor
expressions. Canonical identity retains the exact package-qualified data,
optional selected case, every exact field, and recursively projected values.
Field order follows exact semantic field identity rather than source order.
Changed cases/fields/values change the row; unresolved or mismatched symbols
reject, while private constructor selection is rejected by the existing public-
interface gate before projection.
Review v69 and canonical row v27 admit indexed and ranged contract expressions.
`[` retains an authored operator span and must join exactly one checked public-
interface `Index` or `Range` selection. Canonical identity carries builtin or
declared operator meaning, collection, scalar index or optional endpoints, and
inclusive-end semantics. Changing any retained field changes the row; missing,
ambiguous, or mismatched checked custody rejects. The same recursive projector
allows indexed expressions inside arrays.
Without changing the v69/row-v27 bytes, checked proof custody now includes one
exact `OperatorDeclaration` owner row for every non-crash public-operator
`requires`/`ensures` fact. Projection rejoins that declaration symbol, kind,
and fact exactly; missing, duplicate, or mismatched rows reject. Operator
contracts remain Omega's existing unnamed surface—this adds neither binding
syntax nor evidence lanes.
Review v53 and canonical row v13 add blocking standalone public-const shape.
Every package-owned public const contributes its exact package-qualified
declaration identity, exact typed declared-type identity, and canonical
structural declaration value even when unused. Neither source initializer text
nor runtime storage identity substitutes for that semantic subject. A public
const whose type exposes private data, or whose value lacks supported canonical
identity, rejects closed. Declared-type or value changes become source-backed
`public_const` conflicts; private const-v0 declarations remain unprojected. The
parsed initializer occurrence survives value substitution as source custody
and is rendered under the closed `const_initializer` role; its spelling remains
outside semantic row identity.
Ordinary `pub operator` visibility is now retained independently of carrier or
domain qualification. Exact authored source provenance supplies package
ownership, while proof-static late operator selections finalize only when
exact typed operands choose one declaration; checked lowering then repeats the
ordinary visibility gate. Private cross-package selection rejects and
same-owner implementation use remains legal. Review v54 / canonical row v14
add one blocking standalone public-operator shape. The row key uses exact
package-qualified declaration identity plus the compiler's canonical operand
and result-dispatch identities; the value retains boundary status, fixed
spelling, the complete signature, and declaration contracts without depending
on use-site facts. Public contract binaries retain an exact declared overload
coordinate or explicit builtin meaning rather than only a token. Unresolved
proof-static selections reject closed.
Complete name-first conformances now retain their declaration-owned `pub`
through syntax, source profiling, resolved/typed/checked trees, and stage
snapshots. Exact selection gates reject private cross-package use and public-
interface citation, public headers cannot hide a private carrier or trait, and
private member machines remain implementation. Lexical conformance-binder
requirements inherit the enclosing declaration rather than becoming package
declarations. Explicit row references retain their authored source occurrence
through exact target normalization and obey ordinary package visibility. Every package-owned
`pub Name: Subject satisfies Trait<...>` declaration contributes the exact
package-qualified conformance identity, normalized static telescope, optional
subject, exact trait application, and complete normalized checked requirement
interface. The referenced public-trait row owns requirement contracts and laws;
the conformance must discharge them before projection. Private member machines,
proof bodies, source text, and physical code identity are not public
compatibility material. An
exact machine requirement-satisfier edge and its optional `as Name` grouping
label do not produce this row. Private cross-package conformance selection and
public-interface citation reject; same-package private selection remains legal.

Review v55 and canonical row v15 admit package-owned public conformances with
alpha-normalized lifetime/static telescopes, nominal or telescope-parameter
subjects, exact trait application, and overload-qualified inherited machine
requirements. Closed and attached-machine forms normalize to the same row.
The projector matches every retained closed realization back to the public
closure but fingerprints none of the realization machine, state, body, or
inline/reference/default choice. Validation checks attached and closed
realization signatures plus substituted trait laws before projection. Target
traits with unretained lifetime arguments, inherited lifetime substitution,
and proof-static trait parameters reject rather than producing a partial row.
The conformance declaration is package-owned; its public subject and trait may
come from a direct dependency.
Exact requirement-local `satisfies` edges remain authored selections even
though they do not mint a whole conformance. Trait edges retain the exact trait
application and result-dispatch-selected requirement; operator edges retain the
exact signature-selected overload. A lifetime-parameterized trait application
is complete in source and retains raw machine-binder ordinals for checking, but
its edge identity contains their D55 normalized equality partition rather than
private realizer binder order. The realizing machine's interface exposure
governs both rows. Identity settles before checked, boundary, accepted, or
external supply policy, so rejecting one association cannot erase or substitute
the declaration the source selected.
Domain `established by Trait::requirement` paths retain the same exact trait
and requirement coordinates at signature-free normalization, after uniqueness
and subject authorization are proved. Each source occurrence inherits the
domain's exposure even when the normalized semantic route set deduplicates an
equivalent alternative.

The occurrence key is the exact authored token plus its declaration-owned
exposure and selection kind. Compiler-derived expression copies share that one
key; an exact resolved target dominates an unresolved provisional copy, while
two conflicting resolved targets reject. Checked-only compiler vocabulary is
retained as a closed intrinsic identity. Private termination rankings keep
exact typed expression roots beside their normalized rendered witness, so
neither package admission nor termination checking reconstructs ownership from
display text.
Nominal callable machine-parameter contracts preserve the complete authored
`Trait::requirement` path after signature-free resolution. Typed lowering emits
the exact trait and requirement selections under the enclosing declaration's
exposure, including recursively nested contracts; transitive-only and private
public-interface selections reject before package review.
In particular, true nested machine static applications such as
`consumer<family<Selected>>()` now reject during compiler validation, before
checked lowering. Treating the argument as the uninstantiated `family`
declaration checked the wrong callable shape; monomorphization also has no
closed recursive application identity in conflict equality, specialization
keys/fingerprints, or retained specialization evidence. Supporting this form
requires recursive specialization plus exact declaration-telescope, lifetime,
and static-argument identity throughout those paths. This is distinct from
already coherent bare generic-machine selection and call-target use such as
`Schema<Selected>(...)`.
Other non-public or lifetime-parameterized trait realizations likewise remain
fail-closed; binder-free generic requirements, explicit evidence binders, and
selected conformances with representable complete applications use the same
canonical row as public traits. Checked realizations of public, ordinary,
lifetime-free traits retain exact package-qualified trait and requirement
identities, alpha-normalized arguments, and any explicit conformance alias.
The separate v71 operator-realization lane admits only the checked public,
nongeneric form described above; its unsupported neighbors do not inherit
trait-conformance or external-supply semantics.
Public callable `requires` and `ensures` retain exact structural rows for the
closed boolean/integer expression subset over parameter
ordinals, `result`, generic binders, and package-qualified nominals. Domain-
membership rows retain the exact value expression and package-qualified public
domain; a private package domain cannot leak through a public callable.
Projection reads the earlier typed semantic tree only after checked compilation
succeeds. General logical expressions and bundle projections must retain
normalized binders, exact law/member selections, and fully substituted subject
arguments. Statement identity, witness identity, and admitted assumptions remain
separate; a local alias or diagnostic string is not an identity oracle.
Direct parameter-rooted member paths retain their receiver ordinal
and exact package-qualified case/field chain after a unique checked semantic-
place join and exactly one finalized public-interface member-token selection
to the same field. Missing, duplicate, or mismatched custody rejects. Simple
total, pure calls retain their optional receiver, exact
checked package-qualified entry target, and ordinary arguments after a unique
public-interface declaration-selection join. Their helper bodies remain pinned
by the separate whole-source commitment rather than being confused with
signature identity. In proof-owned domain, proposition, and contract
expressions, attached `self.member()` calls and path-qualified
`Data::member(value)` calls rederive that target only from the exact checked
owner environment. A path qualifier is not encoded as a value receiver, and no
program-wide name fallback participates in the row. Unreduced symbolic const expressions,
general mathematical static arguments, quotient calls, true nested
machine/conformance applications, unrepresented compiler-intrinsic calls,
computed members whose receivers are not in the closed expression vocabulary,
proposition-argument members without their checked join, and unsupported
call forms fail closed.
Contract
casts retain their structural operand, alpha-normalized target, arithmetic
policy, package-qualified semantic domain and arguments, and value/recast form.
Diagnostic spellings are absent, and private package domains reject when a
public cast would expose them. The coarse 64-bit machine-contract fingerprint
is no longer
package-review identity, so private state-machine shape cannot alter the public
contract baseline. Complete rows for the remaining unsupported forms and exact
proof/admission dispositions still gate sealing.

Claim-free opaque `boundary data` is retained in a separate representation-TCB
lane with producer-availability and consumer-demand rows. Availability binds
the package-qualified declaration and public conformance/carrier surface but
accepts no consumer selection. A demanded runtime by-value row is owned by the
selecting consumer and binds its exact requirement application to the selected
or compiler-derived representation: authorized source, carrier, target-
semantics identity, closed shape graph or sealed ABI leaf, physical movement,
role-tagged lifecycle disposition, representation version, evidence origin,
and strong conformance and boundary-plan commitments. Foreign demand rejoins the exact
producer rows and immutable source instance. `Unbound` is accepted only when no
active runtime by-value crossing demands the declaration. Introduction or
material change strongly recommends a code/ABI audit, while unchanged rows
remain visible without recurring blanket approval. Opacity alone is not a
blocking trust claim. Deployment policy may still classify an exact compiler-
owned mechanism as dangerous and blocking.

The first compiler-custody checkpoint retains the complete validated
activation-wide opaque-representation selection collection in the checked
compilation, including an unused selection and its selecting-machine/source
provenance. Checked compilation also retains every exact validated boundary
calling-plan realization and exposes its compiler-derived opaque uses; an
unused selection does not appear in those use rows. For a copyable opaque, the
compiler reharvests the exact selection from the final typed graph and admits
the property only when the complete inert carrier graph is structurally
copyable. Package review retains that target-independent application as an
audit-recommended selected copy receipt owned by the selecting package, even
when the opaque declaration is dependency-owned or the selection is unused.
Its versioned strong commitment binds the named conformance application, inert
lifecycle, and copy disposition; selecting source remains provenance. This is
neither target ABI movement nor D26 consumer demand. Package review separately
publishes exact public opaque/conformance/carrier candidates as producer
availability without accepting any selection. `Unbound` may coexist because it
reports the declaration independently of use. For each actual by-value use,
package review now emits the complete D26 consumer row from the retained use,
its exact carrier shape root, the complete checked shape graph, the semantic
parameter/result path, replay-validated physical placement, and strong
selection and boundary-plan commitments. Equal-looking layouts are never used
to infer an occurrence. Immutable foreign-source agreement remains a later
compiler artifact-composition check against the exact resolved source.

Accepted propositions, boundary/provider guarantees, authority establishment,
executable mechanisms, and derived dangerous reach remain independent
admission rows. Public ABI incompatibility may block on the API axis without
being mislabeled as proof trust. Missing `reaches` does not suppress the
representation row, and package-controlled type names never determine risk.

Open/deferred proof obligations reject package admission. This is an admission
requirement, not a claim that ordinary compilation already exposes such a
status: the current compiler has no explicit deferred-proof carrier, and one
contract-entailment tier may stand down on facts outside its engine language.
Package-aware checked compilation now records exact machine/contract/fact
coordinates and a closed reason for each checked-implementation stand-down
found on the pristine typed graph; the review projection rejects every row.
Accepted and opaque supply stays trust-bearing rather than becoming a proof
stand-down.
This closes the in-memory review hole. The narrow identical-authored-assumption
discharge now also has a source-handle-free canonical package-review row and
ordinary-ledger replay: fresh projection and replay independently reconstruct
the semantic question and invoke the proof kernel, while the original open row
remains visible. Further proof propagation into Terminal remains compiler work.
Source package acceptance consumes the checked result and disclosed assumptions
without requiring a lock-certification stage.
Accepted axioms and opaque boundary claims must remain explicit trust-bearing
rows; authored postconditions remain obligations. Boundary providers must
satisfy exact package-qualified requirement identities, so a same-spelled trait
from another source lineage grants nothing. The current provider carrier pairs
the normalized requirement identity with its exact owner package, but binding
and selection carriers remain unfinished.

Package policy admits the transitive reachable-authority set of the final
resolved artifact. It does not approve dependencies one edge at a time. A new
root-memory, DMA/IOMMU, executable-installation, interrupt-publication, or
equivalent reach blocks unless deployment policy explicitly grants it,
regardless of which transitive package introduced the change. Network,
filesystem, process, dynamic-loader, signing, secret, and other intrinsically
dangerous authority remains audit-relevant even when a candidate update does
not expand the package's declared authority set. Updating
`filesystem + network` to another `filesystem + network` package version may
be lock-admissible only if the normalized capability manifest still matches,
but the update flow should still surface a recommended audit finding because
the changed implementation can now misuse already-admitted power.

Package capability admission uses conflict-resolution artifacts rather than
approval prompts, and it is part of install/update rather than a disjoint
workflow. `omega install` treats the prior admission baseline as empty for the
new dependency closure; `omega update` compares against the normalized accepted
baseline retained by the existing lock. Either command writes a
compiler-generated capability conflict when
the candidate introduces blocking or suspicious authority, stops before
mutating `build.omg` or `omega.lock`, and resumes only after an exact resolution
artifact accepts or rejects every blocking delta. Initial install therefore
requires root-policy resolution and recommends audit when a new dependency
brings filesystem, network, process, dynamic-loader, signing, secret,
executable-installation, root-memory,
DMA/IOMMU, interrupt-publication, or equivalent suspect authority. The conflict
fingerprints the old and new source identities, old and new package manifests
or an explicit empty baseline, delta identities, dependency path, and canonical
rendered evidence. The resolution binds the exact conflict fingerprint and the
decision for every blocking row. Reviewer identities, signatures, quorum,
tickets, or reasons may be requirements of deployment infrastructure, but
Omega does not store them as evidence that review occurred. Missing, stale,
mismatched, duplicated, dependency-supplied, or overbroad resolutions reject
before lock mutation. `omega.lock` records the accepted baseline and decisions.
It is normally generated by this transaction; whoever lands it is trusted to
make those decisions. Reading it validates structure and consistency, not the
history or seriousness of the acceptance process.

The complete-policy comparison joins retained lock policy with the freshly
checked candidate. It includes removed packages, directional root-role changes,
and source replacements at the root or an existing requester/alias binding.
Ordinary API rows have no previous compatibility contract on initial admission;
dangerous authority and accepted assumptions require explicit choices. Every
required change needs its own accept/reject disposition. The comparison covers
both source graphs, targets, full policy meaning, and candidate compiler/build
observations. Missing, duplicate, foreign, stale, and advisory-only choices
reject without reconstructing a compiler result or promoting it into trust.

The editable review document starts with `omega-package-review 1` and its exact
comparison identity. It presents source-qualified package identities, immutable
pins, dependency paths, audit recommendations, and `-`/`+` canonical policy text.
Each required change has a `decision` line ending in `pending`; the project
changes that token to `accept` or `reject`. Advisory-only findings have no
decision line. There is no blanket approval token, free-form audit receipt, or
second resolution checksum. Package prose is excluded; source identifiers are
quoted and the compiler policy codec escapes policy strings.

Recovery regenerates that document from the current comparison and permits
only the decision-token edits. It checks the exact displayed findings and full
decision coverage before returning the project's choices. Versioned LF text
and a caller-selected byte ceiling bound rendering and recovery. These checks
detect stale or accidentally edited review input; they do not authenticate the
author or prove anybody performed an audit. A retained rejection prevents the
represented choices from accepting the candidate. Even complete acceptance is
not permission to publish without the transaction's source/project-file checks.
Install/update commands persist and recover these per-target review files.
The older review-only decision codec and file layer below are
implementation facilities, not additional install/update acceptance gates.

`operations::review_package_change` joins candidate compilation and comparison
for one exact target. It checks the resolved graph through the existing compiler
candidate entrance and rejects any remaining ordinary contract-entailment
obligation in any package. Locally rechecked assumption discharges have already
been applied by the compiler; explicit accepted assumptions still appear as
policy findings. The operation reads the current per-package results directly,
without assembling a second reconstruction question or promoting review into
certified trust.

Its proposed-lock method accepts only a complete accepting resolution for that
comparison, rechecks retained source snapshots and selections, and constructs
the target section from full candidate policy and direct decision history.
No old checkout, native artifact, or file write is required to construct that
proposal. The file transaction must still check concurrent project edits,
immediate source/build consistency, and all selected targets before publication.

Automatic dependency-edit plans can be checked before any live declaration
change. `operations::stage_build_dependency_edit` verifies the planner's expected
old `build.omg` digest and replaces only that file in an immutable candidate
snapshot. The stage retains the original requested root, canonical live root,
and before-edit source identity separately from its proposed content. Staged
project resolution reads the proposed declaration, but retains the original
root path/context as package lineage and the base for relative Path dependencies.
It verifies the proposed snapshot and unchanged live source rather than claiming
the edited bytes were already present in the checkout.

The resulting closure enters ordinary candidate review and lock proposal.
Landing the reviewed declaration bytes yields the same root source pin; neither
staging nor review writes accepted project files. The caller retains the plan
and stage to detect intervening edits. Manual declaration patches still need
author placement. The pin-aware staged resolver preserves unaffected Git
requests against the accepted graph and requires the same original root request.
The command maps root aliases or unambiguous package names to exact selected
keys. Selecting a Git workspace member refreshes its repository lineage as a
unit, while unaffected repositories remain pinned.

### Install/update command lifecycle

`omega install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>]`
installs a Git root, named Git workspace member, or local package. The fetched
declaration supplies the package name;
the consumer supplies an alias only when overriding the default.
`omega update [package-or-alias...] [--to <revision>]` updates all requests or
the selected repository lineages. `--to` requires one selection with a
root-authored Git request. `--package` maps to the existing named selection in
`build.omg`; it accepts a declared package name, not a member path. The resolver
checks workspace membership and rejects absent or duplicate names. Omission
selects the root; local sources use their package directory directly. Selection
persists through the proposed declaration and graph during review/resume.

Member snapshot storage is scoped by repository acquisition before tree ID.
Byte-identical members in unrelated repositories must have distinct physical
compiler roots, while source lineage and declared name continue to determine
package identity. Cache paths do not become nominal identity.

Both commands accept `--project <dir>` and repeated `--target <name>`. Target
selection retains all existing lock targets and adds requested ones; first
acceptance defaults to the host if none is named. An existing valid `build.omg`
is required. Unsupported automatic declaration edits request a manual patch.

Before reading accepted files, the command opens the project transaction and
recovers any recorded publication intent. It stages proposed build bytes,
resolves the candidate, and checks every retained target. Blocking findings
return status 3 with accepted files unchanged and write
`build/package-manager/review-<target>.txt`. A separate `proposal` file retains
the candidate graph, targets, proposed declaration, and original project-file
and source identities. This is restart state, not project acceptance.

The matching command's `--resume` uses the saved candidate pins, fetches exact
missing commits if necessary, recompiles, and recovers only edits to decision
tokens. Changed project sources, accepted files, dependency content, or findings
require a fresh proposal. Rejection leaves the candidate unpublished.
`--discard-review` removes only the proposal; diagnostic review files remain.
It does not discard publication recovery or undo already accepted changes.
Nonblocking candidates publish directly while still reporting audit advice.

Missing lock state gives a fresh complete-graph review through unselected
`omega update`; selected updates need the accepted graph to identify their
selection. Unsupported formats reject with recovery guidance before any
selector refresh. Retained baselines support policy comparison without an old
checkout. Commands write a separate `build/package-manager/source-diff.txt`.
They recover exact old Git commits without refreshing selectors and verified
old local snapshots from the existing cache, using the known source origin and
accepted content pin. Matching source or an unchanged live root can also supply
the baseline. Missing or corrupt old source and bounded-rendering failure are
reported explicitly without suppressing policy comparison. New packages have
candidate source, not a fabricated previous version.
Source text never enters editable capability decisions, and resume regenerates
the source document. No advisory service is needed to resolve compiler findings.

`omega audit packages` checks the current root with accepted dependency pins
and presents graph/API/reach/assumption findings beside historical lock policy.
It never records decisions or changes accepted files. Missing or unsupported
analysis is explicit; an absent lock means unaccepted fresh inspection. Default
output summarizes the findings; `--details` includes complete normalized policy.
The [inspection operation](../../../omega-rust/omega/packages/manager/src/operations/inspect_packages/README.md)
owns target selection, failure behavior, and CLI status.

Ordinary `prepare_local_project_for_target` selects the accepted target before
acquisition and preserves dependency pins. The mutable local application root
may change source while retaining its identity, role, and dependency projection;
dependencies must still match recorded content. Changed dependency requests or
local dependency content require an update. This is distinct from strict
whole-closure lock recovery. Local root identity remains tied to the canonical
path, so moving a checkout requires fresh reconciliation rather than silently
rewriting its accepted identity.

`omega --check` uses scoped candidate checking for a prepared project, including
dependency-generated source and final semantic-binding discovery. It retains
the selected root's checked result, disposes staging, and reports ordinary trust
settlement without rerunning its build or entering native admission. Package
and application roots may be checked; native production keeps its separate
application-root requirement. The requested source entry remains explicit.

`publish_reviewed_package_change` joins the staged build edit with all reviewed
target sections and their exact decisions. It checks original file bytes,
stage/root/edit identity, retained target coverage, and comparison against the
supplied accepted lock. It rechecks snapshots, current local dependency pins,
and unchanged original project source without recompilation or certification.

Publication uses a persistent OS mutex and bounded commit-intent journal under
ignored `build/package-manager`. The journal contains only fixed old/new build
and lock bytes, including explicit absence of an old lock. Once intent is
recorded, recovery completes forward. It first checks the whole live pair:
each file must match its recorded old or new bytes. An unrelated edit stops
recovery and retains the journal. Failures before intent leave the pair unchanged;
a pending-publication error afterward may represent partially completed writes.

The pair is recoverable, not two simultaneously visible filesystem renames.
Package commands must hold the mutex while loading accepted files and recover
pending state first. Ordinary project preparation participates when state already
exists, without creating transaction files in a read-only source-only checkout.
Each replacement preserves permissions and uses synchronized atomic stages under
the ignored state directory, so interrupted staging cannot change source identity.
Unix directory synchronization errors propagate; the current Windows utility
does not flush directory metadata and makes no equivalent power-loss durability
claim. This coordinates processes; it does not protect a project from its own
author or prove review occurred. Pending journals must not be removed as
disposable build-cache cleanup.

Review compilation may execute compiler-scoped build code: package-input reads,
disposable-output writes, and compiler logging are permitted before package
runtime policy decisions. Runtime boundary-service reach from the build machine
is rejected before authored build execution. This does not grant dependencies
resolver credentials or expand their output roots. Later checking failure may
follow permitted build I/O, but does not modify accepted project files.

Command review files and the proposal are the install/update restart state.
The lock retains accepted policy for later comparison, including native builds
under [single package acceptance](#single-package-acceptance). The separate
native root-policy input is superseded, not an additional approval document.
An optional review-baseline capsule adds no approval requirement. Do not require
a parallel baseline archive, governance record, or evidence-promotion step.

Source changes should expose an available code diff or explain why it is
unavailable. Optional model triage may help prioritize an audit, but is not a
prerequisite for install/update. An implementation can misuse already-admitted
power without changing capability
evidence. Retained dangerous authority always produces an audit recommendation.
Claim-free representation-TCB findings appear in the same command and may
produce `no-review-blocker-with-audit-recommended` without manufacturing a
conflict or resolution artifact when no independent policy blocks them.
The prior source tree improves review quality but is not the admission baseline:
if it is unavailable, lock-based capability comparison still works and source
review escalates to a standalone candidate audit. If the accepted lock baseline
is unavailable, the complete closure undergoes fresh admission.

Resuming a proposal rechecks the selected candidate and its current conflicts.
Historical lock decisions are trusted project state, not freshly issued
compiler facts. Any optional checkpoint supports restarting work; its checksum
detects corruption and does not authenticate acceptance or prove serious review.

LLM review is advisory output, not authority to mutate the lock. Optional
review tools consume canonical diffs rendered by package core; package
acceptance and deterministic audit recommendations are identical when those
tools are absent. Bounded and escaped package-origin identifiers are treated as
quoted inert data. Package prose, comments, README text, and commit messages do
not enter capability triage. A following source-code audit may still read
attacker-controlled code; that risk is handled by the reviewer workflow, not
by granting package prose authority over admission.

No package artifact proves that this workflow was performed seriously. Local
compiler output prevents dependency-authored manifests from impersonating
review rows. The package manager drives compiler review in the same `omega`
process, and its reconstruction checks canonical and semantic consistency; it
is not isolation from that process or evidence about the loaded executable.
Consumers trust the project's accepted decisions and the compiler they invoke.
The compiler checks the selected source and any artifact claims it produces. D46
therefore excludes `current_exe()` path-byte observations from review, lock,
conflict, cache, and admission identity. Likewise, signatures and
recorded review fields establish custody over a decision, not its quality; PCC
establishes only the exact proposition checked by its kernel. The accepted
project commit and the organization controlling it authorize the update.
Organizations that need stronger assurance impose their own branch, quorum,
isolated-build, bootstrap, reproducibility, and independent-review policy around
Omega's deterministic conflicts and recommendations.

Tools render these layers separately. A mechanical verdict reports locally
re-derived obligations and certificate results. Admissions report the exact
semantic assumptions accepted by local policy. Producer, reproduction,
signature, and audit metadata appears in a distinct review section and never
inside a `verified` verdict; presentation must not launder pedigree into
checking.

Boundary statements imported from a dependency are inert requests. The root
reviews their complete normalized claim set through the integrated package
comparison. Adding, removing, or changing a claim requires a decision on that
exact change, not blanket reapproval of unchanged claims. A package cannot
accept its own imported claims, and a claim the checker can refute remains an
error despite acceptance.

The complete compiler-derived report remains machine-readable. Human diffs are
severity-ranked: checked local tokens collapse to a short summary, while new
admitted providers, boundary-evidence permissions, provider-owned backing,
generation/revocation machinery, or system authority are elevated with their
dependency path. Package policy decides who may enter with power; checked
contracts still constrain behavior after admission.

## Workspace composition

A workspace build composes member `Build` values with ordinary Omega code.
Shared pins and ceilings may be passed into members and members may only narrow
them. Source code never searches parent directories for ambient imports; only
the build tool discovers the nearest enclosing workspace/build entry.

Members are declared by **path**, so relocating a subtree is a one-line manifest
edit rather than a repository-wide rewrite. A remote dependency names a package,
not a directory: the resolver fetches the source, reads its root manifest,
consults the member list, and selects by declared name. One repository may
therefore publish several packages, and moving one inside its repository breaks
no consumer.

The selected path remains operational custody: it locates retained bytes and is
the base for the member's relative dependency requests. It never becomes stable
package identity. Missing and duplicate declared names, undeclared or escaping
member paths, and recursive `build.omg` search reject.

`SourceIdentity { kind, locator, resolved }` keeps `kind` open. Git is one
supported source kind, not the blessed model. Package selection is a separate
projection and does not fragment one fetched tree into several source identities.
There is no package version field: `PackageKey` is the declared name plus
canonical repository lineage, while `PackageInstance` carries the exact resolved
revision/tree/content. A moving locator such as a branch resolves once and is
pinned thereafter.

One `omega.lock` lives at the workspace root, and a dependency's lock is never
read by its consumers. The lock belongs to whoever builds an artifact; a library
does not pin its consumers' graph. What composes upward from a dependency is its
manifest and its disclosed admissions — separate artifacts with separate rules.
The single lock may contain several independently resolved target-profile
closures; their presence, absence, review state, and exact semantic profile
identity are explicit rather than inferred from the host running the command.

A workspace is a catalog of locatable members, not one combined graph and not a
graph node with its own `PackageKey`. If it contains several applications, each
is an independently selectable closure root; selecting one does not include the
others or unrelated package members. A command may explicitly build or check
several roots, but workspace membership alone never makes a member part of an
artifact.

The compiler itself does not discover or mutate that lock; its command
coordinator uses it for package resolution. A compile request
supplies one complete in-memory admission set; the compiler independently
reconstructs the exact required obligations and returns the consumed,
unresolved, and unused rows with its product. The command coordinator reads the
separate `omega.admissions` compiler policy for an ordinary fail-closed check. Only the explicit
`--accept-admissions` operation replaces the admitted set with the exact
compiler-reconstructed set. Missing compiler admission policy never turns ordinary
compilation into implicit approval, and trust-report files remain diagnostics
rather than policy authority. Filesystem-free obligation/report construction
lives in `trust-model`; `trust-ledger` is limited to coordinator-
facing `omega.admissions` custody. It cannot overwrite package pins through
`--accept-admissions`. Existing compiler policy containing the former strong
digest/commitment rows under `omega.lock` must be explicitly moved to
`omega.admissions` before package operations. A package-format lock must not be
renamed or interpreted as compiler admission policy. This separates two file
owners; it adds no install/update audit or certification requirement.

### Single package acceptance

Package acceptance is one project decision. Native compilation verifies current
requirements against that recorded acceptance; it does not ask the owner to
approve the same requirements again. This explicitly supersedes the former
requirement for a separate candidate-bound `--package-root-policy <file>` input.
Remove that duplicate file/flag workflow, not the native checks it currently
feeds. No additional native-specific package decision has been identified.

Recheck the actual source closure and selected target, then compare its fresh
normalized policy with the project's accepted policy. Matching requirements reuse
acceptance. Missing acceptance or changes requiring a decision use the existing
install/update review workflow; an ordinary native build reports those needs
without approving them. Use the existing package-policy comparison, including
full row content, package replacement, and root-role tracking, rather than a new
approval-reuse scheme. Native preparation must preserve the accepted baseline
instead of substituting an empty one, and native admission must check the join
from that comparison to its freshly reconstructed requirements.

Acceptance covers defined permission and assumption contracts, including their
constraints, not diagnostic wording or incidental presentation. Compatibility
inherits the existing evidence-schema rule: exact semantic-schema identity is
required; an unsupported identity requires reconstruction and fresh admission,
not guessed equivalence. This decision introduces no separate migration policy.

The current acceptance record is `omega.lock`. Its location is not the reason
for consolidation and does not prescribe permanent storage or a new acceptance
vocabulary. Regardless of storage: acceptance remains project-owned; every
consumer checks it against current requirements; regeneration cannot invent it;
and resolution-only updates with unchanged requirements preserve the accepted
permissions and assumptions without adding grants. This is semantic preservation,
not byte-identical decision serialization: comparison and source commitments may
change. An absent accepted baseline remains absent until explicit acceptance.

Unchanged policy does not imply unchanged behavior or filesystem-object
confinement, even for exact mechanism rows. Source changes must remain visible
when policy matches; an available diff or an explanation of its absence supports
audit, not a preventive security guarantee. Do not impose a separate reuse rule
on transitional broad classes under the claim that exact rows confine objects.

Build evaluation can execute build machines. Its filesystem/output grants must
be enforced before access; package acceptance neither grants that authority
retrospectively nor replaces it. Likewise, compiler admissions, proof obligations,
native artifact verification, and the independently supplied receiving permission
policy retain their separate owners. Fresh permission rows must still correspond
to the exact checked source, provider, production, and selected mechanisms.
Recorded acceptance is trusted project intent, never proof or permission to
substitute stale evidence. Receiving-policy denial and unproved contracts still
reject after package acceptance succeeds.

The native implementation retains the accepted target during project preparation
and checks its complete policy against the same fresh reviews used to reconstruct
native obligations. See the [manager implementation](../../../omega-rust/omega/packages/manager/README.md).
Regression coverage belongs beside preparation, native compilation, ordinary
policy comparison, and admission; the CLI exercises update/resume followed by
native compilation without another approval file.

### Core and ordinary library packages

`omega::language::core` is bundled with the compiler by decision rather than by
omission. It is the language: the checker cannot typecheck without it, its
version is the language version, and two versions of it can never coexist in one
graph, so welding it enforces something real instead of hoping a resolver agrees.
`omega::language::std` has no corresponding privilege. It is one possible
ordinary fetchable convenience package with its own version line; it may be
replaced, split into narrower packages, omitted, or retired without changing
the compiler's semantic contract. The compiler grants no authority from the
name `std`, its source lineage, repository, path, filenames, or same-spelled
declarations. Freestanding builds require no std, and the compiler-owned Build
protocol must remain usable when no standard-library package exists.

Where composition genuinely needs to recognize a declaration supplied by an
ordinary package—currently target entry/profile integration and consumer risk
classification—it binds the exact nominal declaration and normalized schema
inside the accepted resolved closure. The binding does not bless the package,
does not grant a provider or capability, and cannot be reconstructed from an
alias or source location. Candidate designations guide confined review only;
accepted bindings come from consumer policy.

The first provider-bearing binding role is the target-independent
`Console::exit_process(i32) -> Unit` compiler-intrinsic consumer. Its input row
binds the exact `PackageKeyIdentity`, canonical nominal declaration path,
collision-resistant normalized `ServiceSchema` digest, and collision-resistant
complete selected `ProviderPlan` digest. Compilation first requires the package
to be in the reconciled closure, then independently rejoins the selected exact
symbols, package owners, schema, and complete plan. A supplied row must settle
exactly once; foreign, stale, ambiguous, and unmatched authority rejects. Downstream review
rederives against the compiler-resolved declaration symbol rather than
searching by path, and only that exact symbol receives the existing Process
dangerous-authority classification on every target. Physical lowering remains
separate: only Linux x86-64 and AArch64 currently close the exact
`LinuxExitGroupI32` execution identity. Constructing or consuming this policy
row proves no human or model audit occurred.

The requirement-only `FilesystemHostService` role binds the exact package,
canonical boundary declaration path, and normalized complete service schema;
it carries no provider-plan digest and cannot synthesize a provider. Confined
candidate review currently nominates package-owned reached declarations named
`FilesystemHost` for this role, but that readable name grants nothing. The
preliminary review also retains that candidate's exact checked `ServiceSchema`
as non-authoritative consumer review material. This lets policy authoring
select complete requirement identities from the compiler-issued schema rather
than reconstructing them from spelling; it still assigns no permission and
the final checked pass must rejoin every submitted row exactly. The
bound compiler replay must consume exactly one declaration matching all
accepted coordinates before review classifies that exact resolved symbol as
Filesystem authority. Foreign, stale, ambiguous, and unmatched bindings reject.
The repository policy canary partitions the complete current 50-method schema:
36 requirements have explicit consumer-authored portable dispositions, while
the fourteen control/lifecycle requirements remain an implementation gap under
the settled [filesystem control/lifecycle policy](effects_authority_and_observation.md#portable-filesystem-control-and-lifecycle-authority),
not an open convention choice. Their absence must not be read as empty
permission. Any schema addition or omission fails that partition
test. This is permission-authoring coverage, not object confinement and not a
target mechanism classification.
Completing the table preserves exact requirement and mechanism identities even
for explicit empty dispositions. Ordinary release narrowing requires checked
occurrence-specific handle flow and release contracts; an unconstrained generic
closer gets a justified broad classification or remains unsupported. A broader
known class set fails service containment, while missing classification fails
realization admission. Neither changes source service reach or invalidates
otherwise valid Terminal Psi.
Package-aware checked interpretation receives an opaque routing token for that
same compiler-resolved declaration symbol. Readable service or operation names
cannot select filesystem dispatch, and the token rejects if substituted into a
different checked-program instance. The token grants no filesystem access and
proves no admission; interpreter options still supply provider authority
separately. Standalone interpretation retains only the exact bundled
`filesystem_host.omg` source fallback.

The requirement-only `UefiX64ProgramEntry` role likewise binds the exact
package, canonical `UefiApplication` declaration path, and normalized service
schema selected by the UEFI x86-64 target consumer. Candidate review may derive
that row from a semantic-only checked compilation, but target compilation must
consume it exactly. Its schema commitment deliberately omits the two
target-evaluated calling-plan fields: the UEFI target independently replays and
retains the exact semantic and physical plans, so duplicating those values in
the package nominal binding would create a second ABI authority. Missing,
foreign, stale, unmatched, or non-UEFI use rejects. The source remains an
ordinary package source; selection of the target or a same-spelled declaration
does not grant the role.

The deliberately non-std `host-services` fixture exercises both Console and
Filesystem roles so package, repository, alias, and bundled-library identity
cannot accidentally become the authority test.

These roles are deliberately narrow. Their present schema normalization is
exact for their checked signatures and reaches; it is not yet a generic
accepted-boundary mechanism for arbitrary package-owned nominal carrier types.
The legacy Console lane remains only as byte-exact bundled standalone
compatibility; it no longer depends on blanket std toolchain provenance.

The vertical implementation canary resolves the repository's real std
directory as an ordinary local package, derives its default
`omega_language_std` alias from its own declaration, compiles from resolver
snapshots, and produces a complete ordinary package-review entry containing
the public Console and wire surfaces. Std's authored self-imports are ordinary
package-local imports. Omitting the dependency edge rejects a consumer import
rather than consulting bundled std. Package-aware compilation rejects every
non-core `omega::language::*` path with ordinary-dependency guidance, admits
only the exact bundled core directory into its toolchain source frontier, and
classifies ordinary std sources as package-owned. Standalone compilation keeps
the legacy compiler-bundle route and provenance over bundled std/alloc until
every standalone compiler consumer has migrated. Console and UEFI already use
narrow byte-exact bundled fallbacks rather than directory authority; the other
standalone consumers must gain equivalent exact source-role recognition before
the broad compatibility classification can be removed. Relabeling the same
directory-derived authority would not complete that migration. Package-aware
compilation does not inherit this lane: its toolchain-overlap gate is limited
to the actual bundled core root, allowing the repository's std directory to
enter as an ordinary package. The compiler product and parser package now
declare their own std edges, import through `omega_language_std`, reconcile one
exact std package in the compiler/psi diamond, and pass the package-aware
command route from resolver snapshot through checked compilation. All 140
std-consuming sample packages likewise declare ordinary std edges and import
through `omega_language_std`; the compiler sample harness supplies an explicit
package graph, and `omega refresh-samples` now uses full resolver custody. The
freestanding UEFI package remains dependency-free; two proof-only source
fixtures remain standalone. The package-aware sample sweep adds no failures to
the seven independently reproduced on its prior standalone baseline. Ordinary
visibility is now explicit on the documented time and ergonomic filesystem
surfaces; public boundary-reaching methods publish exact `TimeHost` or
`FilesystemHost` reaches and synchronous-invocation ceilings, while their
implementation helpers remain private. Remaining fixture migration,
standalone consumer migration, accepted-lock replay, and macOS GUI injection
remain explicit seams to remove. Only `omega::language::core` has a magic mount
in package-aware compilation.

Making std ordinary also makes core's package boundary concrete. Source-facing
float namespaces, formats, meanings, semantic operators, and boundary
operators are explicitly public rather than admitted by toolchain location.
The two canonical primitive-to-proof projections remain private and
compiler-sealed; public float contracts may cite only those exact checked
toolchain declarations. Same-spelled authored declarations do not satisfy that
narrow exception.

## Current engineering delta

The scoped filesystem executor and real/virtual filesystem modes are the live
foundation. The Rust package crate now has reviewed production building blocks
for immutable source custody, typed identity/closure, compiler handoff/review,
row conflicts, restart-stable review baselines, and triage. Install/update
commands now join those facilities to per-change decisions and recoverable
build/lock publication. Native candidate-bound root policy remains in the current
implementation but is superseded by [single package acceptance](#single-package-acceptance);
consolidating its coordinator and admission consumer is unfinished engineering,
not an owner decision or a new prerequisite for source installation.
The name-keyed lock, caller-constructed manifest JSON, mandatory caller-supplied
name/alias, fingerprint-only baseline, and free-form receipt prototypes are
deleted rather than retained as a parallel test model.
The legacy standalone local-Path compatibility scanner is deleted. Standalone
compilation cannot mint package roots or aliases from `build.omg`; only the
validated compiler handoff can route dependency imports. The migration also
preserves package-aware placed-access semantics: discovery rejects ambiguous
policy/schema spellings, retains both exact source identities, and checks both
declarations' package visibility and direct-dependency authority before
synthesis. Because `Placed<P, S>` is erased before ordinary type-selection
capture, both nominal inputs must be public even for local use; this prevents a
public signature from laundering a private declaration through the source-free
compiler shell. Its inert opaque field carriers follow shell visibility,
callable operation visibility follows exact `AccessExposure`, binding-private
access remains policy-package-confined, and statement-position operations
retain their exact generated target.
The package-facing source/staging root capabilities, checked relative resolver,
explicit generated-source handoff, and frozen package-review final pass are
implemented. Final review can now retain its exact application-root checked
compilation in a non-clonable production candidate. After the disposable
sponsored staging session is removed, the compiler consumes that checked value
directly into a package-bound Terminal report, and manager admission can join
the report to fresh accepted evidence before producing an unpublished native
artifact. This continuation does not reload source, rerun `build.omg`, reopen
dependency discovery, or admit standalone checked values. Generated-source
custody is retained in the checked product rather than reconstructed from an
ambient staging path.

Native publication must preserve the checked source/generated-source/artifact
relationship and satisfy the compiler's native guarantees. `TASKS.md` tracks
remaining work under its owning area: compiler native publication or package
source install/update, lock persistence, and review integration. Older native
promotion machinery is owned by that compiler handoff, not a new install/update
requirement. Neither workflow requires a lock that certifies its own acceptance.

## Discovery topics, not package-manager blockers

These are possible build/library refinements, not requirements to finish the
supported install/update workflow. Promote one only when a concrete consumer
needs a decision; existing implementation machinery alone is not motivation.

- workspace inheritance/ceiling details;
- the minimum buffer-oriented spelling of the compiler-owned Build-facet
  operations after package fixtures exercise them;
- which optional library provider families are actually useful beyond the
  current Filesystem/Console packages;
- initial root policy profiles for volatile-capable, record-replayable, and
  source-rebuildable builds; and
- UX for displaying the first failed provenance edge.
