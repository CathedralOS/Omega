# Unit 110 notes

Files covered (all under `omega-rust/psi/`, checkout `<checkout>/`):

1. `foundation/symbols/src/tests.rs` (1846 lines)
2. `interpreter/checked-interpreter/src/build_machine.rs` (1924)
3. `interpreter/checked-interpreter/src/evaluation.rs` (899)
4. `interpreter/checked-interpreter/src/execution.rs` (3087)
5. `interpreter/checked-interpreter/src/execution_expressions.rs` (1707)
6. `interpreter/checked-interpreter/src/execution_scalars.rs` (1722)
7. `interpreter/checked-interpreter/src/filesystem.rs` (3916)
8. `interpreter/checked-interpreter/src/filesystem_real.rs` (3224)
9. `interpreter/checked-interpreter/src/interpreter.rs` (1119)
10. `interpreter/checked-interpreter/src/lib.rs` (88)

Paths below are relative to `omega-rust/psi/`. "read" = I read the lines; "grepped" = a Grep (count or content mode over `omega-rust`) I ran; "inferred" = not traced.

---------------------------------------------------------------------------------------------------

## 10. interpreter/checked-interpreter/src/lib.rs (1-88)

**Purpose.** Crate root of the checked-tree interpreter. It lists the modules and re-exports the public surface (read).

**Shape.** `mod` list at lib.rs:32-41 (`build_machine`, `evaluation`, `execution`, `execution_expressions`, `execution_scalars`, `filesystem`, `filesystem_real`, `pub mod interpreter`, `program_values`, `value`). `program_values` and `value` are not in this unit. Re-exports: `pub(crate) use evaluation::{build_evaluation_sponsor, build_evaluation_usage, build_time, virtual_clock}` (43-45); `pub(crate) use filesystem::{filesystem_access, filesystem_observations, filesystem_operation_attempts}` (46-48). `pub use build_evaluation_sponsor::…`, `pub use evaluation_accounting::…` (65-68), `pub use filesystem_access::…`, `pub use filesystem_authority::*` (70, a glob), `pub use virtual_clock::…` (86). `evaluation_accounting` and `filesystem_authority` are external crates (they are used as `use filesystem_authority::…` at filesystem.rs:11 and `use evaluation_accounting::…` at evaluation.rs:360).

**Inputs/outputs, decisions, fail-open.** None; re-exports only. Fail-open: none seen.

**Stated limits.** None.

---------------------------------------------------------------------------------------------------

## 9. interpreter/checked-interpreter/src/interpreter.rs (1-1119)

**Purpose.** Holds the interpreter's entry points, the `Evaluator` struct (all per-run state), frame types, numeric helpers, directory-record packing helpers, and the thread/stack setup. It also holds the halt type (read).

**Shape.**
- `evaluation_error` module (10-67): `UnavailableStaticInput` {Binding, TypePredicate} (14), `BuildMachineEvaluationError` {StaticInputUnavailable, Rejected} (40).
- `evaluator` module (69-716):
  - `directory_entries` (73-184): `pack_dirent_records` (141) packs a Linux-style dirent buffer (reclen at +16, namlen at +18, type at +20, name at +21; `25 + name_len` rounded up to 8, 82-99). `dirent_record_chunk` (164) takes whole records up to a byte count.
  - `Halt` enum (187): Exit, Crash, Unsupported, Trap, Resource, StaticInputUnavailable.
  - `scalar_numerics` (218-410): `semantic_float_result` (230), `float_operand` (246), `integer_bounds` (283), `semantic_integer_format` (296), `wrap_to_width` (378), `primitive_bit_width` (369), `apply_arithmetic_domain` (349).
  - Constants (482-499): `STEP_BUDGET = 10_000_000`, `CONST_EVAL_STEP_BUDGET = 100_000`, `CALL_DEPTH_BUDGET = 512`, `MAX_DIRECTORY_SNAPSHOT_BYTES = 16 MiB`, `MAX_DIRECTORY_ENTRY_NAME_BYTES = 255`, `VIRTUAL_MTIME_SECS = 1_000_000_000`, `VIRTUAL_UID = 501`, `VIRTUAL_GID = 20`.
  - `Frame` (519), `FrameLocal` (512), `Evaluator<'program>` (537-615, about 60 fields: program, product_declarations, checked, operator_facts, boundary_adapter_dispatch, selected_build_time_*, stdout/stderr/build_log, root_build, executed_* result vectors, product_*_descriptions, `virtual_*` maps (files, fds, dirs, finds, perms, symlinks, times, flocks, char_devices, errno), `real_fs`, `rooted_build_paths_required`, `output_*` maps, `filesystem_*` attempt/handle state, meters, counters), `TransitionDecision` (628).
  - `ambient_step_budget` (484) reads env var `OMEGA_INTERP_STEP_BUDGET`.
- Public API (718-1119): `InterpretOptions` (755), `interpret_entry` (765), `BuildMachineEntry` {Name, Symbol} (783), `SelectedBuildTimeMachineBody` (789) with `validate_all` (795), `BuildMachineEvaluationRequest` (827), `BuildMachineProgram` {Typed, Checked, CheckedClosure} (835), `materialize_zero_initialized_build_time_value` (874), `pure_evaluation_step_ceiling` (901), `evaluate_prepared_build_time_machine` (905), `evaluate_structured_return` (935), `interpret_on_current_thread` (1001).
- Threading: `INTERPRETER_STACK_SIZE = 256 MiB` (730); `run_with_interpreter_stack` (736) spawns a scoped thread unless the thread-local `ON_INTERPRETER_STACK` is set (732-752).

**Inputs/outputs.** `interpret_entry` takes `CheckedTrees`, an entry, stdin bytes and options. It returns `InterpretOutcome` (exit code, stdout, stderr, error, crash cause, usage). `evaluate_prepared_build_time_machine` takes a `BuildMachineProgram` plus request (entry, `BuildTimeValue` arguments, selected operators, selected machine bodies) and returns `BuildTimeOperationEvaluation<BuildTimeValue>` (value, usage, private layout placements).

**How it decides.**
- Filesystem provider is selected by `FilesystemAccess` variant (1049-1084): Virtual leaves `real_fs` None; RealUnscoped builds `RealFs::new(None,None)`; RealScoped, CapturedScoped, RealScopedSponsored build with grants.
- Virtual clock bindings: each is re-validated with `validate_for(checked)` and rejected if `overlaps` an earlier one (1032-1046). Keyed on SymbolHandle identity (see evaluation.rs).
- Entry is found by machine NAME (`run_entry(&str)`) or by SymbolHandle (1085-1090).
- Crash-qualified operator uses are refused for the whole program: `has_crash_qualified_uses` (1023-1031) and `operator.has_crash_contract` (947-954).
- `Halt::Exit(code)` becomes the exit code (1095); every other non-crash halt becomes `InterpretOutcome::error` with exit_code 0 (1102-1106; `InterpretOutcome::error` at evaluation.rs:323 sets exit_code 0).

**Cases by shape.** `integer_bounds` (283) answers only I8..I64/U8..U32 and returns `None` for U64/Addr. `semantic_integer_format` maps `Selector`, `Bool`, `F32`, `F64` to `None` (306-310). `primitive_bit_width` falls to 64 for anything not 8/16/32 (369-376). Those are fixed-list matches with a default.

**Same job elsewhere.** `integer_bounds` also appears as a differently-typed function in `compilation/validation/src/arithmetic_domains.rs:10274`, `typed-trees/src/type_system.rs:4696` (returns i128), `05_typed-trees-to-checked-trees/src/values.rs:2331`, `flow_expression.rs:5228`, `checks_contracts.rs:3558` (grepped `fn integer_bounds`; I did not read those sites, so "suspected" duplicates). `wrap_to_width` and `primitive_bit_width` had one definition each in `omega-rust` (grepped).

**Unused / one-caller.** `Evaluator::run_entry` (execution.rs:2386): one call site (interpreter.rs:1086) (grepped `\.run_entry\(`). `materialize_zero_initialized_build_time_value`, `pure_evaluation_step_ceiling`, `capture_checked_build_evaluation_usage`, `VirtualClockBinding::from_compiler_resolved_requirement`, `with_compiler_resolved_realization`, `SelectedBuildTimeBinaryOperator::prepare` together have 16 matching lines in 8 files (grepped with one combined pattern; 8 of them are definitions/re-exports in this crate), so each has outside users in `compilation/build-time-evaluation`, `omega/build-system/build-settings/.../selected_constants.rs`. I did not split the count per name.

**Stated limits.** `selected operator crash invocations have no build-time execution support` (951-953) and `...no checked execution support` (1029): crash-contract operators are refused wholesale.

