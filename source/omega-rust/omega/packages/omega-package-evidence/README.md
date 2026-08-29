# Omega Package Evidence

This crate projects compiler-owned checked package facts into a stable,
source-handle-free evidence vocabulary. It owns canonical comparison-row
encoding, row recovery, and the ordinary package-obligation ledger. Its output
is inert compiler evidence—not package admission, an accepted lock, or proof
that an audit occurred. Review workflow and root policy belong to
`omega-package-manager::review`.

## Source map

```text
src/
|-- lib.rs                    projection entrance and four responsibility owners
|-- evidence/                 public stable compiler-issued evidence vocabulary
|   |-- identity.rs           package/toolchain nominal identity
|   |-- signatures/           types, callables, traits, and external supply
|   |-- domains.rs            domain identities, roles, aliases, and establishment
|   |-- data.rs               data shapes, members, and declared properties
|   |-- representation.rs     opaque representation trust commitments
|   |-- contracts/            expressions, propositions, declarations, and callable contracts
|   |-- authority.rs          reach, capability, mutation, crash, and termination
|   |-- package/              callables, providers, package aggregate, and source pairings
|   `-- rows.rs               canonical row and source-coordinate carriers
|-- projection/               private checked compiler state -> package evidence
|   |-- orchestration/        validation, surface collection, providers, and assembly
|   |-- authority.rs          reached authority and intrinsic risk classes
|   |-- representation.rs     semantic dependencies and representation TCB
|   |-- source/               compiler-private pairings and final canonical source custody
|   |-- api/                  public domains, data, propositions, constants, operators, traits, and conformances
|   |-- callables.rs          callable envelope projection
|   |-- contracts/            checked facts, propositions, and expressions
|   |   |-- checked/          evidence, operations, parameters, reach, and source custody
|   |   `-- expressions/      calls, members, constructors, names, and operators
|   |-- providers/           selection, installation, families, intrinsics, conformances, and external supply
|   |-- behavior/             reach, invocation, mutation, crash, termination, and flow rows
|   `-- semantics/            declarations, types, signatures, facts, and conformances
|-- encoding/                 public canonical persistence boundaries; no compiler IR
|   |-- encode/               framing, row assembly, limits, and primitive encoder
|   |-- values/               semantic value encoding by evidence family
|   `-- decode/               canonical-row framing, source recovery, and decoding
`-- obligations/             public local reconstruction question; codec remains private

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
canonical rows into a distinct inert type. `obligations` owns the separate
local compiler reconstruction that must precede comparison of recovered rows;
its codec is an implementation detail of that domain, not an encoding owner.

The crate root exports only `project_checked_package_review`, the natural
operation entrance. Stable values are addressed through `evidence`, canonical
persistence and recovery through `encoding`, and local reconstruction through
`obligations`. Their implementation modules, cross-responsibility
construction helpers, and fields remain private or `pub(crate)` rather than
forming a second flattened API.

## Canonical schema

The canonical review schema is version 88 and row schema version 46. Its exact
closed vocabulary and revision notes live in
[`EVIDENCE_SCHEMA.md`](EVIDENCE_SCHEMA.md); they are persistence documentation,
not the reader entrance to this crate.
