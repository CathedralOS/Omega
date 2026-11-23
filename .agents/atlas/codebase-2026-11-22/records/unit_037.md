# Record for unit_037 (areas 13-native-targets-driver and 08-psi-program)

All paths below are relative to `source-rust/subsystems/compiler/`. Abbreviations: `PA` = `psi-to-native/verification/provider-admission/src/terminal_authority_review/`, `PC` = `psi-to-native/verification/provider-correspondence/`, `PV` = `psi-to-native/verification/`, `ACC` = `psi/access/`, `ART` = `psi/artifact/`.
Everything here was read with the Read tool. Counts come from Grep count mode (command named inline). "Handle" in the first group means a Windows file HANDLE.

---

## Part A. Windows file-handle proof replay (PA, 5 files, 7,437 lines)

These five files belong to one proof. A Windows program that calls `CloseHandle`/`DuplicateHandle` through a normalized foreign import has no admitted physical mechanism row for those calls. `checked_calls` replays every control path of the retained Terminal program, tracks each HANDLE value as an alias set, and, if every returning path closes every handle it acquired, mints a per-call "checked argument contract" identity and a policy row with an empty disposition. The only caller of `checked_calls` is `reviewer.rs:323` (grepped `checked_calls\(` in `provider-admission`: 1 call site plus the definition).

### A1. `PA/windows_handle_replay.rs` (2,819 lines)

1. **Purpose.** Holds the per-path state of the replay (which handle instance each integer word may carry, captured pointers, active sum cases, known constants) and the operations that change it: one ordinary operation, one native call, entering a callee, capturing and resuming a return, and following a control edge. It ends with `checked_calls`, which turns a finished replay into per-boundary "checked leaf" rows.
2. **Shape.**
   - `HandleReplayActivations` (:57) maps an activation index to `{machine, source: DomainDigestEncoder}` and `children: BTreeMap<(usize, Caller, MachineId), usize>`; `enter` (:84) and `native` (:134) hash the caller chain into a source-context digest; `scalar` (:215).
   - `KnownHandleScalar` (:242: `Unknown | Integer | Boolean | Nonzero`), `CapturedHandleScalar` (:279), `CapturedMemory` (:291), `CanonicalHandleState` (:310: ledger, aliases, references, cases, constants, stored_constants, bindings).
   - `HandleReplayFrame` (:354) holds the whole suspended caller state; `CapturedHandleReturn` (:385); `CapturedHandleArrival` (:393).
   - Methods on `CanonicalHandleState`: `capture_arrival` (:410), `install_arrival` (:558), `require_active_payload` (:601), `native` (:630, per-contract handling), `capture_return` (:1002), `resume_after_cleanup` (:1108), `output_census` (:1217), `enter_call` (:1359), `operation` (:1548), `equation` (:2354), `capture`/`install`/`replace` (:2442/:2473/:2465), `store_scalar`/`read_scalar` (:2515/:2552).
   - `CheckedHandleLeaf`/`CheckedHandleCalls` (:2592/:2598), `checked_calls` (:2620).
3. **Inputs/outputs.** Reads `ReviewContext`, `HandleCallGraph`, `WindowsHandleInputs`, `HandleStorageGeometry`, `HandleLifetimes` (ledger, in another file), `terminal_semantics::descriptor_*` tables. Produces new `CanonicalHandleState` values (each method consumes `self` so a refusal cannot leave a half-updated state, :2185-2187), `HandleNativeOutcomes` (a list of alternative states per native call, :338), and finally `CheckedHandleCalls { leaves: IdTable<BoundaryMachineId, ..>, rows }` (:2818).
4. **How it decides.**
   - Which call occurrence is meant: the key is the `Caller` coordinate (machine, block, operation index, `InvocationSite`) plus callee id (`enter` :98-108). A second matching call from the same caller refuses as ambiguous.
   - Native call accepted only if the admitted mechanism is a `NormalizedForeign` import for `WindowsX64` with `AdmittedPlan` argument contract and a nonzero implementation contract, and its locator digest equals the settled import locator (:171-200). Keyed on identities and digests.
   - Which native contract applies is keyed on `input.reviewed.contract` (an enum `WindowsHandleContract`) paired with a `PreparedHandleArguments` variant (:815-984); a mismatched pair falls to `_ =>` refusal (:979-983).
   - Branch selection: a Boolean's known value removes the impossible edge (in `successors`, execution file :819-826); `equation` (:2354) folds `IntegerEqual` using a native premise (valid file handle is not 0 or -1, :2388-2402).
   - Handle-valued words are recognized by alias-set contents, not by type or name.
   - `checked_calls` gate: only `WindowsX64`, only with a `permission_policy`, only when the physical policy cannot already classify the trigger, only for `CloseHandle`/`DuplicateHandle` contracts (:2632-2651).
5. **Cases by shape.**
   - `native` handles exactly nine contracts: CreateFile, DuplicateHandle, CloseHandle, QueryFileMode, QueryFinalPath, ReadFile, WriteFile, NoHandleIntervention, CurrentProcess (:815-978; the digest byte table :2711-2720 lists the same nine).
   - Handle scalar role: `allowed_handle` is `Some(1)` for DuplicateHandle, `Some(0)` for Close/QueryFileMode/QueryFinalPath/ReadFile/WriteFile, `None` otherwise (:694-702). Any other scalar argument carrying tracked handle bits refuses (:703-711).
   - `operation` refuses an explicit list of raw/device operation kinds: `StorageAddress`, `EstablishAccessReference`, `ActivateTaskRegister`, `ReadTaskRegister`, `ReloadSegmentDescriptors`, `ReadSegmentSelector`, `GlobalDescriptorTable`, `PortWrite`, `RegisterAccess`, `AtomicAccess`, `AtomicFence` (:1583-1601), comment "no preserving contract for this raw address or device operation".
   - Return shapes: only `Return`, `ReturnUnit`, `ReturnStructural` as a normal return (:1036-1095).
   - Read shapes for a scalar result: byte/element-view length, case membership, byte predicate/equal, `PrimitiveScalarRead`, `ByteSequenceRead`, `ElementViewRead`, then writer-roster read, then an equation (:1809-2086).
   - Bounds: activations < 65,536 (:112), output-census roots < 65,536 (:1231), equation work 65,536 (:2021); `QueryFinalPath` capacity vs buffer bytes/2 (:923-929); transfer window must be a bounded DWORD byte window (:946-952).
6. **Same job elsewhere.** `matches_carrier`/resident-copy presentation rules in `windows_handle_storage.rs` reuse `terminal_semantics::borrowed_referent_presentation`, which is also called in 20 files across psi-to-native, psi/semantics and the interpreter (grepped `fn (matches_carrier|unchanged_storage_presentation|shared_leaf_copy_presentation)|borrowed_referent_presentation`: 38 occurrences in 20 files). The "release every handle on every returning path" ledger duplicates nothing in this unit.
7. **Unused/one-caller.** `checked_calls` is `pub(super)`: 1 call site (`reviewer.rs:323`). `HandleReplayActivations::scalar`, `native`, `enter` are used by `execution` (`windows_handle_replay_execution.rs`) and by this file.
8. **Stated limits.** "Ordinary concurrent namespace deletion remains allowed" (:2693-2694); explicit process exit is "conditional on the entry/runtime's independently delegated domain authority" (:2695-2696); the proof premise list (:2685-2692) is explicit: success handles exclude NULL/INVALID, DuplicateHandle preserves the object, class-16 length-4 mode query retains no output loan, synchronous transfers complete before return, close retires the instance, "closed-process" excludes injection.
9. **Fail-open risks.**
   - `checked_calls` returns `Ok(None)` (no checked leaves, so nothing newly accepted) when target is not Windows, no permission policy, the trigger already classifies, no reviewed call, or the contract is not Close/Duplicate (:2632-2651). Returning `None` leaves the original unknown-mechanism behavior, which is the refusing path. Classification: stated (comment :2616-2619: "Local production without a receiver keeps its existing unknown-mechanism behavior").
   - `NoHandleIntervention`/`CurrentProcess` calls clobber every reachable output interval and accept (:964-978); comment :969-971 gives the reason. Wrong input reaching it: a call whose reviewed contract says it never touches handles but does write a handle into a buffer it was not listed to write; the census of reachable buffers (:765-771) refuses only if tracked bits are already present in those buffers. Classification: stated.
   - `evaluated_operands`/`scalar` fall back to `HandleWordAliases::untracked` for the native status result (:797). A native status integer is declared untracked by assumption of the SDK contract. Classification: stated (premise list :2685-2692).
   - `operation` for a scalar result with no expression and not in the explicit read list falls into the "unsupported scalar observation" branch, which accumulates dependencies instead of refusing (:2024-2084); comment :2036-2038 says the result "cannot become exact HANDLEs". It does refuse for the listed read kinds with no roster (:2025-2035). Classification: stated.
   - `successors` (execution file) selects both edges when a Boolean is `Unknown` (conservative), and one edge when known. No fail-open.

