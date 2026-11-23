# Record: unit_022_gap_010 (area 11-native-stages-11-15), image-emission `function_fragments.rs` lines 13801-19800

Second-pass record for the ranges the first reader of unit 022 did not read. Unit file: `units/gap_010.tsv`
(one row: `function_fragments.rs` 13801-19800, 6000 lines). Paths are under
`source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/`.
The first-pass record (`records/unit_022.md`) covers lines 1-7800 and the three small files; the module map
below was built from `grep -n "mod "` over the whole file (not only my range).

## Where the range sits

All of `function_fragments.rs` is one file of inline modules. My range cuts across these (line numbers from
grep, read for the parts inside the range):

| Lines | What | In my range |
| --- | --- | --- |
| 12969-16798 `source::structural_fields` | retained-operation checks for structural field reads/stores, byte-sequence stores/reads | tail: 13801-16798 (test section 13801-15427, production code 15429-16798) |
| 16800-17028 `source::structural_homes` | resolves which stack slot or formal parameter holds a structural place | all |
| 17030-17890 `source` module-level helpers | lookups into the emission facts, "retained" checks for boundary/byte/element/float ops, temporary-stack replay | all |
| 17892-18437 `source::tests` (+ `temporary_stack` submodule 18226-18436) | unit tests for the above | all |
| 18440-18442 | `pub(crate) use source::selected_constant_operand;` and `#[path]` declaration of `internal_calls` | all |
| 18443-19148 `structural::{byte_input, byte_output}` | build and re-check the boundary settlement records for hosted byte read and byte write | all |
| 19150 | re-export `read_result_cleanup_actions_match` | all |
| 19151-19800 `structural::primitive_locals` (`access`, `establishment`, `frame_location`, `tests`) | primitive-local operation retention | start: 19151-19800 (module continues to ~20046) |

