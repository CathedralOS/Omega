# Terminal Psi boundary calls and realization

[Portable product](product.md) | [Byte views](byte_views.md)

These are semantic and evidence requirements, not a claim that every described
native route is implemented. Unsupported routes reject at their owning consumer.
Boundary [content conservation](../resources/content_custody.md#call-conservation)
and [installed component publication](../build/component_publication.md) have
separate contracts; selecting a provider does not establish either by itself.

## Call and requirement identity

A static bodyless call retains its boundary-machine identity; provider selection
does not rewrite it into a chosen implementation. Declarations retain ordered
scalar parameter types, and calls retain matching ordered value identities
alongside the independent structural argument lane. The optional primitive
scalar result is a separate lane. Encoding binds the orders; verification checks
arity, definition, dominance, and exact types. Interpretation evaluates scalar
inputs before invoking the effect handler. Lowering must preserve both lanes.

Borrowed arguments retain the caller's referent across completion. Only owned
arguments transfer custody; eventual owner cleanup remains separate even when
another argument in that call is consumed.

A boundary's unqualified mutable byte-view parameter may receive an inline
bounded-owned byte field through its exact non-erased record/fixed-array path.
The operand retains the caller's owner and capacity; this is a boundary
presentation, not equality between inline storage and a borrowed descriptor.
Source access, multiplicity, disjointness, and completion checks still apply.
It grants neither view qualifications nor ordinary-call conversion. Runtime
realization must preserve capacity and live-length writeback on that exact
field; canonical transport alone establishes no interpreter or native support.

Trait requirements and explicit top-level boundary requirements have distinct
canonical kinds. A top-level requirement retains its package-qualified operation,
static telescope, signature, contract, and visibility. A bodyless implementation,
bounded effect row, or provider selection cannot synthesize that identity.

## Structural domain requirements

A boundary structural requirement identifies an argument position and an exact
domain identity. Admission checks that identity in the argument's carried
qualification roster. The qualification was established through whatever proof
and/or sealed introduction its domain requires; the boundary call does not
establish it again.

This requirement is not a proposition term or conclusion and creates no
`ObligationId`. Boundary calls have no positional proof-obligation vector for
these requirements. Ordinary calls' `requires` propositions retain their own
proof-obligation lane. Encoding a structural qualification as a trivially true
proposition cannot substitute for the carried identity.

A proof condition may constrain a permitted use but cannot replace a domain's
sealed introduction or routed provenance. Qualification transfer must preserve
the exact argument path, identity, and establishment lineage under
[optimization](../build/optimizations.md#evidence-and-control-flow).

## Checked providers

Checked satisfiers are ordinary Terminal machines with canonical conformance
rows. A row binds the exact boundary requirement, nominal provider, canonical
adapter, artifact-local machine, signature, and checked service refinement.
Resolve selected adapters by exact overload, provider type, and adapter identity
against the verified catalog, then admit only those machine identities for the
exact artifact. A cataloged boundary without that installation rejects; it must
not fall through to a generic host effect handler.

Provider conformance rejoins parameter and result declarations rather than
adding an independently encoded signature copy. Structural results retain exact
type, multiplicity, and qualifications. Candidate result places and caller
operation-result places remain distinct. Admission of a conformance does not
widen the supported candidate-body, claim, content, or crash-contract vocabulary.

Execution transfers actual input custody through the ordinary structural-call
continuation and establishes the caller's result only after the selected
provider's successful, fuel-charged normal return. Suspension does not repeat the call
or invoke a host result fabricator. Projected consumers keep the same residual
complement and cleanup obligations as ordinary calls.

The canonical [process-exit requirement](../language/process_exit.md) instead
retains a closed external completion with no normal result. Conformance must
preserve its exact terminal identity, arguments, and progress contract. A
returning, divergent, or aborting provider is not a substitute merely because
its signature or reach matches. Exit produces no successful-disposition receipt
for abandoned obligations. A simulator terminates the bound simulated domain,
not its embedding host. Conditional ordinary helpers retain their return paths.

For projected claim transfer, replay the exact record/fixed-array argument path,
completion receipt, and complete caller claim-source catalog. The provider's
whole-root claim must match the resulting leaf type and rebase to that caller
path without dropping sibling sources. Replay access attenuation and linear
multiplicity separately. Path, receipt, provider, type, or qualification drift
rejects. Claim transfer alone cannot prove residual content geometry.

## Host result validation

An embedding handler distinguishes Unit, exactly typed scalar, and opaque
structural responses. Structural responses bind the exact declared type,
ordered qualifications, and whole-root path before result custody is installed.
Unsupported result capability rejects before invoking the handler. A malformed
response does not commit interpreter receipts or ownership changes, but this
does not roll back effects already performed by the host.

The host must supply a legitimate owned value. An opaque identity number does
not establish global allocation freshness against interpreter-created values
or suspended callers. Shape validation alone is not external ownership proof.
Current interpreter limits are described beside
[effect handling](../../../omega-rust/psi/semantics/terminal-interpreter/README.md).

## Reachability and attachments

An installation-bound requirement may retain an upper-bound reachability row.
It is keyed by that exact requirement, its normalized service union bound, and
all internal call-graph dependencies. Equal service sets do not merge distinct
requirements. Do not replace the row with Boolean formulas or a caller-selected
implementation.

Installation verifies the selected concrete operation row is contained in the
bound and substitutes it through the complete root closure. Preselection
manifests retain the unresolved identity and bound; selected manifests add the
provider and operation; final admission rejects unresolved rows. Such unresolved
rows cannot cross an ordinary callable package or component boundary.

Verification reconstructs reachable fixed boundary rows, primitive service uses,
and exact installation dependencies from executable operations. That closure
must equal the retained root declaration: missing, padded, stale, or unused rows
reject. A primitive service use does not disappear because the service also
appears in an abstract row's bound.

An attachment retains its nominal receiver and erased provider fields.
Sorted provider-attachment roots bind each used field to exactly its directly
called bodyless requirements, not to an installed native provider. Missing
attachments, duplicate/orphan roots, and roots forwarded as runtime arguments
reject. An unused provider field keeps its type and attachment but no invented
root. Ordinary callees retain their own direct requirements.

An ordinary receiver parameter has the exact attachment type and retains access
and multiplicity independently of the roots. Its signature does not construct
storage. Generated native entry provisioning must establish and lend that
receiver; an erased field's zero-payload layout proves no authority.

For a target-selected attached entry, root-establishment evidence binds the
selected source signature and target slot, source receiver and Terminal
attachment, field, service carrier/base and qualification, exact requirement
schema, and selected Fused plan. Derive rows after exact selection and replay
their exact sorted roster at native settlement. Targetless checking, free entries,
and same-shaped non-root owners establish no such row. Omission, duplication,
reordering, or source/carrier/attachment/plan substitution rejects. This evidence
is not a runtime slot, publication event, or Independent service handle.

## Consumer-owned settlement

Join each demanded evaluated import to one exact retained selected plan.
Provider-execution evidence and same-stack contributions require independently
supplied admission. Callers cannot choose a replacement plan/index, locator,
target, builtin, or checked-source receipt at this join. Replay the complete
plan, external row, atomic locator, target, and strong stack commitment.
Missing, extra, duplicate, or builtin-substituted demanded settlements reject.
Selected but unreachable imports retain identity without requiring execution
inputs. [Callback use](../build/private_callbacks.md) additionally needs combined callback and checked-scope
custody; neither half authorizes the other.

Native provider executions must match the exact canonical requirement before
projection into numeric execution records. Similar Unit signatures do not make
two boundaries interchangeable. Catalog selection binds exact requirement and
realization symbols, accepted package/toolchain custody, normalized signatures,
conformance, and target profile. A targetless plan cannot infer a physical target
from the compiler host.

Compiler-builtin settlement uses the representation-level
`CompilerBuiltinExecution` catalog, not the planner's classification identity.
Its conversion must be one exhaustive checked mapping returning an optional
catalog entry, not a guard and an independently constructed payload. Unclassified
roles reject. Retain complete admitted settlement content for later replay;
a commitment alone cannot reconstruct it.

Physical nonreturning behavior does not itself prove source-semantic successful
external termination. That observation needs the exact canonical ProcessExit
requirement's explicit boundary/Terminal completion and effect identity, not a
separate provider-selected name. A containment trap after an unexpectedly
returning native exit does not authorize a source crash; it contains a violated
provider premise. Exact semantic exit arguments remain separate from the target's
physical status presentation. Likewise, image bytes and provider selection do
not by themselves establish executable publication.

## Operator applications and physical children

Generic demand is not coverage. A symbolic row binds the producer package,
callable, exact operator coordinate, requirement, and declared-binder mapping.
Only final closed substitution can yield coverage. Const identity is evaluated
value in its declared carrier, not spelling. Type/const applications are the
initial supported categories; unsupported lifetime/machine categories reject.

Within a compilation, ordinary machine and selected-provider specialization
reach a fixed point over immutable authored templates. Saved ordinary calls
rejoin by exact callee template and complete canonical tuple; another direct
instantiation cannot donate call choices. Specialized concrete uses receive
the same checks as direct uses, while open template occurrences remain symbolic.
This is not cross-artifact completeness evidence.

Specialization rechecks substituted semantics, target, admission, selected plan,
and realization. Bodyless/external supply remains exact-only; bootstrap lowering
cannot publish authoritative coverage.

A cross-artifact substitution must independently join producer and operator-owner
reviews, complete producer specialization, and the consumer's reviewed closed
selected plan. Every producer binder appears exactly once and every substitution
is used by the reviewed mapping. Rejoin requirement, coordinate, application,
selected artifact, and nonzero strong plan identity; retain original mapping and
specialization for replay. Deduplication follows that join. Checking supplied
requests does not prove they exhaust reachable specializations or establish
realization, admission, Terminal/native, or installation authority. Complete-set
composition remains an independent obligation, not an install/update prerequisite.

### Closed application and physical occurrence

A boundary operator's checked application must be closed after specialization.
The source-free demand retains exact occurrence, requirement coordinate, and
ordered tagged type/constant application. The strong selected-plan identity,
semantic realization, and specialized-body, nongeneric-body, builtin, or external
authority role remain in the exactly bound companion. Replay joins both.
Source spelling, arity, optional fields, compact fingerprints, or zero commitments
cannot substitute for that evidence.

An operator with a zero-length telescope has one canonical empty application.
A boundary-trait machine has no such telescope construct; absence is not an
empty operator application. Open applications and unsupported binder categories
reject authoritative publication.

Each surviving executable boundary occurrence has exactly one physical child.
Its parent is a role-tagged choice between a reconstructible operator-application
coverage reference and a retained, replayable boundary-trait settlement.
The latter reuses `BoundaryExecutionBinding`; its builtin commitment uses
`CompilerBuiltinExecution`. Role discriminants participate in identity.

Equal applications may share a semantic parent, but surviving optimized
occurrences and their children remain distinct. The child binds the parent,
surviving operation, target, selection, assignment, relocation, and emitted-span
identities. Replay derives the surviving occurrence set from the validated
published projection. Missing, duplicate, stale, substituted, padded, or
role-swapped children reject. An independently verified eliminated occurrence
needs no child. A byte digest, plan fingerprint, application string, or dispatch
rewrite alone establishes no semantic-to-physical correspondence.

For foreign calls, replay additionally joins the exact Terminal scalar values
and types to retained source, placement, home, and materialization rows. A
direct callback joins its exact registrar occurrence, context, and private
function identity. The child spans the full semantic-code attribution interval
and exact unresolved import and callback relocations, through object/image bytes
and final image-symbol identity. Partial structural, ranked, port, or callback
custody cannot be published as complete coverage.