### A2. `PA/windows_handle_replay_execution.rs` (1,421 lines)

1. **Purpose.** The scheduler for the replay. It keeps a worklist of (activation, control position or pending cleanup, suspended call frames, state), merges states that are equal up to numeric knowledge, and walks calls, cleanup actions, returns and branches until every path ends in a return or crash or process exit.
2. **Shape.** `Control` (:21: `Position` or `Cleanup{position, edge, ordinal, completion}`), `Completion` (:15: `Arrival | Return`), `SuspendedCall` (:40), `Work` (:46), `ResourceWork` (:140, Ord by resource key), `Pending` (:216: `rows: BTreeMap<ResourceWork, Arc<Work>>` plus a `VecDeque` queue, `push` :222), `Execution` (:268) with `call` (:300), `intrinsic` (:451), `process_exit` (:469), `cleanup` (:539), `step` (:663), `successors` (:801); `CheckedHandleExecution` (:263); `derive` (:932). Tests at :1029-1420 (4 `#[test]`).
3. **Inputs/outputs.** Reads `WindowsHandleInputs`, `PreparedHandleArguments`. Produces `occurrences: BTreeMap<NativeOccurrence, WindowsHandleContract>` and `external_completions: BTreeSet<(ScalarOccurrence, BoundaryMachineId)>`.
4. **How it decides.**
   - Two worklist entries are the same row if activation, control, suspended frames and the resource part of state (ledger resource state, aliases, references, cases, binding table, and whether each constant slot is evaluated, not its value) compare equal (`compare_resources` :57-96). Numeric values widen to `Unknown` at a join (`join_numbers` :187-213) and use-history unions (`merge_use_history`).
   - Which successors run: `Conditional` takes the edge(s) not proved false (:813-827); `StructuralCase` takes the recorded active case or all cases (:846-867).
   - Process exit recognized by `ExternalCompletionKind::ProcessExit` plus the intrinsic identity `HostedExitProcessI32` (:350-358, :459).
5. **Cases by shape.** Call alternatives: `Machine`, `Boundary`, `Intrinsic` (:311-446). Terminators handled: `InvokeNoNormal`, `ExternalCompletion`, `Return`, `ReturnUnit`, `ReturnStructural`, `EliminateEmpty`, `Crash`, else `successors` which supports only `Conditional`, `Jump`, `StructuralCase` and refuses "no supported original successor form" (:868). Process exit requires exactly one scalar argument, no structural arguments, no normal return (:504-516). Limits: call depth 256 (:313), distinct states 65,536 (:252), total steps 1,048,576 (:994).
6. **Same job elsewhere.** none seen in this unit.
7. **Unused/one-caller.** `derive` is `pub(super)`; called once from `windows_handle_replay.rs:2680` (read).
8. **Stated limits.** "The closed entry returns its process status, not a resource to an independently authorized external caller" (:635-636); entry with parameters refuses (:944-946).
9. **Fail-open risks.**
   - `EliminateEmpty` returns `Ok(())` and does not count as a complete path (:777-781). Comment: "Accepted canonical elimination proves this arrival cannot hold a value." Classification: stated.
   - A crash path calls `returning_path(.., &[])` which checks that no handle is still owed (:782-796); so a crash does not excuse a leak.
   - Process exit path is recorded in `external_completions` and does not run the return-path check (:531-536); the comment (:469-473) says this "remains conditional on the runtime's independently delegated containing-domain authority; this replay grants none". Wrong program: one that calls exit while holding a handle is accepted by this replay and left to the domain authority. Classification: stated.
   - End of `derive` requires at least one complete path (:1009) and requires each Close/Duplicate candidate site to be reached (:1017-1022).

### A3. `PA/windows_handle_scalar_table.rs` (180 lines)

1. **Purpose.** A copy-on-write table of per-value slots (64 per shared page) so many retained replay states can share unchanged scalar knowledge.
2. **Shape.** `HandleScalarTable<T>` (:17) with `namespace: Arc<IdTable<ValueId,()>>`, `pages: Vec<Arc<Vec<T>>>`; `get` (:38), `iter` (:45), `from_table` (:54), `get_mut` (:68), `Ord` impl (:82, compares namespace then pages, `Arc::ptr_eq` shortcut). One test (:119).
3. **In/out.** Built from an `IdTable`; read/written by the replay state.
4. **Decides.** Position is `value.get() - first.get()` from the namespace's first key (:32-36); a value outside the namespace returns `None`.
5-6. none. 7. All methods `pub(super)`; used in `windows_handle_replay.rs` and its execution file (read).
8. none. 9. none seen. (`get_mut` returns `None` for a missing id; callers `expect` after a prior check, `windows_handle_replay.rs:2315-2318`.)

### A4. `PA/windows_handle_storage.rs` (2,122 lines)

1. **Purpose.** Resolves a place plus path to the actual bytes of a byte range, following captured references and views, so the replay can tell which words are aliased. Also holds captured data windows (descriptors), borrowed-argument bindings and tests.
2. **Shape.** `CapturedHandleReference` (:32), `CapturedHandleSequence` (:44; `from_complete_array` :57, `element_range` :89, `byte_scalar_selection` :107, `subslice` :141), `HandleScalarStorage` (:184), `HandleScalarSelection` (:191: `Exact | Family`), `HandlePointerSlot/Kind` (:203/:208), `CapturedHandleStorage` (:214), `HandleStorageBindings` (:231: `Arc<IdTable<PlaceId, Option<BorrowedStorageSlot>>>`, `new` :237, `bind` :267, `get` :289), `HandleReferences` (:323), `HandleStorageGeometry` (:420) with `case_tag` (:516), `inline_byte_carrier` (:559), `pointer_slots` (:649), `scalar_selection` (:737), `as_elements` (:837), `present_sequence` (:874), `sequence` (:914), `location` (:985), `resolve` (:1205), `scalar` (:1359), `capture_reference` (:1383). Free functions `unchanged_storage_presentation` (:434) and `shared_leaf_copy_presentation` (:455). 6 `#[test]` (:1496-2121).
3. **In/out.** Reads `TerminalModule` structural types, `DescriptorMachine`, `ReferenceStorageGeometry` (from `terminal_semantics`), `runtime_abi::target_carrier_layout`, `layout::TerminalRecordStorage`. Produces `(HandleStorageRange, ReferenceStorageRegion, Option<ScalarType>, BTreeSet<HandleInstance>)` tuples.
4. **Decides.** Keyed on structural type ids and path segments (`DescriptorPathSegment`). `resolve` picks, in order: element-view `FixedIndex` projection (:1225-1267), a `Referent` through a captured reference (:1268-1312), a borrowed-root binding (:1314-1346), else the activation's own home (:1347-1356). `unchanged_storage_presentation` accepts a borrowed carrier under another type id only if `borrowed_referent_presentation == Unchanged` (:440-448).
5. **Cases by shape.** `sequence` accepts `FixedArray`, `ByteSequence`, `ElementView` carriers (:944-981), plus "inline byte" field contracts for owned-byte fields (:559-615); `pointer_slots` accepts `Reference`, `ElementView`, `ByteSequence` slot kinds, else "changes its declared slot kind" (:710-725); `present_sequence` accepts `ByteSequence` (bounded or borrowed view only) and `ElementView` (:885-910). Path depth cap 256 (:1218).
6. **Same job elsewhere.** Geometry construction `ReferenceStorageGeometry::new(..).with_target_carriers(..).with_record_storage(..)` is repeated four times in this file (:525-535, :819-826, :1174-1184, plus tests).
7. **Unused/one-caller.** Mostly `pub(super)`; consumers are the replay files. `HandleReferences::get`/`sequence` have call sites in `windows_handle_replay.rs` (read).
8. **Stated limits.** "Constructing a pointer to inline backing needs its own producer" (:432-433); inline owned byte contents "have no retained live data window" (:926).
9. **Fail-open risks.** `scalar_selection` with an unknown runtime index returns a `Family` (whole containing range) instead of refusing (:752-773). That is conservative for aliasing (keeps all dependencies); it refuses when a later `Referent` follows (:753-757). Classification: stated (comment :734-736).

### A5. `PA/windows_handle_values.rs` (895 lines)

