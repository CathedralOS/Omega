# slice_0009

| id | file:line | verdict | one-line reason |
|----|-----------|---------|-----------------|
| A0065 | checks_current_scalar_values.rs:604-610, :1994 | REACHABLE | Named type-ref spellings parse as integers; struct field selection can match by name alone (read). |
| A0066 | checks_multiplicity.rs:7218, :7518 | REACHABLE | Non-statement/call permission sources use statement index 0 for type and write ordering (read). |
| A0067 | checks_multiplicity.rs:9802-9937 | REACHABLE | Unrecognized call-argument shapes exit `temporary_owners` without recording a temporary owner (read). |
| A0068 | checks_multiplicity.rs:10872 | REACHABLE | Default `true` on unlisted value shapes sets `obligation_live` on linear establishments (read). |
| A0069 | checks_multiplicity_borrowed_windows.rs:1498-1513 | REACHABLE | Enclosing-target store discharges open window via `storage_path_contains` before exact-type repair (read). |
| A0070 | checks_multiplicity_borrowed_windows.rs:565-567, :960-962 | REACHABLE | Expression roots and flow-only suspend/boundary flags skip borrowed-window enforcement (read). |
| A0071 | checks_multiplicity_claim_outcomes.rs:2167-2187 | REACHABLE | Unknown provenance with no derived receipt is treated valid in join validation (read). |
| A0072 | checks_multiplicity_cleanup_dispositions.rs:822-833, :722-727 | REACHABLE | No storage frontier makes `is_present` true everywhere; join can drop a one-sided frontier (read). |

## A0065

Digest (area_08, preamble read): scalar static arguments and field selection under stage-05 ownership/scalar checks.

**Site 604-610 (`static_argument_value`).** read `checks_current_scalar_values.rs:574-643`. For a `StaticMachineArgument` with `type_reference` `Named { symbol: invalid, name }`, the function parses `name` as `i128` and returns `ScopedArithmeticValue::Integer` (`604-610`) without requiring a real symbol or declaration.

**Callers.** Search `\bstatic_argument_value\b` in `omega-rust`: 6 matches — 1 definition, 1 call in `capture_generic_actuals` at `checks_current_scalar_values.rs:567`, 1 in `checks.rs:2352` (stored-integer call checking), remainder are tokenizer module names in stage 01 (not this function).

**Upstream.** `static_argument_value` is reached from scalar contract/call actual capture during checked-tree construction (`checks.rs` multiplicity/check pipeline uses `CheckValues`).

**Verdict trace.** A static machine argument can carry a type reference whose name is digits (e.g. `"42"`) with no valid symbol; the checker treats it as the integer 42 for contract/scalar reasoning instead of rejecting an ill-formed reference. REACHABLE: plain words — a program whose static argument was meant to name a type but is spelled like a number gets numeric scalar facts.

**Site 1994 (`selects_field`).** read `checks_current_scalar_values.rs:1986-1995`. When `member.member_symbol` is invalid, equality is `program.symbols.name(field) == member.member.as_str()` with no struct/type filter.

**Related (digest wording).** read `checks_multiplicity.rs:10833-10850` inside `expression_establishes_obligation`: struct-literal field lookup uses `field.name.as_str() == field_name` only. Two different struct types sharing a field name could match the same literal field for obligation paths.

**Callers `selects_field`.** Search `\bselects_field\b` in `omega-rust`: 6 matches — 3 in `checks_current_scalar_values.rs` (def + `captured_field_lengths`), 3 in `borrow.rs` (separate closure).

## A0066

read `checks_multiplicity.rs:7064-7234` (`SelectedCleanup::observe`) and `:7408-7538` (`SelectedCleanup::finish`).

At `7213-7218`, `statement` for `validation::canonical_place_type_reference` is `0` when `event.source` is not `Statement` or `Call` (e.g. `StateExit`, join sources). Same default at `7513-7518` for `captured_at` when filtering establishment-ordered roots and receiving writes (`7520-7538` uses statement `0` or skips non-statement/call writes).

