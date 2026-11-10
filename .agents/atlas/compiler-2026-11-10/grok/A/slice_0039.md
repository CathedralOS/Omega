# slice_0039

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0293 | omega-rust/psi/compilation/psi-compiler/terminal-production/src/build_bindings.rs:33-41 | REACHABLE | Ambiguous `TerminalMachineSelection::Name` yields `root: None` and capture still returns `Ok` scopes. |
| A0294 | omega-rust/psi/compilation/psi-compiler/terminal-production/src/lib.rs:173-177 | REACHABLE | `calling_signatures::capture` returns `Ok(())` while per-instance ABI failures sit in `Result` fields on the module. |
| A0295 | omega-rust/psi/compilation/psi-compiler/terminal-production/src/lib.rs:188-198 | REACHABLE | Non-external machines without `abi_for_boundary` make `boundary_application` return `None` and the boundary row is skipped. |
| A0296 | omega-rust/psi/compilation/representations/checked-trees/facts/src/lib.rs:1859-1867 | REACHABLE | Equal bare-name labels make `places_match` true and feed domain proof via `proves_place_domain_membership_in_program`. |
| A0297 | omega-rust/psi/compilation/representations/checked-trees/facts/src/lib.rs:1466 | REACHABLE | Unresolved members become `PlaceSegment::Field { symbol: invalid }`, so distinct paths can share segment lists. |
| A0298 | omega-rust/psi/compilation/representations/checked-trees/facts/src/predicates.rs:100-125 | UNDECIDED | Truncated byte compare is implemented; whether a violation is an assumption or a hard requirement on the acceptance path was not settled. |
| A0299 | omega-rust/psi/compilation/representations/checked-trees/src/contract_and_obligation_facts.rs:109 | REACHABLE | Unsupported predicate shapes make `retain` return `None`, so no per-target obligation is retained while checking may still complete. |
| A0300 | omega-rust/psi/compilation/representations/checked-trees/src/contract_and_obligation_facts.rs:189 | UNDECIDED | `discharge_all` passes `&\|_, _\| true` at evaluate time; no production caller of that `discharge_all` was found. |

## A0293

- Cited `TerminalMachineSelection::Name` handling at `build_bindings.rs:32-41` (read): filters machines by `machine.name.as_str() == name`, keeps a symbol only when exactly one match (`next().filter(|_| matching.next().is_none())`), otherwise `root` is `None` (inferred: zero or multiple matches).
- Read enclosing `native_formations::capture` (`build_bindings.rs:18-96`, read): on `Err` only when scope mismatch or missing declared request; ambiguous name does not produce `Diagnostic`.
- `root` is passed to `typed_trees::product_operand::native_binding_formation` (`build_bindings.rs:64-68`, read). Read that function (`product_and_trait_declarations.rs:1475-1527`, read): `root_scope` is `root.and_then(...same_source_package...)` — `None` root yields `root_scope: false`, not an error (`:1519-1527`, read).
- Caller search `capture_native_formations(` in `omega-rust`: 1 call site — `terminal-production/src/lib.rs:440` (read), inside `TerminalProductionRequest::produce` (`:431-441`), which maps formation errors to `TerminalArtifactProductionError::BuildContext` but does not treat ambiguous `Name` as an error.
- Input that reaches the branch (read): terminal production with `TerminalMachineSelection::Name` for a string that names zero machines or more than one machine in `checked.machines()`, plus native-binding Build declarations so `capture` runs.
- Wrong program (plain words): authored native-binding scope is pinned to a machine name that is missing or duplicated in the checked program; compilation should reject ambiguous custody but production proceeds with `root: None` and `root_scope: false`.

## A0294

