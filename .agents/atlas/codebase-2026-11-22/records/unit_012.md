# Record: unit_012 (area 10-native-stages-04-10)

Crate: `source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions/src/instruction_selection/`.
All five files are modules of the stage 05 crate. `mod.rs:17-21` declares them (grepped). `selection.rs` and
`selection_structural.rs` re-export their inner modules under shorter names, for example `selection.rs:1869`
`selection_call_abi as scalar_call_abi`, `selection.rs:756` `selection_edge_transfers as edge_transfers`,
`selection_structural.rs:446` `selection_access_lifecycles as access_lifecycles` (grepped).
A "source" in the code below is `NativeFunction`, the function being selected, which carries its blocks, its
structural signature and its type geometry. Stage 05 reads it and does not mutate it.

---

## 1. selection_access_lifecycles.rs (6,968 lines, read in full)

### Purpose
Emits the selected instructions for the four resident-access lifecycle operations on a storage extent or run:
`Initialize`, `Relocate`, `View` and `MoveOut` (extraction). At run time the emitted code reads a caller-supplied
"resource profile" record from memory, checks that it is well formed, checks it against the compile-time placement
(`ValidatedPlacement`), and then either does the copy/commit or writes a typed rejection into the result sum.
It also holds the byte-image encode/decode codec for the schema fields (`ScalarImageCodec`, `transfer_image`).

### Shape
- Top-level `emit` (`:6186`) finds the `TerminalAccessLifecyclePlan` by `(boundary, operation)` in
  `signature.structural_types.access_lifecycles()` (`:6205-6210`) and dispatches to `initialization::emit` (`:36`),
  `view::emit` (`:6029`), `relocation::emit` (`:563`), or handles `MoveOut` inline (`:6219-6392`).
- `initialization` (`:23-372`): `emit`, `stage_image` (`:283`), `commit` (`:347`; one `CopyBytes` of a staged image into the backing).
- `outcome` (`:373-552`): `PlacementDecision{ready,outcome_tag,failure_tag,copy_length}`, `decide` (`:404`),
  `compare` (`:483`), `write_failure` (`:509`), `write_word` (`:380`). `decide` builds the ready/rejection bits from
  arithmetic on registers, not branches.
- `relocation` (`:553-894`): checks both profiles per run (`run::assess`), then checks the two runs are disjoint or empty (`:786-802`).
- `resource_profile` (`:895-4362`):
  - `compatibility` (`:902-1944`) with `COMPATIBILITY_SCRATCH_BYTES = 200` and scratch slot offsets (`:1146-1165`);
  - `run` (`:916-1144`) with `SCRATCH_BYTES = 240`; `assess`, `check_cell`, `boundary_cell` (a binary search emitted as a native loop, `:1091`);
  - `assess` (`:1735`) and `assess_cell` (`:1768`); `extend_coverage` (`:1594`, nested loops over up to 32 regions); `equivalent`, `reaches`, `effective_alignment`, `combine_alignment`;
  - `geometry` (`:1945-2295`): `Node`, `ProfileLayout::derive` (`:2235`) reads the profile type's native layout;
  - `Emission` helper (`:3942-4230`) and `validate_layout` (`:4246-4361`), which checks region and rule well-formedness at run time.
- `result_layout::ResultLayout::derive` (`:4363-4652`) reads the Ready/Rejected sum layout and returns byte offsets.
- `view::emit` (`:6021-6169`).
- Helpers: `failure`, `coordinate`, `offset` (`:6171-6184`), `operand_indices` (`:6395`), `owner_word` (`:6412`),
  `ImageHome` (`:6437`, with `scratch` at `:6445` that allocates one `LocalStorageSlotId::Boundary` slot), `transfer_image` (`:6508`),
  `field_region` (`:6599`), `aggregate_chunks` (`:6633`), `ScalarImageCodec` (`:6749`, `Bytes` or `Fragments`).
- Tests: `resource_profile::tests` (`:2296-3924`) and top-level `tests` (`:4654-6020`). They include a small interpreter
  (`execute_with_frame` `:4781`, `execute_operation` `:4903`) that runs the emitted selected instructions on a byte array,
  over all four targets.

### Inputs and outputs
Reads the legalized row (`AccessLifecycleOperands::{Initialize,Relocate,View,MoveOut}`), the plan (placement, storage
roles, schema, image_fields), and `NativeGeometry` for field offsets and sizes. Mutates `Builder` (instructions, registers,
`transport.local_slots`, `transport.pointers`, `transport.memory` footprints). Produces instructions that fill the
result sum's tag, rejection words and, on ready, the moved owner/value. A well-formedness failure of the profile goes
through `operation_control::require_nonzero`, which ends in an `OperationTrap` crash block, not a Rejected result
(test `:2761-2770`, "malformed supply must not return Rejected").

### How it decides
- Compile-time decisions are keyed on `plan.operation` (an enum), `plan.storage.owner` (`Extent` vs `Run`), and structural ids:
  `field.id == storage`/`count` (`:665-672`), and `plan.storage.carrier`/`plan.schema` identity in `ResultLayout::derive` (`:4453`, `:4460`, `:4546-4556`).
- Run-time decisions are encoded as register arithmetic over the supplied profile bytes and the actual base/length words.
  Examples: ready = range AND supply AND alignment (`:458-460`); the congruence is valid only if the modulus is a power of two (`:437-443`).