**Callers `observe`.** Search `selected_cleanup.observe` in `omega-rust`: 1 production call `checks_multiplicity_permission_events.rs:1199` (linear permission replay). Also internal `RemainingCustody::observe` at `7564` inside `finish`.

**Callers `finish`.** Search `selected_cleanup.finish` in `omega-rust`: 1 call `checks_multiplicity_permission_events.rs:1266`.

**Input path.** `validate_linear_permission_events` → `checks_multiplicity_permission_events.rs` builds `SelectedCleanup` over flow permission events for each state cleanup edge.

**Verdict trace.** A permission event whose source is not statement- or call-indexed still runs place typing and write-order queries at statement index 0. REACHABLE: plain words — a selected-cleanup transfer attributed to a state exit or other non-statement source uses types and establishments as if at the first statement.

## A0067

read `checks_multiplicity.rs:9762-9954` (`temporary_owners` in `mod owners` / `temporary_results`).

The loop over call parameters (`9822-9954`) only pushes `TemporaryOwner` when `owned_temporary_type`, `owned_projection`, or a shared/mutable borrow path succeeds; otherwise `continue` (`9846-9851`, `9873-9874`, `9912-9913`, `9922-9936`). Unrecognized argument shapes never append an owner.

**Callers.** Search `\btemporary_owners\b` in `omega-rust`: 30 matches — definition/export in `checks_multiplicity.rs`; primary call `temporary_results::temporary_owners(...)` at `checks_multiplicity_permission_events.rs:1384-1385`; many parameter passes in `checks_multiplicity_linear_validation.rs`.

**Downstream.** read `checks_multiplicity_permission_events.rs:912-972`: prepared temporaries feed linear validation replay; empty `temporary_owners` means no temporary consumption/disposal obligation recorded for that call pattern.

**Verdict trace.** REACHABLE: plain words — a call passes a value the temporary classifier does not recognize (e.g. falls through all branches with `continue`), compilation proceeds with no `TemporaryOwner` row and thus no cleanup/disposal obligation from this function.

## A0068

read `checks_multiplicity.rs:10781-10873` (`expression_establishes_obligation`). After place-at, array literal, and struct literal arms, unlisted expression nodes fall through to `return true` (`10872`).

**Callers.** Search `\bexpression_establishes_obligation\b` in `omega-rust`: 5 matches — definition plus two recursive calls in the same function; production use in `checks_multiplicity_linear_validation.rs:1557-1565` (`written_owner_targets`) setting `WrittenLinearTarget.obligation_live`.

**Upstream.** read `establish_targets` at `checks_multiplicity_linear_validation.rs:3862-3967`: `obligation_live` from the target drives `claim_identities.mint` (`3865-3868`) and `FlowPermissionEventFact.obligation_live` on `Establish` (`3967`).

**Verdict trace.** A linear write whose completing value is an expression shape not handled by the match (not place-at, array index, struct field, or enum variant name) gets `obligation_live: true` on establishment. REACHABLE: plain words — assigning into a tracked linear place from such a value marks the linear obligation live without the function proving the value established it.

## A0069

read `checks_multiplicity_borrowed_windows.rs:1463-1528` (`BorrowedStorageWindows::check_repair`). Comment at `1458-1461` (read): store to absent place or whole enclosing owner discharges when types match exactly on the equal-length path.

At `1498-1500`, when `storage_path_contains(program, captures, &target_path, &absent.path)`, the open window is removed (`repaired.push`) with no diagnostic before the equal-length type check at `1511-1527`.

read `storage_path_contains` at `1566-1576`: true when compatibility is `Same` or `LeftContainsRight`.

**Callers `check_repair`.** Search `\bcheck_repair\b` in `omega-rust`: 2 matches — definition; 1 call `checks_multiplicity.rs:2815` during statement walk with `StorageWalk`.

