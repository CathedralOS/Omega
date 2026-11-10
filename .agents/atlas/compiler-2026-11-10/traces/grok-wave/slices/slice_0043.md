slice_0043

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0325 | representation_selection.rs:136-142 | CLOSED | Primitive carriers are leaf inert graphs; `validate_inert_carrier` is the only production entry (rs:370). |
| A0326 | access_views_and_borrows.rs:27-189 | CLOSED | `validate_conformances` (traits.rs:6493-6498) runs before `validate_agreements` (program_validation.rs:455-482). |
| A0327 | access_views_and_borrows.rs:1294-1305 | REACHABLE | If `declared_place_type` fails, accessor discipline is skipped with no fallback diagnostic in this path. |
| A0328 | access_views_and_borrows.rs:1444-1463 | REACHABLE | When lookup returns `None`, `validate_statement` never runs binding-private checks (ab:1183-1188). |
| A0329 | access_views_and_borrows.rs:2993-2996 | REACHABLE | Out-of-range literal bounds return `true` with no error, blocking later write-only assignment diagnostics. |
| A0330 | access_views_and_borrows.rs:3365-3418 | CLOSED | Unresolved statement call targets are reported later by `validate_call_node` (calls.rs:3470-3479). |
| A0331 | arithmetic_domains.rs:7791-7793 | REACHABLE | Missing `declared_type` yields `NEUTRAL` operands so representability/division guards at ad:5717-5718 do not run. |
| A0332 | arithmetic_domains.rs:838-864 | REACHABLE | Range errors collected in `throwaway` are never appended to the caller's `diagnostics` buffer. |

### A0325

1. Read `CarrierClosure::validate_type` in full at `representation_selection.rs:129-227` (read); entry matches rs:136-141 early `Ok` when `primitive_type_reference` is `Some` (read).
2. Search `\bvalidate_inert_carrier\b` in `omega-rust/`: **2 hits** (definition rs:32, call rs:370). Search `validate_type(` under `representation-planning/`: **5 hits**, all internal to `representation_selection.rs` (read).
3. Caller chain: `form_opaque_representation_request` (rs:251-384, read through rs:370) calls `validate_inert_carrier`; upstream `collect_opaque_representation_selections` (rs:386+) (read header only).
4. Input at accepting branch: a `TypeReferenceHandle` for a primitive scalar used as an opaque representation carrier during `Build::select_representation` closure.
5. **Verdict: CLOSED.** `representation-planning/README.md` D44 (read) treats an inert carrier as a complete closed graph; a primitive has no fields to violate inertness. Earlier gates at rs:337-368 (read) reject malformed conformances before the primitive shortcut runs.
6. Unsure: whether any primitive type is specification-forbidden as a carrier (no spec read beyond README).

### A0326

1. Read `access_custody::validate_agreements` in full at `access_views_and_borrows.rs:27-189` (read); six `continue` paths at ab:35, 38, 45, 52, 57, 63, 69 (read).
2. Search `\bvalidate_agreements\b` in `omega-rust/`: **2 hits** (definition, call from `validate_plans` ab:1170).
3. Upward: `validate_plans` (ab:1168-1171) ← prologue `program_validation.rs:481-482` (read); `validate_conformances` at `program_validation.rs:453-461` (read) runs **before** line 482 in the same prologue list.
4. Input at continues: `AccessCustody` conformances with wrong arity, non-named arguments, missing plans, or missing data symbols—agreement validation is skipped for that iteration.
5. **Verdict: CLOSED.** Wrong argument count is already diagnosed at `traits.rs:6493-6498` (read) with `return`; unknown trait at `traits.rs:6462-6467` (read). Compilation retains those errors even if `validate_agreements` `continue`s.
6. Unsure: conformances with structurally valid but non-concrete argument nodes (ab:47-52); did not read all of `validate_trait_application_obligations` (traits.rs:6506+).

### A0327

1. Read `validate_expression` at `access_views_and_borrows.rs:1255-1394` (read) and `access_view_field_for_place` at ab:1406-1422 (read); entry matches ab:1294-1305 guard and ab:1415-1420 `?`.
2. Search callers of `validate_expression` in `access_views_and_borrows.rs`: used from `validate_statement` (ab:1189), assignment path, and internal recursion (inferred from grep).
3. Upward: `access_views::validate_statement` ← `program_validation.rs:3760` (read).
4. Input: atomic or member expression on a place where `declared_place_type` returns `None`, so `access_view_field_for_place` is `None` and atomic-accessor rules at ab:1298-1303 are skipped.
5. **Verdict: REACHABLE.** A machine state can use an atomic accessor field shape on a place the compiler cannot type; binding-private and “accessor not ordinary value” checks never run; only the generic atomic-load fallback at ab:1322-1332 may apply (read).
6. Unsure: whether `declared_place_type` failure is unreachable for well-typed programs from earlier stages.

### A0328

