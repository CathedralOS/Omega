slice_0025

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0184 | attached_unit.rs:15243-15267 | CLOSED | `Ok(false)` marks non-applicable ingress; all three call sites discard the bool with `?`, and subsequent lowering errors if borrow storage was actually required. |
| A0185 | attached_unit.rs:11984-11991 | CLOSED | `defer_to_exit: true` is intentionally skipped in the argument schedule; exit cleanup emits it in `lower_reference_completion_regions`. |
| A0186 | attached_unit.rs:11666-11670 | CLOSED | Pre-establishment stops at the first `External` call; later literal operands must use pre-allocated places or lowering fails at consumption checks. |
| A0187 | attached_unit.rs:8792-8802,8827-8833 | CLOSED | Unscoped fallbacks are last resort; downstream carrier checks and scope tests reject ambiguous or wrong-scope resolution. |
| A0188 | attached_unit.rs:12242-12244 | CLOSED | Skips quotient types with no `type_ids` row—types not retained for the product—not unchecked source acceptance. |
| A0189 | attached_unit.rs:4491-4531 | CLOSED | Each `continue` drops optional establishment metadata only when a checked prerequisite link is absent; output is conservative. |
| A0190 | emission.rs:1355-1357 | CLOSED | Empty transfer list means no matching checked consume events; affine paths still require claims via `capture_operand_claims`. |
| A0191 | emission.rs:436-444 | CLOSED | Skips undecided (`selected == remaining`) or non-matching bindings by design; `edge_bindings` applies the same filter. |

### A0184

1. Read `prepare_immutable_local_borrow` in full at `attached_unit.rs:15243-15351` (read).
2. Search: `rg '\bprepare_immutable_local_borrow\b' /agent/repos/omega/omega-rust` — 4 hits (1 definition, 3 call sites) (read).
3. Callers read: `prepare_borrowed_argument` `attached_unit.rs:15227-15238`; `unit_arguments.rs:2079-2090`; `unit_operation_frame.rs:7930-7941` — all use `?` and discard the `bool` (read).
4. Upstream: `establish_reference` `unit_operation_frame.rs:7905-8014` calls prepare then `lower_reference_source` `attached_unit.rs:13510+` (read partial `lower_reference_source` through delegate to `lower_structural_arguments`).
5. `Ok(false)` when `root` is not `PlaceRoot::Symbol`, local missing, non-primitive, or mutable (`attached_unit.rs:15256-15267`) (read).
6. Entry note “one caller” is wrong: three production call sites (read).
7. Input reaching `Ok(false)`: checked structural-argument plans whose storage root is not an immutable primitive local symbol (inferred from guards).

**Verdict: CLOSED.** Check: guards encode “does not apply”; true-path prerequisites use `ok_or`/`unsupported` (`attached_unit.rs:15273-15300`). Callers that require materialization continue into `lower_reference_source`, which needs established locals/parameters.

**Unsure:** Full `lower_structural_arguments` failure modes for every `Ok(false)` combination in `prepare_borrowed_argument`’s non-`ScalarParameter` branch (not read end-to-end).

### A0185

1. Read `lower_step` branch `attached_unit.rs:11983-11991` (read).
2. Read defer handler `unit.rs:2235-2247` in `lower_reference_completion_regions` (read).
3. Caller of exit regions: `attached_unit.rs:6032-6039` during cleanup lowering (read).
4. Input: `CheckedUnitEffectOperationPlan::ReleaseReference { defer_to_exit: true, .. }` on the argument schedule path (read).

**Verdict: CLOSED.** Check `unit.rs:2237-2247` emits deferred release at exit; `attached_unit.rs:11984-11991` only defers mid-schedule emission.

**Unsure:** None for the defer split.

### A0186

1. Read `establish_call_literals` in full `attached_unit.rs:11637-11760` (read).
2. Read caller `attached_unit.rs:10595-10609` and balance check `attached_unit.rs:10823-10824` (read).
3. Read `emit_call_through_frame` `attached_unit.rs:12131-12135` and `literal_argument_places` `attached_unit.rs:14125-14148` (read).
4. Input: literal call operands on operations before the first `External` completion in `plan.operations` order (read).

