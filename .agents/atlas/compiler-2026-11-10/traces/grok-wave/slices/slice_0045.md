slice_0045

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0341 | arithmetic_domains.rs:15933 | CLOSED | Sole production use pairs `integer_bound_terms_equal` with `integer_bound_offset` in structural bound helpers before overlap decisions. |
| A0342 | arithmetic_domains.rs:23738-23744 | REACHABLE | Stored-integer layout validation is skipped with no diagnostic when the destination primitive does not accept integer literals, even though other checks may still run. |
| A0343 | borrow_and_reflection.rs:2045 | UNDECIDED | `collect_type` treats `DynamicTrait` as complete without collecting borrow leaves; whether a typed `dyn` can carry lifetimes was not established. |
| A0344 | borrow_and_reflection.rs:2877-2879, 3004-3006 | UNDECIDED | Both region matchers return `true` when the required lifetime name is absent; whether elided requirements intentionally accept any region was not traced to program acceptance. |
| A0345 | call_cycles.rs:2765 | UNDECIDED | Non-transition `Assembly`/`AssemblyFact` statements bypass rank-preservation failure; whether inline assembly can mutate rank coordinates was not read. |
| A0346 | call_cycles.rs:2624-2626 | UNDECIDED | A member with no matching body witness skips per-statement rank checks; whether that can coincide with `topology.judge` accepting the component was not fully traced. |
| A0347 | call_cycles.rs:620-621 | CLOSED | `unranked_component_reached_from` only runs with a machine index and adjacency built in lockstep with `program.machines()`; an out-of-range index yields `None`, treated as “no unranked component.” |
| A0348 | calls.rs:2062-2068 | REACHABLE | `report_unresolved_value_call` returns silently when the target is `encode`/`decode` and the receiver spelling matches any data definition, without resolving the method. |

### A0341

1. Read `integer_bound_terms_equal` in full at `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:15933-15980` (read): `(IntegerBound::Integer(_), IntegerBound::Integer(_)) => true` regardless of literal value; `Symbol` arms compare `symbol` only and use `..` on `offset` (read `15938-15946`).
2. Search `rg '\binteger_bound_terms_equal\b' /agent/repos/omega/omega-rust` → 4 hits (read): definition and re-exports in `arithmetic_domains.rs` and `lib.rs`; import in `checks_borrows_overlap.rs:490`.
3. Read production call sites in `checks_borrows_overlap.rs:779-831` (read): `structural_bound_is_at_or_before`, `structural_bound_is_strictly_before`, and `structural_bound_equal` each require `bound_terms_equal(...)` **and** offset comparison via `bound_offset` (`780-784`, `803-807`, `827-831`).
4. Read callers of those helpers: `bound_is_at_or_before` / `bound_equal` feed `index_expressions_may_overlap` and related borrow-overlap selectors (`checks_borrows_overlap.rs:601-653`, `768-832`) (read); `prove_integer_ordering` in `arithmetic_domains.rs:15512+` uses `bound_shift` (`15861-15930`), not `integer_bound_terms_equal` (read).
5. Input reaching the accepting branch: pairs of `IntegerBound` values passed into `bound_terms_equal` inside structural helpers during index overlap analysis in checked-tree borrow compatibility.
6. **Verdict: CLOSED** — Every production path combines term equality with offset checks (`checks_borrows_overlap.rs:780-831`); mismatched integer literals or symbol offsets do not alone prove order or equality.
7. Unsure: nothing material beyond spec intent of `Integer(_) => true` as “same term kind.”

### A0342

1. Read `validate_stored_integer_write` guard at `arithmetic_domains.rs:23734-23744` (read): returns empty `Vec` when `!destination_primitive.is_some_and(|p| p.accepts_integer_literal())` **or** when no layout plan in the program has nonempty `integer_fields`, or when `state` is `None` (`23746-23747`).
2. Search `rg '\bvalidate_stored_integer_write\b' omega-rust` → 3 hits (read): definition, re-export, caller `program_validation.rs:1919`.
3. Read caller `program_validation.rs:1851-1932` (read): `check_narrowing_assignment` runs before the call; empty writes are only extended into `stored_integer_writes` with no extra diagnostic.
4. Input: an integer-valued assignment whose declared destination primitive does not `accepts_integer_literal()`, while the assignment path still reaches stored-integer write validation.
5. **Verdict: REACHABLE** — A program that stores an integer into a place whose declared primitive rejects integer literals can pass this function with no stored-integer rows and no error from this site, even if layout-backed integer storage might need checking elsewhere.
6. Unsure: whether a later layout or typing gate always rejects the same assignment before acceptance (not traced past `check_narrowing_assignment`).

### A0343

1. Read `collect_type` at `borrow_and_reflection.rs:1477-2047` (read through the `match`); `TypeReferenceNode::DynamicTrait { .. } => true` at `2045-2046` returns success without recursing into arguments or pushing `CarriedLifetime` entries.
2. Read `substituted_result_is_view_free` at `1134-1157` (read): success requires `collect_type(...) && output.is_empty()`.
3. Search `rg 'substituted_result_is_view_free' omega-rust` → hits in `checks_borrows.rs:6465`, `execution_terminal_unit_control_statement_sequence.rs:3486`, etc. (read list only); read `checks_borrows.rs:6464-6476` — view-free locals skip a backing-survival check.
4. Input: a `TypeReferenceHandle` whose node is `DynamicTrait`, evaluated for view-freedom after a call returning that type.
5. **Verdict: UNDECIDED** — If the language allows a `dyn Trait` value that actually carries borrow leaves, this site would classify it view-free; no read established whether such a type can be formed or reach checking.
6. Unsure: whether `DynamicTrait` type arguments can encode borrowed data (entry note matches digest `area_13.md:384`).