- Image transfer matches schema fields to access-plan slots by position and by layout entry name:
  `plan.image_fields.iter().zip(access)` (`:6528`) and `entry.field == slot.field()` (`:6538`). That `field` is a string
  (`LayoutFieldEntryReport.field: String` as built in the test `:3586` via `"value".into()`; read, inferred for the production type).
  So the join is by field name spelling, plus a length check (`:6525`).

### Cases by shape
- `ResultLayout::derive` accepts exactly: a sum of two cases `[ready, rejected]`, ready with one field, rejected with two fields (`:4431-4440`);
  reason sum of exactly three cases `[range, alignment, incompatible]` with the third empty (`:4575-4580`); each geometry pair is two u64 words (`:4581-4593`).
  Everything else returns `failure(..)`.
- `ProfileLayout::derive` demands one fixed shape: regions counted array of capacity 32 with 6 fields
  `[range, peer, stable, external, atomic, reach]`; external rules capacity 16; atomic rules capacity 16 with 12 boolean operation fields;
  reach capacity 32 (`:2240-2274`). Quote: "resource profile record changed its complete field roster" (`:2043`). Sum case counts are hard-coded (`tag::<2>`, `tag::<4>`, `tag::<3>`, `:2280-2283`).
- `aggregate_chunks` (`:6633`) accepts only `At` placements and either a fixed literal-length array or one contiguous non-array placement. Comment at `:6673-6675`: "The owned field's extent includes its selected stride and padding."
- `ScalarImageCodec::placement` (`:6786`) accepts one `At`, one `IntegerAt`, or one or more `Bits` fragments. Widths are 1 to 8 bytes only (`:6790`). It refuses a mix of families (`:6832`).
- `effective_alignment` (`:1438-1442`) refuses `FieldAccess::Inaccessible | Region` with a failure.
- Constants: `if let Some(maximum_count) = u64::MAX.checked_div(footprint)` skips the count bound when `footprint == 0` (`:970`).

### Same job elsewhere
- The run-time loop idiom `emission.repeat` (`:4141`) and `interrupt_installation_guards::checks::iterate` (`selection_interrupts.rs:849`) do the same job (scratch-slot counter loop with `while_nonzero`). Both read (not shared code).
- `outcome::compare` (`:483`), `Emission::compare/equal/at_most` (`:3968-4004`), and `Checks::compare` (`selection_interrupts.rs:667`) all emit `CompareI64` + `MaterializeBoolean*`. Three copies in this unit; `static_interrupts::require_equal` (`selection_interrupts.rs:2058`) is a fourth.
- `ImageHome::scratch` (`:6445`) and `interrupt_installation::control_home` (`selection_interrupts.rs:270`) both allocate a single `LocalStorageSlotId::Boundary{operation}` slot, push a local slot, declare `AddressLocal` memory and emit `FrameAddress`. Same job.
- `descriptors::copy` (`selection_edge_transfers.rs:875`) copies bytes in `extent_copy::chunks`; `transfer_image` copies byte by byte (`:6571`). Different method, same intent.
- Suspected, not read: `leaf_copy::copy_operand` / `project_operand`, `record::initialize_storage`, `fragmented_scalar::*` live in `selection_structural.rs` / `selection_scalar_graph.rs` (outside this unit).

### Unused or one-caller code
- `validate` under `#[cfg(test)]` (`:4233`) is test-only.
- `decode_scalar` is `#[cfg(test)]` (`:6726`).
- `atomic_bits` (`:1374`, `pub(crate)`) is used in `effective_alignment`/`select_rules` and in the test `encode`; not grepped outside the file.
- `PlacementDecision.failure_tag` is read by `write_failure` (`:523`); `.ready/.outcome_tag/.copy_length` used by emitters.
- `access_lifecycles::emit` has its call sites in other files (`selection_structural.rs` re-export at `:446`). I did not grep the dispatch site. Suspected one caller.

### Stated limits
- `:6676` ("do not cover the complete array" refusal) via `failure("resident element placements do not cover the complete array")`.
- `:3585-3586` (Scratch size): none stated as a limit; scratch sizes are hard constants (`:919`, `:1146`, `:3933`).
- No TODO or migration-debt comments seen.

### Fail-open risks
- `:970` `if let Some(maximum_count) = u64::MAX.checked_div(footprint)`: when `footprint == 0` no bound is applied to `count`. Traced: the zero-footprint case then returns early after `check(builder, zero)` (`:1018`). `count * 0 == length` still holds only if `length == 0`; `occupied = count*stride` is exact-compared with `length` (`:967-969`). Low risk, "traced".
- `:1003-1004` run supply: with `role == Source` the code returns before the per-cell checks (`:1000-1002`). Source runs are checked only by coverage (`extend_coverage` with `Stable{read:true}`), not per cell. Intentional by the code split ("traced" structure; no comment states why).
- `:1803` `operation != View` skips the whole-extent coverage pass for `View`. Per-field descriptor and region checks still run (`:1825-1934`). "open" (no comment).
- `ScalarImageCodec::field` `signed` is derived from `field.storage.stored_scalar` only (`:6774`); a field without authored storage is always unsigned. "open".
- `transfer_image` Encode/Decode skips nothing silently: absent field → failure (`:6532`). None seen there.
- Fail-open on lookups: `.find(...)` results always go through `.ok_or_else(failure(..))`. None seen elsewhere.

---

## 2. selection_call_abi.rs (2,326 lines, read in full)

