# Record: unit_022 gap_009 (area 11-native-stages-11-15), image-emission `function_fragments.rs` lines 7801-13800

File: `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/function_fragments.rs`
(21313 lines; I read only 7801-13800, 6000 lines, all in full). Paths below are relative to that `src/` directory
unless they start with `function_fragments/`. Every claim was made by reading these lines; counts come from
the grep commands named inline.

All of this range sits inside `mod source` (opened at `:3070`, read by the first reader). It is the code that
re-checks ("replays") the stage-14 published object against the earlier, more abstract forms: for each abstract
(Terminal Psi) operation it asks "does the selected/target program still contain exactly this operation?". The
entry points that call into it are in `function_fragments/source/retained_source_validation.rs` (943 lines, not in my unit).

## Module map for the range

| Lines | Module | Role |
|---|---|---|
| `7801-7997` | tail of `aggregate_results::operation` (module opens `:7660`; fns `instruction_operation :7670`, `header :7698`, `operation :7737`) | last arms of a `match` on `AbstractOperation` |
| `7999-8788` | `control_flow` | terminator and cleanup-call matching, plus 2 big test fns |
| `8790-12183` | `placed_reference` (`pub(super)`) | address-chain replay for reference formation, case dispatch and borrowed pointers; contains `backing_geometry` |
| `12185-12281` | `primitive_accesses` | `retained` for 3 primitive-access operations |
| `12283` | `use installation_admission::selected::spill_values;` | re-export alias only |
| `12285-12443` | `stack_calls` | `stack_calls` builds call edges for stack-depth accounting |
| `12445-12967` | `structural_case` | structural case dispatch matching |
| `12969-13800+` | `structural_fields` | opens at `12969`, only `use`s and the test module `12977-...` fall in my range; the production fns (`read_access :16664`, `retained :16304`, `retained_selectors :15429`) are past 13800 |

---

## 1. `aggregate_results` tail, lines 7801-7997

**Purpose.** Final arms of a function that decides, for one `AbstractOperation`, whether the target program
retained it. The function head is before my range (`operation` at `:7737`); I read only the arms from `:7801`.

**Shape.** One `match` with an arm per abstract operation kind. Most arms do `retained.is_some_and(|row|
matches!(row, TargetUnitOperation::X { psi_operation: retained, result_home, .. } if retained == psi_operation
&& result_home.operation_result() == Some((*psi_operation, result)) && fields == retained_fields))`
(for example `EstablishRecord :7840`, `EstablishScalarArray :7929`, `EstablishCase :7935`, `EstablishReference :7941`,
`ReleaseReference :7947`). `retained` is the target row already looked up by the caller (not read).

**How it decides.** Keyed on identity: `psi_operation` equality plus the result's `(operation, result)` pair from
`result_home.operation_result()`, and field-by-field equality of payload (`:7843`, `:7932`). Not on spelling.
`CallStructural :7953-7964` counts selected call rows that match owner `CallSiteOwner::Operation(op)`, callee via
`super::call_target_matches`, structural result, `result_placement == call_plan.result` and crash continuations;
requires `count() == 1`. `ReturnStructural :7965-7992` requires an empty `trivial_affine_locals`, an exact
target terminator, cleanup list equality plus `control_flow::return_cleanup_calls_match`, and exactly one selected
`Return` terminator whose kind is `ReturnAggregate`/`ReturnScalar`, or `ReturnUnit` with a structural contract
result and a call-plan result that is either the zero-size integer shape or (AAPCS64 only) an indirect result
pointer in register `Aarch64X(8)` (`:7983-7992`).

**Cases by shape.**
- `EstablishStructuralArray :7915-7928`: accepted only if the declared type is `FixedArray` with literal length
  equal to `elements.len()`, or an `ElementView` with `access: None`. Any other array declaration fails.
- `MoveStructuralField :7860-7879`: the retained row must be `StructuralLeafCopy` with an `Ordinary` path;
  runtime-index segment count must equal `indices.len()`; with `field == None` path equality, otherwise the
  retained path is exactly the source path plus one trailing `Field`.
- Case `StructuralCaseMembership` (`:7801-7826`): only `Boolean` results; counts instructions whose provenance
  is exactly `[psi_operation]` and `[result.value]` and kind is `Load32` (checked by
  `placed_reference::case_tag_load_matches`) or `ZeroExtendU32` with empty path and offset 0; needs exactly 1.
- Default arm `_ => false` (`:7994`): any abstract operation kind not listed here is NOT retained (closed).

**Same job elsewhere.** The "retained row has same operation and result" test is the same one repeated in
`control_flow::retained` (`:8004`), `primitive_accesses::retained` (`:12189`) and `structural_case::retained` (`:12454`),
each with its own terminator or operation kinds. Suspected further copies in `structural_fields::retained` (`:16304`, not read).

**Stated limits.** None in these lines.

**Fail-open risks.** none seen. The `_ => false` is closed. `retained.is_some_and` is closed on `None`.

