# slice_0022

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0165 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values.rs:2164-2170 | REACHABLE | `Access`/`ResidentAccess` (and non-reference-shape cases) yield `SeparateStorage`, so standing source qualifications are not published. |
| A0166 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs:231-254 | REACHABLE | `lower_carrier` always returns `Some(rows)`; empty rows make assignment/local planners return before planning the rest of the statement. |
| A0167 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs:1328 | REACHABLE | On boundary calls, erased formals skip lowering and `continue` instead of failing like non-boundary paths at `:1391`. |
| A0168 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs:2226-2234 | REACHABLE | Eight-hop `generated_from` chains and whole-program “any call passes this expression” make projection correspondence much easier to satisfy. |
| A0169 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs:4480-4489 | REACHABLE | Missing `expression_use` is treated as builtin; Boolean/scalar lowerers gate on that alone at cited sites. |
| A0170 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs:4388 | REACHABLE | Invalid symbols fall back to `members[0]` text vs `parameter.name` / `local.name`. |
| A0171 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs:10041-10060 | REACHABLE | Unresolved operand type skips custody subtree hit; plan walk retains the binding at VS:5442-5443. |
| A0172 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs:8119-8135 | REACHABLE | Non-interval declared domains `continue` without `push_integer(None)`, so the integer roster stays `Some` but omits that obligation. |

## A0165

**Digest (read):** area_09 Published facts — storage other than `Declared | View` needs no source qualification (`digests/area_09.md:205`).

**Cited site (read):** `values.rs:2157-2199`, function `reference_sources::qualification_contract`. Lines `2164-2170` match this checkout; not re-diffed with `git show 457adf4a11f:`.

When `storage` is not `CheckedReferenceStorage::Declared | View { .. }`, or `!validation::stores_reference_shape(program, result_type, &[])`, the function returns `CheckedReferenceSourceContract::SeparateStorage` (`values.rs:2164-2170`). Otherwise it builds `Standing { source_requirements, .. }` from `structural_result_qualification_contract` (`values.rs:2171-2198`).

**Storage values (read):** `CheckedReferenceStorage` includes `Access` and `ResidentAccess { .. }` besides `Declared` and `View` (`flow_terminal.rs:1195-1207`). Reference capture can set `storage` from `formation.interpretation.access_storage(...)` when `is_access_view` (`values.rs:1956-1978`, `:2119-2125`).

**Callers (read):** `rg 'reference_sources::qualification_contract' omega-rust` → 3 production call sites (`execution_terminal_unit_control_statement_sequence.rs:1646`, `execution_scalar_computations.rs:9232`, plus `execution_terminal_unit_state_graph.rs` via `qualification_contract_at_site` at `values.rs:2249`). Internal definition at `values.rs:2157`.

**Downstream (read):** `retain_reference_contract_types` no-ops on non-`Standing` contracts (`execution_terminal_unit_types.rs:3168-3176`). Stage-06 lowering returns `InvalidReferenceContract` for `SeparateStorage` (`attached_unit.rs:15832-15834`).

**Input (read):** A reference local or computation operand whose formation uses `Access` or `ResidentAccess` storage while the supplying type would require standing qualifications for a `Declared` reference with the same backing type.

**Verdict:** REACHABLE — stage 05 records `SeparateStorage` and omits standing qualification rows for those storage kinds; whether every such program is rejected only at later stages was not exhaustively traced.

## A0166

**Digest (read):** area_09 — atomic operand lowerer always `Some`, callers stop planning the statement (`digests/area_09.md:206`).

**Cited site (read):** `values_scalar.rs:208-255`, `atomic_operands::lower_carrier`. `231-254` match.

`declared_place_type_raw` → `primitive_type_reference` supplies `primitive_type`; operand loop `break`s on missing primitive or failed `u32::try_from(ordinal)` (`values.rs:231-235`). Partial rows may be pushed (`values.rs:236-252`). Function always ends with `Some(rows)` (`values.rs:254`).

**Callers (read):** `rg 'atomic_operands::lower_carrier' omega-rust` → 2 production sites (`values_scalar.rs:4624` in `expression_plans::assignment::plan`, `values_scalar.rs:4821` in `expression_plans::local_data::plan`).

**Caller behavior (read):** Both use `if let Some(carrier) = ... && let Some(operands) = lower_carrier(...) { ...; return; }` (`values_scalar.rs:4622-4647`, `:4820-4842`). Empty `operands` still satisfies `Some`, so planners `return` before assignment indices/value (`assignment::plan` `:4649+`) or other local initializer paths (`local_data::plan` `:4844+`).

