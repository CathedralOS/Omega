Second pass for unit 111

Ranges covered (all paths under `omega-rust/psi/interpreter/terminal-interpreter/src/`, checkout `<checkout>/`):
- `boundary_storage_inputs.rs` 1-2703 (2703 lines)
- `boundary_values.rs` 1-2391 (2391 lines)
Total 5094 lines. Both ranges are whole files, so neither starts mid-function. The earlier notes (`unit_111.md:8-9`) list both files as "NOT read"; my ranges contradict nothing there.

Method: every line was read with the Read tool, in chunks. Counts below come from the Grep tool (count or content mode) over `omega-rust` in the checkout. Grep "count" mode counts matching lines, not occurrences. Path prefix `.../src/` is shortened to `src/` below.

---

# File 1. `src/boundary_storage_inputs.rs` (2703 lines, read in full)

## 1.1 Purpose
Header (:1-8): captures storage loans before a host call, shows the host byte-views and structural buffers, checks what the host changed against the original access and type restrictions, and commits the result back into the original storage. It is the "storage" half of the boundary call (scalar loans are in `BoundaryScalarLoans`, in `boundary_effects.rs`, not in this unit). All state for one host call lives in one `BoundaryStorageInputs` value (:2375).

## 1.2 Shape (module map, then types)
Inline modules, in file order:
- `borrowed_completion` (:10-296): `stage` (:25-271) validates a host-completed structural buffer against its `StructuralLoanBinding` and prepares stores or a replacement; `commit` (:273-295) applies them.
- `byte_views` (:297-591): `TerminalBoundaryByteViewBuffer` (:314, public, fields: `identity: Arc<()>`, `argument_index`, `access`, private `length`, `original_storage: Option<TerminalOriginalStorageLoan>`, private `acknowledged`); `BoundaryByteViewBinding` (:350, fields `identity`, `argument_index`, `access`, `source: ByteSequenceBinding`, `referent`, `original: ByteSequenceStorage` (a byte copy), `residents`). Methods on `BoundaryStorageInputs`: `capture_byte_view` (:361), `validate_byte_views` (:441), `commit_byte_views` (:511). Methods on the binding: `entry_bytes` (:555), `view_type` (:558), `array_window` (:561), `raw_bytes` (:582).
- `original_residence` (:592-1626): the "original storage" mode, where the host is given a loan on the real backing storage instead of a copy. `CapturedOriginal` (:612: `owner`, `request`, `endpoints`); free fn `capture_byte_view` (:618-787, a different function from the one at :361); `BoundaryStorageInputs::prepare_original_storage` (:790-1080); `original_scalar_storage` (:1083); `endpoint_parent` (:1111); `impl TerminalExecution`: `original_byte_array_owner` (:1124), `original_scalar_allocation` (:1179), `original_scalar_owner` (:1228), `original_storage_owner` (:1271), `original_storage_request` (:1305), `original_scalar` (:1351); recursive `capture_graph` (:1462-1625).
- `reference_outputs` (:1627-2008): `source` (:1640) finds the one captured input a selection names; `stage` (:1655) validates the references a host writes into mutable reference slots.
- `reference_result` (:2009-2156): `require_result_captures` (:2018) and `prepare_reference_result` (:2048) for a host function that returns a reference.
- `structural_buffer` (:2157-2277): `TerminalBoundaryStructuralBuffer` (:2166, public; fields `original_storage`, `identity`, `argument_index`, `access`, `structural_type`, `values: Vec<TerminalBoundaryStorageValue>`, `references: Vec<BoundaryReferenceReplacement>`); `TerminalBoundaryReferenceSelection` (:2177; `identity`, `argument_index`, `path`, `access`); `ReferenceOutputWrite {Descriptor, Address}` (:2202); host-facing mutators `replace_reference` (:2224), `replace_reference_address` (:2232), `replace` (:2264).
- Top level (:2279-2703): `TerminalBoundaryStorageBuffer {Scalar, Structural, ByteView}` (:2292) with `select_reference` (:2299); `StructuralLoanBinding` (:2343); `OwnedPayloadSnapshot` (:2367); `BoundaryStorageInputs` (:2375; fields `buffers`, `scalars: BoundaryScalarLoans`, `structural`, `owned`, `reference_inputs`, `byte_views`); `CapturedReferenceInput` (:2384). Methods: `is_empty` (:2393), `capture_scalar` (:2402), `capture_fields` (:2423), `capture_reference_inputs` (:2551), `validate_writeback` (:2613), `commit` (:2684).

## 1.3 Inputs and outputs
- Reads the live `TerminalExecution` (structural cases, primitive storage, byte-array storage, call stack, `original_storage_roots`, `original_byte_cells`, `component_borrowed_addresses`, `reference_referents`, `account`) and the declaration (`BoundaryMachineDeclaration`: `reference_outputs`, `result_reference_regions`, `normal_qualifications`, `structural_parameters`).
- Produces host-visible buffers (`buffers`) at capture time and `TerminalOriginalStorageRequest`s that go to `handler.prepare_original_storage` (:868-870).
- Mutates execution only in three places: install of resident backings in `prepare_original_storage` (:940-1032: `prepare_resident_install` / `install_resident` / direct assignment `*cell = resident` at :1031); `commit` paths (`record_original_scalar_write` :285, :541; `store_primitive_storage` :287; `replacement.publish` :281; `references.publish` :292); and the `original_storage_roots` / `original_byte_cells` appends (:991-995).
- Flow (inferred from callers, grepped): `engine_calls.rs:3406-3414` calls `capture_reference_inputs`, `require_result_captures`, `prepare_original_storage`; `engine_calls.rs:3553-3557` calls `validate_writeback` and `prepare_reference_result`. I did not read `engine_calls.rs`.

