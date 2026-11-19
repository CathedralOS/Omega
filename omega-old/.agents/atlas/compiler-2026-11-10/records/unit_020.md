# Unit 20 notes

Unit 20 (reader 20 of 113). The unit file lists 11 files; this record covers the 7 I read in full and says which 4 I did not read.

Files covered (all under `omega-rust/omega/compilation/pipeline/`; paths relative to that):

1. `03_target-operations-to-selected-instructions/src/selection_views_and_bytes.rs` (1-2565, read in full)
2. `03_target-operations-to-selected-instructions/src/structural_inputs.rs` (1-2809, read in full)
3. `04_selected-instructions-to-selected-instructions/src/address_and_load_rewrites.rs` (1-1407, read in full)
4. `04_.../src/lib.rs` (1-223, read in full)
5. `04_.../src/selected_optimization.rs` (1-188, read in full)
6. `04_.../src/unexecuted_rewrites.rs` (1-15, read in full)
7. `04_.../src/copy_and_extension_rewrites.rs` (1-1735, read in full; section 7 below)

NOT READ (see Coverage, nothing is claimed about them): `04_.../branch_and_compare_rewrites.rs`, `04_.../literal_fold_rewrites.rs`, `04_.../rewrites.rs`, and `05_selected-instructions-to-register-homes/src/assignment.rs`.

---------------------------------------------------------------------------

## 1. `selection_views_and_bytes.rs` (Omega 03, instruction selection)

**Purpose.** File header (1-5): selects byte-sequence and element-view operations for structural instruction selection: byte concatenation, byte copying, byte-field reads/stores, subslices, element-view establishment, element writes and observations. It is a set of `pub(crate) mod` blocks, each one a lowering routine per `LegalizedScalarInstructionKind` variant. The file is not a module root; `selection_structural.rs` includes it (callers at `selection_structural.rs:5397-5515` call `byte_concatenation::establish`, `byte_field_store::replace_byte`, `byte_field_store::replace`, `byte_view_field_store::replace`, `byte_views::write`, `element_view_write::write`, `element_establishment::establish`, `element_subslice::create`; re-exports at `selection_structural.rs:5172-5174`) (grepped).

**Shape (modules and entry functions).**
- `byte_concatenation` (7-250): `establish` (23) emits a local slot of `capacity+8` bytes, copies left then right operand with `CopyBytes`, then `ExactAddI64` of the two lengths carrying `capacity_obligation`/`accepted_fact` (95-103), stores the length at offset 0, pushes `(result.place, owner)` into `builder.transport.pointers` (122). `operand_pointer` (126), `operand_span` (164), private `copy` (199).
- `byte_copy` (252-410): `CopyEnd {place, pointer, byte_offset, elements}` (265); `copy` (272): if `byte_size > extent_copy::INLINE_MAX_BYTES` emit one `CopyBytes` with bulk operands (280-316), else a chunked load/store loop over `extent_copy::chunks(byte_size)` (317-371). Called from `selection_structural.rs:364,1259,2767` (grepped).
- `byte_field_read` (412-530): `read_byte` (427) lowers `StructuralByteSequenceFieldRead` to `Load8Indexed`; `payload_pointer` (491) computes the payload address, loading a 64-bit descriptor pointer when `geometry.is_descriptor` (514-527).
- `byte_field_store` (532-907): `replace` (547) handles `ByteSequenceStore` and `StructuralByteSequenceFieldStore` (copy bytes then store length word); `replace_byte` (796) handles `StructuralByteSequenceFieldByteStore`; `scratch` (897) allocates instruction scratch registers.
- `byte_view_field_store` (909-1052): `replace` (924) stores a source descriptor's (pointer, extent) pair into a destination field's descriptor slot.
- `byte_views` (1054-1528): `write` (1071), `byte_observation` (1141), `byte_sequence_read` (1200), `charge_backing_roots` (1268), `unsliced_payload_offset` (1302), `backing_index` (1314), `backing_pointer` (1353), `byte_sequence_length` (1375), `byte_predicate` (1418, emits `ScanByteSequence`), `byte_equal` (1457, emits `CompareByteSequences`), `measured_span_reads` (1509).
- `element_establishment` (1530-2008): `establish` (1550) for `EstablishElementView`; one big function, ~420 lines (1550-1973), branching on `ElementViewRegion::{WholeCollection, WholeRepresentation, RepresentationBytes}` (1831-1965). Helpers `constant` (1975), `local_home_root` (1994).
- `element_subslice` (2010-2186): `create` (2022) for `ElementViewSubslice`; records an `ElementViewHomes` (2148) in `builder.transport.element_views`; materializes a 16-byte descriptor slot only if `requires_descriptor` (2156).
- `element_view_write` (2188-2299): `write` (2201).
- `element_views` (2301-2565): `element_stride` (2319), `element_observation` (2339), `element_view_read` (2372), `element_view_length` (2499), `element_backing_pointer` (2538).

**Inputs/outputs.** Reads `LegalizedScalarFunction` (signature, structural types, parameters), the `LegalizedScalarInstruction` row, and the `Builder` (`builder.transport.{pointers, views, element_views, local_slots}`, `builder.constraints.keys.*`, `builder.registers`). Mutates `builder` only: emits `SelectedInstructionKind::*` instructions, memory-access rows (`memory(...)`/`footprint(...)` with `SelectedMemoryAccessRole::*`), and registers new places into `transport.pointers`/`element_views`/`local_slots`. Returns `Result<_, SelectedInstructionError>`.

