# NEW-TSVV-TERMINAL-PSI-ELEMENT-VIEW-DESCRIPTOR — z142 implementation

Planner-minted item (no TASKS.md row); scope: only
`omega-rust/psi/representations/terminal-psi/src/terminal_module/types/structural.rs`.
HEAD: `bb192d7ea9e`.

## Gap (verified at HEAD)

Terminal Psi carried no runtime-length view descriptor. Three
checked-trees-to-lowered-psi sites reject
`CheckedUnitStructuralTypeShape::BorrowedSliceView` /
`CheckedStructuralValueKind::BorrowedSliceView` with
"borrowed slice view has no Terminal descriptor"
(`returns/structural_types.rs` ×3 sites, `unit/attached_unit/
structural_values/emission.rs`), so the `&[T]` view local that landed in
checked trees cannot lower — the TERMINAL-SLICE-VIEW-VOCABULARY leg named on
SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT's row.

## Landed (in scope)

`StructuralTypeShape::ElementView { element: StructuralTypeId }` — one
immutable borrowed view over a runtime-length sequence of structural
elements; extent is the view's stored runtime length, not the type identity
(mirrors `ByteSequence`'s semantics; `FixedArray` stays the owned
counterpart). `cargo check -p terminal-psi` green.

## Required consumer legs (out of scope — exhaustive-match sites)

The additive variant makes these matches non-exhaustive; each is its own
leg per the planner split (the `Reference` shape precedent `2f5dabbe5c3`
spread across ~15 files):

- `optimization-unit` — `identity/structural_encoding.rs:379` (identity
  encoding needs an ElementView case).
- `terminal-verifier` — 8 sites: `validation/foundation.rs:555`,
  `foundation/structural_types.rs:55/:114`, `references.rs:119`,
  `affine_cleanup.rs:581`, `structural_operations/structural_arguments.rs:307`,
  `structural_paths.rs:25`, `structural_result_contracts.rs:174`.
- `image-emission` codec — `installation_record/codec/
  structural_type_codec.rs` encode match + a new wire tag on the decode
  side (tags 1–7 occupied).
- Lowering producers — the three `borrowed slice view has no Terminal
  descriptor` reject sites in checked-trees-to-lowered-psi flip to emit
  `ElementView` once consumers exist (that crate is also out of scope).

## Verdict

Vocabulary authored in-scope; integration legs enumerated. `cargo check -p
terminal-psi` passes; dependent crates carry non-exhaustive matches until
their sibling legs land — do not merge this commit to main alone.
