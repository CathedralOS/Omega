slice_0072

| entry id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0553 | scc:2953-2958 | CLOSED | Early `Ok(())` only skips non-closed nodes during inventory; field walks and program validation reject unresolved data uses elsewhere. |
| A0554 | tr:1517-1531 | REACHABLE | Specialized machine can pass authored lifetime checks yet silently skip signature/evidence/law when scope construction fails. |
| A0555 | tr:1718-1724 | REACHABLE | Non-proposition named evidence lanes `continue` without diagnostics, so mismatched facts are never compared. |
| A0556 | tr:3681-3697,7907-7909 | REACHABLE | Shape/arity mismatches return without error on some implementation-validation paths that lack upfront length guards. |
| A0557 | tr:2912,2933-2941 | REACHABLE | `requirement_lifetime_matches` returns true when required lifetime is absent or not in the target telescope. |
| A0558 | tr:7397-7399,7490-7505 | REACHABLE | Suspends removal is diagnosed only when keyword source spans are non-empty. |
| A0559 | tr:362-363,5366-5368 | REACHABLE | State lookup by name falls back to the first machine state when the leaf name misses. |
| A0560 | trw:5633 | REACHABLE | Missing codec defaults to `compact_binary`, which satisfies canonicality demands without a declared encoding. |

### A0553

1. Read `structural_qualification_inventory` and inner `collect` in full (`scc:2748-3055`, read).
2. Search `structural_result_projected_qualifications` in `omega-rust/`: 6 hits (def + reexport + 4 call sites, read).
3. At `scc:2952-2955` (read), `Named`/`Generic` without `closed_data_definition` returns `Ok(())` and contributes no projected qualifications; `scc:3017` (read) same for other node kinds; `scc:2979-2986` (read) skips erased/primitive/byte fields by design.
4. Callers: `execution_terminal_unit_control.rs:6680`, `attached_unit.rs:3984/14631`, `emission.rs:11575` (read signatures only); inventory also feeds `structural_storage_path` (`scc:3276-3278`, read), which errors without an exact data owner.
5. Upstream: `program_validation.rs:2628` (read) runs `validate_type_reference_handle` in the validation prologue before machine checking.

**Input to accepting branch:** type references that are not closed data records (parameters, references, non-data symbols, or non-data shapes), not diagnosable “unresolved data” at inventory time.

**Verdict:** CLOSED — `program_validation.rs:2628` and `scc:3276-3278` reject programs that need resolved data for structural custody; `scc:2953-2955` read only omits traversal where there is no closed data tree to walk.

**Unsure:** Full failure modes for every malformed generic that makes `closed_data_definition` return `None` without an earlier diagnostic (did not read all `validate_type_reference_handle` arms).

### A0554

1. Read `validate_machine_single_requirement` in full (`tr:1437-1561`, read).
2. Search `validate_machine_single_requirement` in `omega-rust/`: 2 hits (definition + call, read).
3. At `tr:1508-1530` (read), failed lifetime ordinal mapping (`lifetimes` `None` at 1517-1518) or `AppliedTraitScope::new` failure (1520-1530) returns with no diagnostic.
4. Sole caller: `validate_machine_trait_conformances` at `tr:848` (read), from `program_validation.rs:4217` (read).
5. Lifetime ordinals are checked against `authored_machine` at `tr:792-801` (read), but scope build at `tr:1508-1513` uses `machine.lifetime_parameters`, not `authored_machine`.

**Input to accepting branch:** machine with trait conformance whose lifetime ordinals index the authored template but the checked `machine` has an empty or shorter lifetime parameter list.

**Verdict:** REACHABLE — a specialized instance can pass `tr:792-801` on the authored owner yet hit `tr:1518` or `tr:1530`, skipping `validate_applied_trait_signature`, named evidence, and law checks.

**Unsure:** Whether typed trees always pair conformances on instances with matching lifetime telescopes (did not read specialization lowering).

### A0555

1. Read `validate_named_contract_lane` in full (`tr:1651-1764`, read).
2. Caller `validate_concrete_named_evidence_contract_conformance` (`tr:1564-1637`, read); search: 2 hits in `traits.rs` (read).
3. At `tr:1718-1723` (read), unless both fact spans are exactly one `ProofFact::Proposition`, the loop `continue`s with no diagnostic.
4. Upstream: only from `validate_machine_single_requirement` at `tr:1542-1550` (read) after generic checks at `tr:1597-1612` (read).

**Input to accepting branch:** named requires/ensures rows whose facts are not a single proposition each (expression/membership-style facts).

**Verdict:** REACHABLE — requirement and satisfier can differ on those lanes with no equality check and no error from this function.

**Unsure:** Whether any later validator compares those fact shapes (did not trace beyond `validate_machine_single_requirement`).

### A0556

1. Read `validate_implementation_type_arguments` (`tr:3669-3724`, read) and `validate_implementation_application` (`tr:7895-7962`, read).
2. Search `validate_implementation_type_arguments`: 2 hits; `validate_implementation_application(`: 5 hits (read).
3. Silent returns: non-`Generic` argument (`tr:3686-3687`), missing conformance (`tr:3689-3690`), inner arity mismatch (`tr:3696-3697`); application arity mismatch (`tr:7907-7908`) (all read).
4. Call paths: `validate_trait_application_obligations` (`tr:3336-3344`, read) guards outer trait argument count at `tr:3262-3268` but not inner generic arity; `validate_trait_implementation_arguments` (`tr:7749-7869`, read) calls `validate` without length guard before `validate_trait_implementation_application`.
5. Some `trw` call sites push arity diagnostics but still call `validate_implementation_application` (`trw:5251-5258`, read).

