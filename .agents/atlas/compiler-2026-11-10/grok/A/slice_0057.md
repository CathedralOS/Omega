# slice_0057

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0433 | dp:7976-7987 | REACHABLE | Field store with no matching fact name and non-establishment field type exits before predicate loop while still mutating tracked place state. |
| A0434 | dp:8005-8049 | REACHABLE | Established-place filter can drop domain requirements when disjoint-from-store or fact_names_field is false. |
| A0435 | dp:8982-9019 | REACHABLE | validate_qualified_place_read returns silently on six guards; only the surviving path emits the invariant-window diagnostic. |
| A0436 | dp:8966 | REACHABLE | scan_expression_use `_ => {}` skips child read scan for unlisted expression node kinds. |
| A0437 | dp:9939-9941 | REACHABLE | declared_root_established treats every state parameter symbol as established with no type qualification check. |
| A0438 | dp:11400 | REACHABLE | Unresolved transition targets omit graph edges so successor states keep looser entry-established data. |
| A0439 | dp:11807-11812 | REACHABLE | Case membership falls back to last path-segment spelling equality when resolved symbol differs from active case. |
| A0440 | dp:12902-12904, dp:13231-13233 | REACHABLE | All-true parameter_call_attempts suppress machine diagnostics even when callee re-walk proved empty via skipped/untracked paths. |

## A0433

Digest claim (read): at `dp:7976-7987`, a store to a field that no domain fact names returns before any invariant check.

Code (read): In `handle_field_store` (`domains_and_predicates.rs:7776`), after updating `place.fields` and related tracked state (`7883-7904`), the guard at `7976-7986` returns immediately when no `domain_requirements` fact names the stored field (`fact_names_field`, `10278-10289`) and the selected field type does not `type_requires_establishment` (`7983-7985`). That return skips the requirement filter (`8002-8049`), the proof loop (`8064-8146`), and establishment/window updates (`8147-8160`).

Callers (read): `rg '\bhandle_field_store\b' omega-rust` → **2** matches (definition `7776`, sole production call `7560` inside `handle_assignment` `7355`).

Acceptance path (read): `handle_assignment` ← `walk_state` (`13326`) ← `analyze_default_domain_writes_with_frames` (`12595`) ← `validate_default_domain_writes` (`12582`) ← `judge_contract_entailment` (`program_validation.rs:4675`) ← `validate_program` (`3946-3958`). `finish_diagnostics` (`4683`) rejects when diagnostics are non-empty.

Input that reaches the branch (read): A machine state assignment lowered to one or more field stores via `field_stores`/`CallFrameResolver`, where the written field’s name is absent from every predicate read set for pending `where` facts but the field’s type reference does not require establishment.

Smallest program (plain words): A default-domain record with an open invariant window and a pending `where` fact that only mentions another field; an assignment writes a scalar aux field not referenced by any fact name; validation updates tracked fields then returns without re-proving requirements, so a later whole-value read in the same state is not rejected for the stale window.

## A0434

Digest claim (read): `dp:8005-8049` drops requirements judged disjoint from the store.

Code (read): After `pending_requirements` case filtering (`7996-8001`), `requirements.into_iter().filter` (`8002-8048`) keeps all requirements when `!place.established || place.pending_requirements.contains(requirement)` (`8005-8006`). Otherwise it drops when `!fact_names_field` (`8008-8014`), when `fact_is_disjoint_from_store` is true (`8016-8035`), or when `data_fact_applies_to_field` is `Some(false)` (`8040-8046`). Dropped facts never enter the proof loop (`8064`).

Callers (read): Same `handle_field_store` chain as A0433; `fact_is_disjoint_from_store` defined at `10326-10340` (read).

Acceptance path (read): Same as A0433 through `judge_contract_entailment`.

Input (read): A tracked place already `established` with no pending entry for a given requirement, during a field store whose destination path is classified disjoint from that fact’s predicate read paths (`complete_paths` true and no overlap).

Smallest program (plain words): An established aggregate with a whole-value fact and a separate scalar field; a partial field write disjoint from the fact’s read paths causes the filter to drop the whole-value requirement; the store completes without folding that fact and the place can stay acceptable for later reads.

## A0435

