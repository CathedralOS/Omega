slice_0088

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0672 | CO:204, CO:716 | REACHABLE | Catalog graph `None` leaves call stored effects empty; `validate_compatibility` returns `Ok` on empty requirements while graph-dependent loan births are skipped. |
| A0673 | CO:553-562 | CLOSED | Longest-prefix `access_location` is the defined lookup rule; `forced` overlap still errors at CO:1194-1204 without relying on exact path keys. |
| A0674 | CO:1069-1073 | CLOSED | Return-only moved clearing is inside descriptor-loan `conflicted_operation`; shared pin overlap still uses full `moved` at CO:2174-2179. |
| A0675 | CO:1160-1162 | CLOSED | `zip` pairs only after `structural_transfers::validate` enforces equal binding and parameter counts at UV:9045-9046. |
| A0676 | CO:1620-1623 | CLOSED | Edge closure `?` only drops scalar-ordering edges when target entry position is absent; validated CFG rejects unknown targets at UV:5708-5713. |
| A0677 | CO:2207-2210 | CLOSED | Owner-liveness skip applies only when `structural_source_contract` is missing or not `Owned`; exclusive-borrow edges are vetted at UV:9177-9213. |
| A0678 | CO:2202 | CLOSED | Same binding/parameter length gate as A0675 at UV:9045-9046 before `check_edge` runs. |
| A0679 | CO:2467 | CLOSED | `zip` truncation on bindings is length-gated at UV:9045-9046; claim pairing is separately enforced at CO:2642-2644. |

### A0672

1. Read `BorrowedRegions::origins_with_analysis` in full (CO:182-369, read): when `graph` or `analysis` is `None`, the block starting at CO:204 does not run; `effects` stays empty (CO:202, CO:293-303 skipped).
2. Read `descriptor_loans::compatibility::validate_compatibility` in full (CO:711-759, read): empty requirement vectors return `Ok` (CO:716-717); nonempty requirements with `view_graph == None` error (CO:736-742).
3. Search `rg '\bvalidate_descriptor_compatibility\b' /agent/repos/omega/omega-rust` — 2 hits (CO:9965 re-export, UV:1250 call). Read `unit_validation` caller UV:1207-1254 (read): builds `CallAccesses` (may have `view_graph` `None` per DR:1036-1044), collects per-function requirements, then calls compatibility check.
4. Search `rg '\borigins_with_analysis\b' /agent/repos/omega/omega-rust` — 3 hits, all in `current_ownership.rs` (CO:173, CO:182, CO:2883). Read `CallAccesses::new` CO:2865-2891 (read): shared catalog graph optional; each machine gets `BorrowedRegions` via `origins_with_analysis`.
5. Read `DescriptorLoans::new` loan births from `stored_effects` CO:831-843 (read): empty `effects` skips call-effect births; static `conflicted_*` scan CO:997-1020 may leave `requirements` empty; returned at CO:8837 via `pins.descriptor_requirements()`.
6. Input reaching accepting branch: unit reaches UV:1250 with collected `descriptor_requirements` all empty while `call_accesses.view_graph()` is `None`, after graph build returned `None` (DR:1125, DR:1107-1123).
7. **Verdict: REACHABLE** — A cross-function optimization unit that passes earlier checks but whose descriptor catalog graph build fails could validate with no stored-effect descriptor loans and empty compatibility requirements, so call-effect loan conflicts never reach `validate_compatibility`.
8. Unsure: minimal witness program shape; whether every graph-failure case also fails UV:7190-7236 (read partially — shared unrestricted descriptor returns require graph).

### A0673

1. Read `access_location` in full (CO:553-562, read): tries `path` prefixes longest-first via `location_at`.
2. Search `rg '\baccess_location\b' /agent/repos/omega/omega-rust` — 4 hits in `current_ownership.rs`; read call sites CO:116-120, CO:1184-1187, CO:1279 (read). Primary accepting use: `conflicted_edge` passes result to `loan.authorizes` CO:1208-1209 (read `authorizes` CO:1233-1249).
3. Read callers of `validate_current_ownership_cfg` / `check_edge` UV:9660, CO:8711 (read); did not read all of `loan.authorizes` consumers outside `conflicted_edge`.
4. Input: observation path with no exact `locations` key but a shorter prefix key exists (including empty path = whole root).
5. **Verdict: CLOSED** — Prefix fallback is the stated resolution rule; `forced` owned disturbances still error without needing exact-path `location` (CO:1194-1204).
6. Unsure: none material for verdict.

### A0674

