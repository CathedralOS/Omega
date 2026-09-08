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

[Compiler-derived review](../../spec/packages/review.md) owns complete package
findings and semantic/source identity. Its [capture and encoding notes](../../../omega-rust/omega/packages/review/evidence/README.md)
own compiler-private joins and current support limits.

[Project locks](../../spec/packages/locks.md),
[source selection](../../spec/packages/sources.md), and
[package boundaries](../../spec/packages/boundaries.md) own retained acceptance,
resolution, declaration authority, and carried semantics. Target-sensitive public
identity is part of [review](../../spec/packages/review.md#target-dependent-public-identity);
replacement envelopes belong to [component publication](../../spec/build/component_publication.md).

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