**Input (read):** A statement with an atomic assignment or atomic-once local initializer where the carrier place does not yield a primitive type (or ordinal conversion fails on first operand).

**Verdict:** REACHABLE — scalar planning silently drops the remainder of that statement while compilation can continue with fewer or zero operand rows.

## A0167

**Digest (read):** area_09 — erased formals of boundary calls are skipped (`digests/area_09.md:207`, cites `1328`, `1394`).

**Cited site (read):** `values_scalar.rs:1245-1453`, `call_lowering::lower_call_arguments`. `is_boundary = call_is_boundary(program, target_symbol)` (`values_scalar.rs:1265-1265`, `1214-1242`).

For each formal, `strips_erased_parameter(target)?` (`values_scalar.rs:1327`). When true and `!is_boundary`, erased scalar or proof lowering runs (`values_scalar.rs:1328-1389`); unrecognized erased carriers hit `return None` (`values_scalar.rs:1390-1392`). When `is_boundary`, the entire `if !is_boundary { ... }` block is skipped and execution hits `continue` (`values_scalar.rs:1394`), advancing to the next formal without recording erased scalar or proof terms.

**Callers (read):** `rg '\blower_call_arguments\b' omega-rust` → 7 symbol hits in `values_scalar.rs` (definition plus nested-call capture `:979`, `call_statement::plan` `:4731`, `StatementPlanner::retain_expression_call_arguments` `:5313`, and re-exports/imports).

**Input (read):** A boundary-target call (trait boundary, bodyless machine state, boundary operator, or exact external leaf per `call_is_boundary`) with erased proof-only or erased-scalar formals that would make a non-boundary call return `None` at `:1391`.

**Verdict:** REACHABLE — the same call site can still produce `Some(LoweredCallArguments)` without erased-slot evidence that a non-boundary callee would require.

## A0168

**Digest (read):** area_09 — proof symbols correspond within eight hops; projection kept if any call anywhere passes the expression as an argument (`digests/area_09.md:208`).

**Cited sites (read):** In `call_lowering`, `erased_payload_projection` sets `corresponds` via `proof_symbols_correspond`, `erased_generic_carriers_correspond`, `projection_result_matches`, or `projection_instantiated_by_call` (`values_scalar.rs:2032-2041`). `proof_symbols_correspond` is reflexive or either-directed `proof_symbol_derives_from` (`values_scalar.rs:2226-2234`). `proof_symbol_derives_from` walks at most eight `generated_from` links (`values_scalar.rs:2364-2385`). `projection_instantiated_by_call` scans **all** machines, states, and statements (`values_scalar.rs:2073-2093`); `projection_statement_instantiates` accepts calls/transitions whose argument list **contains** the projection expression (`values_scalar.rs:2096-2150`, `:2124-2133`).

**Callers (read):** `rg '\bprojection_instantiated_by_call\b' omega-rust` → 2 hits (`values_scalar.rs:2035` call, `:2054` definition). `lower_proof_term` / erased paths invoke `erased_payload_projection` (read `values_scalar.rs:1931+` region).

**Input (read):** An erased proof projection actual where unrelated proof-index symbols chain within eight `generated_from` steps, or some distant call in another machine passes the same expression handle as a call argument with index alignment via `proof_symbols_correspond` on formals.

**Verdict:** REACHABLE — correspondence and instantiation tests are intentionally weak relative to call-site-local evidence.

## A0169

**Digest (read):** area_09 — no recorded operator use counts as builtin; lowerers at VS:488, 496, 544, 9333 rely on it alone (`digests/area_09.md:209`).

**Cited site (read):** `values_scalar.rs:4480-4489`, `expression_facts::operator_is_builtin`. `operators.expression_use(expression).is_none_or(|u| u.status == BuiltinFallback)` returns true when there is **no** indexed use row or status is `BuiltinFallback`.

**`expression_use` (read):** Returns `None` when the expression is absent from `ExpressionUseIndex` (`operators_and_proof.rs:1176-1191`).

**Cited lowerer gates (read):** `boolean_lowering::GuardEvaluator::expression` arms at `values_scalar.rs:486-553` (`operator_is_builtin` on not/compare/and/or). `scalar_lowering::entry_lowering` at `values_scalar.rs:9326-9353` (bitwise not and all matching binaries when `operator_is_builtin`). Digest also cites `544` (read: and/or arm at `:542-553`) and `9333` (read: binary arm at `:9333`).

**Operator facts (read):** `build_operator_facts` builds uses via `build_operator_uses(program, values)` over collected value origins (`operators.rs:523-528`). Whether every authored operator expression appearing in scalar plans always gains a use row was not proved.

