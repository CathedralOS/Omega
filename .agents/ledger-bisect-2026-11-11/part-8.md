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

### Verdict

**never-good since `7399f90b98d974e4ee7f8d5c823fae5dd28fc8b2`** (#295, first commit adding `tests/omega/pass/facts/opaque_independent_qualified_result_*_exit`).

`find_good_commit.sh` and fixed-PIN reruns on the first-parent chain from the PIN through that introduction commit never returned exit **0** (`checked` / `built` per PIN snapshot). Outcomes were exit **1** (PIN-broken incomplete) or exit **125** (rejected, build failure, ledger/md skip, or other ≠ expected).

There is **no** `git bisect` good commit; **#2238 is not a breaking commit** (parent is also not good: exit 125).

### Evidence — fixture introduction and walk samples

Witness: `pass/facts/opaque_independent_qualified_result_import_alias_exit`.  
`bisect_witness.sh` + `bisect_test.py` with fixed PIN observation (same command path as bisect).

| commit (40 hex) | `bisect_test` exit | corpus status |
| --- | ---: | --- |
| `7399f90b98d974e4ee7f8d5c823fae5dd28fc8b2` (fixture add) | **125** | `rejected` |
| `a6dfe40c6b59efe1d1af1e453ff9e9e2e2203218` (sample, ~304 on chain) | **125** | `rejected` |
| `643ff5441c15ea62815109514a8047194d7397a0` (sample, nearer PIN) | **1** | `incomplete` (PIN-broken class) |
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

**Mechanism:** not established — the edge correlates with #2238 but parent is not good and `find_good` never succeeds; no causal link proved.

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

### Ledger “checked” vs never-good

Fixed-PIN ledger copy at PIN lists all three rows as **`checked`** stamped `@60ee163d4f0f5f8b9349ecf253660a917917a7c7` (not on the first-parent chain to PIN).

| source | ledger says | fixed-PIN rerun @ that commit (import_alias witness) |
| --- | --- | --- |
| PIN ledger rows | `checked` @ `60ee163d4f0f5f8b9349ecf253660a917917a7c7` | @ `60ee163d4f0f5f8b9349ecf253660a917917a7c7`: exit **125**, `rejected` |
| `c64728d90e83036051d52fb69961b786423230c6` (#889 recorder) | rows `checked` @ `ccdb99ca2214b93c0e2535a31cb34a3e2167d9dd` | @ `c64728d90e83036051d52fb69961b786423230c6`: exit **125**, `rejected` |
| Part 17 opaque cluster (#1721) | first-bad `60cb5e1f79dd05465f77775990e46bcc2e789470` | @ `60cb5e1f79dd05465f77775990e46bcc2e789470`: exit **125**, `rejected` |

`c64728d90e83036051d52fb69961b786423230c6` is the commit that **first recorded** `checked` for these three in ledger history (`git log -S`); `ccdb99ca2214b93c0e2535a31cb34a3e2167d9dd` is the integration stamp named in that ledger file’s context header, not re-run as exit **0** here.

**Resolution:** ledger **`checked`** entries are historical stamps / integration records; they do **not** contradict **never-good since `7399f90b98d974e4ee7f8d5c823fae5dd28fc8b2`** under fixed-PIN `bisect_test` on this host. Part 17’s #1721 first-bad is a separate bisect slice on the same diagnostic family, not a good commit for this cluster.

## Propagated / fixed-on-PIN clusters

All other slice clusters are **fixed at PIN** (no bisect): arithmetic remainder/bounds pack (13 measured), generics runtime-value recipe pair, refined borrowed implementation, open-record specialization, inductive climbing sum.

## Tooling (`.agents/` only)

`tools/pin_bisect_setup.py`, `bisect_test.py`, `bisect_witness.sh`, `find_good_commit.sh`, `run_bisect.sh`, plus corpus batch copies for slice recovery.
