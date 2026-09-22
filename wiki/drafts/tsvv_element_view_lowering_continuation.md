# terminal-slice-view-vocabulary: element-view lowering continuation

Leaf: `leaf/terminal-slice-view-vocabulary` (branch pushed through
`psi: admit borrowed slice views to structural result carriers`).

Witness targets: `samples/cli/text/fletcher_checksum` exits 56 and
`samples/cli/arithmetic/recursive_sum` exits 70 (sum 50 / count 4) on
`omega run --target linux_x86_64`. Minimal probe at
`/home/ubuntu/probe-slices/{build.omg,main.omg}` expects exit 10:
`let s: &[i32 in Wrapping] = self.adder.bytes.as_slice();`
`let r: i32 = self.adder.sum(s, 5);` then `exit_process(r)`.

## Landed on the lane

1. Scalar-graph dispatch for borrowed-self calls: the `mixed` gate in
   `terminal_scalar/mod.rs` only rejects *selfless* attachments, and
   `structural_scalar_graph_signature` skips `is_self` in its `.any`
   pre-check, so `Adder::sum(&self, s, acc) -> i32` now produces a graph
   (probe `--check` passes end to end).
2. `shared_slice_view_argument` (calls/computation_arguments) plans a
   whole-name `&[T]` actual by exact view type identity; call-site lane
   `structural_arguments.rs` uses it for empty-path places.
3. `EstablishStructuralValue` is a custody owner (call_source_custody.rs)
   and is exempt from `occurrences::validate` — `as_slice()` is
   vocabulary-erased and records no checked call occurrence.
4. `borrowed_slice_view_referent` (source_custody/structural/mod.rs):
   `Reference{referee} -> ... -> Slice{element}` with one borrow shell,
   peeling `Constrained`, excluding U8 (byte views keep `ByteSequence`).
   The carrier gate admits the view; `normalized_type_identity` of the
   Slice node matches the ESV result's peeled view identity.
5. `BorrowedSliceView` -> `StructuralTypeShape::ElementView` in all three
   `returns/structural_types.rs` sites; collect() recurses on
   `element_type_identity` like `FixedArray`.

Current reject (probe `omega run`): `source_custody/structural/mod.rs`
`CheckedStructuralValueKind::BorrowedSliceView { source }` arm, still
`unsupported("borrowed slice view has no Terminal descriptor")`.

## Remaining work

### A. View establishment (probe reaches nativity)

- `source_custody/structural/mod.rs` `BorrowedSliceView` arm: `source`
  is a `CheckedUnitStructuralArgumentPlan` naming the lent collection
  place under `SharedBorrow`. `shared_borrow::validate` does NOT apply —
  it requires `ExpressionNode::Borrow`; the authored spelling is
  `collection.as_slice()`. Write a sibling validator replaying the
  `as_slice` receiver path against `source` (root + path) and checking
  `source.access == SharedBorrow`, then register the operand role (the
  establishment reads the carrier place; elements stay owned).
