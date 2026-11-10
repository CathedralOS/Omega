slice_0040

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0301 | contract_and_obligation_facts.rs:2503 | UNDECIDED | `PartialEq` compares `canonical_bytes` (read); predicate build case-qualifies members and maps names to parameter indices (read), but `contract_entailment.rs` consumers not traced |
| A0302 | admissibility_and_borrow.rs:292 | CLOSED | `statement_index` default `0` for non-`StateStatement` operators (read) has zero call sites in `omega-rust` (grep) |
| A0303 | lib.rs:328 | CLOSED | Missing keyed lookup yields `&[]` (read); callers treat empty as none/not-unique, not acceptance |
| A0304 | providers_and_trust.rs:107 | CLOSED | `Ok(None)` when requirement not installation-bound (read); downstream skips reach rows without error (`binding.rs:212`, read) |
| A0305 | providers_and_trust.rs:161-163 | CLOSED | Non-trait requirement yields empty external apps (read); Import plans are trait-scoped at selection (`selected_declarations.rs:48-53`, read) |
| A0306 | termination_and_cleanup_facts.rs:99-120 | UNDECIDED | `path` not `shift_span`'d in `append_shifted` (read); cleanup consumers read `disposition.path` (`execution_terminal_unit_cleanup.rs:146`, read) but accept/reject impact not traced end-to-end |
| A0307 | facts.rs:266 | CLOSED | `_ => true` only after no non-`BuiltinFallback` operator row (`facts.rs:249-253`, read); contract paths pair with `builtin_meaning` |
| A0308 | facts.rs:2999-3004; facts.rs:3171-3177 | CLOSED | Tables sized to max type-ref index at check time (`validation/lib.rs:2072-2087`, read); `or_else(first)` only on lookup miss |

### A0301

1. Read `CrashPredicateIdentity::eq` at `omega-rust/psi/compilation/representations/checked-trees/src/contract_and_obligation_facts.rs:2501-2504` — equality is `Arc::ptr_eq` or equal `canonical_bytes` from `write_canonical`.
2. Read `write_canonical` for `Parameter` (index bytes, `:2401-2404`) and `Member` (member string + receiver subtree, `:2361-2366`).
3. Read crash predicate construction: name-to-parameter at `published_facts_crash_calls.rs:4981-4990`; `member_entry_name` / `field_entry_name` at `:5465-5493` (case-qualified field names).
4. Read guard dedup `CrashRouteBucket::new` at `contract_and_obligation_facts.rs:1405-1409` (`sort`/`dedup` on `alternative_guards`).
5. Read contract-fact matching at `published_facts.rs:8574-8581` (`from_expression(candidate) == *predicate`).
6. Search `rg CrashPredicateIdentity omega-rust` — 86 matches across 8 files (count mode); did not open `validation/contract_entailment.rs`.

**Input to accepting branch:** Two `CrashPredicateIdentity` values whose distinct runtime resolution still yields identical `canonical_bytes`.

**Verdict:** UNDECIDED — build disambiguates same-spelled fields (`:5491-5492`, read) and parameters by index (`:4988-4990`, read), but whether any path still merges semantically distinct guards or contract matches was not verified through entailment consumers.

### A0302

1. Read `StateOperationAcceptance::statement_index` at `admissibility_and_borrow.rs:283-294` — Operator arm uses `statement_index` from `CheckedValueOrigin::StateStatement` or `0`.
2. Read `StateAcceptance::operations` at `:586-611` (constructs operators but does not call `statement_index`).
3. Search `rg 'pub fn statement_index' omega-rust` — 1 hit (`admissibility_and_borrow.rs:283` only).
4. Search `rg '\.statement_index\(\)' omega-rust/psi/compilation` — hits are other types (`IndexedWriteLeaf`, flow facts); not `StateOperationAcceptance`.

**Verdict:** CLOSED — accepting branch (default `0`) is unreachable; no production caller of this method (grep, read definition only).

### A0303

1. Read `KeyedRows::with_rows` at `checked-trees/src/lib.rs:317-329` — `groups.get(key).map_or(&[], Vec::as_slice)`.
2. Read callers: `flow.rs:1299-1301` invalid call → empty vec first; `flow.rs:1347-1356` missing state → `None`; `admissibility_and_borrow.rs:2043-2050` missing key → `None` for non-unique.
3. Search `rg '\.with_rows\(' omega-rust/psi/compilation/representations/checked-trees` — 24 call sites (count); read 5, not all 24.

**Input:** Lookup key absent from `OwnedIndex`.

