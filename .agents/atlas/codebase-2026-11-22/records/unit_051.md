# Record: unit_051 (areas 06-source-driver-build and 02-source-stages-00-04)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/`, `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. All paths below are relative to `source-rust/subsystems/compiler/source-to-psi/` unless they start with another root. `CTI` = `driver/compile_time_invocations`. "read" means I read the lines; "grepped" and "inferred" are marked.

## CTI/calls/arguments.rs (527 lines, read in full)

1. **Purpose.** Turns each runtime argument of a compile-time call (a `BuildTimeValue`) into a typed source expression inside a generated caller machine. Host-backed views and issued opaque values become generated parameters of the caller; everything else becomes literal expressions.
2. **Shape.** `MaterializedArguments {expressions, input_values}` :17; `materialize` :22 (entry, loops over parameters); `selects_member` :111; `DataArgument` :120; `data_argument` :130; `field_argument` :211; `validate_input_declarations` :230; `materialize_input` :289 (adds `@input-N` parameter); `materialize_argument` :322 (adds `@context-N` parameter for `LayoutTarget`, array/struct/primitive literals); `name_path` :513.
3. **In/out.** Reads `TypedTrees` type-reference table, symbols, data definitions; mutates the probe `TypedTrees` and the caller `State` (new statements, parameters, expressions). Returns expression handles plus the `BuildTimeValue`s that must be supplied at execution as inputs.
4. **How it decides.** Keyed on the shape of the formal type reference (peels `Constrained` :38-42; `Reference` to `Slice` means host input :49-50, :405-415). Record/case member selection is keyed on `DataMemberIdentity` ordinal or name spelling (:111-117). Record type match is a string compare of `type_name` against `definition.name` or `normalized_type_identity_text` (:152-153). Layout context is accepted only if the formal's data symbol passes `core_layout_declaration(.. LayoutTargetData)` and is unqualified (:348-359).
5. **Cases by shape.** Primitive formals: only `Bool` with `Bool`, or integer-literal types with `Int` (:459-478), else "invocation value differs from the exact primitive formal". Closed list of formal shapes: Constrained/Reference-to-Slice/Slice/FixedArray/builtin Named/Named+Generic data; everything else "no supported closed source construction" (:508) or "no supported closed declaration" (:285). A byte view requires `BuildTimeValue::Text` (:251). Erased fields refuse (:215-217).
6. **Same job elsewhere.** `decode_snapshot` in `CTI/constants.rs:444` does the reverse walk (value to typed shape) with the same per-shape structure; `CTI/constants/arguments.rs` (read below) supplies constant-initializer arguments.
7. **Unused/one-caller.** `name_path` is `pub(super)`: grepped `name_path` in `driver/`: used in `calls/typed_call.rs:109` and here (see typed_call). `materialize` called from `calls/typed_call.rs:253` only (read).
8. **Stated limits.** "A literal here would instead lend this generated caller's local storage" (:412-413); layout opaque contents "supplied at execution, never constructed in source" (:360).
9. **Fail-open.** (a) `validate_input_declarations` returns `Ok(())` for any builtin-named formal without looking at the value (:269-273): a host-input element of type `u64` given a `Text` value passes this validator; whether a later stage (interpreter binding in `build_invocation`) rejects it is not traced (open). (b) `materialize` zips `parameters`, `arguments` and `input_types` (:31-35), so unequal lengths silently drop operands; the caller `typed_call.rs:160-162` checks `parameters.len() != arguments.len()` first (traced), `input_types` length comes from `instantiate_invocation_signature` (not traced).

## CTI/calls/execution.rs (369 lines, read in full)

1. **Purpose.** Runs one source-authenticated compile-time call in the common Build interpreter under a fixed pure-evaluation budget, and records the run in `SourceCompletionAccounting`.
2. **Shape.** `impl PreparedBuildMachineEntry`: `invoke_recorded` :10, `invoke_with_observations` :29, `invoke_recorded_in_targets` :62 (one run per deployment target when the computation uses an authored layout), `admit_invocation` :118, `layout_supplies` :241, `runtime_supplies` :259, `execute_invocation` :280; free `pure_terminal_invocation_account` :348 (limits `(PURE_PSI_WORK_BUDGET,1,1,1,u64::MAX x4)`).
3. **In/out.** Reads `produced` terminal program (`checked_program_to_psi::production`), `InvocationTarget`, entry fields (`input_values`, `arguments`, `authority`, `purpose`, `source_consent`). Produces a `BuildTimeValue` (and observed calls) and pushes a receipt via `RecordedSourceInvocation::record(...).finish`.
4. **How it decides.** Target name compared to `facts.profile` string for every `LayoutTarget` input (:136-145). Requester custody chosen from `symbol_package_identity` then `symbol_source_origin` (:202-219; no match = `DeclarationCustody::Missing`). Refuses when the program needs imports, missing providers, external completions, or scalar demands without a layout target (:174-182). Root must have exactly one accepted header with `supplies_checked_body` (:192-201).
5. **Cases by shape.** Target-independent vs per-target runs split on `uses_target_layout(module)` (:76); no targets while layout needed is an error (:88-94).
6. **Same job elsewhere.** `pure_terminal_invocation_account` builds a pure sponsor; grepped `pure_terminal_invocation_account` across `source-rust/`: see end-of-record count section.
7. **Unused/one-caller.** Counts at end.
8. **Stated limits.** `layout_supplies` doc: "without layout facts the program keeps no layout supply and refuses at its first layout demand" (:236-240).
9. **Fail-open.** `layout_supplies` returns `Ok(None)` when no target name/facts or no `target_layout_applications` (:247-252): traced, stated (:236-240), refusal happens at first layout demand. `DeclarationCustody::Missing` (:218) is passed to `admit_compiler_evaluation_with_source_consent`; what that does with `Missing` is in another unit (suspected; open).

## CTI/calls/source_selection.rs (113 lines, read in full)

1. **Purpose.** Adapts the entry's captured source consent so the admission step can ask "may this requester select that declaration" using source spans.
2. **Shape.** `InvocationSelection` :14 (entry, custody identity, `SourceMap`, requester span, sorted `(ProofDeclarationId, SourceSpan)`); `new` :23; `source` :84 (binary search); `impl InvocationSourceSelection::allows_selection` :93.
3. **In/out.** Reads the entry's accepted program, terminal artifact proof scope callables, symbol provenance spans. Output is a lookup object, "admission scratch" (:3-4).
4. **How it decides.** Keyed on `ProofDeclarationId` resolved to a source span; compared against consent by span (:106-111) and artifact `custody_identity` equality (:99).
5. **Cases by shape.** None.
6. **Same job elsewhere.** None seen in the unit.
7. **One-caller.** `InvocationSelection::new` called from `calls/execution.rs:220` (read). 
8. **Stated limits.** None.
9. **Fail-open.** `new` returns `None` when consent is absent, invocation scopes differ, no source map, or one declaration maps to two different spans (:27-31,:68-73); execution.rs passes that `None` on as "no selection" (:228-230). Whether `None` means refuse or means no extra restriction is decided in `evaluation_admission` (other unit): open. `allows_selection` itself fails closed (false) on any missing span.

## CTI/calls/target.rs (60 lines, read in full)

1. **Purpose.** Defines `InvocationTarget` and discharges the scalar conditions (ranges, predicates deferred by checking) for one invocation on one target.
2. **Shape.** `InvocationTarget` :15 (name, layout_target, carrier_layout, address_model); `complete_invocation_scalar_applications` :30.
3. **In/out.** Reads the artifact view, target context, provider selections, requested scalar ids; calls `build_scalar_evaluation::evaluate_scalar_applications` :51; returns `AdmittedScalarApplications`.
4. **How it decides.** Missing `layout_target` or `name` is an error (:39-44). The doc says a false condition fails the invocation and none is skipped (:27-29).
5. **Cases by shape.** None.
6. **Same job elsewhere.** Scalar-application admission also done in build crates (suspected, not read).
7. **One-caller.** Called at `calls/execution.rs:314` (read).
8. **Limits.** "a target without layout facts cannot discharge any condition and the invocation refuses" (:11-12).
9. **Fail-open.** none seen.

## CTI/calls/typed_call.rs (319 lines, read in full)

1. **Purpose.** Builds the typed caller (`@const-invocation`) that contains the actual call to the selected declaration, so ordinary call checking judges the call with real argument expressions.
2. **Shape.** `TypedCall {source, caller, call, input_values}` :20; `prepare` :29; `direct_call` :283 (entry state of a machine); `declaration_call` :294 (a bare `TableCallExpression`).
3. **In/out.** Clones the whole `TypedTrees` (`program.clone()` :59) into a probe, adds caller, copies the call operands/static application, substitutes type arguments, materializes arguments via `arguments::materialize` :253. Returns the probe, caller symbol, call expression and input values.
4. **How it decides.** Many string-free identity checks: call node must equal the retained occurrence (:49); selected invocation must not replace an authored receiver (:88); `declaration_query_argument_role == DeclarationDescription` decides whether arguments are runtime (:144-162). Arity must equal checked entry arity (:160).
5. **Cases by shape.** Only type static parameters are materialized: "non-type static arguments are not materialized" (:172-177).
6. **Same job elsewhere.** none seen in unit.
7. **One-caller.** `prepare` called from CTI callers (grepped below). `direct_call` and `declaration_call` counts at end.
8. **Stated limits.** `:176` error text above.
9. **Fail-open.** none seen; every missing handle returns `Err`.

## CTI/constants.rs (637 lines, read in full)

1. **Purpose.** After constants are evaluated, checks that each constant's recorded value is justified by the invocation receipt that produced it, and decodes an evaluator `BuildTimeValue` into a canonical constant value.
2. **Shape.** `key` :21, `family_position` :27, `insert` :32 (sorted-vec set of symbols); `ConstantReaches` :56 (sorted rows constant -> declarations its computation reached); `reached_declarations_unchanged` :84; `validate_constant_evaluations` :169 (pub); `References` :379 and `request_references` :403; `decoded` :429; `decode_snapshot` :444; `decode_snapshot_fields` :612.
3. **In/out.** Reads `TypedTrees` (final program), `SourceCompletionAccounting` receipts, `ConstantReaches`. Output `Result<(), String>`; `decoded` returns `DecodedCanonicalConstValue`.
4. **How it decides.** Handle key `(arena_index, generation)` (:21-23). Receipts matched to constants by requester symbol; initializer machine matched by `generated_from` (:192-195). Reached declarations compared for equality of type reference, plan-laid layout, domain access policy, const declaration (:84-165). Value check compares canonical encoding string of decoded value vs `constant.evaluated_value.encoding()` (:353-365). Record/field matching by spelling: `case.member_identity().spelling() == *variant` (:547), `field.member_identity().spelling() != *identity` (:624), record `type_name` string compare (:483, :528).
5. **Cases by shape.** `decode_snapshot`: layout storage record, fixed array, data record, data case, bool, f32, f64, integer; else the "no product route yet" error (:450-451, :521, :562, :608). Restricted-issuance domains refuse with an authority message (:457-472).
6. **Same job elsewhere.** `decode_snapshot` mirrors `arguments.rs` `data_argument`/`materialize_argument` (CTI/calls/arguments.rs:130,:322) in reverse.
7. **Unused/one-caller.** `insert` is `pub(super)`; `key` is `pub(super)`: grep counts at end.
8. **Stated limits.** "no product route yet: constant result needs its pure snapshot materializer" (:450-451).
9. **Fail-open.** (a) Early `return Ok(())` at :240-247 when no machine was generated for any constant, no receipts, and no constant has an evaluation entry/expression: that skips the loop, which means an `evaluated_value` set without an entry would not be examined (the loop at :270-277 also does not look at `evaluated_value` when there is no entry). Open; no comment states why. (b) A constant with `evaluated_value == None` and no receipts is skipped as "unclaimed request remains pending" (:283-290): stated.

## CTI/constants/arguments.rs (426 lines, read in full)

1. **Purpose.** Finds const arguments written as expressions (`Buffer<capacity()>`, computed array extents), evaluates each as a zero-argument generated machine, and writes the resulting value back into the type argument.
2. **Shape.** `Job` :33 (argument, expression, carrier, owner, holders, roots, application); `pending_argument` :65; `pending` :112 (scans every type-reference row); `Named` :191; `entry` :208 (builds `@const-argument<..>` / `@array-extent<N>` machine); `Arguments {jobs, values}` :230 with `demanded` :236, `into_inputs` :298, `evaluate_job` :302; `impl DemandedValues for Arguments` :366.
3. **In/out.** Reads the type-reference table, declaration type parameters, operand owners (`collect_program_operand_owners` :247), the demanded `DeclarationSelection`. Mutates `TypedTrees` via `data_applications::settle_const_argument` :362 (writes the value spelling), or retains a constant recipe and defers (:316-346). Produces `ValueInputs` for stage 04.
4. **How it decides.** A job exists only for a `ConstExpression` argument whose carrier is a builtin `Integer` atom (or `Bool` when an application asks) and which is not already an integer/boolean literal, not a pending range endpoint, is closed, and is not already a job (:83-93). A job is kept only if one of its owner declarations is demanded (:260-264). Name of the machine built from `format!` of the applied symbol display path and parameter name (:209-221); the name is a label only (inferred). Deferred when the module uses target layout or needs a selected provider and the argument is an array extent (`application.is_none()`, :316-319).
5. **Cases by shape.** Only integer and bool carriers; the result must be `DecodedCanonicalConstValue::Integer` or `Boolean` else "neither an integer nor a Boolean" (:357-361). Doc: "A carrier other than a builtin integer or `bool` ... is left as written" (:109-111).
6. **Same job elsewhere.** Same retained-recipe pattern as `CTI/constants/initializers.rs:285-293` (deferral); `pending` duplicates the "scan every generic application" idea of `CTI/domain_access_plans` (suspected, not compared).
7. **One-caller.** `Arguments::demanded`, `into_inputs` are `pub(in super::super)`; grepped callers after reading `frontier.rs` (see counts).
8. **Stated limits.** The carrier restriction above (:107-111).
9. **Fail-open.** A non-qualifying const argument is simply not a job (:94). Stated (:109-111): it "stays refused where it is demanded". Traced only partly: the refusal site is in typing, not read here. A job with no demanded holder is dropped (:264), stated (:258-259).

## CTI/constants/initializers.rs (419 lines, read in full)