1. Read `DescriptorLoans::conflicted_operation` in full (CO:1059-1141, read): return operations replace `moved` with empty slice (CO:1069-1073).
2. Search `rg 'conflicted_operation\(' /agent/repos/omega/omega-rust` — 6 hits; read `AddressJoinPins::check_operation` CO:2145-2179 (full, read) and static collector CO:1000-1007 (read).
3. Read CFG caller CO:7937-7949 (read): passes `consumed_places` including return moves; descriptor path clears moved only inside CO:1069-1073; shared pin path CO:2174-2179 still receives full `moved`.
4. Read return handling CO:8567-8599 (partial, read).
5. Input on accepting branch: return-shaped operation in descriptor-loan conflict scan with nonempty caller `moved`.
6. **Verdict: CLOSED** — Descriptor-loan moved-vs-loan check is intentionally skipped on returns; shared join overlap and return `value_use.moving` still run on the same moves.
7. Unsure: none material.

### A0675

1. Read `conflicted_edge` disturbance selection (CO:1157-1226, read): `zip(edge.structural_bindings, destination)` CO:1159-1160.
2. Search callers: `conflicted_edge` CO:1010-1017, CO:2192-2198 (read).
3. Read `structural_transfers::validate` UV:9036-9047 (read): `edge.structural_bindings.len() != destination.structural_parameters.len()` rejects unit before ownership replay (UV:9593).
4. Input: cannot reach CO:1160 with extra bindings on a validated unit.
5. **Verdict: CLOSED** — UV:9045-9046 forces equal lengths; `zip` is total on validated input.
6. Unsure: none.

### A0676

1. Read `RangeRelations::new` edge closure `edge` closure (CO:1619-1642, read): `target_index` lookup uses `?` (CO:1620-1622); drops successor edge from scalar ordering graph, not CFG.
2. Search `RangeRelations::new` — 2 hits (CO:146, DR:2669); did not read DR:2669 caller body in full.
3. Read UV:5707-5713 (read): unknown successor target rejected before ownership.
4. Entry note: description “missing or empty target” is imprecise (read CO:1620-1623) — code drops when `(target, 0)` is absent from `positions` or `function.block(target)` is `None`.
5. Input: only malformed or pre-validation edges; validated units have every block’s `(id, 0)` in `positions` CO:1604-1611.
6. **Verdict: CLOSED** — Accepting branch is unreachable on units that pass CFG validation; effect is fewer scalar-ordering edges only.
7. Unsure: whether a dropped ordering edge could change alias-window resolution on an otherwise valid unit (did not trace `resolve` ordering path end-to-end).

### A0677

1. Read `check_edge` exclusive-borrow loop (CO:2202-2248, read): `continue` when no owned contract (CO:2207-2210).
2. Search `check_edge` caller CO:8711 (read).
3. Read `structural_transfers::validate` exclusive-borrow / address-join arms UV:9174-9213 (read): constrains sources when parameter is exclusive-borrow pair.
4. Input: binding into exclusive-borrow parameter whose argument place lacks owned `structural_source_contract` (borrow-sourced edge).
5. **Verdict: CLOSED** — Skip is intentional; edge transfer legality already decided at UV:9177-9213 before replay.
6. Unsure: none.

### A0678

1. Read `check_edge` shared-pin disturbance `zip` CO:2273-2276 (read).
2. Same caller trace as A0675/A0677 (CO:8711, UV:9593).
3. Read UV:9045-9046 (read).
4. **Verdict: CLOSED** — Equal binding/parameter counts required before `check_edge`.
5. Unsure: none.

### A0679

1. Read `consume_edge_sources` in full (CO:2452-2646, read): three `zip` sites CO:2467, CO:2554-2556, CO:2589; final `bound.len() != transferred.len()` CO:2642-2644.
2. Search `consume_edge_sources` — definition CO:2452, caller CO:8772 (read); did not read `establish_edge_parameters` beyond CO:2747-2784.
3. Read UV:9045-9046 (read).
4. Input: extra bindings cannot exist on validated units; extra owned claims caught at CO:2639-2640 or CO:2642-2644.
5. **Verdict: CLOSED** — Binding count gated at UV:9045-9046; claim count gated at CO:2642-2644 (entry’s “only claims” is accurate, read CO:2642).
6. Unsure: none.

### What I did not read

- Full `adapt` / `catalog_graph` bodies beyond DR:1089-1142 and DR:1215-1269 openings.
- Entire `validate_current_ownership_cfg` replay loop (only sites around CO:7937, CO:8711, CO:8837).
- `terminal_semantics` descriptor kernel implementations outside cited `current_ownership` / `descriptor_results` snippets.
- Test-only modules.

SLICE COMPLETE
