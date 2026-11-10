# slice_0012

Area digest preamble (read): `.agents/atlas/compiler-2026-11-10/digests/area_09.md` — stage 05 execution plans (`ESC`, `ETU`, `CALLS`, `CLEAN`, `CTRL`, `SEQ`, `SG`, `SSS`, `TYPES`); builders mostly return `Option` (plan gap vs no row). Spelling-keyed sites grouped under execution plans; words open / stated / traced / suspected. Citations at commit `457adf4a11f`; current checkout lines match cited excerpts for these rows (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0085 | execution_scalar_computations.rs:8329-8330 | REACHABLE | Equal generic `base_name` without equal symbols can make `structural_carriers_match` true at call/case planning sites. |
| A0086 | execution_scalar_computations.rs:7119-7129 | REACHABLE | After eight reference peels the loop stops but `data_application_value_fields` still runs on the not-fully-unwrapped type. |
| A0087 | execution_scalar_computations.rs:10538-10543 | REACHABLE | `let _ = record_completion` ignores failure while refusal rows are kept; no stage-05 check turns that into rejection (state `continue` at 10543 is unreachable given `occupies_scalar_position`). |
| A0088 | execution_scalar_computations.rs:6697-6733 | REACHABLE | Failed or empty `descriptor_retirement` yields empty `normal_cleanup_actions` but `insert` still publishes the observation computation. |
| A0089 | execution_terminal_unit.rs:1212-1240 | REACHABLE | Sixteen-step cap returns a partly rebased place; callers use that place for stores and windows without retry. |
| A0090 | execution_terminal_unit.rs:2359-2364 | REACHABLE | `Dynamic` / `ActivationStorage` / `DescriptorTable` call routes record no dependency edge and do not drop the caller. |
| A0091 | execution_terminal_unit.rs:3374-3380 | REACHABLE | `aliases(...).unwrap_or_default()` and `resolve(...).unwrap_or(place)` keep the alias-local place when lookup or chain resolution fails. |
| A0092 | execution_terminal_unit.rs:6403 | REACHABLE | `value_kind` `_ => {}` skips transfer collection for `Scalar`, `QuotientCall`, `QuotientAdaptation`, `ZeroedStructural`, and `ByteLiteralView` roots. |

## A0085

**Cited site (read):** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_scalar_computations.rs:8329-8330` and `8348-8349` — in `value_predicates::structural_carriers_match`, two `Generic` nodes match when symbols differ but `base_name` strings are equal; `Generic`/`Named` pairs use the same name fallback after identity check fails (`8308-8311`).

**Enclosing function (read):** `structural_carriers_match` (`execution_scalar_computations.rs:8297-8353`).

**Callers (search):** pattern `\bstructural_carriers_match\b` in `omega-rust` — count **5**, all in `execution_scalar_computations.rs`: definition `8297`; import/use `7184`; sites `7514`, `9274`, `9299` (read).

**Upward trace (read):** `structural_case_value` (`7503-7516`) returns `None` when match fails; on success builds case fields. `Builder::structural_value` ladder (`8945+`) reuses roots only when `normalized_type_identity` matches (`8958-8961`); call-result arm (`9264-9288`) requires `structural_carriers_match` **and** `call_result_types.admits` (`9274-9278`). Case-construction arm (`9298-9305`) gates on match only. Caller: `build_checked_value_computation_plans` (`10508+`) via `published_facts.rs:11186` inside `finish_check_facts` (`11148+`); no diagnostic in that path when a match arm succeeds (read `published_facts.rs:11186-11196`, `11232+`).

**Input that reaches the branch:** A call result or structural case whose returned/constructor carrier and expected carrier are different type identities (and generic symbols differ) but share the same spelled generic base name.

**Smallest program (plain words):** Two distinct generic data types that share only the same type name text; a call result or case value is planned as matching the expected carrier because the name strings agree, though identities differ.

## A0086

**Cited site (read):** `execution_scalar_computations.rs:7119-7129` — `descriptor_fields` increments `visited`, breaks after `visited > 8`, and each iteration peels one `reference_shell` before calling `program.data_application_value_fields(reference)` (`7130`).

**Enclosing function (read):** `descriptor_fields` (`7111-7131`); module ends `7132`.

**Callers (search):** pattern `\bdescriptor_fields\b` in `omega-rust` — count **3**, all in `execution_scalar_computations.rs`: definition `7111`; calls in `descriptor_parameter_symbol` at `7066` and `7096` (read).

**Upward trace (read):** `descriptor_parameter_symbol` (`7084-7105`) feeds `Builder::structural_field` (`6936+`) when resolving value-parameter descriptors vs dense scalar parameters (`6949-6962`); failure falls through to other receiver paths, not a compile diagnostic. `build_checked_value_computation_plans` constructs the builder (`10553+`).

**Input that reaches the branch:** A borrowed carrier type with more than eight nested reference shells on the path to descriptor value-parameter fields.

**Smallest program (plain words):** A descriptor-typed field spelled with nine or more nested reference wrappers; field metadata is read from the partly wrapped type, so the wrong parameter symbol may be chosen for a scalar descriptor read.

## A0087

**Cited sites (read):** `execution_scalar_computations.rs:10538-10543` — `continue` when `parameter_types` `collect` is `None`; `10590` / `10597` — `let _ = builder.record_completion(...)` discards the `Option` return.

**Enclosing function (read):** `build_checked_value_computation_plans` (`10508-10705+` read through statement loop and unit-return completion block).

**Callers (search):** pattern `\bbuild_checked_value_computation_plans\b` in `omega-rust` — count **2**: definition `10508`, call `published_facts.rs:11186` (read).

**Upward trace (read):** Scalar parameters are pre-filtered with `occupies_scalar_position` (`10531-10535`), which already requires `primitive_type_reference(...).is_some()` (`values_scalar.rs:4324-4331`); therefore the `continue` at `10543` is **unreachable** for filtered parameters (read). `record_completion` (`1881-1906`) always pushes a `CheckedCompletionRoot` and may set `refusal`; return is `None` when `refusal.is_some()`. Downstream, `SelectedInvocationAccounting::new` returns `None` when `root.refusal.is_some()` (`execution_terminal_unit_control.rs:3171-3173`) but `finish_check_facts` does not treat completion refusals as `Err` in the excerpt read (`published_facts.rs:11148+`). Digest note on “non-primitive scalar parameter” does not match the current `continue` guard (read).

**Input that reaches the branch:** A unit-returning state whose terminal `match` completion planning fails (`record_completion` returns `None` with `refusal` set).

**Smallest program (plain words):** A machine state that returns unit and ends with a `match` the completion planner refuses; compilation still records the refusal row and continues building other computation plans for that state.

## A0088

**Cited site (read):** `execution_scalar_computations.rs:6697-6733` — `observation_completion_actions` returns `arena::HandleSpan::empty()` when receiver checks fail, when canonical place is not a bare expression root, when the reference is not readable, or when `descriptor_retirement` returns `None` (`6725-6730`).

**Enclosing function (read):** `observation_completion_actions` (`6697-6733`).

**Callers (search):** pattern `\bobservation_completion_actions\b` in `omega-rust` — count **2**: definition `6697`, call in `Builder::insert` at `9760` (read).

**Upward trace (read):** `insert` (`9724-9768`) always appends `CheckedScalarComputation` with `normal_cleanup_actions` from `observation_completion_actions`, even when empty. `descriptor_retirement` (`execution_terminal_unit_cleanup.rs:1527-1553`) returns `None` for `Multiplicity::Linear` (`1551-1552`); unrestricted returns empty vec (`1534-1534`). Observations published via `completed_scalar_observation` / `primitive_reference_observation` (`6677-6688`, `6741+` read names only).

**Input that reaches the branch:** A fresh scalar observation whose receiver is a linear or otherwise non-retiring reference descriptor, or whose retirement lookup fails.

**Smallest program (plain words):** A scalar field read through a fresh linear reference carrier; the computation plan is recorded with no normal cleanup actions even though the descriptor carrier may still need retirement.

## A0089

**Cited site (read):** `execution_terminal_unit.rs:1212-1240` — `resolve_storage_place` loops at most 16 times (`1212`), then returns `(root, segments)` even if further rebasing would be possible.

**Enclosing function (read):** `borrowed_windows::resolve_storage_place` (`1202-1241`).

**Callers (search):** pattern `\bresolve_storage_place\b` in `omega-rust` — count **15**: definition `execution_terminal_unit.rs:1202`; calls `execution_terminal_unit.rs:998`, `1108`, `1592`; `execution_terminal_unit_cleanup.rs:2905`; plus **10** method calls on a walk struct in `checks_multiplicity_borrowed_windows.rs` (read grep listing; multiplicity copy at `1721-1750` shares the same 16 bound).

**Upward trace (read):** Window / assignment planning rebases mutable access roots through `resolve_storage_place` before building `CanonicalPlace` (`998-1006` read). Cleanup path uses the same helper (`execution_terminal_unit_cleanup.rs:2905` name only). No fallback loop after the cap (read `1202-1240`).

**Input that reaches the branch:** A chain of seventeen or more reference-copy or reassignment steps from a local root before the use site.

**Smallest program (plain words):** Seventeen `let s = r`-style aliases in one state; the planner stops on an intermediate local and treats it as the storage root for a later write or window.

## A0090

**Cited site (read):** `execution_terminal_unit.rs:2359-2364` — in `admit_call_target`, the match arm for `CheckedUnitCallRoute::Dynamic { .. } | ActivationStorage { .. } | DescriptorTable { .. }` with any result is empty (`=> {}`).

**Enclosing function (read):** `admit_call_target` (`2319-2366`).

**Callers (search):** pattern `\badmit_call_target\b` in `omega-rust` — count **3**: definition `2319`; calls in `retain_available` at `2453` and `2562` (read).

**Upward trace (read):** `retain_available` (`2400-2586+`) walks each retained machine’s operations and prepared calls; `Module` routes call `require_entry` or scalar verdict (`2342-2357`); boundary mismatches `drop_candidate` (`2330-2339`). Closure ends with `close` (`2247-2285`) propagating dropped callees. Invoked from unit plan build at `execution_terminal_unit.rs:7428` (read). `OperationPlan::Call` with dynamic routes still appears in plans from earlier stages (e.g. `execution_terminal_unit_calls.rs:3738` name only).

**Input that reaches the branch:** A machine plan that calls through a dynamic, activation-storage, or descriptor-table route while the callee body has no retained entry plan.

**Smallest program (plain words):** A body that calls via a dynamic route to an implementation the catalog never retained; the caller stays in the plan set because no dependency edge or drop is recorded for that call.

## A0091

**Cited sites (read):** `execution_terminal_unit.rs:3374-3380` — `aliases` returns `None` when `state_flow` or `first_state_borrow` fails; `Some(Vec::new())` when no loans (`3378-3379`). `formation` keeps `source_root` when no parent alias (`3344-3350`). `resolve` (`3421-3445`) breaks on missing alias. `execution_terminal_unit.rs:500-501` — `aliases(...).unwrap_or_default()` and alias rewrite on atomic place. `execution_terminal_unit_structural_scalar_store.rs:1006-1009` — same `unwrap_or_default` / `resolve(...).unwrap_or(place)` on assignment destination (read).

**Enclosing functions (read):** `receiver_aliases::aliases` (`3368-3403`), `formation` (`3266-3360`), `resolve` (`3421-3445`); `atomic_access::place` (`480+` read through `511`); `SSS::resolve` store path (`956-1009`).

**Callers (search):** `receiver_aliases::aliases` / module — `execution_terminal_unit_structural_scalar_store.rs:1`, `execution_terminal_unit_calls.rs:1`, `execution_terminal_unit.rs:2` import/use (grep count for string); atomic `place` and store `resolve` are production consumers (read).

**Upward trace (read):** Failed `aliases` becomes empty list, not rejection (`500-501`, `1006-1007`). Failed `resolve` keeps authored place (`1008-1009`). `formation` without parent alias still emits `ReceiverAlias` rooted at `source_root` (`3350-3358`). Call-site alias path uses `resolve` with `unwrap_or(&authored_place)` (`execution_terminal_unit_calls.rs:6522-6527` read). No diagnostic emitted at these sites (read).

**Input that reaches the branch:** An atomic access or store through an erased `&mut` alias local whose alias list is empty or whose chain is not found in `resolve`.

**Smallest program (plain words):** A store or atomic operation authored through a borrow-alias local; alias collection fails or misses the parent link, so planning targets the alias local instead of the rebased referent storage.

## A0092

**Cited site (read):** `execution_terminal_unit.rs:6403` — `_ => {}` in `value_kind` after arms for `Place`, `Call`, `Projection`, records/cases, arrays, dispatch, scalar arrays, references, slice views, etc. (`6141-6402` read).

**Enclosing function (read):** `cleanup::Query::value_kind` (`6130-6406`); called from `value` (`5982-6012`).

**Kinds not handled (read match):** No arms for `CheckedStructuralValueKind::Scalar`, `QuotientCall`, `QuotientAdaptation`, `ZeroedStructural`, `ByteLiteralView` (contrast `execution_terminal_unit_control.rs:3767-3770` which explicitly lists some of these elsewhere). `QuotientAdaptation` at call site may still require `Unrestricted` multiplicity (`execution_scalar_computations.rs:4995-4998` read).

**Callers (search):** `value_kind` — 1 definition in `execution_terminal_unit.rs`; invoked only via `value` in the same `Query` impl (read). `value` used from `argument_transfers` (`5945+`) and cleanup transfer planning (`query_for_storage` / `through_prefix` at `5567+` read).

**Upward trace (read):** Unmatched kinds return `Some(output)` with default empty `Transfers` (`6140`, `6405`); missing owned-subtree moves are not recorded. Earlier validation of quotient arms noted at ESC `4995-4996` (read); no transfer replay for `QuotientCall` arguments in this function (read `6403`).

**Input that reaches the branch:** A structural value root whose kind is `Scalar`, `QuotientCall`, `QuotientAdaptation`, `ZeroedStructural`, or `ByteLiteralView` when cleanup transfer query walks that node with selection.

**Smallest program (plain words):** A call argument or completion cleanup walk rooted at a quotient call or zeroed structural value that moves an owned subtree; the transfer graph records no moves from that root.

## What I did not read

- Full `omega-rust/psi/compilation/validation/` typing rules for generic/name identity (whether mismatched generics can enter `CheckingInput`).
- Most of `checks.rs` and other stage-05 check drivers for completion refusals, alias failures, or transfer completeness.
- Stage-06 lowering behavior when plans omit cleanups or transfers.
- `git show 457adf4a11f:...` line-by-line diff (current lines read directly).
- `psi/foundation/numerics` `value_kind` (unrelated namesake).

SLICE COMPLETE
