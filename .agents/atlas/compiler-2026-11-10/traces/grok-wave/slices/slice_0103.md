slice_0103

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0792 | `build-system/orchestration/src/package/package_review.rs:205-207` | CLOSED | `continue` only on `Ok(None)` (self-package) or pre-attached description; missing deps error upstream |
| A0793 | `compilation/backend/artifacts/boundary-applications/src/lib.rs:201-203` | REACHABLE | Float `validate_roster` never compares `provider_plan_commitment` unlike integer at `:304` |
| A0794 | `compilation/backend/artifacts/component-description/src/component_description.rs:334-336` | REACHABLE | Missing identity table entry becomes `""` and can flow through describe/verify self-consistently |
| A0795 | `compilation/native-compiler/src/realization/callback_thunks.rs:507` | CLOSED | `(None, None)` is intentional non-installed callbacks; installed paths require cohort/retain match or error |
| A0796 | `compilation/realization/provider-admission/src/provider_admission/terminal_authority_review/windows_handle_origins.rs:87` | CLOSED | Replay maps `ExternalInput`/`OpaqueObservation` to `ArgumentValues::Unknown` (`windows_handle_arguments.rs:250-254`) |
| A0797 | `compilation/realization/provider-admission/src/provider_installation.rs:219-229` | CLOSED | Filter drops only unselected catalog rows; selected requirements still hit `MissingSelectedProvider` in `select_provider_candidates` |
| A0798 | `compilation/realization/provider-admission/src/provider_installation/replay.rs:66-68` | CLOSED | Skipped calls caught by `validate_settlement_roster` (`coordination.rs:447-467`) when install evidence required |
| A0799 | `compilation/realization/provider-planning/src/intrinsic_declarations.rs:314-319` | REACHABLE | `binding.declaration_identity == None` skips declaration-identity check while other match predicates can still succeed |

### A0792

1. Read `component_descriptions` in full (`package_review.rs:185-217`) and `round.component` (`packages/package-manager/src/review/candidate/passes/occurrences.rs:620-655`, read).
2. Search `\bcomponent_descriptions\(` under `omega-rust/`: 4 hits in `package_review.rs` (definition plus call at `:151`); no other production caller of this local function.
3. Search `with_independent_component_descriptions` under `omega-rust/`: 9 hits across 6 files (read `package_compilation.rs:443-478`).
4. At `:205-207`, `continue` runs when `round.component` returns `Ok(None)` — only when `component == occurrence.key.identity()` (`occurrences.rs:627-628`, read), not when a dependency lacks a description.
5. Missing dependency: `None` in `component_candidates` yields `Compilation` error (`occurrences.rs:647-653`); cached without description yields `ComponentDescriptionNotRetained` (`:638-643`).
6. Input reaching the branch: the package under review listed among its own `required_component_packages()`.

**Verdict: CLOSED** — `with_independent_component_descriptions` (`package_compilation.rs:443-478`) does not require completeness, but this `continue` is not a missing-description bypass; real gaps refuse in `round.component`. Compile path requires descriptions (`build_execution.rs:252-270`, read caller only).

**Unsure:** Nothing material for the cited lines.

### A0793

1. Read `TerminalIeeeFloatComparisonOccurrenceProposal::validate_roster` in full (`boundary-applications/src/lib.rs:171-251`) and integer counterpart (`:291-306`, read).
2. Search `validate_roster` under `omega-rust/`: 11 hits; production use at `pipeline/00_terminal-psi-to-abstract-operations/src/settlement.rs:226-237` (read settlement loop `:154-190`, read).
3. Settlement sets `provider_plan_commitment` from `plans[provider_plan_index].identity_digest()` at creation (`settlement.rs:154-190`, read), so compiler-produced proposals are aligned.
4. Float roster at `:201-203` checks only index presence; integer at `:304-305` also compares `plan.identity_digest()` to `occurrence.provider_plan_commitment`.

**Verdict: REACHABLE** — A proposal record with a valid `provider_plan_index` but a wrong `provider_plan_commitment` passes float validation while integer validation would reject the same mismatch.

**Unsure:** Whether any post-`validate_roster` stage re-checks float commitment for terminal-boundary proposals (not read outside `settlement.rs` caller).

### A0794

1. Read `identity_text` (`component_description.rs:334-336`) and `derive_component_inventory` uses (`:567-684`, read sections).
2. Search `identity_text(module` in `component-description/`: 9 hits, all in `component_description.rs` and `component_verification.rs` (read verification `:748-771`, `:320-343`).
3. Accepting branch input: `TerminalIdentityIndex` with no text in the module identity table → `""` via `unwrap_or_default()`.
4. Describe builds inventory and description with that string (`:808-825`); verification requires derived/described key agreement, not non-emptiness (`component_verification.rs:740-771`, read).

**Verdict: REACHABLE** — Artifact whose boundary declaration references a missing identity index can yield a self-consistent description with an empty requirement identity that should be refused as ill-formed.

**Unsure:** Whether canonical Terminal production ever emits dangling identity indices (producer path not fully traced).