**Input to accepting branch:** implementation/type generic applications with wrong inner arity or wrong shape on paths that do not return before the helper.

**Verdict:** REACHABLE — e.g. trait parent/implementation relationship with wrong argument count in `validate_trait_implementation_arguments` can pass `tr:7907-7908` without adding a diagnostic (inferred from `tr:7775-7783` + `tr:7907-7908`).

**Unsure:** Whether stage-04 preparation always emits correct relationship arities (not read).

### A0557

1. Read `type_reference_lifetimes_match_requirement_application` and `requirement_lifetime_matches` in full (`tr:2892-2952`, read).
2. Search `type_reference_lifetimes_match_requirement_application`: 4 hits in `traits.rs` (read).
3. Accepting logic: provider scope passes when `required` is `None` (`tr:2911-2912`, read); requirement scope returns `true` when `required` is `None` or not found in `target_lifetimes` (`tr:2933-2940`, read).
4. Only used from `validate_machine_top_level_requirement_conformance` (`tr:1068-1094`, read) when lifetime counts match (`tr:1060`, read).
5. Ordinary trait satisfaction uses `validate_applied_trait_signature` and borrow refinement without this helper (read `tr:2183+`, did not read all lifetime arms).

**Input to accepting branch:** top-level boundary check with matched lifetime parameter counts but reference types carrying lifetime names outside the requirement’s `lifetime_parameters` list, or unconstrained required regions.

**Verdict:** REACHABLE — provider can match types while required region names are unconstrained in the requirement telescope and checks still pass.

**Unsure:** Whether top-level requirements always list every region name appearing in parameter/result types (spec not read).

### A0558

1. Read `refinement_fit_diagnostics` (`tr:7381-7525`, read); empty `covering_clauses` at `tr:7397-7398` (read) skips the row.
2. Search `refinement_fit_diagnostics`: 4 hits (read).
3. Suspends mismatch only when `!clause.signature.suspends_keyword_source_spans.is_empty()` (`tr:7490-7497`, read).
4. Callers: `tr:8079` (read), `machine_parameters_and_bindings.rs:8152` (read); failures become errors via first diagnostic or `finish_strict_diagnostics`.

**Input to accepting branch:** refinement clause with `suspends == false`, empty `suspends_keyword_source_spans`, and a suspending selected realization.

**Verdict:** REACHABLE — tightening `suspends` on the clause without recorded keyword spans does not trigger the rejection at `tr:7494-7496`.

**Unsure:** Whether typed-tree construction always fills spans when `suspends` changes (did not read lowering).

### A0559

1. Read `producer_entry_state` (`tr:349-364`, read) and `family_realization_state` (`tr:5356-5368`, read).
2. Search `producer_entry_state`: 4 hits (2 validation + 2 omega realization, read); `family_realization_state`: 2 hits (read).
3. Both use `.find(name match).or_else(|| .first())` (`tr:362-363`, `tr:5366-5367`, read).
4. `producer_entry_state` used in `validate_external_via_expression` (`tr:333-335`, read) from `program_validation.rs:4242` (read).
5. `family_realization_state`: `None` gets a diagnostic (`tr:5275-5279`, read); wrong fallback state does not.

**Input to accepting branch:** family/template state name not present on the instance machine; first state is substituted.

**Verdict:** REACHABLE — external `via` family rung or dynamic family row can treat the wrong entry state as the producer/realization entry when the leaf name is missing but the first state satisfies downstream symbol checks.

**Unsure:** None.

### A0560

1. Read `compatibility_demand_verdict` (`trw:5613-5751`, read), especially codec default `trw:5630-5634` and canonicality `trw:5715-5725` (read).
2. Search `compatibility_demand_verdict`: 3 hits (read).
3. When both schemas resolve, `canonical_value = compact_binary && local_schema.is_some() && peer_schema.is_some()` (`trw:5715-5716`, read); missing encoding forces `compact_binary` via `unwrap_or` (`trw:5633`, read).
4. Caller `check_wire_demands` → `validate_wire_protocol` (`trw:6322-6324`, `trw:6438-6469`, read); unsatisfied demands become errors.

**Input to accepting branch:** wire demand with `require_canonical` and resolved schema rows but no `encoding` on either schema entry.

**Verdict:** REACHABLE — canonicality is marked satisfied solely because of the default codec name, not because a codec was declared or verified.

**Unsure:** Whether schema validation always requires explicit encoding for schemas used in canonicality demands (did not read `validate_wire_schemas` fully).

### What I did not read

- Full `validate_type_reference_handle` and all stage-04 type formation paths for A0553/A0556.
- Complete `validate_applied_trait_signature` borrow/lifetime paths for ordinary (non-top-level) trait conformance (A0557).
- Typed-tree builders for `suspends_keyword_source_spans` and specialization lifetime layout (A0554, A0558).
- Entire `omega-rust/omega/` realization paths beyond cited `producer_entry_state` uses.

SLICE COMPLETE
