slice_0033

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0248 | scalar_computations.rs:6060-6082 | UNDECIDED | Equal `Value` arms skip false claims and conditional; did not verify stage-05 forbids asymmetric selection claims for that shape |
| A0249 | scalar_computations.rs:7107-7117 | CLOSED | `consume_pure_call_arguments` is only required for `LocalInitializer`; other roles expand through `argument` with full checks |
| A0250 | scalar_computations.rs:5847 | CLOSED | `0` is a placeholder replaced inside `argument_with_normal_result` via `normal_result_scope` before emission finishes |
| A0251 | scalar_computations.rs:4432-4441 | CLOSED | Early return uses only lowering-established `established_payloads` entries; otherwise full `observe` checks run |
| A0252 | scalar_lowering.rs:1761-1763 | CLOSED | `Boolean`/`Integer` closed clauses are tautological equalities from stage 05, not ignored literal requirements |
| A0253 | scalar_lowering.rs:1783-1792 | CLOSED | Runtime projection deliberately drops `None` and `SourceCheckedGuarantee`; source-checked obligations are recorded elsewhere |
| A0254 | scalar_lowering.rs:1827-1831,2129-2167 | CLOSED | Missing identity text becomes `""` only on checked handles; mathematical scalar paths still `ok_or` errors |
| A0255 | scalar_lowering.rs:2506-2508 | CLOSED | Non-empty domain or nonzero level arity carriers are omitted from the terminal table by design; uses of them error in lowering |

### A0248

1. Read `Expansion::selection` in full at `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/scalar_computations.rs:5975-6123` (read).
2. Search `\.selection\(` under `06_checked-trees-to-lowered-psi`: one production call at `scalar_computations.rs:7625` inside `argument_with_normal_result` for `CheckedScalarComputationKind::Select` (read).
3. Read caller `argument_with_normal_result` from `7297` through the `Select` arm at `7625`; upstream is scalar computation expansion during body emission (read partially; did not read every `argument` caller chain above expansion).
4. Input on the accepting branch (read at `6060-6070`): checked `Select` where neither arm appears in `self.completed`, both nodes are `CheckedScalarComputationKind::Value` with equal payloads per `plans.nodes`.
5. Verdict **UNDECIDED**: skipping the false `claim_continuation` and `Conditional` (read `6069-6107`) can drop false-route claim edges (`emission.rs:426-447`, read) even when arm values match; I did not read stage-05 multiplicity/claim checking far enough to say whether asymmetric `selection_place_bindings` can coexist with equal `Value` arms on a checked tree that reached lowering.
6. Entry description matches the code (read).

### A0249

1. Read `retained_root_argument` in full at `scalar_computations.rs:7093-7119` (read).
2. Search `\bretained_root_argument\b` in `omega-rust`: 3 hits — definition plus `normal_return` at `5756` and `retained_root` at `7074` (read).
3. Read callers `normal_return` (`5733-5857`, read) and `retained_root` (`7065-7090`, read); `normal_return` called from `unit_arguments.rs:2366` (read that callsite only).
4. Input on accepting branch: any checked scalar root whose `role` is not `LocalInitializer` passes through without `consume_pure_call_arguments` (read `7107-7118`).
5. Verdict **CLOSED**: `consume_pure_call_arguments` at `712-777` (read) cross-checks pure-call operands only for local-initializer retention; non-`LocalInitializer` roots still go through `retained_root` type agreement at `7080-7081` and full `argument` expansion at `7083-7090`, which errors on structural mismatch.
6. Entry description matches (read).

### A0250

1. Read `normal_return` in full at `scalar_computations.rs:5733-5871` (read).
2. Search `normal_return(` in stage 06: one caller at `unit_arguments.rs:2366` (read).
3. Read `argument_with_normal_result` at `7297-7333` and `normal_result_scope` at `5872-5924` (read).
4. Input: `NormalCompletionTarget::Selected(_)` supplies `continuation = 0` at `5845-5848` into `argument_with_normal_result` (read).
5. Verdict **CLOSED**: `0` is not used as a final jump target when `normal_result` is present — `normal_result_scope` (read `5877-5922`) builds the real target and optional `NormalDispatchScope`, and `dispatch` at `7548-7563` consumes `normal_dispatch`; placeholder `0` is superseded before the branch state graph is finished.
6. Entry description matches (read).

### A0251