**Verdict:** CLOSED — empty slice is the intentional “no rows” answer; read callers treat it as non-existence, not program acceptance.

### A0304

1. Read `provider_reach` at `providers_and_trust.rs:93-151` — `requirement_reach` then `if !bounded { return Ok(None); }` at `:107-108`.
2. Read `requirement_reach` at `:333-352` — `bounded` is `service_reach_is_installation_bound`.
3. Read `provider_declarations` assembly at `:51-55` — reach only when `request.installation_reach`.
4. Read consumer `omega/.../provider-selection/src/binding.rs:202-214` — `Ok(None) => {}`.

**Input:** Installation reach requested for a requirement not flagged installation-bound.

**Verdict:** CLOSED — omission of reach declaration is intentional; errors only on `Err(reason)` (`binding.rs:213-214`, read).

### A0305

1. Read `provider_external_applications` at `providers_and_trust.rs:153-163` — `trait_machine_signature` `None` → `Ok(Vec::new())`.
2. Read caller at `providers_and_trust.rs:76-79` — result stored in `ProviderApplicationDeclarations.applications` without extra check.
3. Read `binding.rs:292-310` — empty `applications` iter adds nothing; `Err` still errors.
4. Read row shaping `selected_declarations.rs:48-53` — `External` only for `ProviderBinding::Import`.

**Input:** External application view on a row whose requirement is not a trait machine.

**Verdict:** CLOSED — silent empty set is “no import template identities to publish”; Import rows come from plan binding (read), not unchecked arbitrary rows.

### A0306

1. Read `append_shifted` at `termination_and_cleanup_facts.rs:47-122` — `path` copied unchanged at `:115` while `exclusions`/`definite_exclusions` use `shift_span` (`:116-117`); `segments` appended via `append_rows` (`:167`).
2. Read caller `checks_multiplicity_permission_events.rs:594-601` — failed merge aborts walk (`return None`).
3. Read consumer `execution_terminal_unit_cleanup.rs:140-147` — `disposition.path` resolved through `segments.span`.

**Input:** Merged walk state cleanup dispositions with non-empty `path` handles from appended arena.

**Verdict:** UNDECIDED — path-not-shifted on merge is read at `:115`, but whether corrupted spans let an unsound program complete checking was not traced through cleanup publication.

### A0307

1. Read `has_builtin_operators` at `facts.rs:244-267` — early `false` if any `operators.uses` row for expression has `status != BuiltinFallback` (`:249-253`); `_ => true` only on non-Binary/Unary/Borrow leaves after recursion.
2. Read `closed_boolean_value` gate at `:233-234`.
3. Sample callers read: `checks_contracts_prover.rs:2195-2199`, `checks_contracts.rs:4197-4204`, `checks_contracts_exits.rs:2216-2218` — paired with `builtin_meaning` / `has_builtin_spelled_expression_meaning` / `comparison_is_admitted`.
4. Search `rg has_builtin_operators omega-rust/psi/compilation` — 42 matches in 5 files (count); full caller chain not read for all 42.

**Input:** Expression tree whose binary nodes lack operator-use rows but leaves hit `_ => true`.

**Verdict:** CLOSED — recorded non-builtin resolution blocks at `:249-253` (read); every sampled acceptance path adds further gates beyond `_ => true`.

### A0308

1. Read `ContentClassFacts::classes` at `facts.rs:2999-3004` and `ReferenceCustodyFacts::custody` at `:3171-3177` — `get(index).or_else(|| first())`.
2. Read builder `classify_type_references` at `validation/src/lib.rs:2072-2087` — dense `rows` sized to max `arena_index`, filled per table row.
3. Read check pipeline `checking.rs:1263-1264` — tables assigned from validation at check fill.
4. Read lowering consumers `06.../shared.rs:356-360`, `:1469-1473` (single call sites each, read).

**Input:** `TypeReferenceHandle` whose index is outside `rows.len()` after check.

**Verdict:** CLOSED — successful check builds per-index rows (`lib.rs:2072-2087`, read); `or_else(first)` is a defensive miss path, not the normal checked handle path.

### What I did not read

- `omega-rust/psi/compilation/validation/src/contract_entailment.rs` (cited in digest for A0301).
- All 24 `KeyedRows::with_rows` call sites beyond the samples listed.
- All 42 `has_builtin_operators` call sites beyond samples.
- Remaining `CrashPredicateIdentity` consumer logic in `checks.rs` and `proofs.rs`.
- Full provider-plan formation proving Import rows are always trait-backed.
- End-to-end outcome of corrupted `path` spans after `append_shifted` through terminal lowering.

SLICE COMPLETE