---

## 2. `mod control_flow`, lines 7999-8788

**Purpose.** For each abstract terminator operation (`InvokeNoNormal`, `ExternalCompletion`, `EliminateEmpty`,
`Crash`, `Return`, `ReturnUnit`, `Jump`, `Conditional`), checks that the target graph has exactly one block
whose terminator carries the same edge and fields, and that the cleanup-call rows in that block match the
cleanup actions of the source operation.

**Shape.**
- `retained(source, target) -> bool` (`:8004`, `pub(super)`). Step 1 (`:8005-8061`) selects candidate blocks by
  terminator variant and `psi_edge` equality; step 2 (`:8062-8073`) requires exactly one candidate and
  `cleanup_rosters_match`; step 3 (`:8074-8259`) compares every field of the terminator.
- `successor_matches` (`:8262`): edge, target block, bindings, structural bindings, cleanup actions.
- `cleanup_rosters_match` (`:8270`): total `InvokeOwned` actions across the source's cleanup rosters must equal
  the number of `CleanupCall` operations in the candidate block, and each roster must pass
  `edge_cleanup_calls_match`.
- `return_cleanup_calls_match` (`:8295`, `pub(super)`) and `edge_cleanup_calls_match` (`:8307`).
- Tests `cleanup_tests :8404-8787`: 3 `#[test]` fns (`nominal_image_return_rejoins_every_action_and_original_receiver :8420`,
  `empty_elimination_inspection_retains_its_exact_semantic_edge_and_source :8633`,
  `residual_image_cleanup_retains_exact_jump_and_return_rosters :8683`) that mutate one field at a time and assert
  rejection.

**Inputs and outputs.** Reads `target.graph.blocks`, each block's `terminator` and `operations()`, the structural
type catalog (through `crate::object::object_artifact::replay::structural::condition_layout::StructuralReplay`,
`:8276`, `:8301`). Output is a bool; mutates nothing.

**How it decides.**
- Selection of the candidate block is keyed on identity: `psi_edge` (`:8016`, `:8022`, `:8028`, `:8034`, `:8039`,
  `:8043`). The `Conditional` arm (`:8044-8059`) selects a block if EITHER target successor edge equals EITHER
  source successor edge, so selection is loose; the exact comparison happens later via `successor_matches` (`:8255-8256`).
- Cleanup call matching (`:8323-8399`) is keyed on `CallSiteOwner::CleanupAction { edge, action_ordinal }` where
  `action_ordinal` is the index in the actions list (`:8338-8342`), the callee machine id, requirement obligations,
  place, path, structural type, byte offset, runtime-index stride and operand value, and destination placement.
- `DiscardRoot`, `RetireHome`, `DiscardResidual` actions are skipped by `continue` (`:8325-8327`): only
  `InvokeOwned` produces a call row.

**Cases by shape.**
- "exactly one argument": `arguments.len() != 1` rejects (`:8348`); `call_plan.result.is_some()` rejects
  (`:8347`); `nominal.claim_transfers` must be empty (`:8345`); access must be `Owned` (`:8346`, `:8393`).
- Return arms: `Return` pairs only with `ReturnScalar` and `ReturnUnit` only with `Return` terminator variant
  (`:8175-8224`).
- `_ => false` (`:8060`, `:8258`).

**Same job elsewhere.** `edge_cleanup_calls_match` (`:8307`) and `placed_reference::backing_geometry::incoming_calls::cleanup_source_matches`
(`:9868`) both check a target `CleanupCall` against an `OwnedCleanupInvocation` (callee, requirement
obligations, empty `claim_transfers`, `Owned` access, `arguments.len() == 1`, place/path/type). They differ in
which side they trust: `:8307` also re-derives byte offset, strides and shape through `StructuralReplay`;
`:9868` compares only place, path, access and structural type. `StructuralReplay::from_catalog` is rebuilt for
every call to `cleanup_rosters_match` (`:8276`) and `return_cleanup_calls_match` (`:8301`).

**Unused or one-caller code.** `return_cleanup_calls_match`: outside the module, 1 call site
(`:7973`, in `aggregate_results`), plus 2 in-module uses (`:8189`, `:8212`) and tests (`grep -n "return_cleanup_calls_match" function_fragments.rs`).
`control_flow::retained`: 7 calls in `function_fragments/source/retained_source_validation.rs`
(`grep -c "control_flow::retained"`; lines 768, 769, 794, 804, 809, 813, 832) and one test at `function_fragments.rs:15152`.

**Stated limits.** None.

**Fail-open risks.**
- `edge_cleanup_calls_match` skips `DiscardRoot`/`RetireHome`/`DiscardResidual` (`:8325-8327`) without checking
  that the target block performs any matching work. For `Jump`, `Return`, `ReturnUnit`, `Conditional` the full
  action list equality is checked separately (`:8188`, `:8211`, `:8239`, `:8267`). For `InvokeNoNormal`,
  `ExternalCompletion`, `EliminateEmpty` and `Crash` arms (`:8075-8174`) no cleanup-action list is compared;
  only the count of `InvokeOwned` calls against `source.cleanup_rosters()` is. Whether those four source
  operations ever carry non-`InvokeOwned` actions in `cleanup_rosters()` I did not trace. Classification: open.