### Purpose
Validates, for each call row, that the stored calling-convention plan (computed earlier in stage 04, "legalization") agrees
with the call's arguments, its source geometry, the callee constraint row, and the register environment. It also picks
the register-constraint key for a call. It says (`:1943-1945`) that "Selection checks the roles and arguments it is about to encode; it does not compute the plan again."

### Shape
- `borrowed_argument::validate_borrowed_argument` (`:31-491`, about 460 lines), `validate_reference_argument` (`:493`), `projected_loan_shape` (`:515`), `stored_argument_placement` (`:577`).
- `input_site` (`:600-927`): `use_cut` (`:631`), `producer_before_use` (`:721`), `cleanup_call` (`:693`), `no_normal_result` (`:742`), `installation_call` (`:604`). Test `cleanup_tests` (`:768`).
- `normalized_foreign::validate` (`:1062-1533`), `call_key` (`:1030`), `call_constraint` (`:1048`), `structural_parameter_positions` (`:975`).
- `owned_argument::validate_owned_arguments` (`:1544`).
- Top level: `validate` (`:1895-2223`), `unit_key` / `no_normal_key` / `invocation_key` (`:1797-1888`), `register_argument_count/order` (`:1735-1754`), `scalar_stack_placement` (`:1706`), `accepts_stack_parameter_entry` (`:1686`), `integer_carrier_normalization` (`:2229`), `incoming_float_transfer` / `outgoing_float_transfer` / `load_transfer` (`:2249-2295`), `scalar_shape`, `scalar_value_type`.

### Inputs and outputs
Inputs: `NativeFunction`, `LegalizedScalarCall` (arguments, `call_plan`, `result_placement`, `structural_result`), a `CallSiteOwner`,
a `RegisterConstraintKey` and `RegisterInstructionConstraint` row, and `ValidatedTargetRegisterEnvironment`. Output: `Result<(), _>` or `Option<()>`; `*_key` functions return a key.
Nothing is mutated.

### How it decides
- Identity-keyed: `semantic.place == target.place`, `target.root_structural_type == root`, producers matched by `OperationId`/`BlockId`, `use_cut` by `InvocationSite` identity, and "exactly one producer" (`:735-737`).
- Position-keyed: the constraint row's operand list is compared by index with the argument order sorted by float-class (`register_argument_order` `:1750-1752`, sorts floats last), and `row.operands.len() == register_count + result locations` (`:2064-2066`).
- Key choice in `invocation_key` (`:1860-1866`) selects among `call_aggregate`, `call_scalar`, `call_unit`/`call_unit_mixed` by whether the result is structural and non-empty; it then finds the one row whose operand `fixed_view`s equal the register views, and returns `None` if two match (`:1884-1886`).
- Not keyed on a display name; no string compares seen in this file.
- `normalized_foreign::validate` uses a local `invalid!()` macro returning `unsupported_shape(function)`; all failures carry the same error.

### Cases by shape
- `validate_borrowed_argument` is a long if/else ladder over source kinds (`:286-372`): byte_field, address_join, storage_loan, block_forward, byte_view, byte_owner, element_view, established_view_loan, local, projected_loan, else shared-borrow fallback; "exclusive" or scalar-typed targets return `None` (`:360-369`).
- `scalar_stack_placement` (`:1706`): one stack location, `byte_size` of 4 or 8 (1 or 2 for integers only) (`:1722-1723`).
- `direct_fragments`-like fixed forms live in selection_inputs.
- `validate` (`:1923-1942` and onward): `register_count` is computed from placements; stack-passed scalar and pointer-offset placements are handled by `scalar_stack_placement` / `stack_pointer_offset` branches (`:2123-2137`); remaining register forms only `Register{byte_size 8, BorrowedReference|BorrowedScalarReference}`, `Register{byte_size in 1|2|4|8}` or `Indirect{Register, copy None}` (`:2151-2198`). Others return `invalid()`.
- `integer_carrier_normalization` (`:2229-2247`): u8/u16/u32/i8/i16/i32 and Boolean get an extend; "`_ => CopyI64`", including all 64-bit and non-integer kinds.
- `load_transfer` only for byte sizes 1, 2, 4, 8 (`:2288-2294`).
- `normalized_foreign::validate`: callbacks, operand count, scalar and structural arguments, result as `Unit | Scalar | Structural | ScalarReference`. Others `invalid`.

### Same job elsewhere
- `register_argument_order` and key matching: `selection_scalar_graph.rs` calls these (grep count 28 for the group, see below); suspected shared.
- `owned_argument::validate_owned_arguments` and `borrowed_argument::validate_borrowed_argument` both re-derive `use_cut`, `target.*==semantic.*` comparisons and `runtime_indices_match` over targets. Same job twice in this file (`:1560-1587` and `:394-409`).
- `projected_loan_shape` (`:515`) and `selection_inputs::address_join_input::projection` (`selection_inputs.rs:115`) both call `runtime_borrowed_projection` / `reference_source_geometry` and compare `target.*` fields (read).
- Stage 04 also validates: grep of `validate_owned_arguments|validate_borrowed_argument|call_key|register_argument_order` shows hits in `04_.../legalization.rs`, `legalization_source.rs`, `legalization_target.rs` (1 each). Suspected duplicate of the call-plan check on the other side of the stage; not read.

