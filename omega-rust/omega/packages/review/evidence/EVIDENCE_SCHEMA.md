# Omega Package Evidence Schema

The canonical review schema is version 130 and row schema version 88. This file
records the closed vocabulary whose details would otherwise obscure the
crate's architectural entrance. Proof-interface sections follow the
[contract/bundle direction](../../../../../wiki/spec/proofs/contracts.md)
and require encoding migration; they are not a claim that replacement proof
rows already exist in version 130. The source/codec changes and exact-version
rejection controls belong to `PROOF-CONTRACT-MIGRATION`.

This describes the current encoding and support limits, not a
requirement to certify installation. The ratified lock records pins, the graph,
accepted compiler-derived review baselines, and decisions, trusting whoever
lands it. Existing root-admission tags and ledger replay remain documented
implementation while redundant promotion machinery is simplified; no sealed
`PackageInstance` or certificate proving lock acceptance is required. Proof
certificates below answer actual compiler proof questions. Compiler proof/reach
checks and native artifact validation remain independent of installation.

The separate external-supply policy component uses
`OMEGA-EXTERNAL-SUPPLY-POLICY` version 2, bounded to 4 MiB. It preserves the
complete callable and requirement coordinates, all eight binding alternatives,
the exact four foreign-locator forms, target, and producer identities. It omits
evaluation accounting, evaluator/materializer schema markers, closure/evaluation/
materialization digests, and reconstruction receipts. Its signature and
requirement encoders use normalized policy signatures rather than the lossy
legacy review signature. Results are optional, static machine contracts retain
typed crash guards and full progress routes, and trait requirements keep actual
lifetime arguments as well as their equality partition. Changes to nested
encodings must also version the policy component. Typed recovery restores the
complete nested signature, machine-contract, logical-fact, and expression
vocabulary, checking exact re-encoding without reconstructing compiler evidence.
Normalized bindings retain exact versioned target-profile identities from
checked evaluation, not deployment names or CLI aliases. The writer and reader
limit aggregate list elements plus recursive entries to 65,536 and nesting to
128. Recovery additionally limits individual fields to 4 MiB and requested
vector/string/box storage, including canonical comparison scratch, to 64 MiB;
callers may lower but cannot raise these
ceilings. Allocator overhead is outside that storage accounting. The complete
policy baseline below includes this component; the manager joins that baseline
to source pins and accepted decisions. This component does not change the
full-review schema or any compiler validator.

`OMEGA-PACKAGE-POLICY` version 1 composes the full inert package baseline under
those same aggregate ceilings. Its field order is package, target, public API
(traits, conformances, domains, consts, operators, data), callables,
selected providers, terminal permissions, representation, external supplies,
dangerous capabilities, slack, semantic dependencies, and D29 applications
(symbolic demands, closed realizations). Child components share the enclosing
writer/reader directly; they do not embed component envelopes or reset budgets.
The callable, calling, selected-provider, terminal-permission, representation,
and external-supply component schemas are version 2 for complete nested policy
signatures. Conformance and physical-calling component schemas remain version 1.
The full-review, row, and canonical-row recovery versions remain unchanged.
Unknown policy versions reject; recovery does not invent missing meaning from
old signatures. Source pins, graph edges and project decisions belong to the
manager's lock envelope, not this policy record. Compiler proof/discharge and
row-source custody are not policy identity or substitutes for fresh admission
validation.

The complete baseline also has text format version 1, introduced by the exact
header `omega_package_policy_text 1` followed by LF. It names the binary format
and baseline schema before the package-policy record. `field <name> { ... }`
and `record <name> { ... }` delimit named structure; static names use lowercase
ASCII letters, digits and underscores. `sequence <count> { item { ... } ... }`
retains count and order. `option none` and `option some { ... }` distinguish
absence from a supplied value. `tag <variant> <byte>` names each closed variant
without changing its binary tag. Numeric primitives use their explicit width
and canonical decimal spelling; booleans use `true` or `false`.