**How it decides (keys).**
- Place identity: "is there a transport pointer already for this place" (`builder.transport.pointers.iter().any(|(place,_)| *place == result.place)` -> `invalid()`) at 40-47, 1583-1588, 2070-2075. Lookups of a place's pointer are linear scans over `transport.pointers` (e.g. 726-732, 858-864, 973-979, 1725-1731, 2546-2552).
- Type identity: structural type ids matched against `signature.structural_types` by `id` (e.g. 677-683, 961-966, 1594-1630).
- Scalar types: every integer operand is compared by value to `ScalarType::Integer(IntegerType::new(Unsigned, 64))` or `Unsigned,8` and anything else is `invalid()` (53, 449-462, 686-693, 849-857, 1087-1101, 2079-2088).
- Access mode: `StructuralAccess::{Owned, MutableBorrow, WriteOnlyBorrow}` set for stores (577-583, 953-959); `exclusive_borrow_pair_pattern!()`, `reading_borrow_pair_pattern!()` macros (650, 824, 1582).
- No display-name or string comparisons seen. Every failure is the generic `invalid()` except two with text: "exact extent copy has no admitted byte-copy form" (289-292) and "element subslice does not retain readable source descriptor access" (2056-2059).

**Cases by shape.**
- `byte_sequence_length` accepts `length_byte_offset` only `0 | 8` (1382); `byte_observation` expects 0 for `BoundedOwned`, else 8 (1177-1187); `BoundedView` length is materialized as the capacity constant (1163-1176).
- `element_view_length` requires `length_byte_offset == 8` (2356, 2505).
- `element_establishment`: `WholeCollection` rejects an `accepted_fact` (1833); `WholeRepresentation` requires `accepted_fact` and nonzero `length_scale` (1883); `RepresentationBytes` requires `accepted_fact` and nonzero `stride` (1915, 1927); x86-64 only gets an extra division scratch operand (1951-1953).
- Zero-length whole-collection special case: `Fixed(0)` with `WholeCollection | WholeRepresentation` yields a constant 0 base (1684-1690, 1705-1711, 1764-1770, 1801-1808).
- `element_view_write`/`element_view_read` convert `byte_count` to `u8` for the store (2293) and select the load via `load_transfer` (2488).
- `element_establishment` result checks: multiplicity `Unrestricted | Affine` only, no qualifications, no claims, projected qualifications must start with `AllElements` (1567-1582).
- `element_establishment` source-root choice is a fixed priority chain: parameter, local home, block home, descriptor parameter (1632-1657).

**Same job elsewhere.**
- `element_stride` is defined twice: `selection_views_and_bytes.rs:2319` (`element_views::element_stride(function, builder, source)`, checks `builder.transport.element_view` first, then the declared view type) and `selection_structural.rs:762` (`descriptor_operands::element_stride(function, source)`); I read only the first (grepped definitions; second suspected same job without the builder lookup). Callers: the 2319 one is called once, at line 2398; the 762 one at lines 2090 and 2231 (grepped `element_stride\(`, 5 hits in the directory).
- Two byte-source-to-destination copy loops: `byte_concatenation::copy` (199-249, `CopyBytes` with scratch cursor/byte) and `byte_field_store::replace` (769-778, same `CopyBytes` emission) and `byte_copy::copy` (272). Same emitted instruction, three argument-building paths.
- Parameter-vs-local-home type lookup pattern ("`read_parameter(function, place)` else `aggregate_result_input::home`") repeats at 561-576, 629-639 and 1632-1650.
- Check pattern "`index` u64, `length` u64, `value` u8" repeats in `read_byte` (449-462), `replace_byte` (841-857), `write` (1087-1101), `byte_sequence_read` (1216-1226).

**Unused / one-caller.** I did not find unused `pub(crate)` functions in the file; every module entry has a caller in `selection_structural.rs` (grep above). `operand_span` has 3 hits: definition (164), `copy` (220), `measured_span_reads` (1515). The module comment is the file header only; nothing claims "single/shared/canonical".

**Stated limits.** None quoted; the only prose is the 5-line header.

**Fail-open risks.** None seen in this file: the pattern is `?`/`ok_or(invalid())` on every lookup, so missing rows reject. One spot worth recording: `charge_backing_roots` (1268-1300) and `byte_concatenation::operand_span` (181-195): when `view_backing_roots` returns `None` the code charges the memory access against the operand's own place rather than failing (1290-1297, 190-194). "traced" only locally: the `None` means "no view-backed roots", and the fallback charges the place instead; whether `view_backing_roots` (`selection.rs:337`) returns `None` for a view whose roots are unknown I did not read.

---------------------------------------------------------------------------

## 2. `structural_inputs.rs` (Omega 03)

**Purpose.** Header (1-6): reconstructs native structural storage geometry (byte offsets, pointer/descriptor slots, collection views) and unit parameter/result contracts "shared by target legalization and instruction selection". Pure functions over `&[StructuralTypeDeclaration]`; all return `Option`, `None` = reject. Two modules: `structural_reference_input` (8-2707) and `structural_unit_input` (2708-2807); re-exports at 2809.