Within `source`, what "retained" means in this file: each check takes an abstract operation (from the
optimized program's `AbstractFunction`) and asks whether the target-level graph (`TargetFunction`) and, for
footprint checks, the selected-instruction memory accesses still contain exactly the one row that realizes it.
They are called from `function_fragments/source/retained_source_validation.rs` (not in my range). Counts of
those callers are given per function below.

## 1. `source::structural_fields` tail (13801-16798)

### Purpose
Lines 13801-15427 are the end of the `tests` submodule (`structural_fields::tests`, opened at 12978). Lines
15429-16798 are the production checks: for each structural field operation in the abstract function they
decide whether the target graph and the instruction footprints still carry that operation with the same
place, path, access, field, value, length and obligation. A failing check returns `false`; the caller
(`retained_source_validation.rs`) treats that as "the object does not match the source".

### Tests (13801-15427; read)
27 `#[test]` attributes fall inside my whole range (grep: `awk ... /#\[test\]/` over 13801-19800); about 13 are
in this block. Each builds a fixture with `field_fixture` (`:13123`, outside range) and then mutates one field
of the target row at a time, asserting `retained(..)` turns false. Representative names:
`returned_reference_byte_store_retains_exact_pointer_home_and_candidates` (`:13896`),
`indexed_byte_publication_retains_every_operand_and_writable_access` (`:14548`),
`nested_field_membership_reconstructs_parent_local_carrier_identity` (`:14679`),
`indexed_field_membership_reconstructs_fixed_array_carrier_identity` (`:14752`),
`byte_field_read_replays_reference_and_descriptor_loads` (`:14883`),
`owned_parameter_field_membership_keeps_exact_value_access_and_graph_replay` (`:15035`),
`observed_owned_parameter_discard_keeps_exact_scalar_exit_accounting` (`:15112`),
`structural_field_membership_requires_one_matching_read_across_all_blocks` (`:15234`),
`whole_byte_replacement_rejoins_one_qualified_local_producer` (`:15329`). The tests hard-code small constants
(`TerminalIdentityIndex(6)`, `capacity: 16`, `IntegerType::new(Unsigned, 64)`); they do not iterate over a
program corpus.

### Production functions (15429-16798; all `pub(super)` unless noted)
- `retained_selectors(path, indices)` (`:15429`): the runtime-index selectors in a path, in order, must equal
  the target row's `indices` operands by `source_value()`. Callers: 4 in-file `function_fragments.rs:12217,
  12245, 12275, 12512` plus 4 in this module (`:15727, 16022, 16333, 16603`).
- `retained_field_root(function, target, place)` (`:15441`, private): root place must be either an arrival
  parameter (target parameter row at the same position equal in place, type, access, multiplicity and
  projected qualifications), a block parameter retained in the target block, or the result of exactly one
  producing operation whose structural result home (or element view) is retained exactly once. 5 call sites,
  all in this file (`:15722, 16000, 16330, 16591`).
- `indexed_store_retained` (`:15502`): `StructuralByteSequenceFieldByteStore` must map to exactly one target
  row with identical destination (place, access from `read_access`, path), field, length, obligation, and
  index/value operands that are `u64` and `u8`. Access must be `MutableBorrow` or `WriteOnlyBorrow`
  (`:15525-15531`). Call site outside range: `retained_source_validation.rs:567`.
- `runtime_footprint_retained` (`:15567`): for `PrimitiveScalarRead`, `StructuralByteSequenceFieldLength`,
  `WriteOnlyPrimitiveStore`, `StructuralScalarFieldStore`, pairs the runtime indices of the operation with
  `ReadIndexedPrimitive` / `WriteIndexedPrimitive` / `WriteIndexedStructural` access rows keyed by
  `Origin::Operation(psi_operation)`. Call site: `retained_source_validation.rs:609` (1).
- `indexed_store_footprint_retained` (`:15648`): exactly one access row for the operation, with role
  `WriteByteSequence` and matching index/value/length/obligation, `byte_count == 1`. 1 production call
  (`retained_source_validation.rs:568`) and 1 test call.
- `indexed_read_retained` (`:15686`): like the store check for `StructuralByteSequenceFieldRead`; also
  requires `retained_source.access != WriteOnlyBorrow` (`:15726`) and index of type `u64`. 1 call
  (`retained_source_validation.rs:571`).
- `field_root_type`, `loads_backing_pointer`, `projected_field_rows`, `projected_selector_rows`,
  `pointer_read_before`, `owned_observation_storage`, `parameter_access`, `read_access`, `write_destination`
  (`:15737, 15753, 15917, 15930, 16609, 16628, 16637, 16664, 16420`, private): helpers. `read_access`
  (`:16664`) is the central one: it returns the unique `(access, type)` for a place by scanning function
  parameters, non-entry block parameters, element-view results and other structural results, returning `None`
  if the place is declared twice or its target row differs.
- `indexed_read_footprint_retained` (`:15773`): walks the access rows of one operation with a cursor in order:
  optional root-view backing load, one row group per pointer slot (from
  `legalized_operations::structural_inputs::structural_reference_input::projected_byte_field` geometry), an
  optional descriptor load, then either returned-view rows or the single payload read at offset
  `descriptor_offset` or `length_offset + 8` (`:15904-15909`). 1 call (`retained_source_validation.rs:572`) plus
  1 test.
- `view_replacement_retained` (`:15984`) and `view_replacement_footprints_retained` (`:16027`): for
  `StructuralByteViewFieldStore`: destination access must be `Owned`, `MutableBorrow` or `WriteOnlyBorrow`
  (`:16021`); footprint is exactly `2 + 2*count` rows: pointer read at offset 0, extent read at offset 8,
  pointer writes then extent writes, with strictly increasing instruction ids (`:16064-16085`). Each has 1
  call (`retained_source_validation.rs:581-582`).
- `replacement_retained` (`:16088`) / `replacement_footprints_retained` (`:16175`): for
  `ByteSequenceStore` and `StructuralByteSequenceFieldStore`. Destination access `Owned`, `MutableBorrow` or
  `WriteOnlyBorrow`; source place must differ from destination. The footprint check expects one
  `ReadByteSpan`, one `WriteByteSpan` and one `WritePlace` row, with the source read at byte offset 8 when the
  source is a bounded-owned byte sequence produced by concatenation, a Machine call result, or an arrival
  parameter, else 0 (`:16248-16280`). Callers: 6 and 11 mentions respectively including tests; production
  call `retained_source_validation.rs:590-591`.
- `retained` (`:16304`): `StructuralByteSequenceFieldLength` and `StructuralScalarFieldRead`. For a scalar
  read, if the root is a `Reference` type the carrier access must be `Owned` and the reference access
  `SharedBorrow`/`MutableBorrow` (`:16362-16370`); the expected source path is computed with
  `terminal_semantics::canonical_structural_storage_path` (`:16381`); the target must hold exactly one read
  row equal to `(result, expected_source, case, field)`. Calls: `retained_source_validation.rs:605, 608`.
- `scalar_store_retained` (`:16574`): `StructuralScalarFieldStore`; exactly one target store with equal
  destination, path, field, value (id and type) and selectors. 1 call (`retained_source_validation.rs:829`).
- `enum AccessPurpose { Field, ByteLength, ByteOwnerReplacement }` (`:16414`): selects which access rules
  `parameter_access` and `read_access` apply.

### How it decides (keyed on)
Every check is keyed on identity: `psi_operation` ids, `PlaceId`, `StructuralFieldId`, `ObligationId`,
`ValueId`, `StructuralTypeId`, and structural path segments (`:16020-16024` etc.). It never compares display
names or strings. Access is judged by enum value (`StructuralAccess`), not by type name. Shapes are matched by
`StructuralTypeShape` variant (`Reference`, `ElementView`, `BoundedOwned` byte sequence).

### Cases by shape
- `write_destination` (`:16420-16572`) handles exactly two shapes: a path whose first segment is not
  `Referent` with a directly readable place, or a path starting with `Referent` whose carrier is the result of
  exactly one Machine call (not Dynamic, `:16461`) returning an `Affine` `MutableBorrow` reference to a record
  with a single leaf origin set (`let [leaf] = ...`, `:16530`). Everything else returns `None`.
- `indexed_read_footprint_retained` expects the shapes in the comment at `:15820`: "Selection may already hold
  a root view's backing in a register." and `:15846`.
- `loads_backing_pointer` (`:16753`): path starts with `Referent` or the root is an `ElementView` type.

### Same job elsewhere
- The `retained*` functions here and the `retained*` functions in `source::control_flow`, `source::primitive_accesses`
  and `structural::primitive_locals::access` do the same job for other operation families; all are called from
  one function, `retained_source_validation.rs` (grepped: lines 561-830 above). Read: `primitive_locals::access`
  at `:19160`.
- Duplicate in stage 09: see section 5 (`frame_location`, `argument_matches_source`).

### Unused or one-caller code
Every `pub(super)` function in 15429-16798 has exactly one production call site, in
`function_fragments/source/retained_source_validation.rs` (grep `structural_fields::` over
`function_fragments*`; lines 567, 568, 571, 572, 581, 582, 590, 591, 605, 608, 609, 829). `retained_selectors`
has 4 more callers in `function_fragments.rs:12217,12245,12275,12512`. Counts from
`grep -rnw --include=*.rs <name> source-rust tools`.

### Stated limits
Only two comments in the range: `:15820-15821` and `:15846-15847` (quoted above). No TODO/unsupported/migration
markers in 13801-19800 (`awk` with `unsupported|TODO|debt|not yet|remains|migrat|FIXME` printed nothing).

### Fail-open risks
- `runtime_footprint_retained` `:15612-15614`: `if elements.is_empty() { return true; }` returns true for a
  read/store with no runtime indices without looking at any access row. A wrong program in which the selected
  instructions carry extra or wrong rows for such an operation is not caught here. Classification: open (no
  comment; the only caller `retained_source_validation.rs:608-609` also runs `retained` first, which I did not
  read).
- `indexed_read_footprint_retained` `:15883-15900`: if the remaining rows are all `ReadReturnedView` rows with
  zero offset and count and a matching `Call` producer exists, it returns `true` without checking the payload
  read, index, length or obligation. Wrong program: a returned-view read whose payload load was dropped or
  changed. Classification: open (no comment at the site).
- `replacement_footprints_retained` `:16248-16280`: `source_byte_offset` is 0 when the source place is not
  recognized as bounded-owned; the check then expects offset 0. Accepts what the rows say; not an accept-on-
  unknown. Classification: none seen.
- `read_access` `:16664`: unknown place returns `None`, callers return `false`. Fail-closed.

## 2. `source::structural_homes` (16800-17028)

### Purpose
Finds, for a structural place, its root type and where its storage lives: an allocated stack slot, an
indirect pointer slot for borrowed block parameters, or the caller's formal parameter.

### Shape
- `enum SourceHome { Allocation { slot: LocalStorageSlotId, indirect: bool }, BorrowedFormal(PlaceId) }`
  (`:16805`).
- `struct HomeInputs<'a> { target, roots: Vec<(PlaceId, Option<LocalStorageSlotId>, StructuralTypeId)>,
  allocations: Vec<(PlaceId, &SelectedLocalStorageSlot)> }` (`:16813`), `HomeInputs::new` (`:16820`),
  `HomeInputs::resolve` (`:16872`), both sorted by place and searched with `partition_point` (`:16876-16883`).
- `resolve(target, selected, place)` (`:16890`): the same lookup without a prebuilt index, by linear scans.
- `resolve_candidates` (`:16947`), `thin_address_join` (`:16991`).

### Inputs and outputs
Reads `TargetFunction` (operation result homes, parameters, block parameters) and `SelectedFunction`
(`local_storage_slots`). Returns `Result<(StructuralTypeId, SourceHome), &'static str>`; the error strings are
the diagnostics, e.g. "source place has multiple declarations or result homes" (`:16957`).

