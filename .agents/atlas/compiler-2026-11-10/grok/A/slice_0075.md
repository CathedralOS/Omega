# slice_0075 (area_14, rows A0577–A0584)

Preamble read: `.agents/atlas/compiler-2026-11-10/digests/area_14.md` lines 1–27 (`vcc` → `omega-rust/psi/compilation/validation/src/value_custody_core.rs`).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0577 | omega-rust/psi/compilation/validation/src/value_custody_core.rs:3938-3963 | REACHABLE | Only `ProofFact::Membership` facts get default-domain field-vs-domain carrier matching; other `where` facts skip the loop after `validate_proof_facts`. |
| A0578 | omega-rust/psi/compilation/validation/src/value_custody_core.rs:4402-4408 | REACHABLE | `__arm_destructure#V=` markers return silently when the arm local’s type is unresolvable; the non-arm destructure path errors at 4303-4308. |
| A0579 | omega-rust/psi/compilation/validation/src/value_custody_core.rs:4442-4451 | REACHABLE | Case arm names a missing variant and returns without a diagnostic, so exhaustiveness is not checked for that arm. |
| A0580 | omega-rust/psi/compilation/validation/src/value_custody_core.rs:5630-5638 | REACHABLE | Any spelling equal to any data variant name anywhere makes a value root “known”, so unrelated case names are not reported out of scope. |
| A0581 | omega-rust/psi/compilation/validation/src/value_custody_core.rs:5591 | REACHABLE | Any valid `Conformance` / `ConformanceParameter` symbol is a known value root with no scope or declaration check. |
| A0582 | omega-rust/psi/compilation/validation/src/value_custody_core.rs:7303 | CLOSED | `expression_permission_provenance` is never called on the validation acceptance path (only definition + re-export in `omega-rust`). |
| A0583 | omega-rust/psi/compilation/validation/src/value_custody_core.rs:7878-7905 | REACHABLE | `judge_qualification_cast` returns before predicate minting when `domain.target_type` is not a primitive type reference. |
| A0584 | omega-rust/psi/compilation/validation/src/value_custody_core.rs:8454-8456 | REACHABLE | Any non-empty projected path on a reference borrow source is treated as writable for exclusive access. |

## A0577

**Verdict:** REACHABLE

**Cited site (read):** `validate_data_default_domain` at `value_custody_core.rs:3924-3982` runs `validate_proof_facts` on all facts (`3931-3936`), then loops facts (`3938+`) with `let Some(membership) = fact.as_membership() else { continue; }` (`3939-3940`). Carrier mismatch diagnostics only run for membership facts that resolve domain, name path, and field (`3942-3980`).

**Caller search:** `\bvalidate_data_default_domain\b` in `omega-rust` — count 2 (definition + call). Production call: `value_custody_core.rs:3792` inside `validate_data_field_types` (`3757+`).

**Upward (read):** `program_validation.rs:478` runs `validate_data_field_types` in the prologue validator list (`301-587`). `program_validation.rs:4067+` `validate` rejects when diagnostics contain errors.

**Input in plain words:** A data definition’s default-domain `where` block carries a fact that is not in membership form (for example a boolean expression fact) but should still tie a named field to a domain whose carrier disagrees with the field’s carrier; `validate_proof_facts` (`proof_contracts_core.rs:7087-7126`, read) does not compare field carriers to domain carriers for those shapes.

**Why not CLOSED:** No second validator in the read path re-runs the carrier alignment for non-membership default-domain facts; `default_domains` helpers elsewhere target write/interval obligations, not this declaration-time carrier pairing.

## A0578

**Verdict:** REACHABLE (arm-marker path only)

**Cited site (read):** `validate_arm_pattern_marker` (`4368+`) resolves `declared` type (`4390-4401`); `let Some(declared) = declared else { return; }` at `4402-4404` exits with no diagnostic.

**Caller search:** `\bvalidate_arm_pattern_marker\b` — count 2 (definition + call at `4276`).

**Upward (read):** `validate_destructure_exhaustiveness` (`4265+`) is registered at `program_validation.rs:570`. For `__destructure#` locals (non-arm), unresolved type pushes an error at `4303-4308` (read) — digest’s “non-arm path errors” matches.

**Input in plain words:** A machine state holds a compiler-generated `__arm_destructure#V=` local whose declared place type cannot be resolved; arm-pattern exhaustiveness is skipped entirely while the program continues validation.

## A0579

**Verdict:** REACHABLE

**Cited site (read):** In `validate_arm_pattern_marker`, when `variant` is non-empty, lookup `case` by name (`4442-4448`); `else { return; }` at `4449-4450` with no diagnostic.

**Caller search:** same as A0578 (`4276`).

**Upward (read):** Exhaustiveness field checks at `4469-4477` only run after a resolved `case`; missing variant exits before that loop.

**Input in plain words:** A sum-type arm pattern marker spells a variant name that does not exist on the data definition; validation accepts the machine without reporting the unknown case.

## A0580

**Verdict:** REACHABLE

