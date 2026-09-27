# OWNED-LOCAL-WINDOW-ROOTS — design + claim block

Psi already mints clean window pairs for owned local homes (the codec
foundation validator was extended at `5fe3ad09dc` —
`borrowed_field_store_root_type` admits producer-completed places). The
remaining wall is omega-side: every stage types a window root as
`StructuralParameterDeclaration`, and an owned local home has no parameter
row, so `case_field_local_store` declines at
`Lowering(InvalidBorrowedStorageWindow)` in
`00_terminal-psi-to-abstract-operations` `machine/operation/borrowed_windows.rs`.

## Claim state at filing (2026-09-27 ~08:50Z)

Claimed under OWNED-LOCAL-WINDOW-ROOTS (ticket `d3796c5904b0`):

- `omega/pipeline/00_terminal-psi-to-abstract-operations/src/lowering/machine/operation/borrowed_windows.rs`
- `omega/pipeline/02_abstract-operations-to-target-operations/src/lowering/control_flow/borrowed_windows.rs`
- `omega/pipeline/03_target-operations-to-selected-instructions/src/legalization/scalar_graph_input/borrowed_windows.rs`
- `omega/pipeline/03_target-operations-to-selected-instructions/src/legalization/scalar_graph_input/target/unit/borrowed_windows.rs`
- `omega/pipeline/03_target-operations-to-selected-instructions/src/selection/construction/scalar_graph/structural/leaf_copy.rs`
- `omega/pipeline/03_target-operations-to-selected-instructions/src/selection/validation/scalar_graph/structural/leaf_copy.rs`
- `omega/pipeline/03_target-operations-to-selected-instructions/src/tests/legalization/borrowed_windows.rs`

Blocked by live foreign claims — do NOT edit until they lapse or the
coordinator grants `--allow-overlap`:

| path | claim | owner | expires |
|---|---|---|---|
| `representations/abstract-operations/src/abstract_operations/operations.rs` | AFFINE-CLONE-OWNED-FIELD-STORE | Codex / owned scalar field storage | 11:53Z |
| `representations/target-operations/legalized-operations/src/legalized_operations/control_flow/scalar_graph.rs` | AFFINE-CLONE-OWNED-FIELD-STORE | Codex / owned scalar field storage | 11:53Z |
| `representations/target-operations/src/target_operations/operations/unit.rs` | AFFINE-CLONE-STORE-CATALOG | Codex / owned scalar field storage | 11:59Z |
| `pipeline/03_.../legalization/source/scalar_graph/instruction/storage_instructions.rs` | AFFINE-CLONE-OWNED-STORE-LEGALIZATION | Codex / compiler recovery | 15:53Z |

## Design (complete, ready to execute)

New type in `abstract_operations` (target-operations already depends on it):

```rust
/// The root a borrowed-window pair operates under: `place` + `structural_type`
/// are the geometry every consumer needs; `authority` carries which custody
/// admitted the window.
pub struct StructuralWindowRoot {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub authority: StructuralWindowRootAuthority,
}

pub enum StructuralWindowRootAuthority {
    /// Mutable-borrowed signature parameter — retains the complete declared row.
    BorrowedParameter(StructuralParameterDeclaration),
    /// Owned local home completed by a producer op (record/array
    /// establishment, structural call result, leaf copy) under
    /// Unrestricted|Affine custody with no qualifications or claims.
    OwnedHome,
}
```

The struct shape (not a bare enum) keeps `.place`, `.structural_type` and
`==` consumers compiling untouched: `01 representation_specialization`
`admission.rs` (`destination.place == place`), `02` `operations.rs`,
`03` `parameter_use.rs`, `replay/instruction.rs` (`destination ==
expected_destination`, `*source == root.place`), image-emission
`aggregate_results.rs` (`retained_destination == destination`) all need no
edits. Derive `Clone + PartialEq + Eq + Debug` on both types.

## Per-stage changes

- **00** `borrowed_windows.rs`: `borrowed_window_root` returns
  `Option<StructuralWindowRoot>`. Parameter arm: `exact_parameter` +
  MutableBorrow + Unrestricted|Affine + empty custody → `BorrowedParameter`.
  Owned arm: the place's `StructuralPlaceKind::OperationResult` resolves its
  producer; admit when the producer kind is one of `EstablishRecord`,
  `EstablishZeroedStructural`, `CallStructural`,
  `CallStructuralWithScalarArguments`, `StructuralLeafCopy`,
  `StructuralCaseLeafCopy` with result multiplicity Unrestricted|Affine and
  empty qualifications/claims — the same set the terminal verifier's
  `completed_source` and the codec's `borrowed_field_store_root_type` admit.
- **02** `control_flow/borrowed_windows.rs`: `window_root` gains `live` and
  matches `authority`: `BorrowedParameter` keeps the current
  signature-membership + prepared-parameter checks; `OwnedHome` admits when
  `live.structural_homes[root.place]` exists with matching type, non-linear
  multiplicity, no claims, empty quals. `move_field` lowers to
  `StructuralLeafCopy` — its source addressing already covers homes
  (`leaf_copy::copy` reads `live.structural_homes` first).
  `store_field` pushes `TargetUnitOperation::StoreStructuralField` with the
  descriptor as `destination`.
- **target-ops** `unit.rs`: `StoreStructuralField.destination` retypes to
  `StructuralWindowRoot`.
- **legalized-ops** `scalar_graph.rs`: `StoreStructuralField.destination`
  retypes the same way.
- **03** `scalar_graph_input/borrowed_windows.rs`: `extent`/`moved` take
  `&StructuralWindowRoot`; the `access == MutableBorrow &&
  structural_parameters.contains` + `entry_claims.is_empty()` gate applies to
  the `BorrowedParameter` arm only. `storage_instructions.rs` and
  `target/unit/borrowed_windows.rs` follow the signature change.
- **03 selection** `construction`/`validation` `leaf_copy.rs::reseat`: the
  `access == MutableBorrow && signature.parameters.any(semantic == root)`
  gate applies to `BorrowedParameter`; `OwnedHome` resolves
  `destination.place` through `transport.pointers` exactly like today (the
  producer already published the home's durable pointer).
- **tests**: `03 .../tests/legalization/borrowed_windows.rs` constructs the
  ops with declarations — wrap in `StructuralWindowRoot::borrowed(...)`.

## Acceptance

`tests/omega/pass/structural/case_field_local_store` completes `omega run`
on linux_x86_64 (today it declines `InvalidBorrowedStorageWindow` at 00;
with the descriptor it should land machine code). Then re-record the corpus
row and re-check adjacent `move`/`store` fixtures for regressions.
