slice_0059

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0449 | effect_inference.rs:1285 | CLOSED | Exact embed calls are proof-only; skipping them in operational call collection is intentional and does not skip program-validation checks. |
| A0450 | effect_inference.rs:72 | CLOSED | Assembly statements are validated on other paths; empty `visit_statement` arms only omit them from operational call-site harvesting. |
| A0451 | effect_inference.rs:2972-2982 | CLOSED | Failed asm service resolve yields empty direct reach, but `validate_asm_intrinsic_declarations` errors on statement/assignment asm before `finish_diagnostics`. |
| A0452 | effect_inference.rs:3186 | CLOSED | Empty `DirectServiceReach` at fall-through is direct-only; transitive service reach is filled via `call_targets` in the fixed-point (ei:2555-2567). |
| A0453 | lib.rs:172-174 | CLOSED | Invalid expression handles are not expected on typed validation paths; early `return false` is a guard, and callers still run other mismatch logic. |
| A0454 | lib.rs:773 | CLOSED | `ProgramEntryVisibleParameters` has only two variants; `_ => {}` covers lawful success cases, not missing checks for other variants. |
| A0455 | lib.rs:940-951 | CLOSED | Excluding access-view parameters from visible arrival count matches ProgramEntry host contract semantics, not silent acceptance of bad entries. |
| A0456 | lib.rs:421-444 | CLOSED | `continue` filters incomplete runtime rank components out of published compiler-eval dependencies; it does not make `validate()` accept a program. |

### A0449 (effect_inference.rs:1285)

1. Read `OperationalCallSites::expression_call` in full (ei:1279-1314, read) and `is_exact_embed_call` (proof_contracts_core.rs:6433-6443, read).
2. Search `\bis_exact_embed_call\b` in `omega-rust`: 16 hits in 8 files (count mode).
3. `expression_call` is invoked only via `visit_expression` → `visit_state_calls` (ei:198-200, 1223; search `visit_state_calls` in omega-rust: 5 hits, all in effect_inference.rs).
4. Upward: `collect_state_calls` (ei:1210-1240) → `build_machine_work` (ei:1177) → `infer_operational_may_for_dependencies` (ei:1138-1151) → used in `declaration_prologue` / `validate_behavior_plan` (program_validation.rs:230, 4896).
5. Input to accepting branch: a `TableCallExpression` that satisfies `is_exact_embed_call` (IntegerEmbed builtin, no receiver).
6. Verdict CLOSED: `validate_behavior_plan` and other validators still run; embed calls are excluded only from operational/effect call lists, consistent with proof-only classification elsewhere (e.g. expression_types_and_fields.rs:4930-4934).
7. Unsure: none for accept/reject boundary.

### A0450 (effect_inference.rs:72)

1. Read `visit_statement` in full (ei:63-128, read).
2. Search `visit_statement(` in omega-rust: 2 hits (definition + recursive call in effect_inference.rs); production entry is `visit_state_calls` (ei:52-59).
3. Upward: same operational-plan chain as A0449; parallel `InvocationCallSites` also uses `visit_state_calls` (ei:581).
4. Input: `StatementNode::Assembly(_)` or `StatementNode::AssemblyFact(_)`.
5. Verdict CLOSED: assembly is handled in program validation (`program_validation.rs:3792-3794` — Assembly no-op in statement validate hook; `AssemblyFact` validated) and in `validate_asm_intrinsic_declarations` / `validate_asm_discharge` (`machine_calls_core.rs:5626-5677`, `5394-5403`).
6. Unsure: did not read every assembly validator module in full.

### A0451 (effect_inference.rs:2972-2982)

1. Read `direct_service_reach_for_operational_call` in full (ei:2960-2994, read) and `AssemblyServices::resolve` (machine_calls_core.rs:89-109, read).
2. Search `direct_service_reach_for_operational_call(` in omega-rust: 2 hits (definition + call at ei:2769).
3. Upward: `machine_reach_work` (ei:2769-2775) → `infer_service_reaches_uncached` → `validate_behavior_plan` (`machine_calls_core.rs:5399-5403`, `program_validation.rs:4896-4898`) → `finish_diagnostics` (program_validation.rs:4903).
4. Input: operational call whose `target_state_symbol` is an asm intrinsic builtin and `assembly_services.resolve` returns `Err`.
5. Verdict CLOSED: for statement-level asm, `validate_asm_intrinsic_declarations` pushes an error when resolve fails (`machine_calls_core.rs:5649-5658`, read); validation does not return Ok while those diagnostics remain (`finish_diagnostics` at pv:4903). Under-reported reach in inference is therefore harmless for acceptance.
6. Unsure: did not fully trace expression-statement nested asm through `expression_statements::validate` (calls.rs not read in full for that path).

### A0452 (effect_inference.rs:3186)