## 1.4 How it decides (what each judgment is keyed on)
- Buffer-to-binding matching is by `Arc::ptr_eq` on `identity: Arc<()>` plus `argument_index`: `borrowed_completion::stage` :30-31, `validate_byte_views` :457-458, `commit_byte_views` :520-522, `reference_outputs::source` :1644-1647, `reference_outputs::stage` :1688-1689, `TerminalBoundaryReferenceSelection::eq` :2185-2188. No names or spellings.
- Host edits to shared borrows: `buffer.values != binding.original` rejects (:34-35).
- Value ordering: buffer values must have strictly increasing `path` (:36-39), searched by `binary_search_by` on `CanonicalStructuralPath` (:67, :149).
- Scalars (`stage` :49-82): for mutable loans validates restoration (:50-56); reads the live primitive (:57-63); `expected` is the host-supplied scalar from the buffer if `direct_original` else the original scalar (:64-77); mismatch or `direct_original && buffer.original_storage.is_none()` rejects (:78-81).
- Cases: each `Case` leaf's current case must equal the one captured (:84-95). `changed_case` is computed by comparing each visible leaf's case with the original value at that path; an absent original counts as changed (`is_none_or`, :115-117).
- Case replacement (:118-133, :183-268): changing a case requires `validate_normal_case_preservation` (:128). Retained leaves are compared by `declaration` identity (`StorageLeafKind::Opaque`/`ByteField` `declaration`, :186-187, :202, :210). Namespace for a retained byte field is `CasePayload` when the second-to-last path segment is a `Case` (:211-218).
- Source restrictions on written scalars use `satisfies_source_restriction(value, field, encoding)` (:152-158).
- `capture_byte_view` (:361): access must be Shared/Mutable borrow, equal to `source.access()`, parameter type equal to referent type (`structural_type` ids), no projected qualifications; for a mutable borrow no qualifications at all (:372-387). Source must be `Immutable` or `MutableArray`; others rejected (:409-413). Captured length must equal the descriptor length (:415-417).
- `validate_byte_views` (:441-509): requires `original_storage` present (:460) and, for mutable, `acknowledged` (:463). With an array window: shared compares the live byte to the saved byte (:474-478); mutable only validates scalar-loan qualifications per element (:479-488). Without a window: shared only, compares resident backing bytes (:491-505).
- `prepare_original_storage` is a no-op when `original == false` (:798-800).
- Owner selection `original_storage_owner` (:1271-1303): picks the `original_storage_roots` entry with the same `opaque_identity` whose `path` is a prefix of the source path, taking the shortest path (`min_by_key(|(home,_)| home.path.len())`, :1276-1283). Otherwise `original_owned_allocation`, and a non-empty path then errors (:1297-1301).
- Reference-output staging (`reference_outputs::stage`): destination is `BoundaryReferenceDestination::Parameter(index)` (:1671); each declared destination needs exactly one binding (:1672-1676); output access must equal the slot's `StructuralTypeShape::Reference.access` and the binding must be `MutableBorrow` (:1738-1743); inactive case projections are skipped (:1744-1750); exactly one request per output path (:1751-1758); `access_covers(selected.access, output.access)` (:1760-1762); "anchor" matching over `output.dependencies` (:1799-1868); required normal memberships from `declaration.normal_qualifications` filtered by `argument_index` and `strip_prefix(output.path)` (:1893-1936). Total staged must equal total requested (`request_count != staged.len()`, :1997).
- Address-only writes: compares the stored-scalar interpretation of the old and new referent (:1971-1992).
- Reference result (`prepare_reference_result`): allowed only if some `result_reference_regions` row matches origin parameter, access, empty path, and `relative.starts_with(prefix)` (:2079-2095).
- Identity keys are `PlaceId`, `StructuralTypeId`, `opaque_identity: u64`, `CanonicalStructuralPathSegment` paths. Strings appear only in error text (`UnsupportedSemanticVariant("...")`, `stage:` labels). `format!("{error:?}")` is used only to build error detail (:1140-1143, and `in_argument_preparation` in the other file).

