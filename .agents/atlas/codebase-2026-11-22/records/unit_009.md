# Record: unit_009 (area 10-native-stages-04-10)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d`. All paths below are under
`source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/preparation/`
and written as `legalization_*.rs`. Three files, all read in full (Read tool, in order).

Vocabulary used here: "custody" is the module's own word for a check that two program forms agree
(error `LegalizationError::custody()`). "Target rows" are the `TargetUnitOperation` / `TargetControlTerminator`
rows that target lowering (earlier in stage 04) already produced. "Legalized" means the `Legalized*` forms
(`legalized_operations` crate) that stage 05 selects over.

Overall shape of the unit (read): stage 04's `prepare_execution_plan` (`legalization.rs:651`, not in this unit) turns target rows
into `LegalizedScalarFunction` rosters. Two entry paths exist. The ordinary path projects target rows only
(`derive_target_function_rosters`, `legalization_source.rs:3633`; called at `legalization.rs:666` and `:777`). The audit path re-derives the
same rosters from the earlier optimized body and target rows together (`derive_source_function_rosters`,
`legalization_source.rs:3588`; called at `legalization.rs:1023`, with `validate_source_custody` at `legalization.rs:989`).
`legalization_scalar_graph.rs` is the "input custody" module that checks the earlier forms (abstract plan, optimized unit,
target rows) against each other, function by function, and rebuilds the expected target argument/call rows.
`legalization_reference_custody.rs` is a flow analysis that tracks every reference (loan) held in a structural place and is called
from the target-row matcher `legalization_target.rs`.

---

## 1. `legalization_source.rs` (lines 1-3739)

**Purpose.** Projects target functions into `LegalizedScalarFunction` rosters for instruction selection (header comment `:1-5`).
It has two modes: `derive_target_function_rosters` uses only target rows plus retained accepted-obligation facts; `derive_source_function_rosters`
additionally walks the optimized body (`audit_project` paths) so an independent audit can compare. (read)

**Shape.**
- `SourceFunctionRosters { scalar_functions: Vec<LegalizedScalarFunction> }` `:3584`.
- `derive_source_function_rosters(target, abstract_plan, unit, closed_types)` `:3588`: builds two `NativeGeometry` values (plan and unit), wraps them in `NativeInput`, calls `validate_source_custody` `:15` / `:3618`, `validate_native_callback_roster` `:3619`, then `audit_roster::derive_remaining` `:38`.
- `derive_target_function_rosters(target, evidence, types)` `:3633`: per function gets geometry from `NativeGeometries` `:3638-3655`, loops blocks, skips `CleanupCall` rows `:3660-3665`, calls `scalar_graph::instruction::project` (re-export of `legalization_instruction::project`, `:1958`), builds block parameters and calls `legalization_control::project` for the terminator `:3706`.
- Nested modules under `scalar_graph`: `instruction::{call_instructions, scalar_instructions, storage_instructions}` (project one target row into a `LegalizedScalarInstructionKind`), `instruction::audit_project` `:1961` (the one big dispatcher over target-row kinds and abstract operations), `instruction::with_metadata` `:2900`, `structural_case::project` `:2935`, `terminator::audit_project` `:3086`, `terminator::successor` `:3422`, `parameters` `:3454`, `derive` `:3479`.
- Private test module `retained_storage_tests` `:1743` (one `#[test]`, `:1754`).

**Inputs and outputs.** Reads `TargetOperationPlan`, `AbstractOperationPlan`, `PsiOptimizationUnit`, `ModuleTypes`, `PreparationEvidence.accepted_obligation_facts` and `ownership_frontier_facts`. Produces owned `LegalizedScalarFunction` values. No mutation of inputs. (read)

**How it decides.** Almost every refusal is `Error::custody()` with no message. Keys:
- Function-level join is by position, with a machine identity check: "Both stages retain the admitted function order. Join once" `:47`; `target_function.machine != optimized.machine` `:50`. Count mismatch `:24-25`, `:44`.
- Block join by position and block id `:3491`, `:3516`; instruction join by position (`body.len() != rows.len()` `:3516`). Cleanup rows are split off by "first `CleanupCall`" `:3501-3515`: everything from the first `CleanupCall` on is treated as cleanup, in the audit path only. (read)
- Row-kind dispatch in `audit_project` is a chain of `matches!` tests on the target row kind first (`:1977-2300`), and only then a `match` on the abstract operation `:2306-2893`. The abstract op decides only for kinds without a dedicated target row.
- Integer arithmetic is keyed on `(overflow policy, operator)` `:344-365`; the carrier check is `saturating_carrier(scalar_type)` and `value_type(optimized, v) == Some(ScalarType::Integer(scalar_type))` `:379-385`. Division needs exactly one accepted fact: `accepted_nonzero_divisor_fact` `:499-514` (`facts.next().is_some()` refuses a second).
- `project_sequence_length` refuses anything but a `u64` result: `scalar_type.sign() != Unsigned || bits() != Some(64)` `:901-906`; `length_byte_offset` must be `0 | 8` `:932`, `:948`.
- `project_byte_access` requires result scalar type unsigned 8-bit `:1444-1445`.
- `project_element_view_read` requires every path segment to be `Field` or `FixedIndex` ("the scalar leaf inside that element is static" `:1580`) `:1586-1591`.
- `InstallStaticInterrupt` takes call arguments by hard-coded positions: `arguments[if position == 0 { 0 } else { 5 }]` `:2155`.
- Hosted write byte requires exactly one argument `[source]` `:204`.