### How it decides
Keyed on `PlaceId` and `LocalStorageSlotId`. Exactly one root declaration, at most one allocation; an
allocation must equal the producer's slot id (`:16964`); with no allocation the place must be a non-owned
target parameter (`:16983`). `thin_address_join` returns `Some(false)` for any slot that is not a
`StructuralBlockParameter` (`:16995-16997`) and for the entry block or non-address-join parameters
(`:17008-17015`); otherwise `Some(true)` unless the parameter is a primitive scalar with a storage codec
(`:17020-17026`). Indirect slots must be 8 bytes with 8 alignment (`:16969`).

### Same job elsewhere
`HomeInputs::resolve` and free `resolve` duplicate each other's lookup logic (both feed `resolve_candidates`);
the free `resolve` is used at `function_fragments.rs:4339, 10309, 10461`, `HomeInputs` at `:10994, 11002,
11397`. Counts from grep. `thin_address_join` is also called at `:11453` (outside range).

### Fail-open risks
None seen. `resolve_candidates` errors on missing or duplicate declarations.

## 3. `source` module-level helpers (17030-17890)

All `pub(super)` functions here are used by `validation.rs`, `retained_source_validation.rs`,
`function_fragments.rs:2769-2960` (`production::...`) or the `structural` module.

- `fragments`, `function_facts` (`:17039`, unique function per machine, else `Error::Mismatch("shared
  function has no unique current source")`), `callable_invocations` (`:17060`), `code_catalogs` (`:17074`),
  `publication_function` (`:17085`), `function_abi` (`:17099`), `frame` (`:17113`, returns `Ok(None)` if no
  row, error if two): thin accessors into `ObjectEmissionInput::emission_facts()`. Callers: `function_facts` 5 uses in
  `validation.rs:495,557` and `function_fragments.rs:2804`; `frame` 6 uses (`validation.rs:631`,
  `function_fragments.rs:2809, 18560, 18637, 18989, 19068`).
