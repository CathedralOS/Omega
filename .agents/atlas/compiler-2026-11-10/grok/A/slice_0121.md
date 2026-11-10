# slice_0121

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0929 | RE/capture/api/slot_metadata.rs:51-56 | UNDECIDED | Empty metadata when no dynamic-slot plans match; did not trace whether a public trait requirement with returned references can lack such plans on checked input. |
| A0930 | RE/capture/behavior/policy/receivers.rs:20-22 | UNDECIDED | Receiver-root check skips when `call_node` is absent; did not trace whether checked `has_receiver` calls always use call expressions. |
| A0931 | RE/capture/callables/boundary_operators.rs:203-205 | REACHABLE | External boundary-operator supply returns `Ok(())` when no selected plan lists the leaf, so review can finish without plan–binding join. |
| A0932 | callables/boundary_operators.rs:141-143 | REACHABLE | Missing callable identity text becomes `""` and is compared to plan identity text by string equality. |
| A0933 | RE/capture/callables/surface.rs:199-200 | CLOSED | Every callable including Build runs `validate_machine_service_reach` in policy projection before Build skips declared inferred ceilings. |
| A0934 | calling/application/signature/instantiation.rs:227 | UNDECIDED | `_` pass-through leaves some type-reference shapes unsubstituted; did not map which kinds reach production signature instantiation. |
| A0935 | RE/capture/contracts/expressions/calls.rs:99-109 | CLOSED | Review re-checks only borrow arguments; non-borrow value arguments were already matched at Psi check time on the sole admitted input. |
| A0936 | package_policy/native_bindings.rs:21-26 | UNDECIDED | `zip` pairs rows to realizations or methods to requirements without local length guards; upstream equality not traced for permission paths. |

## A0929

Digest (area_06, read): classifies `RE/capture/api/slot_metadata.rs:51-56` as open — no slot plan yields empty returned-borrow and qualification metadata.

Function `project_requirement_slot_metadata` (read, `omega-rust/omega/packages/review-evidence/src/capture/api/slot_metadata.rs:19-66`): iterates `compilation.facts.flow.terminal_unit_effects.dynamic_slots` filtered by `declaring_trait` and `requirement` (`:25-31`); each plan projects via `project_plan` (`:49`); if the collected `projected` vec is empty, lines `:51-55` return `Ok(RequirementSlotMetadata { returned_borrow_origins: Vec::new(), parameter_qualification_preconditions: Vec::new() })`.

Caller search: `grep -r 'project_requirement_slot_metadata' omega-rust` → 4 matches, 1 definition and 1 production call at `capture/api/traits.rs:227` inside public trait requirement projection (read).

Upstream (read, `psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit_control.rs:466-494`): `build_dynamic_slot_plans` builds plans only for selector slots in `program.dynamic_slots`, skipping with `continue` when boundary result or parameter signatures cannot be planned — separate from per-requirement publication in `traits.rs`.

Input reaching branch: any checked compilation whose `dynamic_slots` flow table has zero rows for `(trait_symbol, requirement.symbol)` hits `:51-55` and accepts empty metadata in the review record.

Undecided: did not read whether a public trait requirement whose signature includes returned references must always have a matching `CheckedDynamicSlotPlan` row; deciding that requires tracing Psi trait-requirement publication against `terminal_unit_effects.dynamic_slots`, not done here.

## A0930

Digest (read): `receivers.rs:20-22` — call whose authored expression is not a call node is skipped.

Function `validate_call_receiver_roots` (read, `capture/behavior/policy/receivers.rs:12-28`): loops `facts.flow.control.calls` (`:16`); continues when `!call.has_receiver || !call.authored_expression.is_valid()` (`:17-18`); at `:20-21` if `source.expression_table.call_node(call.authored_expression)` is `None`, `continue` without error.

Caller search: `grep -r 'validate_call_receiver_roots' omega-rust` → 3 matches — definition, re-export in `behavior/policy/mod.rs:13`, single production call `capture/callables/policy.rs:52` at start of `project_checked_callable_policy` (read).

Input reaching branch: a checked call fact with `has_receiver == true`, valid `authored_expression`, and an expression node that is not `ExpressionNode::Call`, bypasses receiver-root validation (read, `:20-22`).

