# SELECTED-OPTIMIZATION-CATALOG-ROUTE — scope verification (2026-09-20, `c431bc8138`)

Mined stub at `TASKS.md:9927`, already scope-verified at `d8041919ad` —
sixth stub on the SELECTED-REWRITE-CATALOG cluster. Re-verified at
`c431bc8138`: the verdict stands.

## Cluster state

- Re-mines the catalog-route leg of EXACT-MACHINE-SIMPLIFICATIONS
  (TASKS_OPTIMIZER.md:641): each `Orphaned` rewrite in
  `rewrites/module_catalog.rs` needs a catalog entry executed by the
  stage entrance (`optimize_selected_instructions` /
  `run_selected_lowering_optimizations` — still identity route +
  selected-lowering pair folds; 42 `Orphaned` markers).
- DISPOSITION row Resolved: disposition is indivisible from the catalog
  leg; delete half already exercised (`literal_compare` +
  `literal_arithmetic` removed, pinned in
  `optimizer_source_organization::retired_paths`).

## Fences (claims snapshot ~04:37Z item lease time)

| Surface | Claim |
| --- | --- |
| `rewrites/{mod.rs,module_catalog.rs}` | ORPHAN-REWRITE-MODULES-CATALOG
  (Devin / z146, 04:27Z) |
| `rewrites/selected_lowering/*` | COMPOSABLE-PAIR-DESCRIPTORS (23:57Z) |
| item: SELECTED-REWRITE-CATALOG-ROUTE | Jarod (04:11Z) |
| item-level: SELECTED-REWRITE-CATALOG-DISPOSITION,
  PIPELINE-REWRITE-ORPHANS, REWRITE-VALIDATOR-INDEPENDENCE |
  zergling-182 |

## Outcome

Folds into SELECTED-REWRITE-CATALOG-EXECUTION — no independent slice.
Record only; no code change.