1. **Purpose.** Prepares per-machine tables for scalar values (their types, producing position, parameter site, dominator tree) and answers "where does this scalar come from": parameter, expression, entry storage, borrowed entry, stored writes, arrivals, call result.
2. **Shape.** `LocalScalarExpressions` (:28; `available_at` :42), `ScalarDefinition` (:63, 8 variants), `StoredScalarSource` (:96), `HandleValueReader` (:126; `new` :313, `read` :492, `read_storage` :708, `local_expressions` :157, `scalar_actual` :566, `storage_actual` :585, `returns` :660, `require_native_output` :251).
3. **In/out.** Reads `HandleCallGraph`, `DescriptorMachine`, `TerminalScalarExpressions`/`TerminalScalarCaptures`, `reviewed_call`. Produces `ScalarDefinition` values; moves captures out via `into_captures` (:145).
4. **Decides.** Availability is by dominance: a value is usable at a position if its producer is earlier in the same block or its block dominates the use (:42-57). Native output writers are keyed on `(machine, position)` and a fixed parameter index per contract: DuplicateHandle `[3]`, QueryFileMode `[2]`, ReadFile/WriteFile `[2]` (:419-424).
5. **Cases by shape.** `read_storage` yields one of `InactiveCase`, `Stored`, `EntryStorage`, `BorrowedEntry`; requires a unique captured entry referent for borrowed contents, "no unchanged producing event or unique captured entry referent" else refuses (:879-883).
6. **Same job elsewhere.** The native-output parameter table (:419-424) overlaps the contract-specific handling in `windows_handle_replay.rs:694-702, :858, :913, :944, :960` (read; two lists of the same facts about which argument is the handle/output).
7. **Unused/one-caller.** `HandleValueReader` is built in the inputs file (not in this unit); `require_native_output`, `value_position`, `scalar_captures` are `pub(super)`. I did not grep external call sites for each (suspected used by `windows_handle_origins.rs`, not read).
8. **Stated limits.** Header (:1-6): "a returned projection still owes substitution of the actual checked callee... This reader identifies those obligations without declaring them proved."
9. **Fail-open risks.** `ScalarDefinition::InactiveCase` is returned when a payload case is proved impossible at the cut (:735-741). Comment :70-73: "replay must match the caller's actual selected-case guard before it can exclude this returning alternative". Classification: stated.

---

## Part B. Provider correspondence (`PC`) and direct boundary bindings

### B1. `PC/direct-boundaries/bindings.rs` (629 lines; crate `direct-boundary-bindings`)

1. **Purpose.** One row (`DirectBoundaryLeaf`) per directly bound requirement. Checks that a selected native binding matches its published callable, service contract and target ABI, and prepares native-grant trust.
2. **Shape.** `DirectBoundaryLeaf` (:13), `NativeRequirementTrust` (:26), `DirectBoundaryExecution` (:57: `Foreign(Result<ExternalBindingRow, Vec<Diagnostic>>) | Intrinsic(Option<..>)`); methods `matches_any_requirement` (:63), `declared_calling_abi` (:70), `declared_schema` (:89), `validate_plain_execution` (:133), `prepare_native_trust` (:186), `bind_to_terminal` (:238), `validate_calling_custody` (:286), `validate_bound_calling_custody` (:412), `bound_calling_declaration` (:421). One test (:558).
3. **In/out.** Reads `TerminalModule`, `EvaluatedNativeBinding` receipts, `TerminalIdentityTable`. Produces `NativeRequirementTrust` (grant, commitment text, contract digest, plan digest, report identity) or `&BoundaryMachineDeclaration`.
4. **Decides.** Everything is keyed on identity strings and digests: declaration/callable identity text equals the grant's (:202), `plan.target` equals the selected target (:207), package identity equals the grant's subject package (:210), the unique evaluated receipt for target+requirement identity (:150-164), mechanism equality import-locator or syscall-number (:172-181), nonzero contract digest (:220).
5. **Cases by shape.** `bound_calling_declaration` has four branches: numeric primitive, hosted primitive (`HostedOperation::ProcessExit` only, "no exact native execution" otherwise :498-503), selected-ABI row, trait requirement. Numeric leaf must match a `CatalogIntrinsic` target realization (:471-478).
6. **Same job elsewhere.** `numeric_requirement_schema`/`requirement_schema` in `schemas.rs` build the expected schema this file compares (:459, :497).
7. **Unused/one-caller.** `compiler_primitive_schemas`, `hosted_requirement_schema`: 10 occurrences in 7 files (grepped `compiler_primitive_schemas|hosted_requirement_schema`), including stage 01 `lib.rs`, `package_review.rs`, `provider_execution_inputs.rs`, `plan_realization.rs`, `realized_supply.rs`. `bind_to_terminal` returns `Ok(false)` when no Terminal declaration exists (:257-259); comment (:237) says "A retained but unused source declaration may be absent from this module".
8. **Stated limits.** none seen.
9. **Fail-open risks.** `validate_calling_custody` returns `Ok(())` early when the execution is not `Foreign(Ok(..))` (:369-371) after checking the schema method; the hosted/numeric branches check execution in `bound_calling_declaration`. Classification: traced to the second function (:466, :504-506). `bind_to_terminal` `Ok(false)` is stated (:237).

### B2. `PC/direct-boundaries/schemas.rs` (246 lines)

1. **Purpose.** Rebuilds the public contract (a `ServiceSchema`) of a direct primitive (hosted `exit_process`, numeric intrinsics) from the accepted Terminal module, so planning does not need a selected provider to recover it.
2. **Shape.** `hosted_requirement_schema` (:17), `requirement_schema` (:90), `compiler_primitive_schemas` (:192), `numeric_requirement_schema` (:221).
3. **In/out.** Reads `TerminalModule` boundary declarations; returns `ServiceSchema`.
4. **Decides.** Refusal is keyed on a list of fields that must be empty (structural parameters, result regions, lifetimes, requires, qualifications, scalar domain/requires/ensures, crash routes, reference outputs, local-root introductions, content guarantees, plus `!unconditional_termination`, :36-55). Only `HostedOperation::ProcessExit` maps to method name `exit_process` (:29-32).
5. **Cases by shape.** Hosted: only `ProcessExit`; comment (:33-35): "The supported canonical contract has no receiver arguments, structural claim, or progress-profile parameter."
6. **Same job elsewhere.** Numeric and hosted share `requirement_schema` (:89 comment). Operator-kind numeric requirements use the fixed method name `"realize"` (:233).
7. **Unused.** see B1.
8. **Stated limits.** `HostedOperation` other than `ProcessExit`: "no complete canonical schema projection" (:31).
9. **Fail-open risks.** none seen.

### B3. `PC/direct-boundaries/scoped_selection.rs` (167 lines)

1. **Purpose.** Verifies a list of proposed direct-provider leaf positions against the requirements actually reached in an execution scope, then extends the authored selection with those leaves' plans.
2. **Shape.** `VerifiedDirectProviderSelection` (:8), `check` (:20), `into_selection` (:77), `check_nominees` (:96). 2 tests.
3. **Decides.** A boundary in `reach` with no `selected_requirement_row` is "unbound"; a leaf position must be nominated if and only if it matches an unbound requirement (:43-53, :96-111). `into_selection` compares by pointer identity (`std::ptr::eq`, `Arc::ptr_eq`) the module, source, leaves and reach (:84-88).
4-9. Cases: none. Same job: none. Fail-open: none seen.

### B4. `PC/src/bodies.rs` (159 lines)

1. **Purpose.** Joins a selected checked-adapter plan row to the unique checked Terminal body candidate, and reconciles the selected external execution identity with retained numeric/sealed meaning.
2. **Shape.** `CheckedBodySelection` (:28), `select_terminal_checked_body` (:73), `selected_external_execution` (:105).
3. **Decides.** Keyed on requirement, application, callable text, package identity, target (:61-70, :80-96). Needs exactly one candidate (:97-102).
4. **Cases.** `selected_external_execution`: sealed native provider first (:117-131), else numeric provider (:132-158); if neither matches and the retained execution is non-numeric it refuses (:140-146).
5. **Fail-open.** When no candidate matches, `reconcile_numeric_meaning(None, retained)` then `Ok(retained)` (:147-148): a retained numeric meaning with no matching provider is refused inside `reconcile_numeric_meaning` (external_providers.rs:168-169); a retained `None` returns `Ok(None)`. Classification: traced.
6. Same job: `external_providers.rs` functions.
7. Count: `selected_external_execution` re-exported `lib.rs:12-14`; called in `boundary.rs:88` (read).

### B5. `PC/src/bodies/callback_roots.rs` (289 lines) and `callback_roots_tests.rs` (227 lines)