## 1.5 Cases by shape (fixed lists that refuse the rest)
- Byte-view source kinds: only `ByteSequenceBinding::Immutable` and `MutableArray` (:389-414). Quote: "original byte-view capture does not support a replaceable bounded byte owner" (:411).
- `array_window` (:561-580): `Immutable` (if it has an original array), `MutableArray`; `_ => return Err(malformed())` (:578).
- `raw_bytes` (:582-589): only `Immutable` with no `original_array`.
- `original_scalar_storage` (:1083): only the `ViewByte` endpoint is rewritten to an `ArrayElement`; any other endpoint is returned as is (:1087-1089). A `ViewByte` without a captured array window errors (:1090-1094).
- `endpoint_parent` (:1111): only `Scalar` and `ScalarField` endpoints; others error: "original foreign storage requires its captured array or descriptor home" (:1117-1119).
- `original_scalar` (:1351): refuses a codec whose `view != source` (:1356-1363); array cells need `encoding.source == cell.scalar_type()` (:1375-1382, :1402-1409); only `Scalar` and `ScalarField` endpoints otherwise (:1433-1448, `_ => malformed` :1448). Case-payload fields push `Path::Case(active.case)` (:1438-1444).
- `capture_graph` (:1462): refuses any recursion on the same referent type: "native original reference graph must have a finite acyclic declaration" (:1479-1483). Child references handled only for leaves of kind `Opaque` whose declaration shape is `StructuralTypeShape::Reference`; other opaque leaves are skipped (`continue` :1553, :1561). A child access must be Shared or Mutable borrow (:1568-1573), and a shared parent forces shared children (:1563-1567).
- Case replacement (`stage` :183-268): only `Opaque` and `ByteField` leaves are matched for retention; `_ => true` for the others (:188). New leaves of kind `Opaque`/`ByteField` that were not in the original are refused (:202, :210). Leaf kinds `EmptyScalarArray` and `ByteArray` are restaged (:221-259); `_ => {}` for others (:260).
- `select_reference` (:2299-2340): only `Scalar` and `Structural` buffers; `ByteView` errors "byte-view input cannot select an untyped reference output" (:2306-2310); accepted accesses are `SharedBorrow | MutableBorrow | WriteOnlyBorrow` (:2323-2328).
- `capture_fields` (:2423): `Owned` parameters take a separate snapshot path (:2434-2470); all other access kinds require `parameter.access == argument.access` and `multiplicity == Unrestricted` (:2471-2476).
- `capture_reference_inputs` (:2551): `ByteView` buffers error "byte-view reference outputs require an exact descriptor lifetime contract" (:2570-2573).
- `reference_outputs::stage` refuses a mutable slot whose destination is not `Parameter` only via the type (single variant, :1671). Mutable-only: `binding.access != MutableBorrow` rejects (:1741).

## 1.6 Same job elsewhere
- Within this file, constructing `PrimitiveStorage { endpoint: PrimitiveStorageEndpoint::ArrayElement { array, index }, encoding: None }` is repeated at :480-486, :541-547, :663-666, :1102-1108 (the last keeps `storage.encoding`).
- The "find the buffer for this argument" match over the three buffer variants appears at :1048-1058 (`argument_index` match), :2305-2321 (`select_reference`) and :2569-2585 (`capture_reference_inputs`). Each extracts `(identity, argument_index, access)` the same way for Scalar and Structural.
- The "is this case projection active under completed leaves" query is built twice with the same closure: `reference_outputs::stage` :1783-1792 and `reference_result::prepare_reference_result` :2111-2120. Both call `case_membership::case_projection_with_membership_is_active` and re-derive the path with `canonical_structural_storage_path`. A third variant using `cases` (a precomputed Vec) is at :1744-1747.
- The "selected source must keep normal-qualification preservation unless shared" check is duplicated: `reference_outputs::stage` :1877-1892 and `prepare_reference_result` :2130-2146. Same message shape ("...lacks exact normal qualification preservation").
- "mutable original storage lacks its owner qualification preservation" is checked in four places with different messages: :644-655 (byte array window), :807-818 (scalar), :1509-1520 (graph); plus `capture_fields` :2486-2501 for non-original loans.
- Buffer/binding pairing by `Arc::ptr_eq` is the same pattern as in `boundary_effects.rs` byte buffers (`BoundaryByteBufferBinding`; per the earlier notes, `boundary_effects.rs:593-606`). Suspected, not read here.
- Two functions named `capture_byte_view`: `BoundaryStorageInputs::capture_byte_view` (:361, copies bytes and records a binding) and the free fn `original_residence::capture_byte_view` (:618, builds the original-storage request). Different jobs, same name.

## 1.7 Unused or one-caller code (grepped)
Grep over `omega-rust/psi/interpreter/terminal-interpreter/src` for the method names:
- `prepare_original_storage`: this file's definition (:790); callers `boundary_effects.rs:1015-1022` (wrapper), `engine_calls.rs:3414`. The trait method `TerminalEffectHandler::prepare_original_storage` is a different function (`embedding.rs:1823,1837`, `boundary_effects.rs:3587`).
- `require_result_captures`: 1 caller, `engine_calls.rs:3410`.
- `prepare_reference_result`: callers `boundary_effects.rs:971-978` (wrapper) and `engine_calls.rs:3557`.
- `capture_reference_inputs`: `boundary_effects.rs:962-968` (wrapper), `engine_calls.rs:3406`.
- `validate_writeback` (on `BoundaryStorageInputs`): `boundary_effects.rs:998`, `engine_calls.rs:3553`.
- `capture_scalar` / `capture_fields` / `capture_byte_view` (on `BoundaryStorageInputs`): each 1 caller in `boundary_effects.rs` (:1071, :1124, :1098).
- Public host API: `acknowledge_original_write` (:336): 1 caller outside, `omega/interpreter/terminal-runtime/src/execution/host_imports/native_provider.rs:759`. `select_reference` (:2299): callers `native_provider/reference_outputs.rs:241,323`, `host_imports/integer_parsing.rs:252`, `compiler/examples/host_reference_result.rs:141`. `replace_reference` (:2224): 1 caller, `native_provider/reference_outputs.rs:335`. `replace_reference_address` (:2232): 1 caller, `integer_parsing.rs:261`.
- `TerminalBoundaryStructuralBuffer::replace` (:2264) and `TerminalBoundaryByteViewBuffer::{access, length, argument_index}` (:324-332): not grepped (generic names). Marked "not counted".
- No comment in this file calls anything "the single / shared / canonical" way.
- `BoundaryStorageInputs::is_empty` (:2393): not grepped.