Digest claim (read): `validate_qualified_place_read` has six silent returns (`8982-9019`).

Code (read): `scan_expression_use` (`8667`) always calls `validate_qualified_place_read` first (`8714-8725`). Silent returns: non-place-root expression kinds (`8982-8989`), `declared_place_type_raw` failure (`8991-8997`), type does not require establishment (`8999-9000`), non-consuming read of non-constrained type (`9002-9008`), named/generic with resolvable data definition (`9010-9015`), missing `self_place_spelling` (`9017-9018`). Only if none fire does `9065-9069` push the open-window diagnostic.

Callers (read): `rg '\bvalidate_qualified_place_read\b' omega-rust` → **2** matches (definition `8970`, call `8714`).

Acceptance path (read): `scan_expression_reads` (`8642`) ← `scan_statement_reads` (`8554`) ← `walk_state` (`13359`) ← same default-domain pipeline as A0433.

Input (read): An expression use during statement read scanning where the outer node hits one of the six early returns but a nested qualified read that should fail is not reached because a parent arm does not recurse (see A0436) or the guard misclassifies the place.

Smallest program (plain words): A constrained default-domain binding read that is wrapped in an expression node kind the validator treats as non-place at the top of `scan_expression_use`, so the qualified-read check exits without diagnostic while the program still performs the bad read elsewhere without an alternate gate on that path.

## A0436

Digest claim (read): `dp:8966` `_ => {}` — reads inside unlisted node kinds are not scanned.

Code (read): In `scan_expression_use`, the `match` on `expression_table(program).expression(expression)` handles `Name`, `Member`, `Indexed`, `Borrow`, `Call`, `Cast`, `Unary`, `Atomic`, `Range`, `ArrayLiteral`, `StructLiteral`, `Match` (`8787-8965`); `_ => {}` at `8966` performs no `read_child!` / `validate_read!` for other kinds.

Callers (read): `rg '\bscan_expression_use\b' omega-rust` → **4** matches (definition `8667`, calls `8653`, `8862`, `8877`).

Acceptance path (read): Same read-scan path as A0435 via `scan_expression_reads` → `walk_state`.

Input (read): A statement read list includes a subexpression whose root `ExpressionNode` variant is not in the enumerated arms; `consumes_value` traversal does not descend.

Smallest program (plain words): A statement evaluates a whole-value read of default-domain data hidden under an expression node kind omitted from the match (for example a proof or opaque wrapper not listed at `8787-8965`), so the invariant-window scanner never visits the inner place read.

## A0437

Digest claim (read): `dp:9939-9941` — every state parameter counts as established whatever its type.

Code (read): `declared_root_established` (`9931`) returns `true` when `program.state_parameter_by_symbol(state, root).is_some()` (`9939-9940`) before checking const/value parameters or locals (`9942-9958`).

Callers (read): `rg '\bdeclared_root_established\b' omega-rust` → **6** matches, all in `domains_and_predicates.rs`: imports/usages `7282`, `8199`, `8539`, `9039`, `9145`, definition `9931`.

Acceptance path (read): Used in `validate_qualified_place_read` (`9039-9043`) and `validate_data_read` (`9145`) on the `walk_state` read path; program acceptance still via `judge_contract_entailment` diagnostic aggregation.

Input (read): A read of a state parameter root symbol where `type_requires_establishment` is true for the parameter’s type but the symbol is a state parameter.

Smallest program (plain words): A state parameter typed as default-domain qualified data is read before any write establishes it; the root is treated as already established because it is a parameter, so the open-window diagnostic is not emitted for that read.

## A0438

Digest claim (read): `dp:11400` — unresolved transition targets add no edge; missing predecessor makes the target look more established.

Code (read): `state_edges` (`11373`) pushes `(from, to)` only when `to` resolves (`11387-11401`); unresolved named targets contribute nothing. `analyze_default_domain_writes_with_frames` builds `predecessors` from these edges (`12754`, `12650-12651`). For state index `> 0`, when `predecessors[index].is_empty()`, the fixpoint skips updating `entry_established[index]` (`12809-12812`) and summary path skips predecessor meet (`12681-12684`), leaving default empty entry sets.

