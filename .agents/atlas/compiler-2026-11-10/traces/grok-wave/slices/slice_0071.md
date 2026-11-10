slice_0071

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0545 | rc:2818-2819 (cited rc:4818-4820 wrong) | CLOSED | `storage_index()` is `None` only for invalid `TypeIdentityId`; recast formation requires matching valid identities before the lender block. |
| A0546 | rc:1028-1050; rc:4660 | REACHABLE | Non-`Domain` constraints yield `qualification_compatible: None`, skipping the explicit refusal at rc:1605-1609; `ArithmeticDomain` is peeled without entering `domains`. |
| A0547 | rrc:3354-3356; rrc:4516-4535 | CLOSED | `Image` carriers are intentional non-suppliers (rrc:151-178); ordinary `Place`/`Borrow` paths still enforce loan capture when `present` would be true. |
| A0548 | rrc:8795-8799 | REACHABLE | `Invocation`-rooted residual sources return true without comparing call arguments, unlike parameter-indexed sources at rrc:8805-8827. |
| A0549 | rrc:7020-7022 | REACHABLE | `supply_mode != CheckedShape` makes `is_reference_record` false, so record-loan obligations via `local_owes_record_loans` (rrc:8016) do not apply. |
| A0550 | rrc:7685-7687; rrc:7742-7747 | CLOSED | Early exits intentionally emit no reference leaves for `NumericOwned` and activation byte carriers by design of `declared_carrier_references`. |
| A0551 | scc:642-656 | REACHABLE | Unspellable `where` facts are skipped in `requirement_rows`; fence `declaration_facts_carried` has no production caller while `state_declaration_predicates` runs in 05. |
| A0552 | scc:438 | CLOSED | `is_none_or` lives only in `complete_owned_storage`, reachable solely via `declaration_facts_carried`, which has no production callers. |

### A0545

1. Cited `rc:4818-4820` at commit `457adf4a11f` is float-interval narrowing (`narrow_to_float_interval`); relocated to `type_has_atomic_storage` at `omega-rust/psi/compilation/validation/src/recasts.rs:2811-2853` (read in full).
2. Search: `rg -w type_has_atomic_storage omega-rust` → **3 hits**, all in `recasts.rs` (definition, import, `form_scalar_recast` call at rc:1601).
3. Callers read: `validate_recasts` (rc:5292-5336) → `judge_scalar_recast` (rc:1448) → `form_scalar_recast` (rc:1521-1738, read through lender block rc:1600-1655); did not read every `judge_scalar_recast_alternative` branch.
4. Accepting branch input: any `TypeReferenceHandle` in the BFS where `program.normalized_type_identity(reference).storage_index()` is `None` causes immediate `return false` (rc:2818-2819), so `shared_mutation` is false at rc:1601 and the `(writable_view \|\| shared_mutation)` lender block at rc:1611-1654 may be skipped when `writable_view` is also false.
5. **Verdict: CLOSED.** `storage_index()` is `None` only when `TypeIdentityId` is invalid (`omega-rust/psi/compilation/representations/typed-trees/src/type_identity.rs:3002-3007`, read). `form_scalar_recast` requires `program.normalized_type_identity(let_referee) == program.normalized_type_identity(cast.target_type)` (rc:1662-1665, read) before returning `RecastFormation`, so the accepting branch is not reached for accepted recasts with valid identities.
6. Unsure: whether every node visited in the BFS always carries a valid identity when the root pair passes rc:1662-1665; did not trace `normalized_type_identity` assignment for all nested carriers.

### A0546

