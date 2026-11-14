# Operator-introducer retirement inventory

NEW-OMS-OPERATOR-INTRODUCER-INVENTORY. Measured at `82741ec439` on
linux x86-64 (z181); counts and paths below were rechecked at
`3b24dc9383`. This draft inventories every surface the
"Introducer retirement" leg of **OPERATOR-MACHINE-SUPPLY** (TASKS.md)
must retire — the separate `operator` source introducer — once the
source migrations close. The token-binding law already spans both
introducers (`9913f44891`), so what remains is spelled out below:
source census, introducer surfaces, migration routing, and removal
order. Delete this inventory once the `operator` introducer,
`Item::Operator`, and `MachineSupplyMode::Boundary` are gone.

## Source census: 129 `operator`-introduced declarations

All declarations spelled with the `operator` introducer (`git ls-files
'*.omg'`; `public`-prefixed forms included):

| Bucket | Count | Where |
|---|---|---|
| `boundary operator` — library | 126 | `core/float_operations.omg` |
| plain `operator` — samples | 3 | `samples/cli/probes/domain_operator_roster/main.omg` 2; `samples/cli/probes/module_domain_path/units.omg` 1 |
| plain `operator` — library | 0 | none remain |

Recount on each landing.

## Introducer surfaces to retire

- **Parse.** `01_tokens-to-syntax-trees/src/declarations.rs`:
  `parse_operator_definition` owns the declaration and is admitted at
  the root in both the boundary and the nonboundary form. Trait
  requirements already refuse it (`parse_trait_definition`); their
  sources use `machine + name(...)`.
- **Representation.** `Item::Operator(OperatorDefinition)` in
  `representations/syntax-trees/src/declarations.rs`, with the
  consumers that must follow it out: `syntax-trees/src/copies.rs`,
  `syntax-trees/src/inspection.rs`, and in
  `02_syntax-trees-to-symbol-resolved-trees/src/` `lowering.rs` and
  `preparation_const_evaluation.rs`.
- **Resolution.** `token_bindings` in
  `02_syntax-trees-to-symbol-resolved-trees/src/lowering_machine.rs`
  enforces the duplicate token/owner/shape space across both
  introducers (alpha-normalized by first-occurrence position); the
  dual-introducer arm retires with the introducer.
- **Supply mode.** `MachineSupplyMode::Boundary` — the
  undifferentiated mode `lowering_machine.rs` still assigns — is
  referenced by 8 files under `omega-rust/psi/` and retires after the
  source migrations.

## Migration routing

- **126 tokenless library `boundary operator` rows** → `boundary
  requirement` via **TOP-LEVEL-BOUNDARY-REQUIREMENTS**.
- **3 plain `operator` rows (all in `samples/cli/probes`)** → split by
  shape: root-scoped declarations move to the `machine` introducer
  with a fixed token; domain-member declarations move to the domain's
  requirement/member route.

## Removal order

1. Migrate sources per the routing above.
2. Remove the root parse arms in `declarations.rs`, then
   `parse_operator_definition`.
3. Remove `Item::Operator` and its consumer sites.
4. Collapse the dual-introducer arm of the token-binding law.
5. Remove `MachineSupplyMode::Boundary` (8 referencing files) once
   no source path assigns it.

## Standing constraints

- Retain operation, static telescope, signature, contract, visibility
  and the selected conformance row as a canonical kind distinct from
  trait requirements; bind installed execution to the selected
  provider execution and token era.
