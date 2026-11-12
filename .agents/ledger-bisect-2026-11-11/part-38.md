# Ledger bisect part 38 (`k=38`, `N=40`)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`. Prior report cluster indices assigned on this slice: **38, 78, 118, 158, 198** (5 prior clusters, 10 member rows). Host: linux x86_64. Jobs: `OMEGA_CORPUS_JOBS=1`.

## Re-measure summary (PIN, direct `corpus_runner`, ledger at PIN fixed in `/tmp`)

| Class | Count |
| --- | ---: |
| fixed on main (slice members) | 2 |
| still broken (slice members) | 8 |
| prior clusters fully fixed | 2 (indices 118, 158) |

Fixed on main:

- `pass/facts/target_family_callback_guarantee/field`
- `pass/dependent/runtime_symbolic_slice_cell_theorem_exit`

Bisect uses fixed copies: `/tmp/ledger_bisect_corpus_outcomes.txt`, `/tmp/ledger_bisect_witness_pin.json`. Ledger-only commits skipped (`tests/omega/corpus_outcomes.txt` and `*.md` only). Good commits chosen by first-parent walk / bisect end verification (`7f2341031b3` or `55f7abfdbc9` / parent of `2824a5bc`).

## Clusters still broken on PIN (re-clustered by first diagnostic at PIN)

| rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 5 | pass/run regression | `state _ receives _ while local borrow _ is still active` (norm) | `pass/borrows/nested_reference_call_checked_index_exit` | `7523e291b2cb` | 1465 | Psi nested-reference execution retains descriptor/loan state so nested field receives are rejected while an outer `entry` local borrow remains active. | bisected |
| 2 | pass/run regression | generated generic application does not match its exact retained instance origin | `pass/build/const_index_dependency_order_exit` | `2824a5bc57a8` | — | Case-pattern / saved-subject dispatch and generic initializer dependency paths tighten retained generic instance origins (`initializer_dependencies`, `data_applications`). | bisected |
| 1 | pass/run regression | provider completion … disagrees with its exact retained requirement contract | `pass/progress/provider_receiver_progress_installation` | `0948d66c9cda` | 1299 | Canonical calling declarations through native execution change provider completion vs retained requirement contracts. | bisected |

### Cluster `c1` members (borrow while local active)

- `pass/borrows/nested_reference_call_checked_index_exit` — broken — rejected — `state read_reference receives value.inner.value while local borrow entry is still active`
- `pass/facts/case_match_payload_loan_through_other_machine_exit` — broken — rejected — `state store receives <saved subject #4:1>::Active.message.body while local borrow entry is still active`
- `pass/facts/case_match_payload_loan_through_other_state_exit` — broken — rejected — same family
- `pass/facts/case_match_payload_loan_through_parameter_exit` — broken — rejected — same family
- `pass/facts/case_match_payload_loan_through_target_body_exit` — broken — rejected — same family

Good: `7f2341031b3c8e990511afad3143e2cc9e77e8b7`. Bad: PIN. Sanity: `741270df60f7` → bad (1); good end → pass (0); deterministic on 2 trials each.

### Cluster `c2` members (retained generic instance origin)

- `pass/build/const_index_dependency_order_exit` — broken — rejected — `generated generic application does not match its exact retained instance origin`
- `pass/generics/runtime_computed_nested_const_call_exit` — broken — rejected — same diagnostic

Good: `55f7abfdbc9a3a82ae71b13ca57cc7c5b730a32b` (parent of `2824a5bc`). Bad: PIN.

### Cluster `c3` members (provider completion)

- `pass/progress/provider_receiver_progress_installation` — broken — rejected — provider completion disagrees with exact retained requirement contract

Good: `55f7abfdbc9a3a82ae71b13ca57cc7c5b730a32b`. Bad: PIN.

## Prior-report clusters now fixed (no bisect)

| prior idx | witness | rows | note |
| ---: | --- | ---: | --- |
| 118 | `pass/facts/target_family_callback_guarantee/field` | 1 | checked at PIN (meets ledger) |
| 158 | `pass/dependent/runtime_symbolic_slice_cell_theorem_exit` | 1 | checked at PIN (meets ledger) |

## Commands

```bash
cd /agent/repos/omega
git checkout work/ledger-bisect-part-38   # at PIN
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null
CARGO_TARGET_DIR=$PWD/target mbx test -p compiler corpus_runner --no-run < /dev/null
# direct measure (10 slice members):
export OMEGA_CORPUS_ROOT=$PWD OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_JOBS=1
export OMEGA_CORPUS_FIXTURE_FILTER='pass/borrows/nested_reference_call_checked_index_exit,...'
export OMEGA_CORPUS_RECORDS=.agents/ledger-bisect-2026-11-11/pin_measure.txt
RUNNER=$(ls -t target/debug/build/compiler/*/out/corpus_runner-* | head -1)
nice -n 19 "$RUNNER" < /dev/null
# per-cluster bisect:
.agents/ledger-bisect-tools/prepare_bisect_witness.sh <witness> 51e8c45c793
export BISECT_WITNESS=<witness>
git bisect start --first-parent 51e8c45c793007f211d0d9baee7de16767172444 <good>
git bisect run .agents/ledger-bisect-tools/bisect_witness.sh
git bisect reset
```

Logs: `.agents/ledger-bisect-2026-11-11/bisect_c{1,2,3}.log`.