1. **Purpose.** Selects which foreign-registered callback uses become roots of native execution, and finds the three helper machines of each installed interrupt registration.
2. **Shape.** `SelectedCallbackRow` (:12), `CallbackProviderDeclarations` (:18; `prepare` :24, `installed_helper_roots` :74, `selected_row` :180), `select_callback_declarations` (:199), `registration_is_foreign` (:265). Tests: 2 `#[test]` (tests file :147, :197).
3. **Decides.** Per use: `has_checked_entry`, `foreign_registration`, `satisfaction_is_boundary`, and the count of calling-plan matches (:230-253). A checked-entry callback needs exactly one calling plan (:247-253). A checked-adapter provider row must supply a checked body, else error (:277-282).
4. **Cases by shape.** Interrupt helpers: ordinals fixed `[1, 2, 3]`, three helpers per registration (:112, return type `[MachineId; 3]` :81). Duplicate retained registrations turn into errors for that operation (:164-176).
5. **Fail-open.** `registration_is_foreign` returns `Ok(false)` when no selected provider row exists for an `External` registration (:273-275). Wrong program: an external registration with no selected row is treated as a non-foreign registration (so not a root). Classification: open (no comment at the site). Traced partially: `select_callback_declarations` then requires a calling plan only when `foreign_registration && satisfaction_is_boundary` (:235), so a missing row could skip the "explicit selected callback calling plan" error.
6. Same job elsewhere: none seen.

### B6. `PC/src/boundary.rs` (139 lines)

1. **Purpose.** `select_boundary_applications`: for each demanded boundary application, finds the single selected plan row and records how it is realized (compiler intrinsic, specialized checked body, non-generic checked body).
2. **Shape.** `CompilerIntrinsicPlanProvenance` (:17), `select_boundary_applications` (:23).
3. **Decides.** Keyed on schema trait name, package, and row requirement identity text equal to the demand's overload (:59-65); exactly one match (:68). Realization by `row.binding` variant: `CompilerIntrinsic`, `CheckedAdapter`, else refusal (:86-127).
4-5. Fail-open: none seen. Count: used from `lib.rs:17`.

### B7. `PC/src/callbacks.rs` (230 lines)

1. **Purpose.** Types for callback placements and a join check tying a callback thunk to its registrar and body in the published artifact.
2. **Shape.** `NativeCallbackPlacement` (:11), `PrivateCallbackDestinationApplication` (:28), `BoundCallbackPrivateMaterialization` (:55), `TerminalCallbackThunkArtifact` (:70; `for_application` :77), `TerminalCallbackOccurrenceProposal` (:123), `validate_callback_thunk_joins` (:183).
3. **Decides.** Identity: body found by `same_identity_as` against the application body (:85-99); registrar must still be a `BoundaryCall` operation (:196-210); thunk identity must equal `MachineFunctionIdentity::callback_thunk(caller.identity(), placement_index)` (:216-222).
4. Fail-open: none seen. Count: `validate_callback_thunk_joins`: 2 occurrences in 2 files (grep): `verified-optimization-input/.../native_proposal.rs` and this file.

### B8. `PC/src/external_providers.rs` (359 lines)

1. **Purpose.** Matches `ExternalProviderConformance` records (from Terminal) to selected provider plan rows, and maps sealed or numeric intrinsic meaning to `CompilerIntrinsicExecutionIdentity`.
2. **Shape.** `RequirementPlanIdentity` (:13), `selected_numeric_provider` (:76), `matches_external_selection` (:125), `reconcile_numeric_meaning` (:153), `canonical_external_execution` (:175), `sealed_native_execution` (:203), `reconcile_sealed_execution` (:241), `selected_sealed_native_provider` (:251). 2 tests (:305, :319).
3. **Decides.** Header (:1-7): "Display names serve only as consistency checks; exact overload identities, package ownership, binding kinds, and deployment targets establish the requirement association". `matches_requirement_schema` still compares the schema/method/owner display texts (:57-73).
4. **Cases by shape.** `sealed_native_execution`: `Hosted` needs `HostedIntrinsicBundle::from_target_name(target)` (:212); `ProcessExit -> HostedExitProcessI32`, `ConsoleWriteByte -> HostedWriteByteI32`, `ConsoleReadByte` refuses "retired" (:216-224); `FixedImage` only for `FixedImageOperation::TARGET` (:227).
5. **Fail-open.** `matches_external_selection` returns false for missing parts (`let ... else { return false }` :48-50, :57-63). No acceptance on missing data. `reconcile_numeric_meaning` `(None, _, false) => Ok(())` (:168): no meaning and a non-numeric selected execution accepted; stated by the signature (no numeric meaning to reconcile).
6-7. `selected_sealed_native_provider`, `selected_numeric_provider`: called from `bodies.rs` (read).

### B9. `PC/src/installation_reach.rs` (157 lines)

1. **Purpose.** Replaces each selected provider row's conservative upper bound with a resolved concrete reach by following installation-bound dependency edges.
2. **Shape.** `InstallationReachRow` (:13), `resolve_installation_reach` (:23), `completed_row` (:96). 2 tests.
3. **Decides.** Dependency edge keyed on `(requirement_identity text, provider_plan_report_identity)`; if the owning plan has no row, a foreign row is allowed only when exactly one publisher exists across the whole selected closure (:29-60). A dependency with no publisher resolves to `None` which `graphs::reach::resolve` reports as `InvalidDependency` (:80-83, inferred from the error enum; `resolve` is in another crate).
4. Fail-open: none seen. Count: `resolve_installation_reach`: 10 occurrences in 5 files (grep).

### B10. `PC/src/intrinsic_selection.rs` (123 lines)

1. **Purpose.** `visit_selected_intrinsics` matches demanded boundary requirement names to selected compiler-intrinsic rows and the unique Terminal boundary, then calls a visitor.
2. **Shape.** `SelectedIntrinsicRow` (:7), `visit_selected_intrinsics` (:16).
3. **Decides.** Keyed on requirement identity text via binary search on a sorted row list (:71-81, :90-97).
4. **Fail-open.** When zero selected intrinsic rows match a demanded requirement, the loop `continue`s with no diagnostic (:90-97). The demand set is a list of boundaries, most of which are not intrinsics, so silence is the normal case. Classification: traced to purpose (no comment at site). Count: `visit_selected_intrinsics`: 3 occurrences in 3 files (grep): `target-bound-program/builtins.rs`, `numeric-intrinsics/admission.rs`, this file.

### B11. `PC/src/lib.rs` (19 lines): module list and re-exports. Nothing else.

### B12. `PC/src/trust/` (mod.rs 13, accepted_machine_grants.rs 34, admission_settlement.rs 239, provider_grants.rs 121, selected.rs 84, tests.rs 417, trust_admission.rs 194)

1. **Purpose.** Turn authored root grants (provider, native requirement, accepted machine) into `TrustAdmission` values: a human-readable commitment plus a domain-separated 32-byte digest. Package policy decides which kinds of grant the root may make.
2. **Shape.** `reconstruct_trust_obligations` (admission_settlement.rs:57), `RootTrustPolicy` (:41: `Unpackaged | Application([u8;32]) | PackageReview`), `accepted_machine` (accepted_machine_grants.rs:6), `match_provider_grants`/`selected_provider_grants`/`commitment` (provider_grants.rs:37/:21/:109), `SelectedTrustEvidence::admit_root_grants` (selected.rs:15), `TrustAdmission`/`TrustAdmissionDigest` (trust_admission.rs:70/:14). Tests: 12 `#[test]` in tests.rs (:58-:367, counted by reading).
3. **In/out.** In: `TrustDeclarations`, `AuthoredRootGrant`s, provider plans, `NativeRequirementTrust` list. Out: sorted, de-duplicated `Vec<TrustAdmission>`.
4. **Decides.**
   - Policy: `Unpackaged` permits all; `Application(root)` permits individual claims only if `owner == Some(root)`; `PackageReview` permits none (admission_settlement.rs:48-54). Native-requirement and accepted-machine grants check the policy (:97-104, :132-139); provider grants do not.
   - Accepted-machine rejoin: arena index+generation key by binary search; exactly one declaration (accepted_machine_grants.rs:15-33); then declaration identity and package must equal the grant's (admission_settlement.rs:142-148).
   - Provider grant: key `(provider_type, application text)` binary search; exactly one selected plan; plan's provider-type package equals the grant's `subject_package` (provider_grants.rs:83-99).
   - Equality of admissions is `(commitment text, digest)` (trust_admission.rs:176-194). Two grants with the same commitment but different digests refuse; same digest collapses (admission_settlement.rs:227-236).
   - Subject domains `provider-plan`, `native-requirement`, `machine-template`, `machine-contract` (trust_admission.rs:47-54).
5. **Cases by shape.** Three `RootGrantSubject` variants only (admission_settlement.rs:81-172). Accepted machine: template is `Ok(Some)` (generic), `Ok(None)` (use contract), or `Err(cardinality)` (refuse, :149-169).
6. **Same job elsewhere.** `native_grant_evidence` in selected.rs:34-83 and the native-grant branch in admission_settlement.rs:96-122 both pair authored native grants positionally (an iterator advanced in grant order); consistent by construction (filter on the same subject kind). Partly traced.
7. **Unused/one-caller.** `selected_provider_grants` re-exported (selected.rs:2) and used in `provider-admission/src/admission.rs` and `target-providers`? I grepped `admit_selected_checked_providers|matching_folded_constant_dependencies` instead (8 occurrences in 4 files). For `selected_provider_grants`, I did not count; suspected small use.
8. **Stated limits.** "Report fingerprints supply no authority" (trust_admission.rs:5-6).
9. **Fail-open.** `validate_commitment` rejects empty/newline/NUL (trust_admission.rs:77-86). `admission_settlement.rs:79`: a `Provider` grant whose resolved iterator runs dry gives "lost its exact selected application" (refuses). None seen.