- Read `calling_signatures::capture` (`lib.rs:46-181`, full function, read).
- Per-instance ABI refusal at `lib.rs:154-163` (read): `application.abi_refusal` becomes `Err(reason)` inside the inner `applications` closure, stored in `CallingPolicyInstance.signatures` (`:164-169`, read). Same pattern for `policy` and `policy_declaration` via `map_err` (`:108-153`, read).
- At `lib.rs:173-177` (read): `CallingPolicyRequest { instances, .. }` is pushed whether `instances` is `Ok(Vec<...>)` with per-field `Err` or request-level `Err(Vec<String>)`; outer `capture` returns `Ok(())` (`:180`, read).
- Consumers read upward: `plan_computation.rs:111-149` (read) uses `instances.as_ref().map_err(...)?` and `signatures.as_ref().map_err(...)?` — fails later at boundary calling-plan preparation, not at terminal capture.
- `CallingProgram::evaluate` (`calling_program.rs:69-78`, read): `policy_declaration` and `policy` `map_err` before use.
- `foundation_validation.rs:1198-1223` (read): empty `instances` `Err` is malformed; non-empty deferred refusals `continue` without validating instance bodies.
- Input (read): checked program with calling-policy instances whose ABI preparation records `abi_refusal`, emitted through `TerminalProductionRequest::produce` when `provider_inventory` is present (`lib.rs:454-463`).
- Wrong program (plain words): a calling-policy instance that should be rejected at terminal capture is serialized into the terminal module with refusal strings in `Result` fields while `produce` still succeeds.

## A0295

- Read `boundary_application` (`lib.rs:183-198`, extended through `:256` for `ExternalRealization` path, read).
- At `:188-190` (read): direct `abi_for_boundary(source)` hit returns `Some(application.clone())`.
- At `:191-197` (read): if machine supply is not `ExternalRealization`, function returns `None` (no `Err`, no diagnostic).
- Caller at `lib.rs:56-59` (read): `let Some(application) = boundary_application(...) else { continue; }` — boundary row omitted silently.
- Search `boundary_application(` in `omega-rust`: 15 matches in 7 files; the cited closure is `terminal-production/src/lib.rs:57` (read). Other matches are different functions or modules (inferred from path names only for non-cited files).
- Input (read): `capture` invoked with a `boundaries` pair whose `source` is a checked machine symbol without prepared boundary ABI and without `ExternalRealization` supply mode.
- Wrong program (plain words): a boundary machine that must publish a calling signature is dropped from `module.calling_signatures` instead of failing terminal production.

## A0296

- Read `places_match` (`fa/lib.rs:1827-1867`, full function, read): after structural `places_equal` (`:1819-1825`) and invocation-root guard (`:1836-1844`), compares canonical labels built at `:1852-1857` / `:1862-1864` (read).
- Read label writer `write_canonical_place_label` / `push_symbol_label` (`contexts_and_places.rs:1335-1338`, read): valid symbols use bare `program.symbols.name(symbol)` (digest `c&p:1337` at commit `457adf4a11f`; line matched at read).
- Read `proves_place_domain_membership_in_program` (`contexts_and_places.rs:1040-1063`, full function, read): domain facts must satisfy `self.plan.places_match(program, fact_place, place)` at `:1062` (read).
- Caller search `\.places_match\(` in `omega-rust`: 11 matches — 4 in `fa/lib.rs` tests (`:91-94`), 1 in `contexts_and_places.rs:1062`, 6 in Psi 05 (`flow_calls.rs:1`, `checks_contracts_prover.rs:4`, `checks_contracts_calls.rs:1`) (read: grep count; production call sites listed, tests separated).
- Read one acceptance path: `checks_contracts_prover.rs:6163-6166` (read) calls `proves_place_domain_membership_in_program` during contract/fact proving.
- Wrong program (plain words): two distinct storage places that share the same bare symbol spelling but different symbol handles; a domain-membership fact on one place satisfies a requirement on the other.

## A0297

