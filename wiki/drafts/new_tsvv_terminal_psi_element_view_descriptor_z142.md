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

## Consumer sweep — 21 of 32 legs landed (2026-09-21, `3dddcb3b0973`)

The descriptor commit reached main on its own, against this draft's closing
instruction, and left the workspace unbuildable for roughly ninety minutes —
`cargo check --workspace` failed, so every worker's landing gate failed with
it. This section records the sweep that reopened it.

**Semantics followed**, from the "Landed (in scope)" section above:
ElementView mirrors `ByteSequence`, with `FixedArray` as the owned
counterpart. Concretely, in each analysis:

- it declares no fields and no cases, so field/case collectors return empty
  and "is this a record?" lookups return `None`;
- it is never an owned argument and never an owned result, like a
  non-`BoundedOwned` `ByteSequence`;
- it projects no children, so a path segment through one is malformed —
  "element-view structural type has no projected children";
- its `element` **is** traversed wherever an analysis collects structural
  children or resolves reachability, and must resolve, because unlike
  `ByteSequence` it names a structural type. The one exception is the
  by-value cycle traversal in terminal-codec, where a borrowed view inlines
  no storage and following it would invent a cycle — that arm is a no-op
  beside `Reference` and `ByteSequence`, matching the comment already there.

**Tag 8 everywhere.** Four independent encoders each had 1–7 occupied, as
this draft predicted: terminal-codec's wire codec, optimization-unit's
identity encoder, legalized-operations' identity encoder, and register-homes'
fixed-view-copy codec. `wiki/spec/terminal-psi/encoding.md`'s
`<!-- structural-type-shape-tags -->` table gains
`| 8 | ElementView | element structural type id |`;
`encoding_contract::structural_type_shape_table_matches_codec` passes.

Trusted-surface digests were re-recorded for the seven terminal-verifier
files in `trusted_surface/sites.rs`. Their justifications are unchanged: no
existing shape's behaviour moved, only a new variant was given arms.

**Landed legs (21):** terminal-verifier ×8, terminal-codec ×5 (wire encode,
wire decode, foundation validation, path projection, cycle traversal),
optimization-unit ×1, legalized-operations ×1, register-homes ×1,
checked-trees-to-lowered-psi ×5.

**Remaining (11), all behind live claims** — untouched deliberately:

- `optimization-unit-semantics/src/unit_validation/` ×7
  (`operation_contracts/structural_access.rs:380`, `references.rs:66`,
  `structural_catalog/catalog.rs:40,69,132`,
  `structural_catalog/type_declarations.rs:134`) — REPRESENTATION-SPECIALIZATION,
  Devin / linw2-field-value-specialization, to 18:46Z.
- `terminal-interpreter/src/terminal_interpreter/` ×4 (`custody.rs:380`,
  `effect_results.rs:127`, `structural_scalar_fields/entry.rs:54,129`) —
  REGISTERED-CALLBACK-LIFETIME, Devin / z139-registered-callback-lifetime,
  to 14:09Z.

Until those land, `cargo check --workspace` still fails on those two crates
and anything that dev-depends on them (`checked-trees-to-lowered-psi`'s test
target, for one). The four encoders are done, so no further tag decisions are
outstanding.

**Also newly visible, not caused by this sweep:** terminal-codec's
`canonical::structural_and_proposition_rows::proposition_nesting_has_a_total_bound`
aborts with SIGABRT on macOS arm64. It could not run at all before — the
crate did not build — so this is a first observation, not a regression. It
reads as a nesting-depth guard that does not fire before the thread stack is
exhausted on a host whose default stack is far smaller than Linux's.