**Cases by shape.**
- `project_boundary_call` accepts only `BoundaryRealization::HostedWriteByteI32` `:212-221`; other realizations are refused.
- Return terminators: `AbstractOperation::Return` only when `scalar_shape(*scalar_type).is_some()` `:3344`; everything not listed falls to `_ => Err(custody)` `:3378`. Same for the instruction `match` `:2893`.
- `Jump` needs exactly one successor `:3357`; `Conditional` exactly two `:3367`.
- `audit_project`'s boundary-call arm is guarded by `ieee_float_arithmetic::row(...)?.is_some()` and then only two target-row kinds (`IeeeFloatClassify`, `NearestIeeeFloatArithmetic`) `:2698-2753`; a third `BoundaryCall` arm takes only `HostedReadByte` `:2755-2773`.
- `structural_case::project` demands `cases.len() == node.successors.len() == layout.cases.len() == stored_cases.len()` `:2987-2991` and `case_tag == position` (`i32::try_from(case_position)`) `:3001-3003`; refuses any `pointer_slots` with `scalar_descriptor` `:2971`.
- Cleanup attachment after a terminator: only `Return` (to `ReturnCleanup`), `Jump`, `Conditional`, `StructuralCase` are handled `:3383-3419`; other terminators with cleanup calls get `_ => Err(custody)` `:3416`.

**Same job elsewhere.**
- `terminator::audit_project` (`:3086`) and `instruction::audit_project` (`:1961`) duplicate, in expected-row form, the projection done in the ordinary `project`/`legalization_control::project` path; the file says so: "Reconstruct the expected execution only for the independent audit" `:1960`, "Source oracle for the independent correspondence audit only" `:3085`. Ordinary path: `derive_target_function_rosters` `:3633`. (read; the ordinary `project` and `legalization_control::project` bodies are in other files, not read: suspected.)
- `call_instructions::project_call` `:95` duplicates the `NativeCallOrigin`/argument lowering in `legalization_scalar_graph.rs` `structural_call` modules only in outcome, not code.
- `target_lowering/references.rs:703` has another `access_call_custody` (grepped, not read): suspected same job as `legalization_reference_custody.rs:7542`.

**Unused or one-caller code.** (greps with the Grep tool over `source-rust`, content mode)
- `derive_source_function_rosters`: 3 hits outside its definition (`legalization.rs:491` re-export, `:1023` call; `legalization_source.rs:3588` def). So one real call (`legalization.rs:1023`).
- `derive_target_function_rosters`: 2 calls (`legalization.rs:666`, `:777`).
- `validate_source_custody`: 2 calls (`legalization.rs:989`, `legalization_source.rs:3618`).
- `parameters` `:3454` is called at `:3490` and `:3724`.

**Stated limits.** "belongs to the independent correspondence audit" appears as a delegation note at `:731-733`, `:859-861`, `:975-977`, `:1255-1256`, `:1405-1407`: the ordinary projection deliberately skips rejoining to the earlier body. No "unsupported" comments.

**Fail-open risks.**
- Cleanup split in the audit path `:3501-3515`: if no `CleanupCall` row exists, `unwrap_or(targeted.operations.len())` keeps all rows; if a non-cleanup row follows a `CleanupCall`, it is silently dropped from `rows` and the length check at `:3516` would then count it against `body.len()`. Traced (read): the length check `body.len() != rows.len()` catches a missing one, but a stray `CleanupCall` in the middle would shorten `rows` and be refused only because lengths differ; so the wrong program is refused. "none seen" beyond that.
- Ordinary path `derive_target_function_rosters` `:3660-3665` drops every `CleanupCall` row without checking it. Classification: stated (the file comment `:3632` "Ordinary legalization consumes only target execution and immutable judgments"); cleanup calls are re-attached from terminator rows in `legalization_control::project` (not read). Open for whether cleanup calls are validated at all on this path (they are validated by `cleanup_calls::validate_target` in `legalization_scalar_graph.rs:3054`, audit path only, read).
- `project_structural_byte_sequence_field_byte_store`, `..._field_store`, `project_byte_sequence_store`, `project_byte_sequence_concatenation`, `project_structural_leaf_copy`: take the destination from the target row (`let ...Store { destination, .. } = targeted`) but the value/length/obligation from the abstract node; no comparison between the two for equal places. Traced? No: open. Wrong input that reaches it: a target row whose `destination` disagrees with the abstract `destination`. The audit path pairs them by position `:3520`; no field-by-field equality.
- `project_primitive_scalar_read` `:960-988` and similar: "The producer retained ... Source reconstruction belongs only to the independent correspondence audit" `:975-977`. Stated.
- `audit_project` `with_metadata` `:2900` copies `work`, `effect`, `ownership` from the target row without cross-checking with the optimized node (`source.metadata`). Open.

---

## 2. `legalization_scalar_graph.rs` (lines 1-9330)

**Purpose.** Checks "scalar-graph input custody" for target legalization and audit (header `:1-7`): per-function ABI/call-plan headers, aggregate results, borrowed windows, byte/element views, normalized foreign calls, structural call/case/field operations, scalar node admission. It also holds the helper functions the other two files call (`value_type`, `scalar_shape`, `callee_plan`, `unique_row`, etc.). It mounts the other two files as modules: `reference_custody` `:5742`, `target` `:8752`. (read)