### A0795

1. Read `for_body` in full (`callback_thunks.rs:479-526`) and caller `lower_callback_thunk` (`:274-327`, read).
2. Search `\bfor_body\(` under `omega-rust/omega/compilation/native-compiler`: 1 production hit at `callback_thunks.rs:307`.
3. `(None, None) => Ok(None)` at `:505-506`; mismatch arms error at `:522-524`.
4. Installed handlers: `retain` fills slots (`:410-475`, read); `prepare_installed_callbacks` errors without matching settlement (`:166-170`, read).

**Verdict: CLOSED** — Input is a callback settlement with no cohort entry and no prepared install slot (ordinary thunk). Installed-interrupt placements that admit cohort membership must match retained evidence or fail before emission.

**Unsure:** Full admission producer for cohort membership (not read in `terminal_psi_to_abstract_operations`).

### A0796

1. Read `OriginKind` (`windows_handle_origins.rs:75-91`, read) and equation construction with empty `sources` (`:147-150`, `:185-229`, partial read).
2. Search `ExternalInput|OpaqueObservation` under `provider-admission/terminal_authority_review`: 9 hits; read consumer `windows_handle_arguments.rs:227-254`.
3. Consumer maps those kinds to `ArgumentValues::Unknown`, not proven empty reach (`:250-254`, read).

**Verdict: CLOSED** — Accepting branch carries unknown-origin equations; replay does not treat absent edges as empty physical reach (comment at `windows_handle_origins.rs:86-87`, read).

**Unsure:** Entire downstream Windows handle replay beyond argument-value pass (not read).

### A0797

1. Read `admit_terminal_provider_installation` in full (`provider_installation.rs:214-233`) and `select_provider_candidates` (`:235-316`, read).
2. Search `admit_terminal_provider_installation` under `omega-rust/`: 2 hits — definition and `build-settings/src/admission/provider_execution.rs:64` (read caller only).
3. Filter input: candidates whose `requirement_identity` text matches no selected adapter row are dropped before `select_provider_candidates`.
4. Selected boundary without a selected row still returns `MissingSelectedProvider` (`:278-280`, read).

**Verdict: CLOSED** — Dropped rows are catalog entries for requirements the build did not select; selected requirements still fail closed in `select_provider_candidates`.

**Unsure:** Nothing material.

### A0798

1. Read `replay_installed_provider_calls` in full (`provider_installation/replay.rs:24-318`, read through boundary-call branch and `or_insert` maps `:33-36`, `:66-68`, `:151-160`).
2. Search `replay_installed_provider_calls` under `omega-rust/`: 2 hits — definition and `provider_installation.rs:200` (read `admit_provider_installation_with_projection` caller only).
3. Skipped boundary calls: `continue` when no installed provider (`:66-68`, `:158-159`).
4. Downstream: `index_installed_provider_calls` plus `validate_settlement_roster` (`coordination.rs:430-475`, read) requires installed evidence or settlement for every `BoundaryCall` that needs it; `selected_profile_call` errors when no matching installed call (`access_lifecycles.rs:476-488`, read).

**Verdict: CLOSED** — Skipping is for calls with no install row; provider calls that require installation must appear in the indexed roster or lowering fails at settlement validation.

**Unsure:** Duplicate-key first-wins on `assumed_by_required` (`replay.rs:37-39`) under duplicate assumed bindings (not traced).

### A0799

1. Read `IntrinsicDeclaration::settle` in full (`intrinsic_declarations.rs:274-333`) and `Hosted` branch `:297-328` (read).
2. Search `settle_declaration|declaration\.settle` under `omega-rust/omega`: 5 hits in `selected-dispatch` (`compiler_intrinsic.rs:127-139`, `intrinsic_review.rs:246-252`, `:389-393`, read callers only).
3. Accepting branch input: `AcceptedIntrinsicBinding` with `declaration_identity: None` (e.g. `intrinsic_review.rs:220-223`) while `subject.matches`, plan schema/target, and shape checks succeed (`:307-319`, read).
4. `intrinsic_review` later requires each accepted binding be consumed exactly once (`:299-303`, read) but that does not restore declaration-identity disambiguation between hosted rows.

**Verdict: REACHABLE** — Two hosted rows that differ only by declaration identity could both match one binding with `declaration_identity: None`; settle accepts the first matching row without identity check.

**Unsure:** Whether planning always supplies `Some(declaration_identity)` on production `AcceptedIntrinsicBinding` paths (`produce_direct_boundary_leaves` uses `Some` at `:368-370`, read; `intrinsic_review` `unresolved` does not).

### What I did not read

- Full `windows_handle_replay.rs` and remaining origin derivation in `windows_handle_origins.rs` beyond cited consumers.
- Terminal producer invariants guaranteeing identity-table completeness and float proposal integrity.
- Full `02_abstract-operations-to-target-operations` lowering for every boundary-call variant beyond `coordination.rs` excerpts.
- `terminal_psi_to_abstract_operations` cohort admission that feeds `AdmittedInstalledInterruptCohort`.

SLICE COMPLETE
