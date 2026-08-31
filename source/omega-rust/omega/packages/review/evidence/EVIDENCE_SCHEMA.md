# Omega Package Evidence Schema

The canonical review schema is version 98 and row schema version 56. This file
records the exact closed vocabulary whose details would otherwise obscure the
crate's architectural entrance.

Compiler-owned provider execution identity is retained independently from the
authored realization nominal. The closed review vocabulary currently covers
builtin functions, the ten primitive float binary operations in both permanent
formats, exact named-float negation formats, and named-float conversions with
explicit source type, target type, and arithmetic domain. Primitive float
execution additionally requires the exact fixed operator token; tokenless and
mismatched package lookalikes remain inadmissible. Unknown intrinsic forms
remain fail-closed until they receive a specific closed identity.

Public contract expressions retain width-landed float literals by checked
`f32`/`f64` format and exact IEEE bits. Typed named operands and the exact return
type of an owning callable contract establish the landing. Decimal source
spelling is excluded; unlanded literals remain fail-closed.

Explicit mutable and write-only reference formation in public contract
expressions retains the access mode and recursively projected target. Review
rechecks proposition arguments against the exact declared parameter type, so
access changed after checking rejects. Shared lending that is semantically
implicit remains represented by the receiving parameter type and target rather
than inventing an explicit reference-expression node. Operator-contract
rederivation also compares access modes instead of treating all borrows as the
same law expression.

Named operators called through paths such as `Token::ordered(left, right)` use
the existing structural call row. Projection rejoins the compiler's exact
named-operator resolution with the authored call-selection occurrence, retains
the package-qualified operator target, and excludes the static namespace from
the optional value receiver. Target drift and explicit reference-argument type
drift reject. This adds no new canonical atom beyond schema v84 / row v42.

Atomic loads in public contracts retain the recursively projected loaded value
and one closed checked ordering: `NoOrdering`, `Receive`, or `GlobalOrder`.
Projection requires the load form and absence of a result carrier; stores,
read-modify-write operations, swaps, compare-exchange operations, invalid load
orderings, and post-check carrier drift reject. This is schema v85 / row v43.

Public conformances targeting lifetime-parameterized traits retain every
target lifetime as an alpha-normalized ordinal in the conformance telescope.
Projection requires exact arity and declaration ownership, substitutes that
mapping through inherited requirements, and rejoins selected closed
applications with their concrete target lifetimes. Missing, undeclared, and
out-of-range forms reject. This is schema v86 / row v44.

Operator-bound external supply retains its requirement as the exact existing
package-qualified operator coordinate in the opaque-blocking executable-supply
row. Projection rejoins that coordinate with the retained overload symbol and,
when selected, the exact provider plan; checked rederivation rejects post-check
requirement drift before any trust row can be issued. The plan's compact FNV is
exposed only as `plan_report_fingerprint`; review and canonical encoding retain
the exact plan name, package owners, schema, target, rows, and declaration
coordinates, so the report value never admits a plan. Disclosure remains
distinct from provider selection and makes no audit or Terminal claim.

Explicit boundary-operator family review rows retain one exact family and
provider identity, selected target, selection authority, complete-declaration
coverage, and the canonical exact-coordinate-to-plan mapping. Independent
single-coordinate selections are not inferred into a family.

D35 retired the provider-asserted exact-application arity/string field formerly
introduced by schema v87 / row v45. Schema 97 / row 55 removes both the
`NonGeneric` and `ExactApplications` variants and keeps recovery limited to the
current version. No current record or compatibility parser may reinterpret the
retired field as D29 coverage; D29 requires compiler-derived tagged demand
joined to an independently rechecked role-specific realization.

Representation-TCB rows distinguish an unbound package-owned opaque from a
public producer candidate. Producer availability binds the exact opaque, one
package-owned public named conformance to the compiler-owned
`OpaqueRepresentation` trait, and its exact public checked-shape carrier. The
ordinary conformance row independently retains that exact interface.
Availability accepts no selection or ABI commitment and may coexist with an
unbound row. Consumer demand remains absent until the compiler can bind an
actual runtime by-value crossing to complete physical movement and finalization
evidence. This is schema v98 / row v56.

Installation-bound selected-provider rows retain the exact published service
ceiling and the exact checked realization reach beneath their existing
package-qualified requirement and realization identities. Projection rejoins
the selected resolution with the typed requirement, checked service row, and
realized contract envelope; missing, orphaned, or drifted resolutions reject.
Rendered service names remain reconciliation data and never become canonical
service identity. This is schema v88 / row v46.

