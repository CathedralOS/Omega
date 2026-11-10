Second pass for unit 112

Ranges covered by this pass (from gap_029.tsv):
- `omega-rust/psi/interpreter/terminal-interpreter/src/engine_execution.rs` lines 1-3900 (3,900 lines; the whole file; it starts at the file head and ends at the file end, so no range begins or ends inside a function)

All paths below are relative to `omega-rust/psi/interpreter/terminal-interpreter/src/` unless they start with `psi/` or `omega/`.

The earlier notes (unit_112.md) listed this file as NOT READ. Nothing in my range contradicts them. One small match: they inferred `dispatched_operation!` is used by `execute_*` methods dispatched from this file; confirmed (macro defined `engine_execution.rs:2077`, dispatch table `:2823-2887`).

---

## engine_execution.rs (1-3900)

The file is a list of eight inline modules; each is a `pub(crate) mod` unless stated:

| Module | Lines |
| --- | --- |
| `atomic_operations` | 10-198 |
| `capabilities` (incl. 863-line `#[cfg(test)] mod tests`, 670-1532) | 200-1533 |
| `content_transformations` | 1535-1706 |
| `execution` | 1708-2947 |
| `native_code_layouts` | 2949-3107 |
| `scalar_operations` (submodules `boolean_float`, `float_result_policy` (a `pub mod`), `integer`, `trapping`, plus free fns) | 3109-3900 |

### 1. atomic_operations (10-198)

1. Purpose. Runs one atomic access operation (load, store, swap, read-modify-write, compare-exchange, compare-exchange-once) against an atomic cell inside a structural value, and publishes the result.
2. Shape. `TerminalExecution::execute_atomic_access(operation, place, path, event)` (`:23`); private `read_modify_write(op, prior, operand) -> Option<TerminalScalarValue>` (`:168`).
3. Inputs/outputs. Reads `self.structural_value(place)` (`:31`), the verified machine (`:30`), `self.values` for operands (`:51-57`), primitive storage via `resolve_live_view_projection`, `atomic_cell_storage`, `read_primitive_storage` (called `:49-50`, `:59`; defined elsewhere, not read). Stages the write in `PrimitiveStorageWrites` and commits at the end (`:103-106`, `:163`). Result goes to `self.values` (scalar result, `:158`), `publish_scalar_aggregate` (once-outcome structural result, `:143`), or nothing (unit, `:160`).
4. How it decides.
   - Access right is found by place identity: an arrival parameter of the machine whose `structural_type == root.structural_type` (`:32-34`), or else the producing operation's result when place and type match (`:35-42`), which counts as `Owned`. Neither -> `malformed()` (`:43`).
   - Refuses only `WriteOnlyBorrow` access and illegal orderings (`:44`). SharedBorrow and MutableBorrow and Owned all pass.
   - Value matching is by scalar type equality (`:55`). A compare-exchange matches when `prior == operand(expected)` as scalar values (`:93`).
   - Result routing is keyed on `(prior, operation.result)` shape (`:107-162`); anything else is `malformed()` (`:161`).
5. Cases by shape. `read_modify_write` handles integers only (`:173-185`), refuses address-typed integers (`:186`: `scalar_type.is_address()`) and mixed types. Five operations: FetchAdd/Sub/And/Or/Xor (`:189-195`), wrapping arithmetic. The once-outcome path takes `cases[index]` and at most one payload field `cases[index].fields[0]` (`:135-140`): a payload is a single scalar field.
6. Same job elsewhere. Other representations of `AtomicReadModifyWrite` (codec `psi/.../codec/src/block_wire.rs:66,131`; lowering `psi/.../06_checked-trees-to-lowered-psi/src/unit_operation_frame.rs:429`; `omega/.../abstract-operations/src/atomic.rs:44,56`; `omega/.../selected-instructions/src/atomic.rs:214`): those are translations of the variant, not execution. No other executor found in a grep for `AtomicReadModifyWrite::FetchAdd` (not read).
7. Unused. `execute_atomic_access`: 2 grep hits (def `:23`, call `:2819`) so 1 call site. `read_modify_write`: 2 hits (call `:80`, def `:168`) so 1 call site.
8. Stated limits. None.
9. Fail-open. None seen in the sense of "unmatched -> accept". One acceptance to record: a store or swap through a `SharedBorrow` parameter is accepted (`:44` rejects only `WriteOnlyBorrow`). Open (no comment); possibly intended because atomic cells are interior-mutable, but nothing at the site says so.

### 2. capabilities (200-1533)

1. Purpose. Before execution starts, walks every machine and block reachable from the entry and reports (a) what the host must supply (imports, missing providers, external completions, scalar applications, scalar entry conditions), or (b) fails with `UnsupportedCapability` / `UnsupportedSemanticVariant` when the program needs something the environment cannot give (native code layout, system registers, assembly target, external-completion invocation).
2. Shape.
   - `enum TerminalUnsupportedCapability` (`:211`): `NativeCodeLayout`, `RegisterAccess`, `TargetMachineMemberSupply`, `ExternalCompletionInvocation`.
   - `struct TerminalExecutionRequirements { imports, missing_providers, external_completions, scalar_applications, scalar_entry_conditions }` (`:230`).
   - `struct TerminalExternalCompletionDemand { boundary, kind }` (`:239`).
   - `check` (`:244`, delegates with no private RAM) and `check_with_private_entry_ram` (`:251-668`, one ~420-line function with a `pending` machine worklist and a per-machine `blocks` worklist).
   - Tests `:670-1532`: about 20 `#[test]` fns using fixture builders `machine`, `program`, `set_machine`, `add_provider`, `boundary_declaration`, `dynamic_program`.