- `cleanup_rosters_match` returns `false` if `StructuralReplay::from_catalog` fails (`:8276-8278`): closed.

---

## 3. `mod placed_reference`, lines 8790-12183

**Purpose.** Reconstructs, from the selected (physical) instruction stream, the address that each reference,
borrowed pointer or case-dispatch read actually computes, and compares it with the address the abstract
operation's source place and projection require. It also proves that a reference's backing storage meets the
alignment and extent the checked reference requires.

### 3a. `backing_geometry` (`:8801-10526`)

**Shape.**
- `activation_residence` (`:8802-9167`): `validate` (`:8837`) checks installation helper calls
  (`InstallStaticInterrupt`) have a backing proof; `BackingGeometry::for_activation_residence` (`:8911`),
  `require_activation_arguments` (`:8933`), `activation_argument` (`:8965`), `descriptor_queries` (`:9073`),
  `fields_queries` (`:9143`).
- `incoming_calls` (`:9169-9996`): `source_sites` submodule (`:9170-9617`, `SourceSites`, `SourceSite`, `SourceCallTarget`,
  3 `#[test]` fns `:9386`, `:9431`, `:9589`); `IncomingCalls` (`:9636`) is a table of
  `IncomingCall { caller, callee, site: CallSiteOwner, inputs: Result<Inputs, &'static str> }` sorted by
  `(callee, caller, site)` (`:9824`); `Inputs` is `Invocation` or `Entry` (`:9645`). Functions: `new` (`:9657`),
  `range` (`:9827`), `substitute` (`:9831`), `cleanup_source_matches` (`:9868`), `invocation` (`:9891`),
  `selected_origin` (`:9923`), `entry` (`:9946`).
- `BackingGeometry<'a>` (`:10017`) holds `abstracted`, `selected`, `targeted`, `graph: &[DescriptorMachine]`,
  `analysis: DescriptorAnalysis`, `incoming`, `failure_detail: Option<String>`.
- `source_operation_position` (`:10029`), `record_selected_formation_calls` (`:10050`), `graph` (`:10169`),
  `completed_graph` (`:10181`), `BackingGeometry::new` (`:10191`), `proves` (`:10211`), `describe_origin` (`:10287`),
  `origin` (`:10352`), `check_origin` (`:10366`).

**Inputs and outputs.** Reads the abstract plan (`AbstractOperationPlan`), the post-allocation machine program
(`selected`), the target plan, and a descriptor/reference-origin graph built by
`optimization_unit_semantics::descriptor_results::abstract_reference_catalog_graph` (`:10172`) or
`abstract_borrowed_storage_catalog_graph` (`:8826`). Result is `Result<(), &'static str>`; on failure
`failure_detail` stores a `format!` of homes and origins (`:10347-10349`) used for error text only.

**How it decides.**
- `proves` (`:10211`) picks the backing kind: `OrdinaryRam` is checked by geometry; `BorrowedExtent` is refused
  unconditionally (`:10222-10224`); `Resident` is checked by the unique access-view plan whose `carrier` equals
  the resident carrier, with `schema` equality and `placement.content_interpretation().commitment()` equality
  (`:10225-10244`), and returns `Ok` without examining any allocation.
- `check_origin` (`:10366`) follows an origin back through callers. A parameter whose access is not `Owned` (or
  an incoming-contents origin) is resolved by substituting every incoming call (`:10415-10423`); refuses if the
  machine is the program entry or appears as a dynamic-dispatch table row (`:10401-10412`), and refuses if no
  caller exists (`:10425-10429`). Cycles refused by a `pending` list (`:10373-10378`). The final check is
  numeric: schema id equal, `region.byte_length == required.source_byte_length`, allocation alignment divisible
  by `modulus`, `byte_offset % modulus == residue`, and `byte_offset + byte_length <= allocation.byte_size`
  (`:10509-10517`). For `Activation` the check is only that the container is inline and the home is a non-indirect
  allocation (`:10438-10471`).
- `source_operation_position` (`:10029`) refuses a missing or duplicated operation id.
- Keys: `MachineId`, `OperationId`, `PlaceId`, `EdgeId`, `InvocationSite`; the only spelling use is
  `failure_detail` text (`:10347`).

**Cases by shape.**
- `descriptor_queries` (`:9087-9138`) refuses `TypeParameter` (`:9088`, "generic backing needs a closed application")
  and `Opaque` (`:9093`); recursion handled for `Reference`, `FixedArray`/`ElementView`, `Record`, `Mixed`, `Sum`;
  scalar, atomic and byte-sequence leaves end the walk.
