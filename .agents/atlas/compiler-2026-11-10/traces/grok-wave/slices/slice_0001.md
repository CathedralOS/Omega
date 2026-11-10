slice_0001

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0001 | preparation.rs:47-61 | CLOSED | Stage 05 `validate_symbolic_array_length` still diagnoses unset fixed-array lengths after `normalize` skips. |
| A0002 | preparation.rs:138 | CLOSED | Only two recipe positions are meant to get evaluation contexts here; `ArrayLength`/`None` use other passes. |
| A0003 | preparation.rs:1452 | CLOSED | Stage 05 `finalize_checked_authored_selections` rejects non-toolchain selections still `LateBound`. |
| A0004 | selection_and_reflection.rs:1087, :1103 | UNDECIDED | Name-only `position` match is real; did not prove a bad bind survives `validate_static_machine_bindings`. |
| A0005 | borrow.rs:4765-4775, :4891-4893, :7764, :7801-7803 | UNDECIDED | Empty loan rosters on `TemplateDependent` are real; did not trace borrow overlap checks for accept. |
| A0006 | borrow.rs:1475-1479 | UNDECIDED | `Ok(None)` when frames/schedule missing is real; did not prove checking completes without custody rows. |
| A0007 | borrow.rs:1058, :1174 | REACHABLE | `implicit_loan` records `Read` even when the formal type is an exclusive mutable reference. |
| A0008 | borrow.rs:2452-2519 | CLOSED | `source_type` `None` makes `output_input_accesses` and resource planning fail closed, not accept. |

### A0001

1. Read `array_extents::normalize` in full (`preparation.rs:15-66`, read): on failure at lines 47-48, 52-53, 56-57, or 59-60 it `continue`s without `set_fixed_array_length`.
2. Search `rg 'array_extents::normalize' /agent/repos/omega/omega-rust` → 3 hits, all `preparation.rs:2033`, `:2095`, `:2119` (read call sites in `prepare_typed_trees`, `prepare_execution_applications`, `prepare_constant_execution`).
3. Read callers through `lower_typed_trees` → `check_program` → `prepare_typed_program` (`checking.rs:1047-1055`, read) → `prepare_specialized_program` (`program_validation.rs:3970-3985`, read) → `validate` over type nodes.
4. Read `validate_symbolic_array_length` (`type_references_and_wire.rs:1564-1660`, read) invoked from `TypeReferenceNode::FixedArray` validation (`type_references_and_wire.rs:5164-5175`, read): still-`Expression` lengths get formation/type diagnostics (e.g. `:1576-1578`, `:1657-1659`).
5. **Input reaching the accepting branch:** fixed-array type references whose extent expression never closed in `normalize` but remain as `FixedArrayLength::Expression`.

**Verdict:** CLOSED — every path to checked trees goes through `prepare_specialized_program` validation, which records errors for unclosed or ill-typed array extents instead of accepting a silent literal.

**Unsure:** Whether `admits_pending_inputs` or build-time-only paths skip the same `FixedArray` validation (not read past `checking.rs:789-791` comment).

### A0002

1. Read `constant_entries::materialize` in full (`preparation.rs:75-170`, read).
2. At `preparation.rs:130-138` (read), `live` is true only for `RetainedConstantPosition::ContractExpression` and `ConstApplication`; `_ => false` for `None`, `ArrayLength`, and any other variant (`access_views_and_commitments.rs:1847-1857`, read defines four variants).
3. Contexts with `live == false` hit `continue` at `:141-145` (read) — no `evaluation_entry` from this loop.
4. Search `rg 'constant_entries::materialize'` → hits in `preparation.rs` specialization loop (`:2009`, read); not re-traced beyond `prepare_typed_trees`.
5. Read separate handling: `ArrayLength` recipes tied via `constant_recipe_for_array_length` (`access_views_and_commitments.rs:2026-2036`, read); extent closing via `array_extents::normalize` (A0001).

**Input reaching the accepting branch:** constant recipe contexts whose `RetainedConstantPosition` is not `ContractExpression` or `ConstApplication`.

**Verdict:** CLOSED — omission is positional routing; `ArrayLength`/`None` are not expected to gain contexts in this function.