**Shape.** One flat file of `pub(crate) mod` blocks (all read):
- `address_joins` `:36`: `parameter` `:48`, `call_argument` `:84`, `window_argument` `:161`, `edge_bindings` `:213`, `loan_access_permitted` `:263`, `root_type` `:275`.
- `aggregate_results` `:362`: submodules `borrowed_arguments` `:376` (`reconstruct` `:389`, `reconstruct_at_owner` `:418`, `view_arrival` `:622`), `owned_arguments` `:648` (`reconstruct` `:662`, `reconstruct_at_owner` `:684`, `call_site_at_owner` `:866`, `owned_projection` `:926`, `trivial_affine_local` `:982`); `uses` `:1030`, `sum_type_layout` `:1086`, `cleanup` `:1095`, `header` `:1150`, `block_home_layout` `:1296`, `call_argument` `:1333`, `call_argument_invocation` `:1357`, `borrowed_block_arrival` `:1482`, `result_home` `:1494`, `home_layout` `:1552`, `type_home_layout` `:1559`.
- `atomic_access` `:1585`: `Access` `:1591`, `event` `:1603`, `access` `:1688`.
- `borrowed_windows` `:1767`: `WindowExtent` `:1776`, `extent` `:1784`, `moved` `:1930`.
- `byte_views` `:1963`: `validate` `:1970`, `writable_bytes` `:2065`, `view_type` `:2119`, `descriptor_return` `:2147`, `owned_byte_result` `:2206`, `completed_descriptor` `:2252`, `descriptor_result` `:2277`, `CallDescriptorOrigins` `:2292` (uses a `OnceCell` descriptor graph `:2296`).
- `call_origin` `:2422`: `validate_boundary_origins` `:2431`, `validate_qualified_numeric_executions` `:2496`, `selected_boundary_operation` `:2587` (rewrites a `BoundaryCall` node into an equivalent `CallStructural` abstract op from the target `Call` row, `:2770-2863`).
- `call_signature` `:2867`, `cleanup_calls` `:2928` (`expected` `:2936`, `validate_target` `:3054`), `control` `:3100` (`validate` `:3105`), `element_views` `:3339`, `header` `:3527` (`function_abi` `:3534`), `hosted_scalar` `:3637`, `ieee_float_arithmetic` `:3701`, `interrupt_installation` `:3886`, `normalized_foreign` `:3994` (`result_at` `:4009`, `validate_native_callback_roster` `:4127`, `native_callbacks_at` `:4160`, `structural_argument_at` `:4240`, `validate_scalar_backing_width` `:4598`), `place_declarations` `:4691`, `read_byte` `:4776`, `scalar_arrays` `:4953`, `structural_case` `:5013`, `invocations` `:5487`, `literals` `:5542`, `primitive_locals` `:5568`, `structural_call` `:5743` (`array_homes`, `byte_views`, `exclusive`, `argument_at` `:6257`, `argument_at_invocation` `:6281`, `view_argument` `:6531`, `primitive_argument` `:6687`), `structural_fields` `:6758` (`scalar_store_destination`, `replacement` `:6874`, `view_replacement` `:7079`, `byte_read` `:7210`, `read` `:7267`), `nodes` `:7511` (`admit` `:7539`, `validate` `:7896`).
- Free functions at the root: `structural_parameters` `:7419`, `structural_contract` `:7428`, `validate_structural_contract` `:7462`, `source_structural_contract` `:7476`, `match_input` `:8848`, `callee_plan` `:9120`, `scalar_carrier_shape` `:9226`, `scalar_register` `:9238`, `scalar_stack` `:9245`, `integer_type` `:9255`, `value_type` `:9269`, `unique_row` `:8836`, `BlockEntryCustody` alias `:8833`.
- One test `sum_layout_refusal_retains_its_geometry_cause_and_operation` `:9287`.

**Inputs and outputs.** Reads `TargetFunction`/`TargetOperationPlan`, `AbstractFunction`, `PsiOptimizationFunction`, `NativePlan`/`NativeUnit`, `FunctionInputs`, `Custody`. Mostly returns `Result<_, LegalizationError>` or `Option`; produces `TargetStructuralArgument` rows and `CallPlan` values that the target-row matcher (`legalization_target.rs`) compares against the stored rows. `match_input` `:8848` is the driver: it returns `()` after calling `validate_target` (`:9118`, in `legalization_target.rs`, not read).

**How it decides.**
- Function header (`aggregate_results::header` `:1150-1294`, `byte_views::validate` `:1970-2063`, `header::function_abi` `:3534-3635`): re-evaluates the call plan with `evaluate_call_plan(CallingPolicy::native_for_target(native), &CallSignature{ parameters, result })` and demands equality with the stored `call_plan` `:1211-1227`, `:3588-3634`. Which header variant to use is chosen by `aggregate_results::uses(optimized)` (`:1030`), then `structural_parameters(target).is_some()` (`:7419`), else scalar `function_abi` `:8870-8876`, `:9190-9196`. Keyed on shapes of the optimized function (presence of owned structural parameters or structural result), not on a declared mode.
- `uses` `:1030-1084` is a hard-coded list of abstract operations (`StructuralCaseMembership`, `EstablishStructuralArray`, ... about 25 variants) whose presence selects the aggregate header. A new structural op not on the list would route to `byte_views::validate` or `function_abi`.
- Place identity: `PlaceId` and `StructuralPlaceKind` matching (`place_declarations::matches_producers` `:4695-4774`), block dominance (`dominates` from `legalization_target`, e.g. `:108`, `:519`, `:771`), operation/block position (`node_site` `:946`, `invocations::position` `:5505`).
- Source-spelling dependence: none found; keys are ids and positions. Exception: `with_metadata`-style messages are `&'static str` reasons (`:8258-8270`).
- Scalar node validation `nodes::validate` `:7896-8750`: for each non-terminator node, `admit_with_source` picks `(operation, result value)` then the code branches by op kind; a node must have `provenance == [PsiProvenance::Operation(operation)]`, non-empty `work` all with that site `:7960-7969`; definitions must be exactly `[definition]` with `site == ValueDefinitionSite::Node{block, node: position}` `:8392-8403`; then an `expected_type` is computed per op and compared to `definition.scalar_type` `:8745`.
- Integer scalars: `integer_type` `:9255-9267` admits exactly four types: `u64, i64, u32, i32`. `valid_literal` `:7870-7880` admits integer constants only when `bits()` is 8/16/32/64. `scalar_carrier_shape` `:9226-9236` lists six shapes (float 4/8, integer 1/2/4/8 bytes).
- `trapping_form` `:8773`, `supports_wrapping_division` `:8819`: keyed on `IntegerType`.
- Exclusive-borrow argument shape `structural_call::exclusive::argument` `:6106-6216`: a 5-way `match` on `(projection, byte_field_projection, byte_view, element_view, byte_owner)` with `_ => Err`.
- Access permission table `loan_access_permitted` `:263-273`: `(Mutable -> Shared|Mutable|WriteOnly) | (Owned|Shared -> Shared) | (WriteOnly -> WriteOnly)`.