- `record_selected_formation_calls` (`:10050`) only handles `TargetUnitOperation::Call` to an `NativeCallTarget::Machine`
  whose original is an `AbstractOperation::BoundaryCall`; any other call is skipped with `continue` (`:10062-10069`,
  `:10095-10097`).
- `IncomingCalls::new` (`:9657`) handles four caller kinds: `Call`, `CleanupCall`, `InstallStaticInterrupt`,
  and terminators `InvokeNoNormal` and `ExternalCompletion` with a `Checked` binding. Interrupt helper calls get a fixed
  `Err("installation helper arrival has no reconstructed backing observation")` (`:9760`) so any backing that
  flows through them is refused.
- `selected_origin` (`:9923`) rejects `NativeCallOrigin::Authored`.

**Same job elsewhere.**
- `activation_residence::graph` (`:8821`) and `backing_geometry::graph` (`:10169`) both build a descriptor graph and then
  call `record_selected_formation_calls`; they differ only in the catalog function (borrowed-storage vs reference).
- `BackingGeometry::new` (`:10191`) and `for_activation_residence` (`:8911`) are identical but for the slot kind in
  `DescriptorAnalysis::with_reference_slots` / `with_borrowed_storage_slots`.
- The `(block, ordinal)` position computation appears three times: `source_sites` (`:9217`, `:9252`) and
  `source_operation_position` (`:10036-10044`).

**Unused or one-caller code** (grep over `function_fragments.rs` and `function_fragments/source/`):
`completed_graph` 2 callers (`retained_source_validation.rs:81`, `:372`); `validate_activation_residence` 1 caller
(`retained_source_validation.rs:355`); `BackingGeometry::new` 2 callers (`retained_source_validation.rs:85`, `:379`);
`for_activation_residence` 1 caller (`:8891`); `record_selected_formation_calls` 2 callers (`:8833`, `:10186`);
`source_operation_position` 2 production callers (`:8960`, `:10082`).

**Stated limits.** Quote: "generic backing needs a closed application before descriptor tracing" (`:9090`);
"opaque backing needs its selected representation before descriptor tracing" (`:9095`); "borrowed extent requires
a successful View lifecycle" (`:10223`); "entry or dynamic input has no closed physical backing supply" (`:10410`).

**Fail-open risks.**
- `proves` `Resident` arm (`:10243`) returns `Ok(())` after a schema/commitment compare with no allocation check;
  stated by the structure only. Classification: open (a resident backing whose carrier is unique and whose schema and
  commitment match is accepted without looking at where it lives; not traced to what else guards the placement).
- `record_selected_formation_calls` skips a machine `Call` whose original is not a `BoundaryCall`
  (`:10095-10097`) and skips when an invocation already exists (`:10072-10076`). Skipped rows leave the call absent
  from the graph; later lookups (`invocation :9902`) return `Err`, so this fails closed. Classification: traced.
- `validate` (activation) returns `Ok(())` when the filtered call list is empty (`:8879-8881`) after the
  required-versus-actual roster compare (`:8874`), so an empty requirement set accepts. Classification: traced (the roster
  compare guarantees `required` is empty too).

### 3b. Address replay (`:10528-11632`)

**Shape.**
- `Address { base: AddressBase, loads: Vec<(u64, Vec<(ValueId,u64)>)>, offset: u64, indices: Vec<(ValueId,u64)> }`
  (`:10535`); `AddressBase::{Allocation(LocalStorageSlotId), BorrowedFormal(PlaceId)}` (`:10529`).
- `SelectedAddressInputs` (`:10635`): per-register `DefinitionOccurrence::{Missing, Unique, Repeated}`
  (`:10627`), `ordinary_outputs`, sorted `operation_instructions`, sorted `locations`; `new` (`:10644`).
- `AddressInputs` (`:10992`), `AddressReplay<'a>` (`:11034`) with lazily built `inputs`, `spills`, `layouts`
  (`NativeGeometry`) and `failure: Option<&'static str>`.
- Free fns: `operand` (`:10542`), `all_instructions` (`:10553`), `precedes` (`:10563`), `precedes_blocks` (`:10594`),
  `address_output` (`:10769`), `source_value` (`:10789`), `resolved_source_value` (`:10826`), `definition` (`:10868`),
  `scaled_index` (`:10914`), `address_root` (`:11396`), `block_entry_pointer` (`:11426`), `geometry_address` (`:11518`).
- `AddressReplay::address` (`:11115`) is the recursive reconstruction with fuel `remaining`, starting at
  `virtual_registers.len() + 1` (`:11106`).
- Public to the module tree: `retained` (`:11265`, reference formation), `case_tag_load_matches` (`:11565`),
  `dispatch_indices` (`:11637`), `dispatch_read_matches` (`:11702`), `dispatch_source_matches` (`:11791`),
  `borrowed_address_matches` (`:11824`), `pointer_definitions` (`:11877`, debug text only).
