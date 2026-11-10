slice_0093

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0712 | UV:8246 | CLOSED | TransformContent uses `validate_memberships`/`validate_contract`; generic issuer loop is for unsupervised additions Move still needs. |
| A0713 | UV:8770-8772 | CLOSED | Early `true` for unresolved machine/dynamic callees is superseded by `operation_structural_call_contract_matches` (dynamic: full projected-qual equality). |
| A0714 | UV:8868-8870 | REACHABLE | `Empty` payload evidence returns `true` without requiring result projected qualifications to match empty payload or `AllElements` shape. |
| A0715 | UV:9999-10029 | CLOSED | `None` from edge mapping becomes `ReachingWrite::Unknown`; residency checks treat set mismatch as violations, not pass. |
| A0716 | UV:10208-10210 | CLOSED | Case-segment paths on the slot declaration intentionally skip transfer rows; dynamic catalog constrains allowed `entry_claim_paths`. |
| A0717 | UV:10220 | CLOSED | `zip` pairs only equal-length args/params; every caller runs `structural_arguments_match` length guard first. |
| A0718 | UV:10749 | CLOSED | Non-`CallStructural` ops have no `selected_evidence` surface this helper is meant to police. |
| A0719 | UV:10803 | CLOSED | `exact_claim_free` only when plain/payloadless shapes already force empty or signature-matched claims before the early return. |

### A0712 (UV:8246)

1. Read `validate_structural_root_operations` match arm for `O::TransformContent` / `O::MoveStructuralValue` (UV:8093-8281, full arm read).
2. Search: `rg '\bvalidate_structural_root_operations\b' /agent/repos/omega/omega-rust` → **2 hits** (definition UV:7190, call UV:9610).
3. Call chain (read): `validate_function` (UV:9546) → `validate_structural_root_operations` (UV:9610) → loop returning `catalog_mismatch` on failure (UV:7301-7316); unit gate `validate_psi_optimization_unit_with_control_cycles` (UV:18260).
4. **Input to accepting branch:** `TransformContent` node that passed source/contract checks (UV:8145-8174) and content helpers (UV:8181-8244), then hits `continue` (UV:8246) instead of the additions loop (UV:8248-8281).
5. **Verdict: CLOSED.** `content_transformations::validate_memberships` (read `content_transformations.rs:2030-2082`) forces `result` memberships to equal inherited contract plus derivation outputs with `projection_owner` on each derived domain; unsupervised extra qualifications are not allowed on this path, which is what the skipped generic loop targets for `MoveStructuralValue`.

### A0713 (UV:8770-8772)

1. Read `reference_result_memberships_match` in full (UV:8754-8820).
2. Search: `rg '\breference_result_memberships_match\b' /agent/repos/omega/omega-rust` → **2 hits** (UV:7308, UV:8754).
3. Call chain (read): UV:7301-7316 → `catalog_mismatch` (UV:1068-1073); later same function pass, `validate_node_operation_contracts` → `operation_structural_call_contract_matches` (UV:10461-10468, UV:12457+).
4. **Input to accepting branch:** `CallStructural` where `callee.machine().and_then(|m| unit.function(m))` is `None` (UV:8770-8771), including dynamic targets and missing machine functions.
5. **Verdict: CLOSED.** Static/dynamic call contract matching requires a resolved callee or slot signature with full structural result match including `projected_qualifications` (`boundary.rs:705-709`, read via UV:12526-12528); machine calls without a function entry fail at UV:12592-12594.

### A0714 (UV:8868-8870)

1. Read `view_memberships_match` in full (UV:8822-8894).
2. Search: `rg '\bview_memberships_match\b' /agent/repos/omega/omega-rust` → **2 hits** (UV:7301, UV:8822).
3. Call chain: same as A0712 (UV:7301-7316, UV:9610, UV:18260).
4. **Input to accepting branch:** `EstablishElementView` or `ElementViewSubslice` with non-empty `result.projected_qualifications` (UV:8835-8837) and descriptor evidence `ElementPayloadMemberships::Empty` (UV:8868-8870).
5. **Verdict: REACHABLE.** Smallest bad case: an optimization unit carrying an element-view operation whose descriptor query yields zero payload targets (`descriptors.rs:3269-3270`, read) but whose structural result still advertises projected qualifications; this function returns `true` on the `Empty` arm without the `AllElements` checks used on `Present` (UV:8876-8891).

### A0715 (UV:9999-10029)