**Cases by shape (quotes).**
- `address_joins::window_argument` `:161`: accepts only a fixed byte-array or fixed element-array view window; otherwise returns `None`.
- `call_origin::validate_qualified_numeric_executions` `:2496`: only `NearestIeeeFloatArithmetic` is consumed (`filter_map(... NearestIeeeFloatArithmetic ...)` `:2507-2510`). Boundary kinds beyond that are not examined there.
- `read_byte::validate` `:4838`: only these four targets pass: `linux_x64, macos_x64, linux_arm64, macos_arm64` (`:4882-4888`). Windows x86_64 is not in the list. (read; grepped nothing else.)
- `read_byte::layout` `:4789`: sum type must have exactly two cases `[empty, byte]` with the second having one `i32`-like field in 0..=255 `:4808-4828`.
- `scalar_arrays::shape` `:4957`: only literal-length `FixedArray` of `PrimitiveScalar`, loops at most `plan.structural_types.len()` times `:4971`.
- `interrupt_installation::validate` `:3891`: exactly two call rows; first `Validate`, second `Work`, with source argument positions 0 and 5 `:3951-3956`. `signature.scalar_parameter_count() != 0 || structural_parameter_count() != 1` refused `:3964`.
- `normalized_foreign::native_callbacks_at` `:4160`: at most one callback with a `NativePlace::Parameter` destination `:4224-4236`; pointer size must be the application shape `:4209-4211`.
- `ieee_float_arithmetic::validate` `:3728`: two row kinds only, `IeeeFloatClassify` and `NearestIeeeFloatArithmetic`; others `Err(custody)` `:3824`.
- `hosted_scalar::validate_normal_call_completions` `:3643`: every `BoundaryCall` must name a boundary whose completion is `NormalReturn`.
- `control::validate` `:3105`: a match over terminator kinds; fall-through `_ => Err` `:3294`.
- `nodes::validate`: `AbstractOperation::IntegerWiden` requires source bits in `{8,16,32,64}`, target `Fixed` carrier and bits in `{16,32,64}` `:8692-8695`.
- `nodes::scalar_instruction` ends `_ => Err(NodeRejection::UnsupportedFamily)` `:7850`, which becomes `LegalizationError::UnsupportedScalarOperation` `:7953-7957`.

**Same job elsewhere.**
- `aggregate_results::borrowed_arguments::reconstruct_at_owner` `:418`, `owned_arguments::reconstruct_at_owner` `:684`, `structural_call::exclusive::argument` `:6106`, `byte_views::argument` `:5934`, `array_homes::argument` `:5766`, `address_joins::call_argument` `:84`, `normalized_foreign::structural_argument_at` `:4240`, `reference_custody::referent_argument` (`legalization_reference_custody.rs:7418`): six-plus routines that each rebuild a `TargetStructuralArgument` for a call argument, each with its own source lookup (caller parameter / block parameter / operation result) and the same final field list (`place, access, path, root_structural_type, structural_type, shape, source_byte_offset, fixed_array_length, element_stride, indices, source, destination`). Dispatch among them is in `call_argument_invocation` `:1357-1480` and `argument_at_invocation` `:6281-6529`, which are themselves two overlapping dispatchers.
- Source lookup of a place ("caller structural parameter, else block parameter, else operation result") is repeated: `:473-578`, `:725-839`, `:5802-5856`, `:5959-6037`, `:4257-4393`, `7454-7496` (custody file). Same job, different code.
- `trivial_affine_local` `:982` and `place_declarations::matches_producers` `:4715-4731` both count `EstablishTrivialAffineLocal` establishments.
- `byte_views::completed_descriptor` `:2252` / `descriptor_return` `:2147` / `structural_fields::*` repeat the check "multiplicity is nonlinear and projected qualifications are empty and claims are empty".
- `nodes::validate` mirrors `instruction::audit_project` in `legalization_source.rs:1961` operation-by-operation (suspected duplicate classification of which abstract ops are admitted: `nodes::admit_with_source` `:7545` vs the `match` at `legalization_source.rs:2306`).
- `legalization_target.rs` (not read) has its own `dominates`/`sources` helpers used from here (`target::control_flow::sources::dominates`).