Callers (read): `rg '\bstate_edges\b' omega-rust/psi/compilation/validation` → **3** matches (`11373`, `12650`, `12754`); distinct `state_edges` module exists in `pipeline/05_typed-trees-to-checked-trees` (not read).

Acceptance path (read): Edge set feeds multi-state default-domain analysis that drives `walk_state` entry-established sets; failures surface as missing diagnostics in `judge_contract_entailment`.

Input (read): A machine with multiple states where a transition target name does not resolve to a state index (symbol/name match fails at `11387-11398`).

Smallest program (plain words): Two states where a transition points at a misspelled or foreign state name so no CFG edge is recorded; the destination state’s entry-established places stay empty rather than intersecting predecessor exits, so reads there are treated as less constrained than they should be.

## A0439

Digest claim (read): `dp:11807-11812` — a case path resolving to a different variant with the same spelling evaluates true.

Code (read): `case_membership_value` (`11797`) returns `Some(1)` when `path.resolved_symbol() == Some(active)` (`11804-11805`). Otherwise it compares the last path member spelling to `program.symbols.name(active)` (`11807-11812`).

Callers (read): `rg '\bcase_membership_value\b' omega-rust` → **2** matches (call inside `fold` at `11666`, definition `11797`).

Acceptance path (read): `fold` feeds store-time predicate proof in `handle_field_store` (`8081-8090`) and symbolic `predicate_holds` (`11481`); false positives let requirements clear and places become `established` (`8147-8149`).

Input (read): `active_case` is one variant symbol; case-membership expression’s resolved symbol differs but last segment text equals the active variant’s name string.

Smallest program (plain words): Two variants with the same authored name in different enclosing types or namespaces; a case-membership test against the active variant’s symbol resolves to the wrong variant handle but the same spelling, so the folded predicate is treated as true and establishment proceeds.

## A0440

Digest claim (read): `dp:12902-12904` with `dp:13231-13233` — callee leaving an invariant window open on its parameter, first call tracked/re-proved, second call untracked.

Code (read): After parallel machine walks, `machine_results` diagnostics are merged unless for that machine symbol every `parameter_call_attempts` entry is `proved == true` (`12893-12906`), in which case `continue` skips attaching `machine_diagnostics`. `restore_parameter_write` (`13180`) returns early without recording an attempt when `entry_valuations.is_empty()` (`13231-13232`), or records `(machine.symbol, false)` when call frames or recursion guards fail (`13205-13209`, `13234-13238`). When a re-walk runs, `proved = callee_diagnostics.is_empty()` (`13263-13267`).

Callers (read): `rg '\brestore_parameter_write\b' omega-rust/psi/compilation/validation/src/domains_and_predicates.rs` → **5** matches (definition `13180`, calls `13476`, `13515`, `13556`, `13671` inside `walk_state` call handling).

Acceptance path (read): `parameter_call_attempts` populated during `walk_state` calls; suppression at `12902-12904` prevents surfacing callee diagnostics in `validate_default_domain_writes`; `validate_program` accepts if aggregated diagnostics are empty.

Input (read): Same callee machine invoked twice from parameter call sites; first invocation’s re-walk yields empty `callee_diagnostics` (attempt pushed `true`); second passes arguments that do not yield `entry_valuations` (`13218-13222` continue) or otherwise skip a failing re-walk while first attempt stays `true`.

Smallest program (plain words): A callee machine leaves its parameter’s default-domain window open; the caller invokes it once with a tracked binding the re-walk accepts as proved, then again with an untracked or non-spelled argument so no second proof is recorded, and the merge step drops all callee diagnostics because the first attempt was marked proved.

## What I did not read

- Full bodies of `walk_state` beyond call sites and statement loop head (`13326-13380+`).
- `predicate_read_set` and complete `fold`/`symbolic_values::predicate_holds` implementations.
- Whether any stage outside `judge_contract_entailment` duplicates these default-domain checks (`inferred` from `validate_program` structure only).
- `ExpressionNode` enum full variant list to name a concrete unlisted kind for A0436 (`inferred` that omitted kinds exist from non-exhaustive match).
- Historical line alignment at commit `457adf4a11f` (re-located by symbol name in current checkout; read).

SLICE COMPLETE

