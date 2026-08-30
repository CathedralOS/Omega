# Omega Package Evidence Schema

The canonical review schema is version 89 and row schema version 47. This file
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
single-coordinate selections are not inferred into a family. Exact static
application coverage is schema v87 / row v45.

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