- `byte_concatenation_retained` (`:17127`), `ieee_float_builtin_retained` (`:17149`), `program_image_retained`
  (`:17191`), `ieee_literal_retained` (`:17208`), `fixed_byte_length_is_static` (`:17231`),
  `byte_operation_retained` (`:17268`), `element_operation_retained` (`:17359`), `empty_run_release_retained`
  (`:17700`), `access_lifecycle_retained` (`:17749`), `placed_scalar_retained` (`:17791`): "one matching target
  row" checks, each called from `retained_source_validation.rs` (lines 10-14 import, 707-765 use; grepped).
- `parameter_abi` (`:17471`): returns the call plan and parameter lists only when
  `legalized_operations::function_abi::publishes_parameter_abi(...)` says the target publishes them. Callers:
  `validation.rs:410`, `retained_source_validation.rs:475-739`, `function_fragments.rs:2860` context.
- `fragment_metadata` (`:17492`): returns the placed fragment's attachment type and provenance, else
  `Mismatch("missing placed fragment")`. Called from `function_fragments.rs:2803`, `validation.rs:562`.
- Temporary stack: `temporary_stack_index` (`:17509`, binary search by machine),
  `ordered_temporary_stack_rows` (`:17517`, sorts and rejects duplicate machines),
  `temporary_stack_bytes_by_function` (`:17531`, clone of `emission_facts().physical.temporary_stack`),
  `decoded_temporary_stack_bytes_by_function` (`:17540`, rebuilds the peaks from
  `readback().instruction_effects` against the CPU roster), `replayed_temporary_stack_bytes_by_function`
  (`:17585`, from `replay.original_encoding()`), `temporary_stack_bytes_for_selected_functions` (`:17613`),
  `temporary_stack_peak` (`:17658`, maximum `TemporaryBytesV1 { byte_count }` else 0). So the temporary stack
  size is computed three ways (published, decoded, replayed) and compared at `validation.rs:396-397, 541-542`.