### Unused or one-caller code
Command: Grep count over `source-rust` for `accepts_stack_parameter_entry|scalar_stack_placement|incoming_float_transfer|outgoing_float_transfer|integer_carrier_normalization`:
`selection_call_abi.rs` 6 (definitions plus uses), `selection.rs` 2, `selection_edge_transfers.rs` 2, `selection_scalar_graph.rs` 28, `selection_structural.rs` 3.
Command: Grep for `fn (validate_owned_arguments|validate_borrowed_argument|call_key|register_argument_order)|owned_argument::validate_owned_arguments|normalized_foreign::(validate|call_key|call_constraint)`: `selection_call_abi.rs` 5, `selection_scalar_graph.rs` 3, `04_*/legalization*.rs` 3 files, 1 each.

### Stated limits
- `:2224` comment block at `:1943-1945`: plan is not recomputed.
- Doc comment `:572-576` on `stored_argument_placement`: "Legalization stored the plan; selection does not compute it again."
- `:332-336` byte_owner shape is `borrowed_reference(capacity+8, 8)`.
- No TODO text.

### Fail-open risks
- `:384-386` `source.attachment.is_some_and(|a| !find_structural_type(..).is_some())` is a rejection guard; if `attachment` is `None` it accepts. "traced": `attachment` is a legalization-side field; `None` means no attachment, so nothing to check.
- `:386-391` `(!parameters.is_empty() && !accepts_graph(..))`: when `parameters` is empty the graph check is skipped. A function with no structural parameters has nothing to check. "open".
- `:1978-1981` owned-argument validation runs only if some argument has `Owned` access; borrowed arguments are validated in the next loop. Each call is validated by exactly one loop per argument. None seen.
- `:2105` for `index == count && aggregate.is_some()` the result locations are only required to be `Register`. Other forms return `invalid`.
- `:2229-2247` `integer_carrier_normalization` falls to `CopyI64` for unknown scalar kinds (floats, 128-bit): a non-matching case does not refuse; it emits an unextended copy. "open" (no comment; callers presumably only pass integer-class values).
- `input_site::use_cut` returns `None` on duplicates (fail-closed).
- `invocation_key` returns `None` on two matches (fail-closed).

---

## 3. selection_edge_transfers.rs (1,662 lines, read in full)

### Purpose
After blocks are selected, this creates "bridge" blocks on control-flow edges that need work: copying structural values into the
destination block-parameter homes, snapshotting descriptors and scalar references, snapshotting enum payloads for structural-case
branches, loading payload fields into registers, and running per-edge preparation/cleanup instructions.

### Shape
- `construction::prepare` (`:379-690`): the main pass over every successor of every block. `EdgeWork{edge, preparation, cleanup}` (`:1617`).
- `construction::structural_case::snapshot` (`:121-376`).
- `descriptors` (`:692-1294`): `snapshot` (`:711`), `store` (`:759`), `copy` (`:875`), `bulk_copy` (`:970`), `memory_pointer`, `frame_address`; tests at `:1065`.
- `emission::Emission` (`:1309-1458`) with `emit`, `store_local_word`, `source_reads`; `pointer_type`, `provenance`.
- `scalar_references` (`:1475-1614`), `addresses::store` (`:44`).
- Helpers: `invalid` (`:1623`, returns `FunctionProjectionMismatch`), `instruction_count`, `stored_transport` (`:1635`).

### Inputs and outputs
Reads `SelectedFunction.blocks`, each successor's `bindings`, `structural_bindings`, `structural_case`, `psi_edge`, `role`, and the supplied `edge_work`. Mutates the function: it appends bridge blocks, adds virtual registers, `contracts.memory_accesses` and `local_storage_slots`, and retargets each successor to its bridge. Afterwards the successor's transports are set to `Unused` (`:671-682`).

### How it decides
- A successor needs a bridge if it has preparation, cleanup, active register bindings, payload work, or any non-`Unused` structural transport (`:439-447`). Otherwise it is skipped (`continue`, `:449`), with one exception: the fall-through slot of a conditional (position 1) is tracked in `fallthrough_next` to avoid creating a cycle of unbridged fallthroughs (`:448-470`).
- Edge work is matched to a successor by `psi_edge` identity and `role == Semantic` (`:411-413`).
- `OperationContinuation` successors get a plain `Jump` bridge and must carry no edge, work, bindings or case (`:476-516`).
- The snapshot loop runs two phases (`for phase in 0..2`, `:546`): phase 0 copies values into fresh registers, phase 1 copies from those snapshots into the transfer registers, so a parallel assignment cannot read a value already overwritten.
- Key: `LocalStorageSlotId::EdgeSnapshot{edge, binding_index}` (`:724`); duplicates return `invalid` (`:728`).
- Chunked copy vs bulk copy is chosen by `byte_size > extent_copy::INLINE_MAX_BYTES` (`:880`).

### Cases by shape
- `structural_case::snapshot` handles payload fields of scalar width 1, 2, 4, 8 only (`:169-175`); other widths `invalid`. It needs `field_byte_offset % byte_count == 0` and in-bounds (`:179-185`). It normalizes only signed 8/16/32 integers after loading (`:335-337`).
- `Registers` transport in a case payload before snapshot is refused (`:161`).
- `stored_transport` returns `Some` only for `WholeValue` (`:1635-1662`).
- `addresses::store` handles `Address` transport only (`:26-38`).
- Test at `:1185` fixes sizes `(0,1,15,4096)` and `4097`.

### Same job elsewhere
- `descriptors::copy` chunking vs `access_lifecycles::transfer_image` byte loop (see unit file 1).
- `Emission::emit` (`:1335`) vs `Builder::emit` used by files 1 and 5: same job (append an instruction, create an output virtual register with `AbiTransport` origin) done twice; `Builder` is in `selection_structural.rs`, not read (suspected).
- `structural_case::snapshot` repeats the register-and-instruction creation seen in `Emission::emit` by hand (`:198-231`, `:262-297`, `:338-366`) instead of calling it. Same file.

