# PACKAGE-REVIEW-HOTSPOT-ATTRIBUTION — linux x86-64 leg

Attribution record for where `omega audit packages` time goes, measured on
linux x86-64 (swarm VM, `cargo` dev-profile `omega` — `mbx` unavailable).
Binary built from main checkout at `649d7ca3802` (origin/main at
`a51cb805cc1` when written, 2026-09-20); the measured surface — typed-tree
arena access, symbol lookup, checked derivation, package-evidence capture —
is unchanged across that delta.

Subject: `samples/cli/basics/cli_mvp` — the smallest real project, a
2-package graph (`cli-mvp` → `omega-language-std` via a local path edge).
Delete this record once the arena span-validation and symbol-lookup hotspots
are repaired or carried on their own board rows.

## Headline measurement

| Command | Wall | User | Sys | Exit |
| --- | --- | --- | --- | --- |
| `omega audit packages --project samples/cli/basics/cli_mvp --details` | **20m0.5s** | 20m0.2s | 0m0.1s | 3 (`policy requires review` — the expected fresh-inspection answer; ~127k lines of `--details` output) |
| `omega --check --timings samples/cli/basics/cli_mvp/main.omg` | **19m38.6s** | 19m38.4s | 0m0.1s | 0 |

The `--timings` split on the check is `prepare: 47.4ms`, `compile:
1178.5s`, `total: 1178.6s` — everything is the compile stage. Both runs are
100% userspace, single-threaded (utime ≈ walltime; one `omega-compile`
thread hot). Nothing in either profile is I/O, sandbox, or filesystem
capture — it is pure derivation.

**Audit ≈ check + ~22 s.** The review layer — fresh-analysis driver,
`package_evidence::capture` projections, audit/row assembly — accounts for
~1.8% of the audit wall on this graph. The other ~98% is the same checked
compilation a plain `--check` pays for the same sources.

## Where the cycles go

`perf record -F 99` flat profile, audit run (1983-sample window mid-run;
identical shape in a `--check` comparison window):

- **`arena::Arena<T>::valid_span_range` dominates the flat profile.** Its
  monomorphizations over typed-tree storages — `control_flow::state::State`,
  `calls::signature::StateParameter`, `declarations::data::DataDefinition`,
  `control_flow::machine::Machine`, `control_flow::statement::StatementNode`
  — plus their `Iterator::any` subroutines are the top ~15 frames combined.
  Each call is O(span length): it walks `occupied[start..end]` and
  `generations[start..end]` (`psi/foundation/arena/src/arena.rs:431`), and
  every `span()`/`span_mut()`/`span_or_empty()` accessor pays it. The cost
  is call volume × span length, not one blowup — the spans being validated
  are the large per-unit lists.
- **Symbol arena + symbol-table lookups.** `PagedArena<Symbol>` indexing
  (`position_from_logical_index`, `get`), `Handle<Symbol>::eq`,
  `Identifier::as_str`, `memchr`/`Option::branch` plumbing, and the
  `SymbolTable::lookup_top_level → find_top_level_by_name_and_kinds_from_source
  → SymbolChildHandles` filter chain — every name resolution materializes a
  `Vec<Handle<Symbol>>` of children and filter-checks each.
- **`validation::value_custody::expression_types::named_value_type_reference`**
  (~2%) — per-named-reference custody typing that re-walks signature
  parameter spans (it appears in the flat profile next to its
  `StateParameter` `find` closure).
- Phase attribution (dwarf callgraph, partially truncated by the unwinder):
  `syntax_trees_to_symbol_resolved_trees::resolution::drive_recorded →
  symbols::assign` accounts for ~10% of visible samples; the
  package-review-specific projection machinery —
  `package_evidence::capture::package_policy::project_checked_package_policy`,
  `project_checked_package_review`,
  `api::data::evidence::require_rederived_data_definition_facts` —
  shows ~0.5–1.2% each. The rest of the attribution is smeared across the
  generic arena/symbol plumbing the phases share.

## Attribution

The hotspot is **not in the review-specific machinery** (audit triage,
candidate/commitment/reconstruction rows, policy compare — ~1.8% of the
wall). It is the shared frontend: package audit re-runs the whole
per-package derivation — tokenize → symbol-resolve → typed → checked —
for each package in the closure, and the cost lands in the two paths every
stage shares: arena handle→span validation and name→symbol lookup. A
2-package graph whose second package is `omega-language-std` costs twenty
minutes; `cli-mvp` itself is a rounding error — the std closure
re-derivation is the bill, and it is paid identically by plain `--check`.

Two consequence directions for the sibling items:

- **PACKAGE-REVIEW-ROUTE-COST-ATTRIBUTION** (route granularity): the
  dominant route is "fresh-analysis → per-package
  `project_checked_package_review` → checked compilation of
  omega-language-std". Nothing review-owned to cache: the win is in not
  re-running a full checked compilation per audited package.
- **PACKAGE-REVIEW-ROUTE-ATTRIBUTION**: same route, row granularity —
  the `--details` dump shows the review rows are all
  `require_rederived_*`-style facts re-derived from scratch per package.

Mechanism fixes (out of this item's scope) point at `arena` and `symbols`,
not at `packages/review`: memoize `valid_span_range`-validated spans or add
an unchecked fast path, and index `SymbolTable` child lookup by name
instead of collect+filter.

## Caveats

- Dev-profile numbers: absolute durations are inflated; the *relative*
  attribution (arena/symbol plumbing ≫ review machinery) is build-mode
  independent.
- Dwarf unwinding truncates deep stacks on this build; the ~90% of samples
  not reaching a named phase frame land in the generic plumbing above, so
  phase shares are floors, not totals.
- No caching knobs were exercised; `--offline` was not needed (all-local
  path edges). Re-running with an accepted lock would test the
  fresh-vs-accepted split, which this leg does not cover.