**Unused or one-caller code.** Greps (Grep tool, count/content, over `source-rust`, `*.rs`):
- `match_input` / `function_abi` / `scalar_carrier_shape` / `unique_row` / `callee_plan` via `scalar_graph_input::` or `match_input(`: 4 files, 1 hit each (counts in `legalization.rs`, `legalization_scalar_graph.rs`, `legalization_source.rs`, `legalization_target.rs`). Not enough to conclude per-function counts; I did not run per-name counts for the rest. Not claimed unused.
- Re-exports with `#[allow(unused_imports)]` at `:9-35` and `:8753-8757`: these exist to avoid unused-import errors; `OWNER_CODE_CONCERNS.md` (per crate map) names this file for an unused import.
- Functions defined `pub(crate)` here that I did not find a call to inside this unit (not grepped repo-wide): `i64_type` `:9252` (used `:9258`), `u32_type` `:8759` (used `:9258`), `precedes` `:959` (used via re-export `:1028`, `:5882`), `call_argument` `:1333` (wraps `call_argument_invocation`; callers not in this unit), `view_replacement` `:7079`, `replacement` `:6874`, `scalar_store_destination` `:6768`, `byte_read` `:7210`, `read` `:7267` (callers are in `legalization_target.rs`/optimization crates, not read: suspected used).
- `nodes::instruction` `:7535` is re-exported `:8755`; callers not grepped.

**Stated limits.** `:5500` "Cleanup calls belong to their edge/action roster, not a node." `:7662-7664` "Instruction discovery also classifies independent raw metadata. Actual source validation retains its catalog here; a miss never falls back to independently closing the same spelled type." `:8859-8861` comments on SSA custody order. `:1128-1129` "A byte literal establishes a descriptor home directly." No "unsupported" or migration-debt comments.

**Fail-open risks.**
- `address_joins::edge_bindings` `:228-230`: `if !is_address_join_in(declaration, ...) { return true; }`. Any destination parameter that is not an address join is accepted without checking the binding. Wrong input that reaches it: a jump edge whose structural binding for a non-address-join parameter is arbitrary. Traced? Not traced (callers in `legalization_target.rs` not read). Classification: open.
- `normalized_foreign::structural_argument_at` `:4553-4576`: the path-segment screen is skipped when `presentation.is_some()` (`presentation.is_none() && ...`), and the bounds check `source_byte_offset + size <= root size` is skipped for `presentation.is_some()`, `projects_view_backing`, or a `Referent` path `:4566-4572`. Classification: open (no comment says why).
- `structural_call::argument_at_invocation` `:6468-6527`: later arms send borrowed arguments to `primitive_argument` for any `SharedBorrow` argument whose path is all `Field`/`FixedIndex` `:6488-6507`, before any shape is classified; `exclusive::argument` then refuses unless a single projection case matches. Refuses by default (`Err` `:6528`); none seen as accept-by-default.
- `address_joins::root_type` returns `None` for unknown cases, and callers use `.is_some_and` / `and_then`, so a `None` refuses. none seen.
- `nodes::validate` `:7931`: nodes that are `EstablishPrimitiveLocal` / `PrimitiveLocalStore` skip the typing section `:7970-7979` (checked separately in `primitive_locals::validate` `:5667`, called `:8869`). Stated by structure, not comment; traced to `match_input`.
- `call_origin::validate_qualified_numeric_executions` `:2556-2583`: `let Ok(index) = executions.binary_search_by_key(...) else { continue; }` skips any `NearestIeeeFloatArithmetic` row whose boundary was not found in the "qualified" executions list `:2570`. A row for an unqualified boundary is therefore not compared to its expected execution here. Classification: stated for the filter above `:2493-2495` ("Check only numeric boundaries consumed ... Each distinct qualified declaration is authenticated once"); the unqualified case is checked elsewhere only if `ieee_float_arithmetic::validate` runs (`:3728`). Traced? partly.
- `call_origin::validate_boundary_origins` `:2476-2488`: an `InstalledProvider` call with `assumption: None` is accepted without any binding check. Stated by the data model (only `Some` assumptions need to be in `input.assumed_bindings`).
- `hosted_scalar::validate_normal_call_completions` has an unused `_native` parameter `:3644` (read).
- `tests`: one.

---

## 3. `legalization_reference_custody.rs` (lines 1-7808)

**Purpose.** Tracks every reference occurrence ("leaf") held in structural places through a function's blocks, to decide whether a call, release, move, store, cleanup or return is allowed given the loans (borrows) that are still live. It is a forward data-flow analysis over blocks (`block_entry_states` `:7106`) that carries a `Custody` value. Header `:1-6`. It is mounted as `scalar_graph_input::reference_custody` (`legalization_scalar_graph.rs:5742`). (read)