Strings and byte fields use quoted ASCII with `\"`, `\\`, and lowercase
`\xhh` escapes; non-ASCII UTF-8 bytes remain lossless. Fixed identity digests use
64 lowercase hexadecimal digits. Indentation is two spaces per markup scope,
every row ends in LF, and the complete named traversal determines field order.
Recovery reconstructs the same binary scalar stream, performs existing typed
recovery, then requires an exact streaming text rerender. Labels and container
boundaries are validated grammar, not ignorable comments over opaque bytes.
The text ceiling is 32 MiB, markup depth is 1,024, and the existing 4 MiB binary,
65,536 aggregate element, 128 semantic depth and 64 MiB owned-storage ceilings
remain. Reconstructed binary capacity is charged before typed recovery and its
canonical scratch; verification does not allocate another expanded text buffer.
This adds no proof, acceptance, or replay fields and changes no binary schema.

Complete normalized comparison rows have their own version 1, independent of
legacy review row version 88. Binary rows start with
`OMEGA-PACKAGE-POLICY-ROW` and a zero byte; named text starts with
`omega_package_policy_row_text 1` and LF. Each row binds its row and baseline
schemas, package, exact target, kind, initial/update decision classification,
audit classifications, and full policy value. The separate semantic key selects
the declaration or application being compared; it is not a report index.

The closed kinds are header; the seven public declaration families; callable;
selected-provider association; terminal service and permission; representation
target, declaration, availability, selection and demand; external supply;
dangerous capability and slack; semantic dependency; and symbolic boundary
demand. The selected-provider association contains complete plans, families,
and closed D29 realizations together, preserving their internal canonical plan
links. Duplicate kind/key coordinates reject. No legacy row or whole-baseline
encoding is changed, and row text is not a whole-baseline recovery envelope.

One projection permits at most 65,536 rows, 128 MiB of requested row-table and
retained buffer storage, 1,048,576 sequence elements and recursive entries across
sizing and emission, and semantic depth 128. Per-row ceilings are 1 MiB of key,
4 MiB of canonical binary, and 32 MiB of text. Caller ceilings may only lower
these bounds. The row table and key/binary/text output are measured and charged
before allocation; usage permits one aggregate budget across both compared
graphs. Allocator overhead and the already borrowed typed policy are excluded.

The ordinary obligation-semantics schema is version 7. Its result
vocabulary explicitly leaves bodyless accepted claims, dangerous authorities,
external executable supplies, and exact terminal-authority permissions open
for root admission, and compiler-retained contract-entailment stand-downs open
for later discharge. The outer ledger encoding remains version 2; no new
persistence authority is introduced.

## Current capture support

Source/declaration custody and structural nominal joins are described in
[capture.md](capture.md). The table below describes the current support boundary,
not a list of past schema increments. A supported declaration is not automatically
a supported selected application, executable call, or native realization.

### Contract expressions and applications