1. Read `direct_service_reach_for_call` in full (ei:3121-3187, read).
2. Search `direct_service_reach_for_call(` in omega-rust: 5 hits (all effect_inference.rs).
3. Upward: `direct_service_reach_for_operational_call` (ei:2984) → service-reach fixed point (ei:2549-2567 extends transitive reach via `call_targets(call_work)`).
4. Input: valid call target that is not a parameter signature, trait signature, or boundary-machine state (per `CallTargetIndex` build at declaration_tables_and_index.rs:1918-1928, read for `boundary_machine_by_state` scope).
5. Verdict CLOSED: line 3186 returns empty *direct* reach; callee machine reach is merged transitively (ei:2555-2567, read), so checked-body machine-to-machine calls are not accepted with missing inferred services.
6. Unsure: none for acceptance boundary.

### A0453 (lib.rs:172-174)

1. Read `pattern_components::report_delivery` in full (lib.rs:164-188, read).
2. Search `report_delivery(` in omega-rust: 3 hits (lib.rs definition, expression_types_and_fields.rs:3370, literals_and_struct_literals.rs:4602).
3. Read caller `report_delivered_type_mismatch_with_component` (expression_types_and_fields.rs:3361-3387, read): on `false`, continues to `delivered_type_conflict`.
4. Upward: used during per-statement validation in the validation crate, after typed trees are built (`program_validation.rs:4067+, read header only).
5. Input: `value` with `!value.is_valid()`.
6. Verdict CLOSED: invalid handles are defensive; validation runs on typed programs where expressions are ordinarily valid. No diagnostic is intentional silence only for the delivery-component check, not overall acceptance.
7. Unsure: did not enumerate every callsite that could pass invalid handles after partial typing failure.

### A0454 (lib.rs:773)

1. Read `validate_source_compatibility` match block (lib.rs:754-774, read) and enum (semantic-vocabulary/src/lib.rs:274-277, read).
2. Search `validate_source_compatibility(` in repo: 3 hits (lib.rs, frozen_program_and_queries.rs:3682, provider-planning/program_entry.rs:175).
3. Upward: product entry selection (`frozen_program_and_queries.rs:3680-3707`, read) rejects incompatible/ambiguous machines; `validate_source_compatibility` returns `Err(diagnostics)` when non-empty (lib.rs:811-814, read).
4. Input: `slot.visible_parameters` combined with `visible` count from `visible_entry_parameters`.
5. Verdict CLOSED: enum is exhaustive with two variants; `_ => {}` is the lawful success arm when `None` has no visible params or `ImageAndInitialStorage` has exactly two visible params (errors already handled in prior match arms).
6. Unsure: none.

### A0455 (lib.rs:940-951)

1. Read `visible_entry_parameters` in full (lib.rs:940-952, read).
2. Search `visible_entry_parameters(` in omega-rust: 2 hits (lib.rs:753, provider-planning/program_entry.rs:181).
3. Upward: `validate_source_compatibility` (lib.rs:753-772) enforces counts against slot schema; failures become diagnostics (lib.rs:811-814).
4. Input: entry `StateParameter` rows whose type has an access-view plan (`access_view_plan_for_input_type_reference` returns `Some`).
5. Verdict CLOSED: filtering access-view parameters is deliberate host-boundary semantics, not dropping errors for illegal extra parameters (non-view params still count toward visibility).
6. Unsure: did not read `access_view_plan_for_input_type_reference` body (typed-trees declaration_tables_and_index.rs:6052, not opened).

### A0456 (lib.rs:421-444)

1. Read `checked_dependencies` loop in full (lib.rs:414-508, read), especially `continue` guard (lib.rs:423-444, read).
2. Search `checked_dependencies(` in repo: 2 hits (lib.rs definition, published_facts.rs:4580).
3. Upward: called from `retain_declaration_descriptions` after checking (`published_facts.rs:4577-4581`, read) — publication of checked facts, not the main `validate()` accept gate (program_validation.rs:4067, read signature only).
4. Input: runtime rank component with empty bodies, non-empty `component.diagnostic`, or bodies failing termination/state alignment checks.
5. Verdict CLOSED: `continue` omits incomplete components from `CheckedCompilerEvaluationDependencies`; it does not return Ok from program validation. Skipped components already carry diagnostics or incomplete judgments in `rank_arrivals`.
6. Unsure: did not read full termination admission pipeline in call_cycles.rs beyond component insert (call_cycles.rs:820-827, read partially).

### What I did not read

- Full `expression_statements::validate` and nested asm operand validation in calls.rs (relevant only to residual uncertainty noted in A0451).
- Full `access_view_plan_for_input_type_reference` implementation in typed-trees.
- Entire `call_cycles` rank admission and whether every `component.diagnostic` always pairs with validation failure.
- All `CallSiteVisitor` implementors beyond `OperationalCallSites` and `InvocationCallSites` (only two production implementors found in effect_inference.rs).

SLICE COMPLETE