## 1.8 Stated limits (quotes under 20 words)
- "case replacement cannot yet transfer or remove opaque child storage" (:192).
- "case replacement cannot establish new opaque child storage" (:202).
- "case replacement lacks a semantic codec for new byte fields" (:210).
- "original byte-view capture does not support a replaceable bounded byte owner" (:411).
- "original storage requires retained enclosing geometry after projected transfer" (:1299).
- "component scalar loan requires original allocation geometry for storage export" (:1319).
- "native original reference graph must have a finite acyclic declaration" (:1481).
- "reference output cannot establish membership on the descriptor itself" (:1911).
- "reference output selects storage invalidated by staged input case replacement" (:1795).
- "host reference result requires captured input storage" (:2027).
- "byte-view reference outputs require an exact descriptor lifetime contract" (:2571).
- 34 lines use `UnsupportedSemanticVariant(` in this file (Grep count); 130 lines mention `malformed` (Grep count, includes the import lines).
- Two `ByteArray` paths require `u8::try_from(byte)` and a literal path set of `FixedIndex` (:232-252).

## 1.9 Fail-open risks
- `borrowed_completion::commit` returns `Ok(())` at once for `SharedBorrow` (:277-279). Traced: `stage` already rejected any shared buffer whose `values != binding.original` (:34-35), so a host edit to a shared view does not reach commit. Reaching commit with a changed shared buffer would need `stage` to be skipped. `validate_writeback` calls `stage` for every non-Owned structural buffer (:2670-2672). Not fail-open given that call.
- Direct-original stores (`binding.direct_original`): `commit` calls `record_original_scalar_write(&storage)` (:285) instead of storing a value. The host has already written the resident backing. `stage` then compares the live value to the buffer value (:64-81). What `record_original_scalar_write` checks is in another file (not read): "open".
- `validate_byte_views` mutable path with an array window checks qualifications only (:479-488). It does not compare the written bytes to anything, because any `u8` is valid for a byte array element. Traced for u8 geometry (`byte_array_length` check at :629-633 in `capture_byte_view`).
- `reference_result::prepare_reference_result` skips the case-activeness check when no `structural` binding matches the selected argument (:2101-2105: `if let Some(binding)`). A selected argument that is a scalar or byte view has no cases, so nothing to check (inferred). Same shape at `reference_outputs::stage` :1771-1798.
- Normal-qualification preservation is skipped when `selected.access == SharedBorrow` (:1877, :2130). No comment says why. Inferred: a shared borrow cannot change membership. "open".
- `reference_outputs::stage` early returns `Ok(())` when `declaration.reference_outputs` is empty (:1667), after rejecting any buffer that carries reference requests (:1664-1666). Traced, fail-closed for undeclared requests.
- `reference_outputs::stage` `continue`s past bindings with no declared outputs (:1711-1713) and past inactive case projections (:1748-1750). A host request for such a path is then counted in `request_count` but not `staged`, so the final check at :1997-1999 rejects it. Traced.
- `capture_reference_inputs` returns `Ok(())` with an empty `reference_inputs` when the declaration has neither outputs nor result regions (:2557-2561). Later `source()` would fail with `malformed()` if a selection arrived (:1648). Traced.
- `original_scalar_storage` returns the storage unchanged for any endpoint that is not `ViewByte` (:1087-1089). Such an endpoint is then passed on; `endpoint_parent`/`original_scalar` reject non-scalar endpoint kinds later (:1117, :1448). Traced.
- `capture_graph`'s `request.scalar_projection` etc. are plain captures. `leaves ... continue` on non-reference opaque leaves (:1553, :1561): an opaque leaf that is not a reference gets no child capture. Open: whether other opaque kinds are rejected earlier by `contents::describe` (not read).
- `validate_byte_views` requires `original_storage.is_some()` (:460), and `prepare_original_storage` returns before setting it when `original == false` (:798-800). So a byte view captured in non-original mode cannot validate (inferred; `engine_calls.rs` not read to confirm that byte views only appear with `original == true`).
- Everything else in the file errors on mismatch: none seen as accept-on-unknown.

---

# File 2. `src/boundary_values.rs` (2391 lines, read in full)

## 2.1 Purpose
Header (:1-8): the vocabulary of values, results and errors that cross between an admitted Terminal Psi execution and its host: entry-result observation, the one error enum, the target memory-ordering and native-code-layout environments, result custody accounting, execution results and statuses, scalar aggregate values with their validation plan, and the runtime place identities used by every structural storage table.

