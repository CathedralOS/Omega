slice_0111

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0855 | ff:function_fragments.rs:20219; tests :20644-20649 | REACHABLE | `cleanup_actions_match` admits strict subsets of required discards; `admit` can accept `ReturnUnit` via that path without full cleanup roster. |
| A0856 | EM/object_artifact.rs:3628; :3633-3641 | CLOSED | `_ => true` is only a local pairing step; custody must match `derive_completion_provider_custody`, and replay re-checks realization at ff:22988-22991. |
| A0857 | EM/object_artifact.rs:1170 | REACHABLE | `ram::validate` `continue`s on most abstract ops, so installed-RAM paths are not proven for every memory-relevant operation. |
| A0858 | EM/object_artifact.rs:1673-1675 | REACHABLE | Missing `descriptor_input` returns `Some(())`, so guarded foreign calls can pass when the record omits guard metadata. |
| A0859 | EM/object_artifact.rs:2594-2609 | REACHABLE | Several argument sources record the whole call span with no per-argument transport verification. |
| A0860 | EM/entries.rs:328 | CLOSED | `_ => {}` only skips extra privileged state; `footprint.decoded` at :248 still runs for every instruction family. |
| A0861 | EM/entries.rs:3891-3896 | CLOSED | `Ok(None)` is the intentional no-shim path; `prepared_parts` at :3794-3800 still builds a valid image from the artifact. |
| A0862 | EM/entries.rs:5575-5594 | CLOSED | `None`/`Some` merge is loose, but `verify` at :5638-5644 rejects merged rows that disagree with retained private metadata. |

### A0855

1. Read in full: `byte_input::cleanup_actions_match` at `omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments.rs:20219-20277` (read).
2. Search `cleanup_actions_match` in `omega-rust/`: 8 hits in `function_fragments.rs` (1 production alias use, 1 re-export, 5 test asserts, 1 negative test) (read).
3. Production caller chain: `read_result_cleanup_actions_match` at `:18994` inside `admit` operation loop `:18698-19025`; `admit` called from `validate` via `source::admit(source)` at `:24432`; `validate_function_fragment_object_artifact` at `:24402-24424` (read). Did not read full `validate` past `:24500`.
4. Input reaching accepting branch: `AbstractOperation::ReturnUnit` with nonempty `cleanup_actions` that is a proper subset of the hosted-read-byte discards implied by the function’s structural contract and boundary settlements (read from `:20233-20276` and tests at `:20644-20649`).
5. Verdict **REACHABLE**: A unit-entry function with two affine hosted-read-byte results but a `ReturnUnit` listing only one `DiscardRoot` should be rejected for incomplete cleanup; `cleanup_actions_match` returns true for the one-action slice (`:20247-20276`, tests `:20645-20646`), and `admit` accepts when the third disjunct matches at `:18990-18994` even if `control_flow::retained` is false because abstract and target cleanup rosters differ (`:10503-10505`, read).
6. Entry description accurate on subset/reorder; empty list is rejected by `!actions.is_empty()` at `:20247` but `ReturnUnit` still admits via `cleanup_actions.is_empty()` at `:18992` without calling `cleanup_actions_match` (read).

### A0856

1. Read in full: `completion_receipts::validate_completion_custody` at `object_artifact.rs:3573-3642` (read).
2. Search `validate_completion_custody`: 3 hits in `function_fragments.rs:23027`, plus tests in `object_artifact.rs` (read); did not read every test block.
3. Caller chain: fragment replay settlement validation at `function_fragments.rs:23027-23028` errors on custody failure; `derive_completion_provider_custody` equality at `object_artifact.rs:3633-3640` (read). Realization equality checked at `function_fragments.rs:22988-22991` before custody call (read).
4. Input reaching `_ => true` at `:3628`: execution/realization pairs not listed in explicit true/false arms (e.g. `AdmittedProvider` with non-hosted realization variants) (read).
5. Verdict **CLOSED**: permissive `_ => true` does not complete acceptance; `:3630-3640` returns `ProviderCustody` on mismatch, and production replay requires `row.realization == expected.realization` at `:22988-22991` before `validate_completion_custody` runs.
6. Unsure whether every caller of `validate_completion_custody` outside replay enforces realization equality; only traced replay and `native-artifact` `completion_receipts_have_exact_custody` reference (grep read).

### A0857

1. Read in full: `ram::validate` at `object_artifact.rs:1089-1188` (read).
2. Search `ram::validate` in `omega-rust/`: 1 hit, definition + call at `:1250` (read).
3. Caller: `installed_body_fault_premises::derive` at `:1192-1259`, only when installed fatal entries exist (`:1198-1200`, read); followed by `observe_native_body_cpu_state` at `:1260-1264` (read, not full body).
4. Input reaching `continue` at `:1170`: abstract body operations other than `BoundaryCall`, `ExternalCompletion`, or interrupt-helper `BoundaryCall` kinds that `continue` at `:1157-1159` (read).
5. Verdict **REACHABLE**: In an installed-interrupt RAM closure, an abstract operation kind that ought to correlate with a unique in-closure machine call but is not one of the three examined owner kinds is skipped; a body with such an operation could pass `ram::validate` without the “unique concrete RAM-preserving body call” check at `:1174-1177`.
6. Did not enumerate all `AbstractOperation` variants to prove one exists on this path (inferred gap).