**Fail-open risks.**
- `interpret_entry`: a panic in the worker thread becomes `InterpretOutcome::error` with `exit_code: 0` (772-779). `InterpretOutcome::error` always has exit_code 0 (evaluation.rs:323-337). A caller that only reads `exit_code` and not `error` would see success for a trap, an unsupported construct, a resource halt, or a panic. Which callers check `error` was not traced (open).
- `ambient_step_budget` (484-489): a malformed `OMEGA_INTERP_STEP_BUDGET` falls back silently to the default (`.ok().and_then(parse).unwrap_or`). Not a soundness issue; the budget stays finite. Stated: none.
- `float_operand`'s `expect("binary32 projection row accepts f32")` (275) and the `expect`s at 328/332 panic rather than return errors; the panic path ends up in the exit-code-0 case above (traced within this file).

---------------------------------------------------------------------------------------------------

## 3. interpreter/checked-interpreter/src/evaluation.rs (1-899)

**Purpose.** Defines the result and accounting vocabulary of an evaluation (identities, usage record, outcomes), the conversion between interpreter `Value` and `BuildTimeValue`, the selected build-time float operator record, and the virtual clock binding (read).

**Shape.**
- Identities: `EvaluationSemanticsIdentity` (13; `CURRENT_EVALUATION_SEMANTICS` = 6 at 21), `EvaluationStepScheduleIdentity` (25; current 1 at 33), `EvaluationUsageSchemaIdentity` (37; current 7 at 45).
- `EvaluationUsage` (49) with fields work_units, work_budget, build_log_bytes, filesystem_operation_attempts, peak cells/text bytes, result cells/text bytes; `charge_*` methods return `Option<()>` on overflow (184-201).
- `BuildIncludedSource` (68), `ExecutedBuildSelection` (101), `ExecutedBehaviorExclusion` (109) with Site {Statement, Expression} (116) and Kind {CrashCause, PhysicalAuthorityClass, Service} (122), `MeasuredEvaluation<T>` (218), `PrivateLayoutPlacementReceipt` (246), `BuildTimeOperationEvaluation<T>` (253), `InterpretOutcome` (297) with constructors `exited`/`error`/`crashed` (307-354), `MAX_INCLUDED_BUILD_SOURCES = 256` (357).
- `build_evaluation_usage::capture_checked_build_evaluation_usage` (373): builds an `evaluation_accounting::BuildEvaluationUsage` record and calls `record.validate()`.
- `build_time` (422): `materialize_build_time_value` (427) and `observe_build_time_value` (499); `SelectedBuildTimeBinaryOperator` (552) with `prepare` (565) and `has_crash_contract` (607).
- `virtual_clock` (629): `VirtualClockOperand`, `VirtualClockResult`, `VirtualClockOperation` {Wait, Read}, `VirtualClockBinding` (693) with `from_compiler_resolved_requirement` (705), `with_compiler_resolved_realization` (764), `matches_target` (808), `overlaps` (814), `validate_for` (821), `requirement_signature` (843).

**Inputs/outputs.** `materialize` turns a `BuildTimeValue` into a `Value` using caller-supplied allocators (so it is metered). `observe` goes the other way and dereferences `Value::Ref` through a caller-supplied reader (546-548).

**How it decides.**
- `VirtualClockBinding` identity is keyed on the ADDRESS of the `CheckedTrees` reference: `std::ptr::from_ref(checked).addr()` stored at 753 and compared at 769 and 822. A binding built against one checked program is rejected for another.
- `from_compiler_resolved_requirement` (705): Wait needs `maximum_milliseconds >= 0` and exactly one `Milliseconds` operand; Read needs `units_per_millisecond > 0`, no duration operand, and `primitive` in {I64, U64} (715-732). Operand count and result type must match the requirement signature. All parameters must be fixed-width integer primitives (743-751).
- `requirement_signature` (843): finds the requirement either as a top-level bodyless `TopLevelRequirement` machine or as a boundary-trait signature; any other count than exactly one returns `None` (894-897).
- `with_compiler_resolved_realization` (764): realization must be a bodyless, non-generic machine with exactly one state whose parameter primitives equal the binding's, and exactly one trait conformance naming the requirement (772-800).
- `LayoutTarget` is materialized as a struct whose `type_name` is the string `"@compiler-layout-target"` with four named fields (447-458). The consumer checks that same spelling and field set at `execution_expressions.rs:1342-1353`.
- Materialized `Struct`/`Case` values get `type_symbol: SymbolHandle::invalid()` (471, 477); `observe` drops the symbols (510-538). The symbols are restored later by `bind_build_time_argument` (execution.rs:594).

**Cases by shape.** Clock Read admits only `I64|U64` (726). Clock Wait/Read operand kinds are `Milliseconds | Ignored | ExactInteger(i64)` (637-641). `SelectedBuildTimeBinaryOperator::prepare` accepts only a `Binary` expression or a two-argument `Call` (578-594) and an operator for which `primitive_float_binary_semantics` returns a row (575-577).

**Same job elsewhere.** The validity re-derivation in `validate_for` (821-840) rebuilds the binding and compares for equality. `SelectedBuildTimeBinaryOperator`'s exactness checks are repeated at use time in `execution_expressions.rs:91-190`.

**Unused / one-caller.** `has_crash_contract` has one use in this crate (interpreter.rs:949) (read). See interpreter.rs section for the combined grep on the other public functions.

**Stated limits.** None in comments.

**Fail-open risks.** `capture_checked_build_evaluation_usage` (373-380) only rejects a sponsor whose `measurement()` is not `CheckedTree`; with `sponsor = None` it records zero session values (403-408 use `map_or(0, ...)`). That is by design for no-sponsor runs; no comment says why. Wrong program accepted: none (this only builds a report). Other fail-open: none seen.

---------------------------------------------------------------------------------------------------

## 2. interpreter/checked-interpreter/src/build_machine.rs (1-1924)

**Purpose.** Implements the interpreter's handling of the toolchain `Build` prelude: calls to `Build`, `BuildSource`, `BuildOutput`, `BuildLog`, `BuildProduct`, `BuildInputs` facet methods, required-output obligations, rooted build paths, product descriptions, behavior exclusions, provider selection, and root/declaration binding requests (read).

**Shape.**
- `impl Evaluator` (14-139): `try_component_assumption_acceptance` (15), `try_build_facet_routes` (37; tries path, product entry, schema, provider, description path, output obligation, required-output path, log, assumption in order), `try_build_product_description_call` (77), `require_root_build_argument` (100), `eval_byte_data_operand` (117).
- Free helpers `required_index` (141), `described_product_index` (153).
- Submodules: `behavior_exclusions` (171), `log` (372), `output_obligations` (428; `OutputSealState` at 445), `paths` (825), `product_entries` (1208), `product_providers` (1316), `product_schemas` (1390), `provider_selections` (1524), `root_bindings` (1650).

**Inputs/outputs.** Reads `Evaluator` state (`root_build`, `output_obligations`, `output_seal_states`, `output_open_writers`, `filesystem_operation_attempts`, `product_*_descriptions`, `pending_build_selections`) and the typed program. Produces marker `Value::Struct`s (see below), pushes into `executed_behavior_exclusions`, `executed_build_selections`, `executed_source_selections`, `build_log`, `build_included_sources`, `output_receipts`, `output_obligations`.