Partial upstream (read, `flow_expression.rs:9777-9788`): for expression-site invocations, `authored_expression` is the site expression and `authored_receiver` is taken only when that expression is a `Call`; did not trace all paths that set `has_receiver` on `FlowCallFact`.

Undecided: whether checked custody can pair `has_receiver` with a valid non-call `authored_expression` on compilations admitted to package review.

## A0931

Digest (read): `boundary_operators.rs:203-205` and `boundary_requirements.rs:108-110` — `Ok(())` when no selected plan lists the machine.

`validate_selected_boundary_operator_external_supply` (read, `boundary_operators.rs:186-234`): `selected_boundary_operator_plans` (`:63-88`) filters plans by operator identity; then filters to rows whose realization is `machine.symbol` (`:193-201`); if `matches.is_empty()`, `:203-204` returns `Ok(())` without comparing `binding` to any plan.

Parallel path (read, `boundary_requirements.rs:78-110`): `validate_selected_top_level_requirement_external_supply` returns `Ok(())` at `:108-109` when the filter over selected plans is empty.

Caller search: `grep -r 'validate_selected_boundary_operator_external_supply' omega-rust` → 2 matches — definition and `capture/callables/external_policy/requirements.rs:121-126` inside external operator projection after intrinsic checks (read). `validate_selected_top_level_requirement_external_supply` → 2 matches — definition and `requirements.rs:83`.

Input reaching branch: a checked external-leaf machine that satisfies its conformance in Psi but does not appear as an authored realization on any selected boundary-operator (or top-level-requirement) provider plan for that candidate passes these validators and still flows into `PackagePolicyExternalRequirement` projection (read, `requirements.rs:127-130`).

Reachable (plain words): a package whose external boundary-operator leaf is not listed on any selected provider plan row for that operator still completes review capture of the operator requirement without any plan–binding join check at this site.

## A0932

Digest (read): missing identity text becomes `""` and is compared or stored at multiple RE sites, including `callables/boundary_operators.rs:141-143`.

Primary site (read, `boundary_operators.rs:126-176`): `validate_selected_boundary_operator_checked_adapter` sets `expected_machine_identity` with `compilation.machine_callable_identity_text(machine).unwrap_or_default()` (`:141-143`); plan match uses `plan.identity_text(*machine_identity) == expected_machine_identity` (`:153`).

Related sites (read): `boundary_requirements.rs:92-94` (`unwrap_or_default` on requirement slot); `external_supply.rs:163-165` (`expected_machine_identity` with `unwrap_or_default`); `static_arguments.rs:319-320, 400-401` (`type_identity_text(...).unwrap_or_default()`); `binders.rs:68-69`; `semantics/types/identity.rs:60, 130`.

Reachable (plain words): two checked artifacts that both lack rendered callable or type identity text would compare equal at `"" == ""` and can satisfy a string identity join that would distinguish them if identity were required non-empty.

## A0933

Digest (read): `surface.rs:199-200,264` — Build role with `InternalInferred` service-reach or synchronous-invocation interfaces yields no declared published values; whether Build reaches are validated elsewhere was open.

`surface::project` (read, `capture/callables/surface.rs:193-206,259-272`): for `ServiceReachInterface::InternalInferred` and `role == PackageReviewCallableRole::Build`, `declared_service_reach` is `None` (`:200`); same for synchronous invocations (`:263-266`). Non-Build roles error (`:201-204`, `:268-271`).

Closing check (read, `capture/callables/policy.rs:105-107`): before `surface::project`, every machine (including Build at `:69-70, 97-98`) calls `validate_machine_service_reach(compilation, machine, &service_reaches)?`, which reconciles checked and reconstructed service-reach facts (`source/service_reach.rs:25-102`, read).

Operational facts in `surface.rs:221-227` still require suspension, blocking, and synchronous-invocation checked facts to agree with realized contract envelopes for all roles including Build (read).

Reachable omission is only from the review record's declared ceiling fields, not from skipping checked-fact reconciliation.

