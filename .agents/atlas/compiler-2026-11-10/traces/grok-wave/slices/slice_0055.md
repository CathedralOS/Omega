slice_0055

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0417 | ce:90950-90963 | REACHABLE | Empty-body `Unknown` with `goal_in_language == false` emits no error and does not record the ensure as proven, yet `judge_machines` can still finish with empty diagnostics. |
| A0418 | ce:90781-90784 | REACHABLE | When a mention-bearing ensure hits structurally contradictory requires, the fact is pushed into `proven.entry` with no contradictory-requires diagnostic. |
| A0419 | dc:133-135 | CLOSED | User `LateBound` public-interface selections are rejected at finalize (`shared.rs:1416-1422`) before visibility is re-checked; the `continue` only covers toolchain, descriptor-skipped, or intrinsic targets. |
| A0420 | dc:998-1064 | CLOSED | `Ok(None)` is an intentional exemption for toolchain, intrinsic, builtin, and toolchain-origin symbols; cross-package user selections still error or fail `allows_declaration_selection`. |
| A0421 | dc:933-934 | CLOSED | `excluded` rows are deliberately omitted from authority validation because those occurrences are not in the checked product frontier. |
| A0422 | dc:1105-1106 | CLOSED | `allows` returning `None` for `Absent` is used only in incremental reuse comparison and fails via `ok_or`; admission uses live `DeclarationSelectionAuthority`, not this tri-state. |
| A0423 | dc:1641-1646 | REACHABLE | Missing provenance makes `is_product_declaration` true via `is_none_or`, so provenance-less symbols can be treated as product declarations in completion discovery. |
| A0424 | dc:3151-3161 | REACHABLE | When `shape` returns `None` for non-Unit/primitive/data results, `terminal_results::validate` returns without emitting a mismatch diagnostic. |

### A0417 (ce:90950-90963)

1. Read `validate_machine_contract_entailment` from `contract_entailment.rs:90324` through the empty-body engine loop (`90873-90966`, read).
2. Read `Engine::judge` (`6779-6823`, read) — non-comparison ensures get `Judgment::Unknown { goal_in_language: false }` at `6818-6821` (read).
3. Search `\bvalidate_machine_contract_entailment\b` in `omega-rust/`: **4 hits** (3 in `program_validation.rs`, 1 definition in `contract_entailment.rs`).
4. Read callers `judge_machines` (`4727-4774`, read) and `validate_generic_machine_contract_entailment` (`181-199`, read); read `finish_diagnostics` (`4819-4826`, read).
5. Read parallel inductive unknown handling (`16355-16367`, read) — same pattern for non-language unknowns on non-empty machines.

**Input reaching the branch:** An ensure on a machine whose states have no statements, surviving structural filtering (`90860-90870`), where `engine.judge` yields `Unknown { goal_in_language: false }` and `premises_prove_expression` at `90951-90955` does not apply; diagnostic emission requires `goal_in_language && requires_fully_visible` at `90957-90962` (read).

**Verdict: REACHABLE.** A checked-body machine with empty state bodies and an ensures fact the arithmetic engine cannot encode (not a supported comparison polynomial) is neither proven nor rejected; `judge_machines` (`4764-4774`, read) keeps `machine_diagnostics` empty when no other errors fire, so compilation can proceed without recording that ensure.

**Unsure:** Whether downstream consumers always require every ensure to appear in `proven.entry` before codegen; `checks_contracts.rs:9015-9021` (read, not fully traced) only extends proven expressions when entry premises are preserved.

### A0418 (ce:90781-90784)

1. Read the `ensures.retain` closure in `validate_machine_contract_entailment` (`90712-90859`, read).
2. Read mention computation (`90713-90720`, read) and the `Some(held)` branch (`90764-90858`, read).
3. Same caller search as A0417 (**4 hits**).
4. Read related vacuous rule `premises_prove` / `judge.hypotheses_contradictory` at `90209-90216` (read).

**Input reaching the branch:** An ensures fact mentioning zero-value or proof-only data (`mention` is `Some`), after structural judging has set `structural.hypotheses_contradictory` true (`90781-90783`, read).

**Verdict: REACHABLE.** Contradictory requires do not produce a diagnostic in this path; the ensure is recorded proven and filtered out of the later engine pass (`return false` at `90783`). A machine whose requires are structurally inconsistent but cites a proof-only or zero-value ensure can compile with that ensure in `proven.entry`.

**Unsure:** Whether any earlier validator rejects all machines with `hypotheses_contradictory` before entailment; repository search for contradictory-requires diagnostics in validation (read, no matches).

### A0419 (dc:133-135)

1. Read `collect_declaration_visibility_diagnostics` (`119-177`, read).
2. Search `collect_declaration_visibility_diagnostics` in `omega-rust/`: **3 hits** (`declarations_core.rs:2`, `program_validation.rs:1`).
3. Read callers: prologue validator in `program_validation.rs:364-368` (read) and `validate_declaration_visibility` (`230-236`, read).
4. Read checking finalize path `checking.rs:1215-1233` (read) and `finalize_checked_authored_selections` guard `shared.rs:1386-1422` (read).
5. Read `normalize_checked_selection` `LateBound` arm `declarations_core.rs:1009-1013` (read).

**Input reaching the branch:** A `PublicInterface` authored selection where `selection.target().as_resolved()` is `None` (`133-135`, read) — `LateBound`, `Intrinsic`, or similar unresolved targets.

**Verdict: CLOSED.** Package-authored selections still `LateBound` after checking are rejected at `shared.rs:1416-1422` before `validate_declaration_visibility` runs again at `checking.rs:1233`. The skip at `dc:133-135` applies when targets stay unresolved only in excluded/toolchain/descriptor-skipped cases, or for intrinsics intentionally exempt from this visibility pass.