**Shape.**
- Data: `RuntimeElement {index, obligation, stride, extent}` (85), `PointerSlot {byte_offset, indices, scalar_descriptor}` (141), `ReferenceSourceGeometry<Endpoint>` (147), `StorageProjectionRegions` (302, private), `PrimitiveAccessGeometry` (415), `CollectionViewGeometry` (911), `BoundedByteViewGeometry` (1223), `ReferenceProjection` (1703), `ProjectedByteField` (2534), `Projection` enum (1842, private), `Parameter` (2718).
- Core walkers: `storage_projection_regions` (311) splits a path at `Referent`/`FixedIndex`/`RuntimeIndex` crossings into pointer-slot regions; `project_storage` (1919) walks `Case`, `FixedIndex/RuntimeIndex`, `Field` through Sum/Mixed/Record/FixedArray/ElementView; `runtime_storage_projection` (2106); `reference_geometry` (277).
- Layout: `shape`/`shape_inner` (679/1392) computes `ValueShape` (size, alignment) per `StructuralTypeShape`; `field_shape` (1623); `primitive_array_shape` (740); `case_layout` (1674) via `calling_conventions::evaluate_conventional_sum_layout`.
- Geometry endpoints: `primitive_geometry` (545), `primitive_read_geometry` (640), `primitive_store` (466), `atomic_cell_geometry` (423), `field_read` (2412), `case_field_read` (2486), `scalar_field_geometry` (2231, ~150 lines), `element_leaf_projection` (2199), `projected_byte_field` (2542), `byte_field_length` (2623), `byte_field_storage` (2637).
- Views: `collection_view_region_geometry` (935, ~250 lines), `bounded_byte_view` (1229), `bounded_byte_field_borrow` (1303), `fixed_byte_array_view` (815), `fixed_element_array_view` (859), `borrowed_byte_view_presentation` (787), `whole_representation_length_scale` (1184), `typed_collection_layout_matches` (2669).
- Unit contract: `accepts_graph` (2736) re-evaluates the call plan from the parameter shapes and requires equality with the stored `CallPlan` (2773-2780); `owned_indirect_pointer` (2783); `parameter_views` (2723).

**Inputs/outputs.** Input is a root `StructuralTypeId`, a `&[StructuralPathSegment]`, and the declaration table; output is offsets, sizes, runtime-index element lists and pointer slots. No mutation. Uses `terminal_semantics::*` for rules about what projections are legal (e.g. `descriptor_source_projection_accessible` 988, `element_view_shape_region_geometry` 1012/1023, `primitive_projection_type` 568/646, `runtime_structural_field` 438/672) (read).

**How it decides (keys).**
- Type identity: `find_structural_type(declarations, id)`; several places insist a type id appears exactly once, else `None` (551-557, 626-634, 749-755, 2654-2658).
- Field identity: `field.identity == name` / `field.id == field` (2020-2025, 2266); no spelling.
- Scalar compare by value: `Ok(integer) == IntegerType::new(Unsigned, 8)` etc. (1007-1009, 1623-1625, 2299-2301).
- Access/multiplicity pairs: `collection_view_region_geometry` allow-list at 943-972; `bounded_byte_view` at 1235-1241; `bounded_byte_field_borrow` at 1314-1348; `primitive_store` at 472-476 and 479-505.
- Bounds: `runtime_span_within_storage` (124) and `scalar_access_within_storage` (2385).
- Position: `parameters[i].semantic.position == i` (2757).

**Cases by shape (fixed lists that refuse the rest).**
- `scalar_shape` (72-82): floats, bool, integers of 8/16/32/64 bits; "`_ => None`" (so 128-bit and address-carrier ints refuse).
- `primitive_array_shape` (740-785): only nested `FixedArray` with `ArrayExtent::Literal` ending in a `PrimitiveScalar`; integers with non-`Fixed` carrier refused (767-770).
- `project_storage` (1939-2043): `Case`+`Field` on Sum/Mixed; `FixedIndex/RuntimeIndex` on `FixedArray`/`ElementView`; `Field` on `Mixed` or `Record`; "`_ => return None`" for anything else.
- `shape_inner` (1402-1483): Record with `TargetLayout` storage returns `None` (1441-1446); `Authored/AccessView/Accessor` storage uses stored size (1447-1456).
- `field_shape` (1628-1651): scalar/atomic/bounded/refined/float/byte-sequence/structural fields; `_ => None`.
- `Projection::from` (1853-1864): only `Field`, `Case`, `FixedIndex`, `RuntimeIndex` map; others become `Unsupported`, which falls to `_ => return None` in `project_storage`.
- `primitive_geometry` special-cases byte sequences indexed by `FixedIndex`/`RuntimeIndex` (558-605) with capacity-based payload offsets: BoundedOwned 8, BoundedView 0, BorrowedView 0 (573-581).
- `scalar_field_geometry` byte-length observation allowed only for BoundedOwned byte sequences, BorrowedView with `SharedBorrow`, or `Structural` ElementView/BorrowedView byte-sequence child (2290-2343); requires `u64` unsigned result.
- `reference_projection` (1711): refuses `Owned` access (1719); `WriteOnlyBorrow` referents can't be crossed twice (1745-1749).
- Stack placement: `stack_pointer_offset` (37-65) accepts exactly two location patterns.

