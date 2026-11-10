slice_0005

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0033 | checks_borrows_resources.rs:100,129-131 | REACHABLE | `install_resource`/`collect` drop mismatched or invalid-operand drafts with no diagnostic in `call_operands::install`. |
| A0034 | checks_borrows_resources.rs:2940-2953 | REACHABLE | Unactivated call-result/descriptor-load loans are skipped without diagnostic when building direct resource rosters. |
| A0035 | checks_borrows_resources.rs:5773 | REACHABLE | `retained_inputs` failure for `UnretainedDerived` loans is swallowed; no `stored_call_inputs` row and no diagnostic at install. |
| A0036 | checks_borrows_resources.rs:4001-4013 | REACHABLE | Last-chance `collect_binding_observation` can resolve a failed plain-observation capture via any live same-symbol loan. |
| A0037 | checks_content.rs:2283-2285 | CLOSED | Early `Some(())` only when `call_target_has_checked_body_contract` after declared-contract and evidence gates. |
| A0038 | checks_content.rs:2286-2288 | CLOSED | Non-boundary `public_contract` path is an intentional authorized route after earlier exact-source and declaration checks. |
| A0039 | checks_content.rs:2307-2317 | REACHABLE | Nested qualified result paths can be accepted without `result_claim` when a boundary trait matches but `boundary_requirement_signature` is `None`. |
| A0040 | checks_content.rs:1258 | CLOSED | `check_used_calls` authenticates Call replacements only; Body replacements are judged in `observe_inputs` via `preserve_replacement`. |

### A0033

1. Read `call_operands::install_resource` in full (`checks_borrows_resources.rs:81-115`, read) and `call_operands::collect` in full (`:117-336`, read).
2. Search `rg 'install_resource\(' omega-rust` — 3 hits (definition at `:81`, call at `:72`, call at `:6086`).
3. Read caller `call_operands::install` (`:42-74`, read): loops drafts and `let _ = install_resource(...)` at `:72`. Read caller `supplied_permissions` install path (`:6086-6091`, read): `ok_or_else` diagnostic on `None`.
4. Search `rg 'call_loan_resources' checks_borrows.rs` — 0 hits (read via grep): borrow overlap path does not validate `call_loan_resources` completeness.
5. Input reaching accepting branch: in `call_operands::install`, a `ResourceDraft` whose backing root differs from `parent_lifetime.root` (`:100-101`), or `collect` hitting an invalid `operand` handle (`:129-130`), or `install_resource` seeing invalid `owner_operand` (`:86-92`).
6. **Verdict: REACHABLE** — A call could pass stage-05 resource install with a borrow-carrying argument that gets no `call_loan_resources` row and no diagnostic on the `call_operands::install` path.
7. Unsure whether every flow that reaches root mismatch can reach `call_operands::install` without already failing earlier typed-tree validation (not traced past `collect_suppliers` construction).

### A0034

1. Read `reconstruct_borrow_resource_rosters` (`checks_borrows_resources.rs:2905-3227`, read in full for the state/loan loop).
2. Search `rg 'reconstruct_borrow_resource_rosters\(' omega-rust` — 2 hits (definition `:2905`, call `initialize_checked_direct_borrow_resources` `:7085`).
3. Read caller `initialize_checked_direct_borrow_resources` (`:7073-7110`, read): propagates `Err(diagnostics)` from reconstruct.
4. Input: state-owned loan that is a call-result loan or descriptor-load origin (`:2940-2948`) with `activated[...] == false` (`:2949-2952`), so the loop `continue`s without adding a roster entry or diagnostic.
5. Read parallel pattern `checks_borrows.rs:5560-5570` (read): descriptor-load loan without activation also `continue`s without diagnostic in reference-slot custody.
6. **Verdict: REACHABLE** — A checked borrow graph could retain an unactivated call-result or descriptor-load loan, omit its direct resource row, and still pass `initialize_checked_direct_borrow_resources` when reconstruct produces no other diagnostics.
7. Did not trace every downstream lowering consumer of `direct_loan_resources` for missing-row behavior.

### A0035

1. Read `stored_call_inputs::install` in full (`checks_borrows_resources.rs:5744-5785`, read) and `retained_inputs` (`:5787-5879`, partial read through input roster).
2. Search `rg 'stored_call_inputs::install\(' omega-rust` — 1 hit (`:7081`).
3. Read caller `initialize_checked_direct_borrow_resources` (`:7081`, read): no `Result` from `stored_call_inputs::install`.
4. Input: `UnretainedDerived` loan with `established_by_call` valid (`:5761-5765`) where `retained_inputs` returns `Err` — non-trace `Err(_)` arm at `:5773` drops the loan.
5. **Verdict: REACHABLE** — A saved derived reference that should bind retained call-input resources can compile with no `stored_call_inputs` row and no diagnostic at install.
6. Later `retained_inputs` paths can error when a consumer needs the roster (`:5871-5877`, read), but install itself does not report failure.