---

## Part C. Verification crates

### C1. `PV/published-objects/src/allocation.rs` (194 lines)

1. **Purpose.** Independently replays which callee-saved registers each function writes, from retained allocation inputs, and checks the allocator's claimed requirements equal the replay.
2. **Shape.** `AllocationOriginals` (:17; `retain` :27, `validate_roots` :39), `replay::reconstruct` (:107), `validation::validate_allocated_callee_saved_requirements` (:163).
3. **Decides.** Roots checked by identity equality (home identity, manifest identity, selected, environment, target, preservation identity, :39-54, :170-177). Only one policy accepted: `AllocatedSelectedWritesIntersectAbiPreservationV1` (:112-116). Replay usage must equal the candidate's and be within the budget (:138-145, :180-182).
4. Fail-open: none seen. Count: `validate_allocated_callee_saved_requirements` used from `lib.rs:420`.

### C2. `PV/published-objects/src/lib.rs` (555 lines)

1. **Purpose.** Audit entry points: replay retained stage inputs against an emitted object only when a validator was requested.
2. **Shape.** `RetainedPhysicalInputs` (:18; `belongs_to` :30), `NativeStageReplay` (:87; `from_bodies` :97, `with_components` :121, `component` :146, `check_bodies` :159), `verify_retained_fixed_frame` (:183), `verify_body_fixed_frame` (:196), `verify_physical_origins` (:246), `verify_retained_selected_optimization` (:323), `verify_body_selected_optimization` (:334), `verify_retained_target_operations` (:344), `retained_target` (:390), `mod physical` (:418; `verify_originals` :440, `verify_selected_optimization_replay` :532, `VerificationError` :545).
3. **Decides.** All joins by identity/pointer: encoding `Arc::ptr_eq` (:71), machine identity (:70), manifest identity (:402-409). Missing originals refuse, never pass (:81-83). `verify_selected_optimization_replay`: with a run -> validate; no run and record not rewritten -> `Ok(false)`; no run but rewritten -> `ReplayUnavailable` (:535-542).
5. **Cases by shape.** Component bodies are matched by slot commitment plus object check (:146-157); private functions looked up by identity bytes (:169-175).
8. **Stated limits.** Header: "Only a compile that requested an audit retains those inputs; without them each audit refuses rather than passing."
9. **Fail-open.** `verify_body_selected_optimization` `Ok(false)` ("no rewrite ran") is not a pass: `StageValidatorRequest::check` reports it as `Inapplicable` (`stage_validators.rs:192-194`). Classification: stated (doc :320-322). `verify_retained_target_operations` for a component body is called with no originals for the component (stage_validators.rs:88) and requires `fragment_replay()` on that object (:350-352). Traced.

### C3. `PV/runtime-publication/publication.rs` (297 lines)

1. **Purpose.** Holds the product handed to the interpreter for each independent native component and checks the roster covers each admitted independent call.
2. **Shape.** `RuntimeComponent` (:22; `admit` :35, `validate_same_stack` :123), `RuntimeExecutionProvisions` (:140), `validate_component_roster` (:214), `component_indices` (:250).
3. **Decides.** `admit` compares ~13 identities/commitments at once (:55-75): provider, entry, artifact identity bytes, target, commitment recompute, stack demand's psi/entry/target, export slot commitment, power-of-two alignment. `component_indices` finds exactly one matching component per host provision by receiver, receiving boundary, provider, candidate, locator, application commitment, entry plan (:272-284); more than one is ambiguous (:285-289). Roster check: DFS with cycle detection and requires every supplied image to be consumed (:223-246).
4. Fail-open: `component_indices` skips provisions whose realization is not `NormalizedForeignCall`/`Component` with `continue` (:257-271). Reason: those are not independent component calls. Classification: traced to purpose. `RuntimeComponent::admit`: 4 occurrences in 3 files (grep `RuntimeComponent::admit|validate_component_roster`).

### C4. `PV/stage_validators.rs` (296 lines)

1. **Purpose.** The registered post-compilation validators and their request/result types.
2. **Shape.** `StageValidator` (:9, 3 variants), `ROSTER` (:21), `check` (:51), `StageValidatorRequest` (:105; `from_exact_names` :113, `stages_not_produced` :148, `check` :179), `NativeValidationError` (:230), `StageValidationRow` (:265).
3. **Decides.** A validator is selected by exact name string (:44-49); unknown name rejects (:119-128). A named validator that comes back `Inapplicable` is an error (:210-222). `Ok(false)` -> `Inapplicable("no selected-instruction rewrite rule ran")` (:192-194) (this reason text is used for any validator returning false; only the third can).
8. **Stated limit.** The Display of the third validator prints "producer repeatability only; independent rewrite validation not performed" (:282-286).
9. **Fail-open.** With `--every-validator` and no named validator, an `Inapplicable` row is not an error (:210-213 check `self.named.contains`). Classification: stated (doc :146-147). Count: `StageValidatorRequest::from_exact_names|stages_not_produced`: 10 occurrences in 6 files.

### C5. `PV/target-providers/synchronous_invocations.rs` (358 lines) and `target_providers.rs` (1,302 lines)

1. **Purpose.** Bind Build-selected provider plans to Terminal's candidate rosters, close checked specializations, reject cyclic synchronous `invokes` graphs, and check folded-constant dependencies match the selected plans.
2. **Shape (synchronous_invocations.rs).** `validate_declared_synchronous_invocation_cycles` (:11), `SynchronousBoundaryTarget` (:65: `Application | Selected`), `validate_selected_synchronous_invocation_cycles` (:127), `exact_selected_synchronous_plans` (:253), `synchronous_cycle_from` (:308, 3-color DFS), `exact_authored_invocations` (:331).
2. **Shape (target_providers.rs).** `TerminalTargetProviderProposal`/`Selection` (:16/:26), `selected_checked_adapter_header` (:41), `selected_checked_invocation_rows` (:105), `TerminalTargetProviderSelection::{from_plans, admit}` (:170/:183), `admit_selected_checked_providers` (:206), `admit_checked_roster` (:220), `validate_synchronous_calls` (:257), `matching_folded_constant_dependencies` (:333), `validate_selected_service_schemas` (:412), `checked_candidate_roster` (:469), `close_checked_provider_specializations` (:491), `selected_checked_candidates` (:552), `selected_provider_candidate_rows` (:593). 17 `#[test]` (read; tests at :819-1301).
3. **Decides.**
   - Cycle edges: a plan row's targets come from checked headers for `CheckedAdapter` rows and from authored `Service(name)` entries (name-only `Application{package: None, application: None}`) for other rows (synchronous_invocations.rs:180-192). A target matches a plan by plan digest, or by application text + package, or by declaration display text with optional package (:78-113). Display text match with `package: None` is a string compare on the trait name (:99-111).
   - A target matching zero selected plans adds no edge (`[] => {}` :207); one plan adds an edge; several is "ambiguous across N package-qualified boundary slots" (:212-218).
   - Selected plan target must be nonempty (`selected_plan_target` target_providers.rs:34-39) and equal the requested deployment (:226-229).
   - Candidate headers must be `product_eligible`, `supplies_checked_body`, package-equal, target-equal-or-absent, callable identity equal to the candidate's (:141-153).
   - Folded constants: exactly one matching normalized plan per dependency (:398-402).
5. **Cases by shape.** `check_synchronous_calls` in `validate_synchronous_calls` treats a boundary with no supplying slot as "no edge" (`[] => {}` :305).
6. **Same job elsewhere.** `selected_checked_adapter_header` (:41) and `selected_checked_invocation_rows` (:105) both look up a callable header by identity; one uses `callable_headers` filtered by callable text (:61-78), the other by declaration association (:127-134).
7. **Count.** `validate_declared_synchronous_invocation_cycles|validate_selected_synchronous_invocation_cycles`: 8 occurrences in 2 files. `admit_selected_checked_providers|matching_folded_constant_dependencies`: 8 occurrences in 4 files.
9. **Fail-open.**
   - An invoked target with no matching selected plan adds no cycle edge (synchronous_invocations.rs:207; target_providers.rs:305). Wrong program: a closed provider that invokes a requirement supplied by no selected slot is not part of any cycle check. Classification: open (no comment at either site); partly explained by the second function header (:1-6: "preserving declared ceilings on opaque rows").
   - Display-name matching: `SynchronousBoundaryTarget::matches` with `application: None` compares trait display text (:99-111) and, for non-checked rows, `package: None` makes owner irrelevant. Wrong program: two plans from different packages sharing a trait display text produce the "ambiguous" error, not a silent pass. Classification: traced.