**Same job elsewhere.**
- `scalar_shape` is defined four times in the pipeline crates: `structural_inputs.rs:72`, `selection_call_abi.rs:1964`, `legalization_scalar_graph.rs:8601`, `02_.../scalar.rs:2163` (grepped `fn scalar_shape\(`; I read only this one). Suspected same job.
- `runtime_borrowed_projection` is defined here (2061) and in `02_abstract-operations-to-target-operations/src/structural_layout.rs:1076`; `runtime_leaf_copy_projection` at `structural_layout.rs:1050` parallels `leaf_copy_projection` (1815) here (grepped; stage-02 versions not read, suspected same job).
- Inside the file: `reference_source_geometry_with_leaf_type` (266) is called by `descriptor_source_geometry` (175), `owner_loan_geometry` (246) and `reference_source_geometry` (222) and `leaf_copy_geometry` (254) with different leaf-type closures. `descriptor_source_geometry` (175) and `owner_loan_geometry` (246) have byte-identical bodies (both call `reference_source_geometry_with_leaf_type(..., descriptor_leaf_type)`).
- `project_with_leaf_type` (1827) vs `runtime_projection_with_leaf_type` (2089): same selection logic; one drops runtime elements (1839), the other keeps them.
- Struct-field byte-sequence checks (BoundedOwned / BorrowedView / ElementView child) repeat in `scalar_field_geometry` (2290-2343), `projected_byte_field` (2554-2572), `bounded_byte_field_borrow` (1338-1345).
- `descriptor_frame` bypass of bounds check appears at 2392-2400 and again via `root.is_none()` at 2430-2439.

**Unused / one-caller (grepped across `omega-rust`).**
- `fixed_element_array_view` (859): 5 call sites outside the definition (`selection_call_abi.rs:215`, `legalization_scalar_graph.rs:175, 5783, 6055`, plus imports).
- `bounded_byte_field_owner` (1294): 2 calls (`selection_call_abi.rs:207`, `legalization_scalar_graph.rs:6047`).
- `owned_indirect_pointer` (2783): 3 calls (`selection_structural.rs:150`, `selection_inputs.rs:977`, `legalization_scalar_graph.rs:9054`).
- `projected_scalar_fragments` (1548): 1 call (`selection_inputs.rs:3292`).
- `element_view_length_geometry` (157): 1 call (`selection_inputs.rs:3377`).
- `runtime_projection_elements` (1889): 1 call (`legalization.rs:338`).
- `leaf_copy_projection` (1815): 1 call (`selection_inputs.rs:1395`).
- `storage_address_geometry` (235): 8+ calls in `selection_structural.rs`, `selection_inputs.rs`, `legalization_source.rs`, `legalization_target.rs`.
- `owner_loan_geometry` (246): 1 call (`selection_inputs.rs:175`); `descriptor_source_geometry` (175): 7 calls.
- Not grepped: `field_read`, `case_projection`, `byte_field_length`; I did not check their callers.

**Stated limits.** Header only. No TODO or "unsupported" prose in the file.

**Fail-open risks.**
- `scalar_access_within_storage` (2385-2410) returns `true` with no size check when the root type is an `ElementView` or a `ByteSequence(BorrowedView)` (the `descriptor_frame` flag at 2392-2401). Wrong program that gets through: an access with an offset past the end of a borrowed view descriptor frame's declared shape. "open": no comment gives the reason; inferred reason is that a descriptor's backing length is dynamic.
- `runtime_span_within_storage` (124-138): `Some(RuntimeReach::Empty) => true` when any runtime element has `extent == Some(0)` (111-113, 131). Inferred: a zero-length array can never execute the access. "open" (no comment).
- `field_read` (2437-2440): when the root is an `ElementView` the size bound is skipped (`root.is_none() ||`). "open".
- `runtime_reach` fails closed when an element has `extent == None` (118 `element.extent?` -> `None` -> `false` at 136).

---------------------------------------------------------------------------

## 3. `04_.../address_and_load_rewrites.rs` (Omega 04)

**Purpose.** Two pre-allocation rewrites over selected (machine-level) instructions: `address_fold` merges an `AddressOffset` producer into the load/store/AddressOffset that consumes it (add the offsets, read from the base register); `load_forwarding` replaces a load by a register copy/zero-extend when an earlier store to the same place/offset/width provably still governs it (header 1-6).

**Shape.**
- `address_fold::{admission, rewrite, validation}` (11-340): `admission::admit` (81), `consumer_shape` (34), `admitted_offset` (61), `rewritten` (227); `rewrite::fold_selected_address` (244) clones the plan, replaces one instruction, then seals via `validation::seal_address_fold` (286); `AddressFoldReceipt`/`AddressFoldError` (312-337) via macros `validated_transformed_receipt!`, `rewrite_receipt!`, `rewrite_receipt_identity!` (defined in `rewrites.rs`, not read), identity domain string `b"omega.terminal-address-fold.v1\0"` (321).
- `load_forwarding::{admission, rewrite, validation}` (342-1407): `admission::admit` (410-820, ~410 lines) walks backwards over blocks collecting the store value register per block, then resolves predecessors by fixed-point (585-685) with block-parameter merging (`merged_parameter` 1119); `interferes` (842), `forwarding_source` (885), `place_store_value` (976), `sequence_source` (1003), `edge_preserves` (1047), `registers_untouched` (1103), `candidate_accesses` (1175), `reject_barrier` (1194), `reject_unaccounted` (1218), `forwarded` (1230). `rewrite::forward_selected_stored_load` (1298) replaces the load by a copy-shaped instruction and removes its memory-access row (1308-1310). A `#[cfg(test)]` unit test at 1241-1288.
- Receipts: `StoredLoadForwardingReceipt` (1373), identity bytes domain `b"omega.stored-load-forwarding.v1"` (1334).