1. **Purpose.** Evaluates each demanded `const` whose initializer is still a machine, once per deployment target that reaches it when it uses an authored layout, and stores the result back as a materialized initializer plus a canonical encoding.
2. **Shape.** `retain_evaluation_entries` :25; `deployment_targets` :56; `Job` :93; `Initializers<'scope> {jobs, values}` :109 with `demanded` :120, `into_inputs` :243, `evaluate_job` :252; `impl DemandedValues for Initializers` :361.
3. **In/out.** Reads `TypedTrees` const declarations and machines, `FixedLayoutTargetScope`s, `SourceCompletionAccounting`. Mutates the const declaration: `pending_target_observation` :291, `materialized_initializer` :355, `evaluated_value` :356; adds an expression :347. Returns `ValueInputs` plus `ConstantReaches`.
4. **How it decides.** Demand = `selected.contains(constant.symbol)` (:128). Defers (sets `pending_target_observation`) when the module observes target layout and either no targets or a scalar result, or has `ScalarApplication::Evaluate`, or needs a selected provider (:276-284). All per-target values must decode to identical canonical encodings, compared as strings (:330-338). Each receipt must have been judged in the round's own program, by `Arc::ptr_eq` (:307).
5. **Cases by shape.** One entry per constant; deployments sorted by `scope.target` (`String` compare :71) and de-duplicated by target equality with an `facts` equality check (:79-84).
6. **Same job elsewhere.** `CTI/constants/arguments.rs:316-346` defers the same way. The validation of this result is `validate_constant_evaluations` in `CTI/constants.rs:169`.
7. **One-caller.** `retain_evaluation_entries` is `pub(in super::super)`; counts at end.
8. **Stated limits.** "The checked initializer is the recipe until a deployment supplies its layout facts or selects its implementation" (:285-286).
9. **Fail-open.** Deferral at :276-294 returns `Ok(())` without a value: stated (:285-286); later checks (`validate_constant_evaluations` :285-290) treat an `evaluated_value == None` constant as pending, so a never-evaluated constant is not an error there (traced: the program is then refused only if something reads it, which is outside this unit). `.expect("constant job was demanded")` :215 is a panic, not accept.

## CTI/constants/tests.rs (804 lines, read in full)

1. **Purpose.** Unit tests for constant evaluation and receipt validation (`#[cfg(test)]` module of `CTI/constants.rs:635-637`). Each test types a small program from a source string (`typed_source` :20, `typed_source_in` :25) and runs `evaluate_demanded_values` (via `evaluate`/`evaluate_with_reaches` :54,:65).
2. **Shape.** Tests: `a_computed_constant_uses_its_source_receipt_only_when_reached` :98; `a_closed_arithmetic_extent_settles_at_its_original_array` :149; `only_a_reader_of_an_array_requests_its_extent` :190; `anonymous_const_argument_requires_the_build_capture_that_owns_the_call` :266 (three capture variants: same map allowed, edited map refused, none refused with "without reconciled package custody"); `a_supplied_requirement_only_evaluates_its_provider_member` :335 (five slot/entry rows, one succeeds); `an_externally_declared_provider_member_requests_its_parameter_layout` :426; `failed_invocations_and_observation_decoding_retain_the_actual_charge` :467; `completed_invocations_release_the_generated_caller_and_program` :557 (16 calls, weak refs must all drop); `each_value_is_evaluated_once_after_the_values_it_reads` :652 (`CHAIN` :642); `a_constant_waits_for_the_constant_that_sizes_a_type_it_reads` :684; `values_that_need_each_other_are_refused_by_name` :704; `a_later_layout_of_data_the_constant_never_reached_is_accepted` :794 and `..._reached_refuses_its_receipt` :799 (helper `join_after_a_later_layout_of` :764 pushes a `PlanLaidLayout` by hand).
3. **Pinned results.** Exact error strings are asserted, e.g. "constant result has no accepted invocation receipt" :135, "constant receipt read a layout that changed after the constant was evaluated" :802, the cycle message :723-725.
4. **How it decides.** Tests are keyed on error message text (string equality) and on receipt order / requester names.
5. **Cases by shape.** none.
6. **Same job elsewhere.** none.
7. **Unused code.** n/a (test file).
8. **Stated limits.** none.
9. **Fail-open.** none seen. The only accept-by-design test is the "never reached" layout join :794.

## CTI/demand_order.rs (327 lines, read below)

## CTI/demand_order.rs (327 lines, read in full; includes 106 lines of in-file tests :221-327)

1. **Purpose.** Computes the order in which compile-time values are evaluated: value A waits for B when A's computation reads a declaration that holds B. It hands out rounds of values that wait for nothing unsettled, and names a circle when none can start.
2. **Shape.** `Round {Evaluate(Vec<usize>), Finished, NoFirstStep(Vec<usize>)}` :20; `DemandOrder` :31 (CSR-style flat tables `need_starts/needs`, `reader_starts/readers`, `waiting_for`, `settled`, `ready`); `new` :53; `needs` :139; `next_round` :147; `settle` :164; `circle` :186. Six tests :247-326.
3. **In/out.** Input: per-value holder symbols, per-value sorted reached symbols, a per-value `waits_for_itself` flag. Output: rounds of positions. No program access.
4. **How it decides.** Keyed on symbol handle `(arena_index, generation)` via `key` :14, joined by binary search of the holder list (:82-90). A value that reads its own holder waits for itself unless `waits_for_itself[v]` is false (:83).
5. **Cases by shape.** none.
6. **Same job elsewhere.** none seen; `driver/compile_time_invocations/demanded_values.rs:178` has its own `key` helper with the same `(u32,u32)` shape, and `constants.rs:21` has another with `(u32,u64)`; grepped `fn key(symbol: SymbolHandle)` in `driver/`: counts at end.
7. **Unused/one-caller.** `DemandOrder` used from `demanded_values.rs:246`.
8. **Stated limits.** none.
9. **Fail-open.** `circle` returns a partial path in the "unreachable while `ready` is empty" case (:204-207): stated; it still returns an error round. `assert!`s panic on misuse (:59-64, :165-168).

## CTI/demanded_values.rs (322 lines, read in full)

1. **Purpose.** Evaluates every compile-time value a request demands (layouts, access plans, const arguments/extents, constant initializers) once each, after the values it reads, in rounds.
2. **Shape.** `ValueDemand` :36 (compilation request, layout scopes, authority, source consent); trait `DemandedValues` :44 (`len`, `holders`, `roots`, `reaches`, `description`, `source_span`, `waits_for_itself` default true :69, `serves_recorded_requests` default false :75, `evaluate`); `RoundProgram` :95 (`calls` and `layout_policies` `PreparedBuildMachineProgram`); `EvaluatedValues` :140; `evaluate_demanded_values` :150 (entry); `reached` :184; `settle_in_demand_order` :212; `no_first_step` :277.
3. **In/out.** Mutates `TypedTrees` (kinds store results). Builds four kinds in this order: `fixed_layouts::Layouts`, `domain_access_plans::AccessPlans`, `constants::Arguments`, `constants::Initializers` (:156-162). Returns `ValueInputs` (arguments first, then constants :169-171) and `ConstantReaches`.
4. **How it decides.** Reachability from `collect_computation_symbol_dependencies` rows (:227-229), per-value `reached` is closure over those rows keyed on handle (:184-204), each symbol mapped through `program.refusal_declaration` (:195). A round with no ready value is an error naming the circle (:255-257). One `RoundProgram` is prepared per round (:260).
5. **Cases by shape.** Four fixed kinds in a slice (:166).
6. **Same job elsewhere.** none.
7. **One-caller.** `evaluate_demanded_values` pub(crate): callers counted at end. `DemandedValues` implemented by four kinds (read: `Layouts` in fixed_layouts, `AccessPlans`, `Arguments` :366, `Initializers` :361).
8. **Stated limits.** Comment :249-251: loop takes at most `places.len()` rounds (inferred correct; not a limit).
9. **Fail-open.** `reached` skips symbols failing `is_valid` (:192-194): silently omits stale symbols from reads, so a value that read a stale symbol would get fewer dependencies; no comment (open). Frontier refresh runs only when a kind `serves_recorded_requests` (:265-270).

## CTI/demands.rs (94 lines, read in full)

1. **Purpose.** Computes the set of source declarations a compilation request reaches (entries, selections, bindings, subject declarations, supplies), used to decide which compile-time values are demanded.
2. **Shape.** `demanded_declarations` :13 (pub(crate)); `FixedLayoutTargetScope::demanded_declarations` :46; `from_supplied_roots` :60.
3. **In/out.** Reads `production::TerminalSourceCompilation`, program declarations, conformance headers, supplies. Output `DeclarationSelection`.
4. **How it decides.** Subject membership by `source_subjects::contains(program, symbol, subjects, purpose)` (:32-37). A supply whose provider is a conformance pulls the header dependencies once (:71-75) and the requirement/realization machines (:79-85); otherwise pushes the provider symbol (:77). Headers are `include`d without following ordinary edges (:88-92).
5. **Cases by shape.** none.
6. **Same job elsewhere.** none seen.
7. **One-caller.** `demanded_declarations` used at `demanded_values.rs:157` (read).
8. **Stated limits.** none.
9. **Fail-open.** `supply.slot` that is not valid is skipped with `continue` (:68-70): open (no comment). `if let Ok(machine) = supply.realization(..)` drops an `Err` (:82): stated ("The structural reader diagnoses missing or ambiguous realizations", :81).

## CTI/domain_access_plans.rs (302 lines), reflection.rs (177), returned_plan.rs (623), tests.rs (1059): see below

## CTI/domain_access_plans.rs (302 lines, read in full)

1. **Purpose.** For each demanded domain that names an access policy (`domain X access Policy;`), runs the policy's `plan` function at compile time and records the checked result on the domain, so later checks can tell who may issue or read/write the domain.
2. **Shape.** `Demand {handle, domain, policy}` :60; `AccessPlans {demands, descriptions, described}` :70 with `demanded` :79; `impl DemandedValues for AccessPlans` :109 (`waits_for_itself` = false :128); `evaluate_domain` :185; `policy_identity` :286. Submodules `reflection` :34, `returned_plan` :40, tests :43.
3. **In/out.** Reads domain definitions, conformance of the policy, core vocabulary tags, the round's `PreparedBuildMachineProgram`. Mutates `domain.access_policy.plan` to `AccessPlanJudgment::Checked(plan)` or `Unjudged(reason)` (:171-180). Calls `entry_selected` :216 and `invoke_recorded` :239 with no target.
4. **How it decides.** A domain is a demand when its policy plan is `Unevaluated`, it is not a Build-prelude symbol (`is_build_prelude_symbol`), has no deferred refusal, and is demanded (:83-91). Policy must be a conformance whose trait has `CoreVocabulary::AccessPolicy` with exactly one row tagged `AccessPolicyPlan` (:205-213): this is keyed on a core-vocabulary tag, not on a name. The receipt must name the domain as requester and have an emitted site (:253). Selected-application equality is by `type_identity_text` string (:235).
5. **Cases by shape.** Refuses with "no product route yet" when the plan has any field observations (:260-262) or any boundary reach services (:263-265). Issuance: `None` -> `Unrestricted`, `Some(routes)` -> `Only(deduplicated authenticated routes)` (:266-277).
6. **Same job elsewhere.** The conformance/receipt checks mirror `CTI/constants.rs:169-376` (receipt join) in structure (inferred).
7. **One-caller.** `AccessPlans::demanded` at `demanded_values.rs:159` (read).
8. **Stated limits.** "no product route yet: an access plan with a nonzero `field_count`" (:261); "...that requires boundary reach" (:264). Module doc :23-25: unjudged plan keeps each use refused.
9. **Fail-open.** Any failure of `evaluate_domain` becomes `AccessPlanJudgment::Unjudged(reason)` and the call returns `Ok(())` (:171-173, :180-181): stated (:145-150, :23-25: uses stay refused). Traced only to the doc, the consumers of `Unjudged` are outside this unit.

## CTI/domain_access_plans/reflection.rs (177 lines, read in full)

1. **Purpose.** Builds the schema argument given to a policy's `plan`, and collects the set of callable descriptions the program's own `reflect::callable(..)` queries produce, so a returned route is accepted only if it equals one of them.
2. **Shape.** `core_reflection_data` :13; `subject_schema` :26 (returns graph, `BuildTimeValue::Struct{storage}`, revision bytes); `Description {route, callable}` :68; `callable_descriptions` :76; `authenticated_route` :162.
3. **In/out.** Reads the expression table for `Call` nodes targeting the core `Reflection(Callable)` machine; produces `DomainEstablishmentRoute` (`BoundaryRequirement`, `CheckedRequirement`, `ExactMachine`) per description.
4. **How it decides.** Keyed on core-vocabulary tag of the call target (:85-88). A route matches a description by full equality of `CallableDescription` bytes `(requirement flag, [declaration, callable, revision])` (:168). More than one differing route for the same bytes refuses (:173-175). `in_scope` uses `source_reference_may_select_symbol` (:108-116).
5. **Cases by shape.** A query is skipped (`continue`) unless it has exactly one argument that is a declaration operand (:101-106), valid application (:94-100), in lexical scope (:125), and `callable_description_identity` succeeds (:118-124).
6. **Same job elsewhere.** "Read the query as Psi 05 does (`read/reflection.rs`, `declaration_query`)" (:91-93): the same query-reading is done in Psi stage 05 (suspected, not read).
7. **One-caller.** `core_reflection_data` also used at `domain_access_plans.rs:198`. Other counts at end.
8. **Stated limits.** none.
9. **Fail-open.** None seen. Skipped queries (:99,:103,:106,:123,:126) yield fewer descriptions, so a plan route naming that callable is refused (:172).

## CTI/domain_access_plans/returned_plan.rs (623 lines, read in full)

1. **Purpose.** Decodes the `BuildTimeValue` an access policy's `plan` returned into an `AccessPlan`, checking every record's exact field roster and every embedded schema/field/callable identity against the actual schema graph.
2. **Shape.** Helpers `record` :30, `roster` :38, `field` :53, `array` :65, `bytes` :72, `word` :79, `boolean` :86, `case` :93, `materialized` :100; `callable` :225, `callers` :250, `described_field` :254, `region` :316 ("Subject" / "Field"), `exposure` :340, `observation` :350 ("Inaccessible"/"Stable"/"External"/"Atomic"), `active` :423, `normalize` :443 (`#[cfg(test)]`), `normalize_for_schema` :463 (pub(super)); `query_schema` :125 is `#[cfg(test)]`.
3. **In/out.** Reads the receipt, plan entry source, `SemanticSchemaGraph`. Output `access_admission::AccessPlan` via `access_admission::normalize_access_plan` :614.
4. **How it decides.** Records matched by exact set of field names (`roster`, string compares :38-51). Variant matched by variant spelling string (`"Machine"`, `"Requirement"`, `"Subject"`, `"Field"`, `"Only"`, `"Unrestricted"`, etc.). Field description checked against the schema's governed fields by `member_identity` bytes via binary search (:289-292). Returned schema must equal the argument value (:543). Callable descriptions authenticated by `authenticate_callable_descriptions` :611.
5. **Cases by shape.** Fixed 32-element storage arrays: "`active`: values.len() != 32" (:427-431), comment: "existing core private construction capacities. They are checked for complete transport, never used as semantic field identity" (:425-426). Atomic operation record has a fixed list of 12 names (:384-396).
6. **Same job elsewhere.** `CTI/constants.rs:444 decode_snapshot` also decodes `BuildTimeValue` records with spelling-keyed field matching; this file uses a name roster instead of declaration order.
7. **One-caller.** `normalize_for_schema` called from `domain_access_plans.rs:257`. `normalize` and `query_schema` are `#[cfg(test)]` (grep for callers below).
8. **Stated limits.** none beyond the 32-element capacity.
9. **Fail-open.** none seen; every unmatched case returns `Err`. One observation: `"Inaccessible"` returns `Ok(None)` (:356-358), meaning the field has no demand; that is a valid variant, not an unmatched case.

