# Omega Package Review

This crate projects compiler-owned checked package facts into a stable,
source-handle-free review vocabulary. It also owns canonical comparison-row
encoding, row recovery, and the ordinary review-obligation ledger. Review output
is not package admission, an accepted lock, or proof that an audit occurred.

## Source map

```text
src/
|-- lib.rs                    public entrance and explicit reexports
|-- evidence/                 stable compiler-issued review vocabulary
|   |-- identity.rs           package/toolchain nominal identity
|   |-- signatures.rs         public signature and generic parameter shapes
|   |-- public_api.rs         domain and data API shapes
|   |-- contracts.rs          contract, proposition, and expression vocabulary
|   |-- authority.rs          reach, capability, mutation, crash, and termination
|   |-- projection.rs         complete review aggregate and internal row pairings
|   `-- rows.rs               canonical row and source-coordinate carriers
|-- projection/               checked compiler state -> review evidence
|   |-- aggregate.rs          total projection entry and final assembly
|   |-- evidence/             retained evidence, authority classes, and sources
|   |   |-- mod.rs            evidence-projection facade
|   |   |-- semantic_projection.rs semantic dependencies and representation TCB
|   |   |-- dangerous_authority.rs reached authority and intrinsic risk classes
|   |   |-- selected_providers.rs exact provider ownership and source custody
|   |   |-- row_finalization.rs canonical row/source assembly
|   |   `-- source_locations.rs bounded canonical source coordinates
|   |-- public_traits.rs      public trait and conformance projection
|   |-- public_api/           public domains, propositions, constants, operators, and data
|   |-- callables.rs          callable envelope projection
|   |-- contracts/            contract metadata, propositions, and expressions
|   |   |-- metadata/         checked contract evidence, operations, and service reach
|   |   `-- expressions/      calls, members, constructors, names, and operators
|   |-- providers/           conformance, boundary selection, and external-supply joins
|   |-- provider_families.rs  atomic operator-family selection reconciliation
|   |-- provider_intrinsics.rs compiler-owned provider execution identity
|   |-- operational.rs        reach, invocation, mutation, crash, and flow rows
|   `-- exact_identity/       exact nominal, type, lifetime, and owner identity
`-- encoding/                 canonical persistence boundaries
    |-- canonical/            framing, row assembly, limits, and primitive encoder
    |-- values/               semantic value encoding by evidence family
    |-- recovery/             canonical-row framing, source recovery, and decoding
    `-- obligation_ledger/    construction, validation, and canonical ledger codec

tests/
|-- support/                  shared package-compilation fixtures
|-- boundary_supply.rs        thin entrance; cases live in boundary_supply/
|-- conformance.rs            thin entrance; cases live in conformance/
|-- contract_expressions.rs   thin entrance; cases live in contract_expressions/
|-- exact_contract_identity.rs thin entrance; cases live in exact_contract_identity/
|-- operators.rs              thin entrance; cases live in operators/
|-- proposition_contracts.rs  thin entrance; cases live in proposition_contracts/
|-- public_api.rs             thin entrance; cases live in public_api/
`-- remaining focused integration targets
```

## Dependency direction

`projection` reads compiler-owned checked state and constructs `evidence`
values. `encoding` reads `evidence` values but never compiler IR. `evidence`
neither traverses compiler state nor depends on persistence. Recovery decodes
canonical rows into a distinct inert type, and the obligation ledger requires
local compiler reconstruction before recovered rows can be compared.

The crate root exports the stable external surface. Cross-responsibility
construction helpers and fields remain `pub(crate)` and are not external API.
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

Explicit boundary-operator family review rows retain one exact family and
provider identity, selected target, selection authority, complete-declaration
coverage, and the canonical exact-coordinate-to-plan mapping. Independent
single-coordinate selections are not inferred into a family.

The canonical review schema is version 84 and row schema version 42.