### A0344

1. Read `infer_inputs` closure `constrain` at `borrow_and_reflection.rs:2870-2938` (read): `let Some(required) = required else { return true; }` at `2877-2879`.
2. Read `region_correspondence::validate` matcher at `2987-3030` (read): same pattern at `3004-3006` when `required` lifetime name is `None`.
3. Search `rg 'infer_inputs|prepare_callable_inputs' borrow_and_reflection.rs` (read): `prepare_callable_inputs` at `205-244` calls `infer_inputs`; used from `PreparedCallableBorrowRefinement::prepare` (`256-262`) and `validate_callable_borrow_refinement` (`518-524`); `machine_parameters_and_bindings.rs:559` calls `prepare` (read line only).
4. Input: callable borrow refinement where a required reference has no named lifetime (`required: None` in the matcher) while the actual side carries a concrete region.
5. **Verdict: UNDECIDED** — The accepting branch is reachable in code, but callers were not read through to a compilation accept/reject decision, and elided-lifetime semantics may intend this behavior.
6. Unsure: whether `region_correspondence::validate` later rejects cases that `infer_inputs` accepted (did not read full `validate` body past `3031`).

### A0345

1. Read `project_component` non-transition branch at `call_cycles.rs:2742-2805` (read): after failed `preserves_rank`, `StatementNode::Assembly(_) | AssemblyFact(_) => continue` at `2764-2765`.
2. Read `prefix::preserves_rank` at `1320-1448` (read): `match statement` has no `Assembly` arm; falls through `_ => false` at `1447-1448`.
3. Read caller `RuntimeRankComponents::judge` → `project_component` at `656-675` (read); termination search accepts `Ok(natural)` at `checks_termination.rs:273-281` (read partial).
4. Input: a state with `relevance.requires_rank(state.symbol)` true, a non-transition `Assembly` statement, and `preserves_rank` false for that statement.
5. **Verdict: UNDECIDED** — The compiler skips the error path for assembly, but whether assembly can write rank-protected coordinates was not read in this slice.
6. Unsure: assembly write semantics (`unit_090.md:287`).

### A0346

1. Read `body_witnesses` / `member_mappings` at `call_cycles.rs:2600-2643` (read): `let Some(witness) = witness else { return Ok(None); }` at `2624-2625`.
2. Read rank walk at `2713-2721` (read): `let Some(mappings) = &member_mappings[position] else { continue; }` skips statement scrutiny for that member.
3. Read return `Ok(natural)` at `3004-3005` (read); `checks_termination.rs:273-281` treats `Ok(natural)` as acceptance when `bodies.len() == component.len()` (read).
4. Input: a cycle component where one machine index has no matching entry in the `witnesses` slice passed to `project_component`.
5. **Verdict: UNDECIDED** — Statement checks are skipped for the witness-less member, but whether witnesses can be missing while the termination search still accepts the component was not traced end-to-end.
6. Unsure: witness supply invariant when `bodies.len() == component.len()` (`search` at `checks_termination.rs:256-271` read only).

### A0347

1. Read `unranked_component_reached_from` at `call_cycles.rs:606-635` (read): `*seen.get_mut(origin)? = true` at `620-621` makes an out-of-range `origin` return `None` from the whole function.
2. Search `rg 'unranked_component_reached_from' omega-rust` → 2 hits (read): definition and `checks_termination.rs:8155`.
3. Read `prepare_runtime_rank_components` at `679-699` (read): `adjacency: graph.edges` with one row per machine; caller uses `position` from machine iteration (`checks_termination.rs:8151-8155`, read partial).
4. Input: only an invalid `origin` index relative to `self.adjacency.len()` (inconsistent topology); not a normal `CheckingInput`.
5. **Verdict: CLOSED** — Production callers pass a machine index into adjacency built from the same machine list; the silent `None` branch is not reachable on consistent graphs.
6. Unsure: nothing.

### A0348

1. Read `report_unresolved_value_call` at `calls.rs:1962-2087` (read): early `return` at `2062-2068` when `target` is `"encode"` or `"decode"` and `receiver` equals some `data_definitions().name`.
2. Search `rg 'report_unresolved_value_call' omega-rust` → 3 call sites in `calls.rs:3470`, `3548` (read); read enclosing value-call validation path `3455-3558` (read partial) — invoked after other resolution paths fail.
3. Input: a value-position call `Receiver.encode(...)` or `.decode(...)` where `Receiver` is spelled like a data type name, without a resolved callable.
4. **Verdict: REACHABLE** — Such a call exits without diagnostic from this function regardless of whether `encode`/`decode` exists on that data type.
5. Unsure: whether a separate wire-schema or trait path validates the same call (`calls.rs:39210-39217` noted but not traced for the same call shape).

### What I did not read

- Full bodies of `region_correspondence::validate`, `prove_integer_ordering` premise transport, and `type_reference_region_applications`.
- Assembly / inline-assembly write lowering and rank-coordinate mutability.
- Whether ill-typed `dyn Trait` + borrow values can reach `substituted_result_is_view_free`.
- Full termination `search` witness matching when `witness.body != machine.states`.
- Earlier typing gates that might reject assignments skipped by `validate_stored_integer_write`.

SLICE COMPLETE
