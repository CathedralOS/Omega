# slice_0088

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0672 | current_ownership.rs:204 | UNDECIDED | Empty effects when `graph` is `None` is read; no unit that passes earlier validation yet fails only via descriptor loans was traced. |
| A0673 | current_ownership.rs:553-562 | UNDECIDED | Longest-prefix `access_location` fallback is read; whether it lets a disturbed access pass `authorizes` was not decided. |
| A0674 | current_ownership.rs:1069-1073 | REACHABLE | Return clears `moved` in descriptor-loan conflict and `check_operation` runs before `ReturnStructural` moves its source. |
| A0675 | current_ownership.rs:1160 | CLOSED | `conflicted_edge` zips bindings to parameters, but edge length is enforced before ownership replay. |
| A0676 | current_ownership.rs:1620-1623 | CLOSED | Ordering `edge` closure drops missing targets; CFG validation rejects unknown/empty successors first. |
| A0677 | current_ownership.rs:2207-2211 | UNDECIDED | Exclusive-borrow owner liveness is skipped when the source is not `Owned`; no accepting program without another check was shown. |
| A0678 | current_ownership.rs:2202 | CLOSED | Same binding/parameter length gate as A0675 before `check_edge` runs. |
| A0679 | current_ownership.rs:2467 | CLOSED | `consume_edge_sources` zips like edges; validated units already match binding and parameter counts. |

## A0672

Digest entry `CO:204` and `CO:716` (area_05 preamble: path prefix `CO` → `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership.rs`). Opened `163-179` (`origins`), `182-368` (`origins_with_analysis`), and `711-759` (`validate_compatibility` in `address_joins::descriptor_loans::compatibility`) (read). Line `204` matches the digest gate on this checkout (read); cited `716` is now `711-717` (re-located by symbol name).

Enclosing function `origins_with_analysis` (`182-`…, read): at `204` the body runs only when both `graph` and `analysis` are `Some` (`204`). If `crate::descriptor_results::optimized_catalog_graph` returns `None` (`163-169`, read), `sources`, `locations`, `effects`, and `flow` stay at their empty defaults (`197-203`, read). `BorrowedRegions::stored_effects` then returns `None` for every position (`503-508`, read). `DescriptorLoans::new` seeds loans partly from `stored_effects` (`831-844`, read).

Search `validate_descriptor_compatibility` in `omega-rust`: **2** matches — export `current_ownership.rs:9965`, call `unit_validation.rs:1250` (read). Upward: `index_and_validate_unit_catalogs` builds `CallAccesses::new` (`2865-2871`, read) with the same graph helper, replays ownership, extends `descriptor_requirements` (`9660-9671`, read), then calls validation with `call_accesses.view_graph()` (`1250-1254`, read). At `711-717`, empty requirement vectors return `Ok(())` regardless of `view_graph` (read); nonempty requirements with `view_graph == None` error at `736-742` (read).

`DescriptorLoans::new` also pre-collects requirements via `conflicted_operation` / `conflicted_edge` (`998-1020`, read), but `AddressJoinPins::check_*` passes `&mut Vec::new()` and exposes leftovers via `descriptor_requirements()` (`2154-2161`, `2192-2199`, `1889-1894`, `8837`, read).

**What would decide REACHABLE vs CLOSED:** A concrete validated `PsiOptimizationUnit` where `optimized_catalog_graph` is `None`, descriptor-loan conflicts depend only on stored call effects in `effects`, and no other validator arm rejects. `descriptor_call_origins_match` returns `false` when the graph rebuild fails (`unit_validation.rs:12842-12849`, read), but that helper’s callers were not fully traced for this row.

**Verdict UNDECIDED (read):** The empty-graph / empty-effects / empty-requirements path is read; whether a unit the checker should reject can still reach `Ok` from whole-unit validation was not shown.

## A0673

Digest `CO:553-562`. Opened `544-562` inside `BorrowedRegions` (read); lines match digest on this checkout.

`access_location` (`553-562`, read) walks `length` from `path.len()` down to `0` and returns the first `location_at` hit (`559-561`, read), so a missing full path can resolve to a strict prefix (including root-only).

Search `access_location` in `omega-rust`: **4** occurrences, all in `current_ownership.rs` (read) — definition `553`, uses `116`, `1184`, `1278` (inferred from count; `116` context is `BorrowedRegions::new` write capture `115-121`, read).

Callers upward include `DescriptorLoan::authorizes` (`1233-1248`, read), which requires `location` to appear in `authorized` for that position (`1239-1244`, read). How `authorized` is populated and whether a coarser `access_location` can satisfy it when the full path is disturbed was not fully traced.

**Verdict UNDECIDED (read):** Prefix fallback is confirmed at `553-562`; an input that should be rejected yet passes conflict checks through this fallback was not identified.

## A0674

Digest `CO:1069-1073`. Opened `1059-1141` (`DescriptorLoans::conflicted_operation`) and replay path `7937-7951` plus `8579-8592` (read).

At `1069-1073`, when the operation matches `return_or_return_unit_pattern!()`, the `moved` slice is replaced with empty before the moved-vs-loan loop at `1130-1137` (read). `return_or_return_unit_pattern!` is defined in `abstract_operations` (`operations.rs:1278`, not opened; macro name read from use at `766`).

`AddressJoinPins::check_operation` always calls `descriptor_loans.conflicted_operation` with the caller’s `moved` list (`2154-2161`, read), so the emptying applies on returns.

