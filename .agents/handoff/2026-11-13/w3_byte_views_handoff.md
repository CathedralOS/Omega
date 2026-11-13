# worker-3 handoff: byte views on the lowered-form route (2026-11-13)

## Result

Nothing landed, no PR, no branch pushed, no code written in the compiler. The session was
stopped by the coordinator minutes after it started (owner winding down), during the first
build. Everything below was read from source and spec; nothing was run, so every statement
about behaviour is unverified unless it says "read in".

## State of the world that changed since the previous handoff

- **#2765 has landed** (`0682e01d91263 psi: carry plain affine moves and normal-edge
  cleanup`, on main at `d892667b7d59f`). The collision warning in the brief no longer applies:
  `read/places.rs`, `body.rs`, `lower.rs`, `lower/places.rs`, `lib.rs`,
  `lowered-discharge/src/{places.rs,verdict.rs}` are free to edit on main. No open PR in
  `gh pr list` touches structure-elaboration except possibly draft #2803 (lowered-discharge,
  "report goals the integer rules cannot decide as incomplete"); check its file list first.
- The route's verdict already governs product compilations
  (`omega/compilation/compiler/src/checked/checking/lowered_route.rs`): refused = error
  `not supported yet`; accepted = Psi 05/06 still build the product. So a byte-view program
  the route accepts is testable end to end with `omega run --both` today.
- `tests/omega/corpus_outcomes.txt` is stale relative to the route (only one row mentions
  "not supported yet"); do not pick rows from it to find red byte-view fixtures.

## Smallest witness (written, saved, not run)

Saved in `scratchpad\w3v_fixtures\` (removed from the slot):

- `pass/slices/byte_view_parameter_guarded_read/{main.omg,build.omg}`:
  `machine first(bytes: &[u8]) -> u8 { transition bytes.len > 0 { true -> bytes[0] false -> 0 } }`
  and `machine at(bytes: &[u8], index: u64) -> u8 requires index < bytes.len { bytes[index] }`,
  with `machine main() {}` bound as the entry. Nothing calls the helpers: the route judges
  every machine of the subject package, so no caller (which would need a literal or an array,
  later slices) is required.
- `fail/slices/byte_view_parameter_unguarded_read`: `first` with a bare `bytes[0]`.
  Existing neighbour `fail/slices/byte_view_element_reference_without_bound` pins
  `cannot prove index `0` is within unknown slice length`; the route's own message for an
  unproved `OperationSafety` will differ, so `expected.txt` must be written from what the
  route prints.

Probe tool, saved as `scratchpad\w3v_dump_example.rs`: drop it into
`omega-rust/omega/compilation/compiler/examples/w3_dump.rs` (never commit). Arguments
`<main.omg> <machine name> <out file>`: prints the route's verdict
(`lowered_discharge::judge_compilation`, with `publish()` result), and dumps the old path's
`structural_types` and `machines` for comparison. It was never compiled. The existing
`examples/route_one_comparison.rs <main.omg>` prints the route's refusal detail without any
new code.

## Where byte views enter the route (read in source, main d892667b7d5)

Reader (`structure-elaboration/src/read/`):
- `types.rs`: `scalar_type` refuses `TypeReferenceNode::Slice` (`type slice`),
  `Reference` (`type reference`) and `FixedArray`; `atom_type` refuses
  `BuiltinTypeAtom::Slice`. `value_type` is the entry for a new kind.
- `machine.rs` `formal` (about line 1096): a `Reference { referee, access, lifetime }` formal
  is read as its referent's `ValueType` plus `body::Borrowed { position, access, home }`.
  A `&[u8]` formal fits here: referent = the new byte kind, `home: None`. Named lifetimes
  and `&write` are refused there today.
- `expressions.rs`: `ExpressionNode::Indexed` (`TableIndexedExpression { collection, index }`),
  `String`, `Borrow`, `Range`, `ArrayLiteral` are refused by name. `.len` arrives as
  `ExpressionNode::Member`; `read/places.rs` `member` refuses a receiver that is not a record.
  The typed trees already recognise the builtin length:
  `expression_table.authored_length_receiver(handle)` (typed-trees `values.rs` about line
  3916: member `len`, no valid member symbol, no case variant). Use it rather than a spelling.

Closed body (`body.rs`): `ValueType` is `Scalar | Record | Rigid`, `Copy`. Proposed:
a fourth variant for the octets a byte view denotes, plus `ExpressionKind` for the length
observation (result `u64`) and the element read (result `u8`). About 34 non-test match sites
name `ValueType::Rigid` across both crates; the compiler will list them.

Lowering (`lower.rs`, `lower/places.rs`):
- `lower.rs` `machine()` lines 141 to 173 lower a borrowed formal as a root
  `StructuralPlaceKind::Parameter` place with `StructuralParameterDeclaration { access:
  borrowed_access(..), multiplicity: Unrestricted }`; a scalar referent uses
  `records.primitive(scalar_type)` and `Bound::Home`. A view needs its own `Bound` variant
  (it is a leaf place, neither a record root nor a scalar home).
- `lower/places.rs` `declare()` builds the module's `structural_types`: records first, then
  one `PrimitiveScalar` declaration per homed scalar type. Add, after those, one declaration
  per view access used, shape
  `StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { access: Some(SharedBorrow) })`.
  The old path's module for a Build program (previous handoff, `w3b_types.txt`) has three
  such types (`access: Some(SharedBorrow)` copyable, `access: None`, and one with
  `descriptor_cleanup: false`); which flags and identity text a parameter's view type takes
  was the first thing the dump was meant to answer.
- Operations (`terminal-psi/src/control_flow.rs` about line 1242):
  `ByteSequenceLength { source: StructuralArgument }` and
  `ByteSequenceRead { source: PlaceId, index, length, obligation }`. Spec
  (`wiki/spec/terminal-psi/byte_views.md`, "Values and observations"): the read needs "`u64`
  index and length; dominating length observation of the identical view; checked
  `index < length`", and "Reading length supplies no indexing proof". Plan: emit a fresh
  `ByteSequenceLength` immediately before each read and mint one
  `ObligationKind::OperationSafety { operation }` for the read through the existing
  `lower/operations.rs` `safety()`.
- Index type: `pass/arithmetic/signed_index_bounds` shows `i64` indexes are legal source with
  an explicit lower bound. Serve `u64` first and refuse other carriers by name.

Checker (`lowered-discharge/src/`):
- `lib.rs` `stated_goal` (about line 1473) takes an `OperationSafety` goal from
  `terminal_semantics::proof_bearing_scalar_leaf_semantics`, which has no row for
  `ByteSequenceRead` (it would be refused as `operation has no safety goal`). The goal
  `index < length` over the two operand values has to be stated there or in
  terminal-semantics (which has `bytes.rs::indexed_byte_store_goal` for the store form, taking
  the bound as an input).
- Knowledge: each `ByteSequenceLength` result needs a row saying it equals the view's extent.
  The vocabulary has `ScalarTerm::ViewExtent { root, path, scalar_type }`, and the kernel's
  `proof/admission/src/predicate_denotation.rs` treats it as a definition subject. Spec:
  "a validated length observation of a machine or block parameter equals that exact
  parameter's `ViewExtent`". For a shared view parameter root the extent cannot change in
  the activation, so `result == ViewExtent(root, [])` never becomes false. A
  `requires index < bytes.len` clause would be the same term over the formal's place
  (`read/contracts.rs` must learn to state it). Whether the kernel proves
  `index < r2` from `0 < r1`, `r1 == ViewExtent(P)`, `r2 == ViewExtent(P)` is untested and
  is the main risk of slice 1.
- Also to visit: `minted.rs` (checks the rows are the ones the lowered form owes; about line
  343 for `OperationSafety`), `places/log.rs` (derives stores from operations; must classify
  the two new operations as non-writing), the initialization "uses" judgment in `places.rs`
  for a read of a parameter place, and `verdict.rs` `publish` (sealing through the codec's
  `foundation_validation.rs`, which has byte-sequence rules).

## Later slices, as far as read

- Literal argument: `EstablishByteSequenceLiteral { destination, bytes, qualifications }`
  into a `ByteSequenceLiteral` place, then (per the old Build module) `StructuralLeafCopy` to
  an `OperationResult` place and a `Jump` passing it `SharedBorrow` to a block parameter.
  byte_views.md: "A literal is established before use and cannot supply an owned or mutable
  argument"; measuring it yields the exact octet count at the length read.
- View field: `StructuralFieldType::ByteSequence(BorrowedView { access: Some(SharedBorrow) })`,
  stored with `StructuralByteViewFieldStore`, measured with
  `StructuralByteSequenceFieldLength`. A field can be overwritten in place, so the
  root-stable `ViewExtent` argument above does not carry over: it needs the write log.
  Needs "data with lifetime parameters" (item 1 of the Build list) for `Build<'storage>`.
- Fixed `u8` array field: `StructuralTypeShape::FixedArray { element, length: Literal(n) }`.

## What a Build program still needs

Unchanged from `w3_build_route_handoff.md`: items 1 and 3 to 7. Item 2 (#2765) has landed;
re-run the previous probe (`w3b_build_route_probe.patch`) to learn the Build slice's new
first refusal before assuming it is still `type generic data`.

## Slot

`C:\Projects\Omega\.agents\worktree\worker-3`: clean, detached at origin/main
`d892667b7d59f`, nothing running. The `mbx build -p omega` started this session was stopped
partway, so `target\debug\omega.exe` is still the previous session's probe-patched binary
(or partially rebuilt dependencies): rebuild before trusting it.