**Shape.**
- `Custody` `:3854`: `leaves: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>`, `retired: BTreeMap<OccurrenceIdentity, RetiredOccurrence>`, `edge_exits: Vec<(EdgeId, BlockId, leaves, retired, Arc<Sources>)>`, `borrowed_sources: Arc<Sources>`, `descriptor_backing`, `stored_calls`, `reference_position`, `placement_catalog`, `access_formals`.
- `Leaf` `:3686`: `identity: OccurrenceIdentity`, `root: Root` (alias of `target_operations::ReferenceRoot`), `backing: Vec<Backing>`, `lifetime_debts`, `parents`, `contract: CarrierContract`, `formations`, `referent`, `placed`.
- `OccurrenceIdentity { place, path, formation }` with `OccurrenceFormation::{Parameter, Operation(id), ResidentJoin(block)}` `:1977-1989`.
- Submodules: `borrowed_arrivals` `:25` (liveness `:31`, restoration `:205`, `Source` `:444`, `captured_storage` `:468`, `normalize` `:504`, `bind` `:531`, `lends_from` `:621`, `merge` `:713`), `carrier_reads` `:889`, `descriptor_backing` `:1000` (`Backing`, `Observations`, `prepare` `:1050`, `capture` `:1293`, `overlaps` `:1362`, `matches_return` `:1454`), `lifetime_debts` `:1475`, `observations` `:1547` (22 `Refusal` variants `:1597-1620`), `occurrences` `:1966`, `resident_joins` `:2453`, `retirements` `:2621`, `stored_calls` `:2802` (`prepare` `:2880`, `stage` `:3266`, `commit` `:3599`), `call_paths` `:4617`.
- Free functions: `apply` `:6015` (per node), `apply_terminator` `:6886`, `block_entry_states` `:7106` (worklist to fixed point), `entry` `:7376`, `call_arguments` `:5269`, `call_results` `:5528`, `call_transition` `:7633`, `access_call_custody` `:7542`, `referent_argument` `:7418`, `reference_results` `:7519`, `accessor_scalar_custody` `:7677`, `validate_return_custody` `:6690`, `complete_reference_outputs` `:6817`, `relocate_subtree_leaves` `:5887`, `discard_*` `:5791-5879`.
- Tests: `borrowed_arrivals::liveness::tests` (3), `restoration::tests` (3), `borrowed_arrivals::tests` (4), `call_origin_tests` (8). (`grep` not run; counted by reading `#[test]`.)

**Inputs and outputs.** Reads the optimized function, `NativeGeometry`, `TargetOperationPlan`, `NativePlan`/`NativeUnit`, `DescriptorInputs` (`descriptor_backing::prepare` `:1050`, `stored_calls::prepare` `:2880`). Produces per-block entry `Custody` (`BTreeMap<BlockId, Custody>`), `TargetReferenceResult` rows for calls (`call_results` `:5738`), `TargetAccessCallBinding` rows (`access_call_custody` `:7630`), `TargetStructuralArgument` for referent paths (`referent_argument` `:7497`), or `Err`. `Custody` is cloned, mutated per node, and discarded.

**How it decides.**
- Identity: occurrence identity `(place, path, formation)`; storage conflicts keyed on `(PlaceId, path)` plus `Root` origin base (`Storage{root}`, `IncomingContents`, `Invocation`, `Provider`) and path overlap via `terminal_semantics::reference_origin_paths_may_overlap` (e.g. `:983`, `:3763`). Not on spelling.
- Overlap: `roots_overlap` `:3751` (may overlap: any non-`Storage` base counts as overlapping `:3756-3761`), `roots_definitely_overlap` `:3768` (all pairs, returns false on `Element`, `AllElements`, empty `FixedRange` `:3795-3803`).
- Access: `access_covers = terminal_psi::structural_access_covers` `:4615`.
- Call argument rule (`call_arguments` `:5269-5497`): only paths of `Case|Field|FixedIndex|RuntimeIndex` segments for non-referent paths `:5311-5320`; a runtime index path to a reference-containing type is refused `:5303-5310`; pairwise duplicate/overlap refused unless both ordinary storage or alternatives under different `Case`s `:5424-5471`.
- Return rule (`validate_return_custody` `:6690`): each returned leaf must be covered by a declared origin in the result's `reference_sources` mapping `:6720-6777`, or its `backing` must match `matches_return` `:6779`.
- Block entry (`block_entry_states` `:7106-7374`): worklist over blocks, `deferred` set for joins that cannot yet merge, `failures` map (an error in a block does not abort until the worklist ends `:7237-7243`, `:7326`). Impossible flow positions/edges are skipped using proofs from `terminal_semantics::descriptor_results::FlowFeasibility::ProvedImpossible` `:6023`, `:6893`, `:6921`, `:7141-7152`.
- Env var dependence: `OMEGA_COMPILER_CHECK_SITES` and `OMEGA_RESIDENT_OCCURRENCE_DIAGNOSTIC` only add `eprintln!` output (`:1701`, `:2704`, `:5331`, many others). They do not change verdicts (read).

**Cases by shape.**
- `entry` `:7376`: leaf paths computed by `leaf_paths(types, root, 4096.saturating_sub(...))`; "4096" is a hard cap on reference leaves per function (`:7388`, also `:6136` `leaf_paths(types, result.structural_type, 4096)`); exceeding it returns `invalid()`.
- `apply` `:6015` is a `match` on the abstract operation: view-formation `BoundaryCall`, `StructuralLeafCopy`, `EstablishAccessReference`, `EstablishReference`, `ReleaseReference`, `MoveStructuralValue`/`TransformContent`, `EstablishRecord`/`EstablishCase`, `EstablishStructuralArray`, `MoveStructuralField`, `StoreStructuralField`, `CallStructural` (structural result and unit/scalar), `BoundaryCall`; `_ => {}` `:6676` means other operations do not change custody.
- `StructuralLeafCopy` only for shared-copy types: `shared_copy_type` `:4196` (all references inside must be `SharedBorrow`).
- `EstablishReference` requires `result.multiplicity == Affine`, no qualifications/claims `:6230-6233`; `EstablishStructuralArray` requires literal-length fixed array `:6429-6442`.
- `window_regions` / `window_storage` `:4111-4182`: `regions.len() != 1` refuses `:4178`; a window must resolve to a single region.
- `call_paths::observe_direct_reference_custody` `:4728`: loops once per `Referent` segment; `cursor_budget = leaves.len().max(1) * path.len()` caps expansion `:4753-4758`, refusing when exceeded `:4904`.
- Resident joins: `resident_joins::merge` `:2465`; unmergeable arrivals return `Ok(None)` and the block is `deferred` `:7306-7310`; at the end any deferred block fails `:7342`.
- `stored_calls::prepare` `:2880`: only machines with writable reference slots (`writable_slots` `:2864`) get summaries; anything the stored-effect summary cannot express (`DescriptorStoredOrigin::Invocation|IncomingSlot|IncomingContents|ProducedContents|Literal|AccessFormation|Carried`) makes the summary `None` `:3007-3013` and a later use of that callee returns `invalid()` `:3308`.

