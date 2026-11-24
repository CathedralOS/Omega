# File collisions between concurrent changes

## Answer

Yes. Commits that land close together usually touch the same files. Of 2,915 ordinary commits, 2,441 (84%) changed a code file that another commit changed on the same day, and 1,936 (66%) shared a code file with a commit that landed within one hour of it. Considered one pair at a time, the overlap is much rarer: 11% of same-day pairs share a code file (35,904 of 341,866). Because those rare overlaps add up as more commits run together, a random draw of 2 same-day commits is code-disjoint 90% of the time, 4 commits 63%, 6 commits 37%, and 10 commits 12% (5,000 draws per size). The overlaps come from a few shared files (`OWNER_CODE_CONCERNS.md` in 603 commits, `TASKS.md` 407, `Cargo.lock` 278) and from bursts on a single stage. One example is 87 commits on 2026-11-20 that all touched the same bootstrap test scripts. A shared file is not necessarily a merge conflict, and these tables cannot show which shared files actually conflicted.

## Measurements

"Ordinary" commits are the 2,915 commits that changed between 1 and 100 files. The 95 bulk commits (more than 100 files each) are left out of every table below. They hold 248,426 of the 281,057 file rows (88%). By lane they are psi 28, compiler 18, samples 9, repo 8, tests 7, bootstrap 5, and 20 spread over 11 other lanes. Six commits had no file rows.

"Document" means `TASKS.md`, `AGENTS.md`, `CLAUDE.md`, root `OWNER_*.md`, anything under `wiki/`, and any file whose name starts with `README`. Every other file counts as "code", which includes tests, config, `Cargo.lock` and other `.md` files.

### Files changed by the most commits (top 30, ordinary commits)

| # | file | commits | days (of 14) | kind |
|---|---|---|---|---|
| 1 | `OWNER_CODE_CONCERNS.md` | 603 | 12 | doc |
| 2 | `TASKS.md` | 407 | 14 | doc |
| 3 | `Cargo.lock` | 278 | 14 | code |
| 4 | `tests/omega/corpus_outcomes.txt` | 144 | 3 | code |
| 5 | `OWNER_QUESTIONS.md` | 119 | 14 | doc |
| 6 | `tools/xtask/src/source_closure.rs` | 104 | 5 | code |
| 7 | `tools/bootstrap/omega/compiler_env.sh` | 97 | 7 | code |
| 8 | `tests/bootstrap/interpreted/omega-executable/gate.py` | 94 | 3 | code |
| 9 | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/machine.rs` | 94 | 5 | code |
| 10 | `tests/bootstrap/interpreted/gate_lib.py` | 88 | 3 | code |
| 11 | `Cargo.toml` | 86 | 11 | code |
| 12 | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/calls.rs` | 85 | 5 | code |
| 13 | `bootstrap/omega/compiler/README.md` | 82 | 3 | doc |
| 14 | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower/places.rs` | 82 | 5 | code |
| 15 | `bootstrap/omega/compiler/omega_compiler.epsilon.sources` | 81 | 3 | code |
| 16 | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lib.rs` | 80 | 5 | code |
| 17 | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read.rs` | 79 | 5 | code |
| 18 | `omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs` | 66 | 5 | code |
| 19 | `bootstrap/omega/compiler/scalar_compilation.epsilon` | 59 | 3 | code |
| 20 | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/expressions.rs` | 59 | 5 | code |
| 21 | `omega-rust/psi/compilation/pipeline/lowered-discharge/src/tests.rs` | 59 | 4 | code |
| 22 | `source-rust/subsystems/compiler/source-to-psi/Cargo.toml` | 54 | 5 | code |
| 23 | `tools/xtask/src/boundary.rs` | 54 | 10 | code |
| 24 | `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptors.rs` | 54 | 7 | code |
| 25 | `AGENTS.md` | 53 | 14 | doc |
| 26 | `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/plain_affine.rs` | 53 | 7 | code |
| 27 | `omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/lib.rs` | 51 | 5 | code |
| 28 | `omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs` | 51 | 4 | code |
| 29 | `omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests.rs` | 51 | 5 | code |
| 30 | `wiki/spec/terminal-psi/encoding.md` | 50 | 12 | doc |

