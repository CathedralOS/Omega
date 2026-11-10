# slice_0001

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0001 | omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/preparation.rs:47-61 | UNDECIDED | `normalize` skips failed extents; `require_evaluated_constants` does not mention `FixedArrayLength::Expression`; full accept path not traced. |
| A0002 | omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/preparation.rs:138 | UNDECIDED | `ArrayLength`/`None` recipe positions never go `live`; whether checking still accepts without those contexts not traced. |
| A0003 | omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/preparation.rs:1452 | UNDECIDED | `continue` keeps `LateBound`; stage-05 rejection is usual but `descriptor_selection_occurrences` bypass not proved exhaustive. |
| A0004 | omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/selection_and_reflection.rs:1087 | REACHABLE | `parameter_name == name.as_str()` can bind by spelling when symbols differ. |
| A0005 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/borrow.rs:4765-4775 | REACHABLE | `TemplateDependent` plus `independent_owned_formation` yields an empty loan roster and checking continues. |
| A0006 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/borrow.rs:1475-1479 | UNDECIDED | `Ok(None)` drops reference custody when frames/contexts are missing; whether normal checking always supplies frames not read. |
| A0007 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/borrow.rs:1058 | REACHABLE | Missing formal types become invalid handles and argument collection falls back to reads, not `&mut` loans. |
| A0008 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/borrow.rs:2452-2519 | CLOSED | Sole `source_type` consumer turns `None` into a diagnostic via `inputs::plan`. |

## A0001

**Cited site (read):** `preparation.rs:47-61` in `array_extents::normalize` (`omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/preparation.rs`). Lines match the atlas cite at this checkout; not re-diffed with `git show 457adf4a11f:`.

On each pending fixed-array length expression, `closed_integer_value_in`, `check_closed_const_expression`, integer decode, and `usize::try_from` failures all `continue` without calling `set_fixed_array_length` (`preparation.rs:44-64`). The array node can remain `FixedArrayLength::Expression`.

**Callers (read):** `rg 'array_extents::normalize' omega-rust` → 3 matches, all in `preparation.rs:2033`, `:2095`, `:2119` inside `prepare_typed_trees` / `prepare_execution_applications` / `prepare_constant_execution`.

**Upstream (read):** `deferred_constants::require_evaluated_constants` at `preparation.rs:363-414` is invoked from `prepare_typed_trees` at `preparation.rs:1958` when `!request.admits_pending_inputs`. Its `pending_lengths` filter handles `ConstCall` without a recipe and some `ConstParameter` cases; the `_ => return None` arm at `preparation.rs:402` drops `FixedArrayLength::Expression` (`preparation.rs:376-402`).

**Accept path (inferred, not traced to end):** Later extent work exists (for example `validation::resolved_array_extent` at `omega-rust/psi/compilation/validation/src/places.rs:1506-1527` errors on unresolved expression lengths), but every consumer between stage-04 freeze and `CheckedTrees` publication was not followed.

**Verdict:** UNDECIDED — whether a program whose length stays `Expression` after `normalize` can still be accepted without hitting some other extent obligation.

## A0002

**Cited site (read):** `preparation.rs:130-138` in `constant_entries::materialize` (`preparation.rs:75-170`). Cite matches.

For each constant-recipe context, `live` is true only for `RetainedConstantPosition::ContractExpression(_)` or `ConstApplication { .. }`; all other positions, including `RetainedConstantPosition::ArrayLength(_)` and `RetainedConstantPosition::None`, hit `_ => false` (`preparation.rs:130-138`). With `!live`, the loop `continue`s at `preparation.rs:140-145` and never pushes that context into `pending` for `create_evaluation_entry`.

**RetainedConstantPosition variants (read):** `omega-rust/psi/compilation/representations/typed-trees/src/access_views_and_commitments.rs:1847-1856` lists `None`, `ArrayLength`, `ContractExpression`, `ConstApplication`.

**Callers (read):** `materialize` is called from the specialization loop in `prepare_typed_trees` (`preparation.rs:2009`).

**Accept path (inferred):** `array_extents::normalize` may still fold some lengths directly (`preparation.rs:15-65`). `constant_recipe_for_array_length` / `closed_fixed_array_length` (`access_views_and_commitments.rs:2103-2124`) may cover some `ConstCall` arrays. Whether a program relying on `ArrayLength` recipe contexts that never become `live` still reaches acceptance was not traced.