**Same job elsewhere.**
- `target_lowering/references.rs:703` also defines `access_call_custody` (grepped in the Grep call above, not read): suspected the same job as `legalization_reference_custody.rs:7542`. Call sites: `target_lowering/control_flow_calls.rs:1027`, `:1185` use that one; `legalization_target.rs:320`, `:5133` use this one. So two implementations are each used on one side (lowering vs audit/match).
- Occurrence `merge_arrivals` `:2225`, `borrowed_arrivals::merge` `:713` and `retirements::merge` `:2783` are three separate merges of three parts of one `Custody`, called together at `:2601-2614`.
- `remap_parents` exists twice: `borrowed_arrivals::remap_parents` `:678` and `occurrences::remap_parents` `:2419` (the second calls the first `:2423`).
- Source lookup (parameter, block parameter, operation result) as in file 2: `source_contract` `:4386`, `occurrences::resident_type` `:2106`, `referent_argument` `:7454-7496`.
- The `Custody` write-back functions `retired`/`leaves` pruning (`prune_retired` `:2675`, `discard_subtree` `:5801`) and `edge_custody` `:3893` re-copy the three maps; `exit.leaves, exit.retired, exit.borrowed_sources` are repackaged as a tuple `:6985-6991` and rebuilt at `:3902`.

**Unused or one-caller code.** Grep results (content mode over `source-rust/*.rs`):
- `block_entry_states`: 1 call (`legalization_target.rs:1414`) plus its import `:242`.
- `apply_terminator`: 2 calls outside its definition (`legalization_reference_custody.rs:7233`, `legalization_target.rs:1504`).
- `access_call_custody`: 2 calls in `legalization_target.rs` (`:320`, `:5133`) plus the recursive use `:7644` in `call_transition`.
- `accessor_scalar_custody`: 1 call (`legalization_target.rs:5892`).
- `reference_results`: 3 calls (`legalization_target.rs:4862`, `:4904`, `:4952`). (Other `reference_results` hits in `loans.rs`, interpreter files are different functions.)
- `referent_argument`: called at `legalization_scalar_graph.rs:1380`, `:6309`.
- `ReferentDestination`, `window_regions`, `window_backing_roots`, `window_destination`, `referent_destination`: called from `legalization_scalar_graph.rs` (e.g. `:1794`, `:1856`, `:6804`, `:7019`, `:7131`), not grepped for counts.
- `contains_reference` `:4189`, `shared_copy_type` `:4196`: not grepped.
- `Custody::position_is_impossible`, `edge_is_impossible`: used in this file only (`:6023`, `:6893`, `:6921`, `:7253`).
- `descriptor_backing::overlaps` `:1362` is also used by `backing_intersects_storage` `:5191`.

**Stated limits.** `:7199-7200` "Keep source blocks in their original order and inspect every matching edge. Unknown targets still fail at the existing edge lookup below." `:3544-3548` comment about keeping the stored mutable reference in place for a read-only view. No migration-debt comments.

**Fail-open risks.**
- `apply` `:6023-6025` and `apply_terminator` `:6893-6895`: `if custody.position_is_impossible() { return Ok(()); }`: if the descriptor flow analysis marks the current position `ProvedImpossible`, the node is not applied at all and nothing is checked. Wrong program that reaches it: any program for which `terminal_semantics::descriptor_results::FlowFeasibility` wrongly says "impossible". Classification: traced to `descriptor_backing::prepare` `:1104` (`analysis.flow_feasibility`) but the analysis itself is in another crate, not read. Open as to analysis soundness.
- Edges the same way: `custody.edge_is_impossible(edge)` skips cleanup and arrival `:6921`, `:7253`. Same classification.
- `descriptor_backing::prepare` `:1050-1076`: if no structural parameter or structural result contains a reference, returns default `Observations` with no checks. Stated by context (no references to track).
- `descriptor_backing::prepare` `:1085-1087`: `let Some(graph) = graph else { return Ok(observations); }` returns with empty flow/positions when `descriptor_inputs.views()` is `None`. Later `capture` `:1325-1329` returns `invalid()` when a position is missing, so a descriptor-sourced observation is refused; but `path_is_impossible` returns false so nothing is skipped. Traced: refuses.
- `stored_calls::stage` `:3281-3294`: when `callee` is `None`, an argument whose slot has no writable reference slots is accepted; refused only if writable slots exist or the type cannot be resolved (`is_none_or`). Stated by structure.
- `stored_calls::stage` `:3305-3307`: `let Some(effects) = effects else { return Ok(staged); }`: if the callee has no entry in `machines`/`boundaries` (i.e. it has no writable slots), no stored effect is applied and the call is accepted. Wrong input: a callee whose writable slots were misclassified at `writable_slots` `:2864`. Classification: open.
- `stored_calls::stage` `:3513-3522`: when the source leaf is absent and its path contains a `Case` segment, the candidate is silently skipped (`continue`); otherwise `invalid()`. Wrong program: an effect through an absent case alternative is dropped. Classification: open (no comment).
- `stored_calls::destinations_may_coexist` `:2812-2831` returns `true` (may coexist) on any other mismatch, which keeps conflict detection conservative; none seen as accept.
- `call_results` `:5670-5672` and `:5623-5636`: `if candidates.is_empty() { continue; }` and `else { continue; }` on `transfers_existing` with no moved/copied leaf: a mapped result path with no source occurrence is skipped (no `Leaf` established for it) rather than refused. Wrong program: a returned reference whose source has no tracked leaf. Classification: open.
- `apply` `:6470-6472`, `:6514-6516`: `if !contains_reference(...) { return Ok(()); }` on `MoveStructuralField` and `StoreStructuralField` skips custody updates (and also skips `prune_retired` and `validate_occurrences` at `:6678-6686`). Stated by context (no references in the moved/stored type).
- `apply_cleanup_actions` `RetireHome` `:7023-7033`: removes leaves for the place after only checking the contract exists or a byte literal exists, with no conflict check. Open.
- `merge_arrivals` `:2237-2255`: absent-on-one-side leaf keys are accepted when listed in `guarded_absence` and shape matches; `guarded_absence` is computed from `path_is_impossible` `:2473-2489`. Stated by comment-free structure; traced.
- `resident_joins::merge` result `None` -> block deferred; end-of-run `deferred` non-empty refuses `:7342`. none seen.
- Env vars `OMEGA_COMPILER_CHECK_SITES` etc. only print; none seen as verdict-changing.
- `entry` `:7388`: `4096usize.saturating_sub(custody.leaves.len())` as a limit, not a fail-open.