**How it decides (keys).**
- Facet method recognition is by STRING NAMES plus prelude provenance. `build_prelude_facet_method` (852-874) requires `symbol_has_build_prelude_source` on target, machine and state, then returns `(machine.attached_data name, state.name)`; callers match on string tuples such as `("BuildOutput", "require"|"complete"|"fail")` (457), `("RequiredOutput","path")` (478), `("BuildLog", LogWriteLine spelling)` (387), `("BuildSource","resolve")` etc. (878-909). `exact_build_facet_method` (1101-1119) scans ALL machines for one with `attached_data == expected_attached`, prelude provenance, and a state named `expected_state` whose symbol equals the target.
- Calls whose `target_symbol` is invalid are recognized by SPELLING: `BuildOperation::from_call_target(call.target.as_str())` (189, 221, 1543) with `!call.target_symbol.is_valid()`.
- Marker values are recognized by `type_name` string spelling: `$OmegaBuildRootedPath` (836), `$OmegaBuildSourceRoot` (837), `$OmegaBuildOutputRoot` (838), `$OmegaBuildInputSlots` (839), `$OmegaBuildLogFacet` (377), `$OmegaBuildRequiredOutput` (440), `$OmegaBuildOutputReceipt` (442), `$OmegaBuildProductFacet` (1213), `$OmegaBuildProductEntry` (1215), `$OmegaBuildProductProvider` (1322), `$OmegaBuildProductTypeSchema` (1397). Each check is `**type_name != *CONST` (e.g. 699, 1136, 1158).
- Enum case arguments (CrashCause, PhysicalAuthorityClass, CompositionMode, OutputCompletion) are accepted when the data definition has the right `name.as_str()` AND prelude provenance (312-316, 1608-1612, 737-745). The case is then found by variant NAME (334-340, 1613-1624).
- The root Build value is identified by cell pointer identity `Cell::ptr_eq(&builder, root)` (105-109, 1573-1577, 1747-1751).
- Rooted paths: `validate_build_relative_path` (1177-1205) rejects empty, > `FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT`, NUL, non-UTF-8, leading `/`, any `\`, a second char `:`, and empty/`.`/`..` components. Root identity comes from a `root` integer field of the facet struct (1139-1146).
- Output obligation collision (545-563): same root and (equal path, or one is a `/`-prefix of the other) is a trap. Max `MAX_BUILD_OUTPUT_OBLIGATIONS` (564).
- Output custody (`note_completed_build_output_attempt`, 758-821) is keyed on `(root, relative path)` and `FilesystemLogicalHandleIdentity`: Created descriptor -> `Open{since}`; last retiring handle -> `Sealed{at}`; writes through a tracked handle after `Completed` push to `post_completion_mutations`. `complete` returns enum `Retry` (not an error) when the file is not sealed (624-637).
- Selection tickets (1665-1922): a prepare call pushes `call.build_selection` onto `pending_build_selections` and returns its index as `Value::Int`; the later call must present a ticket whose stored request `==` its own (1778), and the slot is reset to `Default` after use (1919). Operation is chosen from the request variant and the protocol name of the callee signature in boundary trait `BuildBindingOperations` (1665-1681).

**Cases by shape (fixed lists).**
- Protocol list `"prepare" | "bind" | "bind_description" | "select" | "select_provider" | "exclude"` (1692-1699).
- Exactly-N argument shapes with `let [..] = arguments else trap(...)` at 26, 93, 252, 280, 407, 481, 502, 512, 523, 929, 1021, 1060, 1229, 1794, 1818, 1844-1865.
- `BuildPathOperation` (841-849) fixed list: Resolve, NamedInput, Filesystem{Open|Read|Close for Source; Create|Write|Close for Output}, IncludeSource. Other method names return `None` (891, 903).
- `provider_composition_case` accepts zero or one argument (1594-1599).
- `BuildSelectionRequest::ProviderReadiness` is always refused: `provider readiness requires a retained Terminal Build activation` (1714-1717, 1788-1791).

**Same job elsewhere.**
- Root-Build identity check is written twice: `require_root_build_argument` (100-115) and `require_provider_selection_activation` (1572-1582) (also a third inline copy in `root_bindings` 1747-1755). They differ only in message.
- The three "issue description" functions are near-copies: `issue_product_entry_description` (1276), `issue_product_provider_description` (1363), `issue_product_schema_description` (1497). Each reserves a vec slot, pushes, turns the index into `Value::Int`, and wraps it as a struct with one `description` field. `described_product_index` (153) is the shared reader; entry description uses it at 1262; provider at 1349; schema at 1483.
- Per-kind description call routing: `try_build_product_description_call` (77) is shared by provider (1332) and schema (1466); entry (1218) does not use it because it takes three arguments.
- `eval_byte_operand` (749) is a wrapper of `eval_byte_data_operand` (117) (read; call sites not grepped).

**Unused / one-caller.** `exact_build_facet_method`: 8 matching lines, all in this file (grepped): 1 definition and 7 uses, none outside the file. `try_component_assumption_acceptance`: called once at 74 (read).

**Stated limits.** `unimplemented`-style: none in this file. Refusals stated in trap text: "requires the current activation's original Build value" (112-114); "required output ... duplicates or collides" (557); "package build ... " none here.

**Fail-open risks.**
1. `try_component_assumption_acceptance` (15-35): after checking arity and root Build, it evaluates the digest expression and returns `Value::Unit`; the digest is never compared, stored or recorded (33-34). A call `accept_component_assumption(build, anything)` is accepted. Whether recording happens elsewhere (e.g. from the typed tree) was not traced. Status: open.
2. `note_completed_build_output_attempt` (758-821): a `Duplicated`/`Borrowed` handle whose source is not in `output_open_writers` is skipped silently (788-790), and a `Created` descriptor with no rooted-path operand resolution is not tracked (772-785). Writes through such handles are then not counted as post-completion mutations and the file never becomes `Sealed` here. What wrong program gets through: a write to an output file through a handle opened by a path not carried as a rooted-path resolution. Whether that path can occur: `rooted_build_paths_required` (which forbids non-rooted paths in scoped runs) is never set true anywhere in `omega-rust` (see filesystem.rs findings), so in scoped runs non-rooted paths are not forbidden by that flag. Status: open (not traced to the callers that build the grants).
3. `include_build_source` (1054-1099) is fail-closed: needs real_fs, sponsor, sealed regular file, scoped grant. None seen there.
4. `evaluated_exclusion_case` (295-348) is fail-closed (traps unless prelude case). None seen.

---------------------------------------------------------------------------------------------------

## 4. interpreter/checked-interpreter/src/execution.rs (1-3087)

**Purpose.** Core state-machine execution: assignments, boundary-adapter and dynamic-slot dispatch, console/stdin host operations, virtual clock calls, normal and replacement cleanup, statement execution, transitions, call statements, state/frame setup, and default-value construction (read).

**Shape (modules, with entry lines).**
- `assignments` (10): `Evaluator::assign` (39), `capture_assignment_destination` (204), enum `AssignmentDestination` (19: Cell, ArrayWindow, PackedByte, ScalarRecast, RecordRecast).
- `boundary_adapter_dispatch` (299): `try_selected_named_call` (304), `selected_boundary_adapter` (332), `run_boundary_adapter` (358), `run_selected_realization` (368), `run_dynamic_slot` (388).
- `boundary_console` (472): `read_stdin_byte_value` (480), `read_stdin_line` (527), `boundary_call_requirement` (548), `boundary_trait_requirement` (559).
- `build_time_arguments` (587): `bind_build_time_argument` (594).
- `host_dispatch` (697): `HostOperation` enum (707) with `from_name` (718), `virtual_time_host_value` (756), `try_host_call` (770), `try_host_invocation` (783), `try_host_value_call` (816), `try_virtual_clock_operation` (835), `serve_host_operation` (932), `line_read_result_shape` (1042), `read_stdin_bounded_line` (1077), `exact_console_intrinsic_host_method` (1137).
- `normal_cleanup` (1191): `normal_cleanup_actions` (1211), `normal_cleanup_owner` (1256), `validate_normal_cleanup_selection` (1308), `execute_completion_cleanup` (1391), `execute_nominal_cleanup_receiver` (1482), `normal_cleanup_cell` (1532).
- `replacement_cleanup` (1639): `capture_replacement_cleanup` (1685), `execute_replacement_cleanup` (1785).
- `statements_and_calls` (1798): `exec_statement` (1812), `eval_transition` (1883), `resolve_transition_target` (1932), `eval_call_statement` (2001), `resolve_entry_state_symbol` (2095), `eval_argument` (2161).
- `states_and_transitions` (2200): `Evaluator::new` (2237), `new_build` (2213), `new_checked` (2311), `tick` (2318), `configure_build_evaluation` (2331), `run_build_time_machine*` (2443, 2454), `default_value_for_type_with_bindings` (2562), `run_state_collect` (2841), `run_state_collect_inner` (2857), `bind_frame` (3009), `default_copies_by_cells` (3068).

**Inputs/outputs.** Reads `TypedTrees`/`CheckedTrees` (statement/expression/type tables, cleanup control tables under `checked.facts.flow.terminal_unit_effects`, `cleanup_dependencies`), stdin bytes. Mutates `Evaluator` (stdout, stderr, virtual_ticks, cells) and returns `Value`s or `Halt`.

**How it decides.**
- Run loop (2857-3006): per state, bind frame, `validate_normal_cleanup_selection`, evaluate statements in `finite_execution_order`; first transition whose guard holds (or its `continuation`) ends the pass. Same-machine named transition keeps `carried` locals (2993-2997); a different machine drops them (2999-3003). Step limit `usage.work_units() > step_budget` (2322); recursion limit `CALL_DEPTH_BUDGET` (2848).
- Entry state of a machine (`machine_entry_state`, 2153-2159): the state whose NAME equals the last `::` segment of the machine name, else the first state.
- Call-statement routing order (2001-2093): static requirement dispatch; dynamic slot; selected named call (boundary adapter or operator); the spelled target `asm#popfq`; five fence/hint kinds recognized by intrinsic NAME (`from_intrinsic_name`, 2054-2059); build facet routes; behavior exclusion; host call; then ordinary state call.
- Host recognition: `exact_console_intrinsic_host_method` (1137-1187) matches realization state NAMES `"ConsoleNativeProvider::read_byte" | "::write_byte" | "::exit_process" | "ProcessExitNativeProvider::exit_process"` (1151-1156, 1171-1186) and requirement names. `HostOperation::from_name` (718-752) maps spelled names (`"exit_process"`, `"write"`, `"write_line"`, `"write_error"`, `"write_error_line"`, `"read_byte"`, `"write_byte"`, `"read_line"`, `"monotonic_ticks"`, ..., `"tick_count"`). `try_host_invocation` runs when the target is either an exact console intrinsic or any boundary-trait requirement (800-804), then dispatches on the SPELLING of `target_name` or the intrinsic method (808-812).
- `ProcessExit` check: a boundary `exit_process` whose trait is named `"ProcessExit"` must be the canonical requirement (941-953).
- Cleanup: plans are looked up by machine+state symbol in `terminal_unit_effects` (1226, 1401-1423). A cleanup action's owner is found from `Parameter`, `StructuralLocal`, `StructuralResult` sources (1263-1305). Field/case path segments are matched by `path_identity()` spelling (1592, 1604, 1611).
- Assignment: receiving type from `assignment_stored_value_type`; destination captured BEFORE the replacement value is evaluated (65), replacement cleanup captured (66-85), executed after value evaluation and before the write (139).
- `default_value_for_type_with_bindings` (2562-2712) order: generic-binding substitution, constrained base, fixed array (literal / const parameter / const call / refuses expression length 2601-2605), generic record, nested non-enum data, enum zero case (first variant, 2763-2793), else `default_for_type` (primitive 0/false/0.0, borrowed byte slice -> empty text, else `Unit`, 2824-2839).