**Verdict:** UNDECIDED.

## A0003

**Cited site (read):** `preparation.rs:1452-1454` in `selections::bind_pre_specialization_authored_selections` (`preparation.rs:1365-1479`). When the `match` yields `None` (including non-member/non-call late bindings at `preparation.rs:1450`), `continue` skips `push_consistent_resolution`; the occurrence stays `LateBound`.

**Callers (read):** `bind_pre_specialization_authored_selections` from `prepare_typed_trees` (`preparation.rs:1972`) and `prepare_constant_execution` (`preparation.rs:2113`).

**Stage-05 closing checks (read):** `shared::authored_selections::finalization::finalize_checked_authored_selections` (`shared.rs:973+`) scans for remaining `LateBound` selections; if found (and not filtered), returns `Diagnostic::error` at `shared.rs:1416-1422`. `validation::normalize_checked_selection` returns `Err` for `LateBound` at `declarations_core.rs:1009-1013`, surfaced through `admit_request` → `validate_authored_declaration_selections` (`checking.rs:995-997`).

**Bypass (read, not exhausted):** The guard at `shared.rs:1385-1388` skips occurrences in `validation::execution_projection::descriptor_selection_occurrences` (`frozen_program_and_queries.rs:2079-2092`), which includes selections where `!program.checks_authored_selection(selection)`.

**Verdict:** UNDECIDED — usual checking appears to reject surviving `LateBound` selections, but not every occurrence is forced through the `shared.rs:1416` error (descriptor-set exclusion).

## A0004

**Cited site (read):** `selection_and_reflection.rs:1076-1088` in `selection::static_bindings::infer_static_bindings` (`selection_and_reflection.rs:1066-1110`). Cite matches; companion `:1103` is the parallel const-parameter clause.

For a `TypeReferenceNode::Named { symbol, name }` required type, binder index selection accepts `parameter_symbol == symbol`, or invalid-symbol pairs with matching names, or **`parameter_name == name.as_str()` without requiring symbol equality** (`selection_and_reflection.rs:1082-1087`). The match arm then pushes `(context.candidate_index, index, actual)` (`selection_and_reflection.rs:1090-1092`). The same name-only disjunct exists for const parameters at `:1099-1103`.

**Callers (read):** `rg '\binfer_static_bindings\b' omega-rust` → 21 symbol occurrences across 4 files (`selection_and_reflection.rs` 16, `contextual_and_requirement_calls.rs` 3, `monomorphization.rs` 1 re-export, `constructed_results.rs` 1).

**Input that reaches the branch (read):** Static argument inference comparing a generic required named type to an actual named type whose **spelling** equals a template parameter name while handles differ (for example stale or generated symbols).

**Verdict:** REACHABLE — proposals can bind the actual type to the wrong parameter index whenever the third disjunct fires before a symbol-equal match.

## A0005

**Cited site (read):** `borrow.rs:4758-4775` in `stored_calls::roster_at_expression` (`borrow.rs:4748-4812`). On `stored_reference_effects_at_expression` error `TemplateDependent`, if `independent_owned_formation(...)` is true, the function returns `Ok(Vec::new())` (`borrow.rs:4765-4774`).

**Callers (read):** `rg 'roster_at_expression' omega-rust` → 4 matches in `borrow.rs` (`:4748` def, `:7391`, `:7754`, `:7791`).

**Downstream (read):** At `borrow.rs:7754-7764`, `TemplateDependent` causes `continue` (skip initializer loans). At `borrow.rs:7791-7802`, `TemplateDependent` returns `Ok(Vec::new())` for reference-local formation (`borrow.rs:7801-7803`). Digest secondary cites `:4891-4893`, `:7764`, `:7801-7803`; read those lines in the same functions.

**Input (read):** A call-site expression where stored-reference slot effects are template-dependent but the expression still counts as independent owned formation (per `independent_owned_formation` starting `borrow.rs:4821`).

**Verdict:** REACHABLE — borrow-fact collection proceeds with no loans for that evaluation point instead of failing.

## A0006

**Cited site (read):** `borrow.rs:1475-1478` in `reference_custody::append_reference_custody` (`borrow.rs:1463+`). `frames.and_then(|frames| frames.stored_reference_call_contexts(...))` → `None` yields `Ok(None)` (`borrow.rs:1475-1478`).

