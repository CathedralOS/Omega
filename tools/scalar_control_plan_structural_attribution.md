# Attribution: "scalar control plan structural signature" census family is a label artifact

## Claim (coordinator's census)

~24-33 `squalr-engine-api` machines collapse to `checked_state_graph` in
`omega-rust/psi/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_scalar/mod.rs`
returning None — attributed to scalar-only signature gates
(`non-primitive result type` ×9, `mixed signature` ×11, `multi-state structural for` ×5,
`record locals` ×3, `statement bindings` ×3, `terminator` ×1, `owned parameters` ×1
on tip 449320532a7; 33 machines).

## Finding: routing is already correct; the scalar reason is the surfaced fallback

`machine_dispatch.rs:95-169` order: dynamic plan → return plan → Unit plan →
Unit closure → scalar graph → `unsupported(declined_reason)`. The scalar
`declined_reason` surfaces only when no Unit plan and no closure covered the
machine — it is the *last-attribution* label, not the owning wall.

Method (this diagnosis): a session-local omega integration test compiled
`squalr-engine-api/build.omg` once via
`packages::prepare_local_project` + `check_prepared_local_project_for_inspection`,
then ran `lower_machine(TerminalMachineSelection::Symbol(..))` over every row in
`checked.facts.flow.terminal_machines.machines`, joined with
`terminal_unit_effects.omission_for_machine(..)`. One package check, ~110s;
545 machines lower, 572 fail. Full output was captured at run time.

Every one of the 33 machines has a `CheckedUnitPlanOmissionStage::LocalConstruction`
row — the Unit lane attempted the body and declined at a *body-construction*
phase. A structural-result arm on the scalar graph would not unblock a single
one: the machines are already routed to the Unit graph, and their bodies carry
constructs neither lane covers. "Don't blanket-admit" is right — the correct
shape is per-phase unit-lane leaves.

## Real walls (phase → machines → owning file → fence)

| real unit-lane phase | count | owning file (04 terminal_unit) | live fence |
|---|---|---|---|
| `state graph: terminator: guarded jump successors: edge cleanup`/`roster` | 10 | `state_graph/terminator.rs` | **z1 GUARDED-PAIR-FALSE-FALLBACK (exp ~15:25Z)** |
| `state graph: state signature: parameter custody shape: borrowed non-view carrier` | 5 | `state_graph/mod.rs:534` | free |
| `state graph: terminator: conditional successors: guard expression` | 3 | `state_graph/terminator.rs:387` | **z1** |
| `structural field store: destination parameter` | 3 | `structural_scalar_store/destination.rs:140` | free (z6 claim covers mod.rs only) |
| `statement sequence: call count agreement` | 3 | `control/statement_sequence/completion.rs:471` | free |
| `statement sequence: assignment: structural value field store` | 1 | statement_sequence | free |
| `structural field store: carrier path` | 1 | `structural_scalar_store` | free |
| `call operation: structural arguments: caller structural result` | 1 | call operation | free |
| `outer calls: unconsumed nested call in a transition` | 1 | outer calls | free |
| `statement sequence: local data: scalar local: pure initializer` | 1 | statement_sequence | free |
| `statement sequence: unsupported statement kind` | 1 | statement_sequence | free |
| `state graph: prefix initializers: bound expression` | 1 | state_graph | free |

z1-fenced subtotal: 13 machines; likely to clear or re-attribute when
GUARDED-PAIR-FALSE-FALLBACK lands.

## Mechanism notes on the largest unfenced sub-families

- **`borrowed non-view carrier` ×5** (`format_absolute_address`,
  `copy_text`, `copy_symbol_text`, `symbolic_pointer_chain_format_offset`,
  `copy_task_text`): the failing params are `&mut [u8; N]` — borrowed
  *fixed-array* refs. `types::byte_sequence_carrier` admits `&mut [u8]` slices
  (`BorrowedView`) and owned domain-constrained `[u8; N]` (`BoundedOwned`), and
  `borrowed_slice_view_element` is slice-referee only. A borrowed fixed array
  needs either a new carrier variant or a bounded-view mapping threaded
  through the ~8 downstream carrier consumers (call argument paths, boundary
  admission, returns, local results).

- **`structural field store: destination parameter` ×3**
  (`pointer_scan_results_{sort_u64,dedup_u64,copy_cached_ids}`): destinations
  are `target_addresses[index] = v` stores into `&mut [u64]` slice params.
  `structural_scalar_store::destination::resolve` searches only the
  `structural` parameter roster before `owned_local_root`; `&mut [u64]` gets
  no structural plan at signature planning. Owning fix is upstream — admit
  borrowed slices in the unit signature's structural-parameter mint.

- **`call count agreement` ×3** (`write_hex_upper`, `write_hex_upper_at`,
  `format_module_address`): `completion.rs:471` requires every flow-listed
  call in the state consumed by the sequence; an unconsumed call (nesting or
  ordering shape) drops the machine.

- **`conditional successors: guard expression` ×3**
  (`try_resolve_virtual_module_address`, `is_data_type_selected`,
  `shift_heap_insert`): terminator.rs:387 — same file z1 owns.

## Non-family walls surfaced by the same sweep (outside this leg's scope)

Distinct classes sharing the api list: `call target has no allocated machine
signature` (e.g. `ScanResultRefList::get`), `InvalidTerminalModule(ControlCycle)`,
`primitive reference return has unsupported parameter qualifiers`,
`structural return changed its declared result custody`,
`TrappingIntegerCrashUncovered` (std `Optimizations::{enable,emit_report}`),
`signature outside the current terminal-Psi source slice` (std Nat lemmas —
unchecked-body machines, expected).

## Recommendation

Fan out by real phase, not by the surfaced scalar label:
z1's landing absorbs ~13; `borrowed non-view carrier` (5) is the largest
unfenced single site; `destination parameter` (3) continues the
case-field-store line of work; the remainder are singleton leaves. Sweep
method above re-runs the full census on any future tip.
