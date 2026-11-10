# slice_0130

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1000 | S03/expression_lowering.rs:2918-2921 | REACHABLE | Call-operand `==` with no derivable data type name falls through to plain binary equality lowering. |
| A1001 | S03/statement_lowering.rs:1391-1396 | REACHABLE | A hoist-local whose type inference fails keeps the authored `Unit` type reference while still lowering the initializer. |
| A1002 | S03/type_reference.rs:180-183, 855-857 | REACHABLE | Ambiguous domain constraints stay authored; generic application arity mismatch is skipped in const closing. |
| A1003 | S04/body_cloning_and_rewriting.rs:744-746 | REACHABLE | Machine clone proceeds unless a `requires` type predicate statically evaluates to `Some(false)`. |
| A1004 | S04/body_cloning_and_rewriting.rs:4796-4819, 5551-5553 | REACHABLE | Runtime-bound scan misses struct-literal types; operand recording does not match binding count. |
| A1005 | S04/monomorphization.rs:2667-2669 | REACHABLE | Dynamic slot demand collection errors are swallowed and slot instantiation is skipped. |
| A1006 | S04/monomorphization.rs:3885-3903, 4806-4811 | REACHABLE | Incomplete concrete selection is diagnosed only for statement call sites and partial evidence ignores type bindings. |
| A1007 | S04/contextual_and_requirement_calls.rs:1436-1651, 1898-1917 | REACHABLE | Provider close returns `Ok(None)` without error and callers continue; incomplete bindings skip applied views. |

## A1000

**Digest:** area_07 GROUP Stage 03 — `S03/expression_lowering.rs:2918-2921`.

**Site (read):** In `try_lower_structural_equality` (`expression_lowering.rs:2871-2962`), after mismatch checks, `let type_name = match left_type.or(right_type) { Some(name) => name, None => return Ok(None) }` (`2918-2920`). `operand_data_type_name` (`3132-3181`) returns `None` for call and most non-literal forms (`3180-3181`). `classifier_reference_operand` (`3079-3094`) is true when a `Name` path has two or more members, taking the early `return Ok(None)` path at `2880-2901` without structural expansion.

**Callers (read):** `rg '\btry_lower_structural_equality\b' omega-rust` → 2 matches (`expression_lowering.rs:1079`, definition `2871`). `lower_binary_node` (`1071-1099`) calls it for `Equal`/`NotEqual`; when `Ok(None)`, lowers operands with `lower`/`lower_value` and emits a plain binary node (`1090-1099`).

**Upward trace (read):** `lower_binary_node` is reached from `lower_node` (`1060-1068`). Expression lowering is invoked from stage-03 `lower_expression_handle` (inferred from `statement_lowering.rs:16-20` delegating to it). Programs that pass stage 03 typing can therefore carry plain `==` on call results where structural synthesis was expected.

**Input (read):** A machine body comparing two call results (or other operands with no derivable data type name) using `==` on a structural data type that would require expansion or an `Equatable` implementation, where neither operand is a struct literal, name, member, or indexed form with a known type name.

**Sub-note (read, digest `:1404-1415`):** In call lowering, an invalid `target_symbol` uses `program.symbols.get(target_symbol).parent` (`1404-1409`). Whether `symbols.get` on an invalid handle panics was not verified in this trace → **UNDECIDED** for that sub-claim only.

**Verdict:** REACHABLE — structural equality is not applied and plain `==` lowering accepts the comparison.

## A1001

**Digest:** area_07 — `S03/statement_lowering.rs:1391-1396` ("local typed `Unit`").

**Re-location:** Lines at commit `457adf4a11f` match current checkout (read, `git show`).

**Site (read):** `lower_statement_node` for `LocalData` (`1377-1413`). When `declared_is_unit` (`1378-1380`), `infer_hoist_temp_type` may run (`1382-1387`); on `None`, `type_reference` uses `lower_type_reference_handle_from_table` (`1391-1395`), preserving the authored `Unit` reference. Initial value is still lowered via `lower_statement_expression` (`1402-1406`). `infer_hoist_temp_type` returns `Ok(None)` for unhandled expression shapes (`349-366`, `_ => {}` then fallthrough to `Ok(None)`).

**Callers (read):** `rg '\blower_statement_node\b' omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees` → 3 matches (`statement_lowering.rs:1118`, `1167`, `1277`, definition `1282`).

