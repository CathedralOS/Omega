slice_0077

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0593 | ti-prov:1141-1146 | CLOSED | Omission is silent, but every compilation, provider, scalar-eval, and boundary-call path that needs a head uses `member()` / `entry()` and returns `MissingMember`. |

### A0593

1. Read `provider_and_target_installation.rs:1053-1074` (`selected_member`, `borrowed_selected_member`): `borrowed_selected_member` does `family.members.binary_search_by(|member| member.target.as_str().cmp(target)).ok()?` (`provider_and_target_installation.rs:1069-1072`, read); no row for the deployment tag yields `None`, so `filter_map` drops the family (`provider_and_target_installation.rs:1141-1146`, read).
2. Read `admit_target_machine_selections` in full (`provider_and_target_installation.rs:1137-1151`, read). It always returns `AdmittedTargetMachineSelections` with no error type (read); omission is the accepting behavior.
3. Read `AdmittedTargetMachineSelections::{member,entry,selections}` (`provider_and_target_installation.rs:973-1033`, read). `member` maps absence to `TargetMachineSelectionError::MissingMember` (`provider_and_target_installation.rs:1026-1032`, read). `entry` calls `member` for `TargetMachineCallable::Boundary` heads (`provider_and_target_installation.rs:1003-1006`, read).
4. Caller search: `rg 'admit_target_machine_selections\(' omega-rust --glob '*.rs'` → 47 hits across 22 files (read grep counts). Production callers read in full or in part: `artifact_admission.rs` `NativeScopePreparation::prepare` (`artifact_admission.rs:170-215`, read), `calling_policy_execution.rs:58-91` (read), `terminal-runtime/.../components.rs:29-71` (read), `retained_terminal.rs` `close_family_realizations` (`retained_terminal.rs:910-947`, read), `backend-plan/.../target_providers.rs:228-246` (read), `native-compiler/src/preparation.rs:55-69` (read).
5. Upward trace on those callers:
 - Native lowering: `selections.entry(..., root)` → `ArtifactLoweringError` on missing head (`artifact_admission.rs:184-186`, read).
 - Calling policy: same `entry` (`calling_policy_execution.rs:67-69`, read).
 - Runtime component prep: `admit_runtime_supplies` → `resolve_entry` → `selected.entry(...)` (`embedding.rs:2464`, `2042-2076`, read).
 - Family realizations: `selections.member(Boundary(head))` (`retained_terminal.rs:945-947`, read).
 - Provider roster: `target_members.member(family.canonical)` (`target_providers.rs:238-240`, read).
 - Boundary calls: `admitted_boundary_member` → `selections.member(head)` (`provider_and_target_installation.rs:905-925`, read); `SynchronousCallGraph::boundary_call` uses that (`evaluation_and_synchronous_calls.rs:1213`, read); interpreter `selected_member` uses `selections.member(head)` (`engine_calls.rs:1015-1017`, read).
 - Scalar evaluation: `ScalarApplicationObservations::entry` → `self.targets.entry(...)` (`scalar_applications.rs:2298-2310`, read).
6. `selections()` iterator gap: search `rg '\.selections\(\)' omega-rust/psi/interpreter/terminal-interpreter` with manual filter for target-machine map → one production use at `engine_execution.rs:638-642` (read): `targets.selections().iter().find(|selection| selection.head() == *entry)`; on miss, `continue` (`engine_execution.rs:645-646`, read), so `scalar_entry_conditions` may omit an application. That does not admit installation or lowering; scalar compile still hits `targets.entry` (`scalar_applications.rs:2307-2310`, read) and fails `MissingMember`. I did not read every `admit_target_machine_selections` test-only caller or every line of `execution_requirements` (`engine_execution.rs` ~400-668 partial).
7. Input reaching the accepting branch: a `TerminalArtifactView` whose `target_machine_families` includes families with no `members` row whose `target` string equals the requested deployment tag; `borrowed_selected_member` returns `None` for those families (read `provider_and_target_installation.rs:1065-1073`).
8. Verdict **CLOSED**. Checks: `member` (`provider_and_target_installation.rs:1022-1032`), `entry` (`provider_and_target_installation.rs:974-1007`), and `admitted_boundary_member` (`provider_and_target_installation.rs:925`, read). Any use of a missing family head on that target errors before native admission, provider admission, boundary dispatch, or scalar `Evaluate` entry resolution. Partial `selections` vectors are expected for multi-target artifacts when a family has no row for the chosen tag; rejection happens on lookup, not on map construction.
9. Entry description matches the code at the cited lines (read); digest concern about `selections()` consumers partially traced as above.

### What I did not read

- Full bodies of `engine_execution.rs::execution_requirements` outside the scalar-entry loop (`engine_execution.rs` ~400-668 except the cited loop).
- All 47 `admit_target_machine_selections` call sites (compiler tests under `omega-rust/omega/compilation/compiler/tests/`).
- `scalar_applications.rs` tests modules at the four call sites.
- Most of `build-time-evaluation/src/scalar_applications.rs` beyond the `execution_requirements` / `ScalarEntryPrerequisites::prepare` chain (`build-time-evaluation/.../scalar_applications.rs:92-115`, partial).

SLICE COMPLETE