Authored selected-provider grants are retained on the exact selected provider
row as `PlanName` or `ProviderSlot` plus the collision-resistant digest of the
complete retained plan. Projection rejoins every grant to one selected plan,
requires the exact selected build machine, and retains the authored
`build.omg` occurrence as `ProviderGrant` source custody. Selector strings and
compact report fingerprints grant nothing. This is schema v89 / row v47;
canonical-row recovery v14 adds the source role.

Checked operator realizations may publish crash behavior only within the
operator's declared crash ceiling. The compiler groups routes by exact crash
cause, substitutes operator parameters with their realization parameters, and
requires every provider route to be an exact member of the operator route set.
An unconditional operator route admits any provider route for that cause;
an unconditional provider route requires an unconditional operator route.
Omitted provider causes narrow the contract, while undeclared causes and
stronger routes reject. Projection reruns this judgment and retains the already
complete operator, callable, and checked-crash rows. This is schema v90 / row
v48; no new canonical expression atom or recovery grammar is introduced.

Checked contracts may select an exact nominal member from a computed receiver
that is itself representable by the closed contract-expression vocabulary.
Projection recursively retains that receiver, requires exactly one finalized
public-interface member selection, rejoins it to the typed member symbol, and
derives any case variant from the selected declaration. Selection drift and
duplicate or missing custody reject. The existing structural member atom
already represents the result; this acceptance-boundary expansion is schema
v91 / row v49 and introduces no recovery-grammar revision.

Compiler-owned collection views in public contracts retain one closed call
target: shared slice, mutable slice, text view, or bytes. Projection requires
one public-interface call selection and one retained checked intrinsic fact,
then reruns the compiler's exact owner/type-sensitive derivation and requires
equality. Missing, duplicate, redirected, or stale call custody rejects;
same-spelled package callables remain nominal. The existing structural call
expression owns receiver and argument identity. The new target vocabulary is
schema v92 / row v50; canonical-row recovery remains v14. This does not widen
the call compositions accepted by checking; it retains collection views only
inside public facts that already pass the compiler's denotational rules.

A named public const supplied to a const-generic contract call retains one
exact public-interface static-argument selection to the const declaration.
Projection rejoins that symbol to exactly one checked public const and decodes
its closed canonical integer encoding; neither the source identifier nor the
diagnostic display becomes value identity. A private const exposed through a
public contract, a missing canonical declaration value, a non-integer value,
or a changed selection rejects. This acceptance expansion is schema v93 / row
v51 and introduces no new expression atom or recovery-grammar revision.

Nongeneric, lifetime-free fixed-token boundary operators backed by one exact
selected `CheckedAdapter` may enter package review only when their declaration
has a dispatch-supported shape. The closed local gate admits arithmetic and
comparison tokens with exactly two normalized operands, plus two-operand
indexing; range and every other token/arity shape remain fail-closed. The
existing selected-plan join still binds the exact operator coordinate to the
checked adapter. External, aliased, bodyless, generic, and lifetime-bearing
neighbors remain outside this admission slice. This acceptance-boundary
expansion is schema v94 / row v52 and introduces no new canonical row atom or
recovery-grammar revision.

External executable-supply rows may also bind a bodyless, nongeneric,
lifetime-free external realization to one exact public top-level boundary
requirement. The tagged requirement identity is the requirement's normalized
machine-overload identity, not its source spelling. Projection rejoins the
typed satisfies edge, structural binding, provider type, selected plan when
present, exact requirement and realization declarations, and authored `via`
custody. Unselected leaves remain disclosed without implying selection;
compiler-intrinsic execution remains fenced pending its closed catalog. The
new requirement tag is schema v95 / row v53; canonical-row recovery remains
v14.

Evidence-bearing calls accepted inside public callable contract expressions
retain each erased lane as an exact source-to-callee-parameter binding. The
checked producer keys custody by semantic proof-fact owner, exact proof fact,
and expression-call occurrence, then substitutes ordinary call arguments into
the callee proposition before accepting the source term. Package projection
rejoins that occurrence and emits only package-qualified term owners, contract
kinds, and lane ordinals; local evidence aliases, symbols, and arena handles
are excluded. Missing, duplicate, redirected, rebound, or lane-drifted custody
rejects. This is schema v96 / row v54; canonical-row recovery remains v14.