---

## Coverage

- `legalization_source.rs`: lines 1-3739 read in full (3,739 lines; the Read tool showed a trailing empty line 3740).
- `legalization_scalar_graph.rs`: lines 1-9330 read in full (9,330 lines; trailing line 9331 empty).
- `legalization_reference_custody.rs`: lines 1-7808 read in full (7,808 lines; trailing line 7809 empty).
- Skimmed: none. Not read: none within the unit. Other files referenced (`legalization.rs`, `legalization_target.rs`, `legalization_control.rs`, `legalization_instruction.rs`, `target_lowering/references.rs`) were only grepped.
- Commands used for counts: Grep content search for `derive_source_function_rosters|derive_target_function_rosters|validate_source_custody`; Grep content search for `block_entry_states|accessor_scalar_custody|reference_results\(|access_call_custody|apply_terminator`; Grep count for `scalar_graph_input::(match_input|function_abi|scalar_carrier_shape|unique_row|callee_plan|u32_type|i64_type)|\bmatch_input\(` (result: 4 files, 1 each; not per-name).

## Five things a maintainer most needs to know about this unit

1. These three files are an audit layer: nearly every function re-derives what target lowering already produced and returns `custody()` on mismatch (`legalization_scalar_graph.rs` `match_input` `:8848`, `legalization_source.rs` `audit_roster` `:32`). The ordinary compile path uses only `derive_target_function_rosters` (`legalization_source.rs:3633`) and does not run the source-side checks; the source-side path runs from `legalization.rs:1023` (the independent audit).
2. `custody()` errors carry no reason in most places; only a few spots (`InvalidScalarStore` `:8272`, `ReferenceObservation` with a `Refusal` enum `legalization_reference_custody.rs:1597`, `InstalledInterruptAssociation` `:3911`) say why. Debugging a refusal usually needs `OMEGA_COMPILER_CHECK_SITES` output (prints only; read at `legalization_reference_custody.rs:2704` and others).
3. The reference-custody analysis (`legalization_reference_custody.rs`) is a worklist fixed point over blocks with a clone-heavy `Custody` and an `edge_exits` side table; it skips whole nodes/edges when another crate's flow analysis says `ProvedImpossible` (`:6023`, `:6893`, `:6921`). Soundness of those skips depends on `terminal_semantics::descriptor_results`, which this unit does not contain.
4. Call-argument reconstruction is spread over at least seven routines (list in file 2, "Same job elsewhere"), dispatched by two overlapping dispatchers (`call_argument_invocation` `:1357` and `argument_at_invocation` `:6281`). Each re-implements "find the caller's source for this place".
5. Several fixed lists: integer scalar types `{u64,i64,u32,i32}` (`:9255`), carriers `{float4/8, int 1/2/4/8}` (`:9226`), hosted read-byte targets exclude Windows x86_64 (`:4882-4888`), the 4096 reference-leaf cap (`legalization_reference_custody.rs:6136`, `:7388`), interrupt install call argument positions 0 and 5 (`legalization_source.rs:2155`, `legalization_scalar_graph.rs:3952-3956`).

## What you are unsure about

- Whether `address_joins::edge_bindings` returning `true` for non-address-join parameters (`legalization_scalar_graph.rs:228-230`) is covered by another check; its callers are in `legalization_target.rs`, which I did not read.
- Whether `call_results`' `continue` paths (`legalization_reference_custody.rs:5623-5636`, `:5670-5672`) can drop a returned reference that a later return check would still catch (`validate_return_custody` is for the callee's own return, not for these call results).
- The soundness of `FlowFeasibility::ProvedImpossible` (lives in `terminal_semantics`, not read).
- Which of the `pub(crate)` helper functions listed as "suspected used" in `legalization_scalar_graph.rs` have more than one external call site; I grepped counts only for a handful.
- Whether the abstract-operation lists in `nodes::admit_with_source` (`legalization_scalar_graph.rs:7545-7852`), `aggregate_results::uses` (`:1030`), and `audit_project` (`legalization_source.rs:2306-2893`) agree for every operation; I did not diff them.
- The `target_lowering/references.rs:703` `access_call_custody` was not read, so "same job" is suspected only.
