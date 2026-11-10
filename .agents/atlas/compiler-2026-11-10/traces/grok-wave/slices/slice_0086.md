slice_0086

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0658 | provider_admission/access_lifecycles.rs:326 | REACHABLE | Nested structural types whose shape is not Record/Sum/Mixed/nonzero FixedArray skip `require_runtime_value` checks via `_ => {}`. |
| A0659 | provider_admission/compiler_builtins/retained.rs:210-217 | REACHABLE | A demanded requirement with zero filtered intrinsic matches `continue`s without diagnostic and `Ok(evidence)` omits a proposal for that demand. |
| A0660 | provider_admission/settlements.rs:290-308 | CLOSED | Undeclared schema ABI skips cross-check but callers still bind the retained row's exact application and validate entry plans downstream. |
| A0661 | provider_admission/settlements/source_imports.rs:55-57 | REACHABLE | `selected_external` absent with `selected_count > 1` for non-external rows skips the boundary silently; duplicate check at :66-70 never runs. |
| A0662 | provider_admission/static_interrupts/scopes.rs:556-577 | REACHABLE | `require_masked` only rejects `InterruptControl(Enable)`, not other `RegisterAccess` writes that could re-enable maskable arrivals. |
| A0663 | host_cohorts.rs:127-135; terminal_authority_policy.rs:260-262 | REACHABLE | Non-Windows cohort rows key disposition on `method.name` only, accepting a class that may be narrower than the admitted import's behavior. |

### A0658

1. Read `require_runtime_value` (access_lifecycles.rs:298-330) and `require_fields` (access_lifecycles.rs:332-355) in full; read caller `require_image_contract` (access_lifecycles.rs:239-296) in full; read admission site `settle_native_lifecycles` (access_lifecycles.rs:34-107) through the `require_image_contract` call only.
2. Search `\brequire_runtime_value\b` under `omega-rust/`: **4 hits** (all in access_lifecycles.rs:260, 298, 324, 351).
3. Caller chain read: `settle_native_lifecycles` → `require_image_contract` → `require_fields` → `require_runtime_value` (recursive). Upstream `admit_native_providers` (provider_admission.rs:222-227) calls `settle_native_lifecycles`; did not read full `admit_native_providers`.
4. Input reaching `_ => {}`: a nested `StructuralTypeId` from a runtime-relevant record field whose declaration shape is **not** Record, Sum, Mixed, or a nonzero-literal FixedArray (read StructuralTypeShape at types_and_values.rs:684-710 — Reference, PrimitiveScalar, ByteSequence, ElementView, AtomicCell, or zero-length FixedArray fall through).
5. **Verdict: REACHABLE** — A native access lifecycle whose record field points at a structural type shaped as Reference (or similar) would pass `require_image_contract` without visiting erased-field or acyclic subtree rules for that node.
6. Unsure whether earlier Terminal lowering forbids such shapes in lifecycle schemas (not traced past PA).

### A0659

1. Read `derive_selected_intrinsic_settlement_proposals` in full (retained.rs:141-285).
2. Search `\bderive_selected_intrinsic_settlement_proposals\b` under `omega-rust/`: **6 hits** across retained.rs, compiler_builtins.rs, provider_admission.rs, retained_terminal.rs.
3. Callers read: `retained_terminal.rs:803-810` (prepare path); `admit_terminal_intrinsics` (retained.rs:436-443). Upstream `settle_compiler_builtins` (compiler_builtins.rs:57-95) only iterates `request.compiler_builtins` built from proposals; `require_reached_providers` (provider_admission.rs:421-491) checks `selected_requirement_row`, not proposal presence.
4. Input: a `demanded_boundaries` entry whose `matches` slice after filtering (retained.rs:199-209) is **empty** — loop hits `continue` at retained.rs:216 with no diagnostic.
5. **Verdict: REACHABLE** — A boundary listed in `demands` but with no matching selected CompilerIntrinsic row after `selected_row_matches_requirement` can yield success without a `TerminalCompilerBuiltinProposal`, while later admission may not refuse that omission on the compiler-builtin settlement path.
6. Unsure whether staged selection invariants always guarantee at least one match whenever a compiler-intrinsic boundary is executable.

### A0660