**Inputs/outputs.** Input: `&impl ValidatedSelectedAnalysis` (a sealed selected-program view), `function_index`, instruction id, `ValidatedTargetRegisterEnvironment`, `OptimizationWorkBudget`. Output: a `Validated*` wrapper holding the transformed `SelectedInstructionPlan` and receipt. Clones the whole plan per rewrite (252, 1306) (read).

**How it decides (keys).**
- Address fold: consumer instruction kind (34-59); the pointer operand's last definition earlier in the same block must be `AddressOffset` (119-131); base register not redefined between (162-168); combined offset legal for the architecture (61-72, 179).
- Load forwarding: the load's single memory-access row (434-444) role+offset+count (445-459); candidate stores keyed on `(place, byte_offset, byte_count)` equality in `forwarding_source` (900-908) and on `SelectedMemoryAccessRole` in `interferes` (848-882); register identity for the forwarded value (`resolved[...]`).
- Constraint rows are checked by operand index/access/class (97-117, 492-511).

**Cases by shape.**
- `consumer_shape` (34-59): only `Load8/16/32/64`, `AddressOffset` (scale 1/2/4/8) and `Store` with `byte_size` 1/2/4/8.
- `admitted_offset` (61-72): `Aarch64`: multiple of scale and `/scale <= 4095`; `X86_64`: `<= i32::MAX`.
- Load forwarding admits `Load8/16/32/64` and `Load8Indexed` only (426-433).
- Forwarded kind by width: 8 -> `CopyI64`, 4 -> `ZeroExtendU32`, 2 -> `ZeroExtendU16`, anything else -> `ZeroExtendU8` (785-790).
- Source store forms accepted: `Store` with `WritePlace` or `WriteLocal` role, `Store64` into a local slot, and a `Store{byte_offset:0, byte_size:1}` with `WriteByteSequence` (909-972, 1012-1018).
- `reject_barrier` (1194-1216): calls, hosted ops, returns, jumps and conditional branches inside a walked block are refused outright.

**Same job elsewhere.**
- `interferes`/`extent_intersects`/`extent_reached_by` and `local_slot_is_place_storage` live in `rewrites::place_storage` (imported at 360-363, not read); `unrostered_reach` in `rewrites::window_hazards` (364). Whether other rewrites in `rewrites.rs`/`copy_and_extension_rewrites.rs` re-implement the same alias reasoning is not known; not read.
- The sealing pattern (`sealed!` macro + receipt + identity) is repeated by every rewrite family in this stage (lib.rs 47-145 lists 11).

**Unused / one-caller.** `forward_selected_stored_load` and `fold_selected_address` together appear 11 times across 3 files (Grep count: `rewrites.rs` 6, `lib.rs` 1, this file 4). `consumer_shape` and `admitted_offset` each have one use (inside `admit`, 97 and 179) (grepped).

**Stated limits.** No prose limits. Error enums name the refusal classes (`UnsupportedInstruction`, `UnsupportedPair`, `AliasingWrite`, ...) (326-337, 1393-1404).

**Fail-open risks.**
- `interferes` (842-883) returns `false` (does not block forwarding) for all read roles and for `WriteOutgoing` and `AddressOutgoing` (880-881). Wrong program that could get through: a store to an outgoing-argument slot that overlaps the forwarded place. "open" for the outgoing case (no comment); inferred that outgoing slots are a separate frame area.
- `reject_unaccounted` (1218-1228): an instruction with no memory-access row is accepted when `unrostered_reach` says `Private`; the classifier is in `window_hazards`, not read. "open".
- `interferes` for `WritePlace` with a different place returns `true` (conservative); `WriteBacking`/`WriteBackingSpan` always `true` (848-852); the unit test at 1245-1287 pins that a backing write can't be disjoint.
- Loop case (`head_looped`, 687-749): scans the tail of the load's own block and rejects if a later write forwards a different register. Fail-closed.
- The merge loops (585-685) only produce `resolved[block_index]` or `UnsupportedPair` (686); a missing predecessor value leaves `None`, which is rejected. Fail-closed.

---------------------------------------------------------------------------

## 4. `04_.../lib.rs`

**Purpose.** Crate root for Omega 04. Documents (3-37) that the stage sits between instruction selection (03) and register-home assignment (05), its entry `optimize_selected_instructions`, and the flat module layout. Defines the sealed trait `ValidatedSelectedAnalysis` (47-145) so that only `SelectedProgram`, `OwnedSelectedProgram`, `SelectedProgramRef` and the eleven `Validated*` rewrite outputs can feed the shared analyses. Everything else is `pub use` re-exports of `rewrites`, `selected_instruction_analyses` and `selected_instructions` identities (157-223).

**Shape.** `mod sealed { trait Sealed }` (63-65); `macro_rules! validated_selected_analysis` (76-100) implements `SelectedProgramView` for a receipt-carrying type by delegating to `transformed()`/`receipt()`; manually written for `ValidatedPreAllocationTransformation` (118-134). `#[macro_use] extern crate selected_instruction_analyses` (39-40).

**How it decides.** Nothing; type-level sealing only.

**Stated limits / claims.** Line 19-23: "We seal `ValidatedSelectedAnalysis` ... so an unchecked `SelectedInstructionPlan` can never enter the shared selected-program analyses." Grepped users of the trait: not counted (not required: the claim is "sealed", not "shared by outside users"). Line 36 says `unexecuted_rewrites.rs` is a "design-record disposition roster".

