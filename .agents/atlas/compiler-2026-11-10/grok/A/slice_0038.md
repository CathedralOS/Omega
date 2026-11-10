# slice_0038 (rows A0285–A0292)

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0285 | prf/obligations.rs:344 | REACHABLE | SavedSubject assignment targets can be typed in validation but make `expression_type_reference` return `None`, so no bounded obligation is emitted. |
| A0286 | prf/obligations.rs:3229 | REACHABLE | A `Domain` constraint whose name is not in `ValueDomain::from_name` is dropped in `filter_map`, so a bounded obligation may be checked against fewer constraints. |
| A0287 | prf/obligations.rs:1894 | CLOSED | `check_proof_plan_with_decisions` always passes `None` for the derivation cache; a `None` key only skips cache retain/reuse, not obligation discharge. |
| A0288 | pc/lib.rs:77 | UNDECIDED | I did not trace a production caller that varies `provider_inventory`, `external_declarations`, or `publication` under an equal `ProductionKey`. |
| A0289 | pc/source_checking.rs:744 | CLOSED | Every prior-judgment reuse at this site requires `RetainedCheck::judges_preparation_input` (typed program and admission equality). |
| A0290 | pc/source_compilation.rs:86 | REACHABLE | Selected files with `package_identity == None` skip product-alias and declaration-authority compatibility checks via `continue`. |
| A0291 | tp/build_bindings.rs:696 | UNDECIDED | I did not trace whether a silently dropped protocol call must fail compilation or only omits companion metadata. |
| A0292 | tp/build_bindings.rs:394 | UNDECIDED | I did not trace whether skipped binding-call rows must fail compilation or only shrink the companion record. |

## A0285

Digest item 17 (`area_11.md:278`, read). Cited lines re-located at `omega-rust/psi/compilation/proof/src/obligations.rs:344-345` (read).

`collect_bounded_assignment_obligation` (read full function `:324-414`): on assignment statements from `build_proof_plan` (`:115-123`, read). When `expression_type_reference(program, machine, state, target)` is `None`, the function returns immediately with no diagnostic (`:344-346`), so no `ProofObligation::BoundedAssignment` is pushed (`:411-413`).

Caller search: `rg '\bcollect_bounded_assignment_obligation\b' omega-rust` → 2 matches (definition + one call in `obligations.rs`, read).

`expression_type_reference` in `proof/src/lib.rs:605-644` (read): handles `Borrow`, `Name`, `Member`, `Indexed`, `ZeroValue`; `_ => None` (`:643`). No `SavedSubject` arm.

`declared_place_type` in `validation/src/places.rs:1255-1294` (read): resolves `ExpressionNode::SavedSubject` via `saved_value_source` and `expression_result_type_reference` (`:1286-1294`).

Input that reaches the branch: a state assignment whose target is a saved-subject place with a constrained declared type passes validation typing via `declared_place_type` (`program_validation.rs:1581-1582`, read) but leaves obligation collection at `:344-346` before any bounded check.

Plain words: a program that assigns an out-of-range integer into a bounded saved-subject slot can pass proof planning without a bounded-assignment obligation and then pass `check_proof_plan_with_decisions` (`05_typed-trees-to-checked-trees/src/checking.rs:422`, read) because the obligation was never listed.

## A0286

Digest item 18 (`area_11.md:279`, read). Cited `:3195` and `:3229` at `omega-rust/psi/compilation/proof/src/obligations.rs:3191-3196` and `:3222-3235` (read).

`ProofPlan::store_constraint_nodes_with` (read `:3180-3197`) keeps only constraints where `from_node` returns `Some` (`filter_map` at `:3195`). `ProofConstraint::from_node` (`:3227-3235`, read): `TypeConstraintNode::Domain(domain)` maps through `ValueDomain::from_name(domain.as_str())`; on miss, the arm yields `None` and the constraint is omitted.

`collect_bounded_assignment_obligation` stores constraints via `store_constraint_nodes_with_domain_intervals` (`:372-376`, read), which calls `from_node` first and only then tries interval lifting (`:3238-3253`, read).

Domain lowering: unknown zero-argument domain spellings become `DomainConstraintSubject::Declared` (`03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs:1373-1386`, read), so a non-kernel domain name can remain on the typed constrained type.

Plain words: a program whose constrained type carries a declared domain name the proof layer does not recognize can still get a bounded assignment obligation, but with that domain constraint stripped from the proof constraint span, so discharge may not enforce the authored domain.

## A0287

Digest item 19 (`area_11.md:280`, read). Cited `proof_obligation_key` at `omega-rust/psi/compilation/proof/src/obligations.rs:1894-2065` (read enclosing function in full).

Returns `None` when identity construction sets `unavailable` (`:2060-2065`, read); sets include bad saved subjects and invalid captures (`:2354-2387`, read).

Production discharge: `check_proof_plan_with_decisions` (`checker.rs:6468-6472`, read) calls `check_proof_plan_inner(..., None)` — no `ProofDerivationCache`. Loop at `:6486-6532` still runs `check_bounded_assignment` and siblings regardless of key.

`DerivationConsultation::new` (`checker.rs:2931-2939`, read): `key: proof_obligation_key(...)` may be `None`; `recheck` (`:2942-2960`) and `retain` (`:2963-2966`) no-op without a key but do not skip `check_obligations!`.

