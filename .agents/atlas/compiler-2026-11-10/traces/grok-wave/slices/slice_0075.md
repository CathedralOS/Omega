slice_0075

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0577 | value_custody_core.rs:3938-3963 | REACHABLE | Membership facts with bad/missing field linkage skip carrier check after `validate_proof_facts`. |
| A0578 | value_custody_core.rs:4402-4408 | REACHABLE | Arm destructure marker returns when declared type or data def missing; sibling `__destructure#` path errors. |
| A0579 | value_custody_core.rs:4442-4451 | REACHABLE | Unknown variant in arm pattern returns silently with no exhaustiveness/name diagnostic. |
| A0580 | value_custody_core.rs:5630-5638 | REACHABLE | Global scan accepts any data variant name as a known value root before scope binding. |
| A0581 | value_custody_core.rs:5591 | REACHABLE | Valid `Conformance`/`ConformanceParameter` symbol handles are known without scope or existence check. |
| A0582 | value_custody_core.rs:7303 | CLOSED | `expression_permission_provenance` has no production callers; only definition and re-export. |
| A0583 | value_custody_core.rs:7878-7905 | REACHABLE | Non-primitive domain carrier exits before `operand_mint_judgment` predicate mint checks. |
| A0584 | value_custody_core.rs:8454-8456 | REACHABLE | Any non-empty place path makes `reference_slot_is_writable` true for exclusive borrow. |

Citations use `omega-rust/psi/compilation/validation/src/value_custody_core.rs` (digest label `vcc`); line numbers read at current tree match the cited ranges.

### A0577

1. Read `validate_data_default_domain` in full at value_custody_core.rs:3924-3982 (read).
2. Entry loop: value_custody_core.rs:3938-3963 — `continue` on non-membership, missing `domain_definition`, non-`Name` value, missing field name, or field not in `members` (read).
3. Caller search: `rg validate_data_default_domain` under `omega-rust/` → 2 hits in value_custody_core.rs (definition + `validate_data_field_types` at 3792) (read).
4. Read `validate_data_field_types` call site value_custody_core.rs:3757-3792; caller `program_validation.rs:478` in validator list (read); `run` appends diagnostics (read program_validation.rs:450-587 header only).
5. Upstream: value_custody_core.rs:3931-3936 calls `validate_proof_facts` first (proof_contracts_core.rs:7087+, read 7087-7126). `validate_proof_facts` does not validate field-carrier agreement for data default-domain memberships (read).
6. `unknown_membership_domain_diagnostic` at proof_contracts_core.rs:6817-6821 returns `None` when `domain_symbol.is_valid()`, so missing `domain_definition` can pass `validate_proof_facts` and still hit `continue` at value_custody_core.rs:3942-3943 (read).
7. Input to accepting branch: data `where` membership facts where the named field is absent from `members`, or domain symbol valid but no definition, or value shape is not a simple `Name`.
8. Verdict **REACHABLE**: smallest bad program — a data definition with a default-domain membership fact naming a field that does not exist on that data type should be rejected for carrier mismatch but is accepted past this loop because of `continue` at 3961-3962 (read).
9. Unsure: whether another validator always rejects the same bad membership before `program_validation::run` completes; `validate_proof_facts` membership arm was not read past 7126.

### A0578

1. Read `validate_arm_pattern_marker` in full value_custody_core.rs:4368-4487 (read).
2. Entry: value_custody_core.rs:4402-4408 — `return` when `declared` or `data_definition_for_type` is `None` (read).
3. Caller search: `rg validate_arm_pattern_marker` → 2 hits (definition + `validate_destructure_exhaustiveness` at 4276) (read).
4. Read `validate_destructure_exhaustiveness` value_custody_core.rs:4265-4366: `__arm_destructure#` → `validate_arm_pattern_marker`; `__destructure#` → errors at 4304-4317 when type unresolved (read).
5. Caller `program_validation.rs:570` (read).
6. Input: synthetic local whose name carries `ARM_MARKER_PREFIX` encoding, with `initial_value` / `type_reference` that does not resolve to a data definition.
7. Verdict **REACHABLE**: arm-pattern exhaustiveness is skipped when the arm’s declared type is unresolvable, while the non-arm destructure path would error on the same unresolved type (read 4304-4307 vs 4402-4407).
8. Unsure: whether ill-formed arm markers can exist only when earlier stages already errored.

### A0579

1. Same function `validate_arm_pattern_marker` (read in full).
2. Entry value_custody_core.rs:4442-4451: variant arm with `find_map` miss → `return` with no diagnostic (read); contrast domain-pattern error at 4437-4440 (read).
3. Call chain as A0578 (read).
4. Input: arm marker spelling a variant name that does not exist on the resolved sum type, after domain-pattern check passes.
5. Verdict **REACHABLE**: a destructure arm naming a nonexistent case is accepted by this function with no error (read 4449-4450).
6. Unsure: downstream match/emit stages were not read for a duplicate exhaustiveness check.

### A0580

