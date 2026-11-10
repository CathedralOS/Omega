slice_0070

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0537 | q:5551-5556 | CLOSED | Missing data def only passes when path is a builtin primitive; errors surface at q:880-888 |
| A0538 | q:7540-7549 | CLOSED | `NotQuotient` → exact equality is correct for non-quotient names; open/missing args error |
| A0539 | q:424-426 | CLOSED | `symbol_toolchain_relative_path` is toolchain-only; `validate_equivalence_selection` rejects lookalikes at q:46-51 |
| A0540 | rc:1483-1490 | CLOSED | Defer skips recast layout only while pending; lowering blocks terminal until strict re-check |
| A0541 | rc:1795-1800 | UNDECIDED | Pending formation defers here; full consumption coverage via `report_consumption` not traced |
| A0542 | rc:731-770 | REACHABLE | Cross-machine pending recasts are skipped in `pending_origins` when the consumer machine has no pending formation |
| A0543 | rc:1902-1907 | CLOSED | After `exact_scalar_representation_type`, `PrimitiveType::scalar_byte_size` is always `Some` |
| A0544 | rc:2307-2321 | REACHABLE | Non-scalar referee or initializer exits silently; only scalar–scalar mismatches get diagnostics |

### A0537

1. Read `validate_closed_type_argument` in full at q:5537-5574 (read).
2. Search: `rg 'validate_closed_type_argument' omega-rust` → 2 hits, 1 production call at q:5469 inside `validate_static_application` (read q:5440-5574).
3. Callers of `validate_static_application`: `derive_exact_representative_static_application` q:5432, nested q:5502 and q:5561, `derive_selected_theorem_telescope` q:5622; chain to `derive_direct_terminal_plan` q:6919 and `reject_quotient_operation_requests` q:650-657 (read q:5426-5628, q:622-888 partially).
4. Input on the accepting branch: `TypeParameterKind::Type` static argument with no `data_definition_by_symbol` for `argument.symbol`, no const/evidence, `application` none, single-segment `path` where `PrimitiveType::from_name` succeeds (read q:5547-5555).
5. **Verdict: CLOSED.** Check: `RelationPlanError` from failed closed-type validation is turned into a diagnostic at q:880-888 (`Err(reason)` arm of `derive_direct_terminal_plan`). Non-primitive missing definitions hit q:5557-5558. Only legitimate builtin primitive spellings return `Ok(())`.

### A0538

1. Read `exact_quotient_relation` at q:7536-7563 (read); read `derive_relation_and_representative` at q:7293-7377 (read).
2. Search: `rg 'exact_quotient_relation' omega-rust` → 3 hits, calls at q:7351 and q:7363 (read).
3. Input: resolved `argument_type` / `state.return_type` where `quotient_for_type` (q:505-514) returns `None` → `ExactRelationLookup::NotQuotient` → `PositionalRelation::ExactEquality` (read q:7351-7366).
4. **Verdict: CLOSED.** `quotient_for_type` only matches `data_definitions()` entries with `quotient.is_some()` (read q:510-513). Ordinary data names correctly use exact equality; `OpenApplication` and missing argument types error at q:7356-7359 and q:7349. Callers did not read beyond `reject_quotient_operation_requests` error paths (inferred safe for admission).

### A0539

1. Read `is_sealed_core_equivalence` at q:416-450 (read); read caller `validate_equivalence_selection` at q:17-52 (read).
2. Search: `rg 'is_sealed_core_equivalence' omega-rust` → 2 hits, call at q:46 (read).
3. Input: `Equivalence` static selection; `source_matches` requires `symbol_toolchain_relative_path(symbol) == relation.omg` (read q:424-426, constant q:8046).
4. **Verdict: CLOSED.** `symbol_toolchain_relative_path` returns `None` unless `SourceOrigin::Toolchain` (read typed-trees `lib.rs` 1578-1587). Non-toolchain symbols cannot satisfy q:424-426; q:46-51 emits an error and returns. Every `quotient.equivalence` selection goes through `validate_equivalence_selection` in `collect_validated_quotient_formations` (read q:603-610).

### A0540