**Cases by shape (fixed lists, quoted).**
- `exec_statement` (1812-1869): Assembly/AssemblyFact are `Ok(())` no-ops (1820-1821).
- Cleanup: `"unimplemented: checked-tree interpreter selected owner cleanup"` (1237, 1345, 1448, 1714) when any `selected_regions` is non-empty; `"unimplemented: checked-tree interpreter expression-owned normal cleanup"` (1331); path segments `AllElements`, `RepresentationView`, `FixedRange`, `RuntimeIndex`, `ComputedIndex` refused (1543-1548, 1625-1630).
- `nominal cleanup` target takes only zero parameters or one exclusive reference parameter (1516-1527).
- `line_read_result_shape` (1042-1075): requires data `LineReadResult` with exactly four variants named `Invalid` (no payload), `LineComplete`, `EndOfInput`, `Full`, each of the latter three with exactly one erased-not field `count: u64`.
- `entry_arrival_arguments` (2428-2441): any entry parameter -> `"has no installed arrival value"`.
- `host_boundary_touched` is set true for any host call and later makes build-time evaluation fail (2515-2519).

**Same job elsewhere.**
- `run_selected_realization` (368-386) and the tail of `run_dynamic_slot` (463-467) and `selected trait operator` (execution_expressions.rs:542-546) and operator application (execution_scalars.rs:665-669) and the ordinary call tail (execution_expressions.rs:1013-1019) each repeat "save `guard_depth`, set 0, `run_state_collect`, restore". At least five copies (read).
- `read_stdin_byte_value` (480) exists beside the inline host `"read_byte"` handling in `execution_expressions.rs:890-932` (second and third entry points to the same function).
- Default-value construction here (2555-2839) versus value construction in `execution_expressions.rs:1388-1493` (`populate_data_fields` reused by both).
- Cleanup-owner resolution (1256) used by completion cleanup (1456) and the fallback path (1464).

**Unused / one-caller.** `read_stdin_line` has 1 call site (execution.rs:1013) (grepped). `run_entry` has 1 call site (interpreter.rs:1086) (grepped). `new_build` is called from `interpreter.rs:956` and recursively at 2228 (read).

**Stated limits.** The four `unimplemented:` strings above (1237, 1331, 1345, 1448, 1714); "Partition completion guides compiler joins, not runtime hooks." (1703); `"cyclic assembly control requires a checked execution plan"` (2902); `"call target ... is a bodyless requirement or external declaration; the interpreter has no provider for its selected realization"` (2880); `"host boundary call `{target}` not yet supported"` (810); `"machine ... requires implementation completion before execution"` (2872).

**Fail-open risks.**
1. `normal_cleanup_actions` / `execute_completion_cleanup` (1211-1254, 1391-1471): when `self.checked` is `None` (the `Evaluator::new(&typed, ...)`/`Typed` build paths at 2217 and 2237), cleanup plans are never consulted: `normal_cleanup_actions` returns an empty vec (1221-1223), `validate_normal_cleanup_selection` returns `Ok` (1313-1315), and `cleanup_operations` returns `&[]` (1669-1671). Wrong program accepted: an affine cleanup obligation is silently not run when the evaluator is started from `BuildMachineProgram::Typed`. Traced: `Typed` variant exists (interpreter.rs:836), constructed by callers outside this unit (not read). Status: open for reachability; stated: none.
2. `HostOperation::WriteByte` (989-1004): `.and_then(|argument| self.eval_expression(*argument, frame).ok())` discards an evaluation error of the argument and reports it as "write_byte expects one integer argument" (unsupported). That turns a trap or resource halt inside the argument into an `Unsupported`; it still stops. Not fail-open (rejects).
3. `HostOperation::ReadLine` non-bounded path (1013-1025): `if let Ok(cell) = self.resolve_place(...)` ignores a failed place resolution and still returns `Bool(!line.is_empty())`, having consumed stdin. A destination that cannot be resolved is silently skipped. Status: open.
4. `virtual_time_host_value` (756-768): `wall_clock_epoch_offset_seconds` is `0` and wall_clock_raw is `1_767_225_600_000 + ticks`; hard-coded values, not a fail-open.
5. `bind_build_time_argument` (594-693): if `closed_data_field_definition` is `None` or the data's `supply_mode` is not `CheckedShape` the value is left unchanged and `Ok` (619-624). A build-time argument for a non-`CheckedShape` type is accepted without type-symbol binding. Status: open.
6. `exit_process` via `HostOperation::ExitProcess` where `boundary_call_requirement` is `None` (941-953): the non-canonical check is skipped when the pair is absent, and the process exits. The path is reached with an intrinsic target (800-801) whose requirement lookup succeeded, so `None` is unlikely; inferred only.

---------------------------------------------------------------------------------------------------

## 5. interpreter/checked-interpreter/src/execution_expressions.rs (1-1707)

**Purpose.** Evaluates expressions: memoizing scalar evaluation, selected build-time float operators, binary/unary/cast/array/struct/match nodes, call expressions (with the long routing chain and the float/layout/host intrinsics), struct and case literals, and match arm selection (read).

**Shape.** `eval_expression` (20; memoizes Int/Bool/Float/NaN results per outermost expression in `frame.scalar_expression_values`, 25-48), `eval_expression_node` (51-428), `eval_case_membership` (430), `eval_selected_trait_operator` (512), `byte_sequence_operand_bytes` (554). Submodules: `call_evaluation` (580; `eval_call_expression` 605, `try_float_boundary_value_call` 1023, `unary_float_argument` 1285), `layout_target_facts` (1309; `try_layout_target_facts` 1315), `literal_evaluation` (1371; `eval_struct_literal` 1388, `eval_case_literal` 1429, `eval_call_expression_argument` 1495), `match_dispatch` (1505; `with_selected_match` 1515, `select_match_arm` 1530, `eval_selected_match_equality` 1649).

**Inputs/outputs.** Reads expression table, operator facts (`operator_facts.uses`, `selected_trait_candidate_in_machine`, `selected_float_comparison`), selected build-time operators, product float comparisons. Produces `Value`s; can push `private_layout_placements`, `executed_wire_demands`, set `host_boundary_touched`.