- Tests `dispatch_address_tests :11923-12182`: 2 `#[test]` (`:11938`, `:12132`).

**Inputs and outputs.** Reads one `TargetFunction` and its `SelectedFunction` (instruction rows with `kind`,
`operands`, `provenance`, `memory_accesses`, `local_storage_slots`, `virtual_registers`). Produces `Option<Address>`
or `Option<()>`/`Result<u32,String>`; mutates only the replay's lazy caches and `failure`.

**How it decides.**
- `address` (`:11115`): follows definitions backward. Accepted instruction kinds: `FrameAddress` on a `Local` slot
  (base), `AddressOffset` (adds offset), `CopyI64` (passes through), `Load64` (pushes a load, resets offset),
  `ByteViewAddress` (adds a scaled index); anything else is `None` (`:11219-11261`). Also a `StructuralParameter`
  register becomes `BorrowedFormal` if the declared access is not `Owned` and the register has no defining row
  (`:11127-11152`); a spill reload is followed through `SpillValues::writer` (`:11196-11212`).
- `definition` (`:10868`) accepts a defining row if its provenance names the same operation/edge/block site, OR
  if its kind is `FrameAddress` or `CopyI64` regardless of provenance (`:10892-10896`); then requires uniqueness,
  equality with the unique defining instruction, and an origin of `AbiTransport`/`StructuralObservation`/`InstructionScratch` owned by that instruction.
- Ordering is keyed on instruction position within block, or reachability from `entry_block` across blocks
  (`precedes_blocks :10594-10624`): a producer in another block "precedes" a consumer if the consumer block is not
  reachable from entry without passing through the producer block.
- `retained` (`:11265`) checks, in order: backing proof, unique destination slot of exactly size 8 / align 8
  with id `Structural{operation, place}` (`:11288-11295`), schema match, exactly one `WritePlace` 8-byte `Store`
  at offset 0, all address-forming instructions of the operation precede that store (`:11342-11360`), captured
  address equals the expected one, destination address equals the allocation slot, and exactly one matching
  `EstablishAccessReference` target row (`:11382-11393`).
- `case_address_matches` (`:11592`): requires `geometry.readable`, matching `byte_offset`, no scalar-descriptor
  pointer slot, sum layout `tag_byte_offset == 0` and `tag_shape == integer(4, 4)` (`:11623-11625`), then address
  equality after adding `displacement`.
- `dispatch_indices` (`:11637`) rebuilds `LegalizedRuntimeIndexOperand` rows from geometry and asks
  `unit.accepted_obligation_fact(...)` for the accepted fact (`:11680-11684`), keyed on obligation id.
- `dispatch_read_matches` (`:11702`): exactly one 4-byte `ReadPlace`/`ReadIndexedPrimitive` access in the source block.
- `scaled_index` (`:10914`): the selector must be a `WrappingMultiplyI64` by a `MaterializeI64` unsigned stride; a narrow
  selector must go through one of six exact extension kinds, with width checked (`:10936-10954`).

**Cases by shape.** Quotes: "Address lowering explicitly normalizes a narrow selector." (`:10931`);
`address_output` lists 3 output positions by kind (`:10770-10785`); `block_entry_pointer` requires slot size 8 and
alignment 8 (`:11487`); the destination slot of a reference is size 8 / align 8 (`:11293`); dispatch reads are
4 bytes only (`:11719`, `:11766`); the 7 allowed address-forming kinds in `retained` (`:11347-11353`); the
`resident` flag changes how `geometry_address` folds `byte_offset` (`:11542-11553`).

**Same job elsewhere.**
- Two `precedes`: free `precedes` (`:10563`) scans `selected.blocks` linearly per call; `SelectedAddressInputs::precedes`
  (`:10730`) uses a sorted `locations` table. Both end in `precedes_blocks`. The free one is used by `source_value`
  (`:10822`), `pointer_definitions` (`:11903`) and from `function_fragments.rs:3579`; the method is used by all
  address replay code. `grep -rn "precedes("` shows 42 lines in `function_fragments.rs`, many are tests.
- `source_value` (`:10789`) recomputes the Missing/Unique/Repeated classification (`:10803-10816`) that
  `SelectedAddressInputs::new` already builds (`:10676-10703`). `source_value` has 1 caller outside the module,
  `function_fragments.rs:3558`. The comment at `:10693` says "The old predicate counted an instruction once
  even when more than one of its operands defines this register" and the two do the same classification with
  different counting: `source_value` (`:10803-10811`) tests `any` operand per row (also once per instruction).

**Unused or one-caller code** (grep in `function_fragments.rs` plus `function_fragments/source/`):
`borrowed_address_matches` 1 caller (`function_fragments.rs:3521`); `dispatch_indices`, `dispatch_read_matches`,
`dispatch_source_matches` 1 caller each (`:12516`, `:12529`, `:12652`); `case_tag_load_matches` 2 callers (`:7819`, `:11742`);
`placed_reference::retained` 1 caller (`retained_source_validation.rs:612`); `AddressReplay::new` 2 callers
(`retained_source_validation.rs:423`, `function_fragments.rs:7490`); `pointer_definitions` 1 caller (`:11871`, error text);
`placed_reference::source_value` 1 caller (`:3558`).