### Unused or one-caller code
Grepped `folded_zero|folded_immediate` (count: `selection_inputs.rs` 2 (definitions), `selection_scalar_graph.rs` 6). Not edge-related; see file 4.
`EdgeWork` / `prepare` re-export: `selection.rs:756` (`edge_transfers`). Callers not grepped (suspected one caller in the driver).

### Stated limits
None stated. `snapshot` bails with `invalid` for unsupported shapes without comment.

### Fail-open risks
- `:419-421` `successor.structural_case.is_some() && !successor.bindings.is_empty()` → refuse. Good.
- `:448-470` the skip path: a conditional's fall-through successor with no work is left pointing directly at the target. Traced: correct provided blocks are laid out in `fallthrough_next` order; the cycle guard prevents infinite chains. "traced" at the data-structure level only; I did not follow the later layout stage.
- `:685` leftover `edge_work` is an error, so unmatched work is not dropped. None seen.

---

## 4. selection_inputs.rs (6,995 lines, read in full)

### Purpose
A collection of 19 small "input" modules that each answer one question for selection: given a place or an operation row, is it
a valid X (an address join, an aggregate result home, an arrival parameter, an established view, a primitive local, a reference
establishment...), and what are its shape, offset and indices. The module comment (`:1-7`) calls them "input projections consumed by instruction selection". Almost every function returns `Option` and returns `None` on any mismatch.

### Shape (module → main functions, `file:line`)
- `activation_storage_input` `:9`: `footprint`, `local_shape` (activation claim + bounded owned byte backing).
- `address_join_input` `:71`: `join` (`:90`), `projection` (`:115`), `projection_base`, `storage_base`, `root_type` (`:347`), `loan_declaration`.
- `aggregate_result_input` `:471`: `parameter_home_required` (`:507`, 175 lines matching on about 30 instruction kinds), `call_result` (`:762`), `returned_*`, `fields` (`:1206`), `block_parameter_transport` (`:1324`), `operation_result` (`:1550`), `home` (`:1583`), `block_home` (`:1599`), ABI placement predicates `direct_fragments`, `owned_argument_placement`, `inline_argument_fragments`, `indirect_argument`, `indirect_result`.
- `arrival_parameter_input` `:1628`: `parameter`, `store_destination`.
- `byte_concatenation_input` `:1709`, `byte_owner_input` `:1858`.
- `established_view_input` `:2004`: `access::view_access` (budgeted chain walk, `:2150-2371`), `projected_source_access`, `requires_descriptor`, `transferred`, `view_type`, `element_view_type`, `accepts_at`, `accepts_view_loan`, `completed_descriptor`, `descriptor_home`, `descriptor_result`.
- `literal_compare_input` `:2942`: `folded_zero`, `folded_immediate`.
- `native_code_layout_input` `:3021`: `valid`.
- `owned_block_home_input` `:3136`, `primitive_local_input` `:3167` (`local`, `read_parameter`, `read_storage_codec`, `read_geometry`), `record_input` `:3578` (`fields`, `field_shape`, `structural_operand_shape`, `projected_operand`), `reference_input` `:3985` (`source_access`, `establishment` `:4117`, `root_type`, `referent_projection`, `store_destination`, `borrowed_argument`; tests at `:4549`), `scalar_array_input` `:5211`, `storage_origin_input` `:5312` (origin inventory and `replay_memory_origins`), `structural_array_input` `:6665`, `structural_local_input` `:6750`, `zeroed_structural_input` `:6783`.

### Inputs and outputs
All read `NativeFunction` (blocks, parameters, structural signature, `geometry()`), legalized rows, and the geometry queries in
`structural_reference_input` (outside this unit, suspected size in another file). Outputs are `Option`
tuples (type ids, byte offsets, `ValueShape`s, pointer slots, element indices) or `Option<()>` used as pass/fail. `storage_origin_input::replay_memory_origins` mutates a `Vec<SelectedMemoryAccess>` in place (`:5474`).

### How it decides
- Identity keys: `PlaceId`, `OperationId`, `BlockId`, `StructuralTypeId`. The uniqueness idiom repeats everywhere: take an iterator, `next()`, then require `next().is_none()`
  (`:96-97`, `:1606-1609`, `:1668-1686`, `:3203-3204`).
- `operation_result` (`:1550-1581`) matches a place to its single producing instruction and also to a `StructuralPlaceKind::OperationResult{producer, structural_type}` declaration.
- `view_access` (`:2172-2371`) walks a chain of descriptor sources with a step budget equal to the number of declared structural places (`:2154`); returns `None` when the budget is exhausted or the chain breaks (the final `None` at `:2370`).
- `literal_compare_input::folded_literal` (`:2951-3002`) keys on position: the literal must be the instruction directly before the compare (`comparison_index - 1`), must be unsigned 64-bit, in a fixed numeric range (`0..=0` or `1..=4095`, `:3009`, `:3017`), and no other instruction or terminator in any block may read the literal's value (`:2988-3000`).
- `native_code_layout_input::valid` (`:3029-3133`) demands one exact structure (below).
- `storage_origin_input` decides by a graph walk over `StructuralArgument` coordinates and by op kind (`:5565-5639`, `:5739-6069`); `scalar_storage_origins` returns `Ok(None)` when any origin is unknown (`:5448-5450`).
- `parameter_home_required` decides "is the parameter stored in a home?" by scanning all blocks, terminators and instructions for any use matching a hard-coded list of operation kinds (`:579-681`). Source spelling is not used; no string compares in this file except test names.

