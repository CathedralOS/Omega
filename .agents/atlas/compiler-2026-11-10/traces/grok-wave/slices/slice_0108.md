slice_0108

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0831 | native_artifact/provider_execution.rs:140-169 | CLOSED | Every foreign-call mechanism is Import/Component/Table (same-stack branch) or SystemCall (kernel branch); bare `Ok(())` is not reachable without those checks. |
| A0832 | native_artifact/provider_execution.rs:37-40, 99-102 | CLOSED | Canonical Terminal modules require unique nonempty boundary identities, so `unwrap_or_default()` does not yield `""` on the emission/validate path. |
| A0833 | native_artifact.rs:992-1023 (via tests.rs:287) | CLOSED | Passthrough for non-specialized requirements is deliberate; external rows must have nonempty identities before replay accepts the plan. |
| A0834 | native_artifact/identity.rs:322-325 | CLOSED | Family and external plan identities are interned from present boundary strings; foundation validation rejects absent/empty boundary identities upstream. |
| A0835 | physical/derivation/evidence.rs:147-156 | CLOSED | Custody compare runs only with fragment publication; otherwise `validate()` replays foreign calls via `replay_provider_executions`. |
| A0836 | physical/derivation/evidence.rs:201-204, 261-270 | CLOSED | Same nonempty unique boundary-identity gate as A0832 prevents empty requirement text in plan lookup. |
| A0837 | physical/derivation/evidence.rs:60-65; native_artifact.rs:471-477 | CLOSED | `Blocked` is an explicit admitted gap sealed into artifact identity; emission is not a deny-all gate per crate README. |
| A0838 | physical/derivation/provider_custody.rs:29-37 | CLOSED | Early `Ok(())` applies only when identity is not a head/member in `family_realization_rows`; family rows are populated when family plans are retained. |

### A0831

1. Read `validate_foreign_provider_binding` and `replay_provider_executions` in full (`provider_execution.rs:20-350`, read).
2. Read `NormalizedForeignCallMechanism` and `has_valid_entry_mechanism` (`target-operations/.../realizations.rs:113-262`, read).
3. Search `\bvalidate_foreign_provider_binding\b` under `omega-rust/`: **2 hits** (definition + test).
4. Search `\breplay_provider_executions\b` under `omega-rust/`: **3 hits** (definition, `native_artifact.rs:556`, `dynamic_elf.rs:147`).
5. Read callers `NativeArtifact::validate` (`native_artifact.rs:521-584`, read) and test `kernel_provider_custody_rejects_substituted_plans_and_user_control_records` (`tests.rs:68-99`, partial).
6. Entry description is **partially wrong**: Import/Component/Table always have `same_stack_contribution` (`realizations.rs:116-127`); only `SystemCall` lacks it and enters the kernel branch (`provider_execution.rs:151-168`).
7. Input to accepting branch: only reached after lines 135-138 pass (binding requirement + valid mechanism); then stack or kernel validation runs before `Ok(())` at 170.
8. **Verdict: CLOSED** — check at `realizations.rs:113-130` plus branches at `provider_execution.rs:140-168`.
9. Unsure: hand-built artifacts via `from_replayed_parts` with impossible bindings (not a compiler emission path).

### A0832

1. Read `replay_provider_executions` (`provider_execution.rs:27-112`, read).
2. Search `\breplay_provider_executions\b`: **3 hits** (as above).
3. Read `NativeArtifact::validate` caller chain (`native_artifact.rs:556-563`, read).
4. Input: `module.identities.text(boundary.identity).unwrap_or_default()` when replaying settlements and foreign calls (`provider_execution.rs:37-40`, `99-102`).
5. Read Terminal foundation check `require_unique_nonempty_identities` for `boundary_machines` (`foundation_validation.rs:10342-10348`, read).
6. **Verdict: CLOSED** — canonical Terminal cannot supply missing boundary identity text to this replay.
7. Unsure: non-canonical hand-serialized modules (outside normal compile pipeline).

### A0833

1. Read test at `tests.rs:286-287` and `selected_requirement_identity` (`native_artifact.rs:992-1024`, read in full).
2. Search `\bselected_requirement_identity\b` under `omega-rust/`: **many hits**; production uses include `provider_execution.rs:134`, `evidence.rs` (via plan), `provider_custody.rs:96`.
3. Read `validate_external_requirement_applications` (`native_artifact.rs:941-985`, read).
4. Input: arbitrary requirement string; unknown nonspecial requirement returns `requirement` unchanged (`native_artifact.rs:1020-1023`); external/family paths require nonempty identity text (`941-953`).
5. Entry cited `tests.rs:287` — definition is in `native_artifact.rs`, not a separate file (re-located).
6. **Verdict: CLOSED** — identity passthrough for ordinary requirements; replay enforces external rows.
7. Unsure: none for emission path.