The Rust tree moved from `omega-rust/` to `source-rust/` on 2026-11-18. Before that day, every file row is under `omega-rust/`; after it, every row is under `source-rust/`. File counts do not follow the rename, so a file that changed in both halves of the window appears twice and each entry is under-counted. The `omega-rust/` files above were active only from 11-13 to 11-17.

### Directories under `source-rust/` (depth 4, 5, 6; top 10 each; covers 2026-11-18 to 11-23 only)

| depth | directory | commits | days |
|---|---|---|---|
| 4 | `source-rust/subsystems/compiler/source-to-psi` | 378 | 6 |
| 4 | `source-rust/subsystems/compiler/psi-to-native` | 187 | 6 |
| 4 | `source-rust/subsystems/compiler/build` | 154 | 6 |
| 4 | `source-rust/subsystems/compiler/psi` | 117 | 6 |
| 4 | `source-rust/subsystems/compiler/tests` | 53 | 5 |
| 4 | `source-rust/subsystems/package-manager/review` | 49 | 6 |
| 4 | `source-rust/subsystems/compiler/foundation` | 43 | 6 |
| 4 | `source-rust/application/tests/fixtures` | 39 | 4 |
| 4 | `source-rust/subsystems/interpreter/host` | 34 | 5 |
| 4 | `source-rust/subsystems/interpreter/embedding` | 21 | 5 |
| 5 | `.../source-to-psi/pipeline` | 328 | 6 |
| 5 | `.../psi-to-native/pipeline` | 153 | 6 |
| 5 | `.../source-to-psi/representations` | 105 | 5 |
| 5 | `.../psi/semantics` | 76 | 5 |
| 5 | `.../psi-to-native/verification` | 66 | 6 |
| 5 | `.../build/planning` | 48 | 5 |
| 5 | `.../psi-to-native/representations` | 44 | 5 |
| 5 | `.../build/tests` | 44 | 5 |
| 5 | `.../source-to-psi/_recycling` | 41 | 2 |
| 5 | `.../source-to-psi/driver` | 40 | 4 |
| 6 | `.../source-to-psi/pipeline/06_lowered-program-to-checked-program` | 150 | 6 |
| 6 | `.../source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program` | 119 | 5 |
| 6 | `.../source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees` | 73 | 6 |
| 6 | `.../psi/semantics/src` | 65 | 5 |
| 6 | `.../source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees` | 57 | 6 |
| 6 | `.../source-to-psi/representations/typed-trees` | 57 | 5 |
| 6 | `.../psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program` | 52 | 6 |
| 6 | `.../source-to-psi/pipeline/07_checked-program-to-psi` | 50 | 6 |
| 6 | `.../psi-to-native/pipeline/representations` | 49 | 2 |
| 6 | `.../psi-to-native/verification/provider-admission` | 41 | 6 |

`...` stands for `source-rust/subsystems/compiler`. The 15-row versions are in the script output.

### Per day: share of commits that changed a file another commit changed the same day

| day | commits | any file | a code file | documents only | any document |
|---|---|---|---|---|---|
| 2026-11-10 | 57 | 93% | 61% | 32% | 46% |
| 2026-11-11 | 256 | 97% | 83% | 14% | 41% |
| 2026-11-12 | 206 | 88% | 68% | 19% | 47% |
| 2026-11-13 | 237 | 97% | 91% | 6% | 60% |
| 2026-11-14 | 336 | 99% | 88% | 10% | 73% |
| 2026-11-15 | 313 | 97% | 86% | 12% | 71% |
| 2026-11-16 | 165 | 90% | 79% | 12% | 60% |
| 2026-11-17 | 130 | 93% | 80% | 13% | 40% |
| 2026-11-18 | 214 | 85% | 77% | 8% | 42% |
| 2026-11-19 | 239 | 96% | 94% | 2% | 31% |
| 2026-11-20 | 299 | 96% | 84% | 12% | 68% |
| 2026-11-21 | 172 | 90% | 87% | 3% | 19% |
| 2026-11-22 | 149 | 91% | 84% | 7% | 42% |
| 2026-11-23 | 142 | 93% | 87% | 6% | 34% |
| **all** | **2,915** | **2,739 (94%)** | **2,441 (84%)** | **298 (10%)** | **1,497 (51%)** |