1. Read `external_calling_application` in full (settlements.rs:271-310); read immediate callers `rejoin_normalized_foreign_call` (settlements.rs:426) and `rejoin_system_call` (settlements.rs:570) plus source_imports.rs:95-143 (partial).
2. Search `\bexternal_calling_application\b` under `omega-rust/`: **5 hits** (settlements.rs:3, source_imports.rs:2).
3. Input: `declared == false` when `plan.schema.calling_applications` has no entry for `abi.requirement_identity` (settlements.rs:290-295) — function returns `Ok(application)` without commitment/fingerprint comparison (settlements.rs:308-309).
4. **Verdict: CLOSED** — Same callers still require `row.calling_application(abi)` to succeed (settlements.rs:276-280); source import path runs `validate_boundary_entry_plan` (source_imports.rs:102-104). Schema comparison applies only when the schema declares the ABI (settlements.rs:295-307).
5. Unsure: full stage-00 custody proof for retained applications (not read).

### A0661

1. Read `derive_source_evaluated_mechanisms` (source_imports.rs:32-188) and `join_import_coverage_rows` (source_imports.rs:218-374) in full.
2. Search `\bderive_source_evaluated_mechanisms\b`: **4 hits** (source_imports.rs:2, settlements.rs:2).
3. Input: `rows.selected_external` is `None` (source_imports.rs:55-57 `continue`) while `selected_count > 1` from multiple non-external `ProviderBinding` rows matching the same requirement (increment at source_imports.rs:304-304; external bindings only set `selected_external` at 305-313).
4. **Verdict: REACHABLE** — A demanded boundary with two matching selected non-import/intrinsic rows is not diagnosed here; the `selected_count > 1` error (source_imports.rs:66-70) runs only when `selected_external` is `Some`.
5. Unsure whether `terminal_authority_review` always fails when the mechanism list omits that boundary (reviewer bodies not read in full).

### A0662

1. Read `ConfigurationLease::require_masked` in full (scopes.rs:556-578) and caller loop (scopes.rs:228-232); read `changes_configuration` (scopes.rs:581-589) for contrast with `require`.
2. Search `\brequire_masked\b`: **2 hits** (scopes.rs:232, 556).
3. Input: operations in masked installed-handler bodies that are not `RegisterAccess` + `InterruptControl(Enable)` — e.g. `RegisterAccess` `Write` mapping to `SystemWrite` (read control_flow.rs:2137-2139) — pass `require_masked`.
4. **Verdict: REACHABLE** — A masked handler body could enable nested maskable arrivals via a flags/system-register write without tripping the single Enable pattern check.
5. Unsure which register writes lowering can emit into installed interrupt helpers (selection/legalization not read).

### A0663

1. Read `filesystem_mechanism_row` (host_cohorts.rs:123-135), `settled_filesystem_cohort` (host_cohorts.rs:44-89), and `cohort_mechanism_row` (terminal_authority_policy.rs:247-264) in full; read `settled_host_cohort_row` (source_imports.rs:190-216).
2. Search `\bfilesystem_mechanism_row\b`: **2 hits** (host_cohorts.rs, terminal_authority_policy.rs).
3. Input: non-Windows `NormalizedForeign` mechanism plus `ServiceMethod` whose `name` maps to a narrow or empty cohort (e.g. `sync` → empty facets at host_cohorts.rs:83-84) while the admitted locator/plan carries broader effects; Windows bypass at terminal_authority_policy.rs:255-258 uses `windows_imports::mechanism_row`.
4. **Verdict: REACHABLE** — Portable import admission can mint a cohort row from method name alone (`cohort_mechanism_row` → `filesystem_mechanism_row`) without comparing to the full foreign contract, accepting a disposition mislabeled relative to real import behavior.
5. Unsure whether closure review reclassifies from calling-plan evidence (reviewer.rs not fully read).

### What I did not read

- Full `admit_native_providers` beyond lifecycle/interrupt hook lines.
- Stage-00 retained provider / external-binding validators end-to-end.
- `terminal_authority_review::Reviewer` classification for missing or mismatched mechanisms.
- Terminal/Psi rules that might forbid non-Record nested shapes in access-lifecycle schemas.
- Legalization/selection rules for which register-access forms appear in installed interrupt bodies.

SLICE COMPLETE