1. `declared_qualifications` rc:1028-1050 (read in full); `mutable_scalar_representation_facts` constraint loop rc:4649-4667 (read; cited rc:4660 is the `ArithmeticDomain` arm).
2. Search: `rg -w declared_qualifications omega-rust` → **8 hits** (6 in `recasts.rs`, 2 unrelated names in `execution_terminal_unit_types.rs`); `rg -w qualification_view_compatible omega-rust` → **3 hits** in `recasts.rs`.
3. Callers read: `qualification_view_compatible` rc:972-985 → called from `form_scalar_recast` rc:1602-1604, 1614-1618; `qualification_carrier` rc:1019-1025 uses `unwrap_or(reference)` when `declared_qualifications` is `None`. Upward: `validate_recasts` → `judge_scalar_recast` (rc:5292-5336). Did not read all `mutable_scalar_representation_facts` callers (8 hits in `recasts.rs`).
4. Accepting-branch input: a type reference whose constraint stack includes `TypeConstraintNode::Named(_)` or invalid `Domain` (rc:1039-1040 returns `None` for the whole peel) or peels `ArithmeticDomain` without adding to `domains` (rc:4660).
5. **Verdict: REACHABLE.** Smallest rejected program: a shared qualification-preserving recast where the source or target qualified type carries a `Named` constraint so `declared_qualifications` returns `None`, `qualification_compatible` stays `None` instead of `Some(false)`, and the refusal at rc:1605-1609 does not run, even though domain qualifications are not shown to be preserved.
6. Unsure: whether typed-tree construction blocks `Named` constraints on qualification carriers before `validate_recasts`.

### A0547

1. `expression_leaf_present` rc:3284-3401 (read in full); return-capture loop rc:4516-4534 (read).
2. Search: `rg -w expression_leaf_present omega-rust` → **6 hits**, all in `reference_result_custody.rs`.
3. Callers read: return-origin builder (~rc:4480-4535); `captured_input_sources` rc:2856-2865 (partial). Upward: structural return / reference custody validation in `reference_result_custody.rs`; did not read every caller of the enclosing return-plan builder.
4. Accepting-branch input: structural value whose `stored_carrier_sources` includes `StoredCarrierSource::Image` (rc:3354-3355 counts as not supplied); uncaptured return leaf where `expression_leaf_present` is false so rc:4532-4533 is not taken.
5. **Verdict: CLOSED.** `Image` is produced only for `NativeCodeLayout` field selection and `ByteLiteralView` (rrc:151-178, read); for those carriers, not requiring a borrow loan on a residual leaf is intentional. Ordinary programs using `Place`/`Borrow` suppliers still require `present == true` to fail capture at rc:4532-4533.
6. Unsure: none for ordinary `Place`/`Borrow` paths.

### A0548

1. `transient_call_record_loans` rc:8769-8830 (read in full); accepting branch rc:8795-8799.
2. Search: `rg -w transient_call_record_loans omega-rust` → **2 hits** (definition rrc:8769; call scc:3600).
3. Caller read: `structural_call_custody.rs` rc:3595-3611 — failure yields `Err("reference record result has no exact captured leaf loans")`; did not read full enclosing call-validation function.
4. Accepting-branch input: `CheckedReturnedReferencePlan` with `CheckedReferenceOriginRoot::Invocation` on a residual path while other filters pass (rc:8789-8793).
5. **Verdict: REACHABLE.** Smallest rejected program: a reference-record call whose residual leaf is attributed to an invocation root but actual arguments do not match the origin plan, accepted because rc:8797-8798 returns true without reading `arguments`.
6. Unsure: how often 05 emits invocation-rooted residuals on record calls in ordinary code.

### A0549

1. Inner `visit` in `is_reference_record` rc:6992-7032 (read in full); cited rc:7020-7022 is the `supply_mode` guard.
2. Search: `rg -w is_reference_record omega-rust` → **19 hits** across validation and 05 pipeline (count from ripgrep file list).
3. Callers partially read: `local_owes_record_loans` rc:8012-8018 (read); `reference_record_call_sources` rc:8762; structural call gate scc:3595. Did not read every 05 hit.
4. Accepting-branch input: a nominal data type with `supply_mode != CheckedShape` that structurally stores references; `visit` returns `None` (rc:7020-7021), so `is_reference_record` is false (rc:7037-7043).
5. **Verdict: REACHABLE.** Smallest rejected program: a boundary or opaque carrier typed as non-`CheckedShape` data that still holds reference fields, accepted as non-record for `local_owes_record_loans` and related record-call checks.
6. Unsure: which `supply_mode` values ordinary surface types use at validation time.

