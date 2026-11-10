# slice_0067

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0513 | omega-rust/psi/compilation/validation/src/places.rs:6124-6128 | REACHABLE | Private `carriers_match` treats invalid-symbol `Named`/`Generic` pairs as equal on spelling in `cast_shares_storage`. |
| A0514 | omega-rust/psi/compilation/validation/src/places.rs:1618-1626 | REACHABLE | Array extent `Named` with invalid symbol can close as an integer parsed from the type name string. |
| A0515 | omega-rust/psi/compilation/validation/src/program_validation.rs:74-79 | REACHABLE | Non-unit return states with no finite execution order skip the missing-return completion pass. |
| A0516 | omega-rust/psi/compilation/validation/src/program_validation.rs:4927-4929 | REACHABLE | Cleanup graph walk `continue`s when a dependency target has no operational-plan machine index. |
| A0517 | omega-rust/psi/compilation/validation/src/program_validation.rs:4555-4570 | REACHABLE | `validate` can return `Ok` while `deferred_overflow` holds the only exact-overflow errors. |
| A0518 | omega-rust/psi/compilation/validation/src/program_validation.rs:860-887 | UNDECIDED | Entailment `reuse` keys on `body_hash`/`dependency_context`/callees; full set of non-hashed semantic inputs not traced. |
| A0519 | omega-rust/psi/compilation/validation/src/proof_contracts_core.rs:217-221 | REACHABLE | Under `ContentUse::Quotient`, unresolved `Named` carrier atoms yield no fence when no type parameter matches. |
| A0520 | omega-rust/psi/compilation/validation/src/proof_contracts_core.rs:795-801 | REACHABLE | Missing or non-transparent proposition definitions make `proposition_has_source_builtin_index_meaning` return `true`. |

## A0513

**Digest (read):** area_14 preamble maps `pl` → `omega-rust/psi/compilation/validation/src/places.rs`. Group **mpb, ops, pl (19)**. Cite `pl:6124-6128` (and companion `6142-6146`): carrier match falls back to name equality.

**Cited site (read):** `places.rs:6096-6150`, function `carriers_match` (re-located; digest cites matched this checkout). For `TypeReferenceNode::Named` and `Generic`, when either base symbol is invalid, equality is `n1 == n2` / `n2 == n2` instead of symbol identity (`places.rs:6124-6127`, `6142-6145`).

**Enclosing use (read):** `cast_shares_storage` (`places.rs:6058-6094`) calls `carriers_match` for `MemberPosition::Reference` (`places.rs:6082-6083`). `cast_preserves_place` and `storage_operand` peel casts via `cast_shares_storage` (`places.rs:6033-6037`, `6209-6211`).

**Public API contrast (read):** `declared_carriers_match` → `declared_carriers::compare` uses `normalized_type_identity` / `substituted_identity` on the default arm (`places.rs:4977-4991`), not spelling fallback.

**Callers (read):** `rg '\bcarriers_match\b' omega-rust` → 1 definition in `places.rs` (private). `rg 'cast_shares_storage' omega-rust` → 4 matches, all in `places.rs` (`:6037`, `:6058`, `:6165`, `:6210`).

**Input that reaches the branch (read):** A cast on a reference whose operand and target carrier are distinct named types with invalid symbol handles but identical name spellings; `cast_shares_storage` returns true, so storage-place peeling treats the cast as non-disruptive.

**Verdict:** REACHABLE — two different carriers can be judged storage-sharing carriers by spelling alone on this path.

## A0514

**Digest (read):** `pl:1618-1626` — `Named` reference with invalid symbol read as a number from spelling.

**Cited site (read):** `ExtentResolver::reference` (`places.rs:1602-1638`). On `TypeReferenceNode::Named { name, .. }` without a valid symbol path, `name.as_str().parse::<i128>()` can produce `ResolvedArrayExtent::Closed` (`places.rs:1618-1620`), else `CanonicalConstValue::from_atom` (`places.rs:1622-1625`).

**Enclosing path (read):** `resolved_array_extent` invokes `resolver.reference` for `FixedArrayLength::Expression` (`places.rs:1523-1524`). Exported as `resolved_array_extent` / `resolved_array_extent_at` (`places.rs:1506-1527`, `1462-1487`).

**Callers (read):** `rg 'resolved_array_extent' omega-rust/psi/compilation/validation` → uses in `places.rs`, `lib.rs`, `contract_entailment.rs` (e.g. `contract_entailment.rs:10494` not opened in full).