**Unsure:** Whether a `None` position recipe with live contexts could be needed and never materialized (only read enum, not all recipe producers).

### A0003

1. Read `bind_pre_specialization_authored_selections` in full (`preparation.rs:1365-1479`, read).
2. At `:1408-1453` (read), `target` match can be `None`; `:1452-1453` `continue`s without finalizing — selection stays `LateBound`.
3. Search `rg 'bind_pre_specialization_authored_selections' /agent/repos/omega/omega-rust` → 6 path hits (`preparation.rs:1972`, `:2113`, lib re-export, build-time-evaluation probes; read `prepare_typed_trees` at `:1972`).
4. Read upward in stage 05: `finalize_checked_authored_selections` (`shared.rs:973-1426`, read) scans remaining `LateBound` selections; at `:1385-1422` (read) returns `Diagnostic::error` for non-toolchain unresolved selections.
5. `checking.rs:1219` (inferred from grep) wires finalized selections into checked program; did not read full `lower_typed_trees` body past `:1012`.

**Input reaching the accepting branch:** authored declaration selection occurrences still `LateBound` after stage 04 binding pass when resolution `target` is `None`.

**Verdict:** CLOSED — stage 05 finalization errors on unresolved non-toolchain late-bound selections before accept.

**Unsure:** Toolchain-origin late-bound selections explicitly allowed until build-time (`checking.rs:789-791`, read only comment).

### A0004

1. Read `infer_static_bindings` in full from `selection_and_reflection.rs:1066` through recursive cases to `:1203` (read); cited lines `:1082-1088` and `:1097-1104` (read) include `parameter_name == name.as_str()` without symbol agreement.
2. Search `rg 'infer_static_bindings' /agent/repos/omega/omega-rust` → 15+ hits (monomorphization re-export, `contextual_and_requirement_calls.rs`, `constructed_results.rs`, `selection_and_reflection.rs`; count mode not run separately — listing hits suffice).
3. Read proposal merge in same file (`:746-764`, read): same parameter index conflicts set `selection.conflicted` via `same_type_identity`, not symbol identity.
4. Read `monomorphization.rs:3841-3864` (read): `selection.conflicted` with partial evidence pushes diagnostics — only when types conflict, not when wrong index shares a name.
5. Did not read `validate_static_machine_bindings` (`machine_parameters_and_bindings.rs:7560+`) body beyond signature.

**Input reaching the accepting branch:** required `Named` type actual pair where template parameter index is chosen by spelling match despite differing valid `symbol` handles.

**Verdict:** UNDECIDED — name-only binding at `:1087`/`:1103` is confirmed (read); whether a program that should be rejected still monomorphizes and passes checking requires reading `validate_static_machine_bindings` and specialization apply paths.

**Unsure:** Whether templates can expose two distinct binders with identical spellings (would decide REACHABLE vs CLOSED).

### A0005

1. Read `stored_calls::roster_at_expression` (`borrow.rs:4748-4812`, read): `TemplateDependent` with `independent_owned_formation` → `Ok(Vec::new())` at `:4765-4774`.
2. Read `independent_owned_formation` (`:4821-4873`, read) in full.
3. Read `stored_calls::loans` (`:4875-4897`, read): `TemplateDependent` → `Ok(Vec::new())` at `:4891-4893`.
4. Read reference-local formation (`:7754-7803`, read): `TemplateDependent` → `continue` or `Ok(Vec::new())`.
5. Did not read `checks_borrows.rs` / overlap consumers for statements whose loan vector is empty due to template dependence.

**Input reaching the accepting branch:** call-site or local initialization where `CallFrameResolver` reports `StoredReferenceRelationRefusal::TemplateDependent`.

**Verdict:** UNDECIDED — empty rosters are real (read); whether a borrow-rule violation can pass checking without those loans needs borrow-check trace not done here.

**Unsure:** Whether template-dependent frontiers always force diagnostics elsewhere (`checks_borrows.rs:441`, read snippet only).

### A0006