## CTI/domain_access_plans/tests.rs (1059 lines, read in full)

1. **Purpose.** Tests for access-plan construction, transport, normalization and enforcement. Source fixtures come from `include_str!("access_plan_construction.omg")` :14 and `include_str!("selected_access_plan_family.omg")` :276,:370,:577 (those `.omg` files are not in this unit; not read).
2. **Shape.** Helpers `facts` :16 (a fixed `linux_x86_64` pointer-8 model), `target` :29, `field`/`array`/`empty_case` :45-68, `assert_zero_tail` :70, `evaluate` :123, `typed_source` :141 (types the actual `source/library`), `reflected_schema` :151, `query_call` :813, `last_receipt` :802. Tests: `actual_access_plan_constructor_keeps_schema_backing_and_complete_zero_tails` :187; `a_selected_access_plan_returns_its_declared_route_element_lifetimes` :270; `actual_policy_callers_keep_their_selected_declaration_after_transport` :369 (encodes/decodes the module, tampers with route and machine declaration, expects `"machine caller describes another source declaration or revision"` :507); `actual_governed_reads_and_delegated_writes_execute_and_refuse_denied_axes` :576 (four variants: allowed, read denied, write denied, write after delegation ends); `ordinary_lifetime_invocation_keeps_schema_images_and_refuses_local_escape` :749; `actual_access_plan_normalization_preserves_rules_and_authenticates_descriptions` :851.
3. **Pinned results.** Error substrings such as "this function is not allowed to read this value" :665, "not the genuine core AccessPlan declaration" :1055, "callable description is not bound to the actual source" :1037.
4. **How it decides.** String substring checks on error text.
5. **Cases by shape.** `assert_zero_tail` pins 32-element arrays :72, :96, :113.
6-9. none. No fail-open observed; line 263 `accounting.has_complete_usage()` asserted.

## CTI/fixed_layouts/mod.rs (436), placements.rs (352), reflection.rs (200), schema.rs (136), tests.rs (223), validation.rs (146), CTI/frontier.rs (524), CTI/typed_invocation.rs (94): see below

## CTI/fixed_layouts/mod.rs (436 lines, read in full)

1. **Purpose.** For each storage application written `LayoutStorage<T, Policy>` that a deployment demands, runs the selected layout policy's `plan` at compile time with the record's field roster and the target facts, validates the returned layout, and installs it as `PlanLaidLayout` rows (one geometry per target) in the typed trees.
2. **Shape.** `PrivateLayoutPlacementReceipt` :31; `EvaluatedLayout` :39; `PreparedBuildMachineEntry::invoke_layout_policy` :45; `Demand {scope, requester, storage, holders, roots}` :74; `Layouts<'scope>` :91 with `demanded` :97; `impl DemandedValues for Layouts` :219 (`serves_recorded_requests` true :247); `evaluate_application` :293. Submodules `placements`, `reflection`, `schema`, `validation`, `tests`.
3. **In/out.** Reads `FixedLayoutTargetScope`s, program type occurrences, policy conformance. Mutates `typed.plan_laid_layouts`, `plan_laid_target_layouts`, `plan_laid_schema_fields`, `plan_laid_retired_field_identities` (:373-433). Calls `entry_selected` :321 with arguments `[schema record, LayoutTarget(facts)]`.
4. **How it decides.** Refuses at entry if `plan_laid_layouts` is already non-empty (:101-105). Scopes keyed on `target: String`; same target must have equal facts (:111-116); `facts.profile != scope.target` refuses (:123-127). A demand is one per `(scope, normalized_type_identity(storage.application))` (:174-175). A target is skipped when its geometry is already installed (:260-266). Selected policy must equal the conformance (`selected.symbol != policy.symbol` :335). Existing plan with `Invariant` realizations refuses to take a target-specific row (:401-405); a second invocation for the same target refuses (:407-412).
5. **Cases by shape.** Only closed storage whose semantic data has no type parameters (:145-149: "Open library templates remain requests of their declarations"). `abi_version: 4` hard-coded (:378).
6. **Same job elsewhere.** The schema-argument/validate/close pipeline for access plans is `CTI/domain_access_plans.rs:185`; both call `entry_selected` then `named_conformance_call_member` (:331 here, `domain_access_plans.rs:224`).
7. **One-caller.** `Layouts::demanded` at `demanded_values.rs:156` (read).
8. **Stated limits.** "Open library templates remain requests of their declarations" (:143).
9. **Fail-open.** With no scope having entry or provider declarations, `demanded` returns an empty set after `refresh_evaluated_requests` (:137-142), and also when no closed storage request exists (:176-179). That leaves layout requests deferred (test `an_empty_layout_scope_preserves_the_unrequested_application`, tests.rs:56, pins that no plan is invented); traced to the test only.

## CTI/fixed_layouts/placements.rs (352 lines, read in full)

1. **Purpose.** Records which private layout-placement calls (sealed core helper `PrivateLayoutPlacement`) a layout policy actually executed, decodes their arguments from interpreter observations, and validates that the placements do not overlap public fields.
2. **Shape.** `Site` :16; `executed_sites` :23; `decode` :110; `placement` :194 (`At` / `Bits`); `close` :231; `validate` :305.
3. **In/out.** Reads accepted source, terminal module invocations, observed calls; produces `PrivateLayoutPlacementReceipt`s and `PrivateCallbackLayoutDemandReport`s.
4. **How it decides.** Machines keyed on `SealedIntrinsic::PrivateLayoutPlacement` (:34-35). Emitted call sites are matched in both directions by `(machine, site)` (:70-72, :104-106). Placement variant keyed on variant spelling `"At"`/`"Bits"` (:216-228) and FieldPlan case by id. Slot identity is a `format!("{}#{:016x}", ...)` of declaration identity and a fingerprint (:291-294). Overlap test on bit ranges (:348).
5. **Cases by shape.** Only `At` and `Bits` placements (:227 "needs a pointer offset or bit fragment"). Argument index fixed `1` (:80).
6. **Same job elsewhere.** none seen.
7. **One-caller.** `executed_sites`/`decode` called in `mod.rs:51,:59`; `close` `mod.rs:358`; `validate` `mod.rs:368`.
8. **Stated limits.** none.
9. **Fail-open.** `executed_sites` skips source call rows that are not expressions or lack `private_layout_operation` (`continue` :58, :61, :64) and then requires `emitted.len() == sites.len()` (:104), so the other direction is checked. Traced. `validate` returns `Ok(())` when there are no private callback demands (:310-312): stated by structure (nothing to overlap).

## CTI/fixed_layouts/reflection.rs (200 lines, read in full)

1. **Purpose.** Builds the policy's schema field roster (size, alignment, kind, identity per field) from the record's actual field types on a target.
2. **Shape.** `record_fields` :9; `field_geometry` :72 (recursive, depth capped by `layout_plans::CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT` :78).
3. **In/out.** Reads type references, nested `plan_laid_layout_for_type`, target facts; returns `Vec<PlanLaidSchemaField>`.
4. **How it decides.** Field keys are positions plus one (:40-43). Fields sorted by stable member identity when all have one; mixing numbered and unnumbered refuses (:51-56). Primitive kinds by `PrimitiveType` match (:88-103); `Selector` refuses (:100-102). Nested data uses `conventional_record_geometry` or `conventional_sum_geometry` (:180-194).
5. **Cases by shape.** Records only: "a fixed record policy cannot omit the semantic type's sum cases" (:20); quotient data refuses (:14-16, :148-152); references and slices refuse: "layout field needs an established fixed representation" (:198; comment :196-197).
6. **Same job elsewhere.** Geometry rules duplicate `layout_plans::packing` (calls it); `layout_planning::field_integer_geometry` called :92.
7. **One-caller.** `record_fields` called at `mod.rs:312`.
8. **Stated limits.** ":196 Reference and slice storage must come from the same representation selection as native emission".
9. **Fail-open.** none seen.

## CTI/fixed_layouts/schema.rs (136 lines, read in full)

1. **Purpose.** Builds the `BuildTimeValue` for the core layout `Schema` record handed to a policy.
2. **Shape.** `FIELD_CAPACITY = 32`, `CASE_CAPACITY = 32` :10-11; `record` :13; `field` :46; `vacant_case` :76; `record_argument` :99.
3. **In/out.** `PlanLaidSchemaField`s and retired identities in; a `Value::Struct{type_name: "Schema"}` out.
4. **How it decides.** More than 32 fields or 32 retired identities refuses (:103-105). Record type names are short literals `"Schema"`, `"SchemaField"`, `"SchemaCase"` (:63,:118) not canonical identities (compare `domain_access_plans/reflection.rs:51-62`, which uses the canonical identity).
5. **Cases by shape.** `cases` always 32 vacant, `case_count` 0 (:125-128): a record-only schema.
6. **Same job elsewhere.** The 32-capacity padding is the counterpart of `returned_plan.rs:427` for access plans.
7. **One-caller.** `record_argument` at `mod.rs:318`.
8. **Stated limits.** "The core Schema has fixed array capacities ... truncation could turn an incomplete plan into an apparently complete one" (:96-98).
9. **Fail-open.** none seen.

## CTI/fixed_layouts/validation.rs (146 lines, read in full)

1. **Purpose.** Checks a layout policy's returned record against the supplied field roster and turns it into a `LayoutPlanReport` (offsets, size, alignment).
2. **Shape.** `field` :9, `count` :23, `record_plan` :33.
3. **How it decides.** Entries array must have length 64 and `entry_count <= 64` (:43). `size_is_dynamic` true refuses (:49). Alignment must be a power of two and size a multiple (:54). Each entry key matched to a schema field by `key` (:64-67); only the `At` case with an `offset` payload is accepted, variant normalized with `rsplit("::")` (:72-80). Duplicates, misalignment, overflow, past-end and overlap each refuse (:81-127). Entries sorted back to semantic order (:129-137).
4. **Cases by shape.** "layout representation `{variant}` has no established fixed-field validation" (:76-78): fixed fields only.
5. **Same job elsewhere.** `placements.rs:305 validate` checks overlap of private callbacks.
6. **One-caller.** `record_plan` at `mod.rs:353`.
7. **Stated limits.** The 64-entry carrier (:43-44).
8. **Fail-open.** none seen.

## CTI/fixed_layouts/tests.rs (223 lines, read in full)

Three tests: `an_empty_layout_scope_preserves_the_unrequested_application` :56 (foreign-profile facts refused, empty roots yield no plan and keep a `DeferredRequest::LayoutPolicyStorage`); `generic_layout_policy_retains_its_declared_machine_and_observed_geometry` :96 (wrong semantic type `OtherPair` refused with "selected layout policy belongs to another semantic type"; right one gives offsets `[8,0]`, size 16, align 8); `authored_policy_observes_only_its_invocations_issued_target` :172 (two targets, sizes 16 and 24). Fixture: `include_str!` of `samples/cli/facts/fixed_layout_storage/local/main.omg` via a 7-level `../` path :3-4 (not in this unit). Pinned results are exact numbers and message substrings. Fail-open: none seen.

## CTI/typed_invocation.rs (94 lines, read in full)

1. **Purpose.** Creates a generated machine (`@const-invocation`, `@const-argument<..>`, etc.) whose single state returns a given expression, owned by the requesting declaration.
2. **Shape.** `TypedInvocationDeclaration::{new :22, define :39}`; `TypedInvocationBody {machine, state}` :62 with `return_expression` :68.
3. **In/out.** Mutates the symbol table (`insert_generated_root_from`) and machine table (`push_machine`, `set_machine`).
4. **How it decides.** `return_expression` requires exactly one machine position for the symbol (:75-77).
5-9. none seen. Callers: `calls/typed_call.rs:64`, `constants/arguments.rs:223` (read).

## CTI/frontier.rs (524 lines, read in full; 70 lines of in-file tests :455-524)

1. **Purpose.** Carries unevaluated compile-time requests (layout policy storage, type expressions, array extents, materialized expressions) as "deferred refusals" on the declaration that holds them, so a program that never reaches that declaration still compiles and one that does is refused there. Also holds `PreCheckEvaluation`, the continuation record saved beside retained source.
2. **Shape.** `defer_source_jobs` :25; `refresh_evaluated_requests` :40 (pub(super)); `record_jobs` :65; `source_jobs` :77; `require_no_recorded_work` :84; `declared_jobs` :139; `require_no_unevaluated_constant_use` :238; `PreCheckEvaluation {declaration_scope, needs_scope_check}` :311 with `new` :318, `rejoin_source_authority` :325, `same_pending_work` :349, `defer_declared_requests` :360, `require_recorded_requests` :367, `activate_sources` :390, `record_continuations` :396, `require_rejoined_authority` :411, `record_extension` :419; `impl Record` :433 with tag `"BTE-source-input-7"`.
3. **In/out.** Reads/mutates `TypedTrees.deferred_refusals`; reads `references::collect_source_evaluation_references` and operand owners. Writes the tag string plus the declaration-scope checkpoint.
4. **How it decides.** Request kind keyed on type-reference node shape: layout storage when `layout_storage_arguments` is set and no plan exists (:157-162), `ConstExpression` without a recipe (:164-170), `FixedArray` with expression or const-call length and no declared extent (:171-176); every other node variant is listed and ignored (:177-185). A request with no owning declaration refuses (:204-209, :219-224). `refresh_evaluated_requests` drops `LayoutPolicyStorage` rows always (re-recorded from the inventory) and drops `TypeExpressionEvaluation`/`ArrayExtentEvaluation` rows no longer in the inventory (:53-56). The continuation is matched by the literal tag `"BTE-source-input-7"` (:442).
5. **Cases by shape.** `require_no_recorded_work` lists nine recorded-work categories, each refusing with "no product route yet: ..." (:86-132): constant recipes/contexts/closures, pending range endpoints, static type contexts, wire schemas with encoding or plans/placements/obligations, access-policy invocations/view/lifecycle plans, plan-laid layouts, designated static type arguments with no custody.
6. **Same job elsewhere.** `calls/arguments.rs` and `constants/arguments.rs` both consume `TypeExpressionEvaluation`/`ArrayExtentEvaluation` rows (read).
7. **One-caller.** `refresh_evaluated_requests` called at `fixed_layouts/mod.rs:140,:177` and `demanded_values.rs:269` (read). `PreCheckEvaluation` is `pub`; external users grepped below.
8. **Stated limits.** `unsupported()` helper :16-18 "no product route yet: {job}" used 8 times; `:444` "no product route yet: retired source evaluation continuation"; :229-237 "a referenced constant without a recorded value is work for an evaluator that does not exist yet".
9. **Fail-open.** (a) `refresh_evaluated_requests` removes deferred-refusal rows for type expressions and array extents that are no longer listed (:53-56). Stated (:37-39): the row "goes before the next value reads it". A bad input would be a request that stopped being listed while still unevaluated; traced only to the doc, not to the inventory builder. (b) `declared_jobs` ignores every other `TypeReferenceNode` variant (:177-185) by an exhaustive match, no wildcard (a new variant would fail to compile). (c) `record_jobs` falls back to `unwrap_or_default()` span for a declaration with no span (:67-70): affects diagnostics only. (d) `rejoin_source_authority` lets a missing `sources` pass when either scope is `None` (:332): the check requires both `Some` and no source map to fail; if `declaration_scope` is `None` the call proceeds to `require_current` (open: `require_current` semantics not read).