**Verdict: CLOSED.** Checks `attached_unit.rs:10823-10824` and `attached_unit.rs:14141-14142` fail closed if pre-established literal places are missing; the `break` at `11666-11670` is intentional scoping of the pre-pass.

**Unsure:** Whether stage-05 plans ever place byte-sequence literal calls after an in-unit `External` completion (checker invariant not read).

### A0187

1. Read `expression_in` and `recipe_in` `attached_unit.rs:8783-8833` (read).
2. Search `expression_in(` — hits in `scalar_computations.rs`, `attached_unit.rs` (read grep).
3. Read consumer `scalar_computations.rs:7576-7593` and `scalar_lowering.rs:2376-2387` (read).
4. Read scope tests `attached_unit.rs:9043-9089` (read).
5. Input: expressions/recipes where exact owner scope lookup misses and an unscoped or invalid-owner fallback exists (read).

**Verdict: CLOSED.** Checks `scalar_lowering.rs:2378-2382`, `scalar_computations.rs:7592-7593`; tests `attached_unit.rs:9055-9058`, `9084-9085` show scoped constants stay distinct.

**Unsure:** Every direct `expression_in` call site’s carrier validation (only two consumers read in detail).

### A0188

1. Read `quotients::retain` loop `attached_unit.rs:12228-12274` (read).
2. Read caller `attached_unit.rs:17200-17209` (read).
3. Input: closed quotient definitions in `checked.typed` whose structural identity has no matching `type_ids` entry (read).

**Verdict: CLOSED.** The `continue` at `12242-12243` drops representations for types not in the retained `type_ids` table built in the same closure, not for ill-checked source programs.

**Unsure:** Whether any product-selected quotient could lack a `type_ids` row without an earlier lowering error (retention pipeline not fully read).

### A0189

1. Read `call_result_qualification_establishments` in full `attached_unit.rs:4455-4541` (read).
2. Read caller `attached_unit.rs:1763-1773` (read partial `emit`).
3. Input: result domains on call results with issuer set; each domain’s establishment plans and checked `CallEnsures` facts (read).

**Verdict: CLOSED.** Each `continue` (`4492-4493`, `4495-4496`, `4509-4510`, `4515-4516`, missing catalog route `4526-4538`) omits optional Terminal Psi establishment metadata when a checked link is absent—conservative output, not widened acceptance.

**Unsure:** Downstream validation of incomplete `qualification_establishments` vectors (not read).

### A0190

1. Read `lower_owned_operand_claims` `emission.rs:1320-1412` (read).
2. Read callers `unit_structural_values.rs:8005-8023`, `attached_unit.rs:5158-5173` (read).
3. Input: owned operand expressions filtered from `owned_transfer_paths` / permissions (`emission.rs:1343-1354`) (read).

**Verdict: CLOSED.** `Ok(Vec::new())` at `1355-1356` when no matching checked transfers; `capture_operand_claims` only calls this for restricted multiplicity with existing custody (`unit_structural_values.rs:7994-8022`).

**Unsure:** All call paths that pass `None` for `expected_evaluation`/`expected_source` (only structural-values path read).

### A0191

1. Read `destinations` `emission.rs:426-447` (read).
2. Read related `edge_bindings` filter `emission.rs:471-474` (read).
3. Read caller `scalar_computations.rs:2488-2495` (read).
4. Input: `selection_place_bindings` rows matching machine/state/statement/expression/decision, excluding undecided rows (`selected == remaining`) (read).

**Verdict: CLOSED.** Filter at `emission.rs:436-443` intentionally reserves only decided, matching bindings; undecided rows handled elsewhere in the selection graph (`edge_bindings`).

**Unsure:** None.

### What I did not read

- End-to-end `lower_structural_arguments` and all `prepare_borrowed_argument` callers (`unit_operation_frame.rs:1069`, `1723`; `unit_structural_values.rs:4263`).
- Stage-05 invariants for operation ordering around `BoundaryCompletion::External` and byte literals.
- Full `type_ids` / structural type retention pipeline preceding `quotients::retain`.
- Terminal Psi validators for quotient catalog completeness and qualification establishments.
- Remaining `expression_in` / `destinations` call sites beyond those cited.

SLICE COMPLETE