## 2.2 Shape
- `entry_results` (:10-447)
  - `TerminalEntryResultRequest` (:18; `machine`, `results: &[(ValueId, ScalarType)]`), `EntryResults` (:23; `machine`, `rows: RuntimeRows<ValueId,(ScalarType, Option<TerminalScalarValue>)>`).
  - `impl TerminalExecution`: `request_scalar_entry_checks` (:50), `request_entry_results` (:85), `observe_completed_entry_result` (:125), `discharge_scalar_entry_checks` (:158), `require_scalar_entry_checks_complete` (:172), `require_entry_results_complete` (:183), `observed_entry_result` (:194).
  - Tests `entry_results::tests` (:221-446): 5 `#[test]`s (:282, :324, :361, :384, :400) by reading.
- `errors` (:449-765): `TerminalInterpretError` (:460-677), 83 variants (counted by reading, not by command), `TerminalArtifactInterpretError` (:682), `malformed()` (:696) which returns `VerifiedOperationMalformed`, `in_reference_formation` (:701), `in_argument_preparation` (:708), `From<WorkMeterError>` (:719), `Display` (:725).
- `memory_ordering` (:767-794): `TerminalMemoryOrderingEnvironment { target: RegisterTargetApplicability }`; `private_sequential` (:776), `accepts_target` (:786), `supports` (:790).
- `native_code_layouts` (:796-889): `TerminalNativeCodeRegion {base, length}` (:801), `TerminalNativeCodeLayout {machine, entry, regions}` (:807), `TerminalNativeCodeLayoutEnvironment {terminal_psi, target: String, native_artifact: [u8;32], layouts}` (:814); `linked_image` (:822), `accepts` (:869), `layouts` (:878), `layout` (:882, binary search by `MachineId`).
- `result_custody` (:891-1137): `ResultCustody` (:905; `cells`, `payload_bytes`, `byte_backings`, `allocations`, `residents`); methods `add_cells`, `rows`, `bytes`, `binding`, `element_view`, `reference`, `endpoint`, `provider`; `TerminalExecution::charge_completed_result` (:1061), `measure_result_storage` (:1097).
- `results` (:1139-1312): `TerminalExecutionResult {Unit, Scalar, Structural, ScalarCase, ScalarArray}` (:1153), `TerminalStructuralResult` (:1162), `TerminalScalarCaseResult` (:1168, an `Arc<ScalarCaseStorage>` with an optional storage lease) with `collect` (:1184); `meter_status` (:1248); `TerminalExecutionStatus` (6 variants, :1260); `TerminalEffectWait` (:1270); `TerminalExternalCompletion` (:1279); `TerminalCrash` (:1290); `TerminalCrashSite {Edge, Operation, BoundaryCall}` (:1298).
- `values` (:1314-2391)
  - `TerminalScalarAggregateValue {structural_type, result_case, fields: Vec<(path, TerminalScalarValue)>}` (:1321).
  - Inner `aggregates` module (:1330-2098): private `Element`, `Shape`, `Node`, `Visit`; `ScalarAggregatePlan { nodes, root }` (:1365), `ScalarAggregatePart {Field, Primitive, Array}` (:1370); `ScalarAggregatePlan::new` (:1394), `count` (:1525), `scalar_fields` (:1530), `storage_parts` (:1545), `walk` (:1552); `TerminalScalarAggregateValue::validate_against` (:1639); tests (:1693-2097, 8 `#[test]` by reading). Re-exported at :2099.
  - `TerminalStructuralBooleanFieldValue` (:2102), `TerminalStructuralPrimitiveValue` (:2110).
  - `StructuralRuntimePlace {opaque_identity: u64, path}` (:2116); `structural_storage_path` (:2121); `StructuralFieldNamespace {Member, CasePayload}` (:2131); `StructuralScalarRuntimeField` (:2137) with `member`, `case_payload`, `relocated`; `StructuralByteSequenceRuntimeField` (:2173) with `relocated`; `StructuralFieldNamespace::for_carrier` (:2190); `From<&TerminalStructuralValue> for StructuralRuntimePlace` (:2199); `scalar_view_value` (:2208).
  - `scalar_aggregate_tests` (:2224-2390), 3 tests (:2280, :2307, :2331).

## 2.3 Inputs and outputs
- `entry_results`: reads `TerminalExecution` fields `scalar_entry_checks`, `entry_results`, `entry_storage_open`, `current_machine`, `call_stack`, `program.verified_machine(..)`, `values`, `result`, `failure`; mutates `entry_results` and each row's `Option<TerminalScalarValue>`.
- `result_custody`: reads about 13 runtime row tables of the execution (:1102-1114) and `activation_storage`; produces a cell count and payload-byte count and charges `account.charge_result_custody(cells, payload_bytes)` (:1092-1094). No effect when `account` is `None` (:1065-1067).
- `ScalarAggregatePlan::new` takes a declaration and a lookup closure `StructuralTypeId -> Option<&StructuralTypeDeclaration>` and builds a flat node table.
- Errors: one enum for the whole terminal interpreter (83 variants).