1. Read `judge_scalar_recast` at q:1448-1511, early return q:1483-1490 (read).
2. Search: `rg 'defers_pending_layout_geometry' omega-rust` → production flag set at `checking.rs` 1034-1036 (read); `validate_recasts` caller at `program_validation.rs` 573 (read grep hit, not full `validate` coordinator).
3. Input: `program.defers_pending_layout_geometry()` true and `pending_layout_geometry` true for recast layout roots → return with no diagnostic (read q:1483-1489).
4. **Verdict: CLOSED.** Flag is set when `admits_pending_inputs || !deferred_layout_applications.is_empty()` (`checking.rs` 1034-1036, read). Terminal lowering rejects non-empty `deferred_layout_applications` (`machine_lowering.rs` 1681-1684, read). Executable completion requires a strict pass with defer false; this branch is harmless on the interim pending check only.

### A0541

1. Read `judge_scalar_recast_alternative` q:1742-1800 and pending-view early return q:1795-1799 (read); read `pending_view` q:601-623 and `form_scalar_recast` gate q:1710-1711 (read).
2. Search: `rg 'report_consumption' omega-rust` → 6 hits: `recasts.rs` 953, `program_validation.rs` 1640/2839/3657, `calls.rs` 40216, `literals_and_struct_literals.rs` 4289 (grep only; latter four not read in full).
3. Input: non-scalar recast target, `qualification_compatible` none, `pending_view(...)` `Some` → return before record/scalar refusal (read q:1795-1799).
4. **Verdict: UNDECIDED.** `validate_operand_consumptions` → `report_consumption` (read q:913-965, q:809-883) covers statement value operands, but whether every pending-view use is reached by those sites or the three `program_validation` / `calls` / `literals` call sites was not traced. A definitive CLOSED/REACHABLE needs that full consumption graph.

### A0542

1. Read `pending_origins` in full q:731-770 (read); read `withholds_default_predicate` q:772-806 and `report_consumption` q:830-841 (read).
2. Search: `rg 'pending_origins' omega-rust` → 4 hits in `recasts.rs` (read).
3. Input: `completed_reference_interpretation_sources` recasts whose `formation_owner` is another machine (`owner_machine.symbol != machine.symbol`) are skipped at q:757-758; if `!has_pending_formation(program, machine)` at q:738-739, returns empty `Ok` without scanning (read).
4. **Verdict: REACHABLE.** Smallest program shape: machine A forms a mutable pending-record recast; machine B consumes a reference derived from that formation without pending casts in B’s body—`pending_origins` on B returns empty and consumption proceeds without charging the foreign pending obligation.

### A0543

1. Read `judge_scalar_recast_alternative` q:1877-1918 (read).
2. No separate caller trace; branch is inside scalar alternative judgment after `judge_scalar_recast` (read q:1499-1509).
3. Input: both `source_primitive` and `target` from `exact_scalar_representation_type`; tuple destructure at q:1902-1906 returns silently if either `scalar_byte_size()` is `None` (read).
4. **Verdict: CLOSED.** `exact_scalar_representation_type` yields `PrimitiveType` (read q:4540-4567); `PrimitiveType::scalar_byte_size` always returns `Some` for every variant (`type_system.rs` 4729-4735, read). The `None` branch is unreachable for values that reach this site.

### A0544

1. Read `report_unspelled_reference_pun` q:2298-2337 (read); caller `validate_recasts` q:5338-5346 (read).
2. Search: `rg 'report_unspelled_reference_pun' omega-rust` → 3 hits, one production call at q:5339 (read).
3. Input: `let` with reference type and non-recast initializer; function requires both `declared_place_type` → `exact_scalar_representation_type` and unwrapped referee primitive (read q:2307-2320); otherwise silent return.
4. **Verdict: REACHABLE.** A `let` binding whose referee is a record (or other non-exact-scalar) type and whose initializer borrows a place with a different non-scalar shape should be rejected or require explicit recast syntax; this helper emits nothing and validation continues.

### What I did not read

- Full `program_validation.rs` statement validators and the `report_consumption` call sites at 1640, 2839, 3657.
- `calls.rs` `report_consumption` at 40216 and `literals_and_struct_literals.rs` at 4289.
- Full `reject_quotient_operation_requests` success-path fence logic beyond q:730-888.
- `form_scalar_recast` in full (only q:1656-1738 segment for pending-view interaction).
- Second strict `check_program` orchestration beyond `checking.rs` 938-985 and `preparation.rs` 1957-2050 headers.

SLICE COMPLETE
