slice_0035

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0264 | unit_arguments.rs:4293-4306 | CLOSED | Empty proof actuals on scalar branch jumps only trigger successor formal forwarding inside already-checked call-computation lowering. |
| A0265 | unit_arguments.rs:1849-1855; unit.rs:3703-3716; unit_arguments.rs:5191-5207 | CLOSED | Dropped capture slots and zero-leaf partition markers match checked construction; invalid-symbol adoption is same-place repair only. |
| A0266 | unit_composed_control.rs (16 site bundle) | UNDECIDED | Read all cited sites; stage-05 producers and end-to-end acceptance were not traced for every sub-claim in the bundle. |
| A0267 | unit_operation_frame.rs; unit_structural_values.rs | UNDECIDED | Last dispatch arm and invalid-symbol sites look checker-guarded; suspension frontier and call-ensured skips were not traced to acceptance. |
| A0268 | 07/psi_optimization.rs:48-62; 07/control_flow_and_retention.rs:710,959; 08/result_policy_verification.rs:334-336 | UNDECIDED | No-op optimizations and debug retention are benign; non-Expression float boundary policy skip was not traced to whether bad modules pass finalize. |

### A0264

1. Read `completion_roster` in full at `unit_arguments.rs:4293-4306` (read): if `actuals` is empty and `roster` is non-empty, map each roster entry to `ProofTerm::Formal { position }`.
2. Search `rg 'completion_roster' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/unit_arguments.rs` → 5 hits (read): definition at 4293; call sites 3754, 3802, 3969, 3973 inside `emit_state` terminator lowering.
3. Read `emit_state` at `unit_arguments.rs:3394-3414` and its single production caller at `unit_arguments.rs:6276` (read); did not read the distinct `emit_state` in `unit_composed_control.rs:6804`.
4. Read call sites (read): `Qualification` and `Jump`/`Conditional` terminators assign `*proof_actuals` / `erased_proof_arguments` after `lowered_proof_terms` or `proof_terms!`, then `completion_roster(..., source_erased_proof_formals)`.
5. Input reaching branch (read): lowered scalar branch terminators whose proof-actual list is empty after `lowered_proof_terms`, while `source_erased_proof_formals` (from `&self.erased_proof_formals` at 6283) is non-empty.
6. Read stage-05 gate `checked_call_erased_proof_arguments` at `05_typed-trees-to-checked-trees/src/execution_terminal_unit_calls.rs:302-340` (read): `Iterator::collect` on `Option` yields `None` if any required proof term is missing.
7. Read entry-block jump at `unit_arguments.rs:6252-6257` (read): uses `proof_formal_forwarding()` without `completion_roster` — same formal-forwarding idea.

**Verdict: CLOSED.** Reason: `completion_roster` only fills successor formal positions on internal scalar-graph edges; it is not a substitute for missing call proofs at the checker. **Unsure:** whether any non-call scalar edge can require concrete proof witnesses while still lowering an empty actual list (not traced in scalar branch preparation).

### A0265

1. Read `ScalarArgumentCaptures::values` at `unit_arguments.rs:1849-1854` (read): `filter_map` drops `None` slots when building captured argument vectors.
2. Read `emit_cleanup_regions` fast path at `unit_arguments.rs:744-753` and `1532-1535` (read): when every region has no partition leaf, conditions, or selected region, `filter_map(|region| region.action.into_physical())` drops `CheckedPartition`.
3. Read `CleanupRegionAction::into_physical` at `unit.rs:2414-2418` (read): `CheckedPartition` returns `None`.
4. Read `materialize_prepared_cleanup_regions` / `PartitionOnly` at `unit.rs:3696-3716` and producer loop at `unit.rs:3175-3188` (read): zero-`leaf_count` partitions become `PartitionOnly` markers with `CheckedPartition` action.
5. Read `record_owned_home` at `unit_arguments.rs:5187-5209` (read); search `rg 'record_owned_home' omega-rust` → 2 hits (read): definition + one call at 5326 in `establish_structural_result`.
6. Input (read): checked cleanup plans with zero-leaf partition markers; capture tables with sparse ordinals; structural owners sharing a place where the existing symbol is invalid.

**Verdict: CLOSED.** Reason: empty capture ordinals are intentional omissions; `CheckedPartition` dropping applies only to zero-leaf markers produced at `unit.rs:3175-3188` or the trivial-region fast path; symbol adoption requires the same place and errors on conflicting valid symbols (`5196-5200`). **Unsure:** no exhaustive grep of every `PreparedNormalCleanupAction` producer beyond `unit.rs:3180` (inferred).