## driver/judge_lowered_source.rs (298 lines, read in full)

1. **Purpose.** Freezes the settled typed source, lowers it, runs the lowered-form checker, and hands the publisher a source-bound judgment, keeping the checker's advisories.
2. **Shape.** `SettledSource {program, values, selected_providers, accounting}` :24 with `materialized_call` :32, `freeze` :41, `judge` :59; `FrozenSource` :81; `JudgedSource` :86; `SourceRoots {Selected, Product}` :91; `judge_source` :99; `judge_terminal_source` :124 (pub); `SourcePublication` :141 with `judge` :154, `advisories` :174, `check` :180; `#[cfg(test)] judge_authored_ieee` :191 and two module-level `#[test]`s :231, :273.
3. **In/out.** `preparation::prepare_typed_trees` then `lower_for_publication`/`lower_roster` then `judge_publication`; output `AcceptedTerminalSource` or `Vec<Diagnostic>`.
4. **How it decides.** The verdict is the lowered-form checker's (`judge_publication` :120). Stage tags `DiagnosticStage::{Preparation,Lowering,Checking}` attribute errors (:45,:118,:121). `judge_terminal_source` ignores advisories (:129 "A selected invocation is used and never reported").
5. **Cases by shape.** `SourceRoots::Selected` (entry list) vs `Product` (roster).
6. **Same job elsewhere.** none seen.
7. **One-caller.** `SettledSource::materialized_call` and `judge` counted at end.
8. **Stated limits.** none.
9. **Fail-open.** none seen. Tests pin "float meaning wrapper does not establish its result identity" :259 and "proof-only FloatMeaning has no runtime ABI" :268, "float meaning equality mixes permanent formats" :295.

## driver/package_handoff.rs (46 lines, read in full)

1. **Purpose.** Computes the provider selections, native declarations and native intents a dependency's Build handoff carries, from the package's own typed declarations.
2. **Shape.** `PackageHandoffDeclarations {providers, native, intents}` :8; `package_handoff_declarations` :14.
3. **In/out.** `StageSource` plus native binding formations in; captured provider selections via `capture_provider_selection` :31 and `capture_package_native_bindings` :35 out.
4. **How it decides.** Requires `source.product.subject` (a named package) else error (:18-22). Admission through `product.policy.admit(&mut typed, Some(package))` :26.
5-9. none seen; counts at end.

## driver/parse_source_batch.rs (111 lines, read in full)

1. **Purpose.** Entry point for turning a batch of loaded source files into syntax trees. Three ways in: append fresh pages, append while reusing retained parses, or parse each file independently for later reuse.
2. **Shape.** `SourceInput(pub LoadedSources)` :17 with `append` :20, `append_reusing` :29, `append_pages` :53, `into_parsed_units` :71, `parse_batch` :96. Re-exports from `parsed_files` :6-9. Test modules `reused_sources`, `tests` :108-111.
3. **In/out.** `LoadedSources` in; `ParsedSourceBatch` (via `SourceSyntax::retain_batch`), or `Vec<RetainedSourceParse>`. Mutates `SourceSyntax.trees` by adopting pages.
4. **How it decides.** A retained parse is reused only if `unit.source == file.source` (text equality of `Arc<str>`) (:41). Reuse layout is chosen if any file has a unit (:44-48). Page id must not overflow `u32` (:59-62).
5. **Cases by shape.** `Layout::{Shared, Independent, Reuse}`.
6. **Same job elsewhere.** `source_lexer::lex`/`source_parser::parse` whole-batch path used in tests (tests.rs:43,:52).
7. **One-caller.** Counts at end.
8. **Stated limits.** none.
9. **Fail-open.** none seen. Retained parse for different bytes falls through to a fresh parse (test `retained_parse_of_other_bytes_is_not_adopted`, reused_sources.rs:115).

## driver/parse_source_batch/batch.rs (157 lines, read in full)

1. **Purpose.** Runs stages 00 (lex) and 01 (parse) for each fresh file, possibly in parallel pages, and places retained (relocated) files in the same input order.
2. **Shape.** `Layout<'a>` :10; `ParsedPageSources {sources, lexical_error, parse_error, reuse_error}` :17; `parse` :24; `parse_page` :131.
3. **In/out.** `LoadedSources` to `(Vec<SyntaxTrees>, ParsedSources)`. Uses `source_lexer::lex_file`, `source_parser::parse_file` :141,:149, `rayon` `par_chunks_mut` :92, `worker_budget`.
4. **How it decides.** Page count: `count.max(1)` pages for non-shared layouts, else `min(count, workers*4)` (:31-37). Error priority: all lexical errors first across pages, then parse errors, then reuse errors (:100-114); after a parse error later files are still lexed "to preserve lexical-error precedence" (:136-138). Reuse failure from `unit.at_coordinates(source_id, page)` becomes `reuse_error` (:60-74).
5. **Cases by shape.** A retained file owns exactly one page; `unreachable!` otherwise (:56-58).
6. **Same job elsewhere.** none seen in the unit.
7. **One-caller.** `batch::parse` from `parse_source_batch.rs:103`.
8. **Stated limits.** none.
9. **Fail-open.** none seen.

## driver/parse_source_batch/parsed_files.rs (340 lines, read in full)

1. **Purpose.** Holds the syntax forest kept for an invocation (`SourceSyntax`), summarizes each parsed file (imports, Build-related roots) for package discovery, and defines a reusable retained parse.
2. **Shape.** `SourceSyntax {trees, partial_build_entry, partial_build_can_return, files}` :11; `ParsedFile` :21; `SourceItems {handles, build_machines, build_data}` :119; `SourceImport` :134; `SourceImportTarget {Namespace, File}` :141; `ParsedSourceSummary` :166; `ParsedSourceBatch` :177 with `project` :182; `RetainedSourceParse` :261 with `free_machines` :272 and `declarations` :290; `declared_names` :311; `take_selected_items` :76.
3. **In/out.** Reads syntax items; writes summaries and file records.
4. **How it decides.** A Build machine is a free machine (no `attached_data`) whose name string equals `"build"` (:230-232); `Build` data is a data item whose name string equals `"Build"` (:238): source spelling. A parsed file is accepted by `take_selected_items` only if the source map's entry matches on path, text, scope, origin and is selected and not repeated, and counts equal (:83-102). Import path building: file imports are joined with `/` and `.omg` appended (:206-211).
5. **Cases by shape.** `declared_names` matches every `Item` variant with no wildcard (:323-339); `Package` and `Use` yield nothing.
6. **Same job elsewhere.** Name-spelling checks for `build`/`Build` may recur in package-compilation (suspected, not read).
7. **One-caller.** Counts at end.
8. **Stated limits.** none ("Acquisition can retain them, but cannot fabricate or edit a declaration through them" :116-117).
9. **Fail-open.** none seen; `.expect("a parsed import has a name")` :200 panics on an empty path.

## driver/parse_source_batch/reused_sources.rs (206 lines, read in full) and tests.rs (95 lines, read in full)

Test-only modules (`#[cfg(test)]` at `parse_source_batch.rs:108-111`).
- `reused_sources.rs`: four tests. `adopted_discovery_parse_equals_a_fresh_parse_at_admitted_coordinates` :38 (every file in `source/library/core` parsed independently under private ids, then adopted and compared with a fresh parse; asserts more than 20 sources :40); `retained_parse_of_other_bytes_is_not_adopted` :115; `mixed_reuse_places_the_roots_and_source_records_in_capture_order` :135 (reuse masks 1,2,5,10,15); `refused_reuse_does_not_leave_the_fresh_file_in_the_forest` :176 (expects "standalone unit" error).
- `tests.rs`: `stage_batch_preserves_lexical_error_priority_and_source_order` :36; `stage_batch_matches_page_identity_metadata_and_owned_literals` :61 (97 generated files; expects the timing phase name `"Source ingress: SourceFiles -> SyntaxTrees"` :80, a display-string compare).
No fail-open seen.

## driver/prepare_typed_source.rs (384 lines, read in full; tests :139-384)

1. **Purpose.** Takes a typed program (stage 03 output) plus the request's entries and admissions, evaluates all demanded compile-time values, joins constant receipts, and returns a frozen or judged source.
2. **Shape.** `SourceAdmission` :14 (invocation consent, external roots, representations, layout scopes, layout authority, wire demands, declarations authority); `ProductRepresentationInput` :26 with `rederive` :34; `SourcePreparationRequest` :51 with `judge` :64, `prepare` :72, `settle` :76; re-export `SourcePreparationFailure` :59.
3. **In/out.** Calls `evaluate_demanded_values` :95 and `validate_constant_evaluations` :105, then builds `SettledSource` (:119-130) whose `selected_providers` come from every layout scope's `provider_declarations` (:122-128). Always calls `accounting.release_constant_inputs()` :117.
4. **How it decides.** Retained opaque representation choices must equal the choices re-derived from the program (`selected != self.retained` :40, vector equality). Refuses outright when `external_roots` is non-empty or `declarations` is `Some` (:81-85) and when `wire_demands` is non-empty (:86-88).
5. **Cases by shape.** Those two refusals are fixed "no product route yet" cases: "source selection authority admission" and "typed preparation: requested work: wire compatibility demands".
6. **Same job elsewhere.** none seen.
7. **One-caller.** `SourcePreparationRequest::judge`/`prepare` counted at end.
8. **Stated limits.** ":79-80 Opaque property receipts came from the deleted checker. Supplying one cannot stand in for evidence from the new route."
9. **Fail-open.** none seen. Tests `an_old_external_root_request_is_not_silently_discarded` :208 and `unauthored_representation_inputs_are_refused` :241 pin that declared/executed/retained inputs refuse with exact messages.

## driver/registered_tests.rs (199 lines, read in full)

1. **Purpose.** Discovers tests registered through Build (`TestGroupRegistration`) and compiles each as its own program entry (`ProgramEntry`) from the same captured source.
2. **Shape.** `TestRoot {group, name, group_declaration, selecting_machine, declaration}` :11 with `bind` :20; `satisfiers` :64; `RegisteredTests {source, continuation, roots, notes}` :81 with `discover` :89 and `compile` :185.
3. **In/out.** Reads `StageSource`, resolves and judges with `ProductEntry::BuildBindings`, then per root `ProductEntry::RegisteredTest{root,target}`, publishes with `source.publish`, returns `PublishedTargetProgram` via `published_target`.
4. **How it decides.** A satisfier is a machine in the same package as the registering machine whose trait-conformance edge resolves to the group requirement (:68-78): identity-keyed. Roots rejoin by retained declaration identity (`source_symbols::rejoin_declaration`). The slot is `format!("{target}::ProgramEntry")` (:56) and tests are sorted by `name` string (:174). Display paths `display_path(.., "::")` used as labels (:120, :129).
5. **Cases by shape.** A test must be a non-generic machine with a body and an entry taking zero parameters (:130-146): refused with "not runnable". A group with any `services` refuses (:163-168): "no accepted binding identifies that slot without trusting a package name".
6. **Same job elsewhere.** none seen.
7. **One-caller.** `RegisteredTests::discover` counted at end.
8. **Stated limits.** ":165 it backs only std's `FilesystemService`".
9. **Fail-open.** `discover` returns `Ok(None)` when no executed Build event is a `TestGroupRegistration` (:94-98): stated by construction (nothing registered). A disabled group yields a note and no roots (:121-126).

## driver/stage_sources.rs (79 lines, read in full)

1. **Purpose.** Bundles the parser output, capture map, root ids and import bindings supplied to one Build or Product, and turns them into a `ResolveAndTypeRequest`.
2. **Shape.** `ParsedStageSources {syntax, files, roots, imports}` :13 with `parse` :22 and `into_request` :51.
3. **How it decides.** Needs at least one root (:27-30). Only `selected_files()` are parsed (:32). `into_request` requires the primary and every root to be selected (:56-65), then calls `take_selected_items` (:67). `build_helper_identity: host` is a `&'static str` passed through (:74).
4. **Cases by shape.** none. Fail-open: none seen.

## driver/resolve_and_type.rs (567 lines, read in full)

