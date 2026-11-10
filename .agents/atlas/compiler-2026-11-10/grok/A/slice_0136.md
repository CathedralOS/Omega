# slice_0136

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1045 | TP/terminal_module.rs:99-101 | REACHABLE | Folded-constant dependency with no `candidate` passes `validate` and skips association checks. |
| A1046 | TP/types_and_values.rs:2180-2182 | REACHABLE | `ElementViewWrite` element path and `StructuralCase` per-case args are omitted from path/projection enumerations. |
| A1047 | TP/lib.rs:891 | UNDECIDED | Default `CallbackUseDeclaration` and `Unresolved` loan placeholders read; did not trace every consumer that rejects before accept. |
| A1048 | TT/calls_and_control_flow.rs:759-772 | REACHABLE | After a finite `requires` disjunction is chosen, later non-expression or non-finite `requires` facts are not applied to the family. |
| A1049 | TT/evidence_and_inspection.rs:1911-1919 | REACHABLE | Witness text fallback can bind an expression outside the root state when duplicate rendered text exists elsewhere. |
| A1050 | TT/values.rs:4003 | REACHABLE | Spelling-only match can treat a shadowing one-member name as the parameter without symbol agreement. |
| A1051 | TT/operator_declarations.rs:3429-3448 | REACHABLE | Name/text fallbacks can equate unrelated declarations sharing spellings across the cited sites. |
| A1052 | TT/operator_declarations.rs:1998-2003 | REACHABLE | Primitive actual type can satisfy a non-primitive expected operand when `primitive_type` is `None`. |

## A1045

Read `FoldedConstantClosure::validate` in full at `omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module.rs:75-131` (re-located; digest cited 99-101).

At `terminal_module.rs:99-101` (read), when `dependency.candidate` is `None`, the loop `continue`s and skips the association uniqueness check at `:102-116`.

Search `\.validate\(module\)` on `folded_constants` in `omega-rust`: 1 match — `omega-rust/psi/compilation/representations/terminal-psi/codec/src/foundation_validation.rs:10260-10261` (read), inside `validate_structural_foundation`, which runs on Terminal Psi decode.

Plain words: a `TerminalModule` whose folded-constant dependency has complete realization identity fields but `candidate: None` is accepted by `validate` without proving provider-candidate association; a module that should be rejected for a missing checked association could pass decode if other foundation checks succeed.

At `terminal_module.rs:1109-1114` (read), `callback_application` uses `binary_search_by_key` on `callback_applications` with no local sortedness check.

Search `require_increasing` with `callback_applications` in `omega-rust/psi/compilation/representations/terminal-psi/codec`: `semantic_module.rs:61-66` (read) enforces increasing `application_index` in `validate_canonical_order` before decode acceptance — closing check for the byte decode path.

At `terminal_module.rs:476-496` (read), `ScalarPredicateCondition::evaluate` passes `&|_, _| true` as `comparison_is_admitted` to `evaluate_nodes` (`omega-rust/psi/foundation/numerics/src/lib.rs:678-767`, read). At `lib.rs:743-746` (read) the callback only gates non-`And`/`Or` operators; at `:760` (read) non-integer/non-boolean pairs still yield `None`, so this callback alone does not force a false boolean acceptance (inferred).

## A1046

Read `OperationKind::structural_projections` at `types_and_values.rs:1886-1909` and `structural_projection_paths_mut` at `:2125-2200` (re-located 2180-2182).

At `:1899-1900` (read), `StructuralStorageUse::Element { descriptor, .. }` pushes only `descriptor`, dropping the separate element `path` carried on `ElementViewWrite` at `:1977-1983` (read).

At `:2180-2182` (read), `ElementViewWrite` pushes only `destination.path`, not the inner element-view `path` used for runtime indices.

Search `structural_projection_paths_mut` in `omega-rust`: 3 production call sites — `control_flow.rs:1602`, `emission_cleanup.rs:3971`, `storage_primitives.rs:4661` (read call lines only). `emission_cleanup.rs:3971-3976` (read) remaps `RuntimeIndex` obligations only in paths returned by `structural_projection_paths_mut`; missing element-view paths would not be remapped (inferred).

Read `Terminator::storage_uses` at `control_flow.rs:2328-2372` (re-located 2360-2365). `StructuralCase` at `:2360-2364` records only `source` and `path`; `StructuralCaseSuccessorEdge` at `:2605-2611` (read) also carries per-case `structural_arguments`, which are not listed in `storage_uses`.

Read `OperationKind::module_invocation` at `control_flow.rs:387-473` (re-located `:472`). `CallStructural` is handled at `:455-471` (read) and returns `Some`; the digest claim that `:472` silently drops a call-like variant does not match — `:472` is the catch-all for unrelated variants (read).

## A1047

Read `CallbackUseDeclaration::default` at `TP/lib.rs:884-900` (re-located 891) and `TerminalSuspensionLoanCapture` / `TerminalSuspensionLoanResource` defaults at `TP/ownership.rs:644-705` (read).

`requirement: Handle::default()` at `lib.rs:891` (read) and `Unresolved` defaults at `ownership.rs:662-675, 674-675` (read) are placeholder roster values, not acceptance checks at this site.