| Form | Current identity and checks |
| --- | --- |
| Pure call / member path | One exact public-interface selection joined to checked owner environment. Retain receiver ordinal and qualified case/field chain, or optional receiver, exact entry target, and arguments. No global name fallback; helper bodies remain source-content identity. |
| Contract static argument | Exact selected callee telescope/category: concrete type, canonical const, caller binder ordinal, or exact machine entry. Recursive generic data applications retain checked base/telescope and caller lifetime ordinals; forwarded type/const binders rejoin caller categories. True nested machine applications and general mathematical arguments remain unsupported. |
| Byte literal | Typed decoded octets without an invented text encoding; escape-equivalent spelling is stable. |
| Array literal | Ordered recursive elements; an unsupported child rejects the complete row. |
| Record/case constructor | Exact qualified data, case, fields and recursive values, sorted by semantic field identity rather than source order. |
| Index/range | Exact public-interface operator occurrence, builtin/declared meaning, collection, index/endpoints, and inclusive-end bit. |
| Cast | Structural operand, target, arithmetic policy, qualified domain/arguments, and value/recast form; no private domain exposure through a public contract. |
| `zero_value<T>()` | Exact qualified/alpha-normalized observed type, not layout bytes or a checker verdict. Quotients remain outside the representation-observer contract. |
| Collection length | Exact `len` selection on fixed array/slice after preferring an actual nominal field. Package fields keep their owner. |
| Unary `!` / `~` | Authored operator occurrence finalized to checked builtin meaning before projection. |
| Byte predicates / builtin functions | Exact closed predicate or fixed root builtin slot/kind. Same-spelled package/nested/generated declarations stay nominal; malformed receivers, static arguments, or symbol joins reject. |
| Named operator call | Exact operator resolution and authored occurrence; static namespace is not a value receiver. Target or reference-argument drift rejects. |
| Explicit mutable/write-only reference | Access mode plus recursively projected target and exact declared parameter type. Implicit shared lending uses receiving type and target, not a fabricated explicit node. |
| Atomic load | Recursive loaded value and closed ordering `NoOrdering`, `Receive`, or `GlobalOrder`; requires load form with no result carrier. Stores, RMW, swap, compare-exchange, invalid ordering, and carrier drift reject. |
| Float literal | Checked `f32`/`f64` format and exact IEEE bits, landed by typed operands or the owning result contract. No decimal display or unlanded literal. |
| Computed-receiver member | Receiver must fit the closed expression vocabulary; one finalized member selection rejoins the typed declaration and determines case identity. |
| Collection view | Shared slice, mutable slice, text view, or bytes: public call selection, retained intrinsic fact, and fresh owner/type-sensitive derivation must agree. Lookalikes stay nominal. |
| Named scalar const | Exact public static-argument selection, checked declaration/carrier, and canonical integer or Boolean value; Boolean requires the compiler-owned carrier. |
| Named structured const | Acyclic monomorphic checked record/pure sum recursively built from integers, Booleans, literal fixed arrays, and the same data cohort. Rerun syntax-free canonical validation against the declared carrier. Top-level arrays, generic/recursive carriers, private selection, and embedded-name spoofing reject. |
| Closed conformance argument | Exact owner, fact, call occurrence, static ordinal, and complete application. Reclose authored selection and compare declaration, ordered arguments, subject, and target-trait application. Type arguments, integer const literals, caller const binders, and selected supported public scalar/structured consts are retained. The target-trait telescope remains type-only in this lane; lifetime, machine, and general mathematical arguments remain unsupported. |

Missing, duplicate, redirected, or stale selection custody rejects. These forms
retain already-checked public facts; they do not widen the checker's denotational
call grammar. Executable evidence projections and nested executable machine
applications are not admitted by adding a review row. Contract/bundle migration
must preserve exact occurrence, substitution, law/member, and witness joins;
replacement encodings remain `PROOF-CONTRACT-MIGRATION` work.

Outcome-specific guarantees retain exact result-data/case identity and canonical
fact joined to their checked guarded owner. Ordering is not meaning, but moving
a fact to another case is. They cannot collapse to unconditional postconditions.
Each non-crash public-operator fact rejoins one exact operator owner row. Operator
crash tables must match fresh derivation, including crash-free entries; cause/guard
sorting and deduplication preserve identity, not changed guards.

Public consts retain declared carrier and supported canonical value even when
unused. Public operators retain exact overload coordinate, boundary status, fixed
spelling, signature, and declaration contracts independently of uses. Whole public
conformances retain declaration-owned visibility, normalized telescopes, subject,
trait application, and inherited checked requirements. Member bodies/states and
inline/reference/default choices do not enter public conformance identity; their
signatures and substituted laws still validate. A requirement-local satisfier and
its optional grouping alias do not become a whole-conformance declaration.
Explicit/inferred selections keep visibility and direct-dependency admission.