**How it decides.**
- Selected build-time operators (91-190): the occurrence is matched by `(expression handle, origin.machine_symbol() == frame.machine_symbol)`. It then re-checks operand handles equal the recorded ones, exactly one row, nonzero provider commitment, and exactly one matching row in the checked operator facts (141-160). Any mismatch traps.
- Call routing order inside `eval_call_expression` (605-1021): source selection; layout target facts; refuse `native_code_layout`; guard memo; static requirement dispatch; dynamic slot; boundary adapter (receiver from member symbol or name path); virtual clock; operator application; byte predicate; build facet routes; private layout placement; spelled `RepresentationSelection | WireCompatibilityRequest` with invalid symbol (726-744); build selection; behavior exclusion; builtins (`AsmSnapshotFlags`, `FloatMaximum/FloatMinimum`); spelled float intrinsics (`eval_named_float_intrinsic`, `float#classify_*`, `float#is_nan` ...; spelled `target` strings 801-877); `CollectionMeasure::Length` on arrays by spelling (879-888); exact console `read_byte`; then static resolution of the callee (896).
- When static resolution fails (899-1002) the code falls back on CALLEE NAME STRINGS: `"tick_count" | "key_state" | "dc_create" | "get_dc" | "window_create" | "is_window" | "window_destroy" | "foreground_window" | "msg_peek" | "msg_translate" | "msg_dispatch"` mark the host boundary touched (912-927) and are served with fake values: `key_state` -> 0, `dc_create/get_dc` -> 1, `msg_*` -> 0, `window_*` use `virtual_live_windows`, `blit` returns its 5th argument (989-1000). Only `if qualified { return Err(halt) }` (908) guards, i.e. only when the receiver is a declaration namespace.
- Match arm selection (1530-1647): arms tested in order; a `Value` pattern with float subject/pattern or a selected float comparison goes through `eval_selected_match_equality` which requires exactly one operator use and one execution row (1649-1705). No matching arm -> `unsupported("checked match has no selected arm; exhaustive dispatch evidence is required")` (1644).
- Case membership (430-510): requires a classifier name path, an exact case owner, a subject type, and compares `runtime_case == case_symbol` by variant symbol found through variant NAME (494-509).
- `eval_call_expression`'s struct/case literals use `selected_construction_definition`/`_case`/`_field_type` from `validation` (1397, 1443, 1406, 1463).

**Cases by shape.** `ExpressionNode::Integer` that does not fit 64 bits -> unsupported (220-225). `Range` outside an index position, `TypeExpression`, `ZeroValue` -> unsupported (417-426). Float intrinsics: only the names in the match at 1128-1277 (`"negate"`, `"square_root"`, `"multiply_then_add"`, `"fused_multiply_add"`, `"minimum"`, `"maximum"`, `"classify"`, `is_*`, directed `*_toward_{zero,positive,negative}` for add/subtract/multiply/divide/square_root/fused_multiply_add); any other name returns `Ok(None)` (1277). Layout target struct must have exactly the four named fields (1342-1353). "`LayoutPlan::place_private` requires one plan and one placement" (707-711).

**Same job elsewhere.**
- Float classification is written three times: inline `float#classify_f32/f64` (801-850), the `"classify"` arm of `try_float_boundary_value_call` (1157-1184), and nothing else in this unit, with the `SemanticFloatClass` -> variant-name mapping duplicated at 816-831 and 1159-1169.
- Float named operations: `try_float_boundary_value_call` (1023) versus `eval_named_float_intrinsic` (execution_scalars.rs:1117) both map named float operations to `FloatSemantics` kernels; the second handles `float#..._f32/_f64` spellings and directed rounding via tables (execution_scalars.rs:1037-1079); the first has an explicit 30-arm match.
- Saturating/trapping policy application is repeated at execution_expressions.rs:1103-1127, execution_scalars.rs:1179-1193 and execution_scalars.rs:1522-1568.
- Memo + guard memo: `eval_expression` scalar memo (25-48) and `guard_call_result` memo (623-627; `Frame::guard_call_result` at interpreter.rs:672 uses `expressions_structurally_equal`).

**Unused / one-caller.** `eval_call_expression_argument` (literal_evaluation, 1495): definition plus 2 call sites (at 972 and 998), grepped count 3 matches, all in this file; it is a one-line wrapper of `eval_argument`. The window/DC/blit stub names (`"key_state" | "dc_create" | "window_create" | "msg_peek"`): 8 matches in this file and 8 in `omega/compilation/backend/calling-conventions/src/host_operations.rs` (grepped); the backend copy is not read.

**Stated limits.** `"late native code-layout metadata is unavailable to interpreted or same-product semantic constant, type, layout, or optimization evaluation"` (619); `"proof-only zero_value<T>() reached runtime evaluation"` (425); `"single-attempt outcome requires its exact structural receiving type"` (215); `"selected floating Match equality has no interpreter execution custody"` (1660).

**Fail-open risks.**
1. The callee-name fallback (912-1000). Input: any unresolved call whose spelled name is `key_state`, `dc_create`, `get_dc`, `window_create`, `is_window`, `window_destroy`, `foreground_window`, `msg_peek`, `msg_translate`, `msg_dispatch`, `tick_count`, `read_byte`, or `blit` (with at least 5 args) and whose receiver is not a declaration namespace. Instead of failing as "no resolved state" it returns a made-up value. Wrong program that gets through: a user machine that calls an undeclared function with one of those names. The guard at 908 only covers qualified calls. Status: open (callers of eval_call_expression and whether resolution can fail for user code not traced; `host_boundary_touched` is set for most of them, which blocks build-time evaluation but not a normal `interpret_entry` run).
2. `eval_expression_node` for `Value::Ref` subjects in `Binary` (302-322) is conditional; none seen as fail-open.
3. `eval_case_membership` etc. are fail-closed (traps). `select_match_arm` is fail-closed. `try_layout_target_facts` returns `Ok(None)` when `layout_target_facts_result` is `None` (1320-1326), which hands the call to ordinary resolution; not a skip of a check.
4. `try_float_boundary_value_call` returns `Ok(None)` on wrong arity (1056-1061) and unknown names (1277); the caller then reports the original resolution halt. Not fail-open.

---------------------------------------------------------------------------------------------------

## 6. interpreter/checked-interpreter/src/execution_scalars.rs (1-1722)

**Purpose.** Scalar semantics: casts, destination-typed expression evaluation ("numeric landing"), argument evaluation order and aliasing, stateless operator-application dispatch, and integer/float binary/unary/comparison operations (read).

**Shape.**
- `casts_and_recasts` (9): `try_dynamic_selector_cast` (17), `cast_target_primitive` (50), `eval_cast` (57), `eval_named_conversion` (109), `eval_interior_recast` (216).
- `numeric_landing` (247): `eval_expression_at_type` (255), `eval_array_literal_at_element_type` (388), `capture_value_operand` (409), `eval_expression_with_destination[_and_carrier]` (431, 441), `anonymous_integer_landing_value` (478), `eval_state_arguments` (514), `check_state_argument_count` (586).
- `operator_applications` (604): `try_operator_application` (652), `selected_realization` (708), `requirement_application` (776), `realization_kind` (820), `compiler_known_realization` (846), `destination_conversion` (889); enums `SelectedRealization` {CheckedBody, Conversion, PrimitiveFloatBinary} (623), `RealizationKind` (645).
- `scalar_operations` (929): `NamedFloatIntrinsic` (950), `eval_unary` (1082), `named_float_operation_name` (1099), `eval_named_float_intrinsic` (1117), `eval_integer_comparison` (1197), `eval_binary` (1249), `eval_int_binary` (1452), `eval_float_binary` (1505), `eval_float_extremum` (1588), `values_equal` (1612), `arithmetic_operand_int` (1685), `enum_variant_tag` (1702).

**Inputs/outputs.** Values are `i64` for every integer width (u64 as bit pattern); floats are `f64` plus an `UndeterminedNaN(format)` marker. Domain `ArithmeticDomain` {Exact, Wrapping, Saturating, Trapping} is carried with the primitive as `scalar_type`.

**How it decides.**
- `eval_cast` (57-107): exact authored float/integer coercions trap when the numeric denotation changes (63-74, 87-105).
- `eval_named_conversion` (109-214): Bool target = `value.as_bool().unwrap_or(false)` (143). Integer targets: float -> integer by domain (Saturating/Trapping/Exact; Wrapping traps, 170-175); integer -> integer: Wrapping uses `wrap_to_width`, Saturating clamps, Exact/Trapping trap when out of range (183-211).
- `eval_binary` (1249-1450): logical ops; `Equal/NotEqual` via `values_equal`; Str + Str concatenation; floats route to `eval_float_binary`; signed Divide/Modulo with non-Exact domain use i128 (1299-1335); Wrapping Add/Sub/Mul/Shift (1336-1358); Saturating/Trapping Add/Sub/Mul/Shift (1359-1448); everything else `eval_int_binary` (wrapping 64-bit arithmetic, 1452-1503). Unsigned handling keyed on `unsigned_operands` bool.
- Operator application (708-774): requires `operator_facts`, a requirement symbol that is a boundary `Operator` or a bodyless top-level-requirement machine with no type/lifetime params; exactly one checked selection for the application; then counts realization contracts: exactly one checked-body machine and nothing else -> run it; no checked bodies and at least one compiler-intrinsic realization -> `compiler_known_realization`; otherwise `Ok(None)` (765-773).
- `compiler_known_realization` (846-887): either a primitive float binary operation (domain from the adapter, format must match) or a "destination conversion" (889-917): one non-const non-mutable parameter, source != destination, both numeric, at least one float.
- Argument order: `argument_evaluation_order` from the call site or default (522-530); runtime-descriptor aliases are recorded as `match_subjects` captures (532-561).