1. Read `append_reference_custody` (`borrow.rs:1463-1479`, read opening; read through `:1500` for error arms).
2. At `:1475-1478` (read), `frames` missing or `stored_reference_call_contexts` `None` → `Ok(None)` (no custody rows).
3. Search `rg 'append_reference_custody' /agent/repos/omega/omega-rust` → 2 hits: definition and `borrow.rs:8685` (read caller in state borrow fact loop).
4. Read `build_borrow_facts` (`:9437-9466`, read): accepts `call_frames: Option`; comment at `:9434-9435` says `None` retains failed preparation.
5. Read `build_check_facts` (`published_facts.rs:10982-10983`, read): `CallFrameResolver::new(program)` passed as `Option`; `CallFrameResolver::new` (`calls.rs:15337-15348`, read) returns `Some` when `CallFrameTables::build` succeeds — did not read `build` failure modes.

**Input reaching the accepting branch:** statements processed while `CallFrameResolver` is `None` or while `stored_reference_call_contexts` returns `None`.

**Verdict:** UNDECIDED — early `Ok(None)` is confirmed (read); did not prove a complete check runs with missing custody and no later diagnostic.

**Unsure:** When `stored_reference_call_contexts` returns `None` for reachable stored-reference calls (only opened `calls.rs:14537`, not read body).

### A0007

1. Read `collect_target_argument_accesses` (`borrow.rs:1027-1066`, read) and `collect_formal_argument_evaluation` (`:1095-1175`, read) in full.
2. At `:1049-1058` (read), missing parameter metadata → `receiving_type` `unwrap_or_default()` (invalid handle).
3. At `:1150-1165` (read), `implicit_loan` branch sets `Some(BorrowAccessKind::Read)` regardless of formal mutability; uses `formal` only in `shared_argument_referent`, not for access kind.
4. Read caller `collect_read_accesses` call path (`:718-727`, read): runs for `ExpressionNode::Call` with `validation::call_target_symbol` — unresolved targets can yield invalid `target_symbol` while descriptor operand types may still supply a `&mut` formal via `:1036-1056`.
5. Did not read `validation::call_target_parameters` or `global_descriptor_table_operand_types` bodies.

**Input reaching the accepting branch:** call argument evaluation where formal type is an exclusive mutable reference but the implicit-loan path runs (not explicit `Borrow` expression).

**Verdict:** REACHABLE — a call whose callee operand types are known from the descriptor table while static target resolution is incomplete could record read access instead of mutate for a `&mut` formal, weakening the ledger.

**Unsure:** How often `shared_argument_referent` succeeds with invalid `target_symbol` (not read validation helpers).

### A0008

1. Read `lifetime_contract::source_type` in full (`borrow.rs:2446-2616`, read).
2. Early `None` returns throughout (`:2452-2456`, `:2460-2465`, `:2467-2469`, `:2490-2492`, `:2504-2505`, `:2513-2520`, `:2526-2527`, `:2530-2531`, `:2562-2564`, incomplete relation, etc.; read).
3. Search `rg 'lifetime_contract::source_type' /agent/repos/omega/omega-rust` → 1 direct call site in `checks_borrows_resources.rs:6679` plus internal uses in `borrow.rs:2778`, `:2914` (grep/read).
4. Read `retained_inputs` (`:2772-2823`, read): `source_type?` → whole function `None`.
5. Read `output_input_accesses` (`:2898-2916`, read): `source_type(...).is_none()` → `Err("output lifetime input does not match...")`.
6. Read `checks_borrows_resources.rs:6679-6687` (read): `source_type(...).ok_or(...)?` — propagates error.

**Input reaching the accepting branch:** `FlowOperandRetainedSupplier` edge where contract/operand/call correspondence fails any `source_type` guard (returns `None`).

**Verdict:** CLOSED — consumers on the checked path turn `None` into errors or aborted retention, not silent acceptance.

**Unsure:** Whether any `retained_inputs` caller treats `None` as success without diagnostic (only read `flow_retained_borrows.rs:603-605` fragment).

### What I did not read

- Full `validate()` / `validate_static_machine_bindings` bodies beyond entry points.
- `checks_borrows.rs` overlap/conflict paths for empty template-dependent loan rosters (A0005).
- `stored_reference_call_contexts` full logic when it returns `None` (A0006).
- `CallFrameTables::build` failure conditions.
- Remaining `infer_static_bindings` downstream specialization for A0004 beyond conflict flags at `selection_and_reflection.rs:746-818`.

SLICE COMPLETE
