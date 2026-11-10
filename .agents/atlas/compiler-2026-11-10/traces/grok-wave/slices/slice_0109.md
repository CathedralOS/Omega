slice_0109

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0839 | NA/.../internal_calls.rs:474-497 | CLOSED | Conformance is pinned by struct equality before `unwrap_or_default`; Terminal Psi rejects absent/empty provider identities earlier. |
| A0840 | NA/.../fragment_custody.rs:401-404 | CLOSED | Past-end `register_operand` makes `scalar_immediate_span` return `None`, and custody rejoin fails with an error. |
| A0841 | NA/runtime_provisions.rs:172-179,233-236 | CLOSED | `ok_or` on requirement identity and row-matching reject empty identity pairs before admission records are kept. |
| A0842 | NA/runtime_provisions.rs:215 | CLOSED | Allowing `None` when `import()` is absent is intentional; import bindings still fail at 215-218. |
| A0843 | CC/terminal_layout.rs:191-203 | CLOSED | Backend trusts declared sizes; Terminal Psi already cross-checks Authored/AccessView envelopes before layout runs. |
| A0844 | CC/plans/call_plan_evaluation.rs:167-310 | REACHABLE | Structural validation alone can accept register assignments the policy would not assign; syscall path is the main equality-checked exception. |
| A0845 | CC/plans/call_plan_evaluation.rs:290-292 | REACHABLE | Result placements are not tracked in `occupied_registers`, so an indirect-result pointer can overlap a parameter register undetected here. |
| A0846 | CC/host_operations.rs:380-394 | REACHABLE | Non-matching Darwin symbols fall through to libSystem at image link time; no compile-time rejection on the bootstrap import path. |

### A0839

1. Read `derive_boundary_internal_call_child` in full (`internal_calls.rs:194-559`, read) and `installed_provider_internal_call_identity` (`settlement_identity.rs:205-233`, read).
2. Search `derive_boundary_internal_call_child` under `omega-rust`: **3 hits** (definition plus two call sites in `evidence.rs`).
3. Read callers `evidence.rs:240-251` and `evidence.rs:341-358` (read); Provider branch at `internal_calls.rs:317-364` rejoins `provider_candidates` by full struct equality before lines 474-497 run (`internal_calls.rs:344-351`, read).
4. Input on the accepting branch: a matched `ProviderConformance` row whose `provider_identity` or `candidate_identity` index exists but `module.identities.text(...)` is `None`, so `unwrap_or_default` yields `""` in the digest and role strings.
5. **Verdict: CLOSED.** `foundation_validation.rs:1902-1909` (read) requires provider/candidate indices and nonempty source-related identity text; `provider_and_target_installation.rs:329` (read) rejects empty selected `provider_identity`. Wrong programs should not reach this derivation with mismatched conformances.
6. Entry description matches the code (read).

### A0840

1. Read `fragment_scalar_argument_custody_rejoin` in full (`fragment_custody.rs:317-493`, read) and `scalar_immediate_span` (`object_artifact.rs:3221-3289`, read).
2. Search `fragment_scalar_argument_custody_rejoin` under `omega-rust`: **2 hits** (definition and caller at `fragment_custody.rs:241`).
3. Read caller `fragment_normalized_foreign_rejoin` (`fragment_custody.rs:237-250`, read).
4. Input: scalar argument whose `native_position` is absent from `register_positions`, so `register_operand` becomes `register_arguments` (past the end).
5. **Verdict: CLOSED.** `scalar_immediate_span` uses `instruction.operands.get(register_operand)?` (`object_artifact.rs:3242`, read) → `None` → `span_rejoins` false → `fragment_scalar_argument_custody_rejoin` false → `Err("... drifted from its projected custody")` at `fragment_custody.rs:250` (read). This blocks evidence rejoin, not compiler admission.
6. Entry description matches (read).

### A0841

1. Read `TerminalRuntimeHostProvisions::capture_native_admission` in full (`runtime_provisions.rs:148-249`, read) and `selected_row_matches_requirement` (`provider_evidence.rs:738-760`, read).
2. Search `TerminalRuntimeHostProvisions::capture_native_admission` under `omega-rust`: **2 production call sites** (`callable_objects.rs:39`, `runtime_provisions.rs:606`; read signatures only).
3. Read `callable_objects.rs:33-58` (read): failure becomes `realization_error`, blocking callable preparation.
4. Input: `identities.text(boundary.identity).unwrap_or_default()` and `binding.requirement_identity(...).unwrap_or_default()` passed to `selected_row_matches_requirement` (`runtime_provisions.rs:172-178`, read).
5. **Verdict: CLOSED.** Line `182-184` (read) requires `requirement_identity` via `ok_or` (not missing). Line `172-180` (read) rejects row mismatch. Fallback `requirement == selected_requirement` at `provider_evidence.rs:759` (read) only helps when both sides are empty; Terminal Psi foundation validation keeps boundary/requirement identities nonempty for admitted modules (`foundation_validation.rs:1893-1909`, read). I did not read every `selected_row_matches_requirement` caller (8 other sites from grep).
6. Entry partially right: behavior on `("", "", id)` is read at `provider_evidence.rs:759` (both empty → `true` if no table rows).

### A0842