**Input (read):** A binary or unary operator expression present in the scalar/boolean plan walk but missing from `CheckedOperatorFacts` uses (or only `BuiltinFallback`), where a user overload should have been selected.

**Verdict:** REACHABLE — lowering treats the expression as builtin arithmetic/logic solely from `operator_is_builtin`.

## A0170

**Digest (read):** area_09 — name with no symbol resolves to parameter/local of same spelling (`digests/area_09.md:210`, also `4417`, `5619-5622`).

**Cited site (read):** `values_scalar.rs:4370-4393`, `expression_facts::parameter_position`. With a single name-path member, position is `parameters.iter().position` matching valid `path.symbol`, else valid `path.head_symbol`, else `parameter.name.as_str() == members[0].as_str()` (`values_scalar.rs:4380-4389`).

**Companion (read):** `local_position` uses the same symbol/head pattern, else compares `local.name` to `display_name(expression)` (`values_scalar.rs:4395-4422`). Boolean `lower` at `5619-5622` matches `name_symbol` or `name_text` vs `parameter.name`.

**Callers (read):** `rg '\bparameter_position\b' omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs` → 71 symbol hits (definition plus scalar/boolean lowering and plans).

**Input (read):** A `Name` path with invalid symbols but display/member text equal to a parameter or local name, including shadowing cases where symbol identity would disagree.

**Verdict:** REACHABLE — binding can follow spelling alone without a valid symbol handle.

## A0171

**Digest (read):** area_09 — cast whose operand type cannot be resolved counts as not needing custody and keeps plain plan row (`digests/area_09.md:211`, with `VS:5442-5443`).

**Cited site (read):** `values_scalar.rs:10041-10060`, `semantic_casts::requires_custody`. `program.subtree_any` visits `ExpressionNode::Cast` when `!cast.semantic_domain.is_empty()` **or** `result_type(program, state, cast.value)` is `Some` and `has_declared_domains(program, reference)` (`values_scalar.rs:10050-10056`). `result_type` needs `find_state` and `expression_result_type_reference` (`values_scalar.rs:9949-9956`).

**Plan retention (read):** After the walk, `build_scalar_expression_plans` drops expressions tied to bindings where `requires_custody` is true (`values_scalar.rs:5435-5441`); otherwise bindings append to `retained_bindings` (`values_scalar.rs:5442-5444`).

**Callers (read):** `rg '\brequires_custody\b' omega-rust` → 2 files, 6 total hits (`values_scalar.rs:2`, `execution_scalar_computations.rs:4`).

**Input (read):** A cast subtree where the operand has undeclared domains and `expression_result_type_reference` fails in that state (so `result_type` is `None`), while `cast.semantic_domain` is empty—so the cast never marks custody but still plans as an ordinary scalar expression.

**Verdict:** REACHABLE — semantic-cast custody filtering is bypassed and the plain plan row is kept.

## A0172

**Digest (read):** area_09 — non-interval domain constraint adds no slot and does not mark roster incomplete (`digests/area_09.md:212`).

**Cited site (read):** `values_scalar.rs:8069-8145`, `parameter_ranges::lower_callable_parameter_range_requirements`. For `TypeConstraintNode::Domain` with integer primitive, the code requires `exact_declared_domain_carrier_interval` (`values_scalar.rs:8119-8128`). On failure it **`continue`s** (`values_scalar.rs:8129-8131`) without calling `push_integer(None)` (contrast `push_integer` at `values_scalar.rs:8170-8173`, which sets `integer_entry_ranges = None`).

Roster starts as `integer_entry_ranges: Some(Vec::new())` (`values_scalar.rs:8075-8076`).

**Consumers (read):** `rg '\blower_callable_parameter_range_requirements\b' omega-rust` → 3 hits (`values_scalar.rs` definitions/wrapper, `published_facts.rs:3262` into `ClosedScalarValueContractPlan.with_integer_entry_ranges`).

**Input (read):** A callable parameter whose constrained type carries a declared domain that is not an exact integer interval (so `exact_declared_domain_carrier_interval` returns `None`), while other parameters still populate the roster.

**Verdict:** REACHABLE — the obligation is omitted but `integer_entry_ranges` remains `Some`, so consumers see a complete roster missing that bound.

## What I did not read

- Corpus witnesses for each failure mode; no compile or corpus run.
- Full `build_operator_uses` coverage proof vs every executable operator expression.
- Exhaustive stage-05 diagnostic paths for silent plan gaps (digest family 62 at `area_09.md:264`).
- All `execution_scalar_computations` / contract-check consumers of `integer_entry_ranges` and partial rosters.
- Whether `SeparateStorage` reference plans always die in stage 06 before shipped acceptance.

SLICE COMPLETE
