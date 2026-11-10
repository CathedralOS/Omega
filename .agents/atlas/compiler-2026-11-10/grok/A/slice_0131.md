# slice_0131

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1008 | omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/conformance.rs:253-258 | UNDECIDED | Stage 04 elision uses silent `continue` at several sites; `close_selected_conformance` later errors missing lifetimes for many conformance calls, but I did not prove every skip reaches that gate or that visit early-returns cannot leave a satisfiable wrong binding. |
| A1009 | omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/constructed_results.rs:844-848 | REACHABLE | Derived result type left invalid at 844-848 without a diagnostic; checking often still types the same expression via fallbacks when `retain` returns `Ok(None)` at 65-67, 130-141, or 297-299. |
| A1010 | omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/data_and_product_operands.rs:341-345 | REACHABLE | Arity mismatch or failed `instantiate_type` can leave invalid or uninstantiated field types stored with no Stage 04 error on that assignment path. |
| A1011 | omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/lib.rs:122 | CLOSED | Invalid `receiving_type` is stored at 122, but `assignment_stored_value_type` recomputes from the assignment target whenever the handle is invalid (calls.rs:22288-22311). |
| A1012 | omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/assembly_and_operators.rs:1503-1512 | REACHABLE | Bodiless top-level-requirement operator sites with a selected float comparison are skipped in operator binding with no diagnostic; the authored binary can remain unbound to the requirement body. |

## A1008

**Digest (read).** area_07 preamble: Stage 04 rewrites typed trees for generic machines, operators, requirement calls, and assembly (`digests/area_07.md:20-24`). Silent-drop inventory lists `S04/conformance.rs:253, 338, 444, 534, 850` (`digests/area_07.md:367`).

**Trace (read).** `resolve_elided_conformance_lifetimes` at `conformance.rs:228` collects calls and, per call: if neither `find_state` nor `trait_machine_signature` resolves the callee, `continue` (`253-255`); if `machine_by_symbol(caller_machine)` is missing, `continue` (`256-258`); operand lifetime walk with `inferred.is_none()` → `continue` (`338-340`); for conformance static arguments, `resolved` is `None` when inference conflicts → `continue` (`444-446`); mismatch with explicit lifetime args pushes a diagnostic (`527-532`), otherwise `continue` (`534`); conflicting ordinary-borrow bindings push a diagnostic (`342-347`). `visit_lifetime_bindings_in_scope` (`823-865`) returns without visiting when `operand_lifetime_owner` disagrees with scope (`850-854`), when declared-field scope rejects operand origin (`855-861`), or when `actual_scope.resolve` fails (`863-865`).

**Callers.** Search `resolve_elided_conformance_lifetimes` in the repository: count **2** (definition `conformance.rs:228`; production call `preparation.rs:2129` inside `specialize_machine_calls`, which returns `Result` to `prepare_typed_trees`).

**Closing check (read).** `close_selected_conformance` at `validation/src/traits.rs:4379-4421`: when conformance has erased lifetime parameters and the application supplies none, it returns `Diagnostic::error` (`4416-4420`). `validate_conformance_lifetime_arguments` (`4324-4351`) errors undeclared lifetime names on explicit applications. These run in conformance validation paths (e.g. `traits.rs:4303`, `machine_parameters_and_bindings.rs:5835`).

**Input path (inferred).** A call with elided conformance lifetimes that Stage 04 skips filling still reaches immutable checking; if the conformance has lifetime parameters and no lifetimes are supplied after elision, `close_selected_conformance` should reject compilation even though Stage 04 stayed silent (`444-446`, `534`).

**Verdict.** **UNDECIDED.** I read the silent `continue` sites and a later closing gate for missing conformance lifetimes (`traits.rs:4416-4420`), but I did not trace every program shape that hits `253-258`, `850-865`, or partial inference success with wrong bindings through to either final acceptance or a guaranteed diagnostic. What would decide: a corpus program that hits each skip arm and whether `omega` compile ends `pass` or emits a conformance lifetime error.

## A1009