**Stated limits.** "The old predicate counted an instruction once even when more than one of its operands
defines this register." (`:10693`) records a prior behavior, not a limit. Comment at `:11562-11564`: "A large tag
displacement may move from the load into an earlier pointer adjustment."

**Fail-open risks.**
- `definition` (`:10892-10896`) takes any `FrameAddress` or `CopyI64` defining row without checking its provenance
  site. A copy with a different operation's provenance still passes this filter (it must still be unique, precede
  the consumer and have an allowed register origin). Classification: traced (bounded by the uniqueness and origin checks at `:10901-10910`).
- `retained` / `dispatch` paths all return `None` on any miss: closed.
- `block_entry_pointer` (`:11426`) closed on every miss.
- `address` fuel exhausted returns `None` (`:11124`): closed.

---

## 4. `mod primitive_accesses`, lines 12185-12281

**Purpose.** Checks that the target kept three primitive-access operations: `ObserveStorageLocation`,
`PrimitiveScalarRead`, `WriteOnlyPrimitiveStore`.

**Shape.** One function `retained(source: &AbstractOperation, target: &TargetFunction) -> bool` (`:12189`).

**How it decides.** For each kind, filters `target.graph.operations()` by `psi_operation` equality (and `query`
equality for `ObserveStorageLocation`, `:12210`), takes the first match, compares result, source place, path, and
`structural_fields::retained_selectors(path, indices)` (`:12217`, `:12245`, `:12275`), and requires there to be no
second match (`queries.next().is_none()` etc.). For stores it compares `stored.source_value()` and `scalar_type()`
with the abstract value (`:12274`). Keyed on identity.

**Cases by shape.** `_ => false` (`:12278`). Only these three kinds.

**Unused or one-caller code.** `primitive_accesses::retained`: 1 caller
(`retained_source_validation.rs:602`, `grep -rn "primitive_accesses::retained"`).

**Same job elsewhere.** Same "find the single target row with this `psi_operation`, then compare fields" pattern as
`aggregate_results` (section 1) and `structural_fields::retained` (`:16304`, not read).

**Fail-open risks.** none seen.

---

## 5. `mod stack_calls`, lines 12285-12443

**Purpose.** Lists the call edges of one function for stack-depth accounting: ordinary internal calls from the
text section, plus callable-value and dynamic-dispatch calls expanded to every possible callee.

**Shape.** `CallStackEdge { owner: CallSiteOwner, callee: MachineId, section_offset: u64 }` (`:12292`, `Copy`);
`From<&PlacedInternalMachineCallResolution>` (`:12298`); `stack_calls(source, machine, callable_invocations) ->
Result<Vec<CallStackEdge>, Error>` (`:12308`).

**Inputs and outputs.** Reads `source.text_section().resolved_internal_machine_calls` filtered by caller, the
selected calls of the function (`emission_facts().physical.calls.function(machine)`), the placed function's
instruction spans, the dynamic dispatch catalog tables, and `ReachableCallableInvocations`. Output is the edge list.

**How it decides.**
- `NativeCallTarget::Callable { callee, contract }` (`:12338`): looks up the invocation by `call_site(call.owner)`;
  if none, then errors only when the machine is in `reached_machines` (`:12343-12351`); otherwise `continue`
  (`:12353`). A found invocation must match `CallableInvocationTarget::Value(callee, contract)` (`:12355-12361`),
  have `targets_are_resolved()` else `Error::Unsupported` (`:12362-12366`), and the instruction target must be
  `SelectedCallTarget::Callable { contract }`. One edge per `invocation.selected_machines`.
- `NativeCallTarget::Dynamic { interface, slot }` (`:12391`): requires the slot to exist and, for every table of
  that interface, takes the row at `slot`; `None` rows are skipped (`:12424`); a `Some` target without an emitted
  function is an error; targets sorted and deduplicated (`:12433-12434`).
- Keyed on identity (machine, interface, slot, contract), not spelling.

**Cases by shape.** `let NativeCallTarget::Dynamic { .. } = call.call.target else { continue; }` (`:12391-12396`):
every target kind other than `Callable` and `Dynamic` is skipped here; plain machine calls are expected to arrive through
`resolved_internal_machine_calls` (`:12314-12319`).

**Unused or one-caller code.** `stack_calls`: 2 callers (`function_fragments.rs:2831`,
`function_fragments/validation.rs:632`); re-exported at `function_fragments.rs:17032`.

**Stated limits.** Comment: "Retaining an uninvoked body adds no activation demand." (`:12352`).
`Error::Unsupported("callable stack demand has unresolved code and no physical ceiling")` (`:12363-12365`).

