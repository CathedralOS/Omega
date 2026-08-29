# Omega Package Review

This crate projects compiler-owned checked package facts into a stable,
source-handle-free review vocabulary. It also owns canonical comparison-row
encoding, row recovery, and the ordinary review-obligation ledger. Review output
is not package admission, an accepted lock, or proof that an audit occurred.

## Source map

```text
src/
|-- lib.rs                    operation entrance; exports only project_checked_package_review
|-- evidence/                 public stable compiler-issued review vocabulary
|   |-- identity.rs           package/toolchain nominal identity
|   |-- signatures/           types, callables, traits, and external supply
|   |-- public_api.rs         domain and data API shapes
|   |-- contracts/            expressions, propositions, declarations, and callable contracts
|   |-- authority.rs          reach, capability, mutation, crash, and termination
|   |-- review/               callables, providers, package aggregate, and source pairings
|   `-- rows.rs               canonical row and source-coordinate carriers
|-- projection/               private checked compiler state -> review evidence
|   |-- orchestration/        validation, surface collection, providers, and assembly
|   |-- authority.rs          reached authority and intrinsic risk classes
|   |-- semantics.rs          semantic dependencies and representation TCB
|   |-- source_custody/       bounded source coordinates and final row/source pairing
|   |-- public_api/           public domains, data, propositions, constants, operators, traits, and conformances
|   |-- callables.rs          callable envelope projection
|   |-- contracts/            checked facts, propositions, and expressions
|   |   |-- checked/          evidence, operations, parameters, reach, and source custody
|   |   `-- expressions/      calls, members, constructors, names, and operators
|   |-- providers/           selection, families, intrinsics, conformances, and external supply
|   |-- operational/          reach, invocation, mutation, crash, termination, and flow rows
|   `-- checked_semantics/    declarations, types, signatures, facts, and conformances
|-- encoding/                 public canonical persistence boundaries; no compiler IR
|   |-- canonical/            framing, row assembly, limits, and primitive encoder
|   |-- values/               semantic value encoding by evidence family
|   `-- recovery/             canonical-row framing, source recovery, and decoding
`-- obligation_ledger/       public local reconstruction question; codec remains private

tests/
|-- support/                  shared package-compilation fixtures
|-- authority.rs             thin entrance; cases live in authority/
|-- boundary_supply.rs        thin entrance; cases live in boundary_supply/
|-- conformance.rs            thin entrance; cases live in conformance/
|-- contract_expressions.rs   thin entrance; cases live in contract_expressions/
|-- exact_contract_identity.rs thin entrance; cases live in exact_contract_identity/
|-- operational.rs           thin entrance; cases live in operational/
|-- operators.rs              thin entrance; cases live in operators/
|-- proposition_contracts.rs  thin entrance; cases live in proposition_contracts/
|-- public_api.rs             thin entrance; cases live in public_api/
|-- trait_contracts.rs        thin entrance; cases live in trait_contracts/
`-- remaining focused integration targets
```

## Dependency direction

`projection` reads compiler-owned checked state and constructs `evidence`
values. `encoding` reads `evidence` values but never compiler IR. `evidence`
neither traverses compiler state nor depends on persistence. Recovery decodes
canonical rows into a distinct inert type. `obligation_ledger` owns the separate
local compiler reconstruction that must precede comparison of recovered rows;
its codec is an implementation detail of that domain, not an encoding owner.

The crate root exports only `project_checked_package_review`, the natural
operation entrance. Stable values are addressed through `evidence`, canonical
persistence and recovery through `encoding`, and local reconstruction through
`obligation_ledger`. Their implementation modules, cross-responsibility
construction helpers, and fields remain private or `pub(crate)` rather than
forming a second flattened API.
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

Operator-bound external supply retains its requirement as the exact existing
package-qualified operator coordinate in the opaque-blocking executable-supply
row. Projection rejoins that coordinate with the retained overload symbol and,
when selected, the exact provider plan; checked rederivation rejects post-check
requirement drift before any trust row can be issued.
The plan's compact FNV is exposed only as `plan_report_fingerprint`; review and
canonical encoding retain the exact plan name, package owners, schema, target,
rows, and declaration coordinates, so the report value never admits a plan.
Disclosure remains distinct from provider selection and makes no audit or
Terminal claim.

Explicit boundary-operator family review rows retain one exact family and
provider identity, selected target, selection authority, complete-declaration
coverage, and the canonical exact-coordinate-to-plan mapping. Independent
single-coordinate selections are not inferred into a family.

The canonical review schema is version 85 and row schema version 43.
