# slice_0129

Claims: ACCEPT, area_07. Digest preamble read at `.agents/atlas/compiler-2026-11-10/digests/area_07.md:1-14` (read).

## Summary table

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0992 | omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs:932-934 | REACHABLE | `has_continuation` skips the whole exhaustiveness run with no later S03 exhaustiveness pass. |
| A0993 | omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs:955-967 | REACHABLE | Compare-arm closure uses structural equality that equates calls by target name string and ignores cast target types. |
| A0994 | omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs:257-259 | REACHABLE | Three-segment case name operands and machine binders with evidence projection exit without rejection. |
| A0995 | omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs:1883-1895 | REACHABLE | `validate_computed_arguments` runs only when frontier counts differ from the resolved tree counts. |
| A0996 | omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs:322-324 | REACHABLE | Progress premises are normalized before domain membership settlement and skip invalid domain symbols. |
| A0997 | omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs:2128-2136 | REACHABLE | Ambiguous domain selection leaves an invalid cast domain and invalid `result_type` without error; construction `land_one` can no-op. |
| A0998 | omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/contracts_and_signatures.rs:193-194 | REACHABLE | Contract membership with not-exactly-one domain candidate stays invalid through S03/S04; checker paths skip invalid facts. |
| A0999 | omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/declarations.rs:1550 | REACHABLE | `::provider_defaults` purpose is assigned by name suffix only with no toolchain-origin gate. |

## A0992

**Verdict:** REACHABLE (read).

**Cited site:** `check_dispatch_run` at `equality_and_exhaustiveness.rs:922-934` returns `Ok(())` when `has_continuation` is true, before any arm classification or matrix work (`read`).

**Enclosing function:** `check_dispatch_run` (`equality_and_exhaustiveness.rs:922-1010+`, read through the early-return and the `compare_pair_closes` branch).

**Callers:** Search `\bcheck_dispatch_run\b` in `omega-rust` — count 4, all in `equality_and_exhaustiveness.rs` (definition at `:922`, calls at `:860`, `:866`, `:872`). Sole external entry: `validate_case_dispatch_exhaustiveness_in` (`:829-876`), which sets `run_has_continuation |= transition.continuation.is_valid()` (`:858`) before each call (`read`).

**Upward trace:** `validate_case_dispatch_exhaustiveness_in` is invoked from `validate_resolved_semantics_in` (`lowerer.rs:1619`) during `lower_symbol_resolved_trees` (`lowerer.rs:1625`), before typing completes (`read`). Search for `has_continuation` exhaustiveness in stage 05 found no second exhaustiveness validator on transition guard matrices (`inferred` from grep in `05_typed-trees-to-checked-trees`).

**Input that reaches the branch:** A machine state whose transition run has at least one guard with `transition.continuation` valid and whose finite/compare arms do not cover all dispatch subjects, with no `_` arm in that run (`read` at `:857-860`).

**Plain-language accept gap:** A program whose state dispatches on a finite subject but arms only cover part of the space, while one transition in the same run continues elsewhere, should be rejected for incomplete dispatch; the compiler returns `Ok(())` for that run and proceeds with lowering.

## A0993

**Verdict:** REACHABLE (read).

**Cited site:** `compare_pair_closes` at `equality_and_exhaustiveness.rs:955-967` treats paired `==` / `!=` arms as closing the run when `expressions_structurally_equal` holds on both sides (`read`).

**Structural equality:** `expressions_structurally_equal` (`:1801-1817`, read): `Cast` compares only `left.value` to `right.value`; `Call` compares `left.target.as_str() == right.target.as_str()` plus receiver and argument subtrees, not resolved callee symbols (`read`).

**Enclosing flow:** Same `check_dispatch_run` path as A0992 when `has_continuation` is false (`read`).

**Plain-language accept gap:** Two transition guards that spell the same call target but resolve to different callees (or two casts to different types with the same value expression) are treated as mutually exclusive compare arms that complete coverage, so a dispatch that should be rejected as non-exhaustive can pass.

## A0994

**Verdict:** REACHABLE (read).

**Sites:**

1. `check_equality_operand` (`equality_and_exhaustiveness.rs:250-298`): for `ExpressionNode::Name`, destructuring `let [type_name, case_name] = members else { return Ok(()); }` at `:257-258` accepts any name path that is not exactly two members without diagnostic (`read`). Digest `:297` ancillary: other arms return `Ok(())` for non-payload cases (`read`).

2. Proposition lowering (`equality_and_exhaustiveness.rs:2159-2161`, relocated from digest `:2160-2162` at same commit shape): `PropositionBinderKind::Machine` with `argument.evidence_projection.is_some()` assigns `(ArgumentKind::Machine, symbols::SymbolHandle::invalid())` with no further machine-resolution check in that branch (`read`). The following `symbol.is_valid()` guard at `:2178` skips type insertion only (`read`).

**Callers of `check_equality_operand`:** Only within `validate_equality_operands_in` scan (`equality_and_exhaustiveness.rs:184-185`, `:199`, read). Entry: `validate_resolved_semantics_in` (`lowerer.rs:1615`).

**Plain-language accept gap:** A three-segment bare case name used as an equality/match operand is not rejected; a proposition machine binder written with an evidence projection is lowered with an invalid machine symbol and no error at this site.

## A0995

**Verdict:** REACHABLE (read).

**Cited site:** `close_typed_cohort` (`lowerer.rs:1883-1895`) calls `validate_computed_arguments` only when generic-application-origin, operator, and data-definition frontier counts all equal the corresponding `source_trees` lengths (`read`). Otherwise the call is skipped.

