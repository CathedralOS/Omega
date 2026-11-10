# slice_0072 (A0553–A0560)

Digest: `area_14` (`.agents/atlas/compiler-2026-11-10/digests/area_14.md`, commit baseline `457adf4a11f`). Path shorthand: `scc` = `omega-rust/psi/compilation/validation/src/structural_call_custody.rs`, `tr` = `omega-rust/psi/compilation/validation/src/traits.rs`, `trw` = `omega-rust/psi/compilation/validation/src/type_references_and_wire.rs`.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0553 | scc:2953-2958 | REACHABLE | `collect` returns `Ok(())` when `closed_data_definition` is missing for Named/Generic, omitting qualifications without error |
| A0554 | tr:1517-1531 | REACHABLE | `validate_machine_single_requirement` can `return` with no diagnostic when lifetime collection or `AppliedTraitScope::new` fails |
| A0555 | tr:1718-1724 | REACHABLE | Named contract lane `continue`s when facts are not exactly one `ProofFact::Proposition` each, skipping equality |
| A0556 | tr:3681-3697, tr:7907-7909 | REACHABLE | Implementation-type and implementation-application validators silently `return` on shape or arity mismatch |
| A0557 | tr:2912, tr:2933-2941 | REACHABLE | `requirement_lifetime_matches` treats missing or unmapped required lifetimes as success |
| A0558 | tr:7397-7399, tr:7490-7505 | REACHABLE | `suspends` removal is diagnosed only when `suspends_keyword_source_spans` is non-empty |
| A0559 | tr:362-363, tr:5366-5368 | REACHABLE | State lookup falls back to the first machine state when the expected name is absent |
| A0560 | trw:5630-5634, trw:5715-5725 | REACHABLE | Missing schema `encoding` defaults to `compact_binary`, satisfying a canonicality demand |

## A0553

**Trace.** Read `structural_qualification_inventory` and nested `collect` in `scc:2748-3055`. For `TypeReferenceNode::Named` or `Generic` (`scc:2952-2954`), if `program.closed_data_definition(reference)` is `None`, `collect` returns `Ok(())` and does not recurse into fields (`scc:2953-2955`); erased primitives and byte carriers are skipped at `scc:2979-2986`. The inventory still returns `Ok(StructuralQualificationContract { ... })` with whatever qualifications were accumulated (`scc:3049-3054`). **Callers:** `structural_qualification_inventory` is used only inside `structural_call_custody.rs` (4 references: definitions at `scc:2730`, `2737`, `2744`, `2748`); public entry points `structural_qualification_contract` / `structural_result_qualification_contract` are imported from validation `lib.rs` and called from checked-lowering, e.g. `execution_terminal_unit_types.rs:1025` (read) when building mutable-borrow domain plans. **Input that reaches the branch:** a structural parameter or return type reference that remains `Named`/`Generic` without a closed data definition at checking time. **Closing check:** none at this site; absence of definition is treated as “no qualifications” not rejection. I did not read every upstream type-reference validator to prove such a type cannot reach lowering (`inferred` gap only for whether typed validation always closes the type first).

## A0554

**Trace.** Read `validate_machine_single_requirement` (`tr:1437-1561`), called once from `validate_machine_trait_conformances` (`tr:848`; repository search `validate_machine_single_requirement` in `omega-rust`: 1 production call site in `traits.rs`). Earlier conformance checks diagnose bad lifetime *counts* and out-of-range ordinals against `authored_machine` (`tr:782-801`, read). Inside `validate_machine_single_requirement`, lifetimes are built by indexing `machine.lifetime_parameters` (`tr:1508-1516`), not `authored_machine`; on any missing slot, `collect` yields `None` and the function `return`s at `tr:1517-1518` with no diagnostic. `AppliedTraitScope::new` (`product_and_trait_declarations.rs:3721-3756`, read) returns `None` on argument/lifetime count mismatch or `enclosing::append_data_application` failure; at `tr:1529-1530` that again `return`s silently, skipping `validate_applied_trait_signature`, `validate_concrete_named_evidence_contract_conformance`, and `check_law_conformance` (`tr:1532-1560`). `validate_trait_application_obligations` at `tr:822` does not call `AppliedTraitScope::new` (read `tr:3250-3284`). **Input:** a conforming machine where `machine` has a shorter lifetime telescope than `authored_machine` while ordinals remain valid for the authored owner, or a trait application scope that `new` cannot build though conformance argument counts matched.

## A0555