Verdict CLOSED: every Build callable path through policy capture runs `validate_machine_service_reach` at `policy.rs:106` before the Build-only `None` branches at `surface.rs:200` and `:264`.

## A0934

Digest (read): pass-through arms at `instantiation.rs:227` and `policy_arguments.rs:190` leave some type references unsubstituted; node kinds not traced.

`project` in `instantiation.rs` (read, `:63-230`): recurses `Named`, `Reference`, `Constrained`, `FixedArray`, `Slice`, `Generic`; at `:227` `_ => return Ok(reference)` returns the handle unchanged for any other `TypeReferenceNode` variant.

`instantiate` / `instantiate_application` callers (grep, read): `signature/parameters.rs` (`instantiate` on parameters and return types); `signature/inheritance.rs:110`; `signature.rs:233,270,296`; `scope.rs:93`.

`argument_type_reference` (read, `policy_arguments.rs:30-191`): nested recursion handles `Reference`, `FixedArray`, `Slice`, `Constrained`; at `:190` `_ => return Ok(argument.type_reference)` passes through other argument type shapes.

Undecided: which residual `TypeReferenceNode` or static-argument shapes appear on checked signatures and conformances that reach `:227` or `:190` without substitution, and whether later identity projection rejects them.

## A0935

Digest (read): `calls.rs:99-109` — only `Borrow` arguments get `checked_argument_matches_type_reference`.

`require_checked_contract_call_arguments` (read, `contracts/expressions/calls.rs:62-111`): pairs call arguments with contract parameter types (`:91-97`); loop at `:98-109` type-compares only when `expression_table.expression(*argument)` is `ExpressionNode::Borrow(_)` (read, `:99-102`).

Caller search: `grep -r 'require_checked_contract_call_arguments' omega-rust` → 3 matches — definition, import/use in `projection/call_expression.rs:49` (read).

Review capture input (digest area_06 preamble, read): `capture/**` takes a **checked compilation** (candidate trees plus custody).

Psi check (grep, read): `validation::checked_argument_matches_type_reference` is used widely in the checker (e.g. `execution_terminal_unit_state_graph.rs:6588`); contract calls are type-checked before admission to review.

CLOSED: non-borrow contract-call arguments are not re-compared in review because the only input path is an already-checked compilation where arity and types were matched during Psi checking; the borrow arm is an extra audit for reference custody.

## A0936

Digest (read): `zip` truncation at `native_bindings.rs:21-26`, `runtime_permissions.rs:185-187`, `terminal_authority_permissions/declarations.rs:87-92`; lengths not shown equal in review-evidence.

`native_bindings::project` (read, `package_policy/native_bindings.rs:11-76`): `candidate.plan.rows.iter().zip(&candidate.provenance.row_realizations)` (`:21-25`) with no explicit length assert in this file; caller `package_policy.rs:100` (grep, read).

`runtime_permissions.rs` (read, `:183-188`): `schema.declaration.methods.iter().zip(declaration.requirements)` inside permission resolution for accepted semantic bindings.

`terminal_authority_permissions/declarations.rs` (read, `:79-98`): `requirements = source.requirements.to_vec()` then `methods.iter().zip(&requirements)`.

Upstream partial (read, `provider-planning/src/provider_declarations.rs:46-47`): asserts `provider.row_realizations.len() == plan.rows.len()` when closing provider declarations — applies to plan rows paired in `native_bindings`, not read through into review custody here.

Undecided: whether `schema.declaration.methods` and catalog `requirements` are always the same length for accepted bindings reaching `runtime_permissions` and `terminal_authority_permissions`; if not, trailing methods or requirements are silently ignored by `zip`.

## What I did not read

- Full `TypeReferenceNode` variant set and all conformance/signature paths into `instantiation.rs:227` and `policy_arguments.rs:190`.
- Psi proofs tying public trait requirements with returned references to `terminal_unit_effects.dynamic_slots` rows.
- `git show 457adf4a11f` line-by-line diff for cited sites (current checkout lines matched on read).
- Package-compilation service catalog invariants equating `methods.len()` to `requirements.len()` for accepted semantic bindings.
- End-to-end package-manager admission tests for review-evidence on corrupt custody shapes.

SLICE COMPLETE