### Cases by shape
- `native_code_layout_input::valid`: exactly `fields.len() == 2`, `*shape == ValueShape::integer(24, 8)`, first field address-64 at offset 0 width 8, second an `ElementView` with `access` None or `SharedBorrow` of shape `integer(16, 8)` at offset 8, region record with two fields at `(0,8)` and `(8,8)` (`:3052-3132`). All else `false`.
- `literal_compare_input`: only `Equal`, unsigned 64-bit, literal in `0..=0` or `1..=4095` (comment-free; `:3009`, `:3017`).
- `scalar_array_input::shape` (`:5250`): nested `FixedArray` ending in a `PrimitiveScalar` at depth ≥ 1.
- `structural_array_input::elements` (`:6676`): `FixedArray` or owned `ElementView{access: None}`.
- `zeroed_structural_input::storage` (`:6826`): matches the leaf kinds `RecordField`, `EmptyView{field: Some}`, `CaseField`, `ArrayScalar`, `EmptyScalarArray`, `ByteArray`, `SumSelection`. `SumSelection` requires the first case (`cases.first()`, `:6937`) and `CaseField` the first case (`:6972`). Comment `:6870-6871`: "Typed immutable sequences retain real backing in Psi." and the next line says native descriptor initialization needs its layout owner; code returns `None` for non-byte `EmptyView` (`:6872`).
- `aggregate_result_input::call_result` (`:762-818`): shapes `Record|FixedArray`, `Reference|Sum|Mixed`, `BoundedOwned` byte sequence, descriptor with non-linear multiplicity.
- `direct_fragments` (`:1096`): integer class and one or two register locations of 1 to 8 bytes (`:1102-1123`). `owned_argument_placement` (`:1004`) allows register or stack locations of 1 to 8 bytes (`:1031`).
- `record_input::fields` (`:3609`) / `aggregate_result_input::fields` (`:1206`): sum or mixed only; erased fields dropped.
- `block_parameter_transport` (`:1324`) handles `Owned` with static projection or borrowed address projection; `presentation` is `Unchanged | AddressOfBacking` (`:1492-1514`).

### Same job elsewhere
- Local/parameter/place lookups repeat: `arrival_parameter_input::parameter` (`:1664`), `primitive_local_input::read_parameter` (`:3223`) + `block_arrival` (`:3237`), `reference_input::root_type` (`:4324`), `address_join_input::root_type` (`:347`), `record_input::place_parameters` (`:3717`), `byte_concatenation_input::owner_layout_type` (`:1749`), `established_view_input::declared_view_type` (`:2596`). Each re-finds "the declaration for this place" with its own priority order (parameter, block parameter, operation result, local, literal). Read; seven versions in this file.
- `descriptor_result`/`descriptor_home`/`completed_descriptor` (`:2876-2939`) vs `aggregate_result_input::home` (`:1583`).
- `parameter_home_required`'s instruction-kind list (`:579-681`) overlaps `established_view_input::requires_descriptor` (`:2481-2534`): both list instruction kinds by which operand place is a source. Same job, two lists.
- `selection_call_abi::borrowed_argument` rebuilds the same view/loan tests from `address_join_input`, `established_view_input`, `byte_owner_input` (see file 2).
- `record_input::fields` (`:3609`) and `aggregate_result_input::fields` (`:1206`) both compute per-field offsets and shapes for establishment; one for records (computes alignment offsets itself, `:3660`), one reads the stored `layout` (`:1267-1271`).

### Unused or one-caller code
Command: Grep count over `source-rust` for `owned_parameter_uses_local_home|replay_memory_origins|scalar_storage_origins|activation_storage_input::|native_code_layout_input::valid|zeroed_structural_input::storage|structural_array_input::elements|scalar_array_input::storage`:
`selection.rs` 1, `selection_inputs.rs` 4, `selection_scalar_graph.rs` 4, `selection_structural.rs` 5 (14 occurrences in 4 files). So each of these is referenced from the selection modules, not isolated.
Command: Grep count for `folded_zero|folded_immediate` over `source-rust`: `selection_inputs.rs` 2, `selection_scalar_graph.rs` 6.
Not grepped: `loan_declaration` (`:450`), `readable_byte_type` (`:2865`), `accepts_view_loan` use, `store_destination` in `reference_input` (`:4422`). Suspected single callers; I did not count them.
The module comment (`:1-7`) describes the group as "input projections consumed by instruction selection"; it does not say the group is the single path.

### Stated limits
- `:1298-1299` "A control edge copies the complete descriptor, rather than retaining an address..." (explains why descriptor joins are required for borrowed block parameters).
- `:1523-1524` doc: "The retained placement supplies the hidden result register. The target audit separately checks..." (indirect results are checked elsewhere).
- `:3880-3882` "Owned sequences carry aggregate storage...".
- `:4663-4666` test fixture "captured from the unchanged CLayout".
- `:5795-5796` "The retained result describes its real backing width. It supplies neither an input origin nor new permission."
- `:6374-6375` "The unused binding commitments are fixture data, not an admitted product."
- `:5103-5104` "Equal native footprint cannot manufacture a reference permission from arbitrary source bytes."
- No TODO, FIXME or migration-debt words found by reading.