**Fail-open risks.** none seen (no logic).

---------------------------------------------------------------------------

## 5. `04_.../selected_optimization.rs`

**Purpose.** Stage admission. `optimize_selected_instructions` (114-157): stages liveness (117), live ranges (119); if no executed selections exist returns an `Identity` output (122-126); if any non-executing catalog slice has selections returns `UnsupportedComposition` (127-129); if more than one executed phase has selections, `UnsupportedComposition` (135-137); stages frameless-leaf allocation legality (138); dispatches `SelectedLowering` -> `run_selected_lowering_optimizations` or `PreAllocation` -> `run_pre_allocation_optimizations`; any other phase -> `UnsupportedComposition` (155).

**Shape.** `SelectedInstructionOptimizationEvidence {Identity, LiteralFolds, PreAllocation}` (20-24); `SelectedInstructionOptimizationOutput {current, evidence}` (27-30), built by `from_evidence` which replays/validates the evidence (33-38, 62-85); `SelectedInstructionOptimizationError` (88-97); helpers `executed_slice_phases` (166), `has_executed_selections` (173), `unexecutable_catalog_composition` (177).

**How it decides.** Keyed on `OptimizationExecutionPhase` and on which `SELECTED_STAGE_RULE_CATALOG` slices have `rows()` of kind `SelectedLowering(_)`/`PreAllocation(_)` (159-171). Identity replay rejects if an executed phase has selections (`MissingExecution`, 66-70).

**Cases by shape.** One executed phase at a time; "`_ => Err(UnsupportedComposition)`" (155).

**Same job elsewhere.** `from_evidence` re-validates custody via `validate_*_optimization_custody` (74, 79), the same functions `rewrites.rs` re-exports (`lib.rs:194-195`); `rewrites.rs` not read.

**Unused/one-caller.** `executed_slice_phases` is `pub(crate)`; used at 66, 131, 174 in this file only (read; no grep of other crates).

**Fail-open risks.** none seen: every unmatched case is an error. `.expect(...)` at 133 relies on the check at 122.

---------------------------------------------------------------------------

## 6. `04_.../unexecuted_rewrites.rs`

**Purpose.** Prose-only roster (1-13) of rewrite families that have no catalog row: `arm_relocation`, `boundary_boolean`, `boundary_branch`, `commuting_relocation`, `interchange`, `confluence_relocation`, `inflow_relocation`, `scheduled_relocation`, `relocation` ("owned by `EXACT-MACHINE-SIMPLIFICATIONS`"), `dead_store`, `store_motion` ("owned by `ALIAS-AWARE-MEMORY`"), `peepholes` (`projected_access`) ("owned by `DECLARATIVE-PEEPHOLES`"). Code: an empty `pub(crate) mod peepholes {}` (15). Re-exported as `rewrites::unexecuted` (`rewrites.rs:2730`) and `lib.rs:186`; `grep unexecuted_rewrites|peepholes` finds only those lines (4 files-lines) so the module has no users beyond the re-export.

**Stated limits.** The whole header is a stated limit: these families "do not yet have an active catalog row".

---------------------------------------------------------------------------

## 7. `04_.../copy_and_extension_rewrites.rs` (Omega 04; read in full, 1-1735)

**Purpose.** Header (1-6): three pre-allocation machine-instruction rewrites. `copied_call_operand` makes a direct call read the source of a preceding `CopyI64` instead of the copy's result; `copy_removal` deletes a same-block `CopyI64` whose result is only used later in that block, rewiring the uses to the copy's input and renumbering every later virtual register; `redundant_extension` turns a zero/sign extension into a plain `CopyI64` when the producer of its input already leaves the upper bits in the extended form.

**Shape.**
- `copied_call_operand` (11-756): `admission::admit` (38-260), `pinned_view_written_by_call` (262), `mentioned_after_call` (300), `rewritten` (327). `pair` module (341-617): `CopiedCallOperandRule {producer, consumer, operand_shape, unit_flow, effects}` (377), three consts `COPY_CALL_UNIT/SCALAR/AGGREGATE` (398-412) collected in `COPIED_CALL_OPERAND_RULES` (521); `admits_roster_shape` (424), `admits_consumer_row` (457), `admits_producer_record` (482), `admits_declarations` (507), `isolated_copy` (539), `retained_call` (551), `call_alternative` (581). `rewrite::fold_selected_copied_call_operand` (626). Receipt/identity domain `b"omega.terminal-copied-call-operand.v1\0"` (713).
- `copy_removal` (758-1375): `admit` (859-1022), `apply` (1024-1056), `redensify` (1058-1093) plus `lower_selected_instruction` (1125), `lower_successor` (1145), `lower_local_slot` (1214), `lower_frame_slot` (1224), `lower_register` (1234); `copy_scan` (1342) step counting; `remove_selected_copy` (1257); identity `b"omega.terminal-copy-removal.v1\0"` (1323).
- `redundant_extension` (1377-1735): `extension` (1408), `Normalization` (1421), `producer_normalization` (1428), `identity` (1472), `admit` (1493-1612), `rewritten` (1614), `remove_selected_redundant_extension` (1633); identity `b"omega.terminal-redundant-extension-removal.v1\0"` (1714).