- `call_target_matches` (`:17673`): Machine equals Machine, Dynamic equals Dynamic on `(selector, interface,
  slot)`; anything else false. Callers `function_fragments.rs:7960`, `:16518`.

### How it decides
Almost all keyed on `psi_operation` identity with `.count() == 1` or a `next()`/`next().is_none()` uniqueness
test (e.g. `:17197-17205`, `:17276-17356`). `byte_operation_retained` (`:17268`) and the non-float arms of
`element_operation_retained` (`:17359`) match only the variant and `psi_operation`; they do not compare
operands or results. (`ElementViewRead` float arm at `:17380-17403` compares result home, type and format.)
`temporary_stack_peak` is keyed on the enum variant `MachineEncodedStackEffect::TemporaryBytesV1`.

### Cases by shape
- `fixed_byte_length_is_static` `:17236`: only `ByteSequenceLength`.
- `empty_run_release_retained` `:17700`: only `BoundaryCall` with `AbstractBoundaryResult::Structural`, no
  scalar arguments, exactly one structural argument with `Owned` access, and a matching `StructuralLeafCopy`
  row (`:17729-17746`).
- `access_lifecycle_retained` `:17749`: only `BoundaryCall` with structural result and no scalar arguments.
- `placed_scalar_retained` `:17791`: only boundary calls realized as `AccessorScalarAccess`; offset is fixed 0
  (`:17853`); width is `transfer_width_bits / 8` that must fit `u8` (`:17846`).
- `ieee_float_builtin_retained` `:17149`: `NearestIeeeFloatArithmetic` and `IeeeFloatClassify` target rows
  only; selected side only checks `NearestIeeeFloatArithmetic` (`:17181-17186`). An `IeeeFloatClassify` target
  row is matched against the arithmetic instruction kind built from `realization.operation`; I did not read
  the types to see whether that is intended (see "unsure").

