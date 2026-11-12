# Ledger bisect — part 8 (k=8, N=40)

**PIN:** `51e8c45c793007f211d0d9baee7de16767172444`  
**Branch:** `work/ledger-bisect-part-8` (no PR)  
**Host:** linux_x86_64 cloud VM, `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `nice -n 19`  
**Methodology:** Lead correction #2 — fixed PIN ledger copy + `pin_observation.json` under `/tmp/ledger-bisect-part-8/opaque/`; `bisect_test.py` never reads `corpus_outcomes.txt` at the checked-out revision; ledger-only first-parent commits skipped (exit 125).

## Slice summary (orig_index % 40 == 8)

| orig_index | report_rows | witness (short) | PIN @51e8c45c793007f211d0d9baee7de16767172444 |
| ---: | ---: | --- | --- |
| 8 | 17 (13 measured) | `fail/arithmetic/remainder_postcondition_signed_minimum_underbound_rejected` | fixed (all measured members pass expected fail/check) |
| 48 | 3 | `pass/facts/opaque_independent_qualified_result_import_alias_exit` | **broken** — PIN cluster below |
| 88 | 2 | `fail/generics/runtime_value_recipe_join_bound_loss` | fixed |
| 128 | 1 | `pass/traits/refined_borrowed_implementation_exit` | fixed |
| 168 | 1 | `fail/facts/open_record_const_implementation_specialization_agreement_rejected` | fixed |
| 208 | 1 | `fail/proofs/inductive_climbing_sum_unbounded_accumulator` | fixed |

**PIN re-measurement (21 unique fixtures in slice):** 18 fixed, 3 broken (opaque qualified-result trio), 0 host-only, 0 missing. State: `/tmp/part8_pin_state.json`, records: `/tmp/part8_pin_records.txt`.

Report cluster 8’s prior “breaking” `f3612364ffbb9489d7be1409397df6867fcbebc6` (#2228) is **ledger-only** under lead #2 — it must not be treated as a compiler break.

## PIN cluster (still broken at PIN)

| diagnostic (norm) | members | witness | PIN outcome | ledger expects |
| --- | --- | --- | --- | --- |
| `` `_` has no executable body … (authored result domains) `` | 3 opaque `pass/facts/*_qualified_result_*_exit` | `pass/facts/opaque_independent_qualified_result_import_alias_exit` | `incomplete` | `checked` |

## Opaque qualified-result cluster (orig_index 48)

### Verdict (tie-break vs part 30, fresh target per commit)

**Not never-good.** Outcomes are **non-monotonic** on the first-parent line (e.g. `rejected` at fixture add, **`checked` later**, then broken again). Earlier “never-good” used a **shared** `$ROOT/target` and falsely missed good commits (same failure mode as part 9).

| role | commit (40 hex) | PR | `bisect_test` (all 3 members) |
| --- | --- | ---: | --- |
| **good** (bisect) | `4d6278c81b8360d8100cef23e2100be18ddf32a7` | #1759 | exit **0**, status `checked` (3 fixtures × 3 reruns) |
| **first-bad** | `60cb5e1f79dd05465f77775990e46bcc2e789470` | #1721 | exit **1**, status `incomplete` (AddressBridge class; 3×3) |
| diagnostic-class edge | `33cefbc6c241048ff6f6c583acc5aca9b0176ec5` | #2238 | parent `rejected` (125) → child `incomplete` (1); not a bisect “good” |

**Parent check:** `60cb5e1f79dd05465f77775990e46bcc2e789470^1` = `4d6278c81b8360d8100cef23e2100be18ddf32a7` (matches part 30).

At **#1721** the trio stops matching PIN-expected `checked` (exit **1** incomplete, not exit **0**). Tie-break does **not** show exit **125** `rejected` at `60cb5e1f79dd05465f77775990e46bcc2e789470` with a fresh target (reconciles part 30’s good parent; first-failure **class** here is incomplete).

**#2238 is not the bisect first-bad**; it is the first commit on the `cdb14cc0c3e5b5bf3ea887f1c9d35737cab677d2` → `33cefbc6c241048ff6f6c583acc5aca9b0176ec5` edge where corpus status flips **`rejected` → `incomplete`** (NativeFloats rejection → AddressBridge incomplete).

### Tie-break runs (log: `/opt/cursor/artifacts/part8-tiebreak.log`)

Method: fixed PIN ledger in `/tmp/pin_corpus_outcomes_51e8c45.txt`; `BISECT_CARGO_TARGET_DIR=/tmp/bisect-target-<full-sha>` **new per commit**; one build/corpus at a time.

**`4d6278c81b8360d8100cef23e2100be18ddf32a7`** — all lines `exit=0 status=checked`:

- `opaque_independent_qualified_result_import_alias_exit` reps 1–3
- `opaque_independent_qualified_result_renumbered_catalog_exit` reps 1–3
- `opaque_independent_qualified_result_through_local_exit` reps 1–3

**`60cb5e1f79dd05465f77775990e46bcc2e789470`** — all lines `exit=1 status=incomplete` (AddressBridge `classified_make` … authored result domains).

**`60ee163d4f0f5f8b9349ecf253660a917917a7c7`** (ledger stamp; one run per member, fresh target) — all `exit=0 status=checked`. The prior report’s **`exit=125`** at this SHA used the **shared** target dir, not a commit-scoped build tree.

### Earlier walk samples (shared target — superseded for good/bad)

| commit (40 hex) | exit | status (import_alias) |
| --- | ---: | --- |
| `7399f90b98d974e4ee7f8d5c823fae5dd28fc8b2` (fixture add) | **125** | `rejected` |
| `643ff5441c15ea62815109514a8047194d7397a0` | **1** | `incomplete` |
| `51e8c45c793007f211d0d9baee7de16767172444` (PIN) | **1** | `incomplete` |

### First commit showing current diagnostic class (rejected → incomplete), not “breaking”

First-parent parent → child where all three members flip from **`rejected`** (exit 125) to **`incomplete`** AddressBridge class (exit 1):

| | `cdb14cc0c3e5b5bf3ea887f1c9d35737cab677d2` (#2292) | `33cefbc6c241048ff6f6c583acc5aca9b0176ec5` (#2238) |
| --- | --- | --- |
| PR | psi: lower nested-machine cycles… | compiler: realize canonical float operations… |

**All three members @ parent** (`cdb14cc0c3e5b5bf3ea887f1c9d35737cab677d2`), `bisect_test` exit → corpus status:

| fixture | exit | status |
| --- | ---: | --- |
| `pass/facts/opaque_independent_qualified_result_import_alias_exit` | **125** | `rejected` |
| `pass/facts/opaque_independent_qualified_result_renumbered_catalog_exit` | **125** | `rejected` |
| `pass/facts/opaque_independent_qualified_result_through_local_exit` | **125** | `rejected` |

**All three members @ child** (`33cefbc6c241048ff6f6c583acc5aca9b0176ec5`):

| fixture | exit | status |
| --- | ---: | --- |
| `pass/facts/opaque_independent_qualified_result_import_alias_exit` | **1** | `incomplete` |
| `pass/facts/opaque_independent_qualified_result_renumbered_catalog_exit` | **1** | `incomplete` |
| `pass/facts/opaque_independent_qualified_result_through_local_exit` | **1** | `incomplete` |

Typical parent diagnostic: `NativeFloats::float_add_f32_f32` requires a checked body. Child: `AddressBridgeProvider::classified_make` has no executable body (authored result domains).

**Mechanism (#2238):** not established.

**Diff note (#2238):** adds `omega-rust/omega/compilation/backend/target/src/float_realization.rs` (new file), opening:

```diff
+/*
+Each hosted target declares how it realizes every canonical `f32`/`f64`
+operation, so float arithmetic no longer depends on a Build program choosing a
+provider for 104 operator slots on every compile. The table is the target's
+declaration under decision `target-realized-float-arithmetic`
+...
+*/
+
+use crate::TargetProfile;
+...
+pub const CANONICAL_FLOAT_OPERATIONS: [&str; 104] = [
```

### Mechanism notes (named commits)

**`4d6278c81b8360d8100cef23e2100be18ddf32a7` (#1759):** mechanism **not established** for why opaque qualified-result fixtures pass here (commit is mostly sample `block` transition syntax, e.g. `samples/cli/algorithms/insertion_sort/main.omg`).

**`60cb5e1f79dd05465f77775990e46bcc2e789470` (#1721):**

```diff
+    if let Some(boundary) = requirements.missing_providers.first() {
+        ...
+        return Err(format!(
+            "calling policy calls `{}` without a selected provider for target `{}`",
+            ...
+        ));
+    }
```

(`calling_policy_execution.rs`; `provider_supply.rs` delegates display to `callable_display_path`.)

### Ledger stamps vs measured good

PIN ledger rows: **`checked`** @ `60ee163d4f0f5f8b9349ecf253660a917917a7c7`. With **fresh** `CARGO_TARGET_DIR`, reruns at `60ee163d4f0f5f8b9349ecf253660a917917a7c7` return exit **0** for all three members (aligns with ledger). **`c64728d90e83036051d52fb69961b786423230c6`** (#889) first **recorded** `checked` in ledger history (`ccdb99ca2214b93c0e2535a31cb34a3e2167d9dd` integration context in file header).

## Propagated / fixed-on-PIN clusters

All other slice clusters are **fixed at PIN** (no bisect): arithmetic remainder/bounds pack (13 measured), generics runtime-value recipe pair, refined borrowed implementation, open-record specialization, inductive climbing sum.

## Tooling (`.agents/` only)

`tools/pin_bisect_setup.py`, `bisect_test.py`, `bisect_witness.sh`, `find_good_commit.sh`, `run_bisect.sh`, plus corpus batch copies for slice recovery.