## 2.4 How it decides
- Entry results: a result is observable only if some operation in some block of the machine declares a scalar result with that `ValueId` and carrier type (:105-115). The check scans every block and operation (linear). Identity is `ValueId` plus `ScalarType`; duplicates in a request error (:102-104). A request must name `self.current_machine` with an empty `call_stack` (:94-98).
- Observation (:125-156): silently returns `Ok(())` when the call stack is non-empty, when there are no observations, when the machine differs, or when the result id is not a requested row (:129-140). Errors when an occurrence completes twice (:141-143), the value is missing (:144-148), or the carrier changed (:149-153).
- `observed_entry_result` requires `self.result.is_some() && self.failure.is_none()` (:199-203) and `observations.machine == machine` (:207).
- `discharge_scalar_entry_checks` forwards to `checks.discharge_ready(...)` in `terminal_installation` (not read) (:165-169).
- `linked_image` (:822-867): rejects empty `target`, zero `native_artifact`, duplicate machine ids; each region must have `base != 0`, `length != 0`, no overflow, and `region.base > previous_end` (:848-851, `<=` rejects). Because `previous_end` is the exclusive end, two regions that touch (`base == previous end`) are rejected. `layout.entry != 0` and the entry must lie in some region (:854-858). Keyed on numeric addresses.
- `accepts` (:869-876): `code == TerminalExecutionIdentity::portable(self.terminal_psi) && self.target == target`, a string equality on the target name.
- `private_sequential(target: &str)` (:776-784): matches the target string against X86_64 then Aarch64 via `accepts_target_name`. An unknown target string returns `None`. `supports(kind)` compares `self.target == kind.target()` (:790-792).
- `ResultCustody::bytes` (:946-965): counts a byte backing's retained payload once, deduplicated by `shares_backing`. `provider` (:1027-1057): counts each live allocation and each live resident once, deduplicated by handle; non-live handles error with `malformed()` (:1033-1040, :1047-1051).
- `ScalarAggregatePlan::new` (:1394-1523): iterative DFS with an `active` stack for cycle detection and a dedup by `StructuralTypeId` (:1402). Rejects `quotient_carrier.is_some()` (:1406) and any revisit of an active type (:1407). Child types are looked up by id and must satisfy `declaration.id == identity` (:1414-1416). Count of leaves is checked/saturating; a root with unknown count (overflow) errors (:1519-1521); a zero-length array makes the count `Some(0)` even if the inner count overflows (:1493-1495; see test :2050-2096).
- `validate_against` (:1639-1690): the declaration id must equal `self.structural_type` (:1644). `result_case: None` accepts only `Record | FixedArray | PrimitiveScalar` declarations and validates by walking the plan (:1659-1671). `Some(case)` accepts only `Sum` declarations, finds the case by id with `find_structural_case`, and validates each field in declaration order (:1672-1683). Each leaf must match path, scalar type, and `value.satisfies_field(field_type)` (:1649-1658). Leaf count and trailing extras are checked (:1667, :1686-1688).
- `for_carrier(path)` (:2190): `CasePayload` iff the last path segment is `Case`, else `Member`. `structural_storage_path` (:2121) drops one trailing `Case` segment.

## 2.5 Cases by shape (fixed lists)
- `ScalarAggregatePlan::new` handles only `PrimitiveScalar`, `Record`, and `FixedArray` with `terminal_psi::ArrayExtent::Literal(length)`; `_ => return Err(malformed())` at :1445 (first pass) and :1509 (second pass). Record storage `AccessView(_) | Accessor(_)` is refused (:1423-1429). A record field that is neither a structural child nor a scalar errors (:1439-1441). Erased fields (`relevance.is_erased()`) are skipped in records (:1433, :1472).
- Asymmetry: in the `Sum` branch of `validate_against` an erased field or a non-scalar field is an error (:1675-1680), while in records the plan skips erased fields. Both read at those lines.
- Sum cases in `TerminalScalarCaseResult::collect` are validated with `lookup = |_| None` (:1226), so a case with a `Structural` field type fails (`scalar_type().is_none()` at :1677).
- `walk(group_scalar_arrays)` (:1552-1635): when grouping, any array node whose subtree is purely scalar (`scalar_tree`) is emitted as one `ScalarAggregatePart::Array` (:1566-1579); otherwise arrays are unrolled element by element (:1596-1627). A zero-count node is skipped, except a scalar-tree array when grouping (:1580-1585).
- `result_custody::binding` (:967-977): only `Immutable` and `Representation` bindings are counted; `_ => {}`. `endpoint` (:1008-1025): only `RepresentationByte`, `ViewByte`, `Projection`; `_ => {}`.
- `charge_completed_result` (:1061-1095): `Unit | Scalar` cost nothing beyond the base cell; `ScalarCase` counts `fields.len()`; `ScalarArray` counts `elements().len()`; `Structural` measures the 13 tables unless the result is a byte sequence or element view (:1078-1083).
- `scalar_view_value` (:2208-2222): `Boolean`, `Integer`, `IeeeFloat` (three scalar kinds, exhaustive match).