3. Inputs/outputs. Reads `AdmittedTerminalProgram` (catalogs, verified machines), optional `TerminalRuntimeSupplies` (supplies, target machines, layouts, x86 flags, memory ordering, native layouts), `private_entry_ram` recipes, `scalar_scope`. Calls `terminal_semantics::storage_operand_dependencies` (`:351`, `:541`), `inline_storage_extent_dependencies` (`:624`), `terminal_access_origin_graph` and `AccessRamOriginAnalysis` (`:592-595`), `engine::calls::target_families::{selected_member, validate_result_transport, completion_callee}` (`:458`, `:470`, `:552`). Output is the requirements struct; no mutation of the program.
4. How it decides.
   - Reachability is by machine id and block id, via `visited` sets (`:290-293`, `:329-332`); the callee for each operation is `kind.direct_callee()` (`:534`), plus special arms.
   - Boundary supply is keyed on `BoundaryMachineId` lookups in `installation.supplies` (`:490-532`): `CheckedProvider(selected)` -> follow the selected machine (`:513-515`); HostImport/DeferredComponent/ComponentProvider -> import (`:516-523`); no supply and `program.has_provider(boundary)` -> `missing_providers` (`:524-527`); no supply and no provider -> import (`:528-531`).
   - Target applicability of a machine is keyed on `machine.assembly_target_requirement` vs the selected target name or layout target, compared by `requirement.accepts_target_name(target)` (`:310-326`). The check is on the target NAME, a string/enum identity.
   - External-completion boundaries pick the smallest boundary id (`:485`: `*boundary < found`) as the one reported.
   - Dynamic calls (`:435-454`): every table with the same `interface` contributes its row at `slot_index` to `pending` (`:447-452`), not just the table selected at runtime.
   - Scalar entry conditions (`:628-660`): keyed on position in the scalar application catalog (position + 1 becomes the id, `:651-654`), selected machine reached (`:648`).
5. Cases by shape.
   - Six register/descriptor operations are refused outright at `:399-434`: `ActivateTaskRegister`, `ReadTaskRegister`, `ReloadSegmentDescriptors`, `ReadSegmentSelector`, `GlobalDescriptorTable`, and any other `RegisterAccess`. Two `RegisterAccess` shapes are admitted when the environment has the capability: Rflags Read/Write when `installation.x86_flags.is_some()` (`:377-389`), and `MemoryFence { kind }` when `memory_ordering.supports(kind)` (`:390-398`).
   - Terminators with callee edges handled: `InvokeNoNormal`, `ExternalCompletion`, `Jump`, `Conditional`, `StructuralCase` (`:549-581`); all others fall to `_ => {}` (`:580`).
   - Cleanup actions reached through `terminator.cleanup_rosters()` -> `invoked_machine()` (`:582-588`).
   - Private entry RAM: every recipe must match terminal identity, entry machine, and be in `catalogs.access_view_inputs` (`:269-275`).
6. Same job elsewhere (inside my range). The refusal arms for the six unsupported register/descriptor operations appear twice: `:399-434` (preflight) and `:2756-2791` (run time, in `resume_operations`). The Rflags and MemoryFence capability tests appear twice as well: `:377-398` and `:2726-2755`. Suspected duplication outside the unit: `psi/compilation/build-time-evaluation/src/machine_execution.rs` also runs Terminal Psi machines (it calls `terminal_interpreter::float_result_policy::meaning`, lines 4750-4751); I did not read it.
7. Unused / callers (Grep):
   - `check_with_private_entry_ram(` and `capabilities::check(`: calls in `embedding.rs:2313`, `:2324`, `:2708`; within this file `check` -> `check_with_private_entry_ram` at `:248`. The grep with `-c` gave 5 hits total (2 here: def `:251` and call `:248`; 3 in `embedding.rs`).
   - `compiler_owns_boundary_execution` (called at `:479`): defined `embedding.rs:2908`; callers `embedding.rs:2226`, `:2511`, `:2553`, `:2567`, plus this file's `:479`. 5 call sites.
   - `TerminalExecutionRequirements` is a `pub` struct; users outside the file not counted.