### Same job elsewhere
- The three temporary-stack computations (above) share `temporary_stack_peak` (`:17658`).
- `parameter_abi` and `abi.parameter_abi()` (called at `validation.rs:587`) both exist; I did not read the
  second.

### Unused or one-caller code
`decoded_temporary_stack_bytes_by_function` 1 call (`validation.rs:541`); `replayed_temporary_stack_bytes_by_function`
1 call (`validation.rs:396`); `fragment_metadata` 2 calls; `fixed_byte_length_is_static`, `empty_run_release_retained`,
`access_lifecycle_retained`, `placed_scalar_retained`, `ieee_float_builtin_retained`, `program_image_retained`,
`byte_operation_retained`: each 1 production call in `retained_source_validation.rs` (grep list above).
`#[cfg(test)] temporary_stack_bytes_for_function` (`:17600`) is test-only. `pub(super) use ...
requires_graph_storage_replay` at `:17031` is `#[cfg(test)]`; the non-test callers use a path from
`abstract_operations` (28 grep hits for the name in total).

### Fail-open risks
- `decoded_temporary_stack_bytes_by_function` `:17572-17575`: a row whose `effects()` is `None` contributes
  nothing to the peak (stays 0). Wrong program: an encoded instruction with no recorded effects whose real
  temporary stack use is nonzero would read as 0 here. The result is compared with the published value at
  `validation.rs:541-542` (not read), so a mismatch there would still be caught. Classification: open.
- `temporary_stack_peak` `:17669`: `.unwrap_or(0)` for no temporary-stack effects. Intended: empty maximum.
- `byte_operation_retained` / `element_operation_retained`: the check passes if exactly one target row has
  the same variant and `psi_operation`, regardless of operands. Wrong program: a target row for the same
  operation id with different operands (offset, source, length) is accepted by this function. Classification:
  open (whether operands are checked by another function was not traced; `retained_source_validation.rs:707-720`
  is the caller).

## 4. `source::tests` (17892-18437) read in full

Tests for `element_operation_retained` (`:17898`), `requires_graph_storage_replay` (`:17975, 18000`),
`ieee_literal_retained` (`:18081`), `byte_concatenation_retained` (`:18150`), and the `temporary_stack`
submodule (`:18226-18436`, five tests with a one-function `SelectedFunction` fixture). The temporary-stack
tests assert reverse function order is preserved by identity (`:18367`), duplicate / missing / extra roster
rows are errors (`:18372`).

## 5. `structural::byte_input`, `byte_output` (18443-19148)

### Purpose
Build (`settlement`) and re-check (`validate`) the `ObjectBoundarySettlement` record for two hosted byte
boundaries: reading one byte (`HostedReadByte`) and writing one byte from an `i32` (`HostedWriteByteI32`).
Each record names the instruction span in the placed function, the frame slot used as scratch or result home,
and the boundary kind.

### Shape
- `byte_input` (`:18444`): `decoded_home(target, bytes) -> Option<u32>` (`:18458`) disassembles one x86_64 or
  aarch64 instruction through `target_x86_64::inspection::decode_x86_64_selected_hosted_read_byte` /
  `target_aarch64::inspection::decode_aarch64_selected_hosted_read_byte` to recover the frame offset;
  `cleanup_actions_match(function, selected, actions)` (`:18476`); `settlement` (`:18535`); `validate`
  (`:18613`); `result_declaration` (`:18697`); tests (`:18710-18950`).
- `byte_output` (`:18953`): `settlement` (`:18965`), `validate` (`:19047`), `current_settlement` (`:19121`).

### Inputs and outputs
`settlement` reads the `ObjectEmissionInput` (fragment bytes, text section, frame layout, publication
function), the located physical settlement, and builds a `BoundarySettlementRecord` plus
`placed_text_offset(...)`. `validate` recomputes the same facts and compares the proposed record field by
field (`:18648-18691`, `:19089-19112`).

