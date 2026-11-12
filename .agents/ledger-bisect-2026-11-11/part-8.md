# Ledger bisect — part 8 (k=8, N=40)

**PIN:** `51e8c45c793007f211d0d9baee7de16767172444`  
**Branch:** `work/ledger-bisect-part-8` (no PR)  
**Host:** linux_x86_64 cloud VM, `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `nice -n 19`  
**Methodology:** Lead correction #2 — fixed PIN ledger copy + `pin_observation.json` under `/tmp/ledger-bisect-part-8/opaque/`; `bisect_test.py` never reads `corpus_outcomes.txt` at the checked-out revision; ledger-only first-parent commits skipped (exit 125).

## Slice summary (orig_index % 40 == 8)

| orig_index | report_rows | witness (short) | PIN @51e8c45c793 |
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

## Bisect — opaque qualified-result cluster (orig_index 48)

### Good commit (`find_good`, exit 0)

`find_good_commit.sh` walked **>400 first-parent steps** from PIN and sampled the **2074** first-parent commits between fixture introduction (`7399f90b98d`, #295) and PIN. Every runnable outcome was either:

- exit **1** — PIN-broken `incomplete` + AddressBridge `classified_make` “no executable body”, or  
- exit **125** — build failure, ledger/md-only skip, or **`rejected`** (e.g. `NativeFloats::float_add_f32_f32` requires checked body) — not ledger-expected `checked`.

No first-parent ancestor on this host returned exit **0** for the fixed PIN expected outcome (`checked`). The ledger stamp `@60ee163d4f0f` on this row is **not** a first-parent ancestor of PIN; re-running that SHA here still yields `incomplete` with the same diagnostic class.

**Recorded good commit for `git bisect`:** _none (methodology requires exit 0; not satisfied on first-parent chain)._  
**Boundary parent (last non–PIN-broken class before break):** `cdb14cc0c3e5b5bf3ea887f1c9d35737cab677d2` — `rejected`, `NativeFloats::float_add_f32_f32` (exit 125).

### Breaking commit (manual first-parent boundary + reruns)

| field | value |
| --- | --- |
| **breaking_sha** | `33cefbc6c241048ff6f6c583acc5aca9b0176ec5` |
| **PR** | [#2238](https://github.com/cathedralos/omega/pull/2238) — `compiler: realize canonical float operations from the target's declared table` |
| **parent** | `cdb14cc0c3e5b5bf3ea887f1c9d35737cab677d2` (#2292) |
| **mechanism (diff)** | Moves canonical `f32`/`f64` operator realization into per-target `TargetProfile::canonical_float_realization` (`float_realization.rs`), binding the sealed float catalog from the target table instead of Build-time `NativeFloats` selection in core; downstream checking/lowering no longer follows the prior float-selection path — witness flips from unrelated float rejection to **AddressBridgeProvider `classified_make` bodyless / authored result domains incomplete**. |
| **bisect_test @child** | exit **1** (PIN-broken class) |
| **bisect_test @parent** | exit **125** (`rejected`, different diagnostic class) |

Ledger-only #2228 (`f3612364`) sits **between** child and PIN but does not change compiler behaviour (skip 125).

### Commands (witness)

```bash
export BISECT_FIXTURE_DIR=/tmp/ledger-bisect-part-8/opaque
export BISECT_WITNESS=pass/facts/opaque_independent_qualified_result_import_alias_exit
python3 .agents/ledger-bisect-2026-11-11/tools/pin_bisect_setup.py \
  pass/facts/opaque_independent_qualified_result_import_alias_exit \
  /tmp/part8_pin_records.txt /tmp/ledger-bisect-part-8/opaque

# parent / child reruns
git checkout cdb14cc0c3e5b5bf3ea887f1c9d35737cab677d2
bash .agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh   # → 125

git checkout 33cefbc6c241048ff6f6c583acc5aca9b0176ec5
bash .agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh   # → 1
```

## Propagated / fixed-on-PIN clusters

All other slice clusters are **fixed at PIN** (no bisect): arithmetic remainder/bounds pack (13 measured), generics runtime-value recipe pair, refined borrowed implementation, open-record specialization, inductive climbing sum.

## Tooling touched (`.agents/` only)

`tools/pin_bisect_setup.py`, `bisect_test.py`, `bisect_witness.sh`, `find_good_commit.sh`, `run_bisect.sh`, plus corpus batch copies for slice recovery.
