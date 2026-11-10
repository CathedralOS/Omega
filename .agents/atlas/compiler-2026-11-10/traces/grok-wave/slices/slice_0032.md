slice_0032

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0240 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:3556 | UNDECIDED | `_ => return None` in `operand_memberships::retained` (read); did not trace every domain-reachable `OperationKind` for a compensating retain arm. |
| A0241 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:3742;3783-3785 | CLOSED | `payload_memberships::retain` early exits and per-machine `continue` only when no structural domains or no domain-reachable structural results apply (read). |
| A0242 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:3474;3490;3814 | UNDECIDED | Three `if let Ok` sites keep lowering on membership `Err`/partial data (read); did not trace whether missing merged qualifications can change compile outcome. |
| A0243 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:5369;5393;5547;5647 | UNDECIDED | Loans map to `Unresolved` terminal enums (read); no stage-06/08 rejection found; stage-05 carry check at checks.rs:6715-6717 not traced on every path. |
| A0244 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:4881-4886 | CLOSED | Crossing filter matches partial machine lowering: `source_machines` from selection (machine_lowering.rs:1965-1968 read). |
| A0245 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/scalar_computations.rs:712-778 | UNDECIDED | `consume_pure_call_arguments` returns `Ok(())` without checks on several call shapes (read); did not trace stage-05 whether invalid pure args can reach this. |
| A0246 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/scalar_computations.rs:3104-3110;3457-3459 | CLOSED | Missing place loan yields `Ok(None)` and `scalar_field_store` uses the non-loan emit path (unit_operation_frame.rs:6067+ read). |
| A0247 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/scalar_computations.rs:3568-3572 | CLOSED | `checked_path_segment` runs on field ids taken from validated structural declarations (scalar_computations.rs:3268-3281 read), not ad-hoc text. |

### A0240

1. Read `operand_memberships::retained` in full at retention.rs:3411-3570 (read); cite line 3556 `_ => return None`.
2. Search `rg 'operand_memberships::retained' /agent/repos/omega/omega-rust` → **1** hit: retention.rs:3840 (read).
3. Read caller `payload_memberships::retain` loop retention.rs:3798-3850 (read); on `None`, no `copies.push` for that source.
4. Search `rg 'payload_memberships::retain' omega-rust` → **1** hit: machine_lowering.rs:2065 (read); lowering returns `Ok(())` when retain completes.
5. **Input to branch:** lowered `Operation` with structural result in domain reachability, kind outside the five `match` arms (e.g. not `MoveStructuralField` / `EstablishReference` / array / record / case).
6. **Verdict: UNDECIDED.** Other retain arms cover `StructuralLeafCopy` (3894+) and `MoveStructuralValue` (3852+) (read), but I did not enumerate all operation kinds that produce domain-carrier results without another arm.
7. **Unsure:** Whether a kind can need operand-derived `projected_qualifications` with no alternate handler and still complete lowering without later failure.

### A0241

1. Read `payload_memberships::retain` in full at retention.rs:3741-3956 (read).
2. Search `rg 'payload_memberships::retain' omega-rust` → **1** hit: machine_lowering.rs:2065.
3. Read `lower_terminal_selection` retention block machine_lowering.rs:2060-2067 (read); `Err` from retain aborts lowering.
4. **Input to branches:** (a) `module.structural_domains.is_empty()` at 3742-3743; (b) no machine with any op whose result type `memberships.may_contain` at 3768-3769; (c) per-machine `!has_candidate` at 3783-3784.
5. **Verdict: CLOSED.** Checks at retention.rs:3742-3743, 3760-3769, 3783-3784 (read): skipping retention only when domains are absent or no operation result is domain-reachable for that machine; fewer qualifications reflect nothing to attach, not acceptance of a bad program.
6. **Unsure:** Nothing material for CLOSED on cited lines.

### A0242

1. Read `EstablishReference` arm and retain loop at retention.rs:3474-3496, 3814-3828 (read).
2. Search callers as for A0240; same `retain` loop merges only successful `copies` (3938-3948 read).
3. **Input:** (a) `reference_backing_memberships` returns `Err`; (b) `history.current_selected_payload_contract_in` returns `Err`; (c) `reference_memberships.for_operation` returns `Err` (references.rs:609+ returns `Result<_, &'static str>` read at 613-641).
4. **Verdict: UNDECIDED.** Ignored errors still emit a module with partial or empty added `projected_qualifications`; I did not read whether `descriptor_residences::complete` (emission.rs:2479+ read start only) or default compile path rejects the gap.
5. **Unsure:** End-to-end effect of under-merged qualifications on compile/execute vs validator-only checks (foundation_validation.rs:495-506 inferred from grep, not read in full).

### A0243