- Read `append_place_from_expression` for `ExpressionNode::Name` (`fa/lib.rs:1444-1468`, read): unresolved member gets `SymbolHandle::invalid()` at `:1466` then `push_field_place_segment` (`:1467`, read).
- Read `ExpressionNode::Member` arm (`:1471-1482`, read): same invalid fallback at `:1479`.
- Read `push_field_place_segment` (`:1507-1517`, read): always appends `PlaceSegment::Field { symbol }` even when `symbol` is invalid.
- Read `places_equal` (`:1819-1825`, read): segment equality is derived `PartialEq` on `PlaceSegment` (`contexts_and_places.rs:2064-2066`, read) — two `Field { symbol: invalid }` segments compare equal.
- Digest note on equal segment lists for `a.x` / `a.y` with both unresolved (read): two different member names that both resolve to invalid symbols produce identical segment slices if only invalid field segments are recorded (inferred: labels may still differ by spelling elsewhere; structural `places_equal` does not distinguish invalid fields).
- Wrong program (plain words): two member projections that failed symbol resolution are treated as the same place for structural fact grouping and equality downstream of `places_equal` / label fallback in `places_match`.

## A0298

- Read `byte_literal_equality_in_extent` (`predicates.rs:100-125`, through loop at `:121-125`, read): `maximum_length` caps `enumerate().take(...)` so only a prefix of `bytes` is compared (read).
- Callers (read): `values_scalar.rs:622-627` (`byte_carrier_literal_equality`) and `scalar_computations.rs:348-354` (lowering closure).
- Read `byte_carrier_literal_equality` context (`values_scalar.rs:600-627`, read): builds predicate during scalar/value checking when a byte literal is paired with a bounded carrier.
- Read lowering use (`scalar_computations.rs:340-362`, read): emits checked boolean comparison for terminal lowering; errors only if construction returns `None`.
- UNDECIDED (read): both sites build a comparison expression; I did not trace whether a false prefix-only match is treated as a proof assumption, a hard requirement failure, or only a runtime lowering condition on the path that decides program acceptance.

## A0299

- Read `ConstantPredicateObligation::retain` (`contract_and_obligation_facts.rs:49-126`, full function, read).
- At `:86-89` (read): `ExpressionNode::Binary` requires `builtin(current)` or `return None`.
- At `:109` (read): any other expression shape hits `_ => return None` — no obligation allocated.
- At `:112-114` (read): if no node needed target closure, `retain` returns `None` even after a successful walk.
- Caller (read): `checks_contracts_calls.rs:3450-3473` pushes obligation only when `retain` returns `Some`; otherwise falls through to proof-failure diagnostics (`:3507+`) unless another proof path succeeds.
- Wrong program (plain words): a call-site requires contract whose boolean part needs per-target closure but uses an expression shape the walker discards; no `constant_predicate_obligation` is retained, so later target discharge cannot re-check it while Psi 05 may still complete if proof appears closed without that obligation.

## A0300

- Read `ConstantPredicateObligation::discharge_all` (`contract_and_obligation_facts.rs:173-216`, read).
- At `:182-190` (read): `scalar_predicates::evaluate_nodes` is called with operator filter `&|_, _| true`, unlike `retain`’s `builtin(current)` at `:87-88`.
- Read `retain` vs `capture_predicate` (`:220-308`, read): capture uses `scalar_predicates::source_node` at `:256` without the `builtin` closure passed into `retain`.
- Search `discharge_all` in `omega-rust`: 2 definitions in `contract_and_obligation_facts.rs` (predicate at `:173`, recipe at `:462`); 0 call sites outside that file (read: repository grep).
- Pending obligations without discharge (read): `machine_execution.rs:1010-1025` errors when pending `constant_predicate_obligations` remain in demanded closures; lowering (`attached_unit.rs:7496+`) materializes pending predicates.
- UNDECIDED: whether any acceptance path invokes predicate `discharge_all`, and whether `capture_predicate` can retain binary nodes that `retain`’s `builtin` walk would reject, were not determined.

## What I did not read

- Psi 05 bodies at every `places_match` call site beyond the samples cited above (`flow_calls.rs`, all four `checks_contracts_prover.rs` sites, `checks_contracts_calls.rs:4015`).
- Full `boundary_application` external-realization branch effects on acceptance (`lib.rs:199-256` only skimmed for control flow).
- `validation` helpers backing `byte_carrier_literal_equality` and runtime evaluation of prefix-truncated compares.
- Entire `area_11.md` beyond the preamble (lines 1–120, read) and digest rows 28–32.
- Git commit `457adf4a11f` tree for line drift except `c&p:1337`, which matched at read.

SLICE COMPLETE

