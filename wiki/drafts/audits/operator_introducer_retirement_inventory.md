# Operator-introducer retirement inventory

NEW-OMS-OPERATOR-INTRODUCER-INVENTORY. Measured at `82741ec439` on
linux x86-64 (z181). This draft inventories every surface the
"Introducer retirement" leg of **OPERATOR-MACHINE-SUPPLY** (TASKS.md)
must retire — the separate `operator` source introducer — once the
source migrations close. The token-binding law already spans both
introducers (`9913f44891`), so what remains is spelled out below:
source census, introducer surfaces, migration routing, and removal
order. Delete this inventory once the `operator` introducer,
`Item::Operator`, and `MachineSupplyMode::Boundary` are gone.

## Source census: 212 `operator`-introduced declarations

All declarations spelled with the `operator` introducer (`git ls-files
'*.omg'`; `pub`-prefixed forms included):

| Bucket | Count | Where |
|---|---|---|
| `boundary operator` — library | 157 | `core/float_operations.omg` 124; generic rows in `core/{slice,vec,array,fixed_vec,ptr}.omg` 29; `blocking-executor/queue.omg` 4 |
| `boundary operator` — tests | 18 | `tests/omega/{pass,fail}` provider/domain/text/operator fixtures |
| plain `operator` — tests | 37 | `tests/omega/{pass,fail}` operator/termination/generics fixtures |
| plain `operator` — library | 0 | none remain |

The board row's count (208 at its measurement base) drifts to 212 at
`82741ec439`; recount on each landing.

## Introducer surfaces to retire

- **Parse.** `01_tokens-to-syntax-trees/src/declarations/`:
  `parse_declaration.rs` admits the introducer at the root
  (`:144` boundary form, `:326` nonboundary form) and inside
  `domain.rs` / `trait_definition.rs`; `operator.rs` owns
  `parse_operator_definition`. The `boundary requirement` route is
  already separate (`parse_declaration.rs:103-118`,
  `is_top_level_boundary_requirement`).
- **Representation.** `Item::Operator(OperatorDefinition)` in
  `syntax-trees/.../declarations/item.rs:24`, with the consumers that
  must follow it out: `identity.rs:194`, `item_snapshots.rs:605`,
  `item_copies.rs:284`, `syntax_trees.rs:179`.
- **Resolution.** `lowering/machine/token_bindings.rs` enforces the
  duplicate token/owner/shape space across both introducers
  (alpha-normalized by first-occurrence position); the dual-introducer
  arm retires with the introducer.
- **Checked stage.** `operators/token_bound_machine_calls.rs`
  rewrites each resolved binary/indexed use into an ordinary call on
  the declaration's entry state.
- **Supply mode.** `MachineSupplyMode::Boundary` — the
  undifferentiated mode `syntax-trees-to-symbol-resolved-trees/src/
  lowering/machine.rs` still assigns — is referenced by 23 files
  under `omega-rust/psi/` and retires after the source migrations.

## Migration routing

- **157 tokenless library `boundary operator` rows** → `boundary
  requirement` via **TOP-LEVEL-BOUNDARY-REQUIREMENTS**, which today
  executes only public nongeneric receiver-free requirements. The 29
  generic rows (`slice`/`vec`/`array`/`fixed_vec`/`ptr`) wait on that
  item's generic-requirement leg. `compiler_intrinsics/
  requirement_view.rs`, plan stamping, call-row retirement, D29
  coverage, and review policy rows already key on either spelling.
- **18 test `boundary operator` rows** → `boundary requirement`,
  same gate; several are `fail` fixtures whose expected diagnostics
  must be re-pinned to the requirement spelling.
- **37 plain `operator` rows (all in `tests/omega`)** → split by
  shape: root-scoped declarations move to the `machine` introducer
  with a fixed token; domain-member declarations move to the domain's
  requirement/member route. These rows exist to pin the introducer's
  surfaces (duplicate-binding, alpha-equivalence, overload-signature
  fixtures) — several retire outright once `Item::Operator` is gone,
  rather than migrating.

## Removal order

1. Migrate sources per the routing above (gated legs first:
   generic requirements, then the 124 float rows + 4 executor rows).
2. Re-pin test fixtures; delete fixtures that exist only to exercise
   the introducer itself.
3. Remove the parse arms in `parse_declaration.rs`, `domain.rs`,
   `trait_definition.rs`, then `operator.rs`.
4. Remove `Item::Operator` and its four consumer sites.
5. Collapse the dual-introducer arm of the token-binding law.
6. Remove `MachineSupplyMode::Boundary` (23 referencing files) once
   no source path assigns it.

## Standing constraints

- `..._terminal_exit` harness asserts the retired requirement is
  absent from the artifact.
- Retain operation, static telescope, signature, contract, visibility
  and the selected conformance row as a canonical kind distinct from
  trait requirements; bind installed execution to the selected
  provider execution and token era.
- Both pass canaries for the operator surface are currently
  checked-only; runtime legs stay with OPERATOR-MACHINE-SUPPLY's
  acceptance.