In `replay::validate_current_ownership_cfg`, `pins.check_operation` runs with `consumed_places` built from call/establish operations; return terminators fall through `_ => &[]` for `structural_arguments` (`7831-7842`, read), so returns usually pass an empty move list. `ReturnStructural`’s `value_use.moving` on `source` runs later in the same node handler (`8579-8592`, read), after `check_operation` (`7937` precedes `8567`, read).

**Input reaching the branch:** A function whose last operation is a return that consumes/moves a place still backing a live descriptor loan, where conflict would be visible only via the `moved` list (not via `writes_at` / result replacement checks at `1088-1128`, read).

**Verdict REACHABLE (read + inferred):** Descriptor-loan moved checking is explicitly disabled on return patterns, and return moves are applied after `check_operation`; `validate_normal_return` (`7379-7410`, read) checks reference custody, not descriptor loans at the return site.

## A0675

Digest cites `CO:1160` and `CO:1276` zip truncation. Opened `1143-1229` (`conflicted_edge`) (read). At `1157-1160`, `edge.structural_bindings.iter().zip(destination)` pairs bindings to destination parameters (read). Cited `1276` in the digest is not a `zip` on this checkout; it is `result.push(Access { ...` in `operation_observations` (`1276-1282`, read) — re-located by symbol: the zip claim applies to `1160`, not `1276`.

Search `conflicted_edge` in `omega-rust`: **3** matches in `current_ownership.rs` — definition `1143`, body use `1010`, `2192` (read).

Caller chain: `DescriptorLoans::new` (`1010`, read) and `AddressJoinPins::check_edge` (`2192-2199`, read) → `replay` edge loop (`8711-8719`, read) → `validate_current_ownership_frontier_collecting` (`9839-9868`, read) → `validate_function` (`9660`, read).

Closing check: `parameters::` edge validation requires `edge.structural_bindings.len() == destination.structural_parameters.len()` or errors (`unit_validation.rs:9045-9047`, read). `validate_function` runs `validate_nodes_and_edges` before ownership replay (`9594`, `9660`, read).

**Verdict CLOSED (read):** Extra bindings cannot exist on a unit that already passed `9045-9047`; truncation at `1160` does not drop pairs on accepted programs.

## A0676

Digest `CO:1620-1623`. Opened `1598-1683` inside `RangeRelations::new` (read). The inner `edge` closure at `1619-1641` returns `None` when `(target, 0)` is absent from `positions` (`1620-1622`) or `function.block(target)` is missing (`1623`, read). Jump/successor lists use `.filter_map` / `.flatten()` so `None` drops the edge (`1644-1668`, read).

`RangeRelations::new` is invoked from `BorrowedRegions::new` only when windowed alias projections exist (`141-146`, read).

Search `RangeRelations::new` in `omega-rust`: **2** matches — `current_ownership.rs:146`, `descriptor_results.rs` (1 hit, not read).

Upstream on accepted units: `control_flow::validate_nodes_and_edges` rejects empty blocks (`5680-5684`, read). Edge validation rejects unknown successors (`9038-9043`, read). Positions list includes `(block.id, 0)` for every non-empty block (`1604-1612`, read).

**Verdict CLOSED (read):** Dropped ordering edges require missing blocks or empty targets that unit validation already rejects before `RangeRelations::new` runs.

## A0677

Digest `CO:2207-2211`. Opened full `AddressJoinPins::check_edge` (`2183-2300`, read). Loop at `2202-2249` handles exclusive-borrow destination parameters. At `2207-2210`, `structural_source_contract` filtered to `Owned` — if missing or not owned, `continue` skips owner-liveness (`2212-2248`, read).

Search `check_edge` in `omega-rust`: **2** production callsites in `current_ownership.rs` — `8711`, definition `2183` (read).

Edge structural binding validation (`9048-9075`, read) runs earlier in `validate_function` but was not exhaustively compared to every exclusive-borrow / non-owned source combination.

**Verdict UNDECIDED (read):** Skip path is read at `2207-2211`; no concrete program that should fail owner-liveness yet passes all other pins and catalog checks was demonstrated.

## A0678

Digest `CO:2202`. Same `check_edge` loop as A0677; `2202` is `edge.structural_bindings.iter().zip(destination)` (read). Digest claim matches code.

Closing check: identical to A0675 — `unit_validation.rs:9045-9047` (read) before ownership replay.

**Verdict CLOSED (read):** Zip length mismatch cannot occur on validated units.

## A0679

Digest `CO:2467`, `2554`, `2589`. Opened `2452-2646` (`consume_edge_sources`, read). Zips at `2467`, `2554-2556`, `2589` pair `bindings` with `target.structural_parameters` (read). Final claim reconciliation uses `bound.len() != transferred.len()` (`2642-2644`, read).

Search `consume_edge_sources` in `omega-rust`: **2** matches in `current_ownership.rs` — definition `2452`, call `8772` (read). Caller: `replay` edge transfer (`8772-8779`, read) with `edge.structural_bindings` and `blocks[&edge.target]` parameters.

Same length gate as A0675/A0678: `9045-9047` (read).

**Verdict CLOSED (read):** On validated units, binding and parameter counts match; zip truncation does not skip bindings, and `2642-2644` still ties claim transfers to the processed pairs.

## What I did not read

- Full `descriptor_results::catalog_graph` / `adapt` failure modes beyond `1100-1142` (`descriptor_results.rs`).
- Complete `validate_structural_root_operations` and all `descriptor_call_origins_match` callers (`unit_validation.rs` beyond excerpts).
- How `DescriptorLoan::authorized` is populated across all operation kinds.
- Producer paths or corpus programs demonstrating acceptance (no build per atlas).

SLICE COMPLETE