Column definitions:
- "a code file": the commit shared at least one code file with another commit that day.
- "documents only": every shared file was a document.
- "any document": at least one shared file was a document, whether or not a code file was also shared.

Results for narrower time windows and for pairs of commits:

| measure | result |
|---|---|
| Shares a code file with a commit landed within ±1 h | 1,936 / 2,915 (66%); median 25 other landings in that window |
| ... within ±3 h | 2,260 / 2,915 (78%); median 76 other landings |
| ... within ±6 h | 2,376 / 2,915 (82%); median 129 other landings |
| Same as the day figure, but the top 10 code files do not count | 2,381 / 2,915 (82%) (day figure: 84%) |
| Same-day commit pairs that share a code file | 35,904 / 341,866 (11%) |
| Commits whose work time overlaps another commit's work time | 1,982 / 1,994 (99%) |
| ... and that other commit changed one of the same code files | 1,312 / 1,994 (66%) |
| Landings per clock hour (hours with at least one landing) | median 7, max 63, over 311 hours |

A commit's work time is the period from its landing time minus its provenance `Time` back to its landing time. Each commit overlapped a median of 71 others (p90 162).

### Commits per day touching each of the top 10 code files

| file | 11-10 | 11-11 | 11-12 | 11-13 | 11-14 | 11-15 | 11-16 | 11-17 | 11-18 | 11-19 | 11-20 | 11-21 | 11-22 | 11-23 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `Cargo.lock` | 4 | 7 | 16 | 22 | 9 | 24 | 16 | 4 | 38 | 103 | 13 | 12 | 8 | 2 |
| `tests/omega/corpus_outcomes.txt` | 24 | 113 | 7 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `tools/xtask/src/source_closure.rs` | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 9 | 5 | 87 | 0 | 0 | 2 |
| `tools/bootstrap/omega/compiler_env.sh` | 0 | 1 | 7 | 1 | 0 | 0 | 0 | 0 | 6 | 4 | 76 | 0 | 0 | 2 |
| `tests/bootstrap/interpreted/omega-executable/gate.py` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 5 | 87 | 0 | 0 | 2 |
| `omega-rust/.../05_typed-trees-to-terminal-psi/src/read/machine.rs` | 0 | 0 | 0 | 14 | 28 | 30 | 11 | 11 | 0 | 0 | 0 | 0 | 0 | 0 |
| `tests/bootstrap/interpreted/gate_lib.py` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 4 | 82 | 0 | 0 | 2 |
| `Cargo.toml` | 3 | 1 | 2 | 3 | 1 | 4 | 0 | 0 | 21 | 44 | 2 | 3 | 2 | 0 |
| `omega-rust/.../05_typed-trees-to-terminal-psi/src/read/calls.rs` | 0 | 0 | 0 | 11 | 32 | 24 | 6 | 12 | 0 | 0 | 0 | 0 | 0 | 0 |
| `omega-rust/.../05_typed-trees-to-terminal-psi/src/lower/places.rs` | 0 | 0 | 0 | 5 | 28 | 34 | 8 | 7 | 0 | 0 | 0 | 0 | 0 | 0 |

Most of these files are busy for one to three days and then quiet. `Cargo.lock` is the exception: it changed on all 14 days and peaked at 103 commits on 11-19, the day after the tree moved. The 87 commits on 11-20 that touched `gate.py` came mostly from the `bootstrap-omega` lane: 66 of the 94 commits that touched that file over the window were Codex commits in that lane.

### Spread: how many crates or directories one commit touches

A "unit" is the nearest Cargo crate directory at HEAD (247 crates, `_recycling/` and the root manifest excluded). For a file outside any crate, the unit is its first two path components, or the top-level folder for files one level deep, or `(root)` for files at the root.

| units per commit | all files | code files only (document-only commits excluded) |
|---|---|---|
| 1 | 881 / 2,915 (30%) | 1,119 / 2,662 (42%) |
| 2 | 864 (30%) | 705 (26%) |
| 3 | 464 (16%) | 345 (13%) |
| 4 | 346 (12%) | 227 (9%) |
| 5 or more | 360 (12%) | 266 (10%) |

1,614 of 2,915 commits (55%) touch at least one document, 407 (14%) touch `TASKS.md`, and 253 touch only documents.