### A0858

1. Read in full: `descriptor_input::validate` at `object_artifact.rs:1668-1739+` (read through guard materialize checks to `:1738`).
2. Search `\bdescriptor_input::validate\b`: 1 production call at `:1969` (read).
3. Caller: `derive_normalized_foreign_call_custody` at `:1969-1970` maps `None` from `validate` to `InvalidForeignCallProjection` (read); called from `function_fragments.rs:2611` and `:3762` during artifact derivation (grep read). Did not read full derive entry paths.
4. Input reaching accepting branch: `record.call.descriptor_input == None` at `:1673-1674`, returning `Some(())` without searching for a guard instruction (read).
5. Verdict **REACHABLE**: A normalized foreign call that requires a descriptor-input guard but whose selected record omits `descriptor_input` returns `Some(())`; derivation then continues past `:1969` only if `validate` returns `Some`, so the artifact can retain the call without guard proof.
6. Did not trace whether lowering always sets `descriptor_input` when a guard is mandatory (would decide CLOSED).

### A0859

1. Read `foreign_scalar_argument_custody` match arm at `object_artifact.rs:2593-2608` and `IntegerImmediate` branch with `scalar_immediate_span` at `:2610-2633` (read); did not read full function `:2486-2700`.
2. Search `foreign_scalar_argument_custody`: definition + call at `:2040` inside `derive_normalized_foreign_call_custody` loop (read).
3. Caller chain same as A0858 derivation loop `:1951-1971` (read partial).
4. Input reaching accepting branch: scalar arguments whose `TargetUnitScalarArgumentSource` is `Parameter`, `BlockParameter`, `BooleanImmediate`, or `IeeeFloatImmediate`; span is entire call via `fragment_instruction_span` at `:2607-2608` with no operand-level transport check (read).
5. Verdict **REACHABLE**: A foreign call whose register/stack transport for a parameter-origin argument is wrong but the call instruction span is still known would record custody using the call-wide span and skip the per-argument transport proof used for `IntegerImmediate` at `:2621-2633`.
6. Did not read whether later replay compares scalar argument spans byte-for-byte (not traced).

### A0860

1. Read `body_footprint::collect_from` match on `row.alternative.family` at `entries.rs:249-328`; enclosing function `:89-337` (read).
2. Search callers: `observe_native_body_cpu_state` at `entries.rs:850-857` → `body_footprint::collect` (read); also `object_artifact.rs:1260` after `ram::validate` (read).
3. Input: physical interrupt instructions whose `MachineAlternativeFamily` is not one of the explicitly handled variants hit `_ => {}` at `:328` after `footprint.decoded(model, encoded)?` at `:248` (read).
4. Verdict **CLOSED**: unlisted families still contribute decoded encoded effects at `:248`; `:328` only declines extra privileged state union, not validation bypass on bad input.
5. Did not read full `MachineAlternativeFamily` enum to list unhandled variants (inferred).

### A0861

1. Read `hosted_unit_entry::prepare` at `entries.rs:3880-3897` (read).
2. Search `hosted_unit_entry::prepare(`: calls in `image.rs:157,238,1830,1911` (grep read).
3. Input reaching `Ok(None)`: not a free-unit entry per `has_free_unit_entry` at `:3891-3892` (`function_fragments.rs:3972-3985`, read), or `unit_entry_form(artifact.target)` is `None` at `:3894-3895` (`:3817-3877`, read).
4. Verdict **CLOSED**: early `Ok(None)` is defined behavior; `prepared_parts` at `:3790-3807` uses base artifact bytes when no shim, which is correct when no adapter applies—not fail-open acceptance of an invalid free-unit entry on a supported target (supported targets get `Some` from `owned_stack::prepare` at `:3897`).
5. Unsure about rare `NativeTarget` values missing from `unit_entry_form` with free-unit entry (inferred edge).

### A0862

1. Read `EntryOrigins::new` merge loop at `entries.rs:5571-5598` and `verify` at `:5603-5649` (read).
2. Search `EntryOrigins::new`: `object.rs:900`, `object_artifact.rs:188` (grep read); both followed by `verify` (read at `:909`, `:188`).
3. Input: duplicate identity rows where one side has `None` for fields 3–5 and the other `Some`; `:5575-5577` only errors when both sides are `Some` and differ (read).
4. Verdict **CLOSED**: lax merge is reconciled by `verify`, which requires `entry_bridge.installed_interrupt() == plan` and `installed == plan.is_some()` at `:5638-5643`.
5. Did not read full `collect` at `:5652+` for ordering invariants (read partial at `:5667-5724`).

### What I did not read

- Full `foreign_scalar_argument_custody` after line 2640.
- Full `descriptor_input::validate` after line 1738.
- Full `function_fragments::validate` after line 24500.
- Full `derive_completion_provider_custody` in `machine_code`.
- Complete `MachineAlternativeFamily` enumeration.
- `control_flow::return_cleanup_calls_match` and hosted-read cleanup interaction beyond cited lines.

SLICE COMPLETE
