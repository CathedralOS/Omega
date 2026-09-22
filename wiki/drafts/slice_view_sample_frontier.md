# Slice-view local entry establishment — twelve-sample frontier record

Board row: `TASKS.md` **SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT** (:13407,
split-of:SAMPLES-COMPILE-MULTI-HOST) and its vocabulary leg
**TERMINAL-SLICE-VIEW-VOCABULARY** (:13358). Recorded from `163618c895` on
linux x86-64. Delete this record once the twelve samples reach selected
ProgramEntry establishment, or when the vocabulary leg publishes the missing
spec section.

## The failure class

On `linux_x86_64`, `linux_arm64` and `macos_arm64` the entry-establishment
gate reads:

```
selected ProgramEntry establishment rejoins 0 Terminal attachment identities; expected one
```

for every sample whose entry machine leaves the unit-plan roster at local
construction over a borrowed slice view local:

```
let s: &[i32 in Wrapping] = self.adder.bytes.as_slice();
```

The pattern appears in 12 samples (the row's roster):
`cli/text/fletcher_checksum`, `cli/arithmetic/recursive_sum`,
`cli/probes/dual_accumulator_recursion`, `cli/collections/slice_accum_probe`,
`cli/collections/slice_maximum`, `cli/collections/subslice_sum`,
`cli/systems/framed_payload`, `cli/proofs/clamp_sum`, and the
`cli/games/dungeon_crawler` modules (`rooms/room_lookup`,
`commands/command_parser`, `inventory/inventory`).

## What landed

`d7a48d7af0` made the view local a first-class checked shape:

- `CheckedUnitStructuralTypeShape::BorrowedSliceView { element_type_identity }`
  in `checked-trees/src/checked_trees/flow/terminal/structural_type_plans.rs`
  carries the element identity and no length — a slice's extent is its own
  stored runtime length.
- `CheckedStructuralValueKind::BorrowedSliceView` rejoins the shared loan the
  checked borrow admission already records for the lent collection.
- Ordinary statement sequencing establishes the local and forwards it whole
  to a `&[T]` formal; callee `&[T]` parameters carry the same shape.
- The rejoin diagnostic now names the recorded omission stage.

The class frontier moved from local construction to the call rather than
closing.

## The current frontier

`fletcher_checksum` and `recursive_sum` now both stop at
`statement sequence: call: call operation` (recursive_sum: state 0,
statement 6): `Adder::fletcher` / `Summer::sum` have no unit plan of their
own because their bodies need `s.len`, `s[0]` and `s[1..]` over a non-byte
view. Nine lowering sites across `checked-trees-to-lowered-psi` and
`selected-dispatch` reject the shape with:

```
borrowed slice view has no Terminal descriptor
```

rather than dropping the extent.

## The gap is a vocabulary, not a decision

`wiki/spec/terminal-psi/byte_views.md` supplies length, read and subslice
operations only for borrowed **byte** views (its title and :5 scope it so).
`Slice::index` and `Slice::range` are settled at the language level
(language guide chapter 5 :244-248, chapter 19 :46-50), so the missing piece
is the Terminal Psi form of an already-decided semantics plus the spec
section that states it — owned by TERMINAL-SLICE-VIEW-VOCABULARY. Do not add
a recognizer for this statement arrangement (AGENTS.md: compositional
lowering).

One question must settle before the read operation can be written — does
`Slice::index` require a `[copy]` element? `source/library/core/slice.omg:19`
and chapter 5:244 declare `boundary machine [] Slice::index<T>(items: &[T],
index: u64) -> T` unconstrained, while chapter 19:46 declares the same
machine `Slice::index<T [copy]>`. `wiki/spec/language/ownership.md` makes
Affine the default for owned data, so returning a non-copy `T` by value out
of a shared `&[T]` moves out of borrowed storage — pointing at chapter 19
being right and the library declaration being under-constrained. Nothing in
the tree settles it: the only in-tree uses of `Slice::index` are fail
fixtures pinning duplicate-operator rejection
(`fail/operators/root_operator_{duplicate,alpha_equivalent_generic_duplicate}`),
which carry `<T>` incidentally. Resolve from the checker's actual behavior;
if the checker does not decide it, this is an owner question about the core
surface.

## Acceptance (verbatim from the owning rows)

- A callee taking `&[T]` for a non-byte `T` reads its length, indexes it and
  takes a subslice, reaching native production; the nine
  "borrowed slice view has no Terminal descriptor" rejections are replaced by
  real descriptors.
- `fletcher_checksum` and `recursive_sum` reach selected ProgramEntry
  establishment on `linux_x86_64` without the "rejoins 0 Terminal attachment
  identities" refusal, and `cargo nextest run -p
  typed-trees-to-checked-trees --lib` stays green.