**Input that reaches the branch (read):** Fixed-array length written as a type reference whose `Named` node has an invalid symbol and a name spelling a decimal integer (for example `"8"`), so extent resolution closes without a const declaration.

**Verdict:** REACHABLE — extent can authenticate from the type-name string rather than a resolved const/value.

## A0515

**Digest (read):** `pv:74-79` — state with return type and no finite order not checked for missing return.

**Cited site (read):** `completion_results::validate` (`program_validation.rs:59-144`). After guards for checked body, non-unit return, and core-vector exemption (`program_validation.rs:65-72`), `finite_execution_order` must be `Ok(Some(order))`; `Ok(None)` or `Err(_)` returns early (`program_validation.rs:74-78`) before the loop that can emit implicit-completion errors (`program_validation.rs:130-143`).

**Caller (read):** `rg 'completion_results::validate' omega-rust` → 1 production call at `program_validation.rs:4284` inside per-state machine validation during `validate` (`program_validation.rs:4275-4284`).

**Control-flow semantics (read):** `StatementTable::finite_execution_order` returns `Ok(None)` when the control graph’s `finite_execution_order` hits a revisited block (`typed-trees/.../calls_and_control_flow.rs:2320-2324`).

**Other diagnostics (read):** `rg 'implicit body completion' omega-rust/psi/compilation/validation` → only `program_validation.rs:140-154`.

**Input that reaches the branch (read):** A checked machine state with a non-unit return type whose statement control has no finite linear order (cycle), so completion checking is skipped entirely.

**Verdict:** REACHABLE — such a state is not reported here for missing/implied wrong completion.

## A0516

**Digest (read):** `pv:4927-4929` — cleanup target missing from operational plan skipped.

**Cited site (read):** `validate_cleanup_normal_completion` (`program_validation.rs:4906-4970`). While expanding cleanup-dependency reachability, `operational.machine_index_by_symbol(target)` `None` triggers `continue` (`program_validation.rs:4927-4928`) instead of diagnosing or stopping.

**Caller (read):** Single call from the tail of `validate` after operational plans are built (`program_validation.rs:4899`).

**Operational index (read):** `machine_index_by_symbol` in `flow-effects` (`omega-rust/psi/foundation/flow-effects/src/lib.rs:245+` — signature only, not full plan construction).

**Input that reaches the branch (read):** A `cleanup_dependencies` edge whose `target_machine` symbol is absent from the operational plan’s machine index while the walk still needs that machine’s call-closure external-exit property.

**Verdict:** REACHABLE — that machine’s “must return normally” requirement is not evaluated.

## A0517

**Digest (read):** `pv:4555-4570` — program whose only errors are exact-overflow messages could pass `validate` if a caller ignores `deferred_overflow`.

**Cited site (read):** Inside `validate` (`program_validation.rs:4554-4570`). When any error exists, exact-overflow diagnostics are drained into `deferred_overflow` via `is_exact_overflow_diagnostic` (`program_validation.rs:4558-4562`, `4645-4648`). If no other errors remain, the second branch does not re-append overflow (`program_validation.rs:4565-4567`), and `finish_diagnostics(diagnostics)?` succeeds on an empty error list (`program_validation.rs:4570`). `ProgramValidation` carries `deferred_overflow` (`program_validation.rs:4641`, `3943`).

**Message filter (read):** `is_exact_overflow_diagnostic` requires error, message prefix `"exact arithmetic in "`, and substring `"may overflow"` (`program_validation.rs:4645-4648`).

**Downstream compile path (read):** `prepare_typed_program` in `05_typed-trees-to-checked-trees/src/checking.rs` retains `deferred_overflow` and, at end of main checking, returns `Err` if it is non-empty (`checking.rs:1148-1149`). `validate_program` also consults `discharge_storage_formations` for overflow on the facts path (`program_validation.rs:3957-3961`).

**Callers of bare `validate`/`prepare_specialized_program` (read):** `prepare_specialized_program` only from `checking.rs:413`; production checking merges overflow before success.

**Input that reaches the branch (read):** Validation emits only diagnostics matching the exact-overflow wording; `validate` returns `Ok(ProgramValidation { deferred_overflow: [...] , .. })`.

**Verdict:** REACHABLE — at the `validate` API surface the program is accepted while overflow failures sit only in the field (shipped checking closes via `checking.rs:1148-1149` when that path is used).

## A0518

**Digest (read):** `pv:860-887` — body reuse; semantic inputs outside hashed source text would not invalidate reuse; missing inventory “not traced”.

