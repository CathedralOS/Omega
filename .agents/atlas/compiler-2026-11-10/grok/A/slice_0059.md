# slice_0059

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0449 | omega-rust/psi/compilation/validation/src/effect_inference.rs:1285 | CLOSED | Operational `expression_call` skips only exact `IntegerEmbed`; that call form carries no suspend/block/service work elsewhere either. |
| A0450 | omega-rust/psi/compilation/validation/src/effect_inference.rs:72 | REACHABLE | States whose bodies are only assembly statements never contribute calls to operational or service-reach collection. |
| A0451 | omega-rust/psi/compilation/validation/src/effect_inference.rs:2972-2982 | REACHABLE | An asm-intrinsic operational call whose assembly service fails `resolve` keeps default empty direct reach, understating inferred service reach. |
| A0452 | omega-rust/psi/compilation/validation/src/effect_inference.rs:3186 | REACHABLE | When the call target is not a parameter signature, trait signature, or indexed boundary machine, `direct_service_reach_for_call` returns empty reach. |
| A0453 | omega-rust/psi/compilation/validation/src/lib.rs:172-174 | UNDECIDED | Invalid value handles return without diagnostic; whether such handles still reach these call sites in accepted programs was not established. |
| A0454 | omega-rust/psi/compilation/validation/src/lib.rs:773 | CLOSED | `ProgramEntryVisibleParameters` has only two variants; the fallback arm is followed by the `ImageAndInitialStorage` contract block at lib:776. |
| A0455 | omega-rust/psi/compilation/validation/src/lib.rs:940-951 | REACHABLE | Entry parameters with an access-view plan are excluded from `visible`, so parameter-count checks can pass while real arrival slots remain on the entry machine. |
| A0456 | omega-rust/psi/compilation/validation/src/lib.rs:421-444 | UNDECIDED | Incomplete rank-runtime recursive components are dropped from `checked_dependencies`; effect on whether invalid programs are accepted was not traced to a diagnostic. |

## A0449

**Cited site (read):** `effect_inference.rs:1279-1314` in `OperationalCallSites::expression_call` (`omega-rust/psi/compilation/validation/src/effect_inference.rs`). At `effect_inference.rs:1285-1287`, `is_exact_embed_call` causes an early `return` before `push_call`. Line numbers match this checkout (not re-diffed against `git show 457adf4a11f:`).

**Enclosing function (read):** `expression_call` is invoked from `visit_expression` when traversing `ExpressionNode::Call` (`effect_inference.rs:198-206`). `visit_statement` (`effect_inference.rs:63-128`) routes statement kinds into `visit_expression` but not into assembly arms (`effect_inference.rs:72`).

**Call-site visitor callers (read):** `rg '\bvisit_state_calls\b' omega-rust` → 5 matches, all in `effect_inference.rs` (definition at `effect_inference.rs:36`, uses in invocation/operational collection around `effect_inference.rs:581` and `effect_inference.rs:1223`).

**`is_exact_embed_call` (read):** `proof_contracts_core.rs:6433-6443` requires invalid receiver, target name `IntegerEmbed`, and matching builtin symbol.

**Upstream to acceptance (read):** `push_call` (`effect_inference.rs:1348-1396`) records `CallWork` with suspend/block from `DirectSignatureEffects::for_symbol`. Service reach for operational calls uses the separate `OperationalPlan.calls` list built by `collect_state_calls` (`effect_inference.rs:1210-1240`), then `direct_service_reach_for_operational_call` at `effect_inference.rs:2769-2774`. Service ceiling errors use understated reach only when inference misses services (`machine_calls_core.rs:5463-5488` compares `inferred_transitive` to declared rows). Exact integer embed calls are proof-only: typed as proof int at `expression_types_and_fields.rs:4930-4934` and excluded from embed misuse elsewhere (`calls.rs:3174`, `arithmetic_domains.rs:22341`).

**Input (read):** A checked body whose only expression calls are exact `IntegerEmbed` proof embeddings.

**Verdict:** CLOSED — skipping `push_call` removes operational `CallWork` for a form that has no operational or service-reach obligations in other validators read here; it does not open an accept path for a program that should be rejected for suspend/block/service reasons on that call shape.

## A0450

**Cited site (read):** `effect_inference.rs:71-72` in `visit_statement`: `StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}`.

**Enclosing function (read):** Full `visit_statement` (`effect_inference.rs:63-128`) and `visit_state_calls` (`effect_inference.rs:36-60`).