- `unit/attached_unit/structural_values/emission.rs` arm: resolve
  `source` to a `StructuralArgument` (same shape as the
  `Reference{source}` arm's `shared_structural_argument`) and emit the
  establishment op writing the 16-byte {base, extent} descriptor at the
  result place.
- New `OperationKind::EstablishElementView { destination, source, path,
  element }` in terminal-psi `operations.rs`: element type checked
  against the source collection's element (FixedArray element identity);
  extent is the collection's declared length (compile-known) read from
  the source structural type. Wire codec, verifier, and abstract-op
  lowering mirroring `EstablishReference` / `EstablishByteSequenceLiteral`.
- Realization: `ValueShape::integer(16,8)` descriptor residence — same
  16-byte base+extent carrier as `ByteSequenceCarrier::BorrowedView`
  (see function_fragments/structural/established_views.rs for the u8
  precedent: stack local, `AddressLocal` access, AbiTransport register).
- Call ABI: `ElementView` structural arguments already have the
  descriptor layout; verify the graph call `sum(s, 5)` binds the
  two-word descriptor to the callee's `ElementView` parameter place.

### B. Sample vocabulary (fletcher + recursive_sum)

`.len`, `s[i]`, `s[i..j]` on `&[T]` for T != u8, inside scalar graphs:

- Check level: `s.len` currently U8-gated (structural_fields ~:83);
  element-generic needs an `ElementViewLength` checked scalar expr (u64)
  over both view *parameters* and view *locals* — locals are new
  territory (u8 views never flowed as named locals).
- `s[i]` element read: `StructuralParameterIndexedRead` covers parameter
  storage; view locals need the analog producing `ElementViewRead`
  (element-typed scalar for scalar elements; scope leaf to scalar
  elements — samples use `i32 in Wrapping`).
- `s[a..b]` subslice: `TransitionSubsliceStart/End` +
  `ByteSequenceSubslice` control plan (`structural_control_plans.rs:286`)
  need the element-generalized `ElementViewSubslice` producing a NEW
  view descriptor {base + a*stride, extent b-a} fed to call arguments
  (`sum(s[1..], ...)`).
- CONFIRMED call-arg gap (2026-09-22 bisect): `sum(s[1..], ...)` inside
  `sum`'s own body drops `sum`'s whole unit plan — `s[1..]` canonicalizes
  to `Symbol(s) + [PlaceSegment::Index{expr}]` (open end is dynamic, see
  `flow/place/canonicalization.rs::index_place_segment`), then
  `structural_call_arguments` reaches its segment-dispatch `_ => return
  None` (~structural_arguments.rs:618). The closure pass then drops
  `main`'s `sum(s, 0)` call — `main` omits at
  `statement sequence: call: call operation (state 0, statement 6)`,
  exactly the `rejoins 0 Terminal attachment identities` diagnostic both
  samples emit. `byte_subslice::argument` (the `&[u8]` template producing
  `ArgumentSourcePlan::ByteSequenceSubslice{parameter_index,expression,
  start,end}` with `ByteSequenceSubslice{Start,End}` scalar roles) runs
  only for boundary callees (call_operations.rs:318) or unit-returning
  callees (structural_arguments.rs:133 `is_unit` gate) — so `s[1..]` to a
  non-unit ordinary callee is unsupported even for u8. Element version
  needs: a source variant (new `ElementViewSubslice` or a generalized
  byte variant), bound scalar-expr roles reused or mirrored, and the
  call-lane admission moved in front of canonical-place for view-typed
  targets regardless of the callee's return shape.
- New ops `ElementViewLength { source } -> u64`,
  `ElementViewRead { source, index, length, obligation } -> element`,
  `ElementViewSubslice { source, start, end, length, obligation }` in
  `operations.rs` — mirror `ByteSequenceLength/Read/Subslice` field for
  field; the `length` ValueId must be an `ElementViewLength` result;
  obligations prove `index < len` / `start <= end <= len`.
- Codec + verifier + `LoweredDirectExpression` mirrors + realization:
  element-stride addressing off the descriptor base (element stride
  from the element's `structural_layout`), vs the byte ops' stride 1.
- `recursive_sum` uses `&mut self` receivers on `Summer` — CONFIRMED
  admissible: `is_self && is_reference` params stay ambient
  (signatures.rs:503-508 `retain_reference_self` skip) and
  `receiver_argument` admits (MutableBorrow, MutableBorrow) receiver
  operands on field places. Probe bisect: `&mut self` + a live borrow of
  the SAME field (`s` borrows `self.adder.bytes`, receiver `self.adder`)
  correctly rejects at borrow validation — disjoint fields
  (`self.summer` vs `self.arr`) pass through to the `s[1..]` gap above.
- Rank/decrease: `terminates by s -> Slice::Length` machinery is
  existing; verify `s.len` after `s[1..]` retains length facts.

### C. Negative gates

`bounds/element-type/alias violations reject`: out-of-bounds index,
mismatched element identity between actual and formal views, and
mutable-aliasing of the lent collection must each reject at check or
lowering — extend `slice_view_locals` tests.

## Watchpoints

- `&[u8]` keeps the `ByteSequence` path end to end — do not route it
  through `ElementView` (U8 exclusion is deliberate in every helper).
- `StructuralPathSegment` has no runtime-index segment
  (Field/FixedIndex/FixedByteRange/Referent only); `s[i]` uses an
  index ValueId, not a path segment — same as `ByteSequenceRead`.
- Field-through-runtime-index (`sub[index].value` for the canary leg)
  still unsolved; check `places.rs` before designing.
- `is_bounded_structural_scalar_store_path` (places.rs:29) may need a
  view-read arm for stores into `&mut [T]` elements — out of leaf scope
  unless samples require it.