**Cited site (read):** `dependency_bodies::DependencyBodyIndex::reuse` (`program_validation.rs:860-887`). Reuse requires `prior.body_hash == admission.body_hash`, matching `dependency_context`, and `callees_match` (`program_validation.rs:872-875`). `body_hash` comes from `local_body_hash` (`program_validation.rs:1216-1275`), which hashes statement source spans (when present), contract fact source via `hash_fact`, and callee indices — not an exhaustive semantic snapshot.

**Caller (read):** `rg '\.reuse\(program' omega-rust` → 1 site, `judge_machines` (`program_validation.rs:4736`).

**Upstream (read):** `reuse` is consulted during deferred entailment judging (`program_validation.rs:4727-4761`), not during initial `validate` acceptance.

**What was not traced (per digest):** Which program mutations leave `body_hash`/`dependency_context` unchanged but change entailment semantics (tests hint at query-input sensitivity in `contract_entailment.rs:67805` — title only, body not read).

**Verdict:** UNDECIDED — reuse gate is explicit at `program_validation.rs:872-875`, but whether a satisfiable semantic change can reuse without hash change was not established end-to-end.

## A0519

**Digest (read):** `pcc:217-221` with `245-255` — quotient over carrier whose type name resolves to something other than data or a data type parameter.

**Cited site (read):** `first_forbidden_content` (`proof_contracts_core.rs:157+`). For `TypeReferenceNode::Named` without data definition (`proof_contracts_core.rs:186-196`), with `ContentUse::Quotient`, parameter lookup searches all data definitions’ type parameters (`proof_contracts_core.rs:207-211`). If `parameter` is `None`, the `let Some(parameter) else` arm returns a violation only for `ValueObservation`/`SharedObservation`, not `Quotient` (`proof_contracts_core.rs:216-221`). For `Generic` without data definition, same asymmetry at `proof_contracts_core.rs:245-254`.

**Consumer (read):** `first_forbidden_quotient_content` (`proof_contracts_core.rs:79-91`) called from `collect_validated_quotient_formations` (`quotients.rs:564-568`). Outer checks require `base_data_symbol` and `data_definition_by_symbol` on the quotient’s carrier root (`quotients.rs:545-562`).

**Input that reaches the branch (read):** A quotient carrier data type whose nested `Named` type reference is not a primitive, not a data definition, and not any data type parameter symbol — the quotient fence walk returns `None` instead of `CarrierFenceViolation`.

**Verdict:** REACHABLE — forbidden carrier content inside an otherwise admitted quotient carrier can escape this fence for `ContentUse::Quotient`.

## A0520

**Digest (read):** `pcc:795-801` — unresolved or opaque proposition returns `true`.

**Cited site (read):** `proposition_has_source_builtin_index_meaning` (`proof_contracts_core.rs:781-815`). Missing `proposition_definition_by_symbol` → `return true` (`proof_contracts_core.rs:795-797`). Non-`Transparent` body → `return true` (`proof_contracts_core.rs:799-800`).

**Caller chain (read):** `fact_has_source_builtin_index_meaning` dispatches proposition facts here (`proof_contracts_core.rs:775-776`). `rg 'fact_has_source_builtin_index_meaning' omega-rust` → `proof_contracts_core.rs` (definition, `correspondence_fact` at `:1246-1248`) and `contract_entailment.rs` (`:15226`, `:15441`, `:15597`, `:15624` — cites not all read in full).

**Effect on acceptance (read):** `correspondence_fact` returns `None` when the gate is false (`proof_contracts_core.rs:1246-1249`); with missing proposition, the gate is true, so correspondence extraction may proceed. `entailed` requires the goal to pass the same gate (`contract_entailment.rs:15597-15600`).

**Input that reaches the branch (read):** A proof fact that applies a proposition symbol with no definition entry (or an opaque/non-transparent declaration body) in a contract or quotient correspondence path using this gate.

**Verdict:** REACHABLE — unresolved/opaque propositions are treated as having source builtin index meaning.

## What I did not read

- Full bodies of every `declared_carriers_match` production call site in `contract_entailment.rs` and `calls.rs` (counts only for `carriers_match` / `cast_shares_storage`).
- Operational-plan construction proving when `machine_index_by_symbol` can be absent for a listed cleanup target.
- Exhaustive inventory of fields excluded from `local_body_hash` beyond `program_validation.rs:1216-1275`.
- Entire `entailed` and correspondence pipelines in `contract_entailment.rs` beyond cited gate lines.
- Tests at `contract_entailment.rs:67641` / `:67805` (titles seen via search only).

SLICE COMPLETE