**Fail-open risks.**
- `:12353` `continue` on a callable call with no reached invocation, when the machine is not in `reached_machines`:
  contributes no stack edge. Classification: stated ("Retaining an uninvoked body adds no activation demand").
- `:12424` `None` dispatch table row contributes no edge: a call through an empty slot is counted as no callee.
  Classification: open (no comment; not traced to whether an empty slot is unreachable by construction).
- Targets other than `Callable`/`Dynamic` skipped (`:12391-12396`): they depend on `resolved_internal_machine_calls`
  holding them. Classification: open (I did not read where that list is built).

---

## 6. `mod structural_case`, lines 12445-12967

**Purpose.** Checks that a source `StructuralCase` dispatch (a branch on which variant a structural value holds)
was kept intact by selection: the target block has a `StructuralCase` terminator, each case edge maps to one
selected conditional-branch or jump successor, and the tag read and each captured payload address resolve to the
same element.

**Shape.** `retained(function, source_blocks, source_parameters, target, source_block, source, path, cases, unit,
addresses) -> bool` (`:12454`, `pub(super)`, `#[allow(clippy::too_many_arguments)]`);
`projected_source_matches` (`:12630`); `source_slot_matches` (`:12702`). Tests `:12773-12966`: 3 `#[test]`
(`addressed_case_parameter_keeps_its_declared_owner_and_read_access :12794`,
`case_parameter_accounting_requires_the_owned_function_parameter :12842`,
`case_home_accounting_requires_the_unique_actual_arrival_owner :12896`).

**How it decides.**
- Block selection (`:12469-12497`): the target block's `StructuralCase` terminator must have the same `psi_block`,
  same `root.place()`, same projection path and contain the first case's `psi_edge`; exactly one block may match
  (`:12509`).
- Runtime index selectors checked through `structural_fields::retained_selectors` (`:12512`) and
  `placed_reference::dispatch_indices` (`:12516`).
- `dispatch_read_matches` is called only `if cases.len() > 1` (`:12528-12541`). A single-case dispatch skips the
  tag-read check.
- Per case (`:12544-12626`): exactly one target row with that edge; `case`, `target`, `structural_bindings` equal;
  exactly one `Semantic`-role successor with that `psi_edge` among `ConditionalBranch` (`when_zero`, `when_nonzero`) and
  `Jump` terminators; successor `source_target`, binding list, case id, `case_tag`, `cleanup_actions`, and each payload
  (field id, `field_byte_offset`, parameter value, scalar type) must agree.
- `source_slot_matches` (`:12702`): for a `Borrowed` dispatch source, the place must have exactly one declaration, either
  as a block arrival parameter with a borrowed access (`reading_borrow_pair_pattern!`), or a function parameter whose
  access is `Owned` or a borrow (`:12746-12748`); for a `Local` slot it checks owner uniqueness per slot kind
  (`Structural`, `StructuralBlockParameter`, `StructuralParameter` must be `Owned`).

**Cases by shape.** Only `ConditionalBranch` and `Jump` terminators are searched for case successors
(`:12559-12566`, `_ => [None, None]`); other terminator kinds (for example a multiway table) are not seen here.
`SelectedCaseDispatchSource::Local` source: rejects any `Referent` or runtime-index path segment (`:12645-12646`),
and does no address replay. `source_slot_matches` final `_ => false` (`:12769`) rejects other slot kinds.

**Same job elsewhere.** `source_slot_matches` repeats the "unique owner of this place among block arrival
parameters" scan seen in `placed_reference::address` (`:11127-11141`, `StructuralParameter` origin) and
`structural_homes::resolve` (suspected, not read; used at `:9309`).

**Unused or one-caller code.** `structural_case::retained`: 1 caller (`retained_source_validation.rs:834`).
`dispatch_indices`/`dispatch_read_matches`/`dispatch_source_matches` are used only here.

**Fail-open risks.**
- `cases.len() > 1` guard at `:12528`: for exactly one case the tag read is not replayed. What wrong program gets
  through: a single-case `StructuralCase` whose tag load was altered or removed. The index and case-edge checks
  still run. Classification: open (no comment; I did not trace whether a one-case dispatch emits a tag read at all).
- `Local` dispatch source (`:12644-12648`): no physical address check, only slot ownership. Classification: open.
- `source_slot_matches` accepts an `Owned` function parameter for a `Borrowed` dispatch source (`:12746-12748`, test at `:12802-12813`).
  Classification: stated by the test name "addressed_case_parameter_keeps_its_declared_owner_and_read_access".

---

## 7. `mod structural_fields`, lines 12969-13800 (test module only)

**Purpose.** In my range this is only the head of the module (imports `:12970-12975`) and the start of its
`#[cfg(test)] mod tests` (`:12977`). The production functions are after line 13800 and were not read by me.

