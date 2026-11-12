# Ledger bisect part 32 (k=32, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`.

## Slice summary (prior report clusters 32, 72, 112, 152, 192)

| Metric | Count |
| --- | ---: |
| Slice members (unique) | 11 |
| Fixed on PIN | 7 |
| Still broken on PIN | 4 |
| Still-broken clusters (re-grouped by PIN first diagnostic) | 2 |

Classify rule: `classify_corpus.py` on `/tmp/part32_pin_observed.txt` vs slice member list. Witness test uses fixed PIN ledger + PIN observation (`prepare_witness.sh`); good commit located by walking first-parent until exit 0 (`20fd485ca46` for recoverable; `3bbd06849b998f4fef4ccdffb8ef1f6059c07def` for foreign-retained). `git bisect run` skips ledger-only first-parent commits; each step rebuilds via `target/bisect-pool/`.

## Clusters (still broken on PIN)

| rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | result |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 2 | pass/run rejected | machine `swap_pending` stores a borrowed value without a complete normal-exit backing relation | `pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap` | `4faf6413467dd7a7f6cf7e62ba77d1c607591f18` | 1718 | replay-time per-write invalidation re-checks retained borrow storage and rejects normal-exit backing on foreign swap_pending/store_pending | bisected |
| 2 | pass/run rejected | `main` calls `forward`, which calls `repair`; `repair` has no executable body (structural operand custody) | `pass/facts/recoverable_replacement_window_through_other_machine_exit` | `f6b76a817aa80d8e4e986e2ff2fc9bfbbe4e3c12` | 220 | compiler preserves completed call and borrowed descriptor identities and the repair assignment no longer lowers | bisected |

### Cluster 0 members (`machine stores borrowed value…`)

- `pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap` — broken, rejected
- `pass/facts/foreign_retained_shared_borrow_persistent_restore` — broken, rejected

### Cluster 1 members (`repair has no executable body…`)

- `pass/facts/recoverable_replacement_window_through_call_result_exit` — broken, incomplete
- `pass/facts/recoverable_replacement_window_through_other_machine_exit` — broken, incomplete

## Fixed on PIN (7)

- `fail/borrows/dynamic_store_local_backing_escape`
- `fail/borrows/indexed_carrier_state_local_escape`
- `fail/facts/foreign_retained_shared_borrow_borrowed_slot_swap`
- `fail/facts/foreign_retained_shared_borrow_persistent_restore`
- `fail/termination/lexicographic_component_zero_step_rejected`
- `pass/facts/target_family_contract_operand/const_name`
- `pass/recast/runtime_captured_guard_conjunction_exit`

## Bisect sanity

- `3bbd06849b9` / `20fd485ca46`: witness exit 0 (good); triple probe at `3bbd06849b9` for cluster 0: 0/0/0 (deterministic).
- `4faf6413467dd7a7f6cf7e62ba77d1c607591f18`, `741270df60f7`, `51e8c45c793`: cluster 0 witness exit 1 (bad).
- `f6b76a817aa80d8e4e986e2ff2fc9bfbbe4e3c12` is first bad for cluster 1; parent `55c524c09bdeb382f714d48c7c04ab781ff72a8b` is good per bisect.
- Neither breaking commit is ledger-only; both precede `741270df60f7` on `main`.

## Commands

Host: linux/x86_64, 4 CPUs, `OMEGA_CORPUS_JOBS=1`, `CARGO_TARGET_DIR=$PWD/target/bisect-pool`.

```bash
git fetch --unshallow origin
# re-measure slice (direct corpus_runner)
FILTER=$(tr '\n' ',' <.agents/ledger-bisect-2026-11-11/part-32-members.txt)
CARGO_TARGET_DIR=$PWD/target/bisect-pool mbx build -p omega </dev/null
CARGO_TARGET_DIR=$PWD/target/bisect-pool mbx test -p compiler corpus_runner --no-run </dev/null
export OMEGA_CORPUS_ROOT=$PWD OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_JOBS=1
export OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS=/tmp/part32_pin_observed.txt
RUNNER=$(ls -t target/bisect-pool/debug/build/compiler/*/out/corpus_runner-* | head -1)
nice -n 19 "$RUNNER" </dev/null
# cluster 0
export BISECT_WITNESS=pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap
.agents/ledger-bisect-2026-11-11/tools/prepare_witness.sh "$BISECT_WITNESS" 51e8c45c793 /tmp/part32_pin_observed.txt
git bisect start --first-parent 741270df60f7 3bbd06849b9
git bisect run .agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh
# cluster 1
export BISECT_WITNESS=pass/facts/recoverable_replacement_window_through_other_machine_exit
.agents/ledger-bisect-2026-11-11/tools/prepare_witness.sh "$BISECT_WITNESS" 51e8c45c793 /tmp/part32_pin_observed.txt
git bisect start --first-parent 741270df60f7 20fd485ca46
git bisect run .agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh
```