1. Read `retain_loan_graph` at retention.rs:5099-5652 (partial full: 5275-5650 read); Unresolved at 5369, 5393, 5547-5548, 5647-5648.
2. Search `rg 'retain_loan_graph' omega-rust` → **2** hits: 5044, 5099 (read).
3. Search `rg 'TerminalSuspensionLoanCapture::Unresolved' omega-rust` → **2** hits: ownership.rs:923 (hash), retention.rs:5369 (read).
4. Read stage-05 `SuspensionValueBorrowFrontier::Unresolved` handling checks.rs:6709-6717 (read): returns `Err("the stored loan-leaf frontier is incomplete")` on that variant in `carry_policy` path.
5. **Input:** checked suspension loan rows where stored-loan capture has no formation capture (5368-5369), parent `resource` matches no known arm (5393), backing fact is `Unresolved` (5547), or live value `borrow_frontier` is `Unresolved` (5647).
6. **Verdict: UNDECIDED.** Lowering still returns `Ok(graph)` (5652 read); no omega-rust read of a post-lowering validator rejecting terminal `Unresolved` capture/resource; stage-05 rejection at 6715-6717 not shown on every suspension path.
7. **Unsure:** Runtime/interpreter handling of terminal suspension plans with `Unresolved` fields; whether capture/resource `Unresolved` can appear only when stage-05 already allowed the crossing.

### A0244

1. Read `retain_suspension_call_plans` filter at retention.rs:4876-4886 and missing-occurrence path 4895-4901 (read).
2. Search `rg 'retain_suspension_call_plans' omega-rust` → machine_lowering.rs:2041 (read).
3. Read `source_machines` construction machine_lowering.rs:1965-1968 (read).
4. **Input:** `suspension_crossings` fact whose `machine` ∉ `source_machines` and whose `state` ∉ any `state_blocks` state.
5. **Verdict: CLOSED.** Filter omits crossings outside the machines/states being lowered; consistent with partial selection wiring at machine_lowering.rs:1965-1968; filtered-out crossings are not required to receive a plan in this lowering invocation.
6. **Unsure:** Whether a crossing on a non-selected machine could be required for correctness of a selected machine (not traced).

### A0245

1. Read `consume_pure_call_arguments` in full scalar_computations.rs:712-778 (read); early `Ok(())` at 733, 742, 757.
2. Search `rg 'consume_pure_call_arguments' /agent/repos/omega` → **2** production hits in scalar_computations.rs:712, 7108 (read).
3. Read caller `retained_root_argument` scalar_computations.rs:7093-7118 (read); calls only when `role == LocalInitializer`.
4. **Input:** scalar computation root with `CheckedScalarCallee::Body` pattern failing other guards: non-`Body` callee (732-733), non-empty structural args (741-742), or operand not `CheckedScalarComputationKind::Value` (755-757).
5. **Verdict: UNDECIDED.** Lowering accepts without pure-argument cross-check in those cases; I did not read stage-05 `execution_scalar_computations.rs` far enough to prove such roots cannot exist after checking.
6. **Unsure:** Whether non-`Body` or non-`Value` call operands in `LocalInitializer` roots are impossible or already diagnosed in stage 05.

### A0246

1. Read `planned_place_loans`, `loan_subject`, `loaned_store` scalar_computations.rs:3078-3460 (read).
2. Search `rg 'loaned_store\(' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi` → unit_operation_frame.rs:6054 (read).
3. Read `scalar_field_store` unit_operation_frame.rs:6048-6165 (read): `if let Some(loan) = loan` at 6067, else parameter/local store path 6115+.
4. **Input:** store to local symbol with no matching entry in `planned_place_loans` for that state/machine.
5. **Verdict: CLOSED.** `loan_subject` returns `Ok(None)` (3108-3109); `loaned_store` returns `Ok(None)` (3457-3459); emitter still lowers the store via non-loan destination path (6115-6164 read).
6. **Unsure:** Nothing material for CLOSED.

### A0247

1. Read `checked_path_segment` scalar_computations.rs:3562-3577 (read); `unwrap_or_default` at 3570.
2. Read `captured_place_subject` field handling 3233-3282 (read): `carrier_path` fields use `selected.identity` from resolved record field declaration.
3. Read `captured_store_path` 3534-3558 (read) calling `checked_path_segment` on `loan.carrier_path` and `loan.element_suffix`.
4. **Input:** `StructuralPathSegment::Field` whose `identities.text(*field)` is `None`.
5. **Verdict: CLOSED.** Typical path builds terminal field indices from declaration-backed `selected.identity` (3268-3281 read) after spelling match errors on absent/ambiguous field (3242-3247 read); empty spelling branch is defensive on missing table text, not the normal checked-loan path.
6. **Unsure:** Whether `identities.text` can be `None` for indices inserted from module structural declarations in a successful lowering.

### What I did not read

- Full `descriptor_residences::complete` (emission.rs:2479 onward beyond opening).
- Full `foundation_validation.rs` suspension/membership sections.
- Stage-05 proof that every `LocalInitializer` computation call shape reaching `consume_pure_call_arguments` is already pure-checked.
- Complete enumeration of `OperationKind` variants vs payload retain handlers beyond spots cited.
- Psi interpreter / native runtime use of `TerminalSuspensionLoan` `Unresolved` variants.

SLICE COMPLETE
