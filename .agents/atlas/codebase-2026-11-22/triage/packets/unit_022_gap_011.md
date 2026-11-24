# Packet unit_022_gap_011: stage 15 structural settlement replay

Packet complete. None of the 11 claims in this record shows a wrong result in the compiler. Ten are intended behavior or confirmed counts. One is still open as a code-quality item: the duplicated `refuse_symbol_collision` helper (C3). No code change was needed, so this packet carries no fixture.

Scope: [`records/unit_022_gap_011.md`](../../records/unit_022_gap_011.md), area `11-native-stages-11-15`, the range listed in [`units/gap_011.tsv`](../../units/gap_011.tsv): `function_fragments.rs` lines 19801 to 21313 in `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/`. Baseline `24ad2700685e6cc6062026c87ed5fd24ed415265`. The file is still 21,313 lines there, and every cited line below still holds the quoted code (test function names sit one line below the record's citations, at the `fn` line rather than `#[test]`). `FF` abbreviates that file. Earlier evidence used: digest 11 lines 94, 108, 212, 280 and 297. No open PR touched this range when the packet was written (checked against the open PR list on 2026-11-24). Nothing was built or run except `mbx xtask provenance` and searches. Every outcome comes from reading the code.

## Claims

### C1. Process-exit replay finds its span by first match (record §3 items 4 and 9; five-things 2; digest 11 line 212)

- **Code path.** `process_exit::validate` (`FF:20153-20158`) takes the first instruction span whose `instruction == located.anchor` across all blocks. The lookup that refuses duplicates is `settlement_offset` (`FF:20849-20874`), reached through `validate_settlement_position` (`FF:20514`).
- **Contract.** Object replay must refuse a fragment whose settlement anchor is not a unique physical span.
- **Trace.** `function_fragments/validation.rs:551-662` walks every placed function with no `continue`. For each one it calls `structural::validate_settlement_attributions` (`:655`), which calls `validate_settlement_position` for every located settlement, `HostedExitProcessI32` included (`FF:20498`). Only after that loop does `validate` call `structural::validate_settlements` (`validation.rs:701`), which reaches `process_exit::validate`. So the duplicate check refuses a duplicate anchor or block before the first-match lookup runs.
- **Outcome.** Intended (traced). This path does not accept a bad fragment.
- **Proposed fix.** None required. Optional tidy-up: have `process_exit::validate` call `settlement_offset` itself so the check no longer depends on call order.

### C2. Unknown settlement payloads fall through to the byte-output code (record §4 item 9; five-things 1; digest 11 lines 108 and 212)

- **Code path.** `validate_settlements` dispatches with `_ => byte_output::validate` (`FF:20392`), and `settlements` dispatches with `_ => byte_output::settlement` (`FF:20955`).
- **Contract.** Each payload must be checked by the code written for it, and an unexpected payload must be refused.
- **Trace.** `SelectedBoundarySettlementPayload` (`representations/selected-instructions/src/calls.rs:1528-1548`) has exactly four variants: `HostedReadByte`, `HostedExitProcessI32`, `ClaimCompletion` and `HostedWriteByteI32`. Both dispatch sites handle `ClaimCompletion` first and name the read and exit variants, so today the wildcard can only receive `HostedWriteByteI32`. Both `byte_output::settlement` (`FF:18970-18978`) and `byte_output::validate` (`FF:19053-19063`) start with `let HostedWriteByteI32 { .. } = located.settlement else { return Err(..) }`. A future variant would therefore be refused, not accepted.
- **Outcome.** Intended. The wildcard refuses rather than accepts, which resolves the record's "inferred; `byte_output` not read" hedge.
- **Proposed fix.** Optional hardening: replace `_` with `HostedWriteByteI32 { .. }` at both sites, so that adding a variant fails to compile rather than producing a runtime refusal.

### C3. `refuse_symbol_collision` and `host` exist in both stage 14 and stage 15 (record §6 item 6; five-things 5; digest 11 lines 94, 280, 297)

- **Code path.** `FF:21268-21281` and `14_placed-functions-to-objects/src/body_object.rs:62-75`.
- **Trace.** The bodies are identical except for the `ObjectPlan` path (`object_file::ObjectPlan` vs `crate::ObjectPlan`) and each crate's own `Error`. The stage 14 copy has three callers (`body_object/code_descriptors.rs:131`, `native_code_layouts.rs:94`, `image_residence.rs:33`). The stage 15 copy has one (`object.rs:542`, imported at `:429`).
- **Outcome.** Duplicate code, confirmed (matches digest 11 §10 item 12). It is not a correctness defect: both copies refuse on a name collision and accept only when no symbol matches.
- **Still open.** This is a code-quality cleanup, not an ordinary bug, so this packet makes no code change. Proposed fix: make the stage 14 helper `pub`, call it from `object.rs:542` and map its error with the existing `From<object_file::body_object::Error>` (`FF:21302-21312`). Then delete the stage 15 copy. `host` stays, because stage 15 uses it widely.

### C4. `refuse_symbol_collision` scans the symbol table linearly and compares names (record §6 item 4; digest 11 line 52)

- **Trace.** `FF:21277` runs `object.layout.symbols.iter().any(|(_, s)| s.name == name)`. Its only stage 15 caller runs once per private function symbol (`object.rs:542`).
- **Outcome.** Intended. A symbol-table collision is by definition a comparison of name spellings. The cost is quadratic in the number of private functions, and no run has measured it as a problem. Folded into C3.

### C5. `validate_function` refuses any `unit_affine_cleanup` (record §4 item 4; five-things 3)

- **Trace.** `prepare_object_for_image` always builds `unit_affine_cleanup: None` (`FF:2892`). Nothing else under `source-rust` writes the field (search for `unit_affine_cleanup`: only the definition at `object_artifact.rs:394`, the construction at `FF:2892` and the check at `FF:20218`). The check therefore refuses only an object that differs from what production builds.
- **Outcome.** Intended. Production and replay agree.

### C6. Production and replay select the parameter record set by the same key (record §4 item 4, §5 item 3; five-things 3)

- **Trace.** `populate` (`FF:20891`) and `validate_function` (`FF:20221`) both branch on `mixed_structural_scalar_abi.is_some()`. Validation also requires the unused set to be empty and the used one to equal the plan.
- **Outcome.** Intended.

### C7. `validate_incoming_location` is the exact table of accepted incoming-pointer placements (record §4 items 4 and 5; five-things 4)

- **Trace.** Every fallback arm returns `Err(invalid())` (`FF:20298`, `:20304`, `:20368`). The borrowed scalar descriptor is allowed only in the indirect arm (`!scalar_descriptor` at `:20329`, `:20343`). The four tests at `FF:20542`, `:20574`, `:20669` and `:20750` each pin an accepted case and mutated refusals.
- **Outcome.** Intended. The table fails closed, and the owning unit tests pin it.

### C8. `primitive_locals::tests` tail (record §1)

- **Trace.** The three tests at `FF:19839`, `:19885` and `:19994` mutate one field at a time and assert refusal. The record's only limit, that the tests cover just the `f32` to `u32` view on `linux_x64` plus 8-byte homes (`FF:19915-19918`), describes test coverage, not a compiler result.
- **Outcome.** Intended (test code). Wider coverage would be a test addition, not a repair, and no failing program calls for it.

### C9. Re-export `primitive_operation_retained` count not grepped (record §2; "unsure" item 2)

- **Trace.** Search for `primitive_operation_retained` over `source-rust`: one re-export (`FF:20046`) and one caller (`function_fragments/source/retained_source_validation.rs:650`).
- **Outcome.** Missing count supplied. Single caller, used.

### C10. Callers of `validate_settlements`, `validate_settlement_attributions` and `settlement_attributions` not grepped (record §4 item 7; "unsure" item 2)

- **Trace.** Each has exactly one caller: `validate_settlements` at `function_fragments/validation.rs:701`, `validate_settlement_attributions` at `validation.rs:655`, and `settlement_attributions` at `FF:2906`.
- **Outcome.** Missing counts supplied. Each has one caller, and each is used.

### C11. Other counts in the record (record §5 item 6, §6 item 7)

- **Trace.** `placed_text_offset` gives 15 lines and `mixed_structural_scalar_abi` gives 36 lines under `source-rust` at the baseline, as the record states. `refuse_symbol_collision` gives 10 lines (two definitions, four uses, four imports), as stated. The `From<installation_admission::cpu::Error>` conversion that goes through `reason.into()` (`FF:21224`) maps one value to one value.
- **Outcome.** Matched.

## Unresolved claims

- **C3:** duplicated `refuse_symbol_collision` in stage 14 and stage 15. Still open as code-quality cleanup, with the proposed fix above. No correctness impact.
