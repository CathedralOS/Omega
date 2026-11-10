slice_0061

| entry id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0465 | float_and_mathematical_contracts.rs:491-498 | REACHABLE | Non-parameter literals in ensures predicates count as input-only, so semantic contract recognition can succeed without operator-formal binding. |
| A0466 | float_and_mathematical_contracts.rs:557-609 | REACHABLE | The ensures scan returns on the first matching equality and never reconciles against later ensures facts. |
| A0467 | float_and_mathematical_contracts.rs:1451-1459,1523,1525 | REACHABLE | Binary float equalities with only one projection side produce no error and no recorded proposition. |
| A0468 | float_and_mathematical_contracts.rs:2381-2383 | REACHABLE | On a call-cycle, `logical_body` returns true without re-checking callee structure. |
| A0469 | float_and_mathematical_contracts.rs:2633 | REACHABLE | The `_ => true` arm admits bare `Name` and other unlisted expression kinds without parameter or local checks. |
| A0470 | expression_types_and_fields.rs:305-312,354-356 | REACHABLE | Missing operand or target primitive types, or `None` from `float_coercion_preserves_denotation`, skips denotation diagnostics. |
| A0471 | expression_types_and_fields.rs:36-38 | CLOSED | Recast casts are validated in `recasts::validate_recasts` during program prologue, not in `validate_cast_types`. |
| A0472 | expression_types_and_fields.rs:484-492,2117-2127 | REACHABLE | Unknown operand primitive types cause early return from float-format checks with no diagnostic. |

### A0465

1. Read (in full): `semantic_term_uses_only_inputs` at `float_and_mathematical_contracts.rs:475-531`; read (partial): enclosing `float_result_semantic_contract` `331-432`, `float_scalar_semantic_contract` `543-610`.
2. Search: `rg '\bsemantic_term_uses_only_inputs\b' omega-rust` — **4 hits** (definition plus three internal call sites at `422`, `522`, `599`; no external symbol callers).
3. Upward trace (read): `float_result_semantic_contract` / `float_scalar_semantic_contract` → `machine_calls_core.rs:5380-5385` (`selected_operator_has_closed_value_denotation` requires sealed float operator plus contract `Some`) → pipeline `checks_operators.rs:538` (`float_boolean_semantic_contract`).
4. Input on accepting branch: an `ExpressionNode::Float/Integer/Boolean` literal anywhere in the ensures predicate spine walked by `semantic_term_uses_only_inputs`, including literals that are not operator parameter symbols (`491-493`, names only checked at `494-502`).
5. **Verdict: REACHABLE** — A boundary float operator whose ensures equality decorates a semantic application with a free numeric literal (not a formal) can still satisfy the input-only test and yield `Some` from contract recognizers, admitting closed denotation and boolean semantic paths that should require formal-only references.
6. Unsure: whether non-toolchain operators can reach `selected_operator_has_closed_value_denotation` in practice (seal check at `705-709` read); literal acceptance in sealed toolchain sources is intentional but the code path is unconditional.

### A0466

1. Read (in full): `float_scalar_semantic_contract` `543-610`; read (partial): `float_result_semantic_contract` loop `339-432` (same first-match structure).
2. Search: `rg '\bfloat_scalar_semantic_contract\b' omega-rust` — **3 hits** (`float_and_mathematical_contracts.rs:543,613,614`; `machine_calls_core.rs:5384`).
3. Upward trace (read): `machine_calls_core.rs:5330-5386`; `checks_operators.rs:538` via `float_boolean_semantic_contract` `534-541`.
4. Input: ordered `ensures_expression_facts` for an operator; first equality where `reserved_result_owner` matches, `float_semantic_application_for_parameters` succeeds, and `semantic_term_uses_only_inputs` passes triggers `return Some(predicate)` at `606` without examining remaining ensures.
5. **Verdict: REACHABLE** — An operator with two competing ensures equalities where an earlier, weaker predicate matches first is bound while a later, stricter ensures fact is ignored, affecting denotation closure and boolean semantic term selection.
6. Unsure: did not read `proof_facts::ensures_expression_facts` ordering guarantees.

### A0467

1. Read (in full): `walk_expression` `1435-1539`; read (partial): `collect_float_meaning_projection_invocations` `1542-1576`.
2. Search: `rg '\bwalk_expression\b' float_and_mathematical_contracts.rs` — **7 hits** (all internal); `rg '\bcollect_float_meaning_projection_invocations\b' omega-rust` — **2 hits** (`1542`, `program_validation.rs:4613`).
3. Upward trace (read): `program_validation.rs:4612-4613` (errors propagate from `walk_expression` only on explicit `Err`; ignored equalities stay `Ok`); `published_facts.rs:11014-11018` binds only collected propositions.
4. Input: proof-fact expression with `BinaryOperator::Equal` where `float_meaning_operands` closure at `1451-1458` returns `None` because only one side is a collected projection (`1452-1457`).
5. **Verdict: REACHABLE** — A program proof fact asserting float-meaning equality between a projection and a non-projection expression completes validation with no diagnostic and no `ValidatedFloatMeaningEqualityProposition` row, so the ill-formed equality is neither rejected nor bound.
6. Unsure: did not read `bind_float_meaning_projection_facts` body for whether missing rows always imply unsound admission.

### A0468