### A0036

1. Read `permission_sources::collect` in full for the resolve tail (`checks_borrows_resources.rs:3617-4022`, read) and `collect_binding_observation` (`:4819-4886`, read).
2. Search `rg 'collect_binding_observation' checks_borrows_resources.rs` — definition only (read).
3. Read upward through `permission_sources::collect` callers in this file: used from `call_operands::collect_suppliers` (`:357-372`, read) with `PermissionFailures::default()` discarded; formation validate uses refusal on failure (`:1666-1683`, read).
4. Input: `plain_observation == true` (`FrozenReferenceSelection::NotFrozen` at `:3714-3715`, read); primary resolution path leaves `resolved == false` and truncates `output` (`:4000-4000`, read); last-chance `collect_binding_observation` succeeds if any same-`owner_symbol` loan is live at the capture statement with matching reborrow effect (`:4857-4885`, read).
5. **Verdict: REACHABLE** — A plain-observation capture whose specific supplier rule failed can still gain permission from an unrelated live loan on the same symbol.
6. Unsure how often flow marks `Observation` completion with `NotFrozen` while a stricter rule correctly failed first.

### A0037

1. Read `check_call_result_qualifications` closure (`checks_content.rs:2163-2554`, read sections `:2193-2317` and `:2527-2552`).
2. Search `rg 'check_call_result_qualifications\(' omega-rust` — 2 hits (definition `:2163`, call module init `:7217`).
3. Read caller content checks driver (`:7217`, not read in full).
4. Input reaching `return Some(())` at `:2283-2284`: prior gates require propagated evidence, exact invocation source, declared contract path (`:2248-2277`, read), then `call_target_has_checked_body_contract(program, target_symbol)`.
5. **Verdict: CLOSED** — Every path to `:2283-2284` passes evidence and declared-contract checks; checked-body targets are vouched elsewhere before this early accept.
6. Did not read full body-contract check implementation (only call sites in `checks_content.rs`).

### A0038

1. Same function as A0037 (`checks_content.rs:2286-2288`, read).
2. Input: `public_contract.is_some_and(|(declaration, _)| !declaration.is_boundary)` at `:2286-2287` returns `Some(())` after the same front-half gates as A0037.
3. **Verdict: CLOSED** — Non-boundary public requirement signatures are an intentional issuance route once exact-source and declaration matching already succeeded (`:2248-2277`, read).
4. Did not trace all `requirement_contract_signature` producers.

### A0039

1. Read `checks_content.rs:2307-2317` and `boundary_requirement_signature` (`:2779-2808`, read).
2. Input: qualified call-result fact with `(projection.is_none() || !path.is_empty())`, no `boundary_requirement_signature`, but some boundary trait owns a machine signature for the target (`:2307-2314`, read) → `return Some(())` at `:2316` before `result_claim` at `:2423-2425`.
3. **Verdict: REACHABLE** — A nested qualified result path on a boundary-trait target with no matching establishment route can satisfy `check_call_result_qualifications` without an owned result-claim proof.
4. Empty-path cases may still fall through to later conservation logic (`:2323+`, partially read).

### A0040

1. Read `call_ancestry::check_used_calls` in full (`checks_content.rs:1233-1274`, read) and `authenticate_call` (`:1305-1321`, read).
2. Search `rg 'check_used_calls\(' omega-rust` — 2 hits (`:1233`, `:7216`).
3. Read `observe_inputs` in `checks_multiplicity.rs` (`:69-78`, read): `descendant_frontier` then `preserve_replacement` for every replacement including `Body` (`:299-324` in `preserve_replacement`, read).
4. Search `rg 'TransformationSource::Body' checks_content.rs` — `:1708` in `qualified_roots` (read context `:1697-1717`), separate from `check_used_calls`.
5. Input at `:1258`: frontier `replacements` entry with `TransformationSource::Call` only gets `authenticate_call`; `Body` handles in the same loop are skipped.
6. **Verdict: CLOSED** — Module comment at `:1217-1224` scopes this to call ancestry; Body-sourced replacements are judged in `observe_inputs` / `preserve_replacement` (`checks_multiplicity.rs:77-78`, read), not omitted globally.
7. Did not prove no `Body` replacement appears only in `check_used_calls` frontier without passing `observe_inputs` first.

### What I did not read

- Full `checks.rs` content driver around `:7216`.
- Full `validation::call_target_has_checked_body_contract` definition (only call sites read).
- Entire `checks_borrows.rs` borrow-check driver beyond cited activation/custody slices.
- Callers of `initialize_checked_direct_borrow_resources` beyond its definition.

SLICE COMPLETE