Public conformance target lifetimes use declaration-order ordinals into the
public conformance telescope, with arity/in-scope checks and substitution through
inherited requirements. Exact-requirement realization edges instead retain raw
machine ordinals privately for substitution and expose the first-occurrence
equality partition in target-trait order for legacy provider/review identity.
Repeated arguments are valid. Private renaming, reordering, or unused binders
must not change that partition. The complete policy signature retains actual
lifetime arguments as well as the partition; it must not reconstruct them from
the lossy legacy row.

### Operator realizations and D29 applications

Checked realizations rederive exact signature-directed selection and the
structural equality/`&&` contract judgment. Crash refinement groups by exact
cause and substitutes parameters: every provider route must belong to the
operator route set. An unconditional operator route admits any provider route
for that cause; an unconditional provider route needs an unconditional operator
route. Omitted causes narrow; added causes or stronger routes reject. This is
structural containment, not a general implication prover. Outcome-specific
operator contracts remain unsupported until their refinement rules exist.

Supported local fixed-token checked adapters cover binary arithmetic/comparison
and two-operand indexing, using the same exact overload coordinate as the named
surface. Range, unsupported arities, lifetime/static-machine applications,
bodyless/external generic realizations, and unsupported alias paths reject.
Checked type/const-generic declarations retain both complete positional
telescopes; categories and const carriers are exact, and provider type-property
bounds may weaken but not strengthen the requirement. Declaration evidence is
not coverage.

Actual applications have distinct role payloads:

- Monomorphic checked bodies bind the exact overload plus explicit empty
  application, strong selected-plan identity, realization machine/entry state,
  and freshly checked machine-contract commitment.
- Type/const checked-body applications retain ordered qualified type/canonical
  const arguments, selected plan, generic template, concrete machine/state,
  specialization commitment, and contract commitment. Psi derives applications
  from authored operands; ordinary-helper and provider specialization alternate
  to a local fixed point. The authored generic API remains intact.
- Monomorphic compiler intrinsics bind the checked use, strong plan, exact
  requirement, aligned provenance, and rederived closed execution identity.
  They do not fabricate a checked-machine contract.
- Producer-side symbolic demands bind the exact public callable, complete
  operator-overload coordinate, stable requirement identity, authored locations,
  and direct operator-type-binder to callable-type-binder mapping. Named
  type-only uses are the bounded producer. Private, nested symbolic, const,
  lifetime, machine, fixed-token, and statement forms are absent or unsupported.

Equal applications deduplicate only after complete equality while retaining all
authored locations. Missing/open/substituted applications, stale specialization,
and category/carrier drift reject. Foreign symbolic substitution, unsupported
Terminal continuations, and native physical coverage remain downstream work.
No role claims package acceptance, an audit, or the complete reachable set from
a provider assertion.

The closed intrinsic vocabulary includes builtin functions, ten primitive float
binary operations across both permanent formats, named-float negation, and
conversions retaining source/target/domain. Primitive float execution additionally
requires the exact fixed token. A generic external operator cannot be published
as an executable template when no catalog entry or application replay consumes
it.

### External executable supplies

Each external key is self-contained for callable shape: exact signature,
lifetime/static telescopes, parameter modes/types, result, and one tagged trait,
operator-overload, or top-level requirement application. Supported authored
aliases are separate key fields, not declaration renamings. Public and private,
selected and unselected leaves remain disclosed. Missing/duplicate supply,
malformed payload, detached table-field type, or requirement/plan drift rejects.

Top-level external requirements support ordinary type, const, static-machine,
and conformance-bound telescopes under fresh recursive refinement. Type-property
bounds may weaken but not strengthen; const carriers and positional uses remain
exact. Every provider conformance demand matches a distinct requirement demand,
preserving evidence-binder presence, subject ordinal, trait, selected application,
lifetimes, and structural arguments. Ignore only local evidence-binder numbering.
Both complete telescopes remain identity even when a demand is omitted.