**Shape of what I read.** Test imports of `retained`, `replacement_retained`, `indexed_store_retained`,
`indexed_read_footprint_retained`, `replacement_footprints_retained` and `requires_graph_storage_replay` (`:12979-12983`).
Fixtures: `field_fixture` (`:13123`, builds one `AbstractFunction` and a matching `TargetFunction` with a `SharedBorrow` record parameter
and one `StructuralScalarFieldRead`), `plain_structural_type` (`:13083`), `packed_record_field` (`:13104`), mutable-field
helpers (`:13274-13357`). Tests that start in range: `borrowed_block_reads_retain_declarations_independently_of_native_carrier :13024`,
`byte_length_membership_preserves_metadata_kind_and_exact_borrowed_subject :13359` (9 mutations, each must be rejected),
`local_byte_length_membership_requires_the_exact_owned_home :13465`, `whole_byte_replacement_rejoins_qualified_entry_and_arrival_storage :13688`,
`byte_replacement_membership_retains_every_operand_and_writable_access :13789` (ends mid-function at the range edge `13800`).

**How the tests decide.** Each test builds a matching pair, asserts `retained(...)` is true, then mutates one field at
a time (operation id, value id, scalar type, place, path, field id, access, position, qualification, duplicate row)
and asserts `retained(...)` is false. Quote: `"forged owned replacement {corruption}"` (`:13637`).

**Stated limits / fail-open.** None in the part I read.

---

## Test inventory in the range (grepped by reading)

`#[test]` fns inside 7801-13800: `control_flow::cleanup_tests` 3, `incoming_calls::source_sites::tests` 3,
`dispatch_address_tests` 2, `structural_case::tests` 3, `structural_fields::tests` 5 (the fifth is cut at `:13800`).
All test fixtures are hand-built synthetic structs; none read a `.omg` program.

---

## Coverage

- Read in full: `function_fragments.rs` lines 7801-13800 (6000 lines), in four reads of 1500 lines.
- Skimmed: none.
- Not read: none of my unit. Not read and outside my unit: `function_fragments.rs` 1-7800 (first reader), 13801-21313,
  and `function_fragments/source/retained_source_validation.rs` (only grepped for callers).

## Five things a maintainer most needs to know about this unit

1. This range is "replay" validation, not production: each module answers "did selection keep exactly this abstract
   operation?" by finding one target row by identity (`psi_operation`, `psi_edge`, place) and comparing every field,
   returning `false` on zero or two matches (`control_flow :8062-8073`, `primitive_accesses :12215-12218`, `structural_case :12509`).
   Its callers live in `function_fragments/source/retained_source_validation.rs` (`control_flow::retained` x7, `structural_case::retained`
   `:834`, `primitive_accesses::retained` `:602`, `placed_reference::retained` `:612`).
2. `placed_reference` rebuilds machine addresses from the selected instruction list by walking definitions backward
   over a closed list of kinds (`FrameAddress`, `AddressOffset`, `CopyI64`, `Load64`, `ByteViewAddress`; `:11219-11261`),
   and compares the result to an address computed from geometry. A new address-forming instruction kind in selection is
   refused here until added to `address` (`:11260 _ => None`) and `address_output` (`:10784`).
3. `check_origin` (`:10366`) proves a reference's backing by following borrowed parameters through every caller,
   refusing entry machines, dynamic-dispatch rows, cycles and installation helper calls (`:9760`), and ends in a numeric
   check of schema id, byte length, alignment modulus/residue and allocation bounds (`:10509-10517`); the `Resident` backing
   kind skips that numeric check (`:10225-10244`) and `BorrowedExtent` is always refused (`:10222`).
4. Two conditions look asymmetric: `dispatch_read_matches` runs only when a dispatch has more than one case
   (`:12528`), and a `Local` dispatch source gets no physical address replay (`:12644-12648`).
5. Several helpers duplicate each other: two `precedes` implementations (`:10563`, `:10730`), a second
   defining-row classification in `source_value` (`:10803`), two cleanup-call matchers (`:8307`, `:9868`) and
   twin `graph`/`BackingGeometry::new` constructors (`:8821`/`:10169`, `:8911`/`:10191`).

## What you are unsure about

- I did not read `retained_source_validation.rs`, so I do not know what supplies `retained` to the `aggregate_results` arms
  at `:7801-7997` or what happens when no target row exists for an operation kind absent from those arms.
- Whether the cleanup-action roster for `InvokeNoNormal`, `ExternalCompletion`, `EliminateEmpty` and `Crash` is compared
  anywhere other than the `InvokeOwned` count (`control_flow :8062-8174`) is untraced.
- Whether `NativeCallTarget` kinds other than `Callable`/`Dynamic` always appear in `resolved_internal_machine_calls`
  (`stack_calls :12314`, `:12391-12396`) is untraced.
- Whether a single-case `StructuralCase` can omit the tag read (guard at `:12528`) is untraced.
- `structural_homes::resolve`, `HomeInputs`, `SpillValues`, `call_target_matches` and `thin_address_join` are used here
  but defined outside my range (`thin_address_join :16991`); their behavior is inferred from names only.