### Which lanes collide with which (same-day pairs sharing a code file, top 15)

| lane A | lane B | colliding pairs | same-day pairs | rate |
|---|---|---|---|---|
| psi | psi | 12,462 | 74,289 | 16.8% |
| compiler | psi | 3,879 | 27,293 | 14.2% |
| psi | tests | 2,414 | 7,858 | 30.7% |
| bootstrap-omega | bootstrap-omega | 1,942 | 2,045 | 95.0% |
| build | psi | 1,242 | 9,494 | 13.1% |
| (none) | psi | 887 | 8,971 | 9.9% |
| compiler | compiler | 770 | 3,613 | 21.3% |
| backend | psi | 732 | 8,293 | 8.8% |
| (none) | bootstrap-omega | 681 | 1,771 | 38.5% |
| omega | psi | 615 | 14,068 | 4.4% |
| tests | tests | 575 | 730 | 78.8% |
| bootstrap | bootstrap-omega | 528 | 2,399 | 22.0% |
| (none) | build | 447 | 1,717 | 26.0% |
| compiler | omega | 430 | 3,141 | 13.7% |
| omega | omega | 337 | 864 | 39.0% |

`(none)` means the title had no lane prefix, which happens when the title uses a lane name outside the table. Half of all colliding pairs (17,808 of 35,904) are within one lane. Lane labels separate work poorly. psi pairs with omega share code files 4.4% of the time and psi pairs with backend 8.8%, but pairs within psi 16.8%, within tests 78.8% and within bootstrap-omega 95%.

### How many commits can run at once without sharing a file

Each draw picks a random day, then k distinct ordinary commits that landed on that day. A draw counts if no file appears in more than one of the k commits. 5,000 draws per k, seed 20261123; all 14 days have at least 20 commits.

| k commits at once | no shared code file | no shared code file, top 10 code files ignored | no shared file of any kind |
|---|---|---|---|
| 2 | 90% | 94% | 80% |
| 3 | 76% | 84% | 58% |
| 4 | 63% | 74% | 41% |
| 5 | 48% | 62% | 27% |
| 6 | 37% | 50% | 18% |
| 8 | 21% | 32% | 7% |
| 10 | 12% | 20% | 3% |
| 15 | 1% | 3% | 0% |
| 20 | 0% | 1% | 0% |

This table answers how many agents can work at once. With work picked the way it was picked in these two weeks, there is about an even chance that 5 simultaneous changes avoid every shared code file. Counting documents too, that even chance holds for only about 3. Ignoring the 10 hub files raises the even-chance point by about one commit, to 6. So the overlaps are spread across many stage files, not concentrated in a few hubs.

## How this was computed

Script: `<tables>/reports/collisions.py`. It reads `../commits.tsv` and `../files.tsv`, plus `crates.txt` beside the script. Produce `crates.txt` with `git -C /home/user/Omega ls-files '*Cargo.toml' | grep -v ^_recycling`. Run it with `python3 collisions.py`; its full output is in `collisions_out.txt`.

- **Population.** All 3,016 first-parent commits in the extract. A commit with more than 100 changed files is bulk; there are 95, and they are reported separately and excluded everywhere else. 6 commits with no file rows are also dropped. That leaves 2,915 ordinary commits. 100 is a judgement call: 190 commits change more than 50 files, 95 more than 100, 60 more than 200 and 34 more than 500.
- **Files.** A file is identified by its exact path string. Renames are not followed.
- **Same day.** Same value in the `day` column.
- **Time windows.** The ±W-hour windows use the commit `date`, which is the landing time on main.
- **Work time.** For the 1,994 commits with a parsed `minutes` value, the interval is [landing time − minutes, landing time]. Two commits overlap when their intervals intersect.
- **Document test.** Path starts with `wiki/`; or the file name starts with `README` (any case); or the path is `TASKS.md`, `AGENTS.md` or `CLAUDE.md`; or the file sits at the root with a name matching `OWNER_*.md`.
- **Directory depth.** Depth d means the first d path components, for example `source-rust/subsystems/compiler/source-to-psi` at depth 4. A commit counts once per directory, and only files deeper than d count.
- **Lane-pair counts.** These count unordered pairs of same-day commits. A pair collides when a non-document file of one commit is also among the files of the other.
- **Sampling.** The k-commit draws use `random.Random(20261123)`, with 5,000 draws for each k in {2,3,4,5,6,8,10,15,20}, run in that order with the generator shared across k. The "top 10 code files ignored" column removes the ten code files changed by the most ordinary commits: `Cargo.lock`, `tests/omega/corpus_outcomes.txt`, `tools/xtask/src/source_closure.rs`, `tools/bootstrap/omega/compiler_env.sh`, the two `tests/bootstrap/interpreted` scripts, `Cargo.toml`, and three `omega-rust/.../05_typed-trees-to-terminal-psi` files.
- **What was read.** No diffs were read. Everything comes from the two TSV tables, plus one HEAD file listing for the crate directories.