1. Read (in full): `logical_body` `2375-2388`, `logical_body_contents` `2390-2534` (partial on transition arms); read (partial): `is_indexed_logical_machine` `2268-2308`.
2. Search: `rg '\blogical_body\(' omega-rust/psi/compilation/validation` — **5 hits** (all in `float_and_mathematical_contracts.rs` at `2289`, `2375`, `2446`, `2520`, `2609`).
3. Upward trace (read): `is_indexed_logical_machine` fixed-point filter `2286-2290`; consumers include `proof_contracts_core.rs:10147-10149` (skips erased-generic checks when indexed logical), `machine_calls_core.rs:6437`, `contract_entailment.rs:90362`.
4. Input: callee machine symbol already on `stack` when `logical_body` re-enters at `2381-2382`, returning `true` before `logical_body_contents` runs.
5. **Verdict: REACHABLE** — Mutually recursive proof machines can satisfy callee checks via the cycle shortcut and land in `indexed_logical_machines`, exempting them from checks that apply to non-indexed machines, without verifying the cyclic callee’s body constraints.
6. Unsure: did not read full fixed-point convergence behavior for all specialization cases.

### A0469

1. Read (in full): `logical_expression` `2574-2644`.
2. Search: `rg '\blogical_expression\(' omega-rust/psi/compilation/validation` — **8 hits** (all internal statement/argument walks in `logical_body_contents`).
3. Upward trace (same as A0468 through `logical_body_contents` → `is_indexed_logical_machine`).
4. Input: `ExpressionNode::Name` or any node not explicitly listed with `false` or specialized arms hits `_ => true` at `2633`, then child walk at `2638-2643`.
5. **Verdict: REACHABLE** — A checked-body proof machine whose statement is a bare name reference that is not a logical parameter, local, or evidence initializer still passes `logical_expression`, contributing to indexed-logical-machine admission.
6. Unsure: whether typed trees reject such programs earlier (not read in typed-trees stage).

### A0470

1. Read (in full): `validate_exact_float_coercion` `295-400`; read (partial): `validate_cast_types` `27-59`; `float_coercion_preserves_denotation` `402-451`.
2. Search: `rg '\bvalidate_exact_float_coercion\b' omega-rust` — **3 hits** (`expression_types_and_fields.rs:295,49,6578`; `calls.rs:2644`); `validate_cast_types` — **3 hits** (`27`, `6576`, `calls.rs:2754`).
3. Upward trace (read): `calls.rs:2744-2762` state scan invokes `validate_cast_types` then `validate_range_cast_at_use` (latter not read in full).
4. Input: `expression_result_type_reference` returns `None` → early `return true` at `307-308`; or `float_coercion_preserves_denotation` returns `None` → `return true` at `354-355` without pushing the error block at `373-397`.
5. **Verdict: REACHABLE** — A cast to `F32`/`F64` (or cross-format coercion) on an operand whose primitive type cannot be resolved, or whose coercion class returns `None`, bypasses exact float denotation errors that `373-397` would emit when `proven` is `Some(false)`.
6. Unsure: extent to which `validate_range_cast_at_use` or `report_invalid_numeric_cast_source` (read only at `144-151` in `validate_cast_types`) closes all cast shapes.

### A0471

1. Read (in full): `validate_cast_types` opening `27-38`; read (partial): `recasts::validate_recasts` `5292-5335`, `program_validation.rs:569-574`.
2. Search: `rg '\bvalidate_cast_types\b'` — callers `calls.rs:2754` only (read); recast path `is_recast()` early return `36-37`.
3. Upward trace (read): program prologue runs `recasts::validate_recasts` for every machine state before body scan (`program_validation.rs:573`).
4. Input: any `cast.form.is_recast()` occurrence in `validate_cast_types`.
5. **Verdict: CLOSED** — Every path through `validate_cast_types` on a recast returns immediately, but `recasts::validate_recasts` at `program_validation.rs:573` (read) and `judge_scalar_recast` at `recasts.rs:5324` (read) still run the representation recast judgment and push diagnostics on failure.
6. Unsure: none material.

### A0472

1. Read (in full): `validate_binary_float_formats` `459-512`; read (partial): `source_primitive` / `report_mismatch` `2096-2183`; caller `validate_binary_operand_types` `1512-1533`.
2. Search: `rg '\bvalidate_binary_float_formats\b' omega-rust` — **3 hits** (`459`, `527`, `1527`); `validate_float_expression_formats` additionally at `frozen_program_and_queries.rs:2906`, `arithmetic_domains.rs:24323,25291` (not read in full).
3. Upward trace (read): `validate_binary_operand_types` maps operands with `expression_result_type_reference` then calls `validate_binary_float_formats`; `report_mismatch` used from `2099` path (read `2589` reference only via grep).
4. Input: `operands` array contains `None` (`484-486`) or primitive resolution fails (`487-491`); `source_primitive` returns `None` so `report_mismatch` emits nothing (`2117-2127`).
5. **Verdict: REACHABLE** — A builtin float binary with mixed `F32`/`F64` operands where either operand’s type reference cannot be computed skips the format-mismatch error that `508-511` would emit when both primitives are known and incompatible.
6. Unsure: did not read all `validate_float_expression_formats` call sites for redundant checks.

### What I did not read

- `bind_float_meaning_projection_facts` implementation in `published_facts.rs` beyond call site `11014-11018`.
- Full `validate_range_cast_at_use` and remaining `validate_cast_types` arms after line `155`.
- `proof_facts::ensures_expression_facts` ordering.
- Full bodies of all `is_indexed_logical_machine` consumer functions beyond cited lines.
- `typed_trees::operator::is_sealed_toolchain_float_operator` definition (only wrapper read at `705-709`).
- Entire `logical_body_contents` transition and assembly statement arms.
- `validate_float_expression_formats` callers in `arithmetic_domains.rs` and `frozen_program_and_queries.rs` beyond grep hits.

SLICE COMPLETE