---

## Part D. Psi access vocabulary and admission (`ACC`)

### D1. `ACC/borrows.rs` (121), `capabilities.rs` (137), `claims.rs` (32), `cleanup.rs` (82), `declarations.rs` (9), `mod.rs` (34), `placement.rs` (325)

Same kind of thing: data definitions in the Psi program record for ownership, loans, cleanup and access views. Short entries:

- `borrows.rs`: types `TerminalReborrowRestoredCallUse` (:57, 21 fields describing a restored reborrow at a call), `TerminalBorrowPlace`/`Segment`/`OwnerSegment`, `RetainedBorrowCustody` (:83), `RetainedBorrowPlace`. No logic.
- `capabilities.rs`: `StructuralMultiplicity` (:2), `StructuralAccess` (:9: Owned, SharedBorrow, MutableBorrow, WriteOnlyBorrow), pattern macros (:16-52), `StructuralAccess::admits` (:56, `self != Owned && covers(granted, self)`), `admits_referent` (:61), `structural_access_covers` (:69: equal, or available is Owned, or available is MutableBorrow and requested is Shared/WriteOnly). Table tests pin all 16 pairs (:90-135). Doc :66-68: "Compares access modes without establishing custody, lifetime, or storage validity." Count: `structural_access_covers` is re-exported in `mod.rs:21`; I did not grep its external callers.
- `claims.rs`: `LoanCompletion`, `EntryClaim`, `ClaimTransfer`, `CompletionReceipt` (data).
- `cleanup.rs`: `TerminalAffineCleanupAction` (:52: `DiscardRoot | DiscardResidual | InvokeOwned | RetireHome`), `OwnedCleanupInvocation` (:26; `module_invocation` :35 builds `ModuleInvocationInputs` with `required_contract: None` and empty `completion_receipts`), `NominalCleanupSelection` (:17).
- `declarations.rs`: `AccessDeclarations` (4 vectors).
- `mod.rs`: module list; comment (:1-5). Re-exports at :15-34.
- `placement.rs`: error enums `AccessViewInputError`, `AccessViewPlanError` (:46, 22 variants), `TerminalAccessViewInput` (:18), `canonical_access_view_identity` (:37: `format!("access-view:{}:{policy}:{}:{schema}", len, len)`, a string key), `TerminalAccessViewPlan` (:79), `TerminalAccessLifecyclePlan` (:91), `TerminalAccessStorage` (:122; `same_custody` :144, `owner_storage_path` :168, `owned_content` :195), `TerminalAccessLifecycleOperation::profile_operands` (:235, fixed table for Initialize/MoveOut/View/Relocate), `TerminalAccessor`, `AccessRamCorrespondence`, `AccessorScalarBoundaryDemand`.
  - Judgment: `TerminalAccessStorage::same_custody` ignores the `singleton` domain of a `Run` owner (:145-166), so two storages that differ only in `singleton` are "same custody". 
  - Fail-open risks: none seen (data only; `owned_content` errors when vacant/resident missing :204-207).

### D2. `ACC/extents/src/lib.rs` (125 lines)

1. **Purpose.** Nonzero u64 identity newtypes (via macro `normalized_extent_identity!` :17-41, 29 uses at :43-111), `ExtentContentInterpretation` (:66) and `LoanPolarity` (:122). Doc: "None of these values issues, loans, maps or mutates live storage authority."
2. Decides: zero identity rejects (:112-119). Both `from_normalized_identity` and the `Record::read` path reject zero.
3. Fail-open: none seen. Count: not grepped.

### D3. `ACC/policies/` (7 files, 474 lines): `access.rs` (25), `access_operations.rs` (137), `boundary_reach.rs` (43), `diagnostic.rs` (10), `fields.rs` (141), `normalized_identity.rs` (30), `resources.rs` (88)

Descriptive values. Highlights:
- `AccessOperation::reads_and_writes` (`access_operations.rs:31-47`): `Read` and atomic `Load` read only; `Write` and atomic `Store` write only; `Take`, `CompoundMutation`, atomic RMW/Swap/CAS read and write.
- `AccessOperation::receiver_polarity` (:49): `Read -> Shared`, `Take|Write|CompoundMutation -> Exclusive`, `Atomic(op) -> op.receiver_polarity()` which is always `Shared` (:83-85). A comment at :28-30 says atomic ordering and the loan "remain separate checks".
- `AtomicPermissions` (12 booleans, `fields.rs:24-37`) with `any` and `contains` (:40-68). Note `FieldAccess::operation_names` returns an empty list for `Atomic` (:120).
- `ExternalCapability`, `AtomicCapability`, `StableCapability::permits` (`resources.rs:11-14`), `ResourceRegion` (offset, length, peer, stable, external, atomic, reach), `ResourceProfile`.
- `BoundaryReach` (`boundary_reach.rs:11`, a `BTreeSet<BoundaryServiceReachId>`): `contains_all` is subset (:30-32).
- No fail-open risks seen. `ResourceProfileReceiptId` is declared in `access.rs:22-25` (grep not run for use).

### D4. `ACC/validated-placements/` (access_fields.rs 524, admission.rs 32, plan.rs 285, plan_policy.rs 905, resources.rs 800, test_support.rs 151, tests.rs 100)

1. **Purpose.** Turn authored per-field access decisions plus a layout report into a sealed `ValidatedPlacement` (geometry, exact transfer containers, overlap rules, identity fingerprints, content-interpretation commitment), and check a placement against a backing resource profile (supplied stable/external/atomic regions, reach, base alignment congruence).
2. **Shape.**
   - `access_fields.rs`: `AccessFields` (:68; `inaccessible` :84, `set` :155), `AccessFieldKey` (:34, layout fingerprint + 32-byte commitment + slot), `FieldAccessDescriptor`, `LogicalFieldExtent`/`Fragment`, `RelativeEffectFootprint` (:248, `overlaps` :266), `ValidatedAccessFields` (:334), `FieldRegion` (:345; `scalar_access` :388), `validate_region_projection` (:471).
   - `plan.rs`: `AccessPlan<Region, Caller>` (:11, `try_bind` :48), `normalize_access_plan` (:117), `ValidatedPlacement` (:188), `validate_placement` (:221), `PlacementSnapshot` (:247, `validate` :270).
   - `plan_policy.rs`: `access_fields_validation::validate_access_fields` (:20), `validate_entry_policy` (:330), `validate_entry_geometry` (:421), `normalized_identities` (:567: fingerprints and the authoritative commitments).
   - `resources.rs`: `resource_compatibility` (`validate_access_resources` :195, `require_stable_region_supply` :81, `select_effective_supply` :329, `require_base_congruence` :435), `resource_profile` (`restrict` :503), `resource_profile_validation::validate_resource_profile` (:549), `run_relocation::validate_run_relocation_resources` (:709).
3. **In/out.** In: `AccessFields`, `LayoutPlanReport`, `BoundaryReach`, `ValidatedResourceProfile`. Out: `ValidatedPlacement`, `ValidatedAccessFields`, `AccessResourceCompatibility` (with `BaseCongruence {modulus, residue}`), `ValidatedResourceProfile`.
4. **How it decides.**
   - Field identity is `member_identity` when present, else the positional field name; a name that maps to two identities or an identity to two names refuses (`access_fields.rs:92-114`). Keys carry the layout fingerprint AND a SHA-256 commitment so a key from another layout is refused (`set` :160-166; test `tests.rs:65`).
   - Retained layout is replayed structurally, not only by fingerprint (`plan_policy.rs:28-35`; test `tests.rs:36`).
   - Structural `Region` fields need: at least one permission, no atomic, not Atomic observation, no `take` under Stable, not both read and take, nonzero power-of-two tiling by 8-128-bit whole-byte transfers, exactly one `At` placement (`plan_policy.rs:63-101`).
   - Other fields: width 8..=128 bits and a multiple of 8 (:409-417); geometry from layout placements: single `At`, single `IntegerAt` (stored width must equal transfer width), or `Bits` fragments sharing one container with container width equal to transfer width (:440-543).
   - Destructive (take) units must cover their whole transfer container and overlap no other field (:247-276, :140-157, :159-183); External write must cover the whole container (:228-245); two atomic fields overlapping must have identical footprints (:202-226).
   - Resource side: field `External` demand is satisfied by an `External` capability (read behaviour compatible: Read needs Repeatable, Take needs Destructive; width rule) or falls back to a Stable region permitting it unless the demand is `Take` (`resources.rs:357-386`); Atomic needs a matching width rule whose operations contain the demanded set (:397-408). Alignment is accumulated as a congruence modulo a power of two and conflicting residues refuse (:435-467). A placement's reach must be a subset of the supplying region's reach (:239, :118).
   - Identities: `PlacementId`, `AccessFieldsId`, `ResourceProfileId` are FNV-1a hashes with `max(1)` (`plan_policy.rs:691-703, :822-877, :891-904`), named `non_authoritative_*`. The authoritative identity is a domain-separated digest over layout, entries and reach (:705-737) stored as `content_interpretation` (`plan.rs:242`).