**Trace.** Read `validate_named_contract_lane` (`tr:1651-1765`), reached from `validate_concrete_named_evidence_contract_conformance` (`tr:1614-1637`; caller search: 1 site at `tr:1542`). For each named requires/ensures pair, facts must be singleton proposition arrays (`tr:1716-1724`); otherwise `continue` skips the `normalize_nominal_proposition_application` comparison (`tr:1747-1763`). Generics in propositions are rejected separately (`tr:1725-1735`). **Input:** a machine satisfying a trait requirement with matching named contract row cardinality and bindings, but required or actual facts are membership or expression facts (or multiple propositions) instead of one proposition each—no equality diagnostic is emitted for that row.

## A0556

**Trace.** Read `validate_implementation_type_arguments` (`tr:3669-3726`), invoked from `validate_trait_application_obligations` for `TypeParameterKind::Implementation` (`tr:3336-3344`; search `validate_implementation_type_arguments`: 2 hits in `traits.rs`, definition + call). Non-`Generic` argument: `return` at `tr:3686-3687`; missing conformance: `tr:3689-3690`; parameter/argument length mismatch: `tr:3696-3697`—all without diagnostics. Read `validate_implementation_application` (`tr:7895-7909`): arity mismatch `return`s at `tr:7907-7908` silently before bound validation. **Input:** trait application with an `Implementation` type parameter whose argument is not a generic conformance reference, points at no conformance, or has the wrong generic arity—implementation bounds and identity checks on that path are skipped.

## A0557

**Trace.** Read `type_reference_lifetimes_match_requirement_application` (`tr:2892-2924`) and `requirement_lifetime_matches` (`tr:2926-2952`). When the requirement-side lifetime is `None`, `requirement_lifetime_matches` returns `true` at `tr:2933-2934`; when the required name is not in `target_lifetimes`, it returns `true` at `tr:2939-2940`. **Callers:** `type_reference_lifetimes_match_requirement_application` is referenced only in `traits.rs` (read): parameter and return checks in `validate_machine_top_level_requirement_conformance` at `tr:1068-1093`, gated on equal lifetime parameter counts (`tr:1060-1061`). **Input:** top-level boundary provider whose reference types carry a provider lifetime not tied to any requirement lifetime parameter—the lifetime telescope check passes even when regions disagree.

## A0558

**Trace.** Read `refinement_fit_diagnostics` (`tr:7381-7520`). For each conformance row, if refinement covering clauses exist (`tr:7396-7398`), the loop may emit contract diagnostics. The `suspends` mismatch check requires `!clause.signature.suspends_keyword_source_spans.is_empty()` (`tr:7490-7496`) alongside `!clause.signature.suspends && machine.suspends`. **Callers:** `refinement_fit_diagnostics` is called from `traits.rs:8079` and `machine_parameters_and_bindings.rs:8152` (search count 3 production references). **Input:** generic refinement binding whose clause signature records `suspends: false` but loses the keyword span metadata while the selected realization machine still suspends—no reject diagnostic.

## A0559

**Trace.** Read `producer_entry_state` (`tr:349-364`): match state by leaf name after `rsplit("::")`, else `.or_else(|| program.machine_states(producer).first())` at `tr:362-363`. Used in external `via` producer validation (`tr:333-336`, read). Read `family_realization_state` (`tr:5356-5368`): match template state name on instance, else fall back to `.first()` at `tr:5366-5368`; used when lowering dynamic family rows (`tr:5270-5280`, read). **Input:** producer or family instance whose states omit the expected name (or use a different naming pattern) but still have at least one state—the fallback state is treated as the match.

## A0560

**Trace.** Read `compatibility_demand_verdict` in `trw:5613-5751`. `codec` is taken from local then peer `schema.encoding`, else defaults with `.unwrap_or("compact_binary")` at `trw:5630-5634`; `compact_binary` is true when that string equals `"compact_binary"` (`trw:5635`). `canonical_value` is `compact_binary && local_schema.is_some() && peer_schema.is_some()` (`trw:5715-5716`); `canonicality` fact uses that value (`trw:5724-5725`). Schemas with no declared encoding therefore count as canonical-capable when both sides resolve. **Callers:** `compatibility_demand_verdict` from `check_wire_demands` (`trw:6324`; search count 3 in `type_references_and_wire.rs`); failures surface only via `validate_wire_protocol` when `!demand.satisfied` (`trw:6445-6461`, read)—a required canonicality demand can be marked satisfied without any declared codec.

## What I did not read

- Full bodies of `validate_type_reference_handle` and all `closed_data_definition` prerequisites in `value_custody_core.rs` (would decide how often A0553 inputs survive validation).
- `enclosing::append_data_application` (`product_and_trait_declarations.rs`, not opened) for all `AppliedTraitScope::new` failure modes beyond count mismatch.
- `authored_conformance_machine` implementation (would fully nail the `machine` vs `authored_machine` lifetime telescope mismatch for A0554).
- Corpus or sample programs demonstrating each reachable case (read-only atlas; no build).

SLICE COMPLETE
