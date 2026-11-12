# Ledger bisect part 34 (k=34, N=40) — lead review revision

PIN `51e8c45c793007f211d0d9baee7de16767172444`. Frozen ledger: `.agents/ledger-bisect-2026-11-11/pin_corpus_outcomes.txt` (from PIN `corpus_outcomes.txt`).

## Slice (accepted)

| Still broken @ PIN | Fixed on main |
| ---: | ---: |
| **4** | **6** |

One still-broken cluster: incomplete `` `_` has no executable body `` (state graph: natural ranks) across 4 pass fixtures (witness `pass/borrows/runtime_mutable_byte_loan_read_exit`).

## Breaking commit

**Verdict: never-good since `648fa099e29f4477cf58d5f9e881ec894aaaaf6f`** (fixture add: `psi: read bytes through exclusive loans`).

Method: exponential first-parent walk from PIN with frozen ledger (`BISECT_CLASS_ONLY` **unset**); exit **125** for build/library failures and ledger-only commits. No step returned exit **0** (ledger `checked` expectation met). Log: `part34_exp_good_walk.log`.

**Rejected:** class-only bisect + manual good `d4c99a57` (#2303) → `dfda1fe963a` (#2296); per-rev ledger bisect → `157ac65f931` (#2457).

**No** accepted `git bisect run` breaking commit.

## First commit showing current diagnostic class (not a breaking commit)

Harness: `ledger_bisect_run.sh` (rebuild `corpus_runner`; `BISECT_CLASS_ONLY=1` **not** cleared inside the script). Predicate: normalized incomplete diagnostic matches PIN witness (`pin_witness_observation.txt`).

**Recorded label (coarse binary search on buildable witness, range `84c2330b018..51e8c45c7930`):** `68b8fb2781acf14b3a2c0f98f2e21cb294d09eb4` — library: move time host supply into package build (**#1170**). First parent `bfcffe6bcfcedb6fd60713e223408566c6c4e10d` (#1175). Log: `part34_class_boundary.log`. **Not accepted as a precise first-parent edge** under lead rule (2): see below.

**PIN landing edge (3× confirmation attempted):** parent `e2e7876a03e415c6aad2defbbe743e0ec24e7caa` (#2463) **does not** satisfy parent **class exit 0** — observed **ledger=1, class=1** with the same incomplete `` `reverse` `` diagnostic (2 runs; log in repo `part34_triple_verify` capture via `/tmp/part34_triple_verify.log`). PIN `51e8c45c7930` likewise **class=1**. **No parent-0 / first-1 confirmation at the PIN merge edge.**

**Earlier buildable anchor:** at `84c2330b018df161905ec21172e542e4572b1c71` witness is **`checked`** (no PIN-class). Full first-parent class boundary between that `checked` anchor and recent `incomplete` spine was not completed in this run (timeouts / 125s on older parents).

## Cluster check (4 rows @ PIN)

`part34_four_at_pin.log`: witness row **class=1**; sibling rows share the same incomplete callee-body pattern at PIN (`part34_measure.txt` / direct corpus). `BISECT_CLASS_ONLY=1` is wired to the witness `pin_witness_observation.txt` (`` `reverse` `` callee), so sibling fixtures return **125** even when the diagnostic class matches (different callee name).

## Mechanism (#2296)

**Mechanism not established.** No concrete diff link from #2296 float identity-law policy to the borrows witness diagnostic was shown.

## Harness notes

- Rebuild `corpus_runner` each step: `mbx test --no-run -p compiler --test corpus_runner`; resolve `corpus_runner` under `target/debug/build/compiler/*/out/`.
- `OMEGA_LIBRARY_ROOT=$ROOT/source/library` (matches `xtask corpus`).
- Do **not** `unset BISECT_CLASS_ONLY` inside the runner when the caller sets class mode.
- Capture exit codes with `ec=0; …/ledger_bisect_run.sh || ec=$?` (plain `$?` after `|| ec=125` lied).

Host: Linux x86_64, `OMEGA_CORPUS_JOBS=1`, `nice -n 19`.