8. Stated limits. Error strings: "execution reaches an authored body outside the completed scalar application closure" (`:301`) and "assembly catalog target requirement is not established by the target environment" (`:324`). `TerminalUnsupportedCapability::TargetMachineMemberSupply` is declared (`:218`) and no construction of it appears in my range (grep not run for the rest of the crate).
9. Fail-open risks.
   - `kind => kind.direct_callee()` (`:534`). Every operation kind not named in the match is accepted with no capability check; only the callee is followed. What passes: a future operation that needs an environment capability but is not listed. Open (no comment).
   - `compiler_owns_boundary_execution(declaration)` -> `continue` (`:479-483`): no import, provider, or demand is recorded for that boundary. What passes: a boundary the compiler claims to run itself is not demanded from the host. Open (reason given only by the function name).
   - `terminator ... _ => {}` (`:580`): terminators other than the five handled add no successor. `Return`, `ReturnUnit`, `ReturnStructural`, `Crash` have no successors, so that is right; no comment says so. Traced by shape: the terminator enum arms at `:2912-2930` list nine variants, and the five handled here plus Return/ReturnUnit/ReturnStructural/Crash cover all nine.
   - Access-origin analysis (`:591-617`): when `terminal_access_origin_graph(...)` returns `None` the whole block is skipped and imports stay demanded. Fail-closed. When the analysis finds formations for a boundary, its import/missing-provider demand is removed (`:612-615`) only if no other call to the same boundary was unresolved (`internally_bound.difference(&unresolved)`).
   - `private_entry_ram` layout check (`:258-268`) uses `is_none_or`, so a missing layout is a mismatch error. Fail-closed.

### 3. content_transformations (1535-1706)

1. Purpose. Validates, before a content-transformation operation runs, that the claims the operation says it consumes (its "derivations") are exactly the live linear ownership claims under its parent places, and returns that claim list.
2. Shape. One method `TerminalExecution::content_parent_claims(parents: &[StructuralArgument], derivations: &[ContentTransformationDerivation]) -> Result<Vec<ClaimId>>` (`:1544-1704`).
3. Inputs/outputs. Reads `structural_values`, `live_claims`, `reference_referents`, `live_affine_frontier`, `structural_cases`; calls `crate::engine::custody::{has_live_loan, projected_ownership_claims}` (`:1608`, `:1621`), `ConstructorValue::resolve` (`:1606`), and `terminal_semantics::{content_transformations::structural_path, owned_structure::validate_accounted_descendants, content_transformations::validate_parent_memberships}`. Returns the sorted expected claim list. Mutates nothing.
4. How it decides.
   - Each derivation input must name a live claim whose `place == subject.root`, whose `path` equals the resolved subject path, and which lies under some parent (`parent.place == root && path.starts_with(&parent.path)`) (`:1571-1579`). Keyed on place identity and path prefix.
   - Each parent must be `Owned` (`:1603`), have no live loan or overlapping resident reference descriptor on the same `opaque_identity` (`:1607-1618`), have at least one projected ownership claim (`:1622`), and no overlapping affine frontier entry (`:1657-1662`).
   - Final test: the set of claims found by walking the parents (`actual`) equals the set named by derivations (`expected`) (`:1698-1702`).
5. Cases by shape. Empty-on-both-sides returns `Ok(vec![])` (`:1550`). One side empty and the other not is an error (`:1553`).
6. Same job elsewhere. The "overlapping reference descriptor with same opaque_identity" test (`:1611-1614`) is the same idea as `has_live_loan` (`:1608`) written as two checks on one value. Overlap by prefix appears in the engine_custody notes from the first pass (`place_is_claimed`, `has_live_loan`, etc.).
7. Unused. `content_parent_claims(`: 2 hits: def here, 1 call in `engine_structural.rs`. So 1 call site.
8. Stated limits. Error text "content parent retains a live loan or resident reference descriptor" (`:1617`).
9. Fail-open risks. None seen. Every mismatch becomes `ClaimTransferMismatch` or `ContentTransformationMismatch`. The membership predicate closures (`:1641-1648`, `:1677-1694`) answer "is another case active" from stored case data; if the stored case is absent (`None`), the first closure returns false (`== Some(case)`), and the second does not return `true` (`is_some_and`). Whether a false answer relaxes validation depends on `terminal_semantics` (not read): open.

### 4. execution (1708-2947)