## 2.6 Same job elsewhere
- `ScalarAggregatePlan` is the shared plan for scalar aggregates. Users outside this file (Grep over `omega-rust`, 18 lines total in 3 files): `storage_records_and_arrays.rs` (:676 import, :720 `new`, :727-750 part matching, 2+ lines) and `boundary_effects.rs:2564`, `:2600`. So 3 construction sites outside this file (storage_records_and_arrays.rs:720, boundary_effects.rs:2564, :2600); the remaining 14 lines are in this file (definitions, tests, re-export).
- `structural_storage_path`: Grep `\bstructural_storage_path\(` shows this file's definition (:2121), 2 callers in this crate (`engine_structural.rs:633`, `engine_custody.rs:2183`), plus a different, same-named pub fn in `psi/compilation/validation/src/structural_call_custody.rs:3256` (callers at :3231, :3241 and `execution_terminal_unit_control.rs:1148`). Same name, different signature (takes `program`, `reference`, `segments`). I did not read the other.
- `StructuralFieldNamespace::for_carrier` (:2190): 1 caller (`engine_structural.rs:659`). The same "last segment is Case" test is written inline in `boundary_storage_inputs.rs:211-218` (`leaf.path.iter().rev().nth(1)` is `Case`, which is the second-to-last, because the leaf path ends in the field). Same idea, different position, written separately.
- `StructuralScalarRuntimeField::relocated` (:2163) and `StructuralByteSequenceRuntimeField::relocated` (:2180) are identical bodies on two structs with identical fields (`parent`, `namespace`, `field`).
- `meter_status` is used by `engine_terminators.rs` (8 sites: :2029, :2160, :2286, :2432, :2465, :2492, :2546, :2594) and `engine_execution.rs:2661`.
- Plan counting and storage-part grouping both exist in `ScalarAggregatePlan` (:1525, :1545); `walk` is a hand-written explicit stack machine (`Visit`).

## 2.7 Unused or one-caller code (grepped)
- `scalar_view_value`: 2 callers (`storage_primitives.rs:4624`, `storage_bytes_and_views.rs:5363`).
- `for_carrier`: 1 caller.
- `charge_completed_result`: 1 caller (`engine_terminators.rs:1301`).
- `require_entry_results_complete` and `require_scalar_entry_checks_complete`: 1 caller each (`engine_terminators.rs:1299-1300`).
- `observe_completed_entry_result`: 2 callers (`engine_terminators.rs:1407`, `engine_execution.rs:2892`).
- `request_scalar_entry_checks`: 1 caller (`embedding.rs:2792`).
- `observed_entry_result` (on `TerminalExecution`): called from `embedding.rs:1706`. `embedding.rs:1698` defines another method of the same name on the public instance, with callers in `compiler/tests/terminal_entry_writeback.rs` (:113, :134, :138) and `build-time-evaluation/src/scalar_applications.rs:462`.
- `TerminalMemoryOrderingEnvironment` / `private_sequential`: 16 lines in 10 files (Grep count), used by terminal-runtime, the CLI (`application/cli/src/execution/compilation.rs`), `engine_execution.rs`, and test support.
- `TerminalNativeCodeLayoutEnvironment` / `linked_image(`: 11 lines in 5 files; `omega/interpreter/terminal-runtime/src/execution/native_code_layouts.rs` has 4. `TerminalNativeCodeRegion|TerminalNativeCodeLayout`: 11 lines in 3 files (this file 7, `lib.rs` 1, `native_code_layouts.rs` 3).
- `TerminalStructuralBooleanFieldValue` / `TerminalStructuralPrimitiveValue`: 18 lines in 6 files (lib.rs 2, engine_execution.rs 4, engine_custody.rs 2, boundary_inputs.rs 6, this file 2, a test support file 2).
- Error variants spot-check (Grep `-o` over `omega-rust`): `ScalarEntryQualificationUnsupported`, `AffineProjectionNotRepresentable`, `UnclosedArrayExtent`, `ContentTransformationMismatch`, `ZeroInitializedArgumentInvalid`, `StructuralBooleanFieldMissing`, `StructuralBooleanFieldArgumentInvalid`, `CompletionReceiptMismatch`, `StructuralPrimitiveValueInvalid`, `StructuralScalarFieldArgumentInvalid`, `UnsuppliedErasedEntryInputs`: each has at least one construction or match site outside this file. `VerifiedOperationMalformed`: 3 lines (`malformed()` here, a second at :2 in this file, and `engine_execution.rs` 1). I did not check the other 70 variants.
- No comment in this file calls a type "the single / shared / canonical" way. `ScalarAggregatePlan` is called "the existing array storage owner" only inside a test panic message (:1993), not a claim about itself.
- `TerminalScalarAggregateValue::validate_against`: callers `TerminalScalarCaseResult::collect` (:1226) and this file's tests; not grepped outside.

## 2.8 Stated limits
- "native layout supply has invalid completed-image metadata" (:830), returned for all `linked_image` rejections.
- Test names state intended limits: `zero_array_dimensions_do_not_hide_missing_cyclic_or_unsupported_elements` (:2003), `outer_zero_suppresses_inner_count_overflow_without_skipping_schema_validation` (:2050).
- `TerminalNativeCodeLayoutEnvironment::private_sequential` has no limit comment; unknown targets return `None` (:776-784).
- No "unsupported"/"migration debt" comments besides the strings above.