### A0550

1. `declared_carrier_references` / inner `visit` rc:7645-7752 (read through cited lines).
2. Search: `rg -w declared_carrier_references omega-rust` → **18 hits** (production uses in `reference_result_custody.rs` and tests).
3. Callers partially read: reference-source builders ~rc:1634, 4170, 4919; did not read all 18 sites.
4. Accepting-branch input: type satisfying `has_contents(..., NumericOwned)` (rc:7685-7686) or `activation_claim_byte_carrier` (rc:7742-7746), returning `Some(())` without pushing `DeclaredCarrierReference` rows.
5. **Verdict: CLOSED.** These branches deliberately treat numeric-owned and activation-byte storage as having no reference leaves; downstream callers use empty leaf lists, not “references absent when present.”
6. Unsure: none.

### A0551

1. `RowEmitter::requirement_rows` rc:627-689 (read in full); skip at rc:642-655 (`continue` when neither `spelling.boolean` nor `data_fact_element_consequences`).
2. Search: `rg -w declaration_facts_carried omega-rust` → **7 hits** (scc:361 definition; scc:2349, lib.rs:1518 re-exports; scc:1600, 2021, 2115, 2152 tests only).
3. Fence read: `definition_own_facts_carried` rc:471-498 (read); `declaration_facts_carried` rc:361-373. Production predicate emission: `state_declaration_predicates` rc:275-322 → `published.rs:9910`, `published_facts.rs:2676`, `execution_terminal_unit_state_graph.rs:9756` (grep hits; did not read those functions in full).
4. Accepting-branch input: a `where` proof fact on state-parameter storage that `FactSpelling::boolean` cannot spell and `collection_predicates::data_fact_element_consequences` cannot decode (rc:650-655).
5. **Verdict: REACHABLE.** Smallest rejected program: a machine state parameter whose closed data `where` facts include an unspellable predicate; compilation publishes incomplete `DeclarationPredicateRow`s because the fence is never invoked in production.
6. Unsure: whether 05 applies a separate predicate completeness check before installing rows.

### A0552

1. `complete_owned_storage` rc:409-447 (read in full); cited rc:438 is the `is_none_or` arm.
2. Search: `rg -w complete_owned_storage omega-rust` → **3 hits** (definition + two internal calls in scc). `rg -w declaration_facts_carried omega-rust` → **7 hits** (no non-test production call).
3. Callers: only `complete_definition_storage` rc:397-403 and recursive rc:439; `declaration_facts_carried` rc:366 has no production callers. Production predicate path uses `RowEmitter::descend` rc:539-540 (`owned_data_definition` missing → `return`, not `is_none_or`).
4. Accepting-branch input: carrier type with no `owned_data_definition`, for which `complete_owned_storage` returns `true` at rc:438-446.
5. **Verdict: CLOSED.** The accepting branch is confined to the unused `declaration_facts_carried` fence; it does not participate in `state_declaration_predicates` or other production acceptance paths.
6. Unsure: none.

### What I did not read

- Full bodies of every `judge_scalar_recast_alternative`, all `mutable_scalar_representation_facts` call sites, and all 19 `is_reference_record` call sites in 05.
- `published.rs` / `published_facts.rs` predicate-installation logic beyond grep-confirmed `state_declaration_predicates` calls.
- Entire `place_leaf_present` (rrc:3424+) and full return-origin builder above rc:4480.
- Whether typed-tree frontends forbid `Named` constraints or non-`CheckedShape` opaque reference carriers before validation.

SLICE COMPLETE
