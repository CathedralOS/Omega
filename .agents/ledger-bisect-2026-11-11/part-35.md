# Ledger bisect part 35 (N=40, k=35)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`.

Prior-report cluster indices in this slice: **35, 75, 115, 155, 195** (10 member rows total; all member paths were explicit in the report—no `(+N more)` recovery needed).

## Re-measure summary (direct `corpus_runner`)

| Class | Count |
| ---: | ---: |
| fixed on PIN | 9 |
| still broken on PIN | 1 |
| slice rows | 10 |

After re-clustering **still-broken** rows on PIN by first diagnostic: **1 cluster** (1 row).

## PIN clusters (still broken)

| Rows | Kind | Diagnostic on PIN | Witness | Breaking commit | PR | Mechanism (one line) | Status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 1 | pass/run rejected | cannot prove requires contract for call drop from main: retained in Capacity::Held in state `entry` | `pass/ownership/registration_owned_claim_retains_provenance_exit` | `e663bd0afca4cd473dd7218090046457da6f9207` | 1911 | Psi call checking now surfaces unmet `requires` alongside exact-arithmetic overflow and related contract validation, so the `drop` call’s `Capacity::Held` qualification no longer checks as `checked`. | bisected |

**Members (pin-cluster-0)**

- `pass/ownership/registration_owned_claim_retains_provenance_exit` — still broken

**Prior slice clusters now fixed on PIN (no bisect; ledger expectation met)**

| Prior index | Rows | Witness | Fixed | Still broken |
| ---: | ---: | --- | ---: | ---: |
| 35 | 5 | `fail/arithmetic/bounded_assignment_unproven` | 5 | 0 |
| 75 | 2 | `fail/providers/assumed_result_bound_equal_count_lifetime_mapping_rejected` | 2 | 0 |
| 115 | 1 | `pass/facts/target_family_contract_operand/match_join` | 1 | 0 |
| 155 | 1 | `pass/ownership/registration_owned_claim_retains_provenance_exit` | 0 | 1 |
| 195 | 1 | `fail/float/primitive_comparison_no_named_helper_exposure` | 1 | 0 |

**Member lines (prior index 35)**

- `fail/arithmetic/bounded_assignment_unproven` — fixed on PIN
- `fail/arithmetic/guarded_read_count_exceeds_target` — fixed on PIN
- `fail/arithmetic/guarded_read_count_unguarded_arrival` — fixed on PIN
- `fail/arithmetic/wrapping_guard_does_not_bound_original` — fixed on PIN
- `fail/range/indexed_field_range_runtime_write_rejected` — fixed on PIN

**Member lines (prior index 75)**

- `fail/providers/assumed_result_bound_equal_count_lifetime_mapping_rejected` — fixed on PIN
- `fail/providers/assumed_result_bound_lifetime_mapping_rejected` — fixed on PIN

**Member lines (prior index 115)**

- `pass/facts/target_family_contract_operand/match_join` — fixed on PIN

**Member lines (prior index 155)**

- `pass/ownership/registration_owned_claim_retains_provenance_exit` — still broken on PIN

**Member lines (prior index 195)**

- `fail/float/primitive_comparison_no_named_helper_exposure` — fixed on PIN

## Bisect notes (corrected methodology)

- Frozen oracle: PIN `tests/omega/corpus_outcomes.txt` copied to `/tmp/ledger_bisect_pin_corpus_outcomes.txt`; witness PIN observation in `/tmp/ledger_bisect_witness_meta.json` (from `.agents/ledger-bisect-2026-11-11/work/pin_observed.txt` via `pin_bisect_setup.py`).
- Test script: `bisect_witness.sh` + `bisect_test.py` (0 = expected `checked`, 1 = PIN breakage + first-diagnostic class, 125 = skip/build/other).
- Ledger-only commits skipped in-script when `git diff --name-only HEAD^ HEAD` touches only `tests/omega/corpus_outcomes.txt` and/or `*.md`.
- **Good commit** (walk / probe with frozen oracle): `b1d6dad3c52a15788391b5e025730dc15493abbd` (first-parent ancestor; witness reaches PIN-expected `checked`).
- **Bad end:** `741270df60f7` (known broken at triage; ancestor of PIN).
- **First bad** (binary search on compiler-touching commits, `b1d6dad3..741270df`, first-parent): `e663bd0afca4cd473dd7218090046457da6f9207` (#1911) — **not** the PIN; ancestor of both `741270df60f7` and PIN.
- Discarded earlier bisect: ledger-at-revision methodology landed on `b1d6dad3` (#1276), which is a **false** break for this witness under the PIN oracle (parent checks `checked` throughout `b1d6dad3..741270df` except post-#1911 history).
- **Stability:** During search, `e663bd0` returned `1` on three consecutive probes (`first_bad_run.txt`). Repeat runs on detached SHAs sometimes return `125` when `mbx build -p omega` fails quickly on a cold checkout (shared `target/`); PIN breakage itself is deterministic via direct batch measurement in `pin_observed.txt`.

## Commands

Host: linux x86_64 cloud agent VM. `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`, `nice -n 19`.

```bash
# PIN batch (10 slice fixtures)
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx test -p compiler --no-run corpus_runner < /dev/null
RUNNER=$(ls -t target/debug/build/compiler/*/out/corpus_runner-* | head -1)
export OMEGA_CORPUS_ROOT=$PWD OMEGA_LIBRARY_ROOT=$PWD/source/library
export OMEGA_CORPUS_FIXTURE_FILTER='fail/arithmetic/bounded_assignment_unproven,...'
export OMEGA_CORPUS_RECORDS=.agents/ledger-bisect-2026-11-11/work/pin_observed.txt
"$RUNNER" < /dev/null

# Bisect setup
export BISECT_WITNESS=pass/ownership/registration_owned_claim_retains_provenance_exit
python3 .agents/ledger-bisect-2026-11-11/tools/pin_bisect_setup.py "$BISECT_WITNESS" .agents/ledger-bisect-2026-11-11/work/pin_observed.txt

# First-bad search (PIN-frozen oracle; skips ledger-only commits)
python3 .agents/ledger-bisect-2026-11-11/tools/find_first_bad.py \
  b1d6dad3c52a15788391b5e025730dc15493abbd 741270df60f7 \
  .agents/ledger-bisect-2026-11-11/work/first_bad.log
```

Branch head at report time: `work/ledger-bisect-part-35` (see `git rev-parse HEAD` after push).