1. Read `capture_native_admission` loop (`runtime_provisions.rs:159-240`, read) and `NormalizedForeignCallBinding::import` (`realizations.rs:273-284`, read).
2. Search `capture_native_admission` (same as A0841): **2 production call sites**.
3. Read admission error path at `callable_objects.rs:58` (read).
4. Input: `calling_application` is `None` when `external_bindings` has no matching row or row lacks an application (`runtime_provisions.rs:200-214`, read); `import()` is `None` for syscall/table/component mechanisms (`realizations.rs:264-284`, read).
5. **Verdict: CLOSED.** Line `215-218` (read) errors only when `calling_application.is_none() && binding.import().is_some()`. Non-import mechanisms are meant to proceed without an external-row application; imports without a complete application are rejected.
6. Entry matches (read).

### A0843

1. Read `resolve_structural_shape` in full (`terminal_layout.rs:146-220`, read) including the Authored/AccessView/Accessor branch at `191-203`.
2. Search `resolve_structural_shape` under `omega-rust`: **5 hits**, all inside `terminal_layout.rs` (read grep output).
3. Read `StructuralLayouts::new` entry (`terminal_layout.rs:38-50`, read) and one consumer signature in `runtime/task-plans/src/activation.rs:278` (not read in full).
4. Input: Terminal record types with `Authored`, `AccessView`, or `Accessor` storage whose `byte_size`/`byte_alignment` are taken as authoritative without summing fields in this backend function.
5. **Verdict: CLOSED.** Terminal Psi validates Authored field layout and intervals (`foundation_validation.rs:8117-8162`, `8539-8548`, read) and runs access-view plan validation (`foundation_validation.rs:10283`, read). Backend layout assumes an already-validated module.
6. Entry matches (read).

### A0844

1. Read `validate_call_plan_structure` in full (`call_plan_evaluation.rs:167-310`, read) and `validate_call_plan` (`call_plan_evaluation.rs:153-165`, read).
2. Search `validate_call_plan_structure` under `omega-rust`: **3 hits** (`call_plan_evaluation.rs`, `plans.rs:420`; read grep).
3. Read `validate_boundary_entry_plan` (`plans.rs:405-412`, read), syscall equality check `settlements.rs:587-592` (read), and `NormalizedForeignCallBinding::has_valid_entry_mechanism` for imports (`realizations.rs:238-241`, read — no register equality). Read `external_calling_application` commitment gate (`settlements.rs:290-308`, read).
4. Input: any `CallPlan` that satisfies shape/disjointness/alignment but assigns registers differently from `evaluate_call_plan` for the same policy and signature.
5. **Verdict: REACHABLE.** A structurally valid plan with wrong-but-architecture-legal registers passes `validate_call_plan_structure` (read). Normal syscall admission compares to `evaluate_call_plan` (`settlements.rs:587-589`, read); import `has_valid_entry_mechanism` does not (`realizations.rs:238-241`, read). External ABI commitment is checked only when declared on the schema (`settlements.rs:295-308`, read). Smallest rejected case: a large-aggregate Windows call with parameter and hidden-result pointer both placed in the same integer register, structurally valid but not policy-canonical.
6. Entry matches (read). I did not trace every provider-planning caller.

### A0845

1. Read `validate_call_plan_structure` parameter loop and result block (`call_plan_evaluation.rs:189-292`, read).
2. Same search as A0844 (**3 hits**).
3. Read Microsoft indirect-result evaluation (`call_plan_evaluation.rs:452-516`, read) for contrast with policy-produced plans.
4. Input: `plan.result` with `ValueLocation::Indirect` pointer register equal to a parameter’s direct register; result validation at `290-292` does not add that register to `occupied_registers` (read).
5. **Verdict: REACHABLE.** Overlap is undetected in this function (read). Policy evaluation avoids the canonical Microsoft overlap (`call_plan_evaluation.rs:455,506`, read), but a hand-built or admitted structural plan can slip through the same gap described in A0844.
6. Entry matches (read).

### A0846

1. Read `darwin_import_library` in full (`host_operations.rs:380-395`, read).
2. Search `darwin_import_library\(` under `omega-rust`: **3 hits** (definition plus `dyld_linking.rs:117` and `:1608`; read).
3. Read both call sites (`dyld_linking.rs:115-125`, `1605-1611`, read): bootstrap / non-normalized import paths bind library from symbol prefix only.
4. Input: any Darwin import symbol that matches no objc/CG prefix; misspelled or erroneous symbol string.
5. **Verdict: REACHABLE.** Unmatched symbols are bound to `DARWIN_LIBSYSTEM_PATH` (read) with no diagnostic; image emission proceeds. Smallest rejected case: a program depending on a nonexistent external symbol that should fail resolution but instead links against libSystem.
6. Entry matches (read). I did not trace whether symbols are validated earlier in the emission pipeline beyond the empty-name check at `dyld_linking.rs:118-122` (read).

### What I did not read

- Full bodies of all eight non-`runtime_provisions` call sites of `selected_row_matches_requirement` (grep-listed only).
- Complete `provider-planning` and `retained_terminal_codec` paths that feed `BoundaryEntryPlan` values.
- Full `dyld_linking.rs` validators beyond the two `darwin_import_library` call regions.
- `activation.rs` consumer of `StructuralLayouts` beyond the grep hit.

SLICE COMPLETE
