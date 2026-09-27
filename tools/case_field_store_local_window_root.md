# Case-field store on non-parameter roots — remaining omega-side wall

Fixture: `tests/omega/pass/structural/case_field_local_store`
(`boxed.fmt = fresh` on `let mut boxed: Box = ...` in `Main::reseat`).

As of the `leaf/case-field-store-roots` work the Psi side is complete: the
checked plan mints the value as a dense structural-parameter index, the
attached-unit frame resolves authored destination positions through the
`evaluation.structural_parameters` pairing and local destinations through
`structural_locals`, emission writes the `StructuralLeafCopy` +
`MoveStructuralField`/`StoreStructuralField` window pair on the local's
established home place, the codec foundation validator admits an owned
completed home as a window root, and the terminal verifier accepts the pair
(`inspect-terminal` reports `verified=true`).

## Wall (declines at `native artifact lowering`)

`omega-rust/omega/pipeline/00_terminal-psi-to-abstract-operations/src/lowering/machine/operation/borrowed_windows.rs`
re-checks the window's static shape and requires the root via
`borrowed_window_root` → `exact_parameter` + `StructuralAccess::MutableBorrow`
— machine parameters only. `Lowering(InvalidBorrowedStorageWindow)` fires on
the move whose root is a local's home place.

## Mechanism gap (why it is not a one-line admit)

`AbstractOperation::MoveStructuralField.source` /
`StoreStructuralField.destination` are typed
`terminal_psi::StructuralParameterDeclaration` — "the complete parameter row
... so no consumer reconstructs borrow authority from physical ABI shape".
An owned local home has no parameter row, so admitting it means a root
descriptor that covers both cases, and then threading it through:

- `01` representation_specialization admission + effect summaries (root
  contributes no caller-parameter demand).
- `02` control_flow `window_root` (currently requires the decl to be in
  `function.structural_parameters` and a mutable-borrowed prepared parameter
  root; a local root's authority is its owned completion and its home comes
  from `live.structural_homes`, not the signature pointer).
- `target_operations::TargetUnitOperation::StoreStructuralField.destination`
  — same `StructuralParameterDeclaration` field type.
- `03` legalization `scalar_graph_input` borrowed-windows + unit legs and
  selection's chunked copy (`structural/leaf_copy.rs`) — the write side
  addresses the field through the local's home slot rather than a parameter
  referent.
- `image-emission` `function_fragments/source/structural_fields.rs`.

The scalar flavor hits the identical boundary: c0f37bf696's canary
`pass/structs/uninit_record_member_store` is pinned `rejected` at
`InvalidStructuralScalarFieldStore` — its target op
`StructuralScalarFieldStore.destination` is the same parameter-decl field.
A single "owned-home root" vocabulary covering both flavors is likely the
right shape for that follow-up leg.