1. Read `validate_function_atomic_coherence` and its `projection_before_edge` closure (UV:9971-10067); read `happens_before_atomic_coherence_violation` and `reaching_writes` (`atomic.rs:414-603`, read).
2. Search: `rg '\bvalidate_function_atomic_coherence\b' /agent/repos/omega/omega-rust` → **2 hits** (UV:9971, UV:14470).
3. Call chain (read): `validate_values_and_bindings` (UV:14453) → `validate_function_atomic_coherence` (UV:14470-14475) → `?` propagates `AtomicEventCoherenceMismatch` (UV:10059-10065).
4. **Input to accepting branch:** CFG edge for parameter `root` where structural bindings count ≠ 1 (UV:10008-10016) or predecessor edges disagree on projected `(source, path)` (UV:10023-10024), so the closure returns `None` (UV:10028-10029).
5. **Verdict: CLOSED.** `None` makes `reaching_writes` insert `ReachingWrite::Unknown` (`atomic.rs:582-584`, read); `residency_edge_violation` compares `reached` to claimed frontiers and returns violations when sets differ (`atomic.rs:720-751`, read), yielding `Err` rather than silent acceptance.

### A0716 (UV:10208-10210)

1. Read `validate_slot_claim_transfers` in full (UV:10178-10212); read dynamic catalog slot `entry_claim_paths` checks (UV:1348-1385).
2. Search: `rg '\bvalidate_slot_claim_transfers\b' /agent/repos/omega/omega-rust` → **3 hits** (def UV:10178, re-export UV:10409, call UV:12532).
3. Call chain (read): `operation_structural_call_contract_matches` dynamic arm (UV:12483-12546) → inside `validate_psi_optimization_unit` function pass (UV:9647-9658).
4. **Input to accepting branch:** dynamic `CallStructural` where an `expected` row in `slot.entry_claim_paths` includes a `Case` segment and no matching `(argument_index, path)` appears in `claim_transfers` (UV:10206-10210).
5. **Verdict: CLOSED.** Exemption applies only to paths already declared on the slot and vetted at catalog time (UV:1365-1371); Case-bearing claim paths are a declared shape, not an arbitrary omission.

### A0717 (UV:10220)

1. Read `validate_internal_claim_transfers` in full (UV:10214-10317).
2. Search: `rg '\bvalidate_internal_claim_transfers\b' /agent/repos/omega/omega-rust` → **4 hits** (def UV:10214, calls UV:12481, UV:12688, import UV:12396).
3. Call chain (read): all call sites pair with `structural_arguments_match` / `structural_arguments_match_at_invocation` (UV:12473-12481, UV:12680-12693); length guard UV:13217-13218.
4. **Input to accepting branch:** argument/parameter pairs only when both slices share length enforced upstream.
5. **Verdict: CLOSED.** Truncating `zip` cannot drop extra actual arguments because arity mismatch returns `false` at UV:13217-13218 before this runs.

### A0718 (UV:10749)

1. Read `payloadless_selected_evidence_surface_matches` in full (UV:10735-10757).
2. Search: `rg '\bpayloadless_selected_evidence_surface_matches\b' /agent/repos/omega/omega-rust` → **3 hits** (UV:10735, UV:12399, UV:12624).
3. Call chain (read): `operation_structural_call_contract_matches` structural arm (UV:12623-12626) under `validate_node_operation_contracts` (UV:10461).
4. **Input to accepting branch:** any operation variant other than `CallStructural` with structural result carrying `selected_evidence` in the destructuring pattern (UV:10741-10749).
5. **Verdict: CLOSED.** Only `CallStructural` carries this evidence field; other kinds are out of scope for this check by construction.

### A0719 (UV:10803)

1. Read `validate_structural_call_result` in full (UV:10759-10821); read `plain_structural_call` (UV:10838-10878) and `exact_plain_affine_structural_call` (UV:9841-9903).
2. Search: `rg '\bvalidate_structural_call_result\b' /agent/repos/omega/omega-rust` → **4 hits** (def UV:10759, import UV:12400, call UV:12613, plus terminal codec name collision).
3. Call chain (read): UV:12613-12620 passes `exact_claim_free` as OR of plain detectors; failure makes `operation_structural_call_contract_matches` false (UV:10461-10468).
4. **Input to accepting branch:** structural call classified payloadless/plain (UV:12616-12620) after earlier signature/catalog checks (UV:10769-10801).
5. **Verdict: CLOSED.** Plain detectors require empty `claim_transfers` and result claims (`UV:10860-10863`, `UV:9846` via `plain_structural_call`); payloadless path equates `result.claims` to callee signature (UV:10725-10726) before `exact_claim_free` bypasses multiplicity-only path rules at UV:10812-10819.

## What I did not read

- Entire body of `validate_structural_root_operations` (~thousands of lines beyond cited match arms).
- Full `reference_call_matches` after UV:12810.
- Full `validate_contract` in `content_transformations.rs`.
- Complete `validate_dynamic_catalog` beyond UV:1320-1459.
- Psi producer lowering that emits `TransformContent` / element views (only validator read).

SLICE COMPLETE