**Cases by shape (quoted).** `values_equal` (1612-1683) handles: Enum/Enum by variant name only (1615-1621), Int/Enum by tag, Str/Str, Array/Str and Array/Array as byte sequences, Bool/Bool, then floats, then `as_int` pairs; anything else -> `unsupported("equality operands have no supported comparison")` (1677). Float `Modulo | ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor` -> `unsupported("float modulo/shift/bitwise not supported")` (1578-1580). `eval_integer_comparison` supports only I8..I64, U8..U64, Addr (1207-1217). `NamedFloatIntrinsic::from_target` parses `float#<stem>_f32|_f64` with stems `multiply_then_add`, `fused_multiply_add`, and `<op>_toward_{zero,positive,negative}` for `fused_multiply_add | sqrt | add | subtract | multiply | divide` (958-992).

**Same job elsewhere.**
- Wrap/clamp/trap arithmetic by domain is implemented in three places in this file/unit: `apply_arithmetic_domain` (interpreter.rs:349-367, takes `i64`), the i128 paths in `eval_binary` (execution_scalars.rs:1299-1448), and the conversion paths in `eval_named_conversion` (183-211). I did not find a call to `apply_arithmetic_domain` in this unit besides its re-export (interpreter.rs:476); call count not grepped.
- Enum equality by variant name appears at 1615-1621 and `enum_variant_tag` ordinal (1702-1720).
- Case comparisons also exist in `execution_expressions.rs:430-510` (exact owner + case symbol) versus equality by name here.

**Unused / one-caller.** Not individually grepped. `eval_unary` handles only `BitwiseNot`/`LogicalNot` (1087-1097).

**Stated limits.** `"Wrapping float-to-integer conversion has no semantic reading"` (172); `"float modulo/shift/bitwise not supported"` (1579).

**Fail-open risks.**
1. `values_equal` enum/enum by variant name only (1615-1621): two enums of different TYPE with the same variant name compare equal. Input: `==` on two `Value::Enum` of different data types that share a case name. In checked programs the type checker presumably rejects that comparison before execution (inferred, not traced). Status: open.
2. `eval_named_conversion` Bool target: `value.as_bool().unwrap_or(false)` (143) turns a non-boolean value into `false` instead of trapping. Input: a cast to `Bool` of an Int/Float. Whether the checker permits such a cast: not traced. Status: open.
3. `selected_realization` (708-774): falls to `Ok(None)` for "no contracts", "mixed" and "multiple checked bodies" cases (773), letting the caller continue to ordinary resolution. When `application` is invalid (statement/transition callers pass `ExpressionHandle::invalid()`, execution.rs:326), the checked-selection cross-check at 720-743 is skipped and `adapter` stays `None` (719). Wrong program: a statement-position operator call is run without checking that the facts selected this requirement. Status: open.
4. `capture_value_operand` (409-429) and the rest fail closed.

---------------------------------------------------------------------------------------------------

## 7. interpreter/checked-interpreter/src/filesystem.rs (1-3916)

**Purpose.** Filesystem plumbing for the interpreter: access mode enum, metadata observation record, operation-attempt record, per-call preparation (typed operand cursor), logical-handle tracking, the shared dispatch macro, and the whole in-memory (virtual) filesystem used when no real provider is selected (read).

**Shape.**
- `filesystem_access` (10): `FilesystemAccess` {Virtual (default), RealUnscoped, RealScoped(FsGrants), RealScopedSponsored, CapturedScoped} (22-36), `CapturedFilesystemRoot` (16).
- `filesystem_observations` (39): `FilesystemMetadataObservation` (52) with fixed fake values in `new` (72-97: device 16_777_220, inode 1_000_000, uid 501, gid 20, times 1_000_000_xxx, blocks 8, block size 4096); `FILESYSTEM_METADATA_API_CARRIER_BYTES = 144` (49).
- `filesystem_operation_attempts` (180): `FilesystemOperationAttempt` (207) with provider, outcome, rooted path resolutions, authorized paths, handle inputs/output, retired handles, grant refusals.
- `evaluator_filesystem` (291): `pop_virtual_find` (298), `close_virtual_find` (318); `filesystem_calls` (327): macro `prepared_filesystem_call_dispatch!` (338), `evaluate_filesystem_operation` (365), `serve_filesystem_call` (494); `descriptor_calls` (563), `directory_calls` (1038), `path_calls` (1140) = `serve_*` methods for the virtual provider; `logical_handle_store` (1564) `FilesystemLogicalHandles`; `logical_handles` (1761); `observations` (2041) `canonical_metadata_carrier` (2073); `preparation` (2131) with `PreparedFilesystemCall` enum (2364-2559, about 50 variants), `FilesystemArgumentCursor` (2812), `prepare_filesystem_call` (3126); `prepared_outputs` (3451); `virtual_filesystem` (3556) `virtual_open*`, `virtual_write`, `virtual_seek` etc.

**Inputs/outputs.** `evaluate_filesystem_operation` (365-492) pushes a `FilesystemOperationAttempt` (provider chosen as Virtual / RealScoped / RealUnscoped / Captured), calls `prepare`, validates, serves, then records outcome. Virtual state lives in `Evaluator.virtual_*` maps keyed by path bytes and fd.