**Digest (read).** Group cites `constructed_results.rs:844-848` plus `65-67`, `130-141`, `297-299`, `965-969` (`slice_0131.tsv` text; matches `area_07.md:319`).

**Trace (read).** `call_results::retain` (`constructed_results.rs:117-169`): unresolved call target paths return `Ok(Vec::new())` at `65-67`; requirement signature with type parameters → `Ok(None)` at `139-141`; `call_type_arguments` failure → `Ok(None)` at `152-153`. `apply_runtime_result_operands` (`287-299`): parameter/argument count mismatch returns `Ok(return_type)` without substituting operands. `retain_constructed_result_types` (`762-851`): when derived `reference` is `None`, sets `constructed_value_type_reference` invalid at `844-847` and `continue`s without pushing `UnavailableValueType` (unavailable only when `retain` returns `Err`, `792-797`).

**Callers.** Search `retain_constructed_result_types` under `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees`: count **7** (definition `constructed_results.rs:762`; production uses in `preparation.rs:1984, 2002, 2022, 2082, 2117` and `monomorphization.rs:3756, 3774`).

**Downstream typing (read).** `constructor_result_type_reference` in `validation/src/expression_types_and_fields.rs:4732-4757`: invalid constructed handle falls back to struct literal `type_symbol` or declared array type. `expression_result_type_reference` (`4600-4611`) walks `result_type` and does not require a valid constructed handle for all expression forms.

**Input path (plain words).** A call expression whose result type cannot be derived in Stage 04 (unknown callee, generic requirement signature, or arity mismatch in runtime operand substitution) keeps an invalid constructed-value handle but may still be typed through callee return type or literal fallback during checking.

**Verdict.** **REACHABLE.** Smallest story: a machine-state call whose target does not resolve in `retain` (`65-67`) or whose argument count does not match parameters (`297-299`) should be ill-typed if the constructed carrier is authoritative, yet Stage 04 leaves the tree authored with invalid constructed metadata and no diagnostic at `844-848`, and checking can still supply a type via non-constructed paths (`4732-4756`, `4600-4611`).

## A1010

**Digest (read).** Group cites arity skip `341-345`, invalid field storage `826-870`, projected-field skip `183-187`, lifetime passthrough `1496` (`area_07.md:320`).

**Trace (read).** `elaborate_value_fields` generic branch: if parameter, type-argument, or lifetime-argument counts disagree with the generic node, `return` (`341-345`) without instantiating. Field instantiation (`386-407`) assigns `field.type_reference = instantiate_type(...)` with no `Result` check. `instantiate_type` (`826-872`): on failed `retain_array_extent_argument`, `retain_operand_lifetime_owner`, or `attach_constant_origins`, returns `TypeReferenceHandle::invalid()` (`853`, `863`, `870`) without emitting a diagnostic in this function. `elaborate_projected_fields` (`176-187`): when `expression_result_type_reference` is `None`, `continue`s without elaboration. Product lowering `lifetime_identifier` (`1492-1496`): `Ok(matched.unwrap_or_else(|| self.identifier(name)))` reuses the authored name when no frame match.

**Callers.** `elaborate` / `elaborate_new_types` invoked from `preparation.rs:1817, 2034` (re-export `data_applications::elaborate` at `preparation.rs:328`). Search `elaborate_projected_fields` in Stage 04 crate: definition only at `data_and_product_operands.rs:163`, called from `elaborate` at `122`. `retain_declared_field_scopes` (`preparation.rs:1983, 2081, 2128`) is separate and can error on scoped fields (`36-83`).

**Input path (plain words).** A generic data application with wrong argument count is not closed at `341-345`; a field whose instantiation fails still stores an invalid type reference on the member row; product lifetime operands can keep stale binder text when environment lookup does not match (`1496`).

**Verdict.** **REACHABLE.** A program using a generic type application with a parameter-count mismatch, or a field type that fails instantiation metadata retention, can pass Stage 04 with an invalid or open generic field type and no diagnostic on the field-assignment path (`341-345`, `386-392`, `826-870`).

## A1011

**Digest (read).** `lib.rs:122` invalid receiving type; `195-197` rebound parameter without formal skipped (`area_07.md:321`).