## 2.9 Fail-open risks
- `observe_completed_entry_result` returns `Ok(())` for: non-empty call stack, no observation set, machine mismatch, unrequested result (:129-140). Traced: an unrequested result is not observed, and `require_entry_results_complete` (:183-192) errors if any requested row stayed `None`. A requested result that was never completed in the root activation cannot pass (test :384-398). Recursive activations are ignored on purpose (test :324).
- `require_scalar_entry_checks_complete` returns `Ok(())` if `scalar_entry_checks` is `None` (:175-181). That is the case when no checks were requested. Wrong program that gets through: none seen, because checks exist only if the installation admitted them (not traced past `request_scalar_entry_checks`, `embedding.rs:2792`). "open" for the admission side.
- `charge_completed_result` returns `Ok(())` with no charge when `account` is `None` (:1065-1067). That is an accounting switch, not a program-acceptance path.
- Result custody under-count possibility: the list of row tables at :1102-1114 is fixed, and `binding()`/`endpoint()` ignore other variants (`_ => {}`, :975, :1022). A runtime row table added later but not listed would not be charged (inferred; I did not check the execution struct for tables absent from the list). "open".
- `validate_against` Sum branch ignores `lookup` (:1672-1683) and rejects non-scalar fields, so it cannot accept a structural field it cannot check. Not fail-open.
- `ScalarAggregatePlan::new`: `nodes.iter().any(|node| node.identity == declaration.id) { continue }` (:1402-1404) skips an id that is already finished without comparing the two declarations. Two different declarations with the same id (from a bad lookup) are treated as one. The lookup filter `declaration.id == identity` (:1414-1416) only checks the id matches the requested one, so two lookups returning different declarations for one id is not possible when the lookup is a map by id. For the root (`declaration` passed in directly), a lookup that returns a different declaration for the root's id is not detected. "open" (all callers' lookups not traced; `storage_records_and_arrays.rs:720` and `boundary_effects.rs:2564,2600` not read in detail).
- `linked_image`: none seen (all failures are errors). Accepting adjacent regions is not possible (rejected at :850).
- `private_sequential` and `accepts_target` are keyed on a target-name string; an unrecognised target is `None`, not accepted.
- `memory_ordering::supports` compares the architecture only (`self.target == kind.target()`, :790-792). Any fence kind for the same architecture is supported. No check on x86 versus ARM fence strength beyond `kind.target()`. Not a fail-open as far as I can tell; the semantics of `MemoryFenceKind::target` are not read.

---

## Coverage
- Read in full: `boundary_storage_inputs.rs` 1-2703 (2703 lines); `boundary_values.rs` 1-2391 (2391 lines). Total 5094 lines.
- Skimmed: none.
- Not read: none (0 lines).

## Five things a maintainer most needs to know about this unit
1. A host call's storage lives in one `BoundaryStorageInputs` (`boundary_storage_inputs.rs:2375`), and a buffer is tied to its binding only by `Arc::ptr_eq` on an identity token plus `argument_index` (:30-31, :457-458, :520-522, :1688-1689). `validate_writeback` (:2613) runs every check before `commit` (:2684) mutates anything; commit only publishes already-staged stores and replacements.
2. "Original storage" mode (`original_residence`, :592-1626) is a second, separate route: the host gets a loan on the real backing and its writes are detected afterwards (`direct_original`, :1072-1078). `prepare_original_storage` returns at once when `original` is false (:798). Byte views only validate if `original_storage` is set (:460), so they depend on that mode (inferred).
3. Case replacement in a completed structural buffer is supported only for scalar, case, empty-array and byte-array leaves. New opaque children or new byte fields are refused with three `UnsupportedSemanticVariant` messages (:192, :202, :210).
4. `ScalarAggregatePlan` (`boundary_values.rs:1365`) is the shared leaf-order contract for scalar aggregates. It has 3 construction sites outside its file (`storage_records_and_arrays.rs:720`, `boundary_effects.rs:2564`, `:2600`). It rejects non-literal array extents, quotient carriers, cycles, access-view/accessor records, and any shape other than primitive, record, fixed array. Erased record fields are skipped in the plan but rejected in a sum case (:1433 versus :1675-1680).
5. `TerminalInterpretError` (`boundary_values.rs:460`) has 83 variants (counted by reading). Most structural checks in the storage file return `malformed()` (`VerifiedOperationMalformed`, no detail); 130 lines in `boundary_storage_inputs.rs` mention `malformed`, versus 34 `UnsupportedSemanticVariant(` lines that carry a message. A rejection therefore often has no reason text.

## What I am unsure about
- What `contents::describe`, `contents::capture`, `execution.record_original_scalar_write`, `validate_scalar_loan_restoration`, `capture_reference`, `resolve_live_projection` and `supports_captured_reference_result` check. They are in other files; I only know their call sites here.
- Whether byte views can be captured without original mode and then always fail in `validate_byte_views` (:460). The caller (`engine_calls.rs`, `boundary_effects.rs:1098`) was not read.
- Whether every runtime row table on `TerminalExecution` is in the custody list at `boundary_values.rs:1102-1114`.
- Whether the `lookup` closures passed to `ScalarAggregatePlan::new` at the three outside sites can return a different declaration for the root's id.
- The meaning of `access_covers` (`crate::storage::reference`) and `case_projection_with_membership_is_active`; I treated them as black boxes.
- Variant counts (83) and test counts come from my reading, not from a command.