1. Purpose. Defines `TerminalExecution`, the resumable interpreter state, its per-call frame (`SuspendedCall`), claim bookkeeping types, the operation dispatch macro, startup from entry arguments, and the main `resume` loop that runs operations and terminators under a work meter.
2. Shape.
   - Enums `TerminatorFlow { Continue, Yield }` (`:1759`) and `OperationFlow { Advance, Redispatch, Yield }` (`:1764`).
   - `struct TerminalExecution<'artifact>` (`:1770-1859`): about 70 fields. Row tables (`RuntimeRows<K,V>`): `values`, `structural_values`, `linear_homes`, `reference_referents`, `structural_cases`, `byte_sequence_values`, `element_view_values`, etc.; sets/maps: `live_affine_frontier: BTreeSet<StructuralAffineDiscard>` (`:1834`), `live_claims: BTreeMap<ClaimId, LiveClaim>` (`:1836`), `access_view_occurrences`; control: `current_machine`, `current`, `next_operation`, `call_stack: Vec<SuspendedCall>`; outcome: `result`, `crash`, `external_completion`, `failure`.
   - `struct SuspendedCall` (`:1861-1881`): a caller's saved copy of 13 of those tables plus `loan_completions`, `result: SuspendedCallResult`.
   - `struct LiveClaim { place: Option<PlaceId>, path, multiplicity: Option<StructuralMultiplicity> }` (`:1884`); `LiveClaim::path_multiplicity` (`:1891`): any non-empty path is `Linear`, else the declared multiplicity; `LiveClaim::at` (`:1902`).
   - `enum ResultClaimOrigin { CalleeReturn, TargetFamilyReturn{head, member}, CheckedProvider }` (`:1917`) with `result_qualifications_match` (`:1927`) and `result_projected_qualifications_match` (`:1950`).
   - `enum SuspendedCallResult { Scalar, Unit, NoNormal, ExternalCompletion, Structural{...}, NominalCleanups{...} }` (`:1977`); `enum CleanupContinuation { Return(CompletedReturn), Successor{target, bindings} }` (`:2003`); `struct CompletedReturn` (`:2011`, with `plain` `:2021`); `struct FailedReturn` (`:2033`).
   - Macros: `dispatch_operations!` (`:2039`), `operation_kind!` (`:2067`, returns `malformed()` on mismatch), `dispatched_operation!` (`:2077`, `unreachable!` on mismatch), `frame_field_mut!` (`:2090`, picks the live field when depth equals stack length, else the saved frame's field, `malformed()` when depth is out of range).
   - Methods: `start_with_program` (`:2133`), `retain_host_structural_payloads` (`:2372`), accessors (`:2424-2531`), `mint_staged` (`:2533`), `resume` (`:2545`), `resume_with_components` (`:2553`), `resume_operations` (`:2593`, ~350 lines), `retire_access_view_occurrences` (`:2943`).
3. Inputs/outputs. `start_with_program` reads the admitted program, scalar arguments, `TerminalStructuralInputs`, optional installation/account, and builds all tables via `bind_arguments`, `bind_structural_arguments`, `bind_affine_frontier`, `bind_entry_claims`, `bind_structural_primitive_values`, `scalar_fields::bind` (custody functions already read in the first pass). `resume_operations` mutates the tables, pushes `TerminalEffect`s, and returns a `TerminalExecutionStatus`.
4. How it decides.
   - Resume order (`:2599-2632`): if an external completion is recorded, return it; if a result exists, return `Complete`; if a crash exists, return `Crashed`; then check every entry in `external_demands` has an admission in `external_bindings` that `answers(identity, process_domain, demand)` else `UnboundExternalCompletions`.
   - Metering: each operation is charged `meter.charge_operation(operation)` unless a pending component call or pending effect for the same operation id/machine/block is resuming (`:2645-2662`). Keyed on `operation.id`, `site.machine`, `site.block`.
   - Operation dispatch is keyed on the `OperationKind` variant (`:2672-2888`). 28 arms are written out under `literal { ... }` (`:2673-2822`) and the rest are rows in `dispatch { Variant | Variant => advance|flow method }` (`:2824-2886`). The compiler's exhaustiveness check on `match $kind` (`:2045`) is the only guard that a variant has a row.
   - Operation errors are wrapped as `ExecutionSite { machine, block, operation id, tag, cause }` (`:2577-2585`) and stored in `self.failure` (`:2588`), after which `resume` returns the same error forever (`:2559-2561`).
   - Runtime index operations are rewritten first when `kind.runtime_indexes()` is non-empty (`:2664-2669`).
   - `result_qualifications_match` decides by origin: for `TargetFamilyReturn` the result's qualification domains must all appear in the declared list (subset, `:1932-1937`); for `CalleeReturn` the result's list must equal declared (`:1947`); for `CheckedProvider` it must equal declared plus qualification-establishment domains, sorted (`:1939-1947`). `result_projected_qualifications_match`: non-callee origins refuse any `preserved` rows (`:1956-1958`); `TargetFamilyReturn` and `CheckedProvider` accept when each result row is in `declared` (subset, `:1959-1966`); `CalleeReturn` demands equality with `declared + preserved`, sorted and deduped (`:1967-1973`).
5. Cases by shape.
   - `start_with_program`: refuses any scalar parameter with qualifications (`:2181-2187`, `ScalarEntryQualificationUnsupported`). Zero-initialised arguments are materialised per index (`:2296-2331`); established erased fields must have both path segments and carrier identity interned (`:2312-2320`).
   - Rflags `Read` must be result type unsigned 64-bit (`:2741`). Memory fence result must be unit (`:2727`).
   - `retain_host_structural_payloads` (`:2372-2422`) applies to Owned parameters only and skips zero-initialised ones and ones where `requires_materialized_host_custody` is true; it records `Materialized` if any of five input lists (contents, scalar_fields, boolean_fields, primitive_values, byte_arrays) names the argument index, else `Opaque` (`:2393-2417`).
6. Same job elsewhere.
   - The unsupported-register arms: `:2756-2791` repeat `:399-434` (see capabilities).
   - `Terminator` dispatch at `:2912-2930` lists nine variants and one `settle_*` method each; the capability walk (`:549-581`) lists the same enum separately.
   - Structural-fields copy in `SuspendedCall` mirrors fields of `TerminalExecution`; both define the same 13 tables twice (`:1861-1881` vs `:1770-1859`). Frame push/pop code is not in my range (engine_calls/engine_terminators, unread by me).
7. Unused / callers (Grep, counts from tool output):
   - `FailedReturn` / `failed_return`: written at `engine_terminators.rs:1282,1291,1303,1324`; read at `engine_terminators.rs:1576-1579` (`failed._completed`, inside an `assert_eq!` test block, inferred). The three fields are named `_completed`, `_expected`, `_loan_completions` (`:2034-2036`), underscore-prefixed. No non-test reader found.
   - `failed_call_result`: written `engine_calls.rs:220`, read `engine_calls.rs:960`.
   - `structural_primitive_values` (pub, `:2501`): grep `structural_primitive_values\(` over `omega-rust/*.rs` shows only `:2501` and `:2504` here, `engine_custody.rs:1356` (`bind_structural_primitive_values`) and `:2345` here. 0 callers of the pub method. `final_structural_primitive_values` (`:2507`): its only call is the wrapper at `:2504`; 1 call site, all through the pub method that has none.
   - `live_affine_frontier()` method (pub, `:2523`): grep `\.live_affine_frontier\(\)` gave 0 hits. 0 call sites.
   - `live_affine_discard_at(`: 1 definition (`:2527`) and 28 hits by the earlier count-mode grep in its combined pattern; the content grep lists 24 call lines (storage_references 2, storage_records_and_arrays 2, storage_primitives 1, storage_bytes_and_views 6, engine_terminators 5, engine_structural 1; the rest are `mint_staged` or other names). Not a single-count claim; treat as "used widely".
   - `mint_staged(`: calls in storage_references.rs:1041, storage_providers.rs (918, 1173, 1979, 2193, 3143), storage_bytes_and_views.rs (2389, 4904): 8 call sites.
   - `unsigned_integer_value(`: storage_bytes_and_views.rs 1261, 1309, 2312, 2903: 4 call sites.
   - `live_structural_values()`: storage_primitives.rs 425 and 3768, boundary_inputs.rs 1832: 3 call sites.
   - `scalar_values_at(`: engine_terminators.rs:2244, engine_calls.rs:70: 2 call sites.
   - `CompletedReturn::plain(`: engine_calls.rs:912, engine_terminators.rs 1545, 1708, 2437, 2469: 5 call sites.
   - `start_with_program(`: boundary_effects.rs:2863, engine_calls.rs:901, embedding.rs:2763, boundary_values.rs:263, engine_terminators.rs:1499: 5 call sites.
   - `retain_host_structural_payloads(`: embedding.rs:2806: 1 call site.
   - `is_finished()`: embedding.rs 1713, 1728; `omega/compilation/compiler/tests/...` 4 more: 6.
   - `ResultClaimOrigin::`, `result_qualifications_match`, `result_projected_qualifications_match`, `LiveClaim::at`, `LiveClaim::path_multiplicity` (one combined grep): 39 hits in 7 files (engine_calls 12, engine_terminators 10, engine_custody 7, storage_records_and_arrays 4, engine_structural 2, engine_execution 2, and `omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/control_flow_calls.rs` 2). Not split per name.
   - Env var `OMEGA_TERMINAL_TRACE` (`:2109`): read only here (grep shows one hit); `trace_scalar_state` has 2 calls (`:2671`, `:2911`). It prints up to 128 scalar values, not structural state.
8. Stated limits. `ScalarEntryQualificationUnsupported` (`:2186`). Header comment `:1-8` for the file; none else.
9. Fail-open risks.
   - Rflags write (`:2744-2755`): `if let Some(TerminalScalarValue::Integer { value: IntegerValue::Unsigned(bits), .. }) = self.values.get(&value)` writes the flags; if the value is absent or not an unsigned integer the write is silently skipped and the operation advances. Where bits exceed `u64`, `unwrap_or(u64::MAX)` writes all ones instead of failing (`:2752`). Other scalar readers in the file return `VerifiedValueMissing` or `malformed()` for the same condition (for example `:3140-3144`). What passes: a verified program whose Rflags value operand is missing or of the wrong type runs on without updating the flags. Open (no comment).
   - `OperationKind::AtomicFence { .. } => OperationFlow::Advance` (`:2810-2812`): no memory-ordering capability check and no effect pushed, unlike `RegisterAccess::MemoryFence` (`:2726-2737`) which refuses without `memory_ordering` support and pushes `TerminalEffect::MemoryFence`. Open (no comment). The capability walk (`:390-398`) also names only `RegisterAccess::MemoryFence`, not `AtomicFence`.
   - `resume_operations` on `result.is_some()` returns `Complete` (`:2604-2608`); a second `resume` returns the same result. Intended (stated by the early-return order, no comment).
   - `OperationFlow::Advance` observes entry result only for scalar results (`:2891-2893`). Structural results are not passed to `observe_completed_entry_result` here. Not a risk of accepting; just a limit.
   - `ResultClaimOrigin` subset rules (`:1932-1937`, `:1959-1966`): for `TargetFamilyReturn` and `CheckedProvider` a result may carry FEWER projected qualifications than declared and still match. What passes: a returned value that claims less than the contract guarantees. Open (no comment); weaker claim, so likely benign, but not argued at the site.
   - `FloatResultPolicy`, `TrappingInteger` yield `Crashed` rather than error; see section 6.

### 5. native_code_layouts (2949-3107)

1. Purpose. Builds, at run time, the value for `EstablishNativeCodeLayout { machine }`: a record `{ entry: integer, regions: view of {base, length} }` filled from the environment's native layout for that machine.
2. Shape. `fn number(integer, u64) -> TerminalScalarValue` (`:2969`); `execute_native_code_layout(operation, machine)` (`:2977-3105`).
3. Inputs/outputs. Reads `self.native_code_layouts` (an `Arc` environment), `environment.layout(machine)` giving `layout.entry` and `layout.regions[].base/.length`; stages scalars with `ConstructorStorage`; inserts an `ElementView::immutable_sequence` into `structural_element_view_fields` and the record into `structural_values`; calls `track_affine_result` (`:3103`).
4. How it decides. By type-shape walk: result type must be a `Record` with exactly two fields `[entry, regions]` (`:3002-3009`); `entry` is `Scalar(Integer)` (`:3010`); `regions` is `Structural(view_type)` (`:3014`); `view_type` is `ElementView { element }` (`:3019`); `element` is a `Record` with exactly `[base, length]`, both `Scalar(Integer)` (`:3025-3040`). Fields are matched by POSITION, not by name or identity (`:3007`, `:3030`). Result must be Affine with no claims, qualifications, projected qualifications or establishments, and the place must be unoccupied (`:2993-2999`).
5. Cases by shape. The exact shape above; anything else is `malformed()`. Missing environment or layout -> `UnsupportedCapability::NativeCodeLayout` (`:2982-2991`).
6. Same job elsewhere. The preflight at `:363-376` checks the same environment (`native_code_layouts.layout(machine)`) before execution; this function repeats the lookup. `number` duplicates the construction `TerminalScalarValue::Integer { scalar_type, value: IntegerValue::Unsigned(..) }` that `:2742` writes inline.
7. Unused. `execute_native_code_layout(`: 1 call (`:2686`).
8. Stated limits. None.
9. Fail-open. None seen. Layout values are cast with `number(entry_type, layout.entry)`; whether `u64` fits a narrow `entry_type` is checked, if at all, by `stage_scalar_endpoint` / `validate_against` (`:3075`) which I did not read: open for the `entry` and `base`/`length` of the staged values (the region elements are validated by `aggregate.validate_against`, the `entry` scalar is not passed through that call).

### 6. scalar_operations (3109-3900)

1. Purpose. Executes the scalar operations of Terminal Psi: IEEE float constants, compare, representation views, fused multiply-add, booleans, float result policy (trapping and saturating), all integer arithmetic/bitwise/shift/cast forms, trapping integer operations, and small shared helpers.
2. Shape.
   - `boolean_float` (`:3110-3261`): `execute_ieee_float_constant` (`:3122`), `_ieee_float_compare` (`:3135`), `_scalar_representation_view` (`:3157`), `_nearest_ieee_float_fused_multiply_add` (`:3197`), `_boolean_constant` (`:3225`), `_boolean_not` (`:3234`), `_boolean_equal` (`:3246`).
   - `float_result_policy` (`pub mod`, `:3262-3357`): `pub fn meaning(IeeeFloatValue) -> FloatMeaning` (`:3271`); `execute_float_result_policy` (`:3293`).
   - `integer` (`:3358-3723`): `integer_scalar_type` (`:3365`), macro `saturating_integer_op!` (`:3375`) generating `execute_saturating_integer_{add,subtract,multiply}` (`:3697-3701`), `integer_operand` (`:3399`), private `integer_operands` (`:3409`), `execute_integer_constant` (`:3430`), `_integer_equal` (`:3442`), `_integer_less_than` (`:3456`, also serves `IntegerLessOrEqual`), `_integer_bitwise_not` (`:3477`), private `execute_integer_unary_cast` (`:3494`), `_integer_widen` (`:3518`), `_integer_exact_cast` (`:3526`), `_integer_bitwise_and` (`:3538`, also Or and Xor), `_wrapping_integer_shift_left` (`:3573`, also right and the two exact shifts), `_exact_integer_add` (`:3621`, serves 12 kinds: exact add/sub/mul/div/rem, wrapping add/sub/div/rem/mul, saturating div/rem), `require_boolean_result` (`:3703`), `store_scalar` (`:3713`).
   - `trapping` (`:3724-3818`): `operation_trap` (`:3733`), `execute_trapping_integer` (`:3751`).
   - Free: `integer_scalar_type` re-export (`:3820`), `terminal_scalar_belongs_to_type` (`:3826`), `ieee_float_compare` (`:3834`), `nearest_ieee_float_fused_multiply_add` (`:3860`).
3. Inputs/outputs. Reads operands from `self.values` by `ValueId`; type of the result from `operation.result.expect_scalar().scalar_type`; writes the result into `self.values` via `store_scalar` (`:3713-3721`) and returns `OperationFlow::Advance`. Arithmetic is delegated to `IntegerType` methods (`wrapping_add`, `exact_add_values`, `saturating_mul`, ...) and `numerics::float_semantics::FloatSemantics` (not read).
4. How it decides.
   - Types are checked by equality of `IntegerType`: operand types must equal the result type for most ops (`:3386`, `:3551`, `:3648`, `:3760`); exact add and subtract use `admits_exact_add_operands` / `admits_exact_subtract_operands` (`:3643-3647`) which allow different operand types; shifts allow a different `count_type` but value type must equal result type (`:3593`); casts take source type from the operand and target from the result (`:3500-3508`).
   - Any arithmetic that returns `None` (overflow, divide by zero in the non-trapping forms) becomes `malformed()` (`:3389`, `:3566`, `:3611`, `:3690`). In `TrappingInteger`, the same `None` becomes a `Crashed` status with `CrashCause::Trap` instead (`:3804-3808`).
   - Float result policy (`:3293-3355`): `Trapping` crashes if `FloatSemantics::apply_trapping_policy(raw_meaning)` errs (`:3308`); `Saturating { eligible }` replaces an infinite raw result by the maximum finite value of the same sign only when the boolean `eligible` operand is true and the raw is infinite (`:3330-3346`); otherwise returns the raw bits.
   - `execute_scalar_representation_view` uses `ScalarRepresentationViewPair::classify(source_type, result.scalar_type)` and refuses unclassified pairs (`:3177-3180`).
   - `terminal_scalar_belongs_to_type` returns `true` for every Boolean and every IeeeFloat, and `scalar_type.admits(value)` for integers (`:3826-3831`).
5. Cases by shape.
   - `float_result_policy` handles `Binary32` and `Binary64` only (`:3325-3328`, `:3339-3346`); `nearest_ieee_float_fused_multiply_add` handles both formats and the catch-all is `unreachable!("verified IEEE FMA operands have one exact format")` (`:3897`), after the caller has already compared the three operand formats to the result format (`:3217`).
   - `meaning` (`:3271`) handles `Binary32` and `Binary64`.
   - `execute_trapping_integer` handles Add, Subtract, Multiply, Divide, Remainder, ShiftLeft, ShiftRight, Convert (`:3765-3803`).
6. Same job elsewhere.
   - `float_result_policy::meaning` (`:3271-3276`) and the local closure `meaning` inside `ieee_float_compare` (`:3840-3843`) are the same `IeeeFloatValue -> FloatMeaning` match written twice in one file. External callers of the `pub fn`: `omega/interpreter/terminal-runtime/src/execution/host_imports/numeric.rs:134`, `:204`; `psi/compilation/build-time-evaluation/src/machine_execution.rs:4750`, `:4751` (4 call sites, plus the 1 here at `:3305`).
   - Exact integer ops appear in three forms in my range: the exact/wrapping/saturating kinds (`:3621-3695`), the saturating macro (`:3375-3396`), and `TrappingIntegerOperation` (`:3765-3803`). They call the same `IntegerType` methods (`exact_add`, `exact_sub`, `exact_mul`, `exact_div`, `exact_rem`) but each with its own operand-type check.
   - `integer_scalar_type` also exists as a separate function at `psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission.rs:9716` (suspected same job, not read). `integer_operand` also appears in `psi/compilation/proof/src/mathematical_declarations.rs:2902`, `psi/compilation/validation/src/contract_entailment.rs:10122`, `psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_scalar_computations.rs:4235` and `.../arithmetic_domains.rs:23349` (suspected, not read; the last is a different signature).
   - Suspected: other evaluators of Terminal Psi scalar operations exist in `psi/compilation/build-time-evaluation/src/machine_execution.rs` (it uses `meaning`), and native code in `omega/.../03_target-operations-to-selected-instructions/src/selection_scalar_graph.rs:3166` (`float_result_policy::emit`). Not read.
7. Unused / callers (Grep):
   - `terminal_scalar_belongs_to_type(`: 26 hits in 8 files (storage_primitives 11, storage_records_and_arrays 5, storage_bytes_and_views 3, boundary_effects 2, engine_structural 2, storage_references 1, engine_execution 1 (the def), engine_custody 1), so 25 call sites.
   - `ieee_float_compare(`: calls `:3153` here and `boundary_effects.rs:424`; 2 call sites.
   - `nearest_ieee_float_fused_multiply_add(`: 1 call (`:3221`) and the def.
   - `operation_trap(` (`.operation_trap(`): 2 hits, both in this file (`:3310`, `:3806`); 2 call sites. `operation_trap` is `pub(crate)`.
   - `integer_scalar_type` re-export (`:3820`): used at `:2740`, `:3756` in this file and in the terminal-interpreter via `crate::engine::scalar_operations::integer_scalar_type`; not counted beyond those.
   - `float_result_policy` is `pub mod` and re-exported as `pub use engine::scalar_operations::float_result_policy` at `lib.rs:139`; its only external use is `meaning` (4 sites above).
8. Stated limits. None. `unreachable!` message at `:3897` states the invariant.
9. Fail-open risks.
   - `terminal_scalar_belongs_to_type` (`:3826-3831`) returns `true` for all Boolean and IEEE-float values. Wrong value that passes: none, since every Boolean and every IEEE bit pattern is a legal value of its type; the float format is not compared here (the caller would need to compare). Traced: the function takes no type argument for Boolean/float, so a float of the wrong format cannot be caught by this function. Callers (25) were not traced. Open for the "wrong float format" case.
   - `execute_float_result_policy` Saturating arm: when `was_infinite` and not eligible, the raw infinity is returned unchanged (`:3330-3349`). Intended per the policy; no comment.
   - `execute_scalar_representation_view`: `ok_or(malformed())` (`:3180`) evaluates `malformed()` eagerly; behavior is still an error on `None`. Not a risk.
   - `execute_ieee_float_constant` compares the result type to `IeeeFloat(value.format())` (`:3127-3131`). Fail-closed.
   - None else seen: `None` from arithmetic always ends as `malformed()` or a trap crash.

---

## Coverage

- Read in full: `engine_execution.rs` 1-3900 (3,900 lines).
- Skimmed: none.
- Not read: none.
- Total: 3,900 read, 0 not read.

Greps run, all over `omega-rust` (Grep tool; counts quoted from tool output): `meaning(` (count mode: 17 hits in 6 files; of the 4 in this file, 3271 def, 3305 call, 3844/3845 closure calls), `terminal_scalar_belongs_to_type(` (26), `check_with_private_entry_ram(|capabilities::check(` (5), `content_parent_claims(` (2), `.operation_trap(` (2), a combined grep for `live_affine_discard_at|live_structural_values|mint_staged|unsigned_integer_value|scalar_values_at|structural_primitive_values|live_affine_frontier()` (content mode, listed above), `frame_field_mut!` (count mode: 6 hits in 4 files: storage_primitives 2, storage_bytes_and_views 1, boundary_storage_inputs 2, boundary_inputs 1; none in this file besides the macro, whose definition line `:2090` begins `macro_rules! frame_field_mut` without `!` after the name in a form the regex matched, so those 6 are call sites), `FailedReturn|failed_return|failed_call_result`, `float_result_policy`, `compiler_owns_boundary_execution`.

Note: the count-mode run of the combined `live_affine_discard_at|live_structural_values|mint_staged|unsigned_integer_value|scalar_values_at` pattern gave 34 hits in 10 files; `frame_field_mut!` alone gave 6 hits in 4 files. I cite per-name call sites only where the content-mode listing let me count them.

## Five things a maintainer most needs to know about this unit

1. `engine_execution.rs` holds three unrelated things in one file: the startup-and-resume loop (`:1708-2947`), the preflight capability walk (`:200-668`), and every scalar operation (`:3109-3900`). The run loop at `:2635-2940` is the only place that dispatches operations and terminators, and the operation table (`:2672-2888`) is a `match` that the compiler checks for completeness over `OperationKind`.
2. The preflight walk (`check_with_private_entry_ram`, `:251-668`) decides what to demand from the host by following machine and block reachability, with a catch-all `kind.direct_callee()` (`:534`) for every operation it does not name. The six unsupported register arms are written out twice, here (`:399-434`) and in the run loop (`:2756-2791`), as are the Rflags and MemoryFence capability tests (`:377-398`, `:2726-2755`).
3. Two operations in the run loop do less checking than their neighbours: `AtomicFence` just advances (`:2810`), and the Rflags write silently skips a missing or non-unsigned operand and saturates an oversized one to `u64::MAX` (`:2744-2755`).
4. Failures are sticky: any error in `resume_operations` is wrapped with machine, block, operation id and tag and stored in `self.failure`; all later `resume` calls return it (`:2559-2590`). Traps are not errors: `operation_trap` records a `crash` and returns `Crashed` (`:3733-3749`, uses at `:3310`, `:3806`).
5. `SuspendedCall` (`:1861`) copies 13 of the tables of `TerminalExecution` (`:1770`) per call frame, and `frame_field_mut!` (`:2090`) addresses either the live table or a saved frame's by depth. The three fields of `FailedReturn` (`:2033-2037`) are underscore-prefixed and the only reader I found is a test in `engine_terminators.rs:1576-1579`.

## What I am unsure about

- Whether the `SharedBorrow` atomic store (`:44`) is intended; no comment and I did not read `atomic_cell_storage` / `resolve_live_view_projection`.
- Whether `scalar_field_projection`, `stage_scalar_endpoint` or `validate_against` range-check the `u64` layout numbers placed into narrow integer fields in `execute_native_code_layout`.
- Whether `ResultClaimOrigin`'s subset rules (`:1932-1937`, `:1959-1966`) are meant to accept results with fewer qualifications than declared.
- `TerminalUnsupportedCapability::TargetMachineMemberSupply` (`:218`): not seen constructed in my range; not grepped across the crate.
- The content-mode greps of `live_affine_discard_at` were mixed with other names in one pattern; I did not compute a clean per-name count for it.
- The six `frame_field_mut!` hits attributed above were inferred from a count-mode grep; I did not open each site.