**Trace (read).** `assignment_destinations::prepare` (`lib.rs:62-129`): `ReceivingType` from `assignment_stored_value_type` or window shape; on failure, `reference = TypeReferenceHandle::invalid()` (`122`) and stored on the assignment (`127`). `rebound_parameter_homes::materialize` (`172-221`): for each rebound symbol, if no matching formal in `state_parameters`, `continue` (`195-197`) without creating `__rebound_*` home locals.

**Callers.** Search `assignment_destinations::prepare`: count **2** (`preparation.rs:2035, 2118`). Search `rebound_parameter_homes::materialize`: count **2** (`preparation.rs:1966, 2108`).

**Closing check (read).** `assignment_stored_value_type` (`validation/src/calls.rs:22282-22312`): if `assignment.receiving_type.is_valid()`, returns it; otherwise recomputes from `declared_place_type_raw` and reference-shell rules on the assignment target. Indexed/window structural replacement in Stage 05 returns `None` when receiving type invalid (`execution_terminal_unit.rs:1456-1458`), which blocks that optimization path but is not the only assignment check.

**Input path (plain words).** An assignment whose destination type cannot be derived during Stage 04 preparation gets invalid `receiving_type`, but later checks that need a stored type call `assignment_stored_value_type`, which ignores the invalid handle when possible.

**Verdict.** **CLOSED** for acceptance via stored-value typing: every path that needs the assignment’s stored type for checking goes through `assignment_stored_value_type` (`22288-22311`), which does not treat the invalid Stage 04 stamp as authoritative. The rebound skip (`195-197`) only omits synthetic homes for symbols that are not formals in the state roster (read `190-198`).

## A1012

**Digest (read).** `assembly_and_operators.rs:1503-1512` checked: bodiless top-level-requirement operator with selected float comparison skipped; also `791-796` ranking witness mapping (`area_07.md:322`, open item 16 at `area_07.md:507`).

**Trace (read).** `bind_selected_operator_calls` (`assembly_and_operators.rs:1433-1551`): for each collected binary occurrence, if `selected` names a machine with `TopLevelRequirement` supply, no body, and `facts.selected_float_comparison` is `Some`, `continue` (`1503-1511`) before `selected_call_target` binding. Other failures can still push diagnostics (`1543-1548`, `1521-1523`). `carry_ranking_witness` (`764-819`): if custody subject text does not match `entry_mapping`, `return` at `795` leaves `implementation_witness` on the owner (`770-775`).

**Callers.** Search `bind_selected_operator_calls`: count **3** (definition `1433`; `preparation.rs:1976, 2014`). `carry_ranking_witness` is called from within `assembly_control` normalization in the same file (inferred from context at `764`; not separately grepped).

**Input path (plain words).** A comparison expression that selected a bodiless float requirement operator as its overload token is not rewritten to call that requirement in Stage 04; the expression remains a binary with catalog float-comparison facts. Ranking witness relocation can abort mid-mapping and leave witness metadata on the original machine.

**Verdict.** **REACHABLE** for the operator skip (digest open #16): a program that should require lowering to the bodiless top-level float requirement call can remain an unbound binary past Stage 04 (`1503-1512`) with no diagnostic at that site. I did not determine whether later operator checking always rejects such programs (not read end-to-end for float-comparison facts). Ranking witness `791-796` sub-claim was not classified separately; witness may remain on owner when name mapping fails (read `791-796`).

## What I did not read

- Git pin `457adf4a11f` file contents; current-checkout line numbers matched cited symbols on read.
- Full immutable-checking driver to list every conformance application that must pass `close_selected_conformance` (`traits.rs:4379-4421`).
- Stage 05 operator fact construction for every float-comparison selection skipped at `1503-1512`.
- Corpus fixtures proving compile acceptance for A1009/A1010/A1012 scenarios.
- `finalize_produced_types` and all consumers of invalid generic field types after `341-345`.
- Entire `resolve_elided_conformance_lifetimes` helper graph beyond cited branches (`collect_elision_calls`, conformance argument loop through `545`).

SLICE COMPLETE