**Verdict trace.** Storing to an enclosing owner path that contains the absent borrowed subtree clears the window without the exact-type test that runs only when paths are same length after removal. REACHABLE: plain words — whole-owner assignment (`owner = other_value`) of the wrong type can still clear the borrowed-storage hole when paths are in containments relation.

## A0070

read `checks_multiplicity_borrowed_windows.rs:553-576` (`check_edge_use`): returns early when `arm_of_expression` lacks the expression (`565-567`) or root is not a symbol (`571-572`).

read `:1356-1370` (`refuse_move_over_open`), `:1422-1435` (`check_use`), `:1476-1481` (`check_repair`): each returns when `target.root.as_symbol()` / `use_place.root.as_symbol()` / `event.root.as_symbol()` is `None`.

read `:906-961` (`check_recorded_invocations`): at `960-962`, skips invocation diagnostics when neither `may_suspend` nor `may_enter_boundary` from flow facts.

**Callers.** `check_use` / `check_edge_use`: `checks_multiplicity.rs:2618-2619` in occurrence walk. `check_recorded_invocations`: `checks_multiplicity.rs:2518` from `check_linear_obligations`. `record_invocation`: `checks_multiplicity.rs:2681`.

**Verdict trace.** Uses and repairs rooted at expression handles (not symbol roots) never consult open borrowed windows; recorded window invocations are only checked when flow marks suspend/block/boundary. REACHABLE: plain words — a use or store through a call-result or other expression-rooted place, or a suspension not flagged in flow, can miss borrowed-window errors.

## A0071

read `checks_multiplicity_claim_outcomes.rs:2064-2197` (`validate_call_claim_joins`). `derive_receipts` (`1646-1869`, read at cites) returns no receipt when state lookup fails (`1678-1679`), call node missing (`1681-1682`), target state missing (`1684-1685`), call site not found (`1694-1705`), frontier incomplete (`1839-1840`), or identity/source mismatch (`1851-1855`).

At `2167-2186`, `invalid_event` sets `valid` for non-`Joined` provenance to “no expected receipt shares `claim_identity`” (`2181-2184`); events with `PermissionProvenance::Unknown` and no receipt therefore have `valid == true` and are not reported.

**Callers.** Search `\bvalidate_call_claim_joins\b` in `omega-rust`: 3 matches — definition, re-export, 1 call `checks_multiplicity_linear_validation.rs:4199` after permission replay.

**Verdict trace.** REACHABLE: plain words — a permission event with unknown provenance and no row in `derive_receipts` output passes the final invalid-event scan and does not add the “no matching claim origin” diagnostic.

## A0072

read `checks_multiplicity_cleanup_dispositions.rs:670-750` (`RemainingCustody::join`). At `722-727`, `storage_frontiers` retains only roots present on both sides; a one-sided frontier is dropped.

read `:822-833` (`is_present`): if no `storage_frontiers` entry for `root`, `find(...).is_none_or(|(_, frontier)| frontier.contains(path))` is true for every path (no exclusion/inactive-case hit required beyond prefixes).

read `:786-798` (`completes_with`): root with no frontier returns `true` (“already whole”).

**Callers `is_present`.** Defined in `checks_multiplicity_cleanup_dispositions.rs`; read use in `checks_multiplicity.rs` (e.g. `7272`, `7538`, `8486`), `checks_multiplicity_linear_validation.rs` (`2099`, `2527-2528`).

**Verdict trace.** JOIN can erase a construction frontier present on only one branch; afterward `is_present` treats that root as fully present at all paths not explicitly excluded. REACHABLE: plain words — after a one-sided join, custody can treat storage as present and complete when one branch still had an incomplete frontier (reader note: over-disposal direction).

## What I did not read

- Full enumeration of every `obligation_live` consumer outside `establish_targets` and execution cleanup (inferred only via grep counts for A0068).
- Stage-04/06 modules that mention unrelated `SelectedCleanup` types (not on the `checks_multiplicity::selected_cleanup` path).

SLICE COMPLETE