1. **Purpose.** Runs stages 02 (resolve) and 03 (type) for one invocation's source: resolves declarations, completes Build-scope bodies, types them, and keeps a resumable checkpoint (`SourceTypingCheckpoint`) so Build queries and generated source continue from the same declarations. Later it completes the remaining authored bodies and extends the program with generated Product source.
2. **Shape.** `ResolveAndTypeRequest` :20 (syntax, source_items, root_sources, frontends, `build_helper_identity: &'static str`, sources, top_level_bindings, selection_authority) with `with_build_entry` :83, `prepare` :129, `resolve_build_scope` :136, `prepare_build_projection` :151; `SourceTypingCheckpoint {input, completions}` :32 with `complete_selected` :280, `complete_authored` :310; `SourceCompletionInput` :38; `CompletedSource` :46 with `finish` :436, `extend` :443; `SourceExtensionRequest` :53; `resolve_build_bodies` :257; `type_completed_bodies` :348; `extend_declarations` :372; `complete_pending_bodies` :397; `SourcePreparation` :504 (mutex-held `SourceCatalogue`); `SourceFrontends` :520 (`Arc<Mutex<Vec<Arc<SourceCompletionInput>>>>`) with `retain` :553, `fork` :528, `replace_with` :534.
3. **In/out.** Reads `SourceSyntax`, source map, bindings, previously retained frontends. Produces `SourceTypingCheckpoint`, `CompletedSource`, `CompletedTypedSource {accounting, typed}`. Mutates the retained list in `SourceFrontends` (:553-563).
4. **How it decides.** `with_build_entry` finds the Product counterpart of a dual-purpose Build file by equal path, source text, origin, package root, resolution stratum and package identity (:92-102) and removes the Build entry machine from the Product copy by name-span equality `machine.name.source_span().span == declaration.span()` (:114-125). Reuse of retained frontiers is decided in `activation.rs`. Loops complete Build-and-toolchain bodies until none remain pending (:216-232). `same_completion_input` compares typing, context, pending bodies and pre-check work by equality (:489-499).
5. **Cases by shape.** `DeferredBodyScope::{BuildAndToolchain, SelectedFiles, All}` selects which bodies complete (:218, :296, :331).
6. **Same job elsewhere.** `complete_pending_bodies` :397 and the loop in `prepare_build_projection` :216-232 and `resolve_build_bodies` :257-277 repeat the "pending machines, complete, type" loop three times with different scopes.
7. **One-caller.** counts at end.
8. **Stated limits.** none.
9. **Fail-open.** `with_build_entry` returns the request unchanged when there is no declaration, no Product counterpart, or no matching root (:87-89, :103-105, :111-113); in those cases the Build entry machine stays in the Product forest. Doc :80-82 describes the dual-purpose case only: open. Retained-frontier reuse: see activation.rs. `Mutex` poisoning refuses with "source frontend retention owner is poisoned" (:548).

## driver/resolve_and_type/activation.rs (336 lines, read in full; tests :234-336)

1. **Purpose.** Decides whether an earlier retained typed frontier can be reused for the files now requested, and if so narrows it to those files ("activation").
2. **Shape.** `SourceContext {bindings, declaration_scope, build_helper_identity}` :13 with `same_input` :20 and `compatibility` :26; `SourceReuseMiss {BuildHelperSelection, LookupContributors, Imports, ProductAliases, DeclarationAuthority}` :81; `ActivatedSource {typing, pre_checks, sources}` :89 with `remove_retained_syntax` :96; `select_retained_source` :123. One test :243.
3. **In/out.** Reads all retained frontiers from `request.frontends.snapshots()`; returns the one with the most reusable files that passes every check, else `None`.
4. **How it decides.** Candidates keyed on shared source storage (`retains_source_storage` :136), selected-source set difference excluding root sources and parked Build entry sources (:143-148), `count == 0` skip (:150), then `compatibility` (build-helper identity string equality :33; added file that is the same checked instance as a previous one :36-49; per-file import bindings equal :51-61; product dependency scope :65; declaration authority :69-77). Candidates ordered by file count, most first (:161). After activation: no machine may still have `MachineBodyStatus::Deferred` (error :174-180); any referenced symbol or checked authored selection whose provenance span lies outside the selected sources discards the candidate (:194-208); each pre-check must rejoin the current authority or the candidate is dropped (:213-224).
5. **Cases by shape.** none.
6. **Same job elsewhere.** `PackageDeclarationScope::require_current` appears in `frontier.rs:335` and here :69 (read).
7. **One-caller.** `select_retained_source` called at `resolve_and_type.rs:163`.
8. **Stated limits.** none ("file identity alone cannot justify reuse" :141 is a rationale).
9. **Fail-open.** Every failing check is `continue` (candidate skipped, fresh typing used): fail closed for reuse. The one accept-path risk: `compatibility` errors are discarded (`.is_err()` :153-157), and the span check treats a symbol with no provenance span as acceptable (`is_some_and(...)` :204 is false for `None`): a referenced symbol with no span is not counted as outside the selection (open, not traced).

## driver/resolve_and_type/authored_completion.rs (67 lines, read in full)

1. **Purpose.** A mutex-guarded list of completed authored-source results, found by input equality and reused so the same completion is not recomputed.
2. **Shape.** `AuthoredCompletions {entries: Arc<Mutex<Vec<Completion>>>}` :8 with `find` :19 and `retain` :40.
3. **How it decides.** `find` returns the first entry whose input `same_completion_input` (:26), after validating accounting charges (:30-34) and marking the clone reused (`mark_reused` :36). `retain` stores only when accounting has complete usage (:45-47).
4. **Same job elsewhere.** `SourceFrontends` (`resolve_and_type.rs:520`) is a second mutex-held retention list; the `cache_state` ratchet name suggests both are tracked (suspected, not checked).
5. **Fail-open.** `retain` silently skips when usage is incomplete (:45-47): the result is simply not cached. none seen otherwise.

## driver/resolve_and_type/storage.rs (22 lines, read in full)

Implements `record_storage::Record` for `SourceContext` (write: bindings, helper identity, declaration-scope checkpoint; read: same order) :5-22. No decisions. Grep of `impl Record for SourceContext` shows this single site.

## driver/resolve_and_type/tests.rs (207 lines, read in full)