**Cited site (read):** `state_value_root_is_known` (`5555-5641`). After locals/constants/scope checks, loop all `data_definitions` and variants (`5630-5637`): if `variant.name.as_str() == name`, `return true`.

**Caller search:** `\bstate_value_root_is_known\b` in `omega-rust` — count 5. Production: `value_custody_core.rs:5448` (`StateValueScope::expression`), `calls.rs:809` (`receiver_is_declared`), `calls.rs:2165` (`report_unresolved_name_use`). Test-only: `value_custody_core.rs:5818` (`#[test]`).

**Upward (read):** `StateValueScope` used from `program_validation.rs:1482`, `declarations_core.rs:1816-1873`, `calls.rs:2274/2745`. Out-of-scope diagnostics at `5458-5461` and `calls.rs:2175-2179` are suppressed when `state_value_root_is_known` is true.

**Input in plain words:** In a machine state, an expression uses a bare identifier spelled exactly like a variant of some unrelated data type defined elsewhere in the package; the name is treated as in scope even though it is not a local, parameter, or type in that state.

## A0581

**Verdict:** REACHABLE

**Cited site (read):** `state_value_root_is_known` `5585-5591`: if `root.is_valid()` and `kind` is `SymbolKind::Conformance | SymbolKind::ConformanceParameter`, `return true` with no lookup of whether that conformance is meaningful in the current state.

**Caller search:** same as A0580.

**Upward (read):** Same out-of-scope suppression paths as A0580.

**Input in plain words:** A state expression uses a conformance or conformance-parameter symbol as a bare value root; it is accepted as “known” without verifying it is declared among that state’s parameters or prior locals.

## A0582

**Verdict:** CLOSED

**Cited site (read):** `expression_origin` in `permission_provenance` (`7231-7304`): after aggregating struct/array fields, `Ok(if different { None } else { common })` at `7303`; bare constants also `Ok(None)` at `7260`.

**Caller search:** `\bexpression_permission_provenance\b` in `omega-rust` — count 2 (`value_custody_core.rs:7221` definition, `lib.rs:1618` re-export). No production call sites in validation or elsewhere in `omega-rust` (read).

**Upward (read):** `program_validation` prologue and `validate` (`4067+`) never invoke this module. Per-state linear permission scanning in pipeline `05_typed-trees-to-checked-trees` uses `common_permission_provenance` (`checks_multiplicity_linear_validation.rs:1766-1770`, not read in full) — separate from this symbol.

**Why CLOSED for acceptance:** Every path through validation-time program acceptance (`validate` returning `Ok`) does not execute `vcc:7303`; the branch cannot change whether validation accepts a program.

## A0583

**Verdict:** REACHABLE

**Cited site (read):** `judge_qualification_cast` (`7855+`). For a resolved domain (`7877+`), provenance domains return early (`7878-7880`). After vacuity checks, `if program.primitive_type_reference(domain.target_type).is_none() { return; }` at `7900-7904` skips `operand_mint_judgment` and predicate diagnostics (`7906-7927`).

**Caller search:** `judge_qualification_cast` is private; caller `qualification_casts::validate` (`7832+`) at `7844-7850`.

**Upward (read):** `program_validation.rs:4549` calls `qualification_casts::validate` during per-machine validation after arithmetic intervals are collected (same function as `validate` at `4067+`).

**Digest note:** digest also cites `7900-7905`; lines match at current checkout (not relocated).

**Input in plain words:** A referenced `as ... in Domain` cast targets a semantic domain whose underlying `target_type` is not a primitive type reference; qualification-cast validation returns without proving or rejecting domain predicates for that cast.

## A0584

**Verdict:** REACHABLE

**Cited site (read):** `reference_slot_is_writable` (`8443-8472`): `if !place.segments.is_empty() { return true; }` at `8454-8456` before mutability checks on the root symbol (`8460-8471`).

**Caller search:** `\breference_slot_is_writable\b` — count 2 (definition + `8417`).

**Upward (read):** Called from `borrow_source_access_refusal` (`8390+`) when `lends_storage` and exclusive access (`8414-8424`). `borrow_source_access_refusal` is used from `receiving_reference_access_refusal` (`8206+`). `receiving_reference_access_refusal` — count 5 in `omega-rust` (3 in `value_custody_core.rs`, 2 in `calls.rs`).

**Input in plain words:** A completed borrow’s source is a reference slot with a field (or other) projection (`canonical_place` segments non-empty) while the underlying root is not a mutable local/parameter/owned slot; exclusive storage borrow is permitted because projected paths are always writable here.

## What I did not read

- Full bodies of `validate_proof_facts` beyond the opening match arms (`proof_contracts_core.rs:7087+` partial).
- Entire `default_domains::validate_default_domain_writes` and other default-domain runtime validators.
- Pipeline `checks_multiplicity_linear_validation.rs` beyond the `common_permission_provenance` header (not on validation `validate` path for A0582).
- `operand_mint_judgment`, `predicates_hold_on_private_arrivals`, and downstream cast checks outside `7878-7928`.
- Test modules other than identifying `state_value_root_is_known` test at `5763+`.

SLICE COMPLETE