**Upward trace (read):** `lower_statement` (`1267-1279`) dispatches state-body statements during stage-03 settlement (`S03/lowerer.rs:1849` cohort close, inferred from digest preamble).

**Input (read):** A hoist temporary or local declared with type `Unit` (or inferred with unit declaration) initialized by an expression form `infer_hoist_temp_type` does not handle (for example a non-boolean binary expression at `359-363`), where the initializer’s real type is not `Unit`.

**Verdict:** REACHABLE — the local is recorded as `Unit` while the initializer is lowered without rejecting the mismatch.

## A1002

**Digest:** area_07 — domain constraints `S03/type_reference.rs:180-183`; const applications `:832-857`.

**Site (read):** In `normalize_constraint_span` (`147-183`), `select_domain_candidates` (`171-179`); `let [domain] = matches.as_slice() else { normalized.push(TypeConstraintNode::Domain(domain_constraint)); continue }` (`180-182`) retains the authored constraint when zero or several candidates match. In `closed_applications::normalize` (`702+`), duplicate conformance headers `continue` (`832-833`, `844-845`); `if parameters.len() != arguments.len() { continue }` (`855-856`) skips arity mismatch without diagnostic.

**Callers (read):** `rg '\bnormalize_domain_constraints\b' omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees` → uses in `lowerer.rs:1865`, `call_and_product_selections.rs:2210`, `contracts_and_signatures.rs:805` (read).

**Upward trace (read):** Stage-03 `close_typed_cohort` runs domain normalization on newly typed references (digest area_07 "Stage 03").

**Input (read):** A type reference with a domain membership constraint that does not resolve to exactly one domain candidate, or a generic/trait/conformance application whose const/type argument count does not match its parameter list (including duplicate-header ambiguity).

**Verdict:** REACHABLE — unresolved domain constraints and arity-skewed applications proceed without closure or rejection here.

## A1003

**Digest:** area_07 (checked) — `S04/body_cloning_and_rewriting.rs:744-746`.

**Site (read):** In `clone_single_specialized_machine` (`691+`), the `requires` loop (`736-754`) calls `input.applied_static_type_predicate_value(*predicate, &substitutions)` and returns `Err` only when the result is `Some(false)` (`744-752`). Any other result (`None` or `Some(true)`) allows cloning to continue (`755` sets `complete = true`).

**Callers (read):** `rg '\bclone_single_specialized_machine\b' omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees` → 4 matches (`monomorphization.rs:1267`, `3394`, `3497`, definition `body_cloning_and_rewriting.rs:691`).

**Upward trace (read):** Invoked from monomorphization when specializing generic machine instances (stage-04 fixed-point loop, digest preamble).

**Input (read):** Cloning a specialized machine where a `requires` type predicate on the template cannot be resolved to `false` at clone time (unknown or unproved), but would fail if fully checked against the supplied argument tuple.

**Verdict:** REACHABLE — clone proceeds when the predicate is not positively `false`.

## A1004

**Digest:** area_07 — runtime-bound scan `4796-4819`; const unfold `3711-3714, 3784`; binding count `5551-5553`.

**Site (read):** `collect_expression_type_roots` (`4788-4820`) walks Cast, Call, TypeExpression, and ZeroValue only (`4797-4818`); struct literals are not visited, so `application_type` on a struct literal in an expression subtree is not collected. `normalize_const_arguments_for` leaves arguments without a carrier unfolded when `closed_const_argument` is `None` and `closed_structural_argument` does not apply (`3779-3799`, carrier filter `3780-3784`). `record_runtime_call_operands` (`5529-5558`) pairs operands with bindings; on exhausted operands it `break`s (`5550-5551`); extra operands are ignored; no count check against binding arity.

**Callers (read):** `rg '\bcollect_expression_type_roots\b' omega-rust` → 6 matches, all in `body_cloning_and_rewriting.rs`. `reject_runtime_bound_static_occurrences` (`4528-4605`) calls it at `4584`. `rg '\brecord_runtime_call_operands\b' omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees` → definition and call sites in same file (read, grep).

**Upward trace (read):** `reject_runtime_bound_static_occurrences` runs after machine body clone (`1551` region, read grep context).

**Input (read):** A generic specialization binds a value parameter to a runtime argument whose type mention appears only inside a struct literal expression, or a static machine application records fewer runtime operands than bindings without error.

**Verdict:** REACHABLE — the runtime-bound rejection scan and operand recording do not cover these shapes.

## A1005