### A0834

1. Read `hash_identity_text` (`identity.rs:317-326`, read) and identity digest loop (`identity.rs:178-195`, read).
2. Search `\bhash_identity_text\b` in native-artifact: **8 hits** (all identity hashing).
3. Read `validate_external_requirement_applications` (`native_artifact.rs:941-953`, read).
4. Read family row construction (`retained_terminal.rs:712-728`, read).
5. Read boundary identity validation (`foundation_validation.rs:10342-10348`, read).
6. Input: `TerminalIdentityIndex` values in plan rows during identity recompute; absent indexes hash as empty bytes (`identity.rs:324`).
7. **Verdict: CLOSED** — emitted plans intern real identity strings; canonical Terminal forbids absent boundary identities.
8. Unsure: whether `from_replayed_parts` with crafted plan rows holding `ABSENT` indices could collide (not traced through full replay).

### A0835

1. Read `derive_physical_evidence` (`evidence.rs:67-462`, read in full).
2. Search `\bderive_physical_evidence\b` under `omega-rust/`: **4 hits** (`native_artifact.rs`, `dynamic_elf.rs`, definition, tests).
3. Read `fragment_publication` and scope helpers (`fragment_publication.rs:26-45`, `physical/mod.rs:248-257`, partial).
4. Input: when `fragment_publication` is `None`, `retained_custody` is `&[]` and line 154 compare is skipped (`evidence.rs:141-156`); object `foreign_calls` still drive derivation.
5. **Verdict: CLOSED** — `NativeArtifact::validate` calls `replay_provider_executions` (`native_artifact.rs:556-563`), which validates each foreign binding.
6. Unsure: whether `from_emitted_parts` without subsequent `validate()` is treated as “accepted” (README says emission is not deny-all; `README.md:19-26`, read).

### A0836

1. Read `boundary_identities` map and plan filter (`evidence.rs:195-271`, read).
2. Same caller trace as A0835 via `derive_physical_evidence`.
3. Input: missing identity text becomes `""` in map (`evidence.rs:201-204`); plan lookup uses that string (`261-267`).
4. **Verdict: CLOSED** — `foundation_validation.rs:10342-10348` prevents empty boundary identity text in canonical Terminal.
5. Unsure: same as A0832 for non-canonical modules.

### A0837

1. Read `blocked` helper and `CleanupAction` arm (`evidence.rs:60-65`, `170-176`, read).
2. Read `from_emitted_parts` / `into_parts` (`native_artifact.rs:289-321`, `471-477`, read) and `dynamic_elf.rs:60-65` (read).
3. Read `native-artifact/README.md:19-26` (read).
4. Grep `physical_evidence` in `publication.rs`: **no matches** (inferred: publication does not refuse gaps).
5. Input: unsupported foreign owner shapes yield `NativePhysicalEvidenceDerivation::Blocked` (`evidence.rs:170-175`); sealing stores gap (`native_artifact.rs:300`, `413`).
6. **Verdict: CLOSED** — explicit gap in identity is intentional admitted state, not silent success with complete evidence.
7. Unsure: downstream ecosystem receiving policy beyond native-artifact crate (not read).

### A0838

1. Read `validate_family_provider_custody` in full (`provider_custody.rs:20-85`, read).
2. Search `\bvalidate_family_provider_custody\b`: **2 hits** (definition + `evidence.rs:272`).
3. Read call site loop (`evidence.rs:261-279`, read).
4. Read family row retention (`retained_terminal.rs:712-728`, read).
5. Input: `identity` requirement string; if no `family_realization_rows` row matches head/member text, returns `Ok(())` (`provider_custody.rs:29-37`).
6. **Verdict: CLOSED** — skip is correct for non-family requirements; family rows are added when family members are selected (`retained_terminal.rs:717-728`).
7. Unsure: full `identity(member)?` failure modes in retention (not read end-to-end).

### What I did not read

- `publication.rs` in full (only grep for gap/evidence).
- Full `retained_terminal.rs` provider projection and all `identity()` helpers.
- Complete `foundation_validation.rs` beyond boundary-identity lines.
- Compiler product driver after `RequestedNativeArtifact::emit` (beyond grep for `.validate()`).
- Hand-crafted replay / deserialization attack surfaces beyond cited validators.

SLICE COMPLETE