5. **Cases by shape.** `FieldAccess` variants: `Inaccessible, Region, Stable, External, Atomic` (`fields.rs:88-115`); `validate_entry_policy` rejects `Region` as "not a primitive transfer policy" (:335-339). `validate_run_relocation_resources` samples cells: first, second, last, and cells adjacent to each destination-region boundary (:752-777) rather than every cell.
6. **Same job elsewhere.** `AtomicPermissions` bits are listed three times (`fields.rs:41-52, :56-67`; `plan_policy.rs:609-624`). Two different hash families for placement identity (FNV and SHA-256 commit) are in the same file.
7. **Unused/one-caller.** `PlacementSnapshot::validate`: not grepped. `validate_region_projection` (access_fields.rs:471): not grepped. Header comment of `plan_policy.rs` (:1-6) lists `access_operations`, `authorization`, `boundary_reach` sub-modules that do not exist in the file (only `access_fields_validation` and `normalized_identities` are present).
8. **Stated limits.** `normalized_identity` fingerprints are named "non_authoritative". `ADDRESS-EXPOSURE` task (from the crate map, not re-read).
9. **Fail-open risks.**
   - `normalize_access_plan` `callers` closure indexes `payload[0]` and `payload[1]` after only checking that no inner vector is empty (`plan.rs:122-124`). A payload with fewer than two entries would panic, not pass. Classification: open (panic, not acceptance); depends on `CallableDescription` shape (not read).
   - `require_stable_region_supply` assumes `profile.regions` sorted and uses unchecked `region.offset + region.length` (`resources.rs:98`); `validate_resource_profile` sorts and range-checks regions (`:553-563`), so a `ValidatedResourceProfile` is safe. Classification: traced.
   - `validate_entry_geometry` computes `offset * 8` and `candidate * 8 + destination_lsb` unchecked (`plan_policy.rs:448, :520`) after `validate_transfer_range` for the `At`/`IntegerAt` cases only (the range check is after the multiplication for the `At` case at :443-448). Overflow behavior in release is wrapping; reaching it needs a layout offset above 2^61. Classification: open.
   - An External-demand field accepted by Stable supply (`resources.rs:376-382`): wrong program would be a field declared External (volatile device semantic) backed by a plain Stable region. Classification: open (no comment at the site); the doc for the validator is the file header only.
   - `PlacementSnapshot::validate` rebuilds from an inaccessible seed and re-validates (`plan.rs:270-284`); not a bypass.

---

## Part E. Psi artifact: authorization and callable encoding (`ART`)

### E1. `ART/authorization/authorization.rs` (213 lines)

1. **Purpose.** Pairs a decoded canonical artifact with the receiving host's decision to trust the producing process, so execution/optimization/native lowering use only an authorized artifact.
2. **Shape.** `HostProducerTrust` (:18, `accept` :28), `AuthorizedTerminalArtifact` (:50), `ArtifactTrustError` (:103), `authorize_terminal_artifact` (:133). 3 tests.
3. **Decides.** Trust identity is pointer identity of an `Arc<str>` (`PartialEq` :35-39, `authorized_by` :72-74): two `HostProducerTrust::accept("same text")` values are different. Artifact match is `manifest().identity()` equality (:77-90). `producer = None` refuses (:137).
7. **Count.** `authorize_terminal_artifact|HostProducerTrust`: 51 occurrences in 15 files (grep).
8. Doc (:129-131): "Absence of producer trust still permits inspection of `artifact`".
9. Fail-open: none seen.

### E2. `ART/authorization/selection_questions.rs` (953 lines) and `selection_questions/tests.rs` (667 lines)

1. **Purpose.** A program may use only declarations its package can select. This joins the source-issued "selection questions" (each read of a declaration, with its purpose Build or Product) to the actual calls and descriptor constructions in the published program, then checks package permissions per question.
2. **Shape.** `InvocationSelectionConsent` (:19), `ProductSchemaCustody` (:29; `from_selections` :44, `requester` :88, `entry_requester` :96), `SelectionAuthorities` (:143), `actual_calls` (:231), `validate_selection_questions` (:393), `admit_selection_questions` (:836), `admit_selection_questions_with_consent` (:846), `admit_questions` (:860). Tests: 7 `#[test]` (tests.rs :65, :112, :153, :409, :492, :614 and the helper fns).
3. **Decides.**
   - Calls are keyed by `(MachineId, CallableInvocationSite)` sorted, each covered exactly once (:303-306, :316-329, :714-716). Descriptor constructions keyed by `(machine, operation)`.
   - Machine origin: a machine may start one source declaration at one head, except two heads where one is the machine's entry (folded cycle) (:574-593).
   - Each machine must have an origin at its entry whose declaration identity text equals the checked semantic declaration (:601-617).
   - Every boundary machine must have a source origin (:618-624).
   - `admit_questions` (:860-953): requester custody must be present (`require_declaration_selection(requester.custody, requester.custody, None)`) unless `requester_consented` (`UnownedUser` custody, Build purpose, invocation-scope table present, Build question or deferred Product input, and the consent object allows that exact question, :885-889). For a selected declaration: same-package uses the question's purpose, cross-package requires `Product` (:914-920); wrong-purpose package declaration refuses (:921-927); consented selections allow `UnownedUser`/Build or `Toolchain` owners (:928-938); otherwise `require_declaration_selection` with the authority for the purpose (:939-945). `Intrinsic` selection needs a toolchain authority (:903-910).
   - Keyed on strings: `row.identity.as_str()` vs `module.identities.text(..)` (:380, :542, :560, :611, :680).
5. **Cases by shape.** `SelectionReference` variants: `Declaration`, `RuntimeContract`, `ProductInput`, `Intrinsic`, `LocalBinding` (:423-467). Call actual targets: module invocations, callable invocations, boundary invocations, cleanup actions (:231-308). Join variants handled: `Machine`, `Boundary`, `Structure`, `Invocation`, `Descriptor`, `FoldedCall`; other variants fall to `_ => ()` (:568, :786).
6. **Same job elsewhere.** `ProductSchemaCustody` (:29-129) is used by 07 (`source_selection_publication.rs` etc., see the count). Not duplicated here.
7. **Count.** `admit_selection_questions|validate_selection_questions|ProductSchemaCustody`: 52 occurrences in 8 files (source-to-psi stage 07, this file, tests).
9. **Fail-open.**
   - `admit_questions` does no declaration check for `ProductInput`, `RuntimeContract`, `LocalBinding` selected references (:899-902 `=> ()`), beyond requester custody (:892-898). Classification: stated (comment :890-891: "A local binder or deferred input still needs its exact Build requester's consent; neither creates missing package custody.").
   - Join variants other than the six listed are ignored (`_ => ()`, :568, :786). Wrong input: a variant added to `SelectionJoin` later would be skipped silently by both loops. Classification: open.
   - Exemption from the "published executable question needs an emitted association" check applies only to rows whose `selected` is `Declaration|RuntimeContract` and role `Executable|MachineValue` (:820-831); other roles/selected kinds are never required to have a join. Classification: open.
   - `normalize` of `ProductDesignation`: "Product designation has no actual Build boundary occurrence" refuses (:813-819).

### E3. `ART/callable-encoding/src/` (catalogs.rs 156, certificates.rs 677, contracts.rs 126, evidence.rs 37, lib.rs 15, machine_declarations.rs 106, proof_declarations.rs 104)

1. **Purpose.** Canonical byte encodings (declarative `wire_record!`/`wire_enum!` tables) for callable descriptors: callable catalog, machine contract, evidence lanes, machine declaration, proposition declarations, and proof certificates in postorder rows.
2. **Shape.**
   - `catalogs.rs`: `Catalog`, `Contract`, `Descriptor`, `Envelope`, `Invocation`, `Termination` records (:23-89); `encode_callable_catalog` (:93), `decode_callable_catalog` (:99, via `decode_canonical(bytes, Catalog::read, encode_callable_catalog)` i.e. decode then re-encode and compare).
   - `contracts.rs`: `Contract: MachineContract` (30 fields, :16-48), `Membership`, `Termination` enums, `encode/decode_proof_formals` (:76/:88).
   - `evidence.rs`: `Term`, `Lane`, `CallableOwner`, `LaneKind`, `Subject` (:10-37).
   - `machine_declarations.rs`: `encode` (:9), `decode` (:40), `validate` (:87).
   - `proof_declarations.rs`: declarations, applications, `BinderArgument` field (:69-104).
   - `certificates.rs`: `canonical_certificate_bytes` (:14), `CertificateTable` (:63, `children` :69, `write_row` :207), `write_proof_row` (:219), `Certificate` field (:367), `read_proof_row` (:453).
