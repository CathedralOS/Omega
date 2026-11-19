# Unit 23 notes

Files covered (all under `omega-rust/omega/compilation/realization/`):
provider-admission/src/provider_admission/terminal_authority_review/{windows_handle_contracts, windows_handle_inputs, windows_handle_lifetimes, windows_handle_origins, windows_handle_paths, windows_handle_replay, windows_handle_replay/execution, windows_handle_scalar_table, windows_handle_storage, windows_handle_values}.rs; provider-admission/src/provider_installation.rs and provider_installation/replay.rs; provider-planning/src/{calling_policy_plans.rs + 13 submodules, data_layout_queries.rs + builder/* + callback_layout/layout_plan/packing, dispatch_tables, evaluated_via_bindings(+2), intrinsic_declarations, intrinsic_requirements, lib, native_bindings(+plans,+tests), native_provider_sources, program_entry}.rs.


Paths below are relative to `omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/terminal_authority_review/` unless stated.

## windows_handle_contracts.rs (628 lines, read in full)

1. Purpose. Holds the fixed list of Windows kernel32/ntdll APIs whose handle behaviour the handle proof understands (CreateFileW, DuplicateHandle, CloseHandle, GetCurrentProcess, NtQueryInformationFile, GetFinalPathNameByHandleW, ReadFile, WriteFile) plus a "no handle intervention" list and ExitProcess. Given a boundary, it checks the declared operand shapes and the native call plan against the reviewed signature and returns the contract.
2. Shape. `enum WindowsHandleContract` :20 (9 variants, NoHandleIntervention..WriteFile); `enum Formal` :66 (Integer, IntegerOutput, WideArray, ByteView, IoStatus); `struct ReviewedApi` :74; `fn reviewed_apis()` :84 (OnceLock table of 8 rows, :92-186); `fn no_handle_intervention(locator)` :35 (7 locators, :39-47); `reviewed_call` :216; `reviewed_process_exit` :263; `process_exit_physical_matches` :320; `process_exit_signature` :346; `original_windows_import` :375; `matches_declaration` :418; `structural_matches` :463; `expected_call` :506. One test, :590.
3. Inputs/outputs. Reads `ReviewContext` (foreign binding, mechanism, selected requirement, module boundary_machine/structural_type). Returns `Option<ReviewedHandleCall{contract, declaration}>` or `Err(String)`; no mutation.
4. How it decides. Keyed on the normalized foreign locator identity digest (`target::normalize_foreign_locator(PeByName{library, export})`), compared with `==` at :236 and via `contains` :62; library/export are byte-literals at :40-47, :94-185. Not keyed on source method names (module comment :4-5). Operand check is by integer sign/bits (:413), structural shape (PrimitiveScalar / FixedArray of u16 / ByteSequence BorrowedView / packed 2-field record, :463-504) and access mode. The call plan must equal `expected_call` (:247), computed from Microsoft x64 policy; ReadFile/WriteFile special-case the plan so a 16-byte byte-view covers pointer+count (:523-545).
5. Cases by shape. The model "is deliberately bounded to native calls in this customer's closure. Unsupported handle operations remain unknown" (:81-83). `matches_declaration` requires result `Formal::Integer`, completion NormalReturn and empty ensures/qualifications/guarantees/crash routes (:423-433). `process_exit_signature` (:346-365) accepts exactly one signed 32-bit scalar param, unit result, ProcessExit completion.
6. Same job elsewhere. `integer_matches` (:413) style checks. Suspected: `boundary_contract_matches_selected_schema` is used at :302 from `super::super::compiler_builtins` (not read).
7. Unused / one-caller. `reviewed_call`: callers windows_handle_inputs.rs:151, windows_handle_values.rs (import :27), windows_handle_replay.rs:2649 (grep of `windows_handle_contracts::` over omega). `reviewed_process_exit` one caller (inputs.rs:139).
8. Stated limits. "Unsupported handle operations remain unknown, never preserving by default" :82. Runtime "still owes authority for this exact containing process domain" :261.
9. Fail-open. `reviewed_call` returns `Ok(None)` for: no foreign binding, non-Import mechanism, non-Windows target (:379-387), unknown locator (:234-239). `None` means "not reviewed"; the consumer (inputs.rs:151-158) puts it in `opaque_calls`, i.e. treated as an intervening unknown call, so it is fail-closed there (traced). `no_handle_intervention` list (:39-47) returns a contract that lets calls through with only storage clobbering (see replay.rs:976-990): stated at :21-23 ("The flow reader still checks argument storage and rejects handle values escaping"). A reviewed API with a wrong shape returns Err not None (:240-245).

## windows_handle_inputs.rs (288 lines, read in full)

1. Purpose. Prepares, once, all inputs the handle replay needs: the selected call graph, per-call classification (native reviewed call, opaque call, process exit, intrinsic), borrowed-storage graph, scalar origins and per-machine prepared rosters. Also an opt-in timing/memory progress printer.
2. Shape. `HandleProofProgress` :29 (`new` reads env `OMEGA_WINDOWS_HANDLE_PROFILE`=="1" :36; `note` :42; `replay` :53; `memory` :63 reads /proc/self/status on Linux). `NativeHandleInput{call, reviewed, scalars: BTreeMap<usize,ScalarSubject>}` :76. `PreparedHandleMachine{storage_index, expressions, operands, resident_transfers}` :87. `WindowsHandleInputs` :94 with fields calls, storage, natives, opaque_calls, external_calls, intrinsic_calls, origins, machines (IdTable), close_sites, duplicate_sites. `WindowsHandleInputs::prepare` :112.
3. Inputs/outputs. Reads `ReviewContext`; builds `HandleCallGraph`, `terminal_borrowed_storage_graph_from`, `HandleValueReader`, `HandleOrigins::derive`. Produces the struct. Uses `IdTable::from_rows_with_gaps(machines, 65_536)` :269.
4. How it decides. Per call target (:130-215): Machine -> selected body pairs; Boundary -> (a) not normal_return and `reviewed_process_exit` => `external_calls`; (b) `reviewed_call` None => `opaque_calls`; (c) reviewed => requires normal_return and `Explicit` arguments, maps declared scalar parameter ordinals to SSA values; Intrinsic => `intrinsic_calls`. Close and duplicate sites recorded by contract (:188-193). Keyed on `Caller` identity and reviewed contract enum.
5. Cases by shape. Duplicate call of the same caller with different bodies errors (:219-226). "extra scalar arguments outside its admitted declaration" error :185.
6. Same job elsewhere. none seen in unit.
7. Unused. `HandleProofProgress::replay` used by execution.rs (5 grep hits in that file for these names; not individually counted).
8. Stated limits. Progress "never supplies a value to the proof or its limits" :28. Fixed budget 65_536 at :269 (and many other places).
9. Fail-open. Unknown boundary calls are kept as `opaque_calls` (intervening) :151-158, not dropped (stated :98-99 "Unknown native executions remain intervening calls"). Process exit is only recognised when `!call.normal_return` and the receiver declaration is ProcessExit external (:138-150). None seen otherwise.

## windows_handle_lifetimes.rs (853 lines, read in full)

1. Purpose. A per-path ledger of Windows file-handle resources: which integer copies ("instances") alias which file object, which were released, used, escaped, or attached to delete-on-close; plus the transitions (acquire, duplicate, preserve, synchronous_io, observe, release, return, caller continuation).
2. Shape. `NativeOccurrence{machine, site, context:[u8;32], boundary}` :17; `ScalarOccurrence` :28; `ScalarUseSite{Operation,Edge}` :35; `HandleUseOccurrence` :41; `HandleInstance(usize)` :54; private `Handle{object, origin, release, uses}` :69; `FileObjectState{acquisition_flags, attached_deletion, escaped}` :77; `HandleLifetimeFailure` (9 variants) :87; `HandleLifetimes{handles: BTreeMap, objects: Vec}` :122 with `compare_resource_state` :131, `merge_use_history` :165, `next_instance` :189, `acquire_file` :208, `duplicate_file` :239, `preserve` :271, `synchronous_io` :293, `observe_scalar` (cfg(test)) :319, `observe_scalars` :337, `escape` (cfg(test)) :360, `attach_deletion` (cfg(test)) :372, `release_file` :387, `returning_path` :410, `caller_continuation` :454, `require_occurrence` :518, `require_live` :531.
3. Inputs/outputs. Pure data structure; each method mutates the ledger or returns a new one; errors are `HandleLifetimeFailure`.
4. How it decides. Keyed on instance index and occurrence identity plus a 32-byte context digest. `require_occurrence` rejects an all-zero context or a site without an operation (:518-529). Acquisition flags: bit 0x0400_0000 (FILE_FLAG_DELETE_ON_CLOSE) sets attached_deletion :221; bit 0x4000_0000 (FILE_FLAG_OVERLAPPED) makes synchronous_io fail :299. Return requires every non-retained instance to be released (:430-434). Release refuses when the shared object has attached deletion (:394). Join: two ledgers merge only if `compare_resource_state` is Equal, i.e. same handles/objects/origins/release; only `uses` are unioned (:131-187).
5. Cases by shape. Instance numbers must be fresh (`RepeatedAcquisition`). `caller_continuation` drops callee-private instances not in outputs (:493-495).
6. Same job elsewhere. none seen.
7. Unused. `observe_scalar`, `escape`, `attach_deletion` are `#[cfg(test)]` only (:318, :359, :371): no production caller (grep of `attach_deletion(`/`escape(`  not separately counted; cfg(test) read directly). Consequence: production never sets `escaped` or `attached_deletion` after acquisition; `attached_deletion` is set only from the acquire flags (:221), and `escaped` is read (:424, :546) but only written in test code.
8. Stated limits. "This states nothing about console or device kind" :290; "does not assert resource reclamation after a failed close" :385.
9. Fail-open. `FileObjectState.escaped` has no production writer (see 7), so `EscapedObject` is unreachable in production ("traced" within this file; callers outside this file in windows_handle_replay.rs do not call `escape`; grepped in the replay file by reading). None else seen: unknown instance => `UnknownHandle` error.

## windows_handle_origins.rs (513 lines, read in full)

1. Purpose. Builds finite "origin equations": for each scalar the handle proof cares about, which other scalars/storage cells/call results/native outputs it can come from, following calls and returns as graph edges.
2. Shape. `ScalarSubject` (Value, Storage, Expression, CallResult, NativeOutput) :22; `OriginEdgeRole` (cfg(test)) :51; `OriginEdge{source}` :68; `OriginKind` (Alternatives, Expression, BoundaryReturn, NativeOutput, IntrinsicReturn, ExternalInput, OpaqueObservation, InactiveCase) :75; `OriginEquation` :94; `HandleOrigins{equations: BTreeMap}` :99 with `derive` :106; `definition_equation` :160; `caller_position` :322; `result_equation` :330; `expression_equation` :410.
3. Inputs/outputs. Reads `TerminalModule`, `HandleCallGraph`, `HandleValueReader`; a worklist from the roots; outputs equations map.
4. How it decides. Closure limited to 65_536 equations (:118). A parameter with no callers becomes `ExternalInput` (:183-188 and similar). Intrinsic results with a single alternative and no projection take their scalar operands as sources (:374-395); boundary results with one alternative and no projection are `BoundaryReturn` (:368-373). Expression terms: Value / field reads / structural reads with static path map to a storage location; reads with a non-static projection segment become `OpaqueObservation` (:453-457); literals have no sources.
5. Cases by shape. `result_equation` `_ =>` arm errors: "handle result mixes a physical result with a body or structural projection" :396-400. Intrinsic with structural operands errors :380-382.
6. Same job elsewhere. none seen.
7. Unused. `OriginEdgeRole` tests-only by design (:47-50).
8. Stated limits. "the graph does not erase a call/return pair, select one incoming source" :3-5; "No consumer may interpret their absent edges as empty physical reach" :87.
9. Fail-open. `ExternalInput`/`OpaqueObservation` carry no sources; comment :87 says consumers must treat as unknown (stated). Whether consumers do so was not traced here (open).

## windows_handle_paths.rs (256 lines, read in full)

1. Purpose. A graph replay of "path programs" that checks every incoming ledger through acquire/release nodes and returns, to prove each required release is covered on every returning path.
2. Shape. `HandleTransition{Acquire,Release}` :14; `HandleReturn` :26; `HandlePathNode` :34; `HandlePathProgram{entry,nodes,required_releases}` :40 with `replay` :66; `HandlePathFailure` :49; `HandlePathProof` :59. One test :206.
3. Inputs/outputs. Input `HandleLifetimes`; BFS over (node, ledger) states, budget 65_536 :140; output proof or failure.
4. How it decides. Structural validation (:70-103) then fixpoint; requires >=1 returning path, all nodes visited, `required_releases` subset of releases seen.
5. Cases by shape. n/a.
6. Same job elsewhere. The main replay (windows_handle_replay/execution.rs, not yet read) does a similar exploration; suspected.
7. Unused. The module is declared `#[cfg(test)] mod windows_handle_paths;` (terminal_authority_review.rs:37-38). grep over omega-rust for `HandlePathProgram|HandlePathFailure|HandlePathNode|HandlePathProof` finds only this file (all hits in windows_handle_paths.rs). So no production user.
8. Stated limits. none.
9. Fail-open. none seen (every structural anomaly returns InvalidProgram).

## windows_handle_scalar_table.rs (179 lines, read in full)

1. Purpose. A copy-on-write paged table (64 values per page, `Arc<Vec<T>>`) of per-ValueId slots so that cloning replay states shares unchanged pages.
2. Shape. `HandleScalarTable<T>{namespace: Arc<IdTable<ValueId,()>>, pages}` :16; `from_table` :53, `get` :37, `get_mut` :67 (Arc::make_mut), `iter` :44, `map` cfg(test) :73; `Ord` compares namespace then pages, skipping pointer-equal pages :81-97. PAGE_LEN=64 :13.
3. Inputs/outputs. Built from an `IdTable<ValueId,T>`.
4. How it decides. Position = value id minus the first key (:31-35).
5. Cases by shape. none.
6. Same job elsewhere. none seen.
7. Unused. One production user: replay.rs (`HandleScalarTable::from_table` :2268, field :323). grep count 10 hits in this file, 4 in replay.rs, 5 in execution.rs (combined pattern count, not per-name).
8. Stated limits. none.
9. Fail-open. `get` returns None for an id outside the namespace; callers (replay.rs:2292-2296, :2316) turn None into errors. none seen.

## windows_handle_replay.rs (2825 lines, read in full)

1. Purpose. Replays the program's actual Windows handle activity (calls, returns, operations) as a symbolic state (`CanonicalHandleState`) to prove that every CreateFileW/DuplicateHandle result is closed exactly once on every returning path, and then mints narrowed "checked" mechanism identities for close/duplicate call sites so the policy treats them as having empty authority.
2. Shape. `HandleReplayActivations` :55 (rows with a digest-encoded `source` context per activation; `enter` :82, `native` :132, `scalar` :213); `KnownHandleScalar` :250; `CapturedHandleScalar{word, known}` :287; `CapturedMemory` :299; `CanonicalHandleState{ledger, aliases, references, cases, constants, stored_constants, bindings}` :318 with methods `capture_arrival` :418, `install_arrival` :566, `require_active_payload` :609, `native` :638, `clobber_native` :1006, `capture_return` :1014, `resume_after_cleanup` :1120, `output_census` :1229, `enter_call` :1371, `operation` :1559, `enter` :2253, `scalar` :2274, `define` :2303, `evaluated_operands` :2330, `equation` :2362, `capture` :2450, `replace` :2473, `install` :2481, `observe` :2510, `store_scalar` :2523, `read_scalar` :2560; frame types `HandleReplayFrame` :362, `HandleCensusRoot` :372; `CheckedHandleLeaf` :2600; `CheckedHandleCalls` :2606; `checked_calls` :2628 (entry point).
3. Inputs/outputs. Inputs: `ReviewContext`, `WindowsHandleInputs`, retained artifact digest. Output: `Option<CheckedHandleCalls>` with per-boundary leaves (mechanism with `with_checked_argument_contract(identity)`) and `TerminalAuthorityPolicyRow::new(mechanism, from_classes([]))` rows (:2799-2802): an empty-class disposition for the narrowed mechanism.
4. How it decides. `checked_calls` returns `Ok(None)` unless target is WindowsX64 with a permission policy (:2638-2641), unless the trigger's original mechanism is NOT already classified by the physical policy (:2646), and unless the trigger is a reviewed CloseHandle or DuplicateHandle (:2649-2657). It then runs `execution::derive`. Narrowed rows are minted only for close/duplicate occurrences whose original boundary is not classified (:2748-2753). Native-call semantics are a `match (contract, arguments)` (:827-996): CreateFile yields a refused state (result -1) plus one acquired state per checked flags value; DuplicateHandle yields refused (0) + duplicate-with-output-word state; Close releases; QueryFileMode/QueryFinalPath/ReadFile/WriteFile preserve and clobber output ranges; NoHandleIntervention/CurrentProcess clobber every possibly-written range. Scalar compare logic (:2362-2448): Nonzero vs 0 => false; exact file handle vs 0 or -1 => false; two instances equal => true.
5. Cases by shape. Raw address/device operation kinds are refused: StorageAddress, EstablishAccessReference, ActivateTaskRegister, ReadTaskRegister, ReloadSegmentDescriptors, ReadSegmentSelector, GlobalDescriptorTable, PortWrite, RegisterAccess, AtomicAccess, AtomicFence :1594-1612 ("no preserving contract for this raw address or device operation"). Operations with `step.may_escape` or unresolved contents writes error :1617. Transfer (ReadFile/WriteFile) requires a whole byte window <= u32::MAX bytes :958-965. Path query requires capacity <= buffer bytes/2 :935-941. Handle-bearing scalars are allowed only in role 1 (DuplicateHandle) or 0 (Close/QueryFileMode/QueryFinalPath/ReadFile/WriteFile) :706-723.
6. Same job elsewhere. Alias/ledger bookkeeping parallels `HandleLifetimes` (lifetimes.rs) and storage geometry (storage.rs, not yet read).
7. Unused / one caller. `checked_calls` has one caller: reviewer.rs:237 (grep). `CheckedHandleCalls::for_boundary` and `physical_rows` are `pub(super)`; `checked_handles` field at reviewer.rs:39.
8. Stated limits. Premise list at :2690-2704, e.g. "closed-process premise excludes external code/handle-table injection"; "Ordinary concurrent namespace deletion remains allowed"; process exit "has no normal continuation and supplies no ordinary-release receipt" :2702-2703. Digest tags embed the premise text (:2708).
9. Fail-open. (a) `checked_calls` `Ok(None)` paths simply leave the original mechanism unclassified (traced via reviewer.rs:237 use). (b) Hard-coded semantic premises (e.g. valid file handle is never 0 or -1, :2396-2401, "stated") make `equation` return Boolean(false) for a comparison between an acquired handle and 0/-1. (c) `KnownHandleScalar::Unknown.has_type` returns true for any type :274-276 (stated by design: no value known). (d) `output_census` for sequences whose element has no pointer slots skips the content walk (:1336-1338). No `None -> accept` seen beyond these.

## windows_handle_replay/execution.rs (1414 lines, read in full)

1. Purpose. The all-path driver of the handle proof: a worklist over (activation, control position, suspended caller frames, symbolic state) that follows original control edges, runs cleanup rosters, enters/returns from calls, and records which native occurrences were reached with which reviewed contract.
2. Shape. `Control` :21 (Position | Cleanup{position, edge, ordinal, completion}); `Completion{Arrival,Return}` :15; `SuspendedCall` :40; `Work` :46; `ResourceWork(Arc<Work>)` :140 with hand-written `Ord` :142 (the dedupe key); `compare_resources` :57; `join_numbers` :187; `Pending{rows: BTreeMap<ResourceWork,..>, queue: VecDeque}` :216 with `push` :222; `CheckedHandleExecution{occurrences, external_completions}` :263; `Execution` :268 with `call` :300, `intrinsic` :451, `process_exit` :469, `cleanup` :542, `step` :661, `successors` :794; `CanonicalHandleState::observe_process_exit_status` :905; `derive` :925 (entry point).
3. Inputs/outputs. Reads `WindowsHandleInputs`, `ReviewContext`, prepared arguments (`windows_handle_arguments::prepare_handle_arguments` :964). Returns the map occurrence->contract and the set of external completions.
4. How it decides. States are deduplicated by an exact resource key: ledger resource state, aliases, references, cases, "is constant evaluated" bit, bindings (:57-96); numeric constants are widened to Unknown at a join (:187-213) and past-use sets are unioned (:227-246). Conditionals: a branch edge is dropped only when the saved Boolean is known to be the opposite (:816-818); StructuralCase keeps all cases unless a current tag is known (:855). Process exit needs the terminator to be ExternalCompletion with kind ProcessExit, one i32 scalar arg, no structural args (:507-520). Crash terminator and entry return both require the ledger to be fully released (:775-789, :648-653). Entry result must carry no tracked handle bits and no structural output (:635-647).
5. Cases by shape. `successors` supports only Conditional, Jump, StructuralCase; anything else errors "no supported original successor form" :861. Call depth cap 256 :313; distinct states cap 65_536 :252; total steps cap 1_048_576 :987. Entry must have no scalar or structural parameters :937-939. Intrinsics: only `HostedExitProcessI32` with no normal return :459-461. Cleanup actions: only `InvokeNominal` is executed as a call; "Plain value disposal retires no external HANDLE" :580-584 (other actions just advance).
6. Same job elsewhere. The BFS-with-ledger-set in `windows_handle_paths.rs:104-147` is a test-only version of the same idea (both bounded by 65_536 states). Suspected: other worklist replays for effects exist in provider-admission (not read).
7. Unused. `CheckedHandleExecution` fields both read at replay.rs:2710, :2730. `derive` has one caller (replay.rs:2686).
8. Stated limits. Process exit "remains conditional on the runtime's independently delegated containing-domain authority; this replay grants none" :470-473. "Numeric snapshots may lose precision at a repeated cut" :3-4.
9. Fail-open. (a) Process exit path increments `completed` without releasing handles (:534-538, stated: "Do not close, erase or mark abandoned handles as released"); a path ending in ExitProcess is allowed to hold live handles. Traced: conditional on the runtime delegation (stated :470-473). (b) `execution.completed == 0` errors :1002. (c) The final reachability check covers close/duplicate candidates only (:1010-1017); native calls with other contracts (e.g. ReadFile) that are unreachable do not fail. none else seen.

## windows_handle_storage.rs (2115 lines, read in full)

1. Purpose. Resolves a descriptor location (place + path of field/case/index/referent segments) to the concrete byte interval and backing it denotes, following captured references and borrowed-formal bindings, and holds the reference/sequence capture tables the replay copies around.
2. Shape. `CapturedHandleReference{referent, range, stored_scalar, dependencies}` :31; `CapturedHandleSequence{range, element, stride, length, precise, dependencies}` :43 (`from_complete_array` :56, `element_range` :88, `byte_scalar_selection` :106, `byte_selection_range` :130, `subslice` :140); `HandleScalarStorage` :183; `HandleScalarSelection{Exact,Family}` :190; `HandlePointerKind/HandlePointerSlot` :202-210; `CapturedHandleStorage` :213; `HandleStorageBindings{rows: Arc<IdTable<PlaceId,Option<BorrowedStorageSlot>>>}` :231 (`new` :236, `bind` :266, `get` :288); `HandleReferences{values,sequences}` :322 (`retire` :360, `capture` :365, `install` :387); `HandleStorageGeometry` :419 with `case_tag` :515, `inline_byte_carrier` :558, `primitive_scalar` :615, `pointer_slots` :648, `scalar_selection` :736, `element_stride` :813, `as_elements` :836, `present_sequence` :873, `sequence` :913, `location` :984, `carrier` :1015, `matches_carrier` :1058, `matches_resident_copy` :1067, `byte_write` :1094, `possible_write` :1111, `new` :1142, `region` :1166, `resolve` :1204, `scalar` :1358, `capture_reference` :1382. Free fns `unchanged_storage_presentation` :433 and `shared_leaf_copy_presentation` :454.
3. Inputs/outputs. Reads `TerminalModule` structural types, `DescriptorMachine` place types, `ReferenceStorageGeometry` (terminal_semantics) with target carrier layout and `layout::TerminalRecordStorage`; profile must be WindowsX64 (:1153). Outputs `(HandleStorageRange, ReferenceStorageRegion, Option<ScalarType>, BTreeSet<HandleInstance>)` tuples.
4. How it decides. Keyed on typed identity: structural type ids, descriptor path segments, place ids, byte offsets and lengths, not names. `resolve` (:1204-1356): path depth <=256; last Referent segment => look up captured reference; otherwise borrowed binding for the root; otherwise an activation-owned backing. Unknown selectors keep a containing window ("Family") not a guessed element (:38-41, :767-770). Type-presentation changes are admitted only if `borrowed_referent_presentation == Unchanged` and access is not Owned (:439-447), or via `shared_leaf_copy_presentation` (:454-510) which requires the original operation to be `StructuralLeafCopy`/`StructuralCaseLeafCopy`, no write-only root access, no claims/qualifications on the result and unrestricted or copyable-affine multiplicity.
5. Cases by shape. `byte_scalar_selection` accepts only `u8` (:115). `from_complete_array` refuses a window or scalar selection (:68-77). `present_sequence` handles ByteSequence (BorrowedView/BoundedView; owned byte storage refused :902) and ElementView only (:884-909). `sequence` handles FixedArray, ByteSequence, ElementView and inline byte fields; anything else errors :980. `pointer_slots` maps Reference, ElementView, ByteSequence/None; other shapes error :719-723.
6. Same job elsewhere. The `ReferenceStorageGeometry::new(...).with_target_carriers(...).with_record_storage(...)` chain is built three times in this file (:524-534, :815-825, :1173-1183). Suspected: provider-planning/data_layout_queries computes layouts for another consumer (see below).
7. Unused. Methods all have callers in replay.rs (read). Not individually counted.
8. Stated limits. "Constructing a pointer to inline backing needs its own producer" :431; "Unknown selectors retain a containing window, never a guessed first byte" :39.
9. Fail-open. `bindings.get` returns `Ok(None)` for a place that is not a borrowed formal (:289-291), then `resolve` treats the place as an activation-owned home (:1346-1355); a borrowed formal with no captured actual errors (:292-294). `scalar_selection` with an unknown runtime index returns a `Family` over the whole collection (:766-771) rather than refusing; consumers treat it as dependency-only (replay.rs:1986-2005). None that accepts without a trace.

## windows_handle_values.rs (920 lines, read in full)

1. Purpose. Reads, for a scalar SSA value or storage location, what defines it: a parameter, an expression, entry storage, borrowed entry, a stored-source set, block arrivals, a call result or an inactive case. Also precomputes per-machine expression/availability/dominator data and the native-output writer table.
2. Shape. `LocalScalarExpressions` :29 (`available_at` :43 uses dominators and producer position); `ScalarDefinition` :64; `ScalarArrival` :81; `ScalarReturn` :86; `StoredScalarSource` :97; `HandleValueReader` :127 with `new` :314, `local_expressions` :158, `read` :495, `scalar_actual` :571, `storage_actual(_location)` :592/:608, `require_machine_call` :640, `returns` :684, `read_storage` :732, `require_native_output` :252, `value_position` :282, `into_captures` :146.
3. Inputs/outputs. Reads `HandleCallGraph`, `DescriptorMachine` slice, `TerminalModule`; uses terminal_semantics descriptor analysis (`PayloadMembershipHistory`, `projected_stored_sources_in`, `require_unchanged_entry_storage`). Produces `ScalarDefinition`s consumed by origins.rs and replay.
4. How it decides. Native-output writers are a hard-coded table by contract: DuplicateHandle param 3, QueryFileMode 2, ReadFile/WriteFile 2 (:420-425). `read_storage`: inactive case if `case_feasibility == ProvedImpossible` (:758-764); stored sources from writes/transfers/native outputs/call results; else entry snapshot; else a unique `Incoming` payload target with unchanged entry storage (:904-916). A value with several matching payload reads errors (:539-543). `require_machine_call` compares original and substituted calls by pointer equality of argument slices (`std::ptr::eq`, :663-668).
5. Cases by shape. `returns` handles `Return` (no projection) and `ReturnStructural` (with projection) only; other combination errors :709-717. Capacity constant 65_536 in all `IdTable::from_rows_with_gaps`.
6. Same job elsewhere. Scalar-definition chasing parallels origins.rs `definition_equation` (consumer, same unit).
7. Unused. `scalar_captures` used by `read` only (:547); `into_captures` one caller (inputs.rs:245).
8. Stated limits. "This reader identifies those obligations without declaring them proved." :5-6.
9. Fail-open. `Some(Err(_)) => Vec::new()` at :827: an unresolved exact writer contributes no source; the comment (:823-826) says the shared version owner still checks its invalidation footprint (stated). Not traced beyond that.

## provider-admission/src/provider_installation.rs (317 lines, read in full)

1. Purpose. Defines the admitted provider-installation carrier and the function that binds the compiler's selected provider rows to exact candidate rows of the retained Terminal artifact, then replays each installed boundary call.
2. Shape. `ProviderInstallationError` :27; `SelectedProviderAdapter` :56 (strings: provider_declaration, plan digest, selected/requirement/provider/machine identity); `AdmittedInstalledProviderCall` :66; `AdmittedProviderInstallation` :118; `admit_provider_installation_with_projection` :185 (crate-level entry); `admit_terminal_provider_installation` :214 (pub); `select_provider_candidates` :235.
3. Inputs/outputs. Input: `AbstractOperationPlan`, `TerminalArtifactView`, `&[SelectedProviderAdapter]`, assumed-binding selections. Output: `AdmittedProviderInstallation{terminal_selection, psi, installed_candidates, installed_calls}`. Delegates to `terminal_installation::admit_provider_installation`.
4. How it decides. `select_provider_candidates` groups candidates by boundary and requires every candidate of a boundary to have the same non-empty requirement identity text (:269-277); the selected row is matched by requirement identity TEXT (:278) and then by three strings: provider_declaration text, provider_identity text, candidate_identity text (machine) (:282-293). Exactly one match required; zero => `SelectedProviderMismatch`, more => `AmbiguousSelectedProvider` (:295-301). `admit_terminal_provider_installation` pre-filters candidates by whether any selected row has the same requirement-identity text (:219-228).
5. Cases by shape. none (generic).
6. Same job elsewhere. none seen in unit.
7. Unused. grep not run on `admit_terminal_provider_installation` (pub, suspected external caller).
8. Stated limits. Comment :5-7 "Optimizations never select providers or redirect family heads."
9. Fail-open. `admit_terminal_provider_installation` silently drops candidates whose requirement identity matches no selected row (:219-229); a boundary whose candidates are all dropped never reaches the `MissingSelectedProvider` check, so a candidate catalog row for an unselected requirement is simply not installed (traced inside this function only; whether the downstream `admit_provider_execution_installation` requires coverage was not read: open).

## provider-admission/src/provider_installation/replay.rs (436 lines, read in full)

1. Purpose. For every boundary call / external completion in the abstract operation plan whose boundary has an installed provider, re-checks the call's shape against the retained Terminal machine, the boundary declaration and the selected candidate, including result lane, completion receipts and content-claim projections.
2. Shape. `replay_installed_provider_calls` :24; `claim_content_projections` :348; `replays_result` :375.
3. Inputs/outputs. Reads `AbstractOperationPlan` (functions, boundary_machines), `TerminalModule`, installed candidates and assumed bindings. Returns `Vec<AdmittedInstalledProviderCall>`; errors `InstalledCallReplayMismatch` / `InstalledContentClaimMismatch`.
4. How it decides. Lookup tables built with `entry(..).or_insert(row)` so the FIRST row per boundary/required/id wins (:33-52). Keyed on boundary id, psi_edge/psi_operation, argument index plus path equality of receipts (:266-273), claim ids, structural type ids and multiplicity/qualification equality in `replays_result` (:416-433).
5. Cases by shape. Two operation kinds only: `ExternalCompletion` (:56-137) and `BoundaryCall` (:138-343); other abstract operations skipped (:147-149). External completion requires unit result, no entry claims and no content entry claims on the candidate (:114-120).
6. Same job elsewhere. none seen.
7. Unused. none flagged.
8. Stated limits. "Requiring the caller's consumed value to be an unchanged entry parameter would repeat a narrower ownership judgment" :6-8.
9. Fail-open. Three silent skips: `if installed.is_empty() { return Ok(Vec::new()) }` :30-32; `let Some(&provider) = installed_by_boundary.get(boundary) else { continue; }` :66-68 (external completion); same for BoundaryCall :151-160. A boundary call with no installed provider and no assumption link is not replayed and produces no `AdmittedInstalledProviderCall`. "stated" only implicitly: replay joins "installed" rows; calls to boundaries with no provider have no row to replay. Whether a later consumer demands a row for each provider-required call was not traced (open). Duplicate keys: first-wins `or_insert` (:35, :39, :43, :47, :51) means a second row with the same id is ignored, not rejected.

## provider-planning/src/calling_policy_plans.rs (198 lines, read in full)

1. Purpose. Module entry that re-exports the calling-policy planning types and holds `admit_terminal_calling_policy` and `complete_calling_scalar_inputs`.
2. Shape. 13 `mod` lines :23-35; `TerminalCallingPolicyAdmission` alias :70; `admit_terminal_calling_policy` :72; `TerminalCallingPolicyInvocation` :95; `CallingPolicyEvaluator` fn-pointer alias :104; inline `mod scalar_completion` :108 with `CallingScalarScope` :117 and `complete_calling_scalar_inputs` :125.
3. Inputs/outputs. Takes artifact + target + providers; yields admitted compiler evaluation / admitted scalar applications. `complete_calling_scalar_inputs` loops over the closure's scopes (targets) and evaluates scalar applications with an address model per target (:149-195).
4. How it decides. Closure scopes are target names; demand is passed only for the primary target scope (`scope == primary.target_name()` :171).
5. Cases by shape. none.
6. Same job elsewhere. none seen.
7. Unused. none flagged.
8. Stated limits. none.
9. Fail-open. none seen.

## calling_policy_plans/boundary_signatures.rs (389 lines, read in full)

1. Purpose. Defines `MaterializedBoundarySignature` (shapes, fields, parameters, callback data, native parameters, result) and the geometry that maps an opaque-representation shape node back to the unique top-level parameter/result and its native placement.
2. Shape. `MaterializedBoundarySignature` :30; `opaque_representation_movement` :99; `BoundaryOpaqueMovementGeometry::movement` :136; `boundary_shape_path` :221; `BoundaryCallbackBinder` :295; type aliases re-exporting `calling_conventions`/`language_semantics` types :288-292.
3. Inputs/outputs. Pure functions over the shape graph; returns `BoundaryOpaqueRepresentationMovement` with role, path and the validated placement.
4. How it decides. By shape-graph index equality `root == target` (:230); node must be reached through exactly one top-level occurrence (:152-157) and one native parameter (:176-181); shape graph must be acyclic postorder (child index < parent, :238, :263).
5. Cases by shape. `boundary_shape_path` matches `BoundaryValueClass`: Integer/Float/Reference => no path; FixedArray; Record (:233-285).
6. Same job elsewhere. none.
7. Unused. `BoundaryOpaqueMovementGeometry` constructed in boundary_signatures.rs:116 and plan_computation.rs:76 (2 sites, read).
8. Stated limits. none.
9. Fail-open. none seen (each mismatch errors).

## calling_policy_plans/build_time_decoding.rs (562 lines, read in full)

1. Purpose. Decodes build-time evaluation values returned by a calling policy into `BoundaryPlanResult`/`BoundaryEntryPlan`/`CallPlan`/`StatePlan`/`NativePlace`, with count ceilings.
2. Shape. `decode_boundary_plan_result` :31 (pub), `decode_call_plan` :60, `decode_native_place` :116, `decode_calling_convention` :165, `decode_value_shape` :196, `decode_value_location` :235, `decode_register` :328, `decode_entry_control` :357, `decode_state_plan` :372, `decode_counted_array` :463, helpers `struct_parts`/`case_parts`/`field`/`uint`/`case` :486-562.
3. Inputs/outputs. Reads `BuildTimeValue`; produces `calling_conventions` structures or `String` errors.
4. How it decides. Keyed on SOURCE SPELLING of enum variants: `case_parts` takes the text after the last `::` of the variant name (:501-504) and the decoders match string literals such as "MicrosoftX64", "SystemVAMD64", "Aarch64X", "X86Rax" (:165-176, :328-354); fields are found by name (`field` :509-519). `uint` casts `BuildTimeValue::Int` to u64 with `as` (:523).
5. Cases by shape. Closed vocabularies with `other => Err("... outside the compiler-owned vocabulary")` for each enum (:46-48, :159-161, :173-175, :212-215, :351-353 etc.). Capacities: LOCATION 16, REGISTER 64 (:23-25); others imported.
6. Same job elsewhere. The inverse encoding is in policy_signature.rs (`case("NativePlace::Field", ...)` :306-328); both sides spell variant names as strings.
7. Unused. `decode_boundary_plan_result` pub; `PARAMETER_CAPACITY` etc. used in policy_signature.rs:7-10. Not individually grepped.
8. Stated limits. none.
9. Fail-open. `decode_counted_array` ignores array cells past `count` (`values[..count]`, :483); a policy returning garbage in the unused cells is accepted. Which wrong program passes: a policy value whose trailing cells hold nonzero data; those cells are not inspected. "traced" within this file only.

## calling_policy_plans/callback_bindings.rs (328 lines, read in full)

1. Purpose. `BoundaryCallingPlanRealization` (the realization state for one boundary contract), replay validation of its retained plan, and construction of `BoundaryCallingApplication` and `CallingPolicySignature` from a materialized signature.
2. Shape. `BoundaryCallingPlanRealization` :27 (fields incl. `report_fingerprint: u64`, `commitment`, `boundary_entry_plan`, `validated_boundary_entry_plan`, `callback_context_closed`); `native_application` :64; `replayed_validated_plan` :78; `replayed_validated_application` :121; `validate_retained_callback_binders` :179; `validate_callback_requirement_report_collision` :206; `callback_materialization_context` :223; `source_free_application` :239; `MaterializedBoundarySignature::policy_signature` :276.
3. Inputs/outputs. Re-derives validated plan from the retained plan (:78-119), checks opaque-representation custody (:135-154) and recomputes the application identity (:165-170).
4. How it decides. Stale-custody test compares `selected_application_commitment` with a rederived value and schema version (:139-148). Binder duplicate check: any of binder id, static machine ordinal or parameter symbol repeated => error (:192-196). Report-collision: same compact report id with different exact identity string => error (:211-213).
5. Cases by shape. `callback_context_closed` selects between two validators (:96-118).
6. Same job elsewhere. THREE builders of a `BoundaryCallingApplication` with the same commitment tags: `source_free_application` here :239, `CanonicalCallingSignature::native_application` terminal_signature.rs:36-86, and `publish_materialized_application` plan_computation.rs:364-380 (which calls the first). The `CallbackMaterializationContext` is built inline at callback_bindings.rs:97-107 and again in `callback_materialization_context` :223-237.
7. Unused. `callback_materialization_context` used at callback_bindings.rs:276-325 area (line 325 in `policy_signature`).
8. Stated limits. "report_fingerprint ... compact compatibility/report coordinate and cannot authorize replacement" :9-11.
9. Fail-open. none seen.

## calling_policy_plans/callback_layout_catalog.rs (308 lines, read in full)

1. Purpose. Pairs callback layout geometry with source review coordinates and validates that the catalog matches the ordered callback demands and the native/semantic parameter association.
2. Shape. Four review-source structs :21-51; `BoundaryCallbackLayoutEntry` :54 with `from_canonical` :102; `selected_realization` :210; `validate` :229; `validate_entry` :263.
3. Inputs/outputs. Reads `TerminalModule.native_layouts`, `SourceRecordCorrespondence`; returns the entry or `String`.
4. How it decides. Producer record found by exactly one `SourceRecordCorrespondence.record == record` (:110-117); slot matched by `application_commitment` equality (:189). Realization: a single row with empty `target` is "invariant", else exactly one row whose `target` string equals the target name (:214-225).
5. Cases by shape. `validate` considers only demands whose destination is `NativePlace::Field` (:242); ordered strictly increasing destinations (:236).
6. Same job elsewhere. none.
7. Unused. none flagged.
8. Stated limits. none.
9. Fail-open. none seen.

## calling_policy_plans/calling_program.rs (288 lines, read in full)

1. Purpose. `CallingProgram`: shared calling declarations, policies and review coordinates; evaluates a calling policy under a 100,000-unit work budget and materializes canonical signatures and storage extents.
2. Shape. `CALLING_EVALUATION_WORK = 100_000` :16; `CallingProgram` :28 (source_owner Weak, artifact Arc, source_records, source_preparation, evaluator fn); manual `PartialEq` omits evaluator :36-43; `evaluate` :58; `materialize_default` :137; `materialize_storage` :156; `describe_borrowed_storage` :178; `with_storage_extents` :204.
3. Inputs/outputs. Admits, builds sponsor/account with `BuildEvaluationSponsorLimits::new(CALLING_EVALUATION_WORK,1,1,1,1_000_000,...)` :83-92, runs the evaluator fn pointer, returns (`CanonicalCallingSignature`, `ValidatedBoundaryEntryPlan`).
4. How it decides. Default materialization requires `row.default_eligible` (:147). Extents must be completed unsigned/signed integers fitting u64 (:263-274).
5. Cases by shape. Two storage scopes: `Policy` and `Description` (:18-25).
6. Same job elsewhere. none.
7. Unused. none flagged.
8. Stated limits. "finite 100,000-unit semantic work allowance" :4.
9. Fail-open. none seen.

## calling_policy_plans/opaque_representations.rs (157 lines, read in full)

1. Purpose. Data types for opaque-representation uses and their movement (role, path, placement).
2. Shape. `BoundaryOpaqueRepresentationUse` :8 with `rederived_selected_application_commitment` :71; `BoundaryOpaqueRepresentationMovementRole` :124; `PathElement` :133; `Movement` :139. One test :90.
3-9. Pure data. Only judgment: commitment rederivation from conformance commitment, lifecycle, copy disposition, origin (:71-78). Fail-open: none seen. Stated limit: "Symbol handles remain in-process compiler join coordinates" :3-4.

## calling_policy_plans/plan_computation.rs (434 lines, read in full)

1. Purpose. Prepares per-target boundary calling plans from the canonical request roster and original source correspondence, evaluates each policy (in parallel across threads, results in request order), and computes the application identity (report fingerprint + commitment). Also evaluates default (non-authored) calling applications.
2. Shape. `evaluate_default_calling_applications` :24; `prepare_boundary_calling_plans` :96; `PreparedBoundaryCallingPlans` :206 with `evaluate` :277; `PreparedCallingPolicy` :215 with `canonical_instance` :232; `boundary_plan_application_identity` :354; `publish_materialized_application` :364; `activate_in_parallel` :382.
3. Inputs/outputs. Reads `CallingProgram`, `source_preparation`, package inputs (as `DeclarationSelectionAuthority`), selected helper providers. Outputs `Vec<BoundaryCallingPlanRealization>` or diagnostics.
4. How it decides. Rosters must match by count and by ordinal position between canonical module and original source (:105-109, :120-124, :150-154, :156-160); policy must have a body for the target via `declared_callable_for_target` (:132-144); request owner compared by identity text (:243). Thread parallelism: `std::thread::available_parallelism().min(items.len())` (:386-388).
5. Cases by shape. none.
6. Same job elsewhere. `evaluate_default_calling_applications` mirrors the classify/evaluate/validate sequence of `validate_calling_policy_result` (policy_signature.rs:376-445) but with the ordinary evaluator.
7. Unused. `evaluate_default_calling_applications` is `pub(crate)`; callers seen in selected_application.rs:247 (read).
8. Stated limits. none.
9. Fail-open. none seen.

## calling_policy_plans/policy_signature.rs (615 lines, read in full)

1. Purpose. Encodes the canonical boundary signature as a fixed-capacity `BuildTimeValue` for the calling policy and validates the policy's returned plan against that geometry.
2. Shape. `CallingPolicySignature` :21; `encode_calling_policy_signature` :60; `build_native_place` :297; `build_boundary_value_shape` :333; `validate_calling_policy_result` :376; tests :451-615.
3. Inputs/outputs. Pads shapes, fields, parameters, binders, native parameters, demands arrays to their capacities with zero rows (:104-260).
4. How it decides. Variant names are string literals ("NativeParameterOrigin::SemanticFormal", "NativePlace::Field", "Integer", "FixedArray", ...) (:180-235, :299-355). Validation: plan parameter count must equal native parameter count (:385); result presence must agree (:392); each placement is checked by `validate_abi_shape` against its semantic root or against the fixed target-function-pointer shape (:404-428).
5. Cases by shape. Count limits: parameters, shapes, fields, native parameters, binders, demands each have a hard capacity; error "calling policies currently support at most {capacity}" (:66-103).
6. Same job elsewhere. Inverse of build_time_decoding.rs (same variant spelling).
7. Unused. `zero_case`, `row!` macro local.
8. Stated limits. "calling policies currently support at most {capacity}" :100.
9. Fail-open. none seen.

## calling_policy_plans/selected_application.rs (282 lines, read in full)

1. Purpose. For a selected provider row, finds the calling applications (physical ABI) that the row's requirement declares, from explicit realizations, or falls back to default applications for component boundaries.
2. Shape. `selected_calling_applications` :11; `receiver_accepts` :126; `default_calling_abis` :160; `SelectedCallingApplications` :193; `component_calling_applications` :198.
3. Inputs/outputs. Reads `TerminalModule.calling_abi_signatures`, `ProviderPlan` schema; returns applications.
4. How it decides. Keyed on requirement identity TEXT equality (`plan.schema.identity_text(...) == requirement` :32, `module.identities.text(...) == Some(requirement)` :45), method NAME equal to `row.method` (:36), owner application text compared to receiver application text (:154). Explicit realizations chosen by `calling_owner_application() == root && abi() == abi` (:90-93) and must have non-zero matching `report_fingerprint` and `commitment` (:94-97). Default ABIs: `default_eligible`, receiver-accepted, and non-callback formals equal operands by ordinal and kind (:168-184), else the declaration is skipped with `continue`.
5. Cases by shape. Only providers whose rows have `ProviderBinding::CheckedAdapter` are considered in `component_calling_applications` (:211-216, :229-234).
6. Same job elsewhere. none.
7. Unused. none flagged.
8. Stated limits. none.
9. Fail-open. `default_calling_abis` silently `continue`s on declarations that are not default-eligible, not accepted by the receiver, or whose formals do not match operands (:168-184); if all are skipped, `component_calling_applications` errors "has no exact materializable declaration application" (:261-265), so the skip is covered for that caller (traced).
Also: `OMEGA_TRACE_COMPONENT_CALLING` env var enables stderr tracing (:268).

## calling_policy_plans/selected_providers.rs (104 lines, read in full)

1. Purpose. Chooses target helper providers for calling-policy evaluation using the shared canonical selector (prepare per target, select for invocation roots).
2. Shape. `CallingPolicyProviderInputs` :13; `prepare` :21; `prepare_target` :44; `select` :67.
3. Judgment. After selection, every choice must be selected by `BuildOverride` or `TargetDefault` provenance, otherwise error (:91-101). Targets without native realization produce an error row (:31-36).
4-9. Fail-open: none seen. Same job elsewhere: none.

## calling_policy_plans/signature_review.rs (305 lines, read in full)

1. Purpose. Prepares source review coordinates (callback binders, direct callbacks, opaque selections) and attaches them to a canonical calling signature to produce `MaterializedBoundarySignature`.
2. Shape. `PreparedOpaqueReviewSelection` :19; `prepare_opaque_reviews` :27; `PreparedCallingReview` :44 with `prepare` :52, `attach` :82, `opaque_review` :190.
3. How it decides. Canonical binder matched to original by `binder` id then must equal on requirement identity, static machine ordinal, requirement, synchronous flag (:98-113). Opaque occurrence matched to exactly one source selection by FULL `SelectedOpaqueApplication` equality (:196-203). Requirement identity must equal (:89).
4. Fail-open: none seen. Opaque selection with an Err application returns that error when scanned (:196).
5. Same job elsewhere: none.

## calling_policy_plans/source_preparation.rs (220 lines, read in full)

1. Purpose. Captures, while Source still exists, the per-request/per-instance/per-signature review data and target helper candidates into `CallingSourcePreparation`.
2. Shape. `CallingSourcePreparation` :17 (manual `PartialEq` :26); `CallingSourceRequest` :36; `CallingSourceInstance` :45; `CallingSourceSignature` :53; `capture` :61; `capture_signatures` :161; `source_abi` :196.
3. How it decides. Requires `checked.typed` pointer-equal to the service context's typed trees and scope Product (:67-73); requester custody by source file origin: Toolchain => Toolchain; package identity => Package; none => UnownedUser; no file => Missing (:135-144). ABI row must match uniquely and its `id()` must equal the signature id (:204-218). Helpers prepared only if any request has pending policy and signatures (:119-120, :148-152).
4. Fail-open: none seen. Refusals are stored as `Err` values to be raised on demand ("inert until consumed" :5-6): stated.

## calling_policy_plans/terminal_signature.rs (269 lines, read in full)

1. Purpose. `CanonicalCallingSignature`: projects canonical declaration storage into the policy input: value layouts, native formals, callback layouts and demands.
2. Shape. `CanonicalCallingSignature` :20; `CanonicalOpaqueUse` :30; `native_application` :36; `materialize` :88.
3. How it decides. For each parameter: Owned access => value recipe, any other access => reference recipe (:134-139); callback layouts sorted by destination (:231-234); requirement ids checked for report-id collisions (:194-212).
4. Same job elsewhere. `native_application` :36-86 duplicates `source_free_application` (callback_bindings.rs:239-273) except `calling_owner_application: None` and `dispatch_only_parameter_count` param.
5. Fail-open: none seen.

## provider-planning/src/data_layout_queries.rs (173 lines, read in full)

1. Purpose. Adapter that answers Psi's source layout queries (`size_of`/`align_of`) for the selected target, and a `sizing` submodule of primitive geometry.
2. Shape. `TargetLayoutQueries` :46 (impl `LayoutQueryProvider` :60: `layout_target_inputs` :63, `prepare` :84), `TargetLayoutMeasurements` :53 (impl `measure` :123), `sizing::primitive_layout` :147 / `primitive_type_layout` :163. Re-exports :26-38.
3. How it decides. `prepare` clones the whole `TypedTrees` (`program.clone()` :96) only when some plan-laid layout has `TargetInputs` realizations, closing each to `Invariant(geometry_for_target(target_name))` (:86-112); otherwise borrows. `measure` returns `Ok(None)` when `try_layout_type_reference` finds an unresolved array extent (:128-134). Primitive sizes by `PrimitiveType` match: Bool/I8/U8 = 1, I16/U16 = 2, F32/I32/U32 = 4, F64/I64/U64 = 8, Addr/Selector = pointer (:152-160).
4. Same job elsewhere. Primitive/case-tag/record layout is also computed in terminal_semantics `ReferenceStorageGeometry` (used by windows_handle_storage.rs:524-534, :815-825, :1173-1183). Suspected duplicate sizing; the other site works on Terminal structural types, this one on typed trees. Not read.
5. Fail-open. `measure` returning `None` for an unresolved extent defers evaluation (stated in module header "deferral" :4); not traced to its consumer.

## data_layout_queries/builder.rs (496 lines, read in full)

1. Purpose. Entry points and state for computing layouts: `build_layout_plan(_for_roots)`, `layout_type_reference`, `try_layout_type_reference`, `LayoutBuilder`, visit stack for recursion detection, and generic binding resolution.
2. Shape. `build_layout_plan` :47; `build_layout_plan_for_roots` :60; `layout_type_reference` :95; `try_layout_type_reference` :110; `OpaqueRepresentationDemand{CatalogConstruction,ByValue}` :127; `LayoutBuilder` :132; `LayoutVisitStack` (16 inline + overflow vec) :155; `layout_data_definition` :277; `type_descriptor` :350; `generic_bindings` mod :425 (`binding_for_type` :439, `fixed_array_length` :456, `const_argument_value` :473).
3. How it decides. Roots skipped when: generic (type_parameters non-empty :80), `DataSupplyMode::BoundaryOpaque` :83, or proof-only :86. Recursion detected by symbol membership in the visit stack (:282). Generic binding lookup: by symbol when valid, ELSE BY PARAMETER NAME string equality (`binding.parameter_name == name`, :447-451). `fixed_array_length`: literal, const-parameter via binding, `Expression` => None, `ConstCall` => closed length from program (:462-470); const argument chase depth cap 16 (:479). `type_descriptor` maps unresolvable array lengths and ConstExpression to `Unit` (:391, :419).
4. Cases by shape. `TypeReferenceNode` variants Reference, Constrained, FixedArray, Slice, DynamicTrait, Generic/Named, ConstExpression, Unit (:360-421). Only built-in `UInt`/`Int` symbols are special-cased as pointer-sized (:341-345).
5. Same job elsewhere. `data_layout_entry_by_symbol` / `data_definition_by_symbol` use linear `.find` over arenas (:323-338); the Arena is used by symbol not dense id (AGENTS.md data-oriented rule; recorded only).
6. Fail-open. `type_descriptor` returning `Unit` for an unresolved fixed array length (:391) means a field descriptor loses its array shape; the physical `layout` is computed separately (type_reference_layouts.rs:80-85 sets `unresolved_array_extent`). Consumers of the descriptor (e.g. `fixed_array()` in semantic_ranges.rs:62) error on a repeated field whose descriptor lacks a fixed array (:62-67), so this is covered there (traced).

## data_layout_queries/builder/data_layouts.rs (354 lines, read in full)

1. Purpose. `compute_data_layout` for records, quotient types, compiler accessors, plan-laid records and case-bearing (Enum/Mixed) data.
2. Shape. `compute_data_layout` :35; `plan_field` :256; `compute_case_bearing_layout` :275.
3. How it decides. Compiler accessor => pointer-sized empty record (:40-55); quotient => carrier's layout re-tagged (:57-74); case-bearing if `shape_kind_from_members` is Enum or Mixed (:77-82); plan-laid if `plan_laid_layout_for_data` returns a plan, checking offsets/field symbols inventory equal (:105-118). Case layout rule (:281-353): i32 tag (`ENUM_TAG_BYTES`) at offset 0, common fields after, payload base = common end aligned to the max payload alignment across all variants; erased fields excluded (:305).
4. Cases by shape. Quote :12-16 "lay out as a tag-prefixed overlay without niche packing: the `i32` tag sits at constant offset 0".
5. Same job elsewhere. Case layout parallels terminal_semantics `geometry.sum_layout` used in windows_handle_storage.rs:535 (suspected duplicate, other site not read here).
6. Fail-open. `canonical_layout_subject = plan.native_policy_identity(..).ok()` and `canonical_data_identity ... .ok()` (:170-185) turn identity errors into `None`; the data identity is then simply not recorded (:211-236); private-callback closure errors only when demands exist (:191-200). So a plan-laid record with no callback demands and failing identity derivation loses its `TargetClosedDataLayoutIdentity` row silently. Traced inside this file; consumer of `data_layout_identities` not traced (open).

## data_layout_queries/builder/private_callback_closure.rs (437 lines, read in full)

1. Purpose. Closes target-specific private callback slots (checks alignment, bounds, non-overlap) and builds the catalog of inline record paths to callback slots from every reaching root.
2. Shape. `close_private_callback_layouts` :22; `reaches_slot` :166; `close_path` :199; `close_private_callback_demands` :313.
3. How it decides. Identity keyed on strings: slot identity string, requirement identity string, layout subject string; collision checks on derived compact ids (`callback_layout_slot_id`, `callback_requirement_id`) vs. exact string (:389-414). Natural (non-plan-laid) roots that reach a slot get a synthesized layout id from a domain-tagged fingerprint over canonical identity + field identities/offsets/sizes (:79-107) with tag "omega.callback-natural-data-layout.v1". Overlap tests use bit ranges (`ranges_overlap` semantic_ranges.rs:144).
4. Cases by shape. Natural roots handled only when `DataShape::Record` (`else { continue }` :69-74); enums are skipped silently.
5. Fail-open. `let DataShape::Record {..} = root.shape else { continue; }` :69-74: a data root that is an Enum/Mixed reaching a callback slot is skipped without error in the identity-synthesis loop; `reaches_slot` also only descends into Record shapes (:184-186). So a callback slot inside an enum payload is never found, and no row or error is produced. Which wrong program: callback slot nested in a case payload; whether such a slot can exist (demand originates from plan-laid records only) not traced (open).
6. Unused / one caller. `close_private_callback_demands` called from data_layouts.rs:201 and tests.rs:137.

## data_layout_queries/builder/semantic_ranges.rs (151 lines, read in full)

1. Purpose. Derives bit-level occupied ranges for plan-laid records so callback slots do not overlap semantic fields (bit fragments, stored integers, repeated-field elements, plain fields).
2. Shape. `plan_laid_semantic_ranges` :10; `SemanticRanges` :99 (`push` :106, `validate_extent` :122); `ranges_overlap` :144.
3. How it decides. Looks up `geometry.bit_fields/integer_fields/repeated_fields` by `field_index == index` with linear `.find` (:21-24, :48-51, :57-60). Repeated fields need a `fixed_array()` descriptor and size divisible by length (:62-73).
4. Fail-open. none seen.

## data_layout_queries/builder/type_reference_layouts.rs (295 lines, read in full)

1. Purpose. Layout of type references: references (fat if referee is a slice), arrays, slices, generics (monomorphized, recorded by symbol), named types, boundary-opaque redirects to the selected representation carrier.
2. Shape. `layout_type_reference_handle` :31; `layout_generic_data_definition` :151; `layout_named_type` :241.
3. How it decides. Named type: compiler accessor; `PrimitiveType::from_name(name)` (SOURCE SPELLING, :254); builtin; invalid symbol => error; data definition (boundary-opaque with a selection redirects to `selection.carrier_type()` :270-274; ByValue demand without selection errors :275-279); boundary trait => (0,1) :284-288. Generic instance identity = joined display strings of arguments (`display_type_reference_with_constraints` :171-178) compared with `==` (:208); on a second distinct instance the earlier layout's symbol is set invalid and the signature blanked "poisoned" (:214-222).
4. Cases by shape. `Generic` with a non-valid symbol or non-data base => error "native layout for generic type `{base_name}` is not implemented yet" :132-134 (stated limit).
5. Fail-open. Unresolved array extent returns `TypeLayout::default()` and sets `unresolved_array_extent` (:80-85) rather than an error; `try_layout_type_reference` converts that to `None` (builder.rs:123); `layout_type_reference` (builder.rs:95-108) does not check the flag, so a caller using it gets a zero layout for an array with unclosed extent. Callers of `layout_type_reference` not grepped (open).

## data_layout_queries/builder/tests.rs (438 lines, read in full)

Test fixtures for the builder (private callback closure mutations at :148-285, erased payload exclusion :288, full-width capacity overflows :331-382, quotient layouts :385-438). No production logic. Fail-open: n/a.

## data_layout_queries/callback_layout.rs (422 lines, read in full)

1. Purpose. `PrivateCallbackLayout`/`PrivateCallbackInlineField`: an exact inline path from a root record to a private callback slot with geometry validation.
2. Shape. `PrivateCallbackLayout` :27 (`from_fields` :82, `native_demand` :111, `validate` :135); `validate_geometry` :181; `valid_layout` :251; `contained` :261; tests :275-422.
3. How it decides. `validate` requires slot identity and requirement identity non-empty and the compact ids rederive from the strings (:140-145); destination must equal the recomputed native demand (:153); each inline hop must be contained in its parent, not repeat a data symbol, have child physical == field layout (:199-214); composed placements must equal the stored ones (:238).
4. Same job elsewhere. Placement validation `layout_plans::validate_private_callback_placements` is also used by private_callback_closure.rs:360 and here :227, :241.
5. Fail-open. none seen.

## data_layout_queries/layout_plan.rs (255 lines, read in full)

Pure data: `LayoutPlan` :245, `TypeLayoutDescriptor` :30, `FieldLayout` :96, `BitFieldLayout`, `StoredIntegerLayout`, `RepeatedFieldLayout`, `DataShape` :167, `DataLayout` :186, `TargetClosedPrivateCallbackDemand` :205, `TargetClosedDataLayoutIdentity` :220. `ENUM_TAG_BYTES = runtime_abi::CASE_TAG_BYTE_SIZE` :21 (shared with the runtime ABI crate). `inline_data_symbol` returns the symbol for `Named` and through `Constrained` only (:63-69), so references and arrays are never "inline hops". No judgment. Fail-open: none.

## data_layout_queries/packing.rs (261 lines, read in full)

1. Purpose. Sequential field packing with checked arithmetic and plan-directed placement.
2. Shape. `pack_fields` :25; `pack_fields_at` :32; `place_fields_by_plan` :81; `align_to` :124 (alignment 0 => value unchanged); `placement_overflow` :132.
3. How it decides. Offsets aligned up in declaration order (no reordering); `place_fields_by_plan` errors on offset-count mismatch or `offset + size` overflow (:90-119); it does NOT check alignment or overlap of the plan's offsets (validated upstream per header comment of data_layouts.rs and `geometry.validated_layout`).
4. Fail-open. `align_to(_, 0)` returns the value (:125-126): a zero-alignment field is accepted unaligned; the field's alignment comes from layouts that are never zero by construction (inferred, not traced).

## dispatch_tables.rs (117 lines, read in full)

1. Purpose. For a table-dispatch foreign leaf, identifies the dispatch-table data declaration (the first parameter's referent) and the function-address field.
2. Shape. `DispatchTable` :19; `dispatch_table` :48; `field_storage` :74; `unrefined` :110.
3. How it decides. First entry-state parameter must be a Reference with access not `WriteOnly` whose referee (stripped of `Constrained`) is a named data declaration (:54-67). Field found by NAME: `field.name.as_str() == field_name` among non-erased fields (:87); its type must be primitive `Addr` (:94).
4. Fail-open. none seen.

## provider-planning/src/evaluated_via_bindings.rs (272 lines, read in full)

1. Purpose. Evaluates every ordinary `via` binding (machine trait conformance with a `via` expression) once at build time, producing a table of evaluated native bindings with receipts.
2. Shape. `EvaluatedViaBindingRow` :49; `EvaluatedViaBindingTable` :88 (`exact` :121 looks up by (realization_machine, satisfied_owner, requirement)); `evaluate_via_bindings` :140; `evaluate_checked_via_bindings` :153; `evaluate_program_via_bindings` :167; `retained_usage` :33.
3. How it decides. Target per machine: authored machine target if present, else the selected target, else error (:208-216). Rows sorted by arena indices (:243-250); duplicate (machine, owner, requirement) rows error (:251-261). Any error makes the whole call fail (`diagnostics` non-empty => Err :262-270).
4. Same job elsewhere. `evaluate_via_bindings` and `evaluate_checked_via_bindings` differ only in the `BuildTimeProgram` variant.
5. Fail-open. none seen.

## evaluated_via_bindings/binding_digests.rs (245 lines, read in full)

Domain-separated SHA-256 digests (producer closure, user source, evaluation, materialization) via `DomainDigestEncoder`. `stable_source_digest` :77: Toolchain origin => toolchain identity digest; else digest of package identity + path RELATIVE to package root (or bare file name when the file is outside the root and has no package identity) + source text (:89-113). `encode_value` :173 covers every `BuildTimeValue` variant (Unit, LayoutTarget, Int, Bool, Float, UndeterminedNaN, Text, Struct, Case, Array). Fail-open: a file outside its package root with no package identity hashes only its file name (:91-93) (stated by code shape only).

## evaluated_via_bindings/binding_evaluation.rs (245 lines, read in full)

1. Purpose. Evaluates one `via` expression: resolves the producer entry state, evaluates it const under the admitted build program, decodes the native binding value (import locator or syscall number), builds digests and receipt.
2. Shape. `evaluate_one` :27; `exact_binding_vocabulary` :216; `binding_widths` :239.
3. How it decides. The `via` expression must be exactly one machine call (:48-53); the callee must be the entry state of exactly one producer machine (:54-74); target-family interface producers resolve to the member for the selected target (:75-90). Syscall bindings allowed only for `LinuxArm64 | LinuxX64` (:142-149); number must fit u32 (:136).
4. Fail-open. none seen.

## provider-planning/src/intrinsic_declarations.rs (986 lines, read in full)

1. Purpose. Decides whether a selected provider plan row is a compiler intrinsic with a known execution identity: sealed scalar accessors, UEFI fixed-image intrinsics (`ProgramStorageEntry`, `ProgramImage`), and hosted Console/ProcessExit rows (`exit_process`, `write_byte`, `read_byte`).
2. Shape. `IntrinsicDeclaration` :36 with `prepare` :116, `prepare_kind` :140, `settle` :274; `HostedSubject` :62 (`matches` :84); `HostedService{Console,ProcessExit}` :337; `BoundaryIntrinsicShape` (4 shapes) :396; `HOSTED_INTRINSIC_ROWS` (4 rows) :411; `ExitProcessDeclaration` :516 (`prepare` :533, `matches` :580); `boundary_row_shape` :606; `exact_boundary_signature` :775; `exact_boundary_state` :839; `canonical_storage_parameters` :867; `exact_byte_read_type` :931 (pub(crate)); `exact_bundled_standalone_source` :497; `bundled_intrinsic_source!` macro :462.
3. Inputs/outputs. Reads `TypedTrees`, `ProviderPlan`, accepted semantic bindings (console / process_exit). `settle` returns `Option<CompilerIntrinsicExecutionIdentity>`.
4. How it decides. KEYED ON SOURCE SPELLING AND BYTES: trait name equals "Console" / "ProcessExit" (`definition.name.as_str() != trait_name` :634, names :350-355); requirement method names "exit_process", "write_byte", "read_byte" and realization names such as "ConsoleNativeProvider::exit_process" are string constants (:411-440) compared with `method.name != requirement_name` (:672) and `realization.name.as_str() != realization_name` (:708); result type name "ByteRead" (:940), parameter type name "i32" (:970), UEFI target string "uefi_x86_64" (:189), file names "extent.omg", "program_image.omg" (:191-208). Bundled-ness: the declaring source file's bytes must equal `include_bytes!` of the library file (`exact_bundled_standalone_source` :497-513, `source.source.as_bytes() == expected_source`). Accepted-package path: owner package identity, declaration path string, schema digest and selected provider plan digest all must match (:84-112).
5. Cases by shape. Four hosted rows and two UEFI fixed rows are the whole list (`HOSTED_INTRINSIC_ROWS` :411-440; UEFI loop :190-209). `HostedIntrinsicBundle` has four variants LinuxArm64, LinuxX64, MacosArm64, MacosX64 (grep result: target_profile.rs:107-112), so `HostedIntrinsicBundle::from_target_name(&plan.target)` returning None => `Unsupported` (:237-239); the bundled-source macros list the same four (:481-495). No Windows variant appears.
6. Same job elsewhere. The Windows ExitProcess is recognised separately by DLL/export locator in windows_handle_contracts.rs:325-332 (ExitProcess in kernel32.dll), not by this table.
7. Unused / one-caller. Grep `intrinsic_declarations::|IntrinsicDeclaration::prepare|ExitProcessDeclaration::` over omega-rust: 8 hits in 3 files (selected-dispatch/compiler_intrinsic.rs 3, provider_rows.rs 4, provenance_replay.rs 1).
8. Stated limits. none beyond the shapes list. Comment :11-13 on `read_byte` requiring "`blocks;` plus a single unconditional `crashes Trap`".
9. Fail-open. `settle` (:274-333): (a) `target != Some(self.target)` => `Ok(None)` (:290-292) meaning "not an intrinsic", not an accept. (b) In the Hosted arm `consumed` is true when the accepted binding matches and `binding.declaration_identity` is `None` (:314-319): with no expected identity supplied the declaration-identity comparison is skipped. Wrong program: one whose accepted binding carries no declaration identity AND matches package/path/digests still passes without the identity equality; traced only to this function (callers not traced: open). (c) A row is accepted when `row.bundled || consumed` and its shape is true (:326).

## provider-planning/src/intrinsic_requirements.rs (99 lines, read in full)

1. Purpose. Thin Omega wrapper over the typed-trees `IntrinsicRequirement` adding numeric execution identity lookup and provider-schema binding.
2. Shape. `IntrinsicRequirement` newtype :14; `closed_float_execution` :28 (only `NumericIntrinsic::FloatBinary`); `execution_identity` :40; `schema_binds` :60; `plan_row_binds` :84.
3. Judgment: `schema_binds` branches on `IntrinsicRequirementKind::{Operator, TopLevelRequirement}` (:66-81). Fail-open: none seen. The module comment states `closed_float_execution` is "narrower than" `execution_identity` :4-5.

## provider-planning/src/lib.rs (81 lines, read in full)

Crate root: `#![forbid(unsafe_code)]` :1; module list and re-exports :23-81. Comment :16-17 says "Start at `provider_settlement.rs`... then `provider_planning.rs`". `source_guards` is `#[cfg(test)]` :46-47. No judgment.

## provider-planning/src/native_bindings.rs (17 lines) and native_bindings/plans.rs (351 lines), plans/tests.rs (105 lines), all read in full

1. Purpose. Forms provider plan candidates from Build-supplied native bindings (a Build declaration naming a locator per requirement per target), evaluating each locator under its own target and producing retained evaluation receipts.
2. Shape. `NativeBuildSupply` native_bindings.rs:14; `PreparedNativeBuildBindings` plans.rs:39 (`capture` :57, `form` :95); `NativeBuildRowOrigin` :34; `evaluate` :240.
3. How it decides. Locators are normalized with `target::normalize_foreign_locator(raw, target)` for EVERY authored target even when unselected (:121-126, tested :83-95); a Syscall locator is valid only for `LinuxArm64 | LinuxX64` (:258-263); then `if selected_target != Some(target) { continue; }` (:132) so only the selected target forms a plan row. Plans are grouped by (schema owner symbol, target name, supplier package) (:144-148); rows for the same requirement identity string merge when identity digests are equal, else error "binds one requirement to conflicting normalized locators" (:192-209). Provider/plan name is derived from the supplier package digest in hex plus `trait_name_text` (:152-163).
4. Cases by shape. Two locator kinds: `Import` and `Syscall` (:250-270). Digest tags with fixed strings and `2` as the materializer schema version literal (:327).
5. Same job elsewhere. `evaluate` (:240-351) parallels `evaluate_one` in evaluated_via_bindings/binding_evaluation.rs:27-214 (locator decode, syscall target check, digest triple, `EvaluatedBindingReceipt::from_evaluation`, `EvaluatedNativeBinding::Import/Syscall`). Both check Syscall against the same two Linux targets (binding_evaluation.rs:142 and plans.rs:258-263). Materializer schema version is 1 in `evaluated_via_bindings.rs:27` and the literal 2 here :327.
6. Fail-open. none seen. Test fixture only.

## provider-planning/src/native_provider_sources.rs (120 lines, read in full)

1. Purpose. Captures, per selected provider row, the source family head identity for target-machine families, then attaches the final plan digests.
2. Shape. `PreparedFamilyRealization` :12; `attach_native_provider_sources` :18; `capture_family_realizations` :56.
3. How it decides. Finds `typed.target_machine_families` whose members include the row's realization machine with `member.target.as_str() == plan.target` (:80-85, STRING compare of target name); mismatched `origin.target` errors (:86-93).
4. Fail-open. `else { continue; }` at :77-79: a row with neither a target-machine origin nor an authored realization is skipped silently (no family realization recorded). Which program: a row realized by import or build-native binding rather than a machine; consistent with "family" only applying to machine realizations (inferred, not traced to consumer).

## provider-planning/src/program_entry.rs (864 lines, read in full)

1. Purpose. Selects the target-qualified `ProgramEntry` root slot from Build root bindings, validates the chosen entry machine's source shape, and later settles its semantic and physical calling plans (`settle_calling_plans`); also derives the UEFI `Extent` record layout.
2. Shape. `SelectedProgramEntry` :49; `selected_program_entry_machine` :55; `validate_selected_program_entry_shape` :160; `PreparedCompilerProgramEntry` :264 (`service_requirements` :272, `settle_calling_plans` :297); `SelectedCompilerProgramEntry` :320 (`bind_physical_adapter` :346); `check_unrequested_program_entry_rows` :478; `prepare_compiler_program_entry` :535; inline `mod calling_plans` :593 (`settle` :598); inline `mod extent_layout` :748 (`capture` :757). Submodules `discovery`, `source_contract` are in other files (not read).
3. Inputs/outputs. Inputs: `RootBinding` list (slot string, implementation name/symbol), selected `TargetProfile`, `ServiceCatalog`, package inputs. Outputs: `PreparedCompilerProgramEntry` then `SelectedCompilerProgramEntry` with `SelectedProgramEntryCallingPlans{semantic_calling_application, physical_calling_application, storage_entry}`.
4. How it decides. Slot string is split with `rsplit_once("::")` into target owner and slot name (:68, :485); owner parsed by `TargetProfile::from_root_slot_owner`; slot looked up by `profile.required_root_slot(slot_name)` (:85). Exactly one binding per required slot for the selected profile (:104-128), and exactly one required slot with a ProgramEntry schema (:143-157). Family resolution: member whose `target.as_str() == profile.target_name()` (:508-512, :556-560). `settle` requires exactly one evaluated realization matching (boundary trait symbol, requirement machine symbol) for each of the semantic and physical requirement (:618-644), replays both (:649-663), and compares the calling policy with the slot's convention (:672-694).
5. Cases by shape. (a) `extent_layout::capture` accepts only owner `UefiX64`, schema `ProgramStorageApplication`, visible parameters `ImageAndInitialStorage` and semantic convention `MicrosoftX64` (:762-773: "restricted to the exact UEFI/Microsoft program-storage schema"). (b) Extent must be a record with exactly two non-erased fields named "base" (type addr) and "length" (type u64), compared by name string (:799-821). (c) `visible_parameters` role by index: 0 => Image, 1 => InitialStorage, `_ => unreachable!("selected source shape validation fixed visible arity")` (:187-191). (d) "uses a target-required schema that the ProgramEntry source-lowering path does not implement" (:130-134).
6. Same job elsewhere. The family-member resolution block appears twice in this file (:501-516 and :548-568) and a third near-identical form in native_provider_sources.rs:80-85.
7. Unused / one-caller. Grep `check_unrequested_program_entry_rows|selected_program_entry_machine|prepare_compiler_program_entry|discover_explicit_entry_binding` over omega-rust: 15 hits (build-settings lib.rs 2, program_entry.rs 8, discovery.rs 3, compiler target_preparation.rs 2).
8. Stated limits. :130-134 quoted above; :762-773 quoted above.
9. Fail-open. `check_unrequested_program_entry_rows` skips (`continue`) bindings whose slot has no `::`, an unknown owner, an unknown slot name, a slot with no ProgramEntry schema, or a family with no member for that profile (:485-515). Traced: `selected_program_entry_machine`, called first at :543, reports the first three as errors for all bindings (:68-92), so the unknown-owner/slot cases are covered; the "family has no member for that profile => continue" (:508-514) is not covered: a Build matrix row whose entry family lacks a body for another profile is not validated and no error is produced (traced inside this file).

---

## Cross-cutting records (same job elsewhere / counts)

- Command: Grep `HandlePathProgram|HandlePathFailure|HandlePathNode|HandlePathProof` over omega-rust => only windows_handle_paths.rs hits; `mod windows_handle_paths;` is `#[cfg(test)]` (terminal_authority_review.rs:37-38). The path-replay program is test-only; execution.rs does the production equivalent.
- Command: Grep `\blayout_type_reference\b` over omega-rust => 3 hits, all in provider-planning (re-export data_layout_queries.rs:27, comment builder.rs:3, definition builder.rs:95): no call site. `build_layout_plan`/`build_layout_plan_for_roots` (Grep count) => data_layout_queries.rs 1, builder.rs 5, builder/tests.rs 7: no caller outside provider-planning's own tests/definitions. Grep `data_layout_queries::` excluding the module dir shows the only outside user of this module is `TargetLayoutQueries` at build-settings/src/admission/selected_constants.rs:52. So `LayoutPlan`, `PrivateCallbackLayout`, `close_private_callback_layouts` have no production caller outside this module (inferred from those greps).
- Parallel job: `layout::TerminalCallbackLayouts` (backend/layout/src/terminal_callback_layouts.rs:42, used from calling_policy_plans/terminal_signature.rs:113) builds private callback layouts from Terminal; `data_layout_queries/builder/private_callback_closure.rs` + `callback_layout.rs` builds `PrivateCallbackLayout` from typed trees. Not read on the other side: suspected duplicate.
- Jobs duplicated inside the unit: (1) BoundaryCallingApplication builders: callback_bindings.rs:239, terminal_signature.rs:36, plan_computation.rs:364. (2) Native binding evaluation: binding_evaluation.rs:27 and native_bindings/plans.rs:240. (3) Family member resolution: program_entry.rs:501 / :548 and native_provider_sources.rs:80. (4) Handle-lifetime join logic: execution.rs `Pending::push` :222-260 and test-only paths.rs :104-147.
- Callers: `checked_calls` (windows_handle_replay.rs:2628) has one call site (reviewer.rs:237). `admit_terminal_provider_installation` has one external caller (build-settings/src/admission/provider_execution.rs:64). `admit_provider_installation_with_projection` has one caller (provider_admission.rs:392). `evaluate_via_bindings` one caller (build-settings/src/admission/checking.rs:120); `evaluate_checked_via_bindings` one caller (compiler/src/checked/target_preparation.rs:109). `dispatch_table`/`field_storage` have callers in plan_derivation.rs:530/:543 and review-evidence (external_supply.rs:112,:167,:223,:240; policy/bindings.rs:189,:192).

---

- **Coverage:** Read in full, every line: all 48 files, 21,987 lines (windows_handle_* 10 files 9,991 lines; provider_installation 2 files 753; calling_policy_plans 14 files 4,459; data_layout_queries 10 files 3,282; remaining provider-planning 12 files 3,502). Skimmed: none. Not read: none. Test modules inside these files were read too. Files referenced but outside the unit and not read: windows_handle_aliases.rs, windows_handle_arguments.rs, windows_handle_calls.rs, windows_handle_conditions.rs, windows_handle_activations.rs, context.rs, program_entry/discovery.rs, program_entry/source_contract.rs, provider_planning/*, terminal_semantics and layout crates.

- **Five things a maintainer most needs to know about this unit.**
  1. The Windows handle proof (windows_handle_replay.rs + execution.rs) is a bounded whole-program symbolic replay that mints a narrowed mechanism identity with an EMPTY authority-class disposition (`from_classes([])`, replay.rs:2799-2802) for close/duplicate sites only. It only runs when the trigger is a reviewed CloseHandle or DuplicateHandle that the physical policy has not already classified (replay.rs:2646-2657), on WindowsX64 with a permission policy (:2638-2641). The proof rests on a hard-coded list of SDK premises (replay.rs:2690-2704).
  2. Its API knowledge is a fixed table of 8 APIs plus 7 "no handle intervention" APIs keyed by normalized PE (dll, export) locator digest (contracts.rs:35-47, :92-186), and the replay's output-slot, handle-role and ReadFile/WriteFile byte-view logic is per-contract (`match (contract, arguments)` replay.rs:827-996; values.rs:420-425). A new API needs edits in contracts.rs, values.rs, replay.rs and the arguments file (not read).
  3. Every analysis here has hard budgets: 65,536 (equations, states, activations, ranks), 256 call depth, 1,048,576 steps (execution.rs:313, :252, :987; origins.rs:118; paths.rs:140). Exceeding any returns an error; nothing falls back to accepting.
  4. In provider-planning, large parts are keyed on source spelling and bundled source bytes: hosted intrinsic trait/method/realization names and `include_bytes!` file comparison (intrinsic_declarations.rs:350-355, :411-440, :497-513), calling-policy enum variants by string suffix after `::` (build_time_decoding.rs:501-504, policy_signature.rs:180-355), Extent fields named "base"/"length" (program_entry.rs:812-813), dispatch table field by name (dispatch_tables.rs:87).
  5. `data_layout_queries` has a large production-looking surface (`build_layout_plan*`, `layout_type_reference`, `LayoutPlan`, `PrivateCallbackLayout`, callback closure) with no caller outside the module's own tests per my greps; only `TargetLayoutQueries`/`try_layout_type_reference` feeds production (selected_constants.rs:52). Similarly `windows_handle_paths.rs` is test-only.

- **What you are unsure about.**
  - Whether callers of `IntrinsicDeclaration::settle` supply `declaration_identity: None` in production (intrinsic_declarations.rs:314-319); not traced.
  - Whether downstream consumers require a replay row for every provider-required boundary call (provider_installation/replay.rs:66-68, :151-160 silently `continue`).
  - Whether any callback slot can sit inside an enum payload (private_callback_closure.rs:69-74, :184-186 skip non-record shapes).
  - Whether `data_layout_identities` rows lost to `.ok()` (data_layouts.rs:170-185) are required by a later consumer.
  - The unseen sides of the suspected duplicates (terminal_semantics sum/record layout vs data_layouts.rs; `layout::TerminalCallbackLayouts`).
  - I did not individually grep every `pub(super)` method in windows_handle_storage.rs for use counts; all were seen called from replay.rs/execution.rs while reading, but not counted.