Search `TerminalSuspensionLoanResource::Unresolved` in `omega-rust`: 4 matches in production (`ownership.rs`, `retention.rs`); did not read full upward paths from omega emission or validation to a compile accept/reject decision.

What would decide: reading consumers that build or validate `CallbackDeclarationRoster` and suspension loans before checked/lowered emission accepts a module.

## A1048

Read `finite_declaration_family` in full at `calls_and_control_flow.rs:720-774` (re-located 759-772).

At `:752-773` (read), the loop over `Requires` facts: `:759-761` skips facts that are not expressions; when `candidate` is `FamilyProbe::Finite` and `family` is already `Finite`, later candidates are not merged (`:763-769`).

Search `finite_signature_family` in `omega-rust`: 17 matches across 12 files (count mode). Read caller `omega-rust/psi/compilation/validation/src/calls.rs:1159-1178` — `validate_family_tuple` uses the finite tuple list for call checking.

Plain words: a signature with one finite explicit disjunction of binder equalities plus another `requires` clause (non-expression, or not expressible as the same finite family) can still yield `FamilyProbe::Finite` listing only the first disjunction’s tuples; a call tuple forbidden only by the ignored clause could be treated as allowed (inferred).

## A1049

Read `resolve_witness_expressions` in full at `evidence_and_inspection.rs:1891-1921` (re-located 1911-1919).

At `:1908-1910` (read) the resolver prefers expressions in `state`; at `:1911-1918` (read) it falls back to any program expression whose `witness_expression_text` equals the expected string.

Search `resolve_machine_witness_expressions|resolve_witness_expressions` in `omega-rust`: 17 matches across 7 files. Read `checks_termination.rs:2319-2324` (read) — termination witness selection calls `resolve_machine_witness_subjects` / `resolve_machine_witness_view_arguments`, which call this resolver.

Plain words: two different expressions in different machine states that render to the same witness text can cause the ranking/termination check to use the wrong expression when the root-state search misses.

## A1050

Read `ExpressionTable::names_parameter` in full at `values.rs:3990-4004` (re-located 4003).

At `:4001-4003` (read), acceptance is true when `name.as_str() == parameter.name` even if `path.symbol` / `path.head_symbol` do not match `parameter.symbol`.

Search `\bnames_parameter\b` in `omega-rust`: 18 matches in 7 files (count mode) — 1 definition (`values.rs:3990`), test/local closures in `access_views_and_commitments.rs` (5), production validation `proof_contracts_core.rs` (6), `arithmetic_domains.rs` (2), pipelines `checks_content.rs` (1), `constructed_results.rs` (2), `unit_composed_control.rs` (2). Read `proof_contracts_core.rs:2162` (read) as one production use on `requires` membership facts.

Plain words: a one-segment name spelling equal to a state parameter’s name but bound to a different symbol (e.g. shadowing local) is still treated as naming that parameter.

## A1051

Read `type_reference_matches_with_policy` entry at `operator_declarations.rs:3450-3458` and `nominal_type_identity_matches` at `:3745-3767` (read `allow_name_fallback` at `:3752-3755`). Read `find_type_parameter` at `:3665-3674` (re-located 3673).

Read `substitute_boundary_type` name arm at `TT/lib.rs:409-420` (re-located 416-419).

Read `type_multiplicity` name fallback at `DTI:6141-6154` and `:6167-6173` (re-located 6148-6171).

Read `type_identity` binder fallback at `type_identity.rs:1997-2006` (read).

Read `PrimitiveType::from_name` at `type_system.rs:4648-4674` (read).

Read `byte_predicate_call` at `evidence_and_inspection.rs:16-29` (re-located 21-24): requires invalid `target_symbol` and `receiver` (read).

Read integer name parse at `values.rs:94-99` (read).

Plain words: whenever symbol resolution is missing or differs but spelling matches (or parses as primitive/integer), the cited matchers can treat unrelated declarations as the same type or operand; I did not trace whether Omega shadowing rules forbid the motivating source programs (UNDECIDED for spec; REACHABLE at the matcher sites).

## A1052

Read operator operand matching closure at `operator_declarations.rs:1991-2003` (read): for `OperandType::Primitive`, `primitive_type(expected.type_reference).is_none_or(|expected| expected == primitive)` (read) returns true when expected type reference is not a primitive.

Read `declared_domain_constraint_matches` at `:2309-2316` (read): when both semantic ids are valid, only `semantic_id` equality is compared; argument lists skipped.

Read `resolve_named_call` at `:781-791` (read): valid `target_symbol` returns `operator_definition_by_symbol` with no visibility/arity check on that branch (arity checked only in the name-resolution branch at `:793-805`, read).

Plain words: a primitive-typed operand can match a user-type operator parameter; distinct domain constraints with the same semantic id can match; a resolved operator symbol is returned without the name-path arity filter.

## What I did not read

- `git show 457adf4a11f:` paths for line drift beyond re-location by symbol.
- Full upward traces from every `names_parameter` / witness / finite-family caller to final `omega` compile accept.
- Language spec on shadowing and folded-constant candidate requirements.
- Entire `type_reference_matches_with_policy` body and all operator-declaration callers.
- Omega producer paths that build `TerminalModule` in memory without `decode_module`.

SLICE COMPLETE