### How it decides (keyed on)
Instruction anchor (`located.anchor`), operation id, boundary id, frame slot id
(`LocalStorageSlotId::Structural{operation, place}` or `::Boundary{operation}`), byte offsets and sizes,
and decoded machine bytes. Not strings. The one place that matches by variant of an execution binding:
`byte_output::current_settlement` `:19134-19142`: accepted when the execution is `AdmittedProvider(_)` or
`CompilerBuiltin(HostedWriteByteI32)`, otherwise `Error::Mismatch`.

### Cases by shape
- `decoded_home` returns `None` unless `HostedReadByteRealization::supports_target(target)` (`:18459`), then
  matches `Architecture::X86_64` / `Aarch64` only.
- The read-byte home must be a sum layout; the record hard-codes empty `scalar_arguments`, `arguments`,
  `byte_sequence_arguments`, receipts and custody (`:18581-18587`) and `validate` demands they stay empty
  (`:18684-18690`).
- The write-byte record has exactly one `runtime_scalar_arguments` entry, signed 32-bit
  (`:19099`), scratch slot of size 1, alignment 1 (`:19102-19103`).
- `cleanup_actions_match` `:18476`: refuses unless the selected function has a structural contract, the
  function result is `Unit`, and actions are non-empty `DiscardRoot` rows with each place appearing once and
  matching exactly one `BoundaryCall` with `Affine` result, no claims or qualifications, exactly one
  `HostedReadByte` settlement, exactly one `OperationResult` place declaration, and a result type accepted by
  `hosted_read_byte_declaration_is_valid` (`:18531`).

### Same job elsewhere
`byte_input::settlement` and `byte_output::settlement` are the same pattern (find span, frame slot, decode home
or register, build record); `validate` in each recomputes by repeating the checks. These are the two arms of
the dispatch at `function_fragments.rs:20390-20392` and `:20953-20955` (outside range). A `process_exit` module
(`:20047`) is the third arm; not read.

### Unused or one-caller code
`read_result_cleanup_actions_match` (re-export `:19150`): 1 call site, `retained_source_validation.rs:814`
(grep). `cleanup_actions_match` itself: 1 production use through that re-export plus 5 test uses.

### Stated limits
Comment `:19118-19120`: "Only an admitted provider or the compiler's built-in host byte output may write a byte."

### Fail-open risks
- `byte_output::validate` `:19107`: delegates exactness of the code bytes to
  `replay::boundary::runtime_scalar_custody::selected_byte_output_bytes_are_exact` (not read). Classification:
  open (not read).
- Otherwise none seen: every unknown (`None` from `ok_or_else`) returns `Err(invalid())`.

## 6. `structural::primitive_locals` first 650 lines (19151-19800)

### Purpose
Check that an abstract `EstablishPrimitiveLocal` or `PrimitiveLocalStore` is realized by the expected frame
address and store instructions in the selected function.

### Shape
- `access::operation_retained` (`:19160`, re-exported at `:20046` as `primitive_operation_retained`): for
  `EstablishPrimitiveLocal` it returns whether `establishment(...)` yields the same operation id; for
  `PrimitiveLocalStore` it requires an established producer with the same scalar type, exactly one memory
  access with role `WritePlace`, offset 0, count equal to the primitive size, and exactly one instruction
  `Store { byte_offset: 0, byte_size }` whose provenance ends with the operation and contains the value
  (`:19200-19239`). 1 production call, `retained_source_validation.rs:650`.
- `establishment::producer` (`:19252`), `primitive_size` (`:19277`: 8, 16, 32, 64-bit integers 1..8 bytes,
  Boolean 1, Callable 8, Binary32 4, Binary64 8; any other width `None`), `establishment` (`:19294`): requires
  `Unrestricted` multiplicity, no projected qualifications or establishments or claims, one matching place
  declaration, one matching `PrimitiveScalar` type, one local slot of the exact size, two memory accesses
  (`AddressLocal`, `WritePlace`), and a `FrameAddress` then `Store` instruction pair with the same virtual
  pointer register and a value register whose origin has the same `source_value` (`:19366-19410`).