## What this does not show

- **Whether a shared file was a merge conflict.** Without diffs, the data cannot tell whether two commits edited the same lines or different functions, or whether one appended a line to `OWNER_CODE_CONCERNS.md` or `TASKS.md` while the other did the same. Generated files like `Cargo.lock` usually re-resolve automatically, and appends to a list may merge cleanly. These numbers are an upper bound on file-level contention, not a count of conflicts.
- **When work actually ran.** Landing time is not start time. The ±W-hour windows and same-day buckets are proxies. The provenance `Time` is active agent time, and some values are implausibly large (one commit overlaps 1,588 others), so those overlaps are rough. The data has no PR-open times.
- **Counts across the rename.** The `omega-rust/` → `source-rust/` move on 11-18 splits each file's history. The source-rust directory tables cover 6 days, and per-file counts for older Rust files cover the days before the move. Other renames inside the window also break counts.
- **Bulk commits.** The 95 bulk commits are excluded. They are likely moves, formatting or generated updates, and they touch most files at once. Any change in flight while one lands probably overlaps it, which these tables do not count.
- **Other ways of choosing work.** The k-commit table reflects how work was chosen in these 14 days, when many agents were sent to the same stage or the same bootstrap scripts at once. It does not predict what would happen if work were picked to avoid overlap.
- **Lanes.** Lane comes from the title prefix. 95%-collision lanes like `bootstrap-omega` may reflect one long serial effort split into many small PRs, not independent agents racing.

## Check

A second analyst recomputed the numbers from `commits.tsv` and `files.tsv` with separate code (`check/coll_check.py`, `check/coll_check2.py`, `check/coll_check3.py` in the `scale/` scratchpad folder). No diffs were read. Every headline count reproduces exactly. The problem is what the counts mean: most same-day collisions are between commits that ran one after another, not at the same time, so the same-day figures overstate how often concurrent agents share a file.

### Reproduced exactly

- Population: 3,016 commits, 6 with no file rows, 95 with more than 100 files holding 248,426 of 281,057 rows, 2,915 ordinary. The `files` column agrees with the row count for every commit, so no rows were lost.
- Same-day sharing: 2,441 / 2,915 commits (84%) share a code file, and 2,739 (94%) share a file of any kind. With the top 10 code files ignored, 2,381 (82%).
- Within ±1 h: 1,936 / 2,915 (66%), with a median of 25 other landings. Within ±3 h: 2,260 (78%).
- Same-day pairs: 35,904 / 341,866. That is 10.5%, which the report rounds to 11%.
- Top files: `OWNER_CODE_CONCERNS.md` 603 commits over 12 days, `TASKS.md` 407 over 14, `Cargo.lock` 278 over 14, `corpus_outcomes.txt` 144 over 3, `OWNER_QUESTIONS.md` 119 over 14, `source_closure.rs` 104 over 5.
- Directories: `source-to-psi` 378 commits; `06_lowered-program-to-checked-program` 150 and `05_prepared-...` 119.
- Lane pairs: psi–psi 12,462 / 74,289, compiler–psi 3,879 / 27,293, psi–tests 2,414 / 7,858, bootstrap-omega within itself 1,942 / 2,045, omega–psi 615 / 14,068, tests within itself 575 / 730. 17,808 colliding pairs fall within one lane.
- `gate.py`: 87 commits on 11-20, and 66 of its 94 commits over the window are Codex commits in the bootstrap-omega lane.
- k-commit draws, rerun with seed 777 and 5,000 draws per k. Share with no shared code file: k=2 89.9%, k=3 76.8%, k=5 48.9%, k=10 10.9%. Counting documents: k=3 59%, k=5 27%. These match the report within sampling error.
- Work-time overlap: 1,982 / 1,994 commits overlap another commit's work time, and 1,312 / 1,994 overlap one that shares a code file.