### Fail-open risks
- `view_access` `:2172-2371`: returns `None` at the end if the budget runs out; callers treat `None` as refusal (`?`). Fail-closed.
- `descriptor_family` (`:3905-3944`) returns `true` for any pair of `ByteSequence::BorrowedView` shapes regardless of their `access` field (`:3934-3941`). A view with one access class can be accepted where another is declared. "open": no comment. What reaches it: a record field declared `BorrowedView` of some access and a child descriptor of a different `BorrowedView` access; the separate `readable` test (`:3814-3820`) uses `view_access` but does not compare against the declared byte carrier's access.
- `storage_origin_input::collect_calls` `_ =>` arm (`:6059-6068`): any unlisted producer kind is accepted as an "origin" (`visited.origin()`), but only if `contains_borrowed_storage` is false, else custody error. "traced" inside the function; the other arm for `StructuralLeafCopy` etc. is explicit.
- `contains_borrowed_storage` (`:6235-6283`): returns `true` (borrowed) when a declaration lookup fails (`:6243-6245`), which makes callers refuse. Fail-closed.
- `access_coordinates` `_ => vec![whole(access.place)]` (`:5638`): for any unlisted operation kind the coordinate is the whole source place. A read through a narrower path is then treated as a read of the whole place. "open": not traced to the memory-origin consumers.
- `replay_memory_origins` (`:5483-5492`): accesses whose role is not one of four read roles are kept unchanged. By design.
- `folded_literal` (`:2951`): returns `None` unless literal is directly adjacent and used once. Fail-closed.
- `owned_store_destination` (`:1619`) compares the result equal to `operation_result`; `None != Some` gives `false`. Fail-closed.
- Else none seen. Many `Option` functions return `None` for unmatched cases; callers use `?`.

---

## 5. selection_interrupts.rs (2,082 lines, read in full)

### Purpose
Emits the instructions for x86-64 static interrupt installation (`InstallStaticInterrupt`) and for the later token operations
(`StaticInterrupt`: `Context`, `FinishCompletion` and the acknowledgement-permit operations). Installation is emitted as a run-time validation of the
caller's interrupt descriptor table, GDT, task-state segment, stack and arrival tables, a refusal region (returns `false`) and, only if every check passes,
the publication of the root record, task register commit and table publication.

### Shape
- `interrupt_installation::emit` (`:45-268`), `control_home` (`:270`), `publish` (`:313`), `address::prepare` (`:33`).
- `interrupt_installation_calls::emit` (`:509-610`) marshals and emits the validator call (`Validate`, scalar Boolean result) and the work call (`Work`, `InvokeNoNormal`).
- `interrupt_installation_guards` (`:613-1793`): `checks::Checks` helper (`:631-880`), `layout::prepare` (`:914`), `pairs::validate` (`:959`), `stacks::{validate_arrivals,validate_stacks}` (`:1106`, `:1160`), `table::{validate_table, validate_code_selector}` (`:1282`, `:1377`), `with_validated` (`:1470-1717`), `view_words`, `bounded_view`, `root_interval`. `GuardEntry` (`:1447`), `ValidatedInstallation` (`:1454`), constants `CONTROL_BYTES=128`, `DESCRIPTOR_OFFSET=48` and counter offsets (`:1437-1445`).
- `static_interrupts::emit` (`:1812-2006`) plus `load`, `store`, `require_equal`.

### Inputs and outputs
Reads the legalized row, `builder.installations` (`BoundInstallations`: scope, entries, stack requirements, anchor), `source.geometry()`. Mutates `Builder` (instructions, definitions, local slots, memory footprints).
Outputs a Boolean refusal register, or reads/writes the root words (`root::STATE/INVOCATION/DEBT/COMPLETION/PROVIDER/CONTEXT`).

### How it decides
- The installation must match the admitted roster by identity: `installation((machine, operation))`, `scope.arguments == arguments` (`:66`), `entries.len() == fatal_entries.len() + 1` (`:76`), application/placement/gate uniqueness (`:143-147`), entry plan equality (`:179`).
- Gate matching uses `gate_byte_offset` (`:162-166`, `:146`) and `fatal.application`, `fatal.machine` (identity values).
- Run-time checks are keyed on byte constants of the x86 descriptor formats: gate attribute `0x8e` (`:1339`), present bit `0x80` (`:1302`), GDT access mask `0xfc == 0x98` (`:1413-1420`), long-mode flags `0x60 == 0x20` (`:1423-1430`), IST ≤ 7 (`:1347`), vector number `gate_byte_offset / 16`, and stack alignment from the requirement.
- `static_interrupts::emit` refuses unless the target architecture is `X86_64` (`:1827`), the operation is not `Install`, and `row.result.is_none()` (`:1829`).

### Cases by shape
- Argument arrays are exactly 6 (`&[StructuralArgument; 6]`, `:916`); the roster is indexed by position: 0 table, 1 GDT, 2 task, 3 stacks, 4 arrivals, 5 context (`:1552`, `:1566-1569`).
- Gate size is 16 bytes (`bytes / 16`, `:1291`), TSS demand `104` bytes (`:1599`, `:414`), descriptor scratch 10 bytes (`:1608`).
- Stack kinds: `Dedicated | Interrupted | ProviderSelected`; `ProviderSelected` is refused in several places (`:1353-1354`, `:1539-1541`). `Interrupted` is accepted only for the `OpaqueFatal` frame with `FatalContext` protocol (`:1515-1538`).
- Frame kinds `NoErrorCode | ErrorCode | OpaqueFatal` (`:1117-1121`). Acknowledgement requires a no-error frame and vector 32 to 255 (`:1122-1129`).
- `interrupt_installation::emit` accepts entries as exactly one acknowledgement application plus fatal entries (`:152-178`).