1. Read `observe` in full at `scalar_computations.rs:4394-4579` (read through first major return paths; remainder of function not fully read).
2. Search `observe(` in `scalar_computations.rs`: call sites at `3019`, `4151`, `4255` (read signatures at those sites, not full upstream chains).
3. Read `established_case_payload` at `expression_bindings.rs:5435-5450` and `established_payload` at `1689-1707` (read).
4. Input on accepting branch: field-in-case observation where `bindings.established_case_payload(...)?` returns `Some` (read `4432-4440`).
5. Verdict **CLOSED**: early return emits `LoweredDirectExpression::Parameter` for a position recorded in `established_payloads` during lowering (`establish_case_payloads` at `1728-1737`, read); if lookup fails, execution continues into carrier, runtime-index, and overlay checks (read `4442-4521`).
6. Entry description matches (read).

### A0252

1. Read `contracts::clauses` in full at `scalar_lowering.rs:1748-1781` (read).
2. Search `contracts::clauses(` in `omega-rust`: 6 hits in `attached_unit.rs` (4), `unit_arguments.rs` (1), `unit_composed_control.rs` (1) (grep count, read `attached_unit.rs:10241-10242` and `unit_composed_control.rs:10951` only among callers).
3. Read `ClosedScalarContractValue` at `contract_and_obligation_facts.rs:3460-3468` and stage-05 closure at `published_facts.rs:3137-3140` (read).
4. Input: `Some(ClosedScalarContractValue::Boolean(_))` or `Integer(_)` in a contract clause slice passed to `clauses` (read `1761-1763`).
5. Verdict **CLOSED**: stage 05 stores `Boolean`/`Integer` only for provably identical literal equalities (`left == right` at `published_facts.rs:3137-3140`, read); mapping both to `Proposition::Truth` is lowering those tautologies, not erasing a `false` requirement; `None` clauses still error at `1772` (read).
6. Entry description partially wrong on “false clause”: read shows the bool payload is from `x == x`, not an unconstrained literal require-false.

### A0253

1. Read `projected_guarantees` in full at `scalar_lowering.rs:1783-1792` (read).
2. Search `\bprojected_guarantees\b` in `omega-rust`: 2 hits — definition and `attached_unit.rs:10206` (read).
3. Read guarantee lowering block at `attached_unit.rs:10205-10242` (read).
4. Input: `ensures()` rows that are `None` or `SourceCheckedGuarantee` are dropped before `clauses` (read `1786-1791`, `10205-10208`).
5. Verdict **CLOSED**: `SourceCheckedGuarantee` in `clauses` directly returns `unsupported` at `1767-1770` (read); filtering in `projected_guarantees` routes ensures away from runtime propositions while source-checked rows are handled via `source_checked_obligations` in `unit_composed_control.rs:10987` (read that line only).
6. Entry description matches (read).

### A0254

1. Read `erased_proof_formal_declarations` at `1823-1834` and `lowered_proof_term` arms at `2124-2168` in `scalar_lowering.rs` (read those functions; full `lowered_proof_term` not read end-to-end).
2. Search `erased_proof_formal_declarations(` in `omega-rust`: 17 hits across stage-06 emitters (grep count; did not read every callsite).
3. Read `lowered_proof_term` `Scalar` arm using `ok_or` at `2113-2117` (read).
4. Input: checked type-identity handle whose `type_identity_text` returns `None` gets `unwrap_or_default()` → interned empty string (read `1827-1830`, `2129-2131`, `2147-2149`, `2163-2165`).
5. Verdict **CLOSED**: paths that must name carriers for mathematical soundness use `ok_or` (read `2113-2117`, `2509-2511`); `unwrap_or_default` applies to structural proof-term shapes whose identities are already fixed on checked trees entering stage 06 — lowering does not treat absence as success for mathematical applications.
6. Entry description matches for empty string; dual behavior at scalar mathematical arguments noted (read).

### A0255

1. Read `mathematical_carriers` in full at `scalar_lowering.rs:2497-2540` (read).
2. Search `mathematical_carriers(` in `omega-rust`: 2 hits — definition and `attached_unit.rs:17217-17218` (read).
3. Read mathematical application lowering guard at `1926-1944` in same file (read).
4. Input: `CheckedSourceEvidenceCarrier` rows with nonempty `domains` or `level_arity != 0` hit `continue` at `2506-2508` (read).
5. Verdict **CLOSED**: only empty-domain, zero-arity carriers are published to `TerminalModule.mathematical_carriers`; attempts to lower mathematical applications without such a carrier fail at `1942-1944` (read), so omission is not silent acceptance of bad programs.
6. Entry description matches (read).

## What I did not read

- Full `observe` below `4579` and full `lowered_proof_term` below the cited arms.
- Stage-05 `checks_multiplicity` / select-claim rules needed to decide A0248.
- All 17 `erased_proof_formal_declarations` callsites.
- Corpus or runtime behavior.

SLICE COMPLETE