**Inputs/outputs.** Same pattern as file 3: `&impl ValidatedSelectedAnalysis`, function index, instruction ids, `ValidatedTargetRegisterEnvironment` (and `ValidatedMachineEffectCatalog` for the call rule), budget; clone the plan (644, 1265, 1642), edit one place, seal into `Validated*` (read). `copy_removal::apply` mutates registers, slots, memory-access roles, instructions and successor bindings to shift all register ids down by one (1058-1093).

**How it decides (keys).**
- Instruction kind enums (`SelectedInstructionKind`, `MachineSemanticKind`) and the rule table lookup `copied_call_operand_for(CopyI64, consumer_kind)` (77); operand number/access/class/fixed_view/tied_to/early_clobber compared field-by-field between the instruction and its constraint row (98-121).
- Register identity: producer must be the last definition of the copy destination before the call (152-159); the copied source must not be redefined between copy and call (212-221); at most one distinct fixed register view among all mentions of source and destination (190-211); if the call writes the pinned view the destination used, the source must not be mentioned after the call anywhere in the function (222-226, 262-325).
- Effects: producer's `MachineEffectDeclaration` must be memory/trap/barrier/call/cleanup none and alternatives transparent (539-549); the call's declaration must be barrier `Call`, `InternalNormalReturnV1`, and each encoded alternative must match the row's read/write/implicit unit lists and `DirectRelativeCallV1` control (551-616).
- Copy removal: every mention of the output register outside the copy must be a plain `Use` in the same block after the copy, with no fixed view/tie/early clobber (957-973); no mention in a spill/local slot, memory access role, another register's origin, successor transport (926-956, 980-985); the input must not be redefined between the copy and the last use (991-1004).
- Redundant extension: the input register must have exactly one defining instruction in the whole function with exactly one def operand (1556-1578); then `identity(form, normalization)` (1472-1491).

**Cases by shape.**
- Copied call operand: only three consumers: `CallUnit` (no results), `CallScalar` (exactly 1 result), `CallAggregate` (1 or 2 results) (398-412, 436-444); producer only `CopyI64` (132); all consumer row operands must have a fixed view (465-468).
- Extensions recognised: `ZeroExtendU8/16/32`, `SignExtendI8/16/32` (1408-1418).
- Producers recognised (1428-1470): `Load8`, `Load8Indexed`, `Load16`, `Load32`, `LoadPacked` (operand 1 only), the six extensions, `Float32ToBits`, five `MaterializeBoolean*`, `MaterializeI64`; anything else -> `None` -> `UnsupportedProducer`.
- `identity` table (1472-1491): e.g. `Zero{width}` is redundant after `ZeroExtended{n<=width}`, after `MeaningfulLow{n<=32}` only when width is 32 (1475-1477), after a `Pattern` that fits; `Sign{width}` after `SignExtended{n<=width}`, `ZeroExtended{n<width}`, or a pattern equal to its own sign extension.

**Same job elsewhere.**
- Finding an instruction's block position: `member_position(function, id)` (imported from `rewrites::block_edges`) is used by address fold (`address_and_load_rewrites.rs:93`), copy removal (871) and redundant extension (1504), but `copied_call_operand::admit` does its own `blocks.iter().enumerate().find_map(...)` search at 60-72 (read). Suspected duplicate; `member_position` body not read.
- "Is this copy-shaped?" building of the replacement: `selected_operands::copy_shaped` used here (1615) and in load forwarding (`address_and_load_rewrites.rs:1231`); `integer_copy_row` used at 1584 and `address_and_load_rewrites.rs:779`.
- Register renumbering `redensify` (1058) is the only renumbering in the files I read; other rewrites that delete a register would need the same. Not read elsewhere.
- Scan-step charging pattern (`rewrite_scan` + `charge_steps`) is repeated by every rewrite (240-252, 1005-1012, 1590-1602, and in file 3).

**Unused / one-caller (grepped, Grep count mode, `fold_selected_copied_call_operand|remove_selected_copy\b|remove_selected_redundant_extension|copied_call_operand_measured_steps` across `omega-rust`):** 19 hits in 2 files (`rewrites.rs` 11, this file 8); no use outside the stage. The three rule consts `COPY_CALL_*` and `COPIED_CALL_OPERAND_RULES` are used only inside this file (grepped, 9 hits, all here). `CopiedCallOperandShape` has a single variant (355-357), `CopiedCallOperandUnitFlow` a single variant (367-369) and `CopiedCallOperandEffects` a single variant (372-374), so their `match` arms at 428, 483, 498, 513 are single-arm (read).

**Stated limits.** None in prose. The `debug_assert_eq!` at 79-83 states: "every declared copied-call-operand pair rewrites to the consumer's own semantic" (debug builds only).

**Fail-open risks.**
- `identity` (1472-1491): `Zero{32}` after `MeaningfulLow{n<=32}` returns `true` (1475-1477), where `MeaningfulLow` is the class given to `ZeroExtendU32`, `Float32ToBits`, and `LoadPacked` outputs (1438-1447, 1451-1453). So a 32-bit zero extension is removed when its input comes from `Float32ToBits`/`LoadPacked` without any proof in the file that the upper 32 bits are zero (the name "MeaningfulLow" suggests only the low bits are defined). Wrong program that gets through: if an instruction classed `MeaningfulLow` can leave nonzero upper bits in the carrier register and the extension result is later read as a 64-bit value. "open": no comment at the site; I did not read the instruction semantics (`selected-instructions`) to decide; for `ZeroExtendU32`'s own output the claim is plausible because a 32-bit move zero-fills on the architectures targeted, but that is inferred.
- `producer_normalization` for `MaterializeI64` truncates `IntegerValue::Unsigned(u128)` with `as u64` (1464); only relevant if a materialization carries more than 64 bits; "open", inferred unreachable because the instruction is `MaterializeI64`.
- The single-producer search for redundant extension scans `block.instructions` only, not terminators (1556-1563). A value defined by a terminator instruction would have no producer -> refused (fail closed).
- `copied_call_operand`: `row.operands[0]`/`[1]` indexing at 175-178 follows `declares_pair` (171); fail-closed otherwise.
- None of the three has a path that accepts on a missing row: every `find_register`/`environment.constraint` miss returns an error (87-97, 168-170, 1544-1546).

