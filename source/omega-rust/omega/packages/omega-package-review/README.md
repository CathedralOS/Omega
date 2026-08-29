# Omega Package Review

This crate projects compiler-owned checked package facts into a stable,
source-handle-free review vocabulary. It also owns canonical comparison-row
encoding, row recovery, and the ordinary review-obligation ledger. Review output
is not package admission, an accepted lock, or proof that an audit occurred.

## Source map

```text
src/
|-- lib.rs                    public entrance and explicit reexports
|-- model/                    stable review vocabulary; no compiler traversal
|   |-- identity.rs           package/toolchain nominal identity
|   |-- signatures.rs         public signature and generic parameter shapes
|   |-- public_api.rs         domain and data API shapes
|   |-- contracts.rs          contract, proposition, and expression vocabulary
|   |-- authority.rs          reach, capability, mutation, crash, and termination
|   |-- projection.rs         complete review aggregate and internal row pairings
|   `-- rows.rs               canonical row and source-coordinate carriers
|-- projection/               checked compiler state -> review model
|   |-- aggregate.rs          total projection entry and final assembly
|   |-- evidence.rs           retained evidence, authority classes, and sources
|   |-- public_traits.rs      public trait and conformance projection
|   |-- public_api.rs         public domain, proposition, const, operator, and data
|   |-- callables.rs          callable envelope projection
|   |-- contracts/            contract metadata, propositions, and expressions
|   |-- providers.rs          conformance and provider/external-supply joins
|   |-- provider_intrinsics.rs compiler-owned provider execution identity
|   |-- operational.rs        reach, invocation, mutation, crash, and flow rows
|   |-- exact_identity.rs     exact nominal, type, lifetime, and owner identity
|   `-- exact_identity/tests.rs focused owner/type identity unit tests
`-- encoding/                 canonical persistence boundaries
    |-- canonical.rs          framing, row assembly, limits, and primitive encoder
    |-- values.rs             semantic value encoding
    |-- recovery.rs           canonical-row recovery envelope
    `-- obligation_ledger.rs  local reconstruction ledger

tests/
|-- support/                  shared package-compilation fixtures
|-- obligation_ledger.rs      ledger reconstruction and closure custody
|-- source_custody.rs         row/source pairing and retained evidence
|-- boundary_supply.rs        external executable and boundary supply
|-- authority.rs              dangerous authority and accepted claims
|-- proposition_contracts.rs  proposition and proof-static contracts
|-- contract_expressions.rs   contract calls, members, casts, and signatures
|-- operators.rs              operator realization and provider joins
|-- conformance.rs            generic and public conformance projection
|-- operational.rs            reach, invocation, source custody, and targets
|-- public_api.rs             public data, domain, quotient, and trait shapes
|-- exact_contract_identity.rs compiler intrinsics and exact checked selections
|-- trait_contracts.rs        trait operational and contract envelopes
`-- package_review_row_recovery.rs
```

## Dependency direction

`projection` reads compiler-owned checked state and constructs `model` values.
`encoding` reads `model` values but never compiler IR. `model` neither traverses
compiler state nor depends on persistence. Recovery decodes canonical rows into
a distinct inert type, and the obligation ledger requires local compiler
reconstruction before recovered rows can be compared.

The crate root exports the stable external surface. Cross-responsibility
construction helpers and fields remain `pub(crate)` and are not external API.
Compiler-owned provider execution identity is retained independently from the
authored realization nominal. The closed review vocabulary currently covers
builtin functions, exact named-float negation formats, and named-float
conversions with explicit source type, target type, and arithmetic domain.
Unsupported intrinsic forms remain inadmissible until they receive a closed
identity.

The canonical review schema is version 80 and row schema version 38.