### Same job elsewhere
- See file 1 on counter loops, compare helpers, and the `Boundary` local slot allocation.
- `Checks::load` (`:763`) and `static_interrupts::load` (`:2009`) both emit a width-selected load with a footprint `memory(..)`; the former uses `load_transfer`, the latter hard-codes `Load64`.
- The arrival/stack/pair validators repeat the "iterate, count matches, require exactly one" pattern (`:1130-1156`, `:1185-1277`, `:1035-1098`).

### Unused or one-caller code
`interrupt_installation::emit`, `interrupt_installation_calls`, `interrupt_installation_guards`, `static_interrupts` are re-exported at `selection_structural.rs:938-940`, `:1704`. I did not grep their callers. `with_validated` is called once from `interrupt_installation::emit` (`:240`; read). `Checks::reset/save/increment/aligned` have uses inside this file.

### Stated limits
None stated by comment. Hard-coded: x86-64 only (`:1827`), six arguments, 104-byte TSS demand.

### Fail-open risks
- `:152-178` accepts any entry whose `application` equals `anchor.application` as the acknowledgement entry and, only for non-ack entries, requires a unique fatal match. Traced: roster size equals `fatal_entries.len()+1` (`:76`) and duplicates are refused (`:143-147`).
- `table::validate_table` first loop (`:1292-1330`) allows a present gate only at the admitted `gate_byte_offset / 16` slots; others must be absent. Fail-closed.
- `view_words`/`bounded_view` stride 0 returns an error (`:1737`).
- `root_interval` returns `Ok(None)` when no entry has an `arrival` demand (`:1758-1760`), so the root-interval disjointness test is skipped. Traced: those entries are dedicated-stack entries; `validate_stacks` then runs without a root interval (`:1221`). "open" whether a dedicated stack overlapping the retained root storage is possible with no `arrival` demand.
- `static_interrupts::emit` `:1859-1863`: the token check demands `readable` and exact byte size and alignment. Fail-closed.
- None else seen.

---

## Coverage
- Read in full (line counts from the unit file):
  - `selection_access_lifecycles.rs` lines 1-6968 (6,968)
  - `selection_call_abi.rs` lines 1-2326 (2,326)
  - `selection_edge_transfers.rs` lines 1-1662 (1,662)
  - `selection_inputs.rs` lines 1-6995 (6,995)
  - `selection_interrupts.rs` lines 1-2082 (2,082)
  Total 20,033 lines read in full (the unit TSV lists 6968, 2326, 1662, 6995, 2082).
  Files report 1 line more than listed in a few places (final line), counts not re-verified with `wc`.
- Skimmed: none. Test regions were read at normal speed but I summarised them in a sentence each, without verifying each assertion: `selection_access_lifecycles.rs:2296-3924`, `:4654-6020`; `selection_inputs.rs:4548-5208`, `:6337-6662`; `selection_edge_transfers.rs:1065-1293`; `selection_call_abi.rs:768-926`.
- Not read: none.

## Five things a maintainer most needs to know about this unit
1. Resource-profile checking is emitted as straight-line native code over a profile record read from memory at run time (`selection_access_lifecycles.rs:4246-4361`, `:1735-1943`). The layout is fixed: 32 regions, 16 external rules, 16 atomic rules with 12 operation flags, 32 reach services (`:2240-2274`), and the scratch offsets (`:919`, `:1146-1165`, `:3933-3939`) are hand-assigned. Adding a field means editing both the layout derivation and these offsets.
2. A malformed profile traps (`OperationTrap`); only a well-formed but incompatible profile produces a `Rejected` result (`:162`, test `:2761`).
3. Selection re-validates, but does not compute, the call-plan placements stored by stage 04: `validate` (`selection_call_abi.rs:1895`) and its helpers compare roughly 40 fields per argument against the target record, with `None`/`invalid()` on any difference. The same checks appear in stage 04 legalization (grep hits in `04_*/legalization*.rs`, not read).
4. `selection_inputs.rs` finds "what is this place" with at least seven separate lookup functions that each try parameters, block parameters, operation results, locals and literals in a different order (`:90`, `:347`, `:1664`, `:1749`, `:2596`, `:3223`, `:4324`). A new place kind has to be added to each.
5. The static-interrupt installer is x86-64 only and emits hundreds of lines of descriptor checks on hard-coded byte layouts (`selection_interrupts.rs:1282-1431`, `:1827`); it is the only code in this unit with a hard-coded 6-argument roster.

## What you are unsure about
- Whether the field-name join in `transfer_image` (`selection_access_lifecycles.rs:6538`) compares `String`s in production (I only saw the string in the test fixture at `:3586`; the type definition is in `layout_plans`, not read).
- The correct classification of `descriptor_family` (`selection_inputs.rs:3934-3941`) as fail-open: I did not trace whether `view_access` or an upstream stage already pins the byte-view access class.
- `access_coordinates` `_` arm (`selection_inputs.rs:5638`): whether other operation kinds can reach it with a path narrower than the whole place.
- Call counts for the `pub(crate)` modules in `selection_interrupts.rs` and for several helpers in `selection_inputs.rs` (listed as not grepped above).
- Whether `integer_carrier_normalization` is ever called with a float or 128-bit type (`selection_call_abi.rs:2242`).
