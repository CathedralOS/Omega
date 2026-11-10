# slice_0077

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0593 | ti-prov:1141-1146 | REACHABLE | Omitted families leave a hole that `selections()`-only requirement gathering never turns into `MissingMember`. |

## A0593

**Kind / digest.** ACCEPT, area_14 (preamble read: `digests/area_14.md` lines 1–52). Entry text: families with no member are omitted; consumers that iterate `selections()` “would not see the gap (not traced).”

**Cited site (read).** `omega-rust/psi/compilation/validation/terminal-installation/src/provider_and_target_installation.rs:1137-1152` (`admit_target_machine_selections`). Lines 1141-1146 match the digest: `filter_map(|family| selected_member(family, target))` drops families where `selected_member` is `None` (`read`, 1053-1063 via `borrowed_selected_member` 1065-1074). Function is infallible; it always returns `AdmittedTargetMachineSelections` with a possibly partial `selections` vector (`read`).

**Enclosing helpers (read).** `selected_member` / `borrowed_selected_member` (1053-1074): no member row for the deployment tag yields `None`, so the family is omitted. `AdmittedTargetMachineSelections::member` (1022-1032): binary search on `selections`; missing head → `TargetMachineSelectionError::MissingMember` (`read`). `selections()` (1018-1020): exposes the vector only (`read`).

**Caller search.** Pattern `\badmit_target_machine_selections\b` over `omega-rust/**/*.rs`: **58** matches in **25** files (`read`, ripgrep count mode). Plus **1** in `tests/omega/.../terminal.rs`. Not opened: every call-site body.

**Fail-closed routes (read).** Paths that need a specific family head call `member()` or `entry()` and error on omission: `admitted_boundary_member` (925), `entry` (1004-1007), `embedding.rs` entry resolution (2074-2076), `entry_settlement.rs` (794-800), `calling_policy_execution.rs` (67-69), `engine_calls.rs` `selected_member` (1015-1017), `02_abstract-operations-to-target-operations/src/lib.rs` (638-639), `artifact_admission.rs` `bind_selected_provider_coordinates` (420-422). These close only when that head is actually consulted.

**`selections()` consumer (read).** Production use of `AdmittedTargetMachineSelections::selections()` outside tests: `engine_execution.rs:637-646` (`read`). For a scalar catalog `Evaluate` entry whose `entry` is a `Boundary` head, the code does `targets.selections().iter().find(|selection| selection.head() == *entry)`; if the family was omitted at admission, `callable` stays `None` and the loop `continue`s (646) without error. `check_with_private_entry_ram` returns `TerminalExecutionRequirements` with a shorter `scalar_entry_conditions` list (`read`, 235, 658, 666). `embedding.rs:2729-2743` rejects only applications listed in `scalar_entry_conditions` (`read`); omitted conditions are never required. `calling_policy_execution.rs:108-112` merges `scalar_entry_conditions` into demand (`read`)—same under-demand if the family was omitted upstream.

**`member()` with silent skip (read, not `selections()`).** `native_execution_scope.rs:488-489` uses `targets.member(*entry)` but `continue`s on `Err` in `scalar_value_demand`; parallel silent gap, not the cited iterator.

**Smallest accepting scenario (plain words, no Omega code).** A Terminal Psi artifact for deployment tag T contains a target-machine family whose canonical boundary head has members for other tags but no row for T. The scalar-application catalog has an evaluate entry tied to that boundary head. Installation runs the infallible admission, which drops that family from the admitted list. Execution-requirement checking resolves boundary-headed scalar entries by scanning the admitted list only, skips the entry, and does not record a mandatory scalar entry condition. Interpretation proceeds without treating that scalar entry as required, even though the family is incomplete for T. If execution never invokes that boundary, `member()`/`entry()` closing checks need not run.

**Description check.** Digest matches code: omission at 1141-1146; `selections()`-only consumers can miss the gap. Digest “not traced” for those consumers is superseded here for `engine_execution.rs:637-646` (`read`).

## What I did not read

- Full body of every `admit_target_machine_selections` call site among the 58 omega-rust matches.
- `git show 457adf4a11f:.../provider_and_target_installation.rs` (cited lines match at current checkout at 1141-1146).
- Whether the Omega producer always emits a per-target member for every family before a shipped artifact can lack a row for T.

SLICE COMPLETE