`derivation_store::store_for` (`derivation_store.rs:98-107`, read) returns `Ok(None)` without key; `candidates_for` (`:126-134`, read) yields empty iterator.

Caller search: `rg '\bproof_obligation_key\b' omega-rust` → 30 matches in `obligations.rs` (20), `checker.rs` (5), `derivation_store.rs` (5) (read counts).

Plain words: keyless obligations still go through the same discharge path; only optional derivation reuse is disabled. Production checking never supplies the cache, so acceptance does not depend on forming a key.

## A0288

Digest item 20 (`area_11.md:283`, read). `ProductionKey` at `omega-rust/psi/compilation/psi-compiler/src/lib.rs:51-84` (read full `new` and `matches`).

Fields: `program` (`Arc<CheckedTrees>` ptr), `entry`, `machines`, `boundaries`, `exact_toolchain_sources`, `optimizations` (`:67-83`, read). Not included: `TerminalProductionRequest::{publication, external_declarations, provider_inventory}` (`terminal-production/src/lib.rs:402-410`, read).

`PortableProductions::produce` (`pc/lib.rs:103-126`, read): equal keys share one `OnceLock` result; closure runs once.

`product_code.rs:84-93` and `:123-133` (read): `PortableProductionInputs` omits inventory/publication; closure passes `provider_inventory: Some(self.inventory)` into `TerminalProductionRequest`. Shared `Arc<psi_compiler::PortableProductions>` lives on `ProductSelections` (`source_checking.rs:847-848`, read).

`external_declarations` non-`None`: only lowering signature reference (`06_checked-trees-to-lowered-psi/src/machine_lowering.rs:1679`, grep, not read in body). Production `product_code.rs` sets `None` (`:126`, read).

Plain words: cache collision would require two produces with equal key fields but different terminal-only inputs. I did not find a traced caller that does that in one session.

## A0289

Digest item 21 (`area_11.md:284`, read). Site `omega-rust/psi/compilation/psi-compiler/src/source_checking.rs:741-751` (read enclosing check function from `:700`).

Reuse: `self.previous.iter().find(|prior| prior.judgment.judges_preparation_input(&program, &admission, self.admission.external_roots))` (`:743-748`, read). Match invokes `reuse_retained_check` (`:757-761`, read).

Predicate read: `05_typed-trees-to-checked-trees/src/checking.rs:849-857` (read full `judges_preparation_input` and `judges_preparation_admission` `:863-868`): requires `self.preparation_admission == *admission`, `self.external_roots == external_roots`, and `*self.preparation_input == *program`.

Caller search: `rg '\bjudges_preparation_input\b' omega-rust` → 5 matches: definition/comments in `checking.rs` (3), use `source_checking.rs:744` (1), string in `product_boundary.rs:49` (1) (read counts).

Plain words: no retained judgment is taken unless typed trees and admission metadata match the current request; the branch is the sole gate before `reuse_retained_check`.

## A0290

Digest item 22 (`area_11.md:285`, read). `SourceContext::compatibility` `omega-rust/psi/compilation/psi-compiler/src/source_compilation.rs:50-122` (read full function).

Per selected activation file (`:74-88`, read): import binding equality is always checked (`:83-85`). If `file.package_identity` is `None`, `continue` (`:86-88`) skips `same_product_dependency_scope` (`:89-91`) and declaration-authority comparisons (`:92-118`).

Called from activation path at `:197` (read reference only).

Plain words: a retained frontend can be reused when an unpackaged selected source’s declaration authority or product-alias picture changes, because those files never enter the authority/alias loop.

## A0291

Digest item 23 (`area_11.md:288`, read). Module `build_bindings.rs` `capture` for protocol calls `:652-713` (read full `capture`).

Inner loop `:676-699` (read): for each lowered call occurrence, resolves `target` from statement or expression site; if no declaration row matches `target` (`declarations.iter().find` at `:696-698`), `continue` drops the call with no diagnostic.

Invoked from terminal production `lib.rs:676-712` path — `capture` for binding calls at `:479-480` (read); errors only from explicit `Err` arms, not from `continue`.

Plain words: a boundary Build-protocol call present in lowering but not matching a captured trait signature disappears from the companion list. Whether compilation must reject such a program was not traced.

## A0292

Digest item 24 (`area_11.md:289`, read). `build_bindings.rs:387-417` (read full `capture` for binding calls).

`:393-402` (read): occurrences without `source_site`, non-call expressions, or `!call.build_selection.is_valid()` hit `continue` with no diagnostic.

Same production caller as A0291 (`terminal-production/src/lib.rs:479`, read).

Plain words: lowered build-binding call occurrences that lost source custody or valid selection are omitted from companions silently. Whether that should fail the compile step was not traced.

## What I did not read

- Full `adm/*`, `md/*`, and remaining `ct/*` bodies outside cited rows.
- `wiki/spec/*` rules for Build companion completeness.
- `06_checked-trees-to-lowered-psi/src/machine_lowering.rs` body around `external_declarations`.
- Consumers of `TerminalBuildBindings` after `TerminalBuildBindings::new` (`tp/lib.rs:501+`, not opened).
- `git show 457adf4a11f` line-by-line verification (current paths used; lines re-located where noted).

SLICE COMPLETE