1. Read `state_value_root_is_known` in full value_custody_core.rs:5555-5641 (read).
2. Entry value_custody_core.rs:5630-5638: any variant name in any `data_definitions()` marks root known (read).
3. Search `rg state_value_root_is_known` under `omega-rust/` → 5 hits: value_custody_core.rs:5448, 5555, 5818 (test); calls.rs:809, 2165 (read grep count).
4. Read callers: `StateValueScope::expression` value_custody_core.rs:5447-5461 pushes “not in scope” only when `!state_value_root_is_known` (read); `receiver_is_declared` calls.rs:809-824; `report_unresolved_name_use` calls.rs:2165-2179 (read); did not read full upstream of those beyond diagnostic push.
5. Input: bare `Name` in machine state whose first segment equals some unrelated enum case string, with invalid/unknown `root` symbol but name matching global variant scan.
6. Verdict **REACHABLE**: a free identifier spelled like an unrelated enum case should be reported out-of-scope but is treated known via 5630-5638 (read), suppressing errors at 5458-5461 and 2175-2179.
7. Unsure: name resolution paths that always bind a different `SymbolKind::Unknown` before this check.

### A0581

1. Read same function; entry value_custody_core.rs:5585-5591 (read).
2. `root.is_valid()` and `SymbolKind::Conformance | ConformanceParameter` → `return true` with no scope query (read).
3. Callers as A0580 (read); `validate_conformances` in program_validation.rs:455 exists but was not traced for interaction with this predicate (inferred listed only).
4. Input: name path whose head symbol is classified Conformance/ConformanceParameter in the symbol table.
5. Verdict **REACHABLE**: conformance roots are accepted as in-scope value roots without verifying they belong to the current machine state’s bindings (read 5585-5591 vs later binding check 5639-5640 only as fallback).
6. Unsure: whether ill-typed conformance-as-value uses always fail later in `validate_conformances` or call typing.

### A0582

1. Read `expression_origin` in full value_custody_core.rs:7231-7304 and `expression_permission_provenance` value_custody_core.rs:7221-7229 (read).
2. Entry value_custody_core.rs:7303: `Ok(if different { None } else { common })` — mixed struct/array operand provenance yields `None`, same as constant path `Ok(None)` at 7260 (read).
3. Search `rg '\bexpression_permission_provenance\b' omega-rust/` → 2 hits: definition value_custody_core.rs:7221, re-export lib.rs:1618; no call sites (read).
4. Pipeline uses `expression_permission_provenance_for_claim` and `common_permission_provenance` at checks_multiplicity_linear_validation.rs:1742-1770 (read 1766-1770 only); not the vcc entrypoint — not traced for equivalence to 7303.
5. Input to vcc branch: aggregate literal whose fields disagree on `PermissionProvenance`.
6. Verdict **CLOSED**: no production path invokes `expression_permission_provenance`/`expression_origin` during validation (search above), so this accepting branch cannot affect program accept/reject in the current Rust producer (read).
7. Unsure: whether exported API is intended for non-`omega-rust` consumers.

### A0583

1. Read `judge_qualification_cast` in full value_custody_core.rs:7855-7950 and caller `qualification_casts::validate` 7832-7853 (read).
2. Entry value_custody_core.rs:7878-7880 early `return` if `domain_requires_provenance`; 7900-7905 `return` when `primitive_type_reference(domain.target_type)` is `None` (read); predicate/mint logic 7906-7927 not reached for non-primitive carriers (read).
3. Caller `program_validation.rs:4549` after arithmetic interval collection (read).
4. Input: checked `Cast` with semantic domain whose `target_type` is a non-primitive carrier, domain not vacuous, no provenance requirement.
5. Verdict **REACHABLE**: cast to a domain over a non-primitive carrier skips predicate mint judgment here and should be rejected when predicates fail but is not diagnosed at this site (read 7900-7904).
6. Unsure: `field_domains` or cast validation elsewhere; `operand_mint_judgment` not read in full.

### A0584

1. Read `reference_slot_is_writable` value_custody_core.rs:8443-8472 and enclosing `borrow_source_access_refusal` 8390-8441 (read).
2. Entry value_custody_core.rs:8454-8456: `!place.segments.is_empty()` ⇒ `return true` (read).
3. Search `rg reference_slot_is_writable omega-rust/` → 2 hits (definition + call at 8417) (read).
4. Read caller context `borrow_source_access_refusal` → used from `receiving_reference_access_refusal` 8206; `report_reference_access_conflict` 8274-8308 pushes diagnostic on refusal (read 8261-8308 header).
5. Input: exclusive borrow (`access.is_exclusive()`) where `place_storage_borrow_access` is `BorrowedStorage` and canonical place has field/index segments under a reference-slot root that is not mutable at root.
6. Verdict **REACHABLE**: any projected path off a reference slot is treated writable, so exclusive borrow from a projection when only the root slot’s mutability should matter may be accepted (read 8454-8455, 8414-8424).
7. Unsure: `place_storage_borrow_access` and `canonical_place_from_expression` were not read in full.

### What I did not read

- Full `validate_proof_facts` beyond proof_contracts_core.rs:7126.
- `place_storage_borrow_access` body (expression_types_and_fields.rs:4242+).
- `operand_mint_judgment` and predicate helpers for qualification casts beyond value_custody_core.rs:7959 header.
- `validate_conformances` body for cross-check with A0581.
- Entire `checks_multiplicity_linear_validation` claim/provenance consumer beyond 1766-1770.
- Call-site upstream of `StateValueScope` beyond program_validation.rs:1482 reference (not opened).

SLICE COMPLETE