### A0266

1. Read `finish_state` / crash skip at `unit_composed_control.rs:356-368` and `sequence.require_closed` guard at `3231-3236` (read).
2. Read crash-exit retain no-op at `unit_composed_control.rs:4359-4361` (read); `_ => Vec::new()` loan arrivals at `2265` (read).
3. Read owner assignment fallback at `1528, 1553` (read); `capture_dispatch_copies` early return at `3564-3571` (read).
4. Read `Foreign`/`Other` loan arrivals at `4838-4861` and `5018-5023` error on `Foreign` in chains (read).
5. Read synthesized `DiscardRoot` at `4226-4235` (read); return value append at `7691-7699` (read); `SourceScalarReturn` only on `Return` at `8217-8221` (read).
6. Read unlisted loan source acceptance at `9333-9432` (read); alias entry at `11325-11425` (read); rank edges skip non-Jump/Conditional at `11920` (read).
7. Read `unwrap_or_default` case spelling at `12261` (read); atomic/non-scalar snapshot skip at `12302-12319` (read).
8. Read retirement `continue` paths at `12629-12700` (read); fallback loan sources at `13428, 13445-13475` (read).
9. Did not read every enclosing function in full (`alias_entry_parameter_loans`, full state-graph emit, ranked-loop closure).

**Verdict: UNDECIDED.** Reason: sites mix intentional crash/ debug behavior with lowering paths that omit errors (e.g. empty dispatch copies, retirement `continue`, alias without intermediate-write check); stage-05 guarantees for each were not traced to program acceptance. **Unsure:** smallest rejecting program for each sub-site (per prompt, not attempted for all 16).

### A0267

1. Read suspension frontier loop at `unit_operation_frame.rs:2889-3017` (read): `CallArgument` and missing-home paths `continue` without binding.
2. Read call-ensured membership recording at `3178-3228` (read): five `continue` arms when home missing, `represents` true, representation complete, etc.
3. Read `ReleaseReference` skip at `8085-8091` (read) when `cleanup_source_is_retired` is true.
4. Read structural dispatch at `unit_structural_values.rs:3593-3634` (read): `boolean_value_split` / `boolean_case_split` only when `position + 1 < arms.len()` — last arm has no preceding test.
5. Read `prepare_owners` at `5442-5444` (read): invalid symbol → `Ok(true)` (always available).
6. Read `content_transformation` at `2268-2277` (read): invalid handle → `Ok(None)`.

**Verdict: UNDECIDED.** Reason: last-arm dispatch and invalid-symbol liveness rest on checker exhaustiveness/plan validity (not retraced in stage 05 here); suspension frontier and call-ensured omissions lack upward trace to acceptance. **Unsure:** consumers of `LoweredSourceFrontierBindings` after `retain` (`2809+`).

### A0268

1. Read `07/psi_optimization.rs:32-52` (read): four optimization variants are empty match arms but still run through the loop; execution record built at 60-62.
2. Read `07/control_flow_and_retention.rs:703-711` and `956-960` (read): debug `sites.retain` keeps unlisted `DebugSubject` variants (`_ => true`).
3. Read `08/result_policy_verification.rs:277-336` and final check at `494-496` (read): filter at 286-287 excludes non-`Expression` sources from `source_policies`; loop `continue` at 334-336 skips adapter verification for non-`Expression` custody.

**Verdict: UNDECIDED.** Reason: stage-07 no-ops and debug retention do not affect program acceptance; whether a float boundary call with non-`Expression` custody can require a policy yet evade both verification and the `matched/actual/source_policies` tally was not traced in stage 05/06 lowering. **Unsure:** production of `NominalMachineUseSite` other than `Expression` for policy-bearing float boundaries.

### What I did not read

- Full bodies of `alias_entry_parameter_loans`, composed state-graph `emit`, and retirement promotion helpers beyond cited ranges in A0266.
- Scalar branch preparation that feeds `LoweredScalarBranchTerminator` erased proof lists (A0264 residual doubt).
- All upstream `PreparedNormalCleanupAction` constructors except `unit.rs:3175-3188` (A0265).
- Consumers of `LoweredSourceFrontierBindings` after suspension `retain` (A0267).
- Stage-05 proofs for composed-graph loan/alias/retirement sub-claims (A0266, A0267).
- Whether non-`Expression` boundary custody arises for policy-bound float calls (A0268).

SLICE COMPLETE