- `frame_location` (`:19418`) and `argument_matches_source` (`:19454`): inside a `#[cfg(test)] mod
  frame_location`.
- `tests` (`:19498-`): `primitive_size_requires_a_known_supported_width` (`:19525`), fixture (`:19552`),
  `initialized_callable_local_retains_the_exact_contract_type` (`:19742`), then
  `initialized_local_requires_exact_producer_address_and_executed_store` (`:19780`, 13 mutations; mutation 7
  starts at `:19797`, the range ends inside it).

### Same job elsewhere (found by grep)
`frame_location` and `argument_matches_source` here (`:19427-19492`, test-only) are line-for-line the same
code as `09_post-allocation-machine-to-framed-machine/src/internal_call_plans.rs:1167-1230` (`frame_location`,
`argument_matches_source`; read). The stage-09 copy has 1 production use each (`internal_call_plans.rs:1128,
1159`). The copy in this file has 0 production uses and test uses only (`function_fragments.rs:19851-19985`).

### Fail-open risks
`operation_retained` `:19198`: operations other than the two listed return `false`. Fail-closed. None seen.

## Coverage

Read in full: `function_fragments.rs` lines 13801-19800 (6000 lines), in six `Read` calls
(13801-14800, 14801-15800, 15801-16800, 16801-17800, 17801-18800, 18801-19800). Skimmed: none. Not read: none
of the unit's range. Outside my range I read only `internal_call_plans.rs:1164-1228` (to confirm the
duplicate) and grep output (call sites in `retained_source_validation.rs`, `validation.rs`). The 27
`#[test]` count is `awk` over 13801-19800; the whole-file count is 76.

## Five things a maintainer most needs to know about this unit

1. Nearly everything in 15429-17890 is a yes/no "retained" check called from one function in
   `function_fragments/source/retained_source_validation.rs`; each demands exactly one matching target row for
   a given `psi_operation` and returns `false` on zero or two (e.g. `:15537-15564`, `:17197-17205`).
2. The structural field checks compare full row identity (place, path, access, field, value, obligation) and
   several also compare the selected-instruction memory-access footprint cursor by cursor
   (`:15814-15915`, `:16046-16086`, `:16217-16302`). Changing the selected-instruction shape for any of these
   stores/reads requires changing the footprint expectation here.
3. `byte_operation_retained` and the non-float `element_operation_retained` arms match on variant and
   `psi_operation` only (`:17278-17356`, `:17368-17468`), so they do not check operands.
4. `runtime_footprint_retained` returns `true` for index-free operations (`:15612`), and
   `indexed_read_footprint_retained` returns `true` for an all-`ReadReturnedView` tail (`:15883-15900`) without
   inspecting the payload read.
5. `frame_location` / `argument_matches_source` in `primitive_locals::frame_location` (`:19418-19492`) are test-only
   copies of the production functions in stage 09 `internal_call_plans.rs:1167-1230`.

## What you are unsure about

- Whether `IeeeFloatClassify` target rows are intended to be matched against a
  `NearestIeeeFloatArithmetic` selected instruction (`:17155-17188`); I did not read `target_operations` or
  `selected_instructions` to see what `realization.operation` holds for classify.
- Whether `retained_source_validation.rs` supplies operand checks for the byte and element operation arms
  (`:17268`, `:17359`) elsewhere; I grepped call lines only.
- Whether the `temporary_stack` decoded path's `row.effects() == None` case (`:17572`) can occur for a real
  nonzero temporary-stack instruction.
- The end of `primitive_locals::tests`, `process_exit`, `validation` and the rest after line 19800 are outside
  this unit.