### What differs or cannot be carried

1. **Same day mostly means one after another.** I used the provenance `Time` to give each commit a start time: start = landing time minus `Time`. Among same-day colliding pairs where both commits have a `Time`, 17,454 of 20,964 (83%) did not overlap at all. The second commit started after the first had landed. Only 3,510 (17%) ran at the same time. The headline's 84% and the k-commit table therefore mostly measure the same files being edited one after another, not agents running at once. `Time` is active agent time, so the real wall-clock span is longer and some of these pairs did overlap. 17% is a lower bound on concurrent pairs, not an exact figure.
2. **Commits that did run at once share files about as often per pair, and more often in groups.** I kept commits whose `Time` is at most 24 h (1,815 of 2,915) and took every pair whose work time overlapped. 2,001 of those 17,298 pairs (11.6%) share a code file, close to the report's 11% for same-day pairs. I then picked 20,000 random instants (seed 4242), took the commits active at each instant, and drew k of them. The draw had no shared code file in 76% of cases for k=2, 52% for k=3 and 25% for k=5. The report's same-day draws give 90%, 76% and 48%. Commits that actually ran together were more often on the same stage, so the report's "about 5 at even chance" is optimistic: among commits that were running at the same instant, the even-chance point is about 3. A median of 4 commits were active at a random instant (p90 15). This rests on the `Time` field and is an estimate.
3. **The `Time` denominator drops mostly Codex.** 921 of 2,915 ordinary commits (32%) have no `Time` value, and 832 of those are Codex commits. Any work-time figure, the report's 1,994-commit figures included, mostly leaves out Codex. `Time` is also implausible at the top: p90 773 min, p99 42,724 min, max 43,299 min (about 30 days). The report notes the extreme overlap counts but not this skew in the denominator.
4. **The headline names the wrong hubs.** It says "the shared files are a few hubs (`OWNER_CODE_CONCERNS.md` 603, `TASKS.md` 407, `Cargo.lock` 278)". The first two are documents, and the report itself excludes documents from the 84% code figure. The report's own finding is that removing the 10 busiest code files moves 84% only to 82%. The code overlap is therefore spread across stage source files. The named hubs explain the any-file figure (94%), not the code figure.
5. **A test-results file counts as "code".** `tests/omega/corpus_outcomes.txt` (144 commits, all on 11-10 to 11-12) is a list of test outcomes that commits added rows to. That directory was deleted by the owner's ruling of 2026-11-13, so this hub no longer exists. I sampled 30 colliding pairs with overlapping work time (seed 99) and read their titles and shared files. 10 of the 30 shared only `corpus_outcomes.txt`, 1 only `Cargo.lock`, and the other 19 shared Rust source files in one compiler stage (for example `05_typed-trees-to-terminal-psi/src/read/machine.rs` or `lower/places.rs`). Across all ordinary commits, 155 of the 2,441 code-colliding commits collided only through `Cargo.*`, `.md` or `.txt` files. This sample has only 30 pairs.
6. **Bootstrap lanes are not the cause.** Without the bootstrap, bootstrap-omega and Greek-letter lanes, 2,269 / 2,736 commits (83%) still share a code file the same day. The 95% bootstrap-omega rate does not drive the headline.
7. **Minor point about the move.** The report says that after 11-18 every row is under `source-rust/`. On 11-18 itself, 89 commits still changed 5,104 `omega-rust/` rows. No row is under `source-rust/` before 11-18. The directory table is unaffected, because it counts only `source-rust/` paths.

I did not recompute the per-unit spread table or the per-day table row by row. The per-day all-days totals match.

### What the check supports

The counts are right. Their reading as concurrency is not. The same files are clearly edited by many commits in short bursts. But at least 83% of same-day colliding pairs ran one after another, and among commits that ran at once, about 3 random ones (not 5) have an even chance of sharing no code file. Neither the report nor this check can show whether a shared file caused a merge conflict.