Evaluated imports require a bijection between retained typed `via` expressions
and the complete evaluated table, including private/unselected leaves. Legacy
review retains the exact target, atomic locator, producer package/callable/closure,
evaluator semantics and measured usage, evaluation result, materializer
schema/result, and locator/aggregate commitments. Every axis affects legacy
review bytes. The normalized policy component intentionally excludes these
execution receipts, as specified above. Atomic locators retain all four forms,
including Mach-O install-name plus dyld symbol.

Known Linux boundary intrinsic rows include distinct exit-process and write-byte
catalog identities. Exact package/toolchain source, declaration, signature,
conformance, plan, and target-origin joins precede physical realization. A
targetless inferred row cannot close physical execution. Unit result or a
provider-known nonreturning implementation does not establish semantic terminal
completion. That separate unresolved contract is owned by
[Terminal observations](../../../../../wiki/spec/terminal-psi/observations.md).
The bounded write-byte native path binds the exact scalar source and syscall
materialization span; constants and preceding internal Unit-call homes are
supported, while incoming parameters require their own replayable ABI ledger.
These are target-specific execution limits, not package-review authority.

### Selection, permission, and representation rows

Selected-provider rows retain exact published and checked realization reach.
Grants join one complete selected plan by strong identity and retain authored
build custody. Compact report fingerprints and selector strings are not admission
identity. Explicit operator families retain family/provider/target/authority,
complete declaration coverage, and exact coordinate-to-plan mapping; unrelated
individual choices do not imply a family.

A terminal-permission row retains exact service, complete schema identity,
requirement, and canonical permitted classes. Consumer-policy provenance does
not imply the declaring package granted itself permission. It records a grant,
not an exercised or admitted terminal mechanism. Transitional broad summaries
grant nothing.

Representation records keep these cases separate:

- Unbound package-owned opaque declaration.
- Public producer availability: opaque, named conformance to the exact compiler
  interface, and checked public carrier; no consumer choice or ABI claim.
- Selected inert semantic-copy application: freshly harvested authoritative
  selection, structurally copyable carrier graph, complete conformance and
  strong selection commitments; no native-copy or actual-use claim.
- Actual by-value consumer demand: target-closed requirement application,
  complete shape graph, exact opaque occurrences/paths, replay-validated
  placement, calling policy, and strong selection/boundary-plan identities.

The selecting package owns its selections/demands even for foreign opaques.
Unused/reference-only choices do not create by-value demand. Capture records
shape roots during materialization rather than guessing from equal layouts.
Representation changes recommend audit; they do not assert native verification.
See the [representation contract](../../../../../wiki/spec/build/opaque_representations.md).

### Proof obligations and non-executable quotient rows

A contract-entailment stand-down binds callable and contract/fact coordinates,
complete machine-contract commitment, projected goal, and closed reason. Equal
goals at distinct positions remain distinct. It stays open for later discharge
and cannot be accepted by root policy.

Its separate discharge row binds the original obligation, exact callable and
coordinates, strong contract, ordered kernel assumptions, canonical goal, and
deterministic selected assumption. Fresh capture and ledger replay reconstruct
the semantic question and invoke the kernel. Missing, stale, reordered, or
changed evidence rejects or leaves the obligation open; the stand-down remains
visible. Neither row certifies a lock or grants execution.

The non-executable quotient entrance reruns the transactional extractor, checks
exact batch equality, and selects a nonempty requested-package subset of public
total direct `define` or position-preserving direct transport-backed `lift`.
Rows retain complete callable/application identities, positional relations,
theorem roles/applications/contract positions, eligibility, and direct result
coordinate. Source custody is the authored public operation, not synthesized
typed calls. Two-argument lift, adaptation, literal/permuted/repeated arguments,
generic/private/unselected/wrong-package or drifted batches remain unsupported,
as do ordinary executable quotient requests. This bounded row is not a claim
that the mathematical-proof migration is complete.