**Unsure:** None for the user-package public-interface case; toolchain public-interface intrinsics were not traced for separate visibility requirements.

### A0420 (dc:998-1064, 947)

1. Read `normalize_checked_selection` (`988-1074`, read) and `validate_authored_declaration_selections` (`905-951`, read).
2. Search `validate_authored_declaration_selections` in `omega-rust/`: **3 hits** (definition, re-export, `checking.rs:996`).
3. Read `admit_request` (`991-999`, read).

**Input reaching the branch:** Authored selections normalized to `Ok(None)` — toolchain requester file (`998-999`), intrinsic target (`1008-1009`), builtin/core symbols (`1020-1030`), or toolchain-origin selected symbol (`1063-1064`) — then matched by the `_ => {}` arm at `947` (read).

**Verdict: CLOSED.** These paths encode deliberate exemptions from package dependency authority, not silent acceptance of cross-package user selections; user-owned selections without package identity still return `Err` (`1065-1071`, read), and `Ok(Some(_))` without dependency permission still errors at `938-943` (read).

**Unsure:** Behavior when `request.declaration_authority` is `None` (`996` guard, read) — authority validation is skipped entirely, which is a separate admission policy choice.

### A0421 (dc:933-934)

1. Read `validate_authored_declaration_selections` loop (`921-948`, read).
2. Read `checked_selection_requirements` exclusion computation (`956-974`, read).
3. Same caller search as A0420 (**3 hits**).

**Input reaching the branch:** Checked selection rows with `excluded == true` from descriptor-skipped occurrences or pruned owning machines (`974`, read).

**Verdict: CLOSED.** Excluded rows are not part of the authority obligation set being validated; they represent occurrences outside the retained checking frontier, not unchecked cross-package selections in the product.

**Unsure:** None.

### A0422 (dc:1100-1110)

1. Read `RecordedDeclarationAuthority::allows` (`1100-1110`, read) and `capture` / `require_requesters` (`1082-1168`, read).
2. Search `recorded.allows(` in `omega-rust/`: **1 hit** (`source_compilation.rs:108`).
3. Read that caller (`95-118`, read).
4. Read `rejoin_source_authority` mapping `Absent => None` (`397-400` in `build-time-evaluation/src/lib.rs`, read) — related but distinct from `allows`.

**Input reaching the branch:** `RecordedDeclarationAuthority::Absent` when `allows(requester, owner)` is invoked (read, `1105-1106`).

**Verdict: CLOSED.** The sole `allows` call site treats `None` as a reuse mismatch (`ok_or(SourceReuseMiss::DeclarationAuthority)?` at `114`, read), not as permission to compile. Checked-tree admission uses `DeclarationSelectionAuthority::allows_declaration_selection` at `938`, not `RecordedDeclarationAuthority::allows`.

**Unsure:** Whether any test-only path interprets `None` as allowed; only the one production callsite was found.

### A0423 (dc:1641-1646)

1. Read `is_product_declaration` and callers in `source_completion` (`1549-1647`, read) — `prepare_source_completion`, `nominate_source_completion`, `has_external_source_candidate` (read).
2. Search `is_product_declaration(` in `declarations_core.rs`: **4 hits** (1 definition, 3 call sites, all read).
3. Did not read the separate `typed_trees::service_declarations::is_product_declaration` export (inferred distinct).

**Input reaching the branch:** Any symbol passed to `is_product_declaration` whose `symbol_provenance_source_span` is missing or whose source file lookup fails (`1642-1646`, read) — `is_none_or` yields `true`.

**Verdict: REACHABLE.** Provenance-less or compiler-generated trait symbols are classified as product declarations, so they can satisfy `is_product_declaration` filters in completion nomination (`1593-1600`, `1625`, read) without an authored product source file.

**Unsure:** How often real programs emit trait symbols with no provenance span that also match the `Console` path and conformance filters.

### A0424 (dc:3151-3161, 3184-3212)

1. Read `terminal_results::validate`, `shape`, and `spelling` (`3142-3221`, read).
2. Search `terminal_results::validate` in `omega-rust/`: **2 hits** (`declarations_core.rs:3302`, `3330`).
3. Read `validate_transition_target_node` caller context (`3279-3338`, read); did not read full statement-validation walkers above this module.

**Input reaching the branch:** A cross-machine terminal transition where `static_transition_type_context_is_unreachable` is false (`3151-3152`, read) but `shape` returns `None` for caller expected and/or callee actual return types because the type is not Unit, primitive, or a named data definition (`3197-3210`, read).

**Verdict: REACHABLE.** A terminal call whose callee normal result is a reference, slice, array, generic, or trait type can disagree with the caller state's return type without triggering `terminal_results::validate`, because the early `return` at `3159-3160` skips diagnosis.

**Unsure:** Whether another validator always rejects incompatible terminal return shapes before this runs; not traced beyond the two call sites.

### What I did not read

- Full `StructuralJudge::hypotheses_contradictory` propagation and all intake sites in `contract_entailment.rs` beyond cited regions.
- Entire `finalize_checked_authored_selections` in `shared.rs` except the `LateBound` rejection block.
- Downstream checked-tree contract publication in `checks_contracts.rs` beyond the `proven.entry` slice at `9015-9021`.
- `build-time-evaluation` selection paths beyond `require_declaration_selection_authority` (`4252-4300`, read) and `rejoin_source_authority` header.
- Corpus/runtime witnesses for reachable cases (read-only assignment; no build/run).

SLICE COMPLETE