**Callers (read):** Same `visit_state_calls` search as A0449 → 5 matches in `effect_inference.rs` only. Operational collection: `collect_state_calls` (`effect_inference.rs:1210-1223`) is the sole production use for operational call lists.

**Downstream (read):** `infer_operational_may_uncached` builds machine summaries from collected calls (`effect_inference.rs:1126+`). `infer_service_reaches_uncached` (`effect_inference.rs:2327-2354`) uses `machine_reach_work`, which iterates `operational.calls` per state (`effect_inference.rs:2756-2775`), not assembly statement nodes directly. Asm intrinsics reached only as operational **table** calls go through `direct_service_reach_for_operational_call` (`effect_inference.rs:2960-2983`).

**Input (read):** A machine state whose executable body consists only of assembly or assembly-fact statements (no `Call`, `Expression` call, or transition transfers that recurse into calls).

**Verdict:** REACHABLE — such a body contributes no `CallWork` and no per-call service reach; a machine that should be rejected for undeclared services or operational effects invoked only through assembly syntax not modeled as table calls could be accepted if no other pass reports it (assembly arms are empty in many other scanners, e.g. `domains_and_predicates.rs:8569`, `calls.rs:2246`).

## A0451

**Cited site (read):** `effect_inference.rs:2966-2982` in `direct_service_reach_for_operational_call`. When the target is an asm intrinsic (`effect_inference.rs:2969`), the function builds default `DirectServiceReach`, optionally extends services only when `assembly_services.resolve` returns `Ok` (`effect_inference.rs:2972-2980`). If `resolve` fails, it returns reach with empty service sets (`effect_inference.rs:2982`).

**Enclosing function (read):** Full `direct_service_reach_for_operational_call` (`effect_inference.rs:2960-2994`).

**Callers (read):** `rg '\bdirect_service_reach_for_operational_call\b' omega-rust` → 2 matches (definition and call at `effect_inference.rs:2769`).

**Upstream (read):** `machine_reach_work` merges `call_reach.services` into `direct` and `dependency_direct.concrete` (`effect_inference.rs:2776-2788`). Fixed-point closure at `effect_inference.rs:2404-2514` fills `inferred_transitive`. `validate_undeclared_service_reach` (read name from callsite context at `machine_calls_core.rs:5445-5488`) errors when `required` services from inference exceed the machine's declared service-reach row — missing inferred services yield no error.

**Input (read):** An operational call to a builtin asm intrinsic whose `asm_intrinsic_service_name` does not resolve for the owning machine in `AssemblyServices::resolve`.

**Verdict:** REACHABLE — inference understates reachable services; the undeclared-service diagnostic at `machine_calls_core.rs:5464-5488` does not fire when the true service is omitted from `inferred_transitive`.

## A0452

**Cited site (read):** `effect_inference.rs:3121-3186` in `direct_service_reach_for_call`. After parameter-signature and trait branches (`effect_inference.rs:3159-3168`), an optional boundary machine match extends reach (`effect_inference.rs:3170-3185`); otherwise the function returns `reach` still at default/empty (`effect_inference.rs:3186`).

**Enclosing function (read):** Full function `effect_inference.rs:3121-3187` (also read redirect arms at `effect_inference.rs:3125-3138`).

**Callers (read):** `rg '\bdirect_service_reach_for_call\b' omega-rust` → 5 matches in `effect_inference.rs` (internal recursion and operational-call path at `effect_inference.rs:2984`).

**Upstream (read):** Same service-reach fixed point and `machine_calls_core.rs:5463-5488` as A0451.

**Input (read):** An operational call whose `target_state_symbol` is valid but is not a machine-parameter signature entry, trait machine signature, or `boundary_machine_by_state` key (for example some operator or unresolved boundary targets after `push_call` normalization at `effect_inference.rs:1362-1370`).

**Verdict:** REACHABLE — direct reach for that call is empty, so transitive inference can miss services the call actually invokes; digest note "traced for ordinary machines only" matches the code path that only fills reach for the three indexed kinds.

## A0453

**Cited site (read):** `lib.rs:164-188` in `value_custody::pattern_components::report_delivery`. At `lib.rs:172-174`, `!value.is_valid()` returns `false` without pushing a diagnostic.

**Callers (read):** `rg '\breport_delivery\b' omega-rust` → 3 matches: definition `lib.rs:164`, `expression_types_and_fields.rs:3370`, `literals_and_struct_literals.rs:4602`.