Four tests: `resolution_handoff_completes_each_batch_without_changing_its_predecessor` :40; `nonempty_selected_completion_retains_prior_refusal_rows` :73 (expects `[TypeExpressionEvaluation, ArrayExtentEvaluation]` rows per machine and 4 total :122); `resolution_handoff_refuses_cross_paired_completion_and_activation` :127 (message "exact retained resolved predecessor"); `completed_body_work_is_recorded_before_retained_source_activation` :150 (judging a helper whose array extent is unevaluated is refused with the request's `construct()` text :201-205). Fail-open: none seen.

## Area 02: `pipeline/00_source-files-to-tokens/` (paths relative to `source-rust/subsystems/compiler/source-to-psi/pipeline/00_source-files-to-tokens/`)

### benchmarks/lexer_timing.rs (108 lines, read in full)

1. **Purpose.** A benchmark binary that times the real lexer (and optionally the parser) over given source files, printing JSON `omega-timed-work/1`.
2. **Shape.** `main` :13 (one-worker `ExecutionBudget`), `measure` :19, `loaded` :100. Usage `lexer_timing <lex|parse> <repeats> <source>...` :26.
3. **In/out.** Reads files from the command line; calls `source_compiler::test_support::lexing::lex` :58,:78 and `test_support::parsing::parse` :84. Prints `elapsed_ns`, `work_units`, a checksum and a fingerprint.
4. **How it decides.** The fingerprint hashes each token's `format!("{:?}", token.kind)` (debug string), span, and decoded bytes or spelling (:63-70): a `format!` of a debug value, used only to detect candidate-run drift.
5-9. none (benchmark). Stated: "Candidate runs must preserve this complete observation, not only token counts" :54.

### lexer.rs (265 lines, read in full)

1. **Purpose.** Stage 00: turns source text into ordered tokens with spans, in a single byte scan, and reports the first lexical error.
2. **Shape.** `lex` :23 (batch; parallel via `par_iter_mut` when more than one source :30); `lex_source` :42; `lex_file` :48 (pub, streaming); `lex_unit_reusing` :68; `ByteScanner` :85 with `fill_tokens` :112, `scan_token` :127, `scan_block_comment` :184, `raw_quote_end` :211; `LexError {message, span, location}` :231 with `into_diagnostic` :253.
3. **In/out.** `LoadedSources` to `LexedSources` (tokens retain the source `Arc<str>` :62). Token capacity guess `len/4` min 16 (:72).
4. **How it decides.** Keyed on the first byte (:128-177). Identifiers are ASCII only (`is_identifier_start` :220). Keywords by `KeywordKind::from_lexeme(&source[start..offset])` string match (:156). Punctuation by longest `PunctuationKind::matching_prefix` (:166). A leading `r` followed by `#*"` is refused as a raw string (:148-152). Block comments nest (`depth` :185-203).
5. **Cases by shape.** Fixed message "spelling is outside the current language profile" (:228) for any byte that matches no token (e.g. non-ASCII, `r"..."`).
6. **Same job elsewhere.** none seen in unit.
7. **Unused/one-caller.** `lex_unit_reusing` is `pub`; grepped below. `LexError::outside_lexical_profile` `pub(crate)` used in `literals/quoted_bytes.rs:35,:95`.
8. **Stated limits.** `OUTSIDE_LEXICAL_PROFILE_MESSAGE` :228; `#![forbid(unsafe_code)]` :5.
9. **Fail-open.** none seen: unknown bytes error. `.unwrap()` on `chars().next()` :168 relies on valid UTF-8 (stated :83-84).

### literals/numbers.rs (91 lines, read in full)

1. **Purpose.** Finds the extent of a numeric literal (radix prefix, digits, fraction, exponent, type suffix) without deciding its value or type.
2. **Shape.** `ByteScanner::scan_number` :9; `scan_digits` :72.
3. **How it decides.** Radix from `0b/0o/0x` (:10-15). A `.` after decimal digits is a fraction only when not followed by `.` or an identifier start (:22-28). Exponent only for decimal (:34). Any following identifier characters become one suffix (:45-51). Kind `Float` if fraction or exponent else `Integer`, with flags `empty_digits`, `empty_exponent`, `has_suffix` (:53-65): flags are recorded, not rejected here.
4. **Cases by shape.** none.
5. **Fail-open.** none seen at this stage: `empty_digits`/`empty_exponent` are passed on as flags. Whether a later stage rejects them is not traced (suspected: parser/typing).

### literals/quoted_bytes.rs (109 lines, read in full)

1. **Purpose.** Validates string-literal escapes during scanning and decodes them to bytes.
2. **Shape.** `decode_string_literal` :8; `ByteScanner::scan_quoted` :24; `read_escape` :55.
3. **How it decides.** Escapes accepted: `\"`, `\\`, `\n`, `\r`, `\t`, `\0`, `\xHH` (:63-93). `\u` is "outside the lexical profile" (:94-99); any other escape is "unsupported escape sequence" (:100-106). A raw `\r` or `\n` inside a string refuses (:34-39).
4. **Cases by shape.** Fixed escape list above.
5. **Same job elsewhere.** `decode_string_literal` re-runs `read_escape` on the already scanned text and uses `.expect("valid string token contains a checked escape")` :16.
6. **Fail-open.** none seen; the `expect` is a panic path only reachable if the scan and the decode disagree.

### tests/byte_inputs.rs (265), tests/lexing.rs (322), tests/source_batches.rs (96), tests/source_coverage.rs (65), tests/token_boundaries.rs (91): read in full, one group entry

These five files are the lexer's integration tests (same kind of thing: each includes the shared `tests/support/lexing.rs` through a `#[path = "../../../../../../../tests/support/lexing.rs"] mod frontend_fixture` at the file end: byte_inputs.rs:264, lexing.rs:321, source_coverage.rs:64, token_boundaries.rs:90; that support file is not in this unit). `source_batches.rs` calls `source_lexer::lex` directly.
- `byte_inputs.rs`: `every_ascii_byte_alone_is_one_token_or_is_refused_at_that_byte` :55 (the single-byte punctuation set is written out as a literal 28-character string :49, deliberately not read from `PunctuationKind`; refused bytes must be controls except tab/LF/CR, `"`, `?`, backtick, DEL :110-114); `a_byte_order_mark_and_a_nul_are_refused_at_their_own_bytes` :121; `a_line_comment_before_crlf_keeps_the_carriage_return` :137; `random_texts_are_covered_exactly_or_refused_at_whole_characters` :202 (60,000 inputs from a fixed-seed xorshift :161-174, up to 24 characters; checks exact tiling and per-token isolated re-lex; requires more than 5 percent accepted and refused :257-258).
- `lexing.rs`: 18 tests including non-ASCII identifiers rejected with exact spans (:38-43), the closed whitespace set (:64-82), nested block comments (:85), string escape forms (:147), all 256 `\xHH` escapes (:167), `\u` and raw strings rejected (:185-192), malformed hex messages (`invalid hex escape digit`, `unterminated hex escape` :216-227), numeric literal metadata (`123abc`, `1e`, `0x` give suffix / empty exponent / empty digits flags :281-304). `tokenizes_retired_invariant_word_as_identifier` :20 pins that `invariant` is an identifier.
- `source_batches.rs`: lexing keeps source `Arc` backing, path buffer pointers, origin and scope (:9-58); first-failure location `first.omg:2:3: unterminated block comment` (:75); source order preserved when a later failure finishes first, 50,000-token first file (:79-96).
- `source_coverage.rs`: line comments leave the newline as the next token; spans reconstruct the file; each token lexes on its own (:30-59).
- `token_boundaries.rs`: longest-match punctuation list (`"+%"`... :7-21), no `>>` token (two touching `>`, :28-49 citing `wiki/spec/language/expressions.md`), keyword prefixes remain identifiers (:52), nested comment UTF-8 spans (:65-85).
Fail-open: none seen. Pinned results are message strings and byte spans (display-string compares of `error.message`).

## Area 02: `pipeline/01_tokens-to-syntax-trees/` (paths relative to `source-rust/subsystems/compiler/source-to-psi/pipeline/01_tokens-to-syntax-trees/`)

### assembly/parse.rs (228 lines, read in full)

1. **Purpose.** Parses an `asm requires/ensures/clobbers { ... }` block (labels, per-label contracts, instructions with operands) into an `AuthoredAssembly` statement. It does not check instruction validity; the header says that happens in declaration preparation.
2. **Shape.** `parse_asm_block_statement_handles` :16; `instruction_end` :117; `assembly_label_starts` :122; `ParsedAsmContract` :131; `parse_asm_contract` :137; `parse_asm_mnemonic` :211.
3. **In/out.** Token `Input` cursor in; pushes `AuthoredAssembly`/`AssemblyInstruction`/`AuthoredAssemblyBlock` into `SyntaxTrees`; returns a one-statement `HandleSpan`.
4. **How it decides.** Contextual words matched by spelling: `asm`, `requires`, `ensures`, `clobbers`, `none` (:20,:29,:145,:166,:180). A mnemonic is an identifier optionally followed by `.condition`, joined as `format!("{}.{}")` (:216-221). A transfer is a mnemonic whose text is exactly `"jmp"` or `"b"` (:52); then a non-bare, non-empty operand parses as a transition target (:56-59), else a comma list of expressions. A label starts when an identifier is followed by `:` or `requires`/`ensures` (:122-128).
5. **Cases by shape.** `"jmp" | "b"` are the only transfer mnemonics (:52): x86 `jmp` and arm64 `b`; other branch mnemonics parse as ordinary operands. `clobbers` once only (:174); empty clobber list must be `clobbers none` (:196-199).
6. **Same job elsewhere.** `parse_transition_block_target_handle` shared with transitions (:5,:57).
7. **One-caller.** `parse_asm_block_statement_handles` grepped in `01_tokens-to-syntax-trees` (see counts).
8. **Stated limits.** header :2 "Instruction validity and target-specific effects are checked during declaration preparation."
9. **Fail-open.** none seen at parse level; instruction validity deferred by design (stated :2).

### benchmarks/parser_timing.rs (96 lines) and benchmarks/streaming.rs (70 lines), read in full

Two benchmark binaries printing `omega-timed-work/1` JSON. `parser_timing.rs`: usage `parser_timing <repeats> <source>...` :24; pre-lexes inputs, hashes tokens via `format!("{:?}", token.kind)` and a `snapshot_json()` of the parsed syntax (:53,:70); times `test_support::parsing::parse` :83. `streaming.rs`: times `SourceInput::append` over `SourceSyntax` (:45-62), fingerprints `format!("{parsed:?}")` :52 and `snapshot_json` :48; uses `.expect("benchmark source")` :32. No decisions; no fail-open.

### contracts/bindings.rs (61 lines, read in full)

1. **Purpose.** Parses the `via ForeignBinding::Case(...)` realization of an external leaf into `ExternalBinding`.
2. **Shape.** `parse_external_provider_binding` :5; `parse_provider_binding_case` :20.
3. **How it decides.** First identifier must be exactly `"ForeignBinding"` (:10). Case by string match (:24): `Syscall(n)`, `CompilerIntrinsic`, `TableFunction(field)`, `VtableField(field)`; `VtableSlot` and `DllImport` give "retired" errors (:31-40); anything else lists the allowed cases (:53-59).
4. **Cases by shape.** Four accepted cases (above).
5. **Same job elsewhere.** none seen.
6. **Fail-open.** none seen.

### contracts/clauses.rs (639 lines, read in full)

1. **Purpose.** Parses the clause list between a machine header and its body or `;`: `requires machine`, `terminates [by ...]`, `reaches`, `invokes`, `suspends`, `blocks`, `stack N bytes`, `crashes`, `requires`/`ensures` (including named and outcome-specific groups), `-> ReturnType`.
2. **Shape.** `MachineClauses` :17 (fields incl. `declares_no_crash_route` :38); `parse_machine_clauses` :48 (entry); `machine_contract_fact_boundary` :377; `outcome_case_path_followed_by_arrow` :390; `reject_ambiguous_outcome_specific_ensures` :405; `parse_outcome_specific_ensures_group` :460; `machine_reaches_clause_boundary` :514; `parse_crash_header` :524; `parse_ranked_subjects` :537.
3. **In/out.** Token cursor to `MachineClauses`; contracts are published through `contracts::effects::publish_capability_contracts` :364.
4. **How it decides.** Every clause is keyed on the spelling of a contextual identifier (`terminates`, `by`, `decreases`, `increases`, `effects`, `reaches`, `invokes`, `suspends`, `blocks`, `crashes`, `none`, `boundary`, `requires`, `ensures`, `stack`, `bytes`, `in`). Retired forms refuse with a message naming "decision 23" (:102, :113, :121). An `ensures` is outcome-specific when a name path of 2+ members is followed by `->` (:390-403). Duplicate `suspends`/`blocks`/`stack` refuse (:154,:164,:174). `crashes none` after a cause or a cause after `none` refuses (:209-213). Duplicate outcome-group names and public selectors are compared by `String` equality (:255-261, :266-274, :304-312). `stack` demand must be positive (:181-185).
5. **Cases by shape.** The `expected_one_of_here` list (:332-345) is the closed clause list. `reject_ambiguous_outcome_specific_ensures` scans the guard tokens against a literal spelling list of twelve words (:420-431) to decide it is not an outcome group; it distinguishes `==` guards and `(`/`{` shapes by message (:436-454).
6. **Same job elsewhere.** `contracts/signatures.rs` and `contracts/facts.rs` (read below) share `parse_proof_facts_until`.
7. **One-caller.** `parse_machine_clauses` counted at end.
8. **Stated limits.** Retired clauses: block-form `terminates { decreases }`, `decreases`, `increases`, `effects`, trailing `boundary` (:100-117, :127-131, :235-239).
9. **Fail-open.** `reject_ambiguous_outcome_specific_ensures` returns `Ok(())` (accepts) when no top-level arrow is found, when the arrow is not followed by `{`, or when the guard contains a `;` or any of the twelve listed words (:406-411, :413-415, :416-435). What reaches it: an `ensures` whose guard text contains those words is parsed as an ordinary ensures proposition. Open (the comment gives no reason; the effect is parse routing, not acceptance of a bad program).

### contracts/conformance.rs (130 lines, read in full)

1. **Purpose.** Parses a machine's `satisfies Trait<args>::requirement [as alias] [via ...]` list and a trait application with lifetime and static arguments.
2. **Shape.** `parse_satisfies_traits` :11; `parse_conformance_trait_application` :89.
3. **How it decides.** A bare `satisfies Trait` (no `::member`) refuses as retired (:29-35). `via` is a `ForeignBinding` binding when the next identifier spelling equals `"ForeignBinding"` (:53-55), otherwise `via` takes an expression (:62-63). Lifetime arguments must precede type arguments (:100-105).
4. **Cases by shape.** One requirement per clause, list separated by commas (:80).
5. **Same job elsewhere.** `contracts/bindings.rs:10` also tests for `"ForeignBinding"` by spelling (same string compared twice: here :55 and there :10).
6. **Fail-open.** none seen.

### contracts/effects.rs (156 lines, read in full)

1. **Purpose.** Shared parsing helpers for operational clauses: `suspends`/`blocks` bounds, crash causes, `reaches` service rows, `crashes none`.
2. **Shape.** `operational_bound` :3; `parse_crash_cause` :18; `reject_retired_operational_reach` :33; `parse_effect_reference` :48; `take_service_reach_row_member` :60; `take_operational_reaches_clause` :79; `publish_capability_contracts` :126; `finish_crashes_none` :142.
3. **How it decides.** A bare `suspends`/`blocks` means `true`; `true`/`false` keywords state it (:7-13). Crash cause is exactly `"Trap"` or `"Abort"` (:24-25). `reaches` rejects the spellings `Suspend`, `Block`, `thread_block`, `sync_wait` with a replacement message (:37-42). `reaches <=` is an installation bound, declared once and not combined (:90-102); `reaches none` must end at a clause boundary (:103-111).
4. **Cases by shape.** Closed crash cause set of two.
5. **Same job elsewhere.** Called from both `clauses.rs` (:134,:201) and `signatures.rs` (:92,:138).
6. **Stated limits.** "`crashes none` ... publishes no crash contract, which is the empty cause set an omitted clause denotes on a published surface today" (:137-141): a `crashes none` header and an omitted clause produce the same contract rows; only `declares_no_crash_route` distinguishes them.
7. **Fail-open.** none seen.

### contracts/facts.rs (321 lines, read in full)

1. **Purpose.** Parses a list of proof facts (`requires`/`ensures` bodies, `crashes Cause` guards): plain expressions, `x in Domain` memberships (with `&`/`|` chains), `x in a..b` range facts, and `value predicate` shorthand.
2. **Shape.** `parse_proof_facts_until` :41; `..._with_machine_semicolon` :49; `parse_required_proof_facts_until` :65; `parse_required_logical_proof_facts_until` :81; `parse_fact_list` :89; `range_membership_expression` :286; `copy_item_path_to_expression_path` :18.
3. **In/out.** Appends `ProofFact::{Expression, Membership}` into `syntax_trees.items`, records source span per fact (:259-264).
4. **How it decides.** After an expression, `in` followed by a name-like token is a domain membership; otherwise an `a..b` or `a..=b` range (:114-232). `&` or `|` between domains build `And`/`Or` binary expressions with `ArithmeticDomain::Exact` (:141-178). With a `|` chain, any indexed domain argument refuses (:181-186). A bare identifier after the value becomes a call `value.predicate()` (:234-248). A trailing `;` ends a machine's list when `machine_final_semicolon` and the next token starts a following item (`starts_following_item`) or the stream ends (:266-274).
5. **Cases by shape.** Separators `;`, `,` or terminator (:266-280).
6. **Same job elsewhere.** `contracts/clauses.rs:48` and `signatures.rs:43` call into this.
7. **Fail-open.** none seen at parse level.

### contracts/signatures.rs (251 lines, read in full)

1. **Purpose.** Parses the clauses of a bodyless signature (trait requirement): `requires machine`, `reaches`, `invokes`, `suspends`, `blocks`, `crashes`, `requires`/`ensures`, bare `terminates`.
2. **Shape.** `signature_clause_boundary` :9; `SignatureClauses` :25 (adds `writes_crash_clause` :40: "A boundary requirement must"); `parse_signature_clauses` :43.
3. **How it decides.** A clause word (`terminates`, `reaches`, `effects`, `invokes`, `suspends`, `blocks`, `crashes`) ends a fact list unless `clause_word_is_an_operand()` (:14-21). `requires proposition` ends the clause list and returns (:74-80). `terminates by` refuses on a requirement (:200-206, cites "decision 23").
4. **Cases by shape.** closed list at :212-223.
5. **Same job elsewhere.** The `reaches`, `invokes`, `suspends`, `blocks`, `crashes`, `requires/ensures` arms (:91-196) repeat `contracts/clauses.rs:133-323` nearly line for line, differing in the fact terminator function (`signature_clause_boundary` vs `machine_contract_fact_boundary`) and that clauses.rs also handles outcome-specific `ensures`, `stack`, `->`, `terminates by` (read both).
6. **Fail-open.** none seen.

## declarations/ (paths below are `declarations/<file>`)

### declarations/capabilities.rs (156 lines, read in full)

1. **Purpose.** Parses a `capability Name { field: Type; state ... }` item: fields and `state` signatures with `requires`/`ensures` contracts.
2. **Shape.** `parse_capability_definition` :13; `parse_capability_state` :62; `parse_capability_contract` :100; `capability_contract_terminator` :147.
3. **How it decides.** Members: `state` keyword starts a state, otherwise `name: Type;` field (:31-55). A contextual `entry` not followed by `:` refuses as the retired host scaffold (:22-29). Contract keywords: `requires`, `ensures` only; `boundary` refuses as retired (:138-142).
4. **Cases by shape.** none.
5. **Same job elsewhere.** The `boundary` retirement message is word-for-word the same as `contracts/clauses.rs:236-238` (two copies).
6. **Fail-open.** none seen.

### declarations/conformances.rs (160 lines, read in full)

1. **Purpose.** Parses a named implementation `Name satisfies Trait<Args> { members }`: inline `machine` members and selection rows `Trait::member = Existing::machine;`.
2. **Shape.** `parse_conformance` :13; `parse_conformance_members` :52; `normalize_conformance_machine_entry` :140.
3. **How it decides.** `Name<...>: ...` (a colon) refuses with "data does not satisfy traits" (:20-24). A member whose head (after optional `public` and `boundary`) is not `machine` must be a selection row (:60-98). Inline machine refuses: installation-bound reach (:103-107), operator spelling (:108-113), attached data (:114-118), satisfies clause without an exact requirement (:119-127). The first state of an inline machine is renamed to the requirement name when its name is the generated `"entry"` (:145-146), or a new empty state is inserted (:151-159).
4. **Cases by shape.** Member kinds: Selection and Machine only (:101).
5. **Fail-open.** none seen.

### declarations/constants.rs (136 lines, read in full)

1. **Purpose.** Parses `const [Scope::]Name[: Type] = expr;` and, when the type is omitted, writes the type the initializer shows.
2. **Shape.** `parse_const_definition` :13; `initializer_shown_type` :68; `ShownType {Named, Cast, Anonymous, NeedsTyping}` :96; `shown_type` :103.
3. **How it decides.** Shown type: integer or float literal with a suffix `landing()` gives `Named(landed_type.to_string())` (a type spelled as text, :106,:113), `true`/`false` give `"bool"` (:116), an `as` cast without a semantic domain reuses its target (:119-120), unary follows its operand (:122), binary is `Anonymous` only when both sides are, else `NeedsTyping` (:125-133), everything else `NeedsTyping` (:134). The parser then inserts `TypeReferenceNode::Named(Identifier::new(name, span))` (:91-93).
4. **Cases by shape.** Fixed list above; others refuse: "not supported yet: a constant without a type annotation whose initializer is not a suffixed literal, a cast or `true`/`false`" (:86).
5. **Same job elsewhere.** none seen. The comment cites `wiki/spec/language/numeric_values.md#literal-landing-and-destinations` (:58).
6. **Stated limits.** ":86 not supported yet: a constant without a type annotation ...".
7. **Fail-open.** none seen.

### declarations/data.rs (554 lines, read in full)

1. **Purpose.** Parses `data Name<...> [props] [requires ...] { fields / case members }`, type aliases (`data A = T;`), callable aliases (`= machine (...) -> T;`), quotient data (`= Carrier % Relation [requires ...]`), and `boundary data Name;`.
2. **Shape.** `parse_data_definition` :22; `parse_boundary_data_definition` :197; `take_retired_identity` :241; `parse_data_members` :256; `validate_data_identity_modes` :273; `validate_identity_scope` :319; `take_optional_identity` :357; `parse_data_member` :368; `parse_case_member` :457; `parse_case_payload_fields` :497.
3. **How it decides.** The data name `Slice` is refused as reserved (:27-31, :202-206). After the header, `=` selects alias or quotient: a type alias if the next token is `;` (:73), else quotient with `%` (:101). Stable identity numbers `#N` are all-or-nothing per scope and unique, including retired ones, by `HashSet<u64>` (:325-353); mixed fields-and-cases with retired entries refuse (:299-304). `retired` is contextual: only `retired #` starts a retired identity (:244-249).
4. **Cases by shape.** Quotient equivalence selection must be lifetime-free: "does not yet retain lifetime arguments" (:114-118). A quotient cannot be linear (:96-100). A field default value refuses as retired, "every field is zero-initialized (ZII)" (:425-433). Case blocks and `case #N Name` refuse (:403-410).
5. **Same job elsewhere.** Identity validation re-run for case payloads at :547-552 with the same helper.
6. **Stated limits.** "does not yet retain lifetime arguments" :116.
7. **Fail-open.** none seen.

### declarations/domains.rs (485 lines, read in full)

1. **Purpose.** Parses `domain Carrier::Name[::Refinement] [satisfies ProgressProfile] [dropped by M] [requires facts] [access Policy | established by routes];` and domain aliases `= A & B;`.
2. **Shape.** `parse_domain_definition` :18; `parse_domain_classification` :198; `parse_domain_requires` :222; `domain_requires_terminator` :242; `parse_domain_alias` :264; `type_reference_target_label` :287; `parse_domain_establishment` :321; `at_legacy_authored_route` :410; `parse_authored_route` :429; `parse_cleanup_selection` :443 (pub(crate), also called from `data.rs:43`); `parse_static_selection` :462.
3. **How it decides.** The domain's name is built as a string: `format!("{target_label}::{segments joined by ::}")` when it has no type parameters (:103-114), else the joined segments alone (:116-123). `target_label` comes from `type_reference_target_label`, which renders `[T]`, `[T; N]`, a named type's text, and `"?"` for any other carrier (:287-304). A refinement chain adds a membership fact `self in Parent` (:74-102). Classification must be exactly `ProgressProfile` (:206-213). A `{` body after the domain head refuses in four different ways by message (:359-382).
4. **Cases by shape.** `Alias` form (`=`) cannot be a refinement chain (:133-137). `access` and `established by` are mutually exclusive (:338-340, :403-405). Explicit subject binders `(...)` retired (:67-71).
5. **Same job elsewhere.** `domain_requires_terminator` :242-262 is a hand-written list of 18 item-starting words and a call to `named_implementation_prefix`/`starts_machine_declaration`; `declarations/items.rs:26-63` `starts_following_item` is the other list of item-start words (the two lists differ: this one includes `export`, `module`, `package`, `const`, `domain`, `operator`, `boundary`, `trait`).
6. **Stated limits.** ":309-311 The retired `established by` list is still read until its declarations migrate (`DOMAIN-ACCESS-AUTHORITY`); a domain writes one or the other."
7. **Fail-open.** `type_reference_target_label` returns `"?"` for other carriers (:302): two domains over different carriers of those kinds would get the same name prefix. Which carriers reach it is not traced (open; bracket path :30-35 can only produce `Slice`/`FixedArray` types in practice, inferred).

### declarations/imports.rs (139 lines, read in full)

1. **Purpose.** Parses `use` items: namespace imports (`use a::b;`) and file imports (`use {root}/dir/name.omg as alias;`), and checks duplicate file aliases.
2. **Shape.** `parse_use_item` :9; `file_component` :84; `check_file_aliases` :107.
3. **How it decides.** An identifier followed by a path member or `;` is a namespace import (:14-24). A file component is a run of adjacent tokens whose characters are alphanumeric, `_` or `-` (:89-100). The implicit alias exists only when the last component is a single identifier token (:40-44); otherwise an explicit `as` is required (:65). An alias that is a builtin type spelling refuses (`is_builtin_type_spelling` :67). Duplicate aliases refuse by sorted `str` compare (:124-136).
4. **Cases by shape.** Namespace import vs file import.
5. **Stated limits.** ":13 Existing library sources still use namespace imports during migration."
6. **Fail-open.** none seen.

### declarations/items.rs (430 lines, read in full)

1. **Purpose.** The top-level item dispatcher: decides which declaration kind starts at the cursor and parses it, including `public`, `boundary` and target-prefixed machines; also provides lookahead predicates to find where an item ends.
2. **Shape.** `ParsedDeclaration {Item, Mathematical}` :21; `starts_following_item` :26; `starts_boundary_item` :65; `named_implementation_prefix` :78; `target_machine_prefix` :116; `starts_machine_declaration` :133; `parse_item` :142; `parse_boundary_item` :186; `parse_declared_item` :230.
3. **How it decides.** Item kind keyed on keyword or contextual-word spelling at the cursor: `package`, `use`, `data`, `domain`, `const`, `proposition`, `abi` (retired), `machine`, `capability`, `target` (retired), `invariant` (retired), `library` (retired), `measure`, `operator`, `host`/`provides`/`platform` (retired), `trait`, `Name machine` target prefix, `Name satisfies` / `Name<...> satisfies` / `Name:`/`Name<` as a conformance (:386-394). `starts_following_item` uses a speculative parse for `operator`/`domain`: it appends a checkpoint, tries a proof-fact parse and `parse_declared_item`, then `rewind_appended` (:54-60). `public` is retained only for Conformance, Const, Data, TypeAlias, Domain, Machine, Operator, Proposition, Trait and Mathematical; any other kind refuses "refusing to compile a silently private API" (:149-165).
4. **Cases by shape.** `named_implementation_prefix` scans tokens with angle/paren depth counters to find `Name<...> satisfies` (:88-113). The unmatched-item message lists 20 spellings (:408-429).
5. **Same job elsewhere.** Item-start word lists: `starts_following_item` :26-63, `domains.rs:242-262`, `contracts/clauses.rs:377-388`; grepped `starts_following_item` below.
6. **Stated limits.** ":162 `public` is not yet retained for this declaration kind".
7. **Fail-open.** `starts_following_item` returns `false` for an unrecognized word (:62 via `starts_boundary_item`), which decides where a proof-fact list ends (`facts.rs:271`); a mis-detected end could swallow or leave a following item. Open (parse routing).

### declarations/machines.rs (175 lines, read in full)

1. **Purpose.** Parses a machine declaration after the `machine` keyword: operator spelling, path, generics, parameters, return type, contracts, `satisfies`, clauses and body.
2. **Shape.** `parse_machine` :18; `MachinePath` :147; `split_machine_path` :153.
3. **How it decides.** An operator spelling is present when the first token is punctuation other than `;` (:22-24). A dotted path `A::b` makes the machine name the full path and `attached_data` the path minus the last member, with the last member as the entry state name (:167-173). `stack N bytes` is accepted only for a bodyless machine with a `satisfies ... via` binding (:85-106). A `via` without a requirement refuses (:52-57); an operator spelling with `satisfies` refuses (:46-51).
4. **Cases by shape.** The entry state is named `entry` (generated) unless the path supplies a name (:77).
5. **Same job elsewhere.** Operator machines parse via `declarations/operators.rs` (read below).
6. **Fail-open.** none seen; `.expect(...)` on empty path :161 and on `same_stack_clause_span` :88.

### declarations/mathematical_bindings.rs (227 lines, read in full)

1. **Purpose.** Parses a mathematical `let name<binders>(params): Type = term;` (and bodyless `boundary let ...;`) definition and its dependent arrow types.
2. **Shape.** `parse_let_definition` :14; `parse_mathematical_parameter` :99; `parse_mathematical_type` :117 (depth-guarded `deepen()` :122); `parse_arrow_domain` :150; `fold_type_applications` :194.
3. **How it decides.** Lifetime parameters and conformance binders refuse (:25-36). A top-level `let` with `;` instead of `= term` refuses and names `boundary let` (:77-82); `boundary` makes the body `Assumption` (:62-74). A named binder `(x: T)` must be followed by `->` (:144-146) and holds one domain (no comma list, :178-183).
4. **Cases by shape.** `Arrow`, `Ordinary`, `Application` type nodes.
5. **Fail-open.** none seen. A `boundary let` is an assumption by construction (assumed binding family; see AGENTS.md rule 2), parse level only.

### declarations/measures.rs (103 lines, read in full)

Parses `measure Name(param) -> T { expr }` and `measure Name lexicographic { e1, e2 }`. `parse_measure_definition` :10 records `token_count` (tokens consumed, :22,:45). A single parameter handle is kept; else invalid (:38-42), so a measure with 0 or 2+ parameters parses with no parameter. Contextual word `lexicographic` (:19). Fail-open: a multi-parameter measure silently gets `StateParameterHandle::invalid()` (:41); whether a later stage refuses it is not traced (open).

### declarations/namespaces.rs (52 lines, read in full)

Parses `package a.b::c [;]` into a path (`parse_dot_or_colon_path` :19 accepts `.` and `::` mixed) and exports `take_optional_semicolon` :44 (used by `capabilities.rs:2` and here). No decisions beyond the separators.

### declarations/operators.rs (186 lines, read in full)

1. **Purpose.** Parses `operator [symbol] Name<...>(params) -> T requires/ensures/crashes ...;` and operator spellings (shared with machines and trait requirements).
2. **Shape.** `parse_operator_definition` :11; `parse_operator_spelling` :66; `parse_operator_contract` :106; `parse_operator_fact_contract` :155; `operator_contract_terminator` :179.
3. **How it decides.** The spelling is the concatenation of consecutive punctuation tokens until `OperatorSpelling::from_symbol(&symbol)` matches (:69-80). A symbol ending in `%`, `|` or `!` whose base is one of `+ - * / %` refuses as a compiler-owned policy operator (:84-92). Unknown spelling lists `OperatorSpelling::ALL` (:93-103). `crashes none` refuses on operators (:135-139).
4. **Cases by shape.** `requires`, `ensures`, `crashes Cause`.
5. **Same job elsewhere.** `parse_operator_spelling` called from `machines.rs:25`, `traits.rs:141`, here (grepped counts below).
6. **Fail-open.** none seen.

### declarations/propositions.rs (73 lines, read in full)

Parses `proposition Name<binders>(params)` with one of three bodies: `;` (Primitive), `evidence Type;` (Witness), `= fact;` (Transparent); `{ Evidence; }` is retired (:38-41). `parse_proposition_definition` :11. Contextual word `evidence`. No fail-open.

### declarations/traits.rs (597 lines, read in full)

1. **Purpose.** Parses `trait`/`boundary trait` definitions: parents (`: A + B`), transparent refinements (`= Base { machine Req ... }`), proposition parameter signatures, `requires Trait;` rows and machine requirements with optional default bodies and native callback parameters.
2. **Shape.** `parse_trait_definition` :20; `parse_refinement_clause` :218; `refinement_reaches_clause_boundary` :375; `parse_proposition_parameter_contracts` :383; `parse_trait_parents` :455; `parse_trait_machine_signature` :482; `parse_boundary_parameter_telescope` :513; `parse_trait_machine_name` :578; `parse_trait_requirement` :590.
3. **How it decides.** In a boundary trait every machine requirement must write `reaches` (:158-164) and a `crashes` clause (:168-174; comment cites `effects.md` :165-167); `reaches <= Bound` only on a bodyless boundary requirement (:175-179). `native callback ... from ...` only on a bodyless boundary requirement (:153-157). A refinement clause qualifier must equal the base trait's name by string compare (:248-252), with at most one qualifier segment (:253). Proposition parameters match declarations by name (`as_str ==` :402). Duplicate native callback names by string (:540-544).
4. **Cases by shape.** A refinement is `*` (all) or one named requirement; `ensures` needs a named requirement (:280-284); only `ensures`, `reaches`, `suspends`, `blocks`, `terminates` clauses.
5. **Same job elsewhere.** `parse_refinement_clause` repeats `suspends`/`blocks`/`reaches` parsing of `contracts/signatures.rs:91-129` and `clauses.rs:133-171` (third copy, with `authored_*` flags in place of keyword-span checks).
6. **Stated limits.** none (many retired forms: `invariant`, `default`, `operator` introducer).
7. **Fail-open.** none seen.

## errors/ and expressions/ (paths below are `errors/<file>`, `expressions/<file>`)

### errors/expected.rs (56 lines) and errors/parse_error.rs (34 lines), read in full

`expected.rs`: builds "expected X, found Y" `ParseError`s (`expected_at` :5, `unexpected_eof` :23, `expected` :28, `expected_one_of_here` :37); `describe_token` :41 renders a token kind as text (`identifier \`x\``, `keyword \`..\``, `punctuation \`..\``, `string literal`, whitespace, comments). `parse_error.rs`: `ParseError {message, source_span, location}` :4, `at_source_span` :11, `into_diagnostic` :22 (prefixes `path:line:col:` when `location` is set). No decisions. Every parse error is a message string; no error codes (display text is the only identity, grepped below for tests that compare it).

### expressions/context.rs (84 lines, read in full)

`ExpressionContext` :2 has 10 variants (`Default`, `NoStructLiteral`, `TypeExpressionNoStructLiteral`, `NoStructLiteralOrMembership`, `ProofFact`, `GroupedProofFact`, `LogicalProofFact`, `LogicalTypeExpressionNoStructLiteral`, `LogicalValue`, `GroupedLogicalProofFact`) and predicate methods `allows_struct_literal` :16, `allows_membership` :26, `allows_type_expression` :33, `allows_logical_subject` :55, with transitions `boolean` :45, `value` :65, `grouped` :73. These tables decide how an expression is parsed in each position (struct literals, `in` membership, type expressions). Fail-open: `boolean()` returns `self` for any context not listed (:51 wildcard), so new variants default to unchanged (parse routing only).

### expressions/membership.rs (73 lines, read in full)

`extend_membership` :12 parses `value in Domain [& Other | Third]` after an arithmetic operand when the context allows membership and the next token is the contextual word `in` (:18). Builds `ExpressionNode::Membership` and `Binary(And/Or)` with `ArithmeticDomain::Exact` (:60-68). Note both `&` and `|` fold left to right with no precedence between them (:34-69). `facts.rs:114-179` implements the same chain separately for proof facts (same job elsewhere, two copies).

### expressions/operators.rs (401 lines, read in full)

1. **Purpose.** Operator tables and unary-prefix handling for expression parsing.
2. **Shape.** `LOGICAL_OPERATORS` :17 (8 rows, precedence 0-3); `ARITHMETIC_OPERATORS` :38 (26 rows, precedence 0-5, including 18 policy-suffixed tokens such as `+%`, `+|`, `+!`); `punctuation_policy` :103; `RIGHT_SHIFT_PRECEDENCE = 3` :129; `take_right_shift_policy` :135; `insert_binary_expression` :159; `UnaryPrefix` :180; `take_unary_prefix` :196; `apply_unary_prefix` :290; `punctuation_label` :365.
3. **How it decides.** The policy of a suffixed operator is read back from the token kind (`punctuation_policy` :103-124). There is no `>>` token: a right shift is two touching `>` and its policy suffix is `%`, `|` or `!` immediately after the second `>` with no gap (`suffix.span.start == shift.span.end` :152). Prefix words `suspend` and `block` are call acknowledgements unless followed by `(` (:205-254); `suspend block` canonical order only (:236-240). `move` is a prefix only when followed by a name or `$` (:279-286).
4. **Cases by shape.** Unary prefixes: `@` borrow, `!`, `~`, `$`, `-`, `suspend`/`block`, `move`.
5. **Same job elsewhere.** `punctuation_label` :365-400 re-lists operator spellings as strings although `PunctuationKind::lexeme()` exists (used at `errors/expected.rs:50`).
6. **Fail-open.** (a) `UnaryPrefix::Move => Ok(expression)` (:361): `move x` yields the same syntax node as `x`; the parser does not record the `move`. Whether a later stage recovers the move from other information is not traced (open). (b) `Negate` of an integer literal calls `literal.negated()` and of a float builds `FloatSpelling::generated(format!("-{}", spelling))` (:320-323): a negated literal is a literal with its sign, not a `0 - x` expression; other operands become `0 - x` with `Exact` policy (:325-333). (c) `punctuation_label` returns `"?"` for other kinds (:399).

## expressions/parse.rs (1828 lines) and expressions/postfix.rs (821 lines): below

### expressions/parse.rs (1828 lines, read in full)

1. **Purpose.** The expression grammar, run as a loop over an explicit frame stack instead of recursion so deep expressions cost heap rather than host stack (header :1-17, citing `compiler_request.md`, "Omega source has no expression or type depth limit"). It parses logical and arithmetic precedence chains, membership, unary prefixes, postfix suffixes, primaries, struct and array literals, `match` expressions with patterns, and destructure fields.
2. **Shape.** Entries: `parse_expression_handle` :54, `..._without_struct_literals` :61, `..._or_membership` :68, `parse_proof_fact_expression_handle` :79 and `parse_logical_proof_fact_expression_handle` :86 (both call `proof_facts::parse`, a module outside this unit), `parse_const_integer_expression_handle` :93, `parse_expression_handle_in` :100, `run` :122, `run_expression` :156. Types: `Goal` :110, `Parsed` :117, `Descent` :184, `Step` :196, `ChainKind` {Logical, Arithmetic, StaticArgument} :205, `PendingOperator` :218, `Frame` (13 variants) :226, `Machine {frames, operators, prefixes}` :354. Methods: `descend` :361, `resume` :452, `chain_operand` :595, `suffixes` :719, `range_end` :810, `finish_index` :845, `begin_arguments` :873, `finish_arguments` :896, `is_equality_operand` :951, `primary` :987, `struct_literal_fields` :1304, `match_value` :1365, `match_arms` :1447, `match_after_fields` :1517, `match_after_pattern` :1559, `match_arm` :1581, `destructure_fields` :1676; free `push_destructure_field` :1787, `finish_array_literal` :1813.
3. **In/out.** Token `Input` cursor in; expression handles out, written into `syntax_trees.expressions`. Depth is tracked on the cursor (`input.depth()`, `deepen()`, `with_depth`) and restored by `Frame::RestoreDepth` (:229, :460-469).
4. **How it decides.** Operator tables from `operators.rs` (`LOGICAL_OPERATORS`, `ARITHMETIC_OPERATORS`) decide by punctuation kind and precedence number (:620-636); right shift is two touching `>` (:604-618). A second comparison in a logical chain refuses unless parenthesized (:654-668). A policy-suffixed operator in a constant position (`input.constant_position()`) refuses: "the policy operator ... is not available in a constant expression yet" (:688-697). Primary dispatch keyed on keyword/contextual-word spelling and token kind: `true`/`false`, `self`, `match`, `zero_value` (followed by `::<`), integer, float, `utf16` followed by a string, string, `(`, `[`, `spawn {`, path head (:1003-1300). A struct literal is chosen when the context allows it and `{` follows a path (:1263); its constructor name is a `String` built by joining path members with `::` (:1269-1274). A `match` arm pattern: `_` by `at_contextual("_")` (:1455), a subjectless arm is a condition (:1459), a bare binding vs a value pattern vs a destructure is decided by `patterns::bindings::bare_binding` and `starts_with identifier` (:1386-1413). Pattern bindings and outcome selectors are checked for duplicates by `str` equality (:1627-1643, :1742-1761). A destructure member named `case` is the case component by spelling (:1794).
5. **Cases by shape.** `self` is accepted as a name only in logical-subject contexts; elsewhere "implicit `self` is retired" (:1025-1046). `spawn { ... }` refuses as retired (:1192-1201). A match needs at least one arm (:1499-1501). `..` in a struct literal must be final with no operand (:1319-1322).
6. **Same job elsewhere.** The membership chain `extend_membership` (`expressions/membership.rs:12`) and the proof-fact membership chain (`contracts/facts.rs:114-179`) overlap; `is_equality_operand` here inspects the frame stack to decide whether `<` starts a generic type operand.
7. **Unused/one-caller.** `Goal`, `run`, `run_expression` are `pub(crate)`; `run_expression` is used from `postfix.rs:24` (read). Counts at end.
8. **Stated limits.** "no expression or type depth limit" (:4). `deepen()` returns `Result` (`token_input/cursor.rs:84`, not in this unit; grepped only the declaration, body not read), so a limit may exist in the cursor: suspected, not read.
9. **Fail-open.** none seen. `unreachable!` and `expect` calls guard invariants (:150, :164, :1441, :973).

### expressions/postfix.rs (821 lines, read in full)

1. **Purpose.** Postfix pieces of the grammar: call argument lanes (`f(a, b; evidence)`), `.member` suffixes, `as` casts and domain entries (`as T +D`, `as -D`, `=D`), ranges, call-expression construction, and static (`::<...>`) argument lists with checkpoint/rewind backtracking.
2. **Shape.** `parse_postfix_expression_handle` :19; `finish_argument_lanes` :33; `parse_member_suffix` :80; `parse_cast_suffix` :139; `at_cast_entry` :217; `parse_cast_entries` :230; `range_separator` :325; `build_call_expression_handle` :346; `try_parse_static_machine_arguments` :412; `parse_static_call_arguments` :436; `parse_static_name_selection` :472; `SelectionPosition` :480; `parse_name_selection` :486; `try_parse_static_argument` :560; `try_parse_static_value_argument` :593; `parse_static_value_alternatives` :605; `try_parse_static_symbol_application` :644; `parse_static_symbol_application` :656; `parse_static_type_argument` :716; `spell_test_group_facet` :742; `call_path_target_and_receiver` :798. Test module `#[path = "../tests/unit/postfix_transactions.rs"]` :819-821 (not in unit).
3. **How it decides.** A call is built by the callee node shape: `Name` becomes a static call with the last path member as target and the rest as receiver (:357-381), `Member` a method-style call (:382-391), anything else gets the placeholder target `"$callable"` (:395). Member suffixes followed by `(` or `<` refuse six retired receiver-style Build method spellings by string match: `accept_boundary`, `select_provider`, `exclude_service`, `select_representation`, `require_wire_compatibility`, `assume_binding_contract` (:96-116). `spell_test_group_facet` rewrites a call whose target spelling is `"group"` or `"service"` with static arguments and a receiver path ending in `"tests"` into a member projection on the owner (:746-795), citing `wiki/spec/build/testing.md`. A cast target label is built from the concatenated token spellings (`target_label_text` :159-162, `label_text` :288-291) as a generated identifier. Cast form: carrier cast may only add domains (`+`), domain-only casts may remove or keep (:247-251); `as T in D` is retired for value casts (:173-178); a domain whose name is an `ArithmeticDomain` name is an arithmetic domain, otherwise semantic (`ArithmeticDomain::from_name` :268, :181). Static arguments are tried as a type (starts with `[` or `@`, or followed by `[` or `in`), as a name selection, or as a value expression (:566-590); backtracking by `append_checkpoint`/`rewind_appended` (:426-433, :565, :574, :579, :597-601, :648-653).
4. **Cases by shape.** A static call list is accepted only if a `(` follows its closing `>` (:420-433). A `;` evidence lane takes identifiers only (:54).
5. **Same job elsewhere.** `parse_conformance_trait_application` (`contracts/conformance.rs:89`) parses a similar `<'a, T>` list with the same lifetime-before-type rule; `parse_static_symbol_application` (:656) is the third copy of that loop (inferred from reading both).
6. **Fail-open.** (a) `try_parse_static_machine_arguments` returns `Ok(None)` and rewinds when the list does not parse or no `(` follows (:420-433), so `f::<...>(...)` that fails to parse as static arguments falls back to other parses; the failure reason is dropped by `if let Ok(Some(..))` (:427). Open. (b) `parse_static_value_alternatives` returns `None` for a bare `Name` expression so names are tried first as name selections (:616-622): parse routing.

## Call-site counts used above (commands run with the Grep tool, count mode, over `source-rust/`; results are matches per file, not per symbol)

- `name_path\(`: 25 matches in 7 files, which include other functions whose names end in `name_path(`; in `driver/` only `compile_time_invocations/calls/arguments.rs` (4) and `calls/typed_call.rs` (1). So `calls/arguments.rs:513 name_path` has its own file's uses plus one in `typed_call.rs:109`.
- `pure_terminal_invocation_account`: 2 matches, both in `calls/execution.rs` (definition :348 and one call :41). It is `pub(crate)` and has no caller outside its file.
- `evaluate_demanded_values|validate_constant_evaluations`: 17 matches in 7 files: `compile_time_invocations.rs` 2, `constants.rs` 1, `constants/tests.rs` 9, `demanded_values.rs` 1, `domain_access_plans/tests.rs` 1, `fixed_layouts/tests.rs` 1, `prepare_typed_source.rs` 2. The production caller is `prepare_typed_source.rs` (lines :95 and :105, read); the other matches are re-exports and tests.
- `parse_asm_block_statement_handles|parse_machine_clauses|starts_following_item|parse_operator_spelling\(`: 15 matches in 9 files, among them `assembly/parse.rs` 1 (definition), `contracts/clauses.rs` 1 (definition), `contracts/facts.rs` 2, `declarations/items.rs` 2, `declarations/machines.rs` 3, `declarations/operators.rs` 1, `declarations/traits.rs` 1, `statements/parse.rs` 2, `tests/unit/definitions_and_contracts.rs` 2. `parse_operator_spelling` therefore has three production callers outside its own file (`machines.rs`, `traits.rs`, and `operators.rs` itself); `starts_following_item` is used at `facts.rs:271` and in `items.rs` itself.
- `package_handoff_declarations|RegisteredTests::discover|materialized_call|lex_unit_reusing|retain_evaluation_entries|invoke_recorded_in_targets|InvocationSelection::new|refresh_evaluated_requests`: 26 matches in 17 files. Outside `source-to-psi` the matching files are `build/planning/registered_tests.rs` (1) and `build/planning/source.rs` (1): these are the external consumers of `RegisteredTests::discover` and `package_handoff_declarations` (inferred from the file names; the match line was not opened). `psi-to-native/.../image-elf/src/file_assembly.rs` (3) matched a different alternate (`materialized_call` or similar name) and is unrelated to this unit (not opened).
- `fn deepen`: one definition at `01_tokens-to-syntax-trees/token_input/cursor.rs:84` (not in this unit; body not read).

## Coverage

All 78 ranges of `units/unit_051.tsv` were read in full, line 1 to the last line, in order: 78 files, 21,937 lines (`awk -F'\t' '{s+=$4; n++} END{print n, s}'` over the unit file gave 78 and 21937). Lines skimmed: 0. Lines not read: none. Files read through the Read tool in 1 or 2 chunks (`expressions/parse.rs` as lines 1-950 then 950-1828).
Not read and referenced only by name: `domain_access_plans/access_plan_construction.omg`, `selected_access_plan_family.omg`, `tests/support/lexing.rs`, `samples/cli/facts/fixed_layout_storage/local/main.omg`, `expressions/proof_facts.rs`, `expressions/primary.rs`, `token_input/cursor.rs`, `tests/unit/postfix_transactions.rs` (all outside this unit).

## Five things a maintainer most needs to know about this unit

1. **Compile-time value evaluation is one scheduler over four kinds.** `demanded_values.rs:150` evaluates layouts, access plans, const arguments/array extents and constant initializers in rounds ordered by what each reads (`demand_order.rs`); every kind implements `DemandedValues` (:44). A cycle refuses and names the values (:277). Results are re-justified afterwards by `validate_constant_evaluations` (`constants.rs:169`), which compares receipts to the final program.
2. **Deferral by declaration is how unrequested work is tolerated.** `frontier.rs` records unevaluated requests as deferred refusals on the declaration that holds them (:25, :40) and `require_no_recorded_work` (:84) refuses nine kinds of recorded work with "no product route yet"; `refresh_evaluated_requests` (:40) drops type-expression and array-extent rows that are no longer in the inventory (stated :37-39).
3. **Fixed shapes are everywhere in the layout and access-plan paths.** Schema arrays are padded to exactly 32 entries (`fixed_layouts/schema.rs:10-11`, `domain_access_plans/returned_plan.rs:427`), layout results must carry exactly 64 entries (`fixed_layouts/validation.rs:43`), only `At` placements validate (`validation.rs:73-80`), layout schemas are records only (`reflection.rs:20`), and a plan with field observations or boundary reach is refused "no product route yet" (`domain_access_plans.rs:260-265`).
4. **Parser decisions are keyed on spelling.** Contextual words (`requires`, `terminates`, `crashes`, `via`, `in`, `established`, ...) and a few exact strings (`"build"`/`"Build"` at `parse_source_batch/parsed_files.rs:230-238`, `"ForeignBinding"` at `contracts/bindings.rs:10` and `conformance.rs:55`, `"jmp"`/`"b"` at `assembly/parse.rs:52`, `"tests"`/`"group"`/`"service"` at `expressions/postfix.rs:746-770`, `"Slice"` at `declarations/data.rs:27`) decide what is parsed. Item-start word lists exist in three places (`declarations/items.rs:26-63`, `declarations/domains.rs:242-262`, `contracts/clauses.rs:377-388`) and the clause arms are repeated in three files (`contracts/clauses.rs`, `contracts/signatures.rs`, `declarations/traits.rs:218-373`).
5. **Retention and reuse caches are mutex-held lists compared by equality.** `SourceFrontends` (`resolve_and_type.rs:520`) and `AuthoredCompletions` (`resolve_and_type/authored_completion.rs:8`) keep `Arc<Mutex<Vec<..>>>` of prior results; reuse is decided by `select_retained_source` (`activation.rs:123`) and `same_completion_input` (`resolve_and_type.rs:489`), and every failed check falls back to fresh work (fail closed for reuse).

## What I am unsure about

- Whether `build_invocation` (not in this unit) rejects a value of the wrong primitive type that `calls/arguments.rs:269-273` lets through the host-input validator.
- What `evaluation_admission` does with `DeclarationCustody::Missing` (`calls/execution.rs:218`) and with a `None` source selection (`calls/source_selection.rs:27-31` returns `None`; `execution.rs:228-230` passes it on).
- Whether `constants.rs:240-247` (early `Ok(())`) can accept a constant with a recorded `evaluated_value` but no evaluation entry.
- Whether `deepen()` (`token_input/cursor.rs:84`) enforces a depth limit, which would contradict `expressions/parse.rs:4`.
- Whether any consumer other than the ones named reads `AccessPlanJudgment::Unjudged` (only the doc at `domain_access_plans.rs:23-25` was checked, not the consumers).
- Several counts above are per-file match counts, not per-symbol call-site counts; where I wrote "inferred" the match line was not opened.