**How it decides.**
- Provider: `real_fs` None -> Virtual; `is_scoped()` -> RealScoped; else RealUnscoped (372-378); if the call touches a captured path/descriptor -> Captured (388-394).
- Success semantics of handle results are table-driven per call variant (`logical_handle_plan`, 2646-2809): e.g. descriptor-returning calls succeed when `NonNegative`, find handles when `NotMinusOne`, `FindNext` when `NonZero`. Results outside the success predicate leave handle state unchanged.
- Integer results must fit `i32` for `I32` result kinds (430-440).
- Cross-domain aliasing (a raw token live in another handle domain) -> trap (1822-1838).
- Rooted paths: `path()` (3009-3065) takes a `$OmegaBuildRootedPath` struct (via `rooted_build_path_parts`) and asks the real provider to resolve it to a native or captured path; a plain string/byte path is accepted as `Native` unless the guard flag is set (see fail-open #1).
- Virtual semantics (3556-3914): errno numbers are literal (2 ENOENT, 9 EBADF, 13 EACCES, 17 EEXIST, 21 EISDIR, 35 EWOULDBLOCK, 1 EPERM; Windows-style 6, 33, 158, 183, 5 in handle calls). Open flags via `host_open_flags` (`o_creat`, `o_excl`, `o_trunc`, `o_append`); writable = `flags & 0x3 != 0` (3816). Permission check uses `virtual_perms` bits 0o200/0o400 (3821-3829). Directory listing is the immediate children by `/`-prefix (3565-3572). `chown` succeeds only when requested owner equals `(501, 20)` after `-1` defaults (3905-3913).

**Cases by shape (fixed lists).** `PreparedFilesystemCall` is a closed list of about 50 operations (2364-2559); each `FilesystemHostOperation` arm in `prepare_filesystem_call` (3147-3443) fixes its operand shape (e.g. `OpenPathHandle` consumes `path, u32, u32, i64, u32, u32, nullable-native-handle`, 3302-3313). `check_filesystem_arity` (2199-2210). `checked_relative_component` allows exactly one nonempty component without `/`, `\`, NUL, `.`, `..` (2180-2193). Package-build refusals: `Canonicalize` ("would expose a host-absolute path") and `FindFirst|FindNext|FindClose` ("unrooted find-cursor protocol not admitted by the Build facet") (2212-2224). `serve_rename` moves only regular files (1341-1347); `serve_remove` only files; `virtual_symlinks` resolved one hop at open (3808) and in `serve_canonicalize` (1452).

**Same job elsewhere.**
- Real provider counterpart: `filesystem_real.rs` has a `real_*` function for the same operation names (dispatch tables at filesystem.rs:515-557 and filesystem_real.rs:343-383). Both use the one macro `prepared_filesystem_call_dispatch!` (2 uses total, one in each file; grepped).
- Directory snapshot building: `build_dirent_records`/`build_find_entries` (3592-3659) mirror `real_directory_entries` (filesystem_real.rs:2073-2134); both enforce the same snapshot-size helpers.
- File-time conversion `filetime / 10_000_000 - 11_644_473_600` appears at filesystem.rs:980 and filesystem_real.rs:1600.
- `FindFirst` queue handling reuses the virtual find tables even for the real provider (`filesystem_real.rs:1899-1902`).
- `virtual_hard_link_file` duplicates file content (copy) rather than sharing it; `serve_rename` ignores existing destination and directories (read).
- The captured-input snapshot store (filesystem_real.rs:766-1190) is a third provider.

**Unused / one-caller.** `virtual_open` (3680) has 1 call site (filesystem.rs:505; grepped). `rooted_build_paths_required`: grepped in all of `omega-rust`: 5 matches: the field (`interpreter.rs:587`), three reads (`filesystem.rs:1845, 3012, 3133`), and ONE assignment, `rooted_build_paths_required: false` (`execution.rs:2285`). No code outside this crate or elsewhere in it sets it true. So the three branches that depend on it are never taken in this tree.

**Stated limits.** Trap texts: `"package build filesystem paths must come from BuildSource::resolve or BuildOutput::resolve"` (3018); `"package build filesystem operation `final_path_name_by_handle` would expose a host-absolute path"` (1856); `"rooted build path names no compiler-supplied grant root"` (3053); `"captured input path cannot enter a native provider"` (2348). Comment on macro: none.

**Fail-open risks.**
1. `rooted_build_paths_required` is never set true (grepped, see above). Wrong program that gets through: a package build calling `BuildSource::open` with a plain string path, or `Canonicalize`, `FindFirst/Next/Close`, or `FinalPathNameByHandle` on a non-unknown handle. The three checks at 1845, 3012, 3133 are skipped. Real-file scoping still applies via grants (`authorized_native_path`, filesystem_real.rs:541-595) when `real_fs` is scoped. Status: traced for the flag (no setter in `omega-rust`); not traced beyond that.
2. `FilesystemMetadataObservation::new` and virtual provider return fixed fake metadata (72-97). Not fail-open.
3. `serve_set_file_times` and `serve_set_file_time` default missing/short bytes to 0 (`unwrap_or(0)` at 867, 979). Wrong program: not a soundness issue; the preparation step already checks size >= 8/32 (3340, 3406).
4. `validate_incremental_logical_handle_inputs`/`complete_logical_handle_observations` are fail-closed (trap). `serve_open_path_handle` maps errno by a small table with default 2 (1159-1164).
5. `virtual_open_flags` (3807-3868): an unwritable existing perm entry blocks; but `virtual_perms` is only consulted via `get(&path)` after symlink resolution; none seen as fail-open.

---------------------------------------------------------------------------------------------------

## 8. interpreter/checked-interpreter/src/filesystem_real.rs (1-3224)

**Purpose.** Serves filesystem calls against the real host (scoped by grants or unscoped), plus an immutable "captured source snapshot" provider, descriptor write grants, and sponsor (staging) accounting (read).

**Shape.**
- Top: `RealFd` (37), `RealFs` (46: files, next_fd, captured_roots, captured_descriptors, captured_descriptions, errno, grants, sponsor), `RealFs::new` (58), `with_captured_inputs` (75), `descriptor_root` (106), `prepare_rooted_path` (120), `require_sealed_rooted_file` (139), `call_uses_captured_input` (174), `is_scoped` (187), `insert` (201), `canonical_metadata_for_path` (245); `Drop for RealFs` (271) closes sponsor descriptors; `try_real_filesystem_call` (293) dispatch (macro with 4 literal arms + 36 dispatch arms); `real_fs` accessor (433).
- `authorized_paths` (437): `resolve_for_check` (457; follows up to `LINK_LIMIT = 41` links manually), `canonical_relative_path` (479), `authorized_path*` (495-540), `authorized_native_path` (541), `record_authorized_path` (597), `record_grant_refusal` (639); unix-only `confinement_tests` (663-763, three `#[test]`s).
- `captured` (766): `CapturedDescriptor`, `CapturedOpenDescription` (783, refcounted cursor), `RealFs::{open,close,duplicate,seek}_captured`, `serve_captured_filesystem_call` (957), `read_captured` (1148).
- `descriptor_calls` (1192), `directory_calls` (1827), `grants` (1925: `Grants`, `matching_root` 1953, `canonical_grants` 1978), `host_io` (2028), `metadata` (2299), `path_calls` (2456), `sponsor` (2843).

**Inputs/outputs.** Reads host filesystem via `std::fs`, `FsGrants` (read_roots, write_roots, canonical metadata), optional `FilesystemSponsor` (staging custody), captured `SourceSnapshot`s. Writes errno in `RealFs.errno` and returns `i64` results.

**How it decides.**
- Authorization (541-595): with grants, the path is resolved (`resolve_for_check` following the real symlink chain, or parent-only for no-follow), then `grants.matching_root(&resolved, write)` picks the deepest root (most components) among write roots plus (for reads) read roots (1953-1968). Match is `resolved.starts_with(&root.path)` (path-prefix on canonical paths). No grants = unscoped = everything allowed (548-551).
- Descriptor writes require `write_granted` computed at open time (226-229) and re-checked by `require_real_descriptor_write_grant` (2905-2921).
- Canonical metadata: if the matching root carries an index, opening a path returns `EACCES` unless the index has a row for the relative path and the host file kind equals the row kind (245-264, 209-214).
- Captured provider (957-1146): open flags must be exactly 0 (others EACCES/45); every mutating call -> `EACCES` plus a grant refusal; native-only calls (metadata, handles, locks, sync, Canonicalize, find) -> `Halt::Unsupported` (1099-1143).
- Sponsor (2843-3223): reads outside session root bypass the sponsor only when no write/create/truncate (2878-2889). Writes through the sponsor must be creates (`prepare_create_object_open`); write-open of an existing file, duplicate, truncate, unlink, rename, hard link, symlink in staging are refused with `UnsupportedStagingOperation` (2949-2953, 3027, 3048, 3128, 3142, 3158, 3171).

**Cases by shape (quoted).** `canonical_relative_path` (479-492) accepts only `Normal` components and valid UTF-8. `open_captured`: `flags & 3 != 0 || creat || trunc || append` -> EACCES, then `flags != 0` -> 45 (835-844). `ReadDir` captured: names over `MAX_DIRECTORY_ENTRY_NAME_BYTES` -> `Halt::Unsupported("captured directory name exceeds native listing carrier; use source snapshot list")` (1063-1068). `Grants::canonical_grants`: write roots cannot carry canonical metadata, identities must be unique, two roots cannot have the same canonical path (1984-2004).

**Same job elsewhere.**
- Same operations as the virtual provider in `filesystem.rs` (see its section).
- `host_io::win32_error_code` (2287) vs `io_errno` (24): two errno conventions chosen per call by `win32_errors` booleans.
- `canonical_relative_path` at filesystem_real.rs:479 and `omega/packages/build-output/src/portable_paths/mod.rs:14` (grepped `fn canonical_relative_path`; the second site is not read; "suspected" duplicate).
- `validate_build_relative_path` (build_machine.rs:1177) vs `checked_relative_component` (filesystem.rs:2180) vs `canonical_relative_path` (479): three relative-path validators with different rules (the first allows nested `/` components, the second a single component, the third host `Component::Normal`).

**Unused / one-caller.** Not individually grepped. `real_fs` (433) is used in build_machine.rs (985, 1077) and filesystem.rs.

**Stated limits.** `"captured inputs expose only logical source operations, not native metadata, host paths, handles, locks or synchronization"` (1142); `UnsupportedStagingOperation` strings (see above). Comment `use source snapshot list` (1067).

**Fail-open risks.**
1. `authorized_native_path` (548-551): `real_fs(self).and_then(|fs| fs.grants.as_ref())` returning `None` returns the path as allowed. This happens for `RealUnscoped`. By design: that mode is the unscoped provider; no comment at the site (the type name `RealUnscoped` states it). Also reachable if `real_fs` is itself `None`, but this function is only called from real-provider paths (`real_fs_mut` panics via `expect` otherwise, 405-409). Status: traced for the mode.
2. `resolve_for_check` (457-477) for no-follow checks uses `resolve_parent_for_check` (446-455): only the PARENT is canonicalized and the final name is appended without resolving it, so a final component that is itself a symlink is authorized by its own location, not its target. That is the intent of no-follow ("namespace leaf"); used by Remove/RemoveDir/Rename/HardLink/Symlink/CreateDir/ReadLink/ReadSymlinkMetadata. Stated: not by a comment in the code; the function names `authorized_namespace_leaf`/`_no_follow` state it.
3. `descriptor_root` (106-118): for a real fd uses `grants.matching_root(&entry.path, false)` (read-or-write match). Used at build_machine.rs:985-991 to compare against the facet root; `None` (unscoped, or fd missing) skips the check (`if let Some(issued) = ... && issued != root`). Wrong program: in an unscoped real run, a Build descriptor operation naming a different facet's root is not rejected. Status: open.
4. `real_seek` (1268-1290): any `whence` other than 1 or 2 is treated as `SeekFrom::Start` with negative offsets clamped to 0 (`offset.max(0)`, 1276). The virtual `virtual_seek` returns error for `whence` outside 0..2 (3880-3885). Real accepts invalid whence silently. Status: open.
5. `real_get_osf_handle` (1519) and `get_osf_handle` virtual (910): `-2` for unknown fd; fail-closed.
6. `captured` `ReadDir` fail-closed. `finish_real_open` maps insert errors to errno; fail-closed.
7. `prepare_sponsored_write` returns `Unsponsored` when the descriptor has no sponsor descriptor (3064-3066), i.e. a descriptor opened by the read-only bypass (2935-2943) is never accounted. Reads are allowed to bypass by design (2878); for writes the descriptor would be refused earlier unless `write_granted`. Status: traced partly.

---------------------------------------------------------------------------------------------------

## 1. foundation/symbols/src/tests.rs (1-1846)

**Purpose.** Unit tests (and three fixture builders) for the `symbols` crate's `SymbolTable`/`SymbolTableBuilder`: generated extensions, child ordering, authored extensions, module namespaces and imports, package scope separation, symbol provenance and package identity, build-subject lookup, source-scoped top-level bindings (read). Test-only.

**Shape.** Inline modules: `generated_extension` (6), `hierarchy` (120), `module_namespaces` (509), `package_scope_separation` (1215), `paths` (1307), `provenance` (1357), `source_scoped_bindings` (1707). Shared fixtures at crate level: `main_entry_symbols` (1781), `sourced_symbol_table<const N>` (1798), `main_console_symbols` (1825). Local helpers inside `module_namespaces`: `namespace_table` (521), `reference` (562), `register_module` (566), `data_reference` (575), `attached_table` (900), `domain_reference` (965), `generic_domain_table` (1021). One `#[should_panic(expected = "existing derivation origins")]` test (1408).

**Inputs/outputs.** Builds symbol tables from `SourceMap`s (`add`, `add_with_metadata`, `add_with_metadata_and_resolution_stratum`, `add_checked_instance`) and asserts on lookup results.

**How it decides (what the tests pin).** Lookup precedence keyed on SourceId, package identity (`PackageKeyIdentity::from_digest([n;32])`), `SourceResolutionStratum` {Base, CurrentActivationExtension, DependencyActivationExtension}, `DependencyScope` {Build, Product}, and imported-module spellings; ties are decided by exact qualified-path spelling (e.g. `"units::u64::Distance"` 1014-1018, `"dependency::main::Item"` 612-616). Specific pinned rules include: generated extension must retain exact rows (29-34); appended authored children invalidate extension status (37-45); current-activation extension units are invisible to base units (421) and preferred over base for themselves (428-436); an exact-source import of a package root exposes that package's dependency-generated stratum in the importer's checked instance only (703-774); same-spelled contested domains stay "not found" without a qualifier (978-1019); a toolchain declaration is reachable by `find_build_subject_declaration_from_source` but not by `find_product_declaration_from_source` (1630-1670); token-family selection needs explicit membership (1673-1705).

**Cases by shape.** Fixtures with fixed sizes: `namespace_table` makes exactly three declarations (544); `attached_table` uses three names (900-928).

**Same job elsewhere.** None in unit.

**Unused / one-caller.** Test code; not applicable.

**Stated limits.** Test comments only: "source-free focused consumers retain their permissive lookup behavior" (452), "source-free spelling cannot acquire the new authored local precedence" (950), "source-free generated names must not inherit an authored source binding" (1769).

**Fail-open risks.** The tests PIN permissive behavior for source-free lookups: `SourceSpan::default()` references see every symbol (454: `source_reference_can_see_symbol(SourceSpan::default(), declarations[1])` is asserted true; 444-453 lookup by default span returns the extension-only declaration). Stated (452: "source-free focused consumers retain their permissive lookup behavior"). Which production callers pass a default span was not traced.

---------------------------------------------------------------------------------------------------

## Coverage

Read in full (every line of every range):
- `omega-rust/psi/foundation/symbols/src/tests.rs` 1-1846 (1846 lines)
- `omega-rust/psi/interpreter/checked-interpreter/src/build_machine.rs` 1-1924 (1924)
- `.../evaluation.rs` 1-899 (899)
- `.../execution.rs` 1-3087 (3087)
- `.../execution_expressions.rs` 1-1707 (1707)
- `.../execution_scalars.rs` 1-1722 (1722)
- `.../filesystem.rs` 1-3916 (3916)
- `.../filesystem_real.rs` 1-3224 (3224)
- `.../interpreter.rs` 1-1119 (1119)
- `.../lib.rs` 1-88 (88)

Total read in full: 19532 lines. Skimmed: 0. Not read: 0.

Greps run (all over `omega-rust` unless noted): `eval_call_expression_argument` (3 matches, 1 file); `read_stdin_line|fn run_entry|.run_entry(|virtual_open(` (6 matching lines, content mode); `exact_build_facet_method` (8, 1 file); `prepared_filesystem_call_dispatch!` (2, in filesystem.rs and filesystem_real.rs); `"key_state"|"dc_create"|"window_create"|"msg_peek"` (8 in execution_expressions.rs, 8 in backend `calling-conventions/src/host_operations.rs`); `fn (wrap_to_width|integer_bounds|primitive_bit_width|validate_build_relative_path|canonical_relative_path|checked_relative_component)` (content); `.unwrap_or(false)|unwrap_or(Value::Unit)` in the crate src (7); combined pattern for six public functions (16 lines, 8 files); `rooted_build_paths_required` (5 lines, content).

## Five things a maintainer most needs to know about this unit

1. `rooted_build_paths_required` is declared, read in three guards (filesystem.rs:1845, 3012, 3133) and assigned only `false` (execution.rs:2285); a grep over all of `omega-rust` finds no other assignment. The "package build paths must be rooted" refusals are therefore never active in this tree.
2. Build-prelude recognition is by spelling plus provenance, not by identity: facet methods are found as `(attached_data string, state name string)` pairs, marker values by `$OmegaBuild...` type-name strings, and enum cases by data-definition name (`CrashCause`, `PhysicalAuthorityClass`, `CompositionMode`, `OutputCompletion`) (build_machine.rs:852, 1101, 312, 1608). Only `root_build` uses cell identity (`Cell::ptr_eq`).
3. `eval_call_expression` has a name-string fallback with fabricated host values for unresolved calls named `key_state`, `dc_create`, `get_dc`, `window_create`, `is_window`, `window_destroy`, `foreground_window`, `msg_*`, `tick_count`, `read_byte`, `blit` (execution_expressions.rs:912-1000); only calls whose receiver is a declaration namespace skip it (908).
4. Cleanup obligations are enforced only when the evaluator holds `checked` custody; with `BuildMachineProgram::Typed` (`Evaluator::new(typed, &[])`, execution.rs:2217) the cleanup lookups return empty/`Ok` (execution.rs:1221-1223, 1313-1315, 1669-1671). Selected-owner and expression-owned cleanup are `unimplemented:` traps.
5. Non-success outcomes of `interpret_entry` (trap, unsupported, resource, thread panic) are `InterpretOutcome::error` with `exit_code: 0` (evaluation.rs:323-337, interpreter.rs:772-779, 1102-1106); callers must look at `error`. Also, one `Evaluator` struct of about 60 fields (interpreter.rs:537-615) carries three filesystem providers (virtual maps, `real_fs` real/captured), and the virtual and real providers each implement ~45 operations in parallel (filesystem.rs vs filesystem_real.rs) via one dispatch macro.

## What I am unsure about

- Whether callers of `interpret_entry` check `InterpretOutcome.error` (not traced; callers are outside this unit).
- Whether `BuildMachineProgram::Typed` is constructed by any production caller (the variant exists, interpreter.rs:836; construction sites not read), which decides how real fail-open item 1 for cleanup is.
- Whether `try_component_assumption_acceptance` (build_machine.rs:15-35) records the digest elsewhere; in this file it is evaluated and dropped.
- Whether type checking prevents the enum-by-name equality (execution_scalars.rs:1615-1621) and Bool-cast `unwrap_or(false)` (143) paths from being reached with mismatched types.
- Per-name call counts for most `pub(crate)` helpers were not grepped (only those listed under Coverage), so "one-caller" claims are limited to the greps named.
- The tests.rs file was read for what it asserts; I did not read the `symbols` crate implementation it exercises, so claims about "how it decides" there come from the assertions.