**Callers (read):** `rg 'append_reference_custody' omega-rust` → 3 matches (`borrow.rs:1463` def, `:1787` re-export, `:8685` in `append_state_borrow_facts`).

**Caller behavior (read):** `append_state_borrow_facts` at `borrow.rs:8685-8709` only extends `current_loans` when `completed_references` is `Some`; `Ok(None)` is not an error.

**Frames source (read):** `build_borrow_facts` (`borrow.rs:9437-9440`) takes `call_frames: Option<&CallFrameResolver<'_>>`; comment at `borrow.rs:9434-9435` states `None` retains failed preparation. `published_facts::build_check_facts` passes `CallFrameResolver::new(program).as_ref()` (`published_facts.rs:10982-10983`). `CallFrameResolver::new` returns `None` when `CallFrameTables::build` fails (`calls.rs:15337-15341`).

**Verdict:** UNDECIDED — missing custody rows are silent, but whether a program accepted through normal checking uses `None` frames or missing stored-reference contexts was not established.

## A0007

**Cited site (read):** `borrow.rs:1048-1058` in `collect_target_argument_accesses` (`borrow.rs:1027-1067`).

Formal type for each argument is `parameters.and_then(...).or_else(instruction_parameters...).unwrap_or_default()` (`borrow.rs:1049-1058`). `collect_formal_argument_accesses` is invoked with that `receiving_type`.

**Enclosing logic (read):** In `collect_formal_argument_evaluation` (`borrow.rs:1095-1175`), both `forwarded_access` and `implicit_loan` closures return `None` when `!formal.is_valid()` (`borrow.rs:1120-1121`, `:1151-1152`), then fall through to `collect_single_expression_argument_access`, which records reads (and explicit `Borrow` nodes) but not implicit `&mut` formal loans (`borrow.rs:1174-1197`).

**Callers (read):** `rg 'collect_target_argument_accesses' omega-rust` → 3 matches (`borrow.rs:718`, `:961`, `:1027` definition).

**Input (read):** A call whose target has no `call_target_parameters` row and no matching `global_descriptor_table_operand_types` entry for that argument position, so `receiving_type` is default-invalid.

**Verdict:** REACHABLE — an argument that should borrow mutably under a `&mut` formal can be recorded only as reads.

## A0008

**Cited site (read):** `borrow.rs:2446-2616` — `lifetime_contract::source_type` (`borrow.rs:2446` definition). Digest range `2452-2519` covers early `None` returns; function continues with further `return None` paths through `:2616` (read).

**Consumers (read):** `rg 'lifetime_contract::source_type' omega-rust` → single production use at `checks_borrows_resources.rs:6679-6687` inside `inputs::plan`. `source_type(...).ok_or("completed output lifetime input does not match its captured call argument")?` turns every `None` into `plan` error.

**Upstream to diagnostic (read):** `inputs::plan` is invoked from `supplied_permissions::completions::install` (`checks_borrows_resources.rs:6010-6018`). `install` returns `Result<(), Vec<Diagnostic>>`. `initialize_checked_direct_borrow_resources` calls `supplied_permissions::install` and propagates `Err` (`checks_borrows_resources.rs:7078-7080`).

**Verdict:** CLOSED — for `BorrowLoanLineage::SuppliedOutput` loans that reach `install`, a `source_type` mismatch cannot pass checking; `None` becomes a diagnostic.

## What I did not read

- Full extent/index check fan-out after unfixed `FixedArrayLength::Expression` (corpus samples, `checks_ranges.rs`, all `resolved_array_extent` callers).
- Complete constant-recipe evaluation paths for `ArrayLength` positions outside `constant_entries::materialize`.
- All stage-05 selection finalization branches beyond `finalize_checked_authored_selections` and `descriptor_selection_occurrences`.
- Static proposal application (`apply_static_application_type` and downstream) after wrong `infer_static_bindings` proposals.
- Full borrow-overlap checking with intentionally empty rosters from A0005/A0006.
- `CallFrameTables::build` failure conditions and frequency in successful compilations.
- Entire `source_type` function past line 2616 and non-`install` consumers in tests only (if any).

SLICE COMPLETE