**Digest:** area_07 (checked) — `S04/monomorphization.rs:2667-2669`.

**Site (read):** `instantiate_dynamic_slots` (`2664-2684`): `let Ok(demands) = validation::collect_dynamic_slot_demands(program) else { return Ok(()) }` (`2667-2668`). No diagnostics are emitted; slot installation and `retarget_selector_calls` are skipped.

**Callers (read):** `rg '\binstantiate_dynamic_slots\b' omega-rust` → 3 matches (`monomorphization.rs:2664`, re-export `2897`, `preparation.rs:2027`).

**Upward trace (read):** Stage-04 preparation calls `instantiate_dynamic_slots` before the monomorphization loop (`preparation.rs:2025-2028`).

**Input (read):** A program where `collect_dynamic_slot_demands` returns `Err` (for example from `collect_dynamic_implementation_selections(program)?` at `traits.rs:5647`) while later compilation still proceeds through preparation.

**Verdict:** REACHABLE — demand-collection failure is ignored and dynamic slots are not instantiated.

## A1006

**Digest:** area_07 (checked) — incomplete concrete selection `3885-3903`, `4806-4811`.

**Site (read):** In `monomorphize_generic_machine_value_calls` (`3746+`), when `enforce_complete_concrete_selections` is true, the incomplete-selection loop (`3885-3902`) filters `CallSite::Statement(_)` only (`3893`), not expression sites. `incomplete_selection_diagnostic` attaches spans for expression sites (`4795-4798`) but is not reached for them in this loop. `holds_partial_static_evidence` (`4806-4811`) requires const, machine, or evidence bindings partially filled, not type bindings.

**Callers (read):** `rg '\bmonomorphize_generic_machine_value_calls\b' omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees` → `preparation.rs:2133`, definition `monomorphization.rs:3746` (read).

**Upward trace (read):** Fixed-point monomorphization during stage-04 preparation (`preparation.rs:2133-2135` with `enforce_complete_concrete_selections` flag).

**Input (read):** A non-generic caller with an expression-position generic machine call whose static selection has only incomplete type bindings (no partial const/machine/evidence bindings), leaving the call unspecialized.

**Verdict:** REACHABLE — no incomplete-selection diagnostic is emitted from this enforcement for expression call sites with type-only partial evidence.

## A1007

**Digest:** area_07 — `providers::close` `1436-1651`, caller `1898-1917`; retarget `2614-2619`; bindings `2127-2129`, `3185-3187`, `3254-3256`.

**Site (read):** `providers::close` (`1436+`) returns `Ok(None)` without error at: non-empty state type parameters (`1463-1464`), missing machine (`1470-1471`), missing state (`1518-1519`), parameter count mismatch (`1543-1544`), incomplete provider bindings (`1605-1611`), and callable already matches (`1615-1626`). Caller at `1898-1917` uses `closed_entry.unwrap_or(argument.symbol)` and `continue`s when `closed_entry` is `None` (`1915-1917`). `invocation_bindings` matching (`2614-2619`) returns `SymbolHandle::invalid()` when zero or several applications match. `invocation_binding_views::public_bindings` failure (`2127-2128`, `3185-3186`, `3254-3255`) leads to `continue` without applied view installation.

**Callers (read):** `rg '\bproviders::close\b' omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees` → 3 call sites (`contextual_and_requirement_calls.rs:342`, `826`, `1898`). Top-level `contextual_calls::close(program)` (`1834+`) is invoked from `monomorphize_generic_machine_value_calls` (`3771`).

**Other cited sites (read):** `demands::retain` continues when `applied_machine_provider` is `None` and no selection flag (`136-147`, `491-497`). `demands::members` returns empty when target is not in a family (`422-427`). Tuple argument length mismatch in requirement preparation `continue`s (`2905-2906`).

**Input (read):** A requirement or contextual machine call whose provider cannot be closed (ambiguous machine argument, arity skew, or incomplete static bindings), where the pass continues with the original symbol and no applied provider tuple.

**Verdict:** REACHABLE — closing failures are silent `Ok(None)` or invalid-symbol no-ops and compilation proceeds.

## What I did not read

- Whether `program.symbols.get` on an invalid `SymbolHandle` panics (`S03/expression_lowering.rs:1409`) — digest marked unsure.
- Exact `Err` paths inside `collect_dynamic_implementation_selections` beyond the `?` at `traits.rs:5647` (inferred only that `collect_dynamic_slot_demands` can fail).

SLICE COMPLETE