**`validate_computed_arguments`:** Defined at `type_reference.rs:1737+`; can return errors on malformed origins when invoked (`read` at `:1788-1798` sample).

**Callers:** Search `\bvalidate_computed_arguments\b` in `omega-rust` — count 4 (definition + `pub(crate) use` in `type_reference.rs:2752`, import and single call in `lowerer.rs:28`, `:1894`) (`read`).

**Plain-language accept gap:** After incremental typing extends only new machine bodies, cohort counts can still match the resolved tree totals while new computed generic arguments are wrong; validation never runs and compilation continues.

## A0996

**Verdict:** REACHABLE (read).

**Cited site:** `authored_premises` (`lowerer.rs:308-345`): for each `requires` membership fact, `let Some(domain) = program.domain_definition(membership.domain_symbol) else { continue; }` at `:322-324` silently drops facts whose domain symbol is still invalid (`read`).

**Subject path:** `subject_path` (`:348-371`) can match parameters by spelled name when `path.symbol` differs (`:363-370`, digest `:363-371`, read).

**Order in `close_typed_cohort`:** `normalize_machine_progress_premises` at `lowerer.rs:1854` runs before `normalize_membership_selections` at `:1870` and `intern_proof_membership_instances` at `:1871` (`read`). Premises are therefore frozen without domains that settlement later resolves.

**Callers of `authored_premises`:** Search in S03 lowerer — count 3 references (`:67`, `:116`, `:308` definition) (`read`).

**Plain-language accept gap:** A machine with a published termination contract and a `requires x in D` progress-domain fact whose `D` is not yet resolved at premise extraction gets fewer authored premises than the source contract states; lowering succeeds without rejecting the unresolved membership at this stage.

## A0997

**Verdict:** REACHABLE (read).

**Cited site:** `normalize_qualification_casts` (`call_and_product_selections.rs:2119-2135`): `select_domain_candidates` may return zero or many matches; `let [domain] = matches.as_slice() else {` pushes `SymbolHandle::invalid()` and `SemanticDomainId::NULL`, then `continue` with no error (`read`). Final application sets `result_type` to `TypeReferenceHandle::invalid()` when `domain_definition(domain)` is absent (`:2212-2214`, read).

**Construction destinations:** `prepare` (`:1377-1419`, read): unresolved call targets `continue` at `:1389-1390`; `land_one` returns `Ok(())` at `:1495-1500` when `closed_data_constructor_definition` is `None`, leaving destinations unprepared (`read`).

**Stage 05:** `judge_value_qualification_cast` (`checks_contracts_writes.rs:2171-2173`) returns early when `domain_definition` is missing — no diagnostic (`read`).

**Plain-language accept gap:** A qualification cast `as T in D` where `D` is ambiguous or missing is lowered with invalid domain and result type; downstream qualification judging skips it instead of rejecting the program.

## A0998

**Verdict:** REACHABLE (read).

**Sites:**

1. `lower_authored_invocations` (`contracts_and_signatures.rs:36-46`): trait invocation targets unresolved when lookup fails map to `AuthoredInvocationTarget::Unresolved` without error (`read`).

2. `domain_selections::normalize` (`contracts_and_signatures.rs:151-194`): skips rows with valid `domain_symbol` (`:151-152`); when `select_domain_candidates` does not return exactly one match, `let [domain] = candidates.as_slice() else { continue; }` at `:193-194` leaves `domain_symbol` invalid (`read`).

**Downstream:** `intern_proof_membership_instances` (`:450-451`) `continue`s on invalid `domain_symbol` (`read`). Stage 04 membership intern (`monomorphization.rs:5079-5080`) also `continue`s on invalid symbol (`read`). Stage 05 `checks_content.rs:5814-5816` processes requires membership only when `membership.domain_symbol.is_valid()` (`read`).

**Plain-language accept gap:** A contract `requires` fact whose domain path does not resolve to exactly one domain keeps an invalid domain symbol through typing; checking ignores the fact rather than failing compilation for the unresolved membership.

## A0999

**Verdict:** REACHABLE (read).

**Cited site:** `finalize_provider_default_calls` (`declarations.rs:1549-1552`): any machine whose `name` ends with `"::provider_defaults"` receives `MachinePurpose::ProviderDefaults` with no file-origin or toolchain gate (`read`).

**Related gaps (digest, read):** `attached_domain_symbol` (`:683-701`) returns `invalid()` when zero or multiple domain name matches (`read`). `resolve_satisfied_declaration` missing for conformance (`:1165-1168`) causes `continue` without error (`read`). Unresolved provider selection calls are still collected when `!call.target_symbol.is_valid()` (`:1607-1610`, read).

**Stage 05:** `PortableReferences::declaration` (`checking.rs:708-710`) treats `ProviderDefaults` machines differently from ordinary declarations (`read`) — classification affects checking scope, not a hard reject at S03.

**Plain-language accept gap:** An author-defined machine merely named with a `::provider_defaults` suffix is classified as provider-defaults machinery and processed through provider-default finalization though it is not the toolchain template.

## What I did not read

- Full bodies of `validate_computed_arguments` beyond the early malformed-origin paths (`type_reference.rs` after `:1800`).
- Remainder of `check_dispatch_run` after the matrix diagnostic branches (only portions needed for continuation and compare closure were read).
- Stage 05 checking beyond the cited qualification, membership, and provider-default sites (no end-to-end compile run; build forbidden).
- Omega sample programs that witness each gap (no corpus execution).

SLICE COMPLETE