1. Read `access_view_field_for_statement_call` at ab:1444-1462 (read) and `validate_statement` call guard at ab:1183-1188 (read).
2. Search `access_view_field_for_statement_call`: **4 hits**, all in `access_views_and_borrows.rs` (read).
3. Upward: same `validate_statement` / `program_validation.rs:3760` path as A0327.
4. Input: statement `TableCall` whose `target_symbol` matches no accessor state in plans, and whose receiver path does not yield `declared_member_path_type` (ab:1455-1460 `?`).
5. **Verdict: REACHABLE.** Binding-private use checks in `validate_binding_private_use` (ab:1484-1502, read) only run when lookup succeeds; unresolved receiver + non-matching target silently skips the check.
6. Unsure: whether call-target resolution elsewhere always diagnoses the same call before acceptance.

### A0329

1. Read `validate_write_only_fixed_array_range_assignment` at ab:2921-2997 (read) and assignment dispatch at ab:2754-2797 (read).
2. Search `validate_write_only_fixed_array_range_assignment`: **2 hits** (definition, call ab:2762).
3. Upward: `write_only_borrows::validate_statement` (ab:2727+) called from `validate_checked_write_only_slice` (ab:2203, read) ← prologue `program_validation.rs:416` (read).
4. Input: write-only fixed-array range assignment with **statically known** `usize` bounds where `start > end` or `end > collection_len` (ab:2993-2994 read).
5. **Verdict: REACHABLE.** The function returns `true` without pushing a diagnostic, which satisfies the `else if` at ab:2762 and prevents `diagnose_unsupported_write_only_assignment_target` at ab:2790 (read); `validate_received_value` at ab:2818 does not bound-check the range index (read ab:3196-3268).
6. Unsure: whether corpus or another validator rejects the same pattern; did not trace `proof_contracts_core` indexed bounds.

### A0330

1. Read `validate_opaque_call_inputs` at ab:3352-3436 (read); early `return` at ab:3367, 3377, 3407, 3417 (read).
2. Search `validate_opaque_call_inputs`: **3 hits** (definition, two call sites ab:2829, 3446).
3. Upward: `write_only_borrows::validate_statement` (ab:2828) and prologue slice; separately `call_statements::validate` → `validate_call_node` at `program_validation.rs:2319` (read).
4. Input: statement call to unresolved `target_symbol` with a `&write` parameter that would need opaque boundary checking.
5. **Verdict: CLOSED.** `validate_call_node` reports unresolved targets at `calls.rs:3470-3479` (read) on every statement call; `validate_opaque_call_inputs` omitting a duplicate check does not let an unresolved call compile cleanly.
6. Unsure: none for acceptance; write-only-specific wording of unresolved-call diagnostic not read.

### A0331

1. Read `place::analyze` at `arithmetic_domains.rs:7783-7805` (read), `binary::analyze` at ad:5599-5807 (read), `result_primitive` at ad:5897-5937 (read), `report_mismatched_integer_operands` at ad:18689-18733 (read).
2. Search not needed beyond opened sites; entry cross-refs ad:5658, 5717-5718 (read).
3. Upward: `validate_value_range_with_decision_sink` → `analyze` at ad:15141-15153 (read) used throughout arithmetic validation.
4. Input: builtin integer binary operation on operand place(s) with no `declared_type` (`place::analyze` returns `NEUTRAL` at ad:7791-7792), no `target_primitive`, and unresolved operand primitives so `result_primitive` is `None`.
5. **Verdict: REACHABLE.** `if let Some(bridge) = policy_bridge && let Some(primitive) = primitive` at ad:5717-5718 (read) is skipped; `division_formed` at ad:5658 requires `primitive.is_some_and` (read); `report_mismatched_integer_operands` bails at ad:18731-18732 when either operand type does not resolve.
6. Unsure: contexts where `target_primitive` or literal intervals still supply a carrier despite missing place type.

### A0332

1. Read `check_value_narrowing` at ad:806-865 (read) and `validate_value_range` wrapper at ad:15011-15034 (read).
2. Search `\bcheck_value_narrowing\b` in `omega-rust/`: **4 hits**—definition, recursive match arm ad:819, `program_validation.rs:1766`, `calls.rs:531` (read call sites' headers only).
3. Upward: narrowing on atomic assignment value (`program_validation.rs:1762-1780`, read); call argument narrowing (`calls.rs:531-541`, read).
4. Input: narrowing check where `validate_value_range` pushes one or more errors into the local `throwaway` vec (ad:838-848, read).
5. **Verdict: REACHABLE.** When `throwaway` is non-empty, ad:850-864 (read) skips `check_validated_value_narrowing` and never appends `throwaway` to the caller's `diagnostics`; range failures discovered during narrowing are dropped.
6. Unsure: whether `validate_value_range` always emits errors the caller would have caught on a second pass with the real buffer.

### What I did not read

- Full `validate_trait_application_obligations` and closed-row validation for every `AccessCustody` continue case (traits.rs:6506+).
- Full `validate_call_node` and `ScanContext` call-resolution graph (calls.rs beyond excerpts).
- Typed-tree pipeline stages before validation for conformances or place typing.
- `Operation::reject_provably_zero_divisor` and full operand `analyze` dispatch for every expression kind.
- Other validators for write-only fixed-array range stores outside `access_views_and_borrows.rs`.

SLICE COMPLETE