**Caller behavior (read):** `report_delivered_type_mismatch_with_component` (`expression_types_and_fields.rs:3361-3387`) treats `true` as handled; on `false`, calls `delivered_type_conflict` (`expression_types_and_fields.rs:3380-3386`), which begins with `delivered_value_type(program, value)?` (`expression_types_and_fields.rs:3307-3310`) and likely yields no diagnostic for invalid handles. Array literal loop at `literals_and_struct_literals.rs:4601-4609` does not check the `bool` return.

**Upstream (read):** Not traced whether typed-tree preparation guarantees valid element handles before validation for programs that reach acceptance.

**Verdict:** UNDECIDED — silent return is real at the cite, but no end-to-end accept path with an invalid expression handle at these sites was demonstrated.

## A0454

**Cited site (read):** `lib.rs:754-774` in `product_entries::validate_source_compatibility`. Match on `slot.visible_parameters`: arms for `None` with nonempty `visible` and `ImageAndInitialStorage` with `visible.len() != 2`; `_ => {}` at `lib.rs:773`.

**Enum (read):** `semantic-vocabulary/src/lib.rs:274-277` defines only `ProgramEntryVisibleParameters::None` and `ImageAndInitialStorage`.

**Follow-on (read):** When `slot.visible_parameters == ProgramEntryVisibleParameters::ImageAndInitialStorage && visible.len() == 2`, `core_arrival_requirement_contract` and per-parameter identity checks run (`lib.rs:776-808`). Failure returns `Err(diagnostics)` at `lib.rs:811-813`.

**Callers (read):** `rg '\bvalidate_source_compatibility\b' omega-rust` → 4 matches (`lib.rs:716`, `frozen_program_and_queries.rs:3682`, `program_entry.rs:175`).

**Verdict:** CLOSED — there is no third `visible_parameters` variant; `ImageAndInitialStorage` with two visible parameters is validated after the `_ => {}` arm, and incompatible `None`/counts still error in the explicit arms.

## A0455

**Cited site (read):** `lib.rs:940-951` in `visible_entry_parameters`: filters out parameters where `access_view_plan_for_input_type_reference(parameter.type_reference).is_some()`.

**Callers (read):** `visible_entry_parameters` used at `lib.rs:753` inside `validate_source_compatibility` and exported to `program_entry.rs:181` (provider planning).

**Upstream (read):** `validate_source_compatibility` compares `visible.len()` to slot expectations (`lib.rs:754-772`) and, for `ImageAndInitialStorage`, compares types of the two visible parameters to the arrival requirement (`lib.rs:776-807`). Parameters classified as access-view inputs never enter `visible`.

**Input (read):** A `ProgramEntry` root binding with schema `ProgramEntryVisibleParameters::None` where the entry machine still declares one or more parameters that use access-view input types, so `visible` is empty while nontrivial parameter slots remain on the entry signature.

**Verdict:** REACHABLE — compatibility can succeed on parameter count while the slot still binds an entry machine that carries parameters not treated as visible arrival parameters; whether that violates a real target contract depends on the slot, but the filter creates an accept path relative to machines that still declare those parameters.

## A0456

**Cited site (read):** `lib.rs:414-498` in `compiler_evaluation::checked_dependencies`. Loop over `ranks.runtime_components` (`lib.rs:421-444`): `continue` when bodies empty, diagnostics present, termination judgments missing, machine/body pairing ambiguous, or later `complete == false` during edge/call alignment (`lib.rs:445-492`). Only when `complete` (`lib.rs:493-497`) inserts a `CheckedCompilerEvaluationComponent`.

**Callers (read):** `rg '\bchecked_dependencies\b' omega-rust` → 2 matches (`lib.rs:414`, `published_facts.rs:4580`).

**Downstream (read):** Result is stored in `CheckedCompilerEvaluationDependencies.recursive_components` during declaration description freeze (`published_facts.rs:4577-4581`). Not traced through every consumer whether omission weakens a validation diagnostic before `CheckedTrees` publication.

**Input (read):** A rank-runtime recursive component whose member bodies fail the termination or edge-completeness predicates at `lib.rs:423-441` or `lib.rs:471-492`.

**Verdict:** UNDECIDED — incomplete components are silently omitted from the dependency bundle, but no traced path from that omission to accepting a program that should be rejected was read.

## What I did not read

- `AssemblyServices::resolve` implementation and full assembly operational modeling outside `effect_inference.rs`.
- Every consumer of `CheckedCompilerEvaluationDependencies` beyond `published_facts.rs:4580`.
- Corpus samples exercising asm-only states, failed asm service resolution, or access-view entry parameters.
- Stage-04 guarantees on expression handle validity before validation.

SLICE COMPLETE