---------------------------------------------------------------------------

## Coverage

Fully read: files 1-6 and 7 above:
- `03_.../selection_views_and_bytes.rs` 1-2565 (2565 lines)
- `03_.../structural_inputs.rs` 1-2809 (2809)
- `04_.../address_and_load_rewrites.rs` 1-1407 (1407)
- `04_.../lib.rs` 1-223 (223)
- `04_.../selected_optimization.rs` 1-188 (188)
- `04_.../unexecuted_rewrites.rs` 1-15 (15)
- `04_.../copy_and_extension_rewrites.rs` 1-1735 (1735)
Total read in full: 8942 lines.

Not read (no claims made about them):
- `04_.../branch_and_compare_rewrites.rs` 1-2385 (2385)
- `04_.../literal_fold_rewrites.rs` 1-4575 (4575)
- `04_.../rewrites.rs` 1-2914 (2914)
- `05_selected-instructions-to-register-homes/src/assignment.rs` 1-3052 (3052)
Total not read: 12926 lines.

(Note on the header list at the top of this file: the section numbering of the files differs from the unit-file order; the unit-file order is selection_views_and_bytes, structural_inputs, address_and_load_rewrites, branch_and_compare_rewrites, copy_and_extension_rewrites, lib, literal_fold_rewrites, rewrites, selected_optimization, unexecuted_rewrites, assignment.)

## Five things a maintainer most needs to know about this unit

1. `structural_inputs.rs` is the single geometry oracle for stages 02/03, but its core helpers are re-implemented per leaf-type closure and several are duplicated across crates: `scalar_shape` has four definitions (`structural_inputs.rs:72`, `selection_call_abi.rs:1964`, `legalization_scalar_graph.rs:8601`, `02_.../scalar.rs:2163`) and `runtime_borrowed_projection` has two (`structural_inputs.rs:2061`, `02_.../structural_layout.rs:1076`) (grepped; only the first of each read).
2. All geometry functions return `Option`; `None` means reject. The three bounds checks that skip checking are: descriptor-frame roots in `scalar_access_within_storage` (`structural_inputs.rs:2392-2401`), `Empty` reach in `runtime_span_within_storage` (111-113, 131), and the ElementView root in `field_read` (2437-2440). None carries a comment.
3. `selection_views_and_bytes.rs` is a set of per-instruction lowering routines that all fail with the same bare `invalid()`; there are two parallel stride lookups (`element_views::element_stride` 2319 and `descriptor_operands::element_stride` in `selection_structural.rs:762`), and three separate code paths that emit `CopyBytes` (`byte_concatenation::copy` 199, `byte_field_store::replace` 769, `byte_copy::copy` 272).
4. Omega 04's rewrites share one skeleton: admit (read-only checks against the register constraint table and the effect catalog) -> clone the whole plan -> edit one instruction -> `sealed!` into a `Validated*` receipt whose identity bytes carry a domain string like `b"omega.stored-load-forwarding.v1"`. Every call clones the entire `SelectedInstructionPlan` (`address_and_load_rewrites.rs:252, 1306`; `copy_and_extension_rewrites.rs:644, 1265, 1642`).
5. `load_forwarding::admit` (`address_and_load_rewrites.rs:410-820`) is a ~410-line backward dataflow walk with its own predecessor fixed point and block-parameter merging; its alias reasoning (`interferes` 842-883) treats `WriteOutgoing`/`AddressOutgoing` as non-interfering and relies on `unrostered_reach` (in `window_hazards`, unread) for instructions with no memory-access row.

## What I am unsure about

- Whether the `MeaningfulLow` normalization rule (`copy_and_extension_rewrites.rs:1475-1477`) is sound for `Float32ToBits` and `LoadPacked`; needs the machine semantics in `selected-instructions` that I did not read.
- Whether `byte_views::charge_backing_roots` / `byte_concatenation::operand_span` fall back to the place's own storage when `view_backing_roots` (`selection.rs:337`) returns `None` for a view whose roots are unknown, versus returns `None` only for non-views.
- Which of `field_read`, `case_projection`, `byte_field_length`, `primitive_store`, `atomic_cell_geometry` in `structural_inputs.rs` have callers; I did not grep them.
- The macros `sealed!`, `validated_transformed_receipt!`, `rewrite_receipt!`, `rewrite_receipt_identity!`, and helpers `plan_function`, `charge_steps`, `rewrite_scan`, `carrier_pair`, `declares_pair`, `integer_copy_row` live in `rewrites.rs` or `selected-instruction-analyses` and were not read; statements about them are inferred from their call sites.
- The per-file "same job elsewhere" claims about stage-02 duplicates are suspected only (grepped definitions, bodies not read).