3. **Decides.** Wire table sizes in generics (e.g. `List<Contract, 54>`, `List<Descriptor, 38>` in `catalogs.rs:25-29`) are per-element minimum byte sizes for count bounds (inferred from `bounded_count(reader, N)` usage in certificates.rs:398, :404). Zero commitments in a machine declaration refuse (`machine_declarations.rs:88-103`). Absent type identity in erased proof formals refuses (`contracts.rs:92-96`). Unknown tags: `InvalidTag` or `NonCanonicalEncoding`.
4. **Cases by shape (certificates.rs).**
   - `write_proof_row` is an exhaustive match over `ProofRule` and writes tags 0-32 (:219-362): in particular 28 `IntegerPowerOfTwoOrder`, 29 `IeeeFloatOrderChain`, 30 `FloatMeaningNormalization`, 31 `FloatIntegerConversion`, 32 `IntegerFloatExact`, and `PrimitiveJudgment` codes 4 (`ClosedIeeeFloatRelation`) and 5 (`ClosedFloatIntegerRelation`) (:235-242).
   - `read_proof_row` handles only tags 0-27 (:458-674); tags 28-32 fall to `_ => Err(NonCanonicalEncoding)` (:675), and primitive judgments 4 and 5 fall to `_ => return Err(NonCanonicalEncoding)` (:481). So a certificate using those rules encodes but cannot be decoded. I grepped the six rule names under `psi/artifact/` and found no other certificate reader (only `certificates.rs` and `obligation_evidence.rs` re-exports and encode-side tests); `canonical_certificate_bytes`: 24 occurrences in 3 files (`certificates.rs:1`, `wire/src/lib.rs:1`, `wire/src/obligation_evidence.rs:22`), all encode-side. Whether a decoded program can hold such certificates was not traced (the decoders in `psi/artifact/wire/foundation_validation.rs` also name these rules; not read).
   - `CertificateTable::children` ends with `_ => {}` (:203) for rules with no premises; the comment on `encode_proof_node` (:55-58) says "The match is exhaustive: a new rule changes this encoding deliberately", which is true of `write_proof_row` only.
5. **Same job elsewhere.** The postorder table encoder `encode_term_table` is shared with propositions (`psi_declaration_encoding::terms::table`, not read). Machine declaration encoding is also used in the callable `Contract` (:19).
6. **Unused/one-caller.** `canonical_certificate_bytes`: see above. `encode_proof_formals`: used by `contracts.rs:110` field table (read).
7. **Stated limits.** "A complete ordinary certificate, including its current proof-system marker" (certificates.rs:366).
8. **Fail-open risks.** `one()`/`two()`/`children.next().expect(..)` rely on the table reader to have checked child counts (certificates.rs:432-452); comments say so. A bad count is meant to be caught by `TableChildren::take` (not read). Classification: stated, not traced.

---

## Coverage

Full reads (every line, in order), line counts from `units/unit_037.tsv`:
- Group A (PA): windows_handle_replay.rs 1-2819, windows_handle_replay_execution.rs 1-1421, windows_handle_scalar_table.rs 1-180, windows_handle_storage.rs 1-2122, windows_handle_values.rs 1-895 = 7,437.
- Group B/C (psi-to-native verification): direct-boundaries bindings 1-629, schemas 1-246, scoped_selection 1-167; provider-correspondence src bodies 1-159, callback_roots 1-289, callback_roots_tests 1-227, boundary 1-139, callbacks 1-230, external_providers 1-359, installation_reach 1-157, intrinsic_selection 1-123, lib 1-19, trust (accepted_machine_grants 1-34, admission_settlement 1-239, mod 1-13, provider_grants 1-121, selected 1-84, tests 1-417, trust_admission 1-194); published-objects allocation 1-194, lib 1-555; runtime-publication/publication.rs 1-297; stage_validators.rs 1-296; target-providers synchronous_invocations 1-358, target_providers 1-1302 = 7,477 (note: counted to 7,480 by file sum below).
- Group D (psi/access): borrows 121, capabilities 137, claims 32, cleanup 82, declarations 9, extents/lib 125, mod 34, placement 325, policies (access 25, access_operations 137, boundary_reach 43, diagnostic 10, fields 141, normalized_identity 30, resources 88), validated-placements (access_fields 524, admission 32, plan 285, plan_policy 905, resources 800, test_support 151, tests 100) = 5,114 approx.
- Group E (psi/artifact): authorization 213, selection_questions 953, tests 667, callable-encoding (catalogs 156, certificates 677, contracts 126, evidence 37, lib 15, machine_declarations 106, proof_declarations 104) = 3,060.
Total unit lines: 21,475 (from `awk` sum of the tsv fourth column). All ranges in the tsv were read in full. Skimmed: none. Not read: none.
(Line-count arithmetic above is approximate for the group subtotals; the whole-unit total is the awk figure.)

Four files were read in two or three Read calls or by `cat -n` through Bash where the tool output was persisted and cut (bodies.rs lines 1-27, trust/mod.rs and accepted_machine_grants.rs lines 1-28 came from the preview of `cat -n`, the remainder by Read). No range was left unseen.

## Five things a maintainer most needs to know about this unit

1. **The certificate codec writes more rules than it reads.** `write_proof_row` emits tags 28-32 and primitive-judgment codes 4-5 (`callable-encoding/src/certificates.rs:235-242, :351-361, :221, :223`), while `read_proof_row` accepts only tags 0-27 and judgments 0-3 and otherwise returns `NonCanonicalEncoding` (:481, :675). A callable descriptor's `refinement_evidence` containing a float-order, float-meaning, float-integer or power-of-two certificate would encode and fail to decode. I found no second reader under `psi/artifact/`; I did not trace whether such certificates can reach a descriptor.
2. **The Windows handle proof is a whole-program path replay with its own bounded state machine** (about 7,400 lines in five files) that exists to let `CloseHandle`/`DuplicateHandle` calls be classified. It fires only for Windows x64 with a permission policy, only for those two contracts, and its entry is a single call site (`reviewer.rs:323`). Its conclusions rest on an explicit SDK/process premise list (`windows_handle_replay.rs:2685-2698`). Process exit is deliberately not checked for leaked handles (`windows_handle_replay_execution.rs:469-473, :531-536`).
3. **Provider identity is keyed on identity text and digests, with some display-name compares.** Trust grants, direct leaves and plan rows join on identity strings plus package digests; the synchronous-cycle check matches targets by plan digest, application text, or trait display text with an optional package (`synchronous_invocations.rs:78-113`), and a target matching no selected plan adds no edge (:207, `target_providers.rs:305`).
4. **Placement and resource admission keep two identity families side by side.** FNV-1a ids named `non_authoritative_*` are used for compatibility ids; the SHA-256 commitment (`authoritative_access_interpretation`) is stored as the placement's content interpretation (`plan_policy.rs:691-737`, `plan.rs:228-243`). `External` field demands may be satisfied by plain Stable supply unless the demand is `Take` (`resources.rs:376-382`).
5. **Stage validators are an opt-in audit with three fixed validators**; without retained originals each refuses rather than passes (`published-objects/src/lib.rs:81-83`), `Ok(false)` from the rewrite replay becomes `Inapplicable`, not `Passed` (`stage_validators.rs:192-194`), and a passed rewrite validator is documented as "producer repeatability only" (:282-286).

## What you are unsure about

- Whether any real program can contain a certificate with the tags the reader rejects (item 1); the decoders in `psi/artifact/wire/foundation_validation.rs` and `psi/proofs/certificates.rs` were not read.
- Whether `CallableDescription`'s payload always has at least two entries (`plan.rs:122-124`); not read.
- `reviewer.rs` (caller of `checked_calls`), `windows_handle_inputs.rs`, `windows_handle_calls.rs`, `windows_handle_lifetimes.rs`, `windows_handle_aliases.rs`, `windows_handle_arguments.rs`, `windows_handle_contracts.rs`, `windows_handle_origins.rs`: not in this unit; behavior of the ledger (`merge_use_history`, `returning_path`, `caller_continuation`) is inferred from call sites only.
- `graphs::reach::resolve` (used by `installation_reach.rs`): not read; the `Unresolved`/`InvalidDependency` mapping is inferred from the match arms (:72-83).
- External call-site counts for several `pub` items (`structural_access_covers`, `PlacementSnapshot::validate`, `validate_region_projection`, `selected_provider_grants`, `ResourceProfileReceiptId`) were not grepped.
- The group subtotals in the Coverage section are approximate; only the whole-unit sum (21,475) comes from a command.
