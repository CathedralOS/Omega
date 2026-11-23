# How often landed work is repaired by later work

## Answer

About 4% of the 3,016 commits that landed on `origin/main` in the 14 days from 2026-11-10 to 2026-11-23 repair a regression that an earlier landed change introduced. That is roughly 130 commits, with a 95% range of 2% to 8% (70 to 250 commits). About another 4% (2% to 8%) finish work that an earlier change left undone. Counting both, about 8% (5% to 13%) of landed commits re-handle earlier landed work. No commit in the window is a revert or a re-land: no title says so, and no body contains "This reverts commit". Where the cause can be dated, the regressions stayed on main for between 1 and 43 hours, median about 5.6 hours (8 dated cases). Tests and later agents caught them, never the owner. Several of these breaks were test targets in another crate that stopped compiling after an API changed.

## Measurements

### Whole window (estimated from a 100-commit sample of 1,664 keyword candidates)

The keyword filter matched 1,664 of the 3,016 commits. I read 100 of them, chosen at random, and sorted each into one class. A class's rate for the whole window is (its share of the sample) × 1,664 ÷ 3,016. The 95% range is the Wilson interval on the sample share, scaled the same way. These rates assume that commits outside the 1,664 candidates are not repairs (see "What this does not show").

| Class | In sample (of 100) | Est. commits in window | Share of all 3,016 commits (95% range) |
| --- | ---: | ---: | --- |
| R: repairs a regression an earlier landed change introduced | 8 | 133 | 4.4% (2.3%–8.3%) |
| V: revert or re-land | 0 | 0 | 0% (0%–2.0%); 0 of 3,016 titles also say so |
| C: completes work an earlier change left unfinished | 7 | 116 | 3.9% (1.9%–7.6%) |
| O: repairs an old defect not traced to recent work | 33 | 549 | 18.2% (13.6%–23.6%) |
| N: not a repair (feature, refactor, performance, docs, ruling) | 52 | 865 | 28.7% (23.3%–34.0%) |
| R + V + C: earlier landed work re-handled | 15 | 250 | 8.3% (5.1%–12.8%) |
| Not a keyword candidate | — | 1,352 | 44.8% (computed from all rows) |

### By lane and by period (the same sample, split; small cells)

The lane and period cells contain only 9 to 75 sampled commits each, so their ranges are wide. Read them as direction, not as rates.

| Group | Commits | Candidates | Sampled | R | R+V+C | Est. R share (95% range) | Est. R+V+C share (95% range) |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| psi | 1,326 | 824 | 49 | 3 | 7 | 3.8% (1.3%–10.3%) | 8.9% (4.4%–16.6%) |
| compiler | 266 | 156 | 9 | 3 | 4 | 19.5% (7.1%–37.9%) | 26.1% (11.1%–43.0%) |
| docs | 241 | 124 | 10 | 0 | 1 | 0% (0%–14.3%) | 5.1% (0.9%–20.8%) |
| all other lanes | 1,183 | 560 | 32 | 2 | 3 | 3.0% (0.8%–9.5%) | 4.4% (1.5%–11.5%) |
| Nov 10–16 | 1,616 | 1,132 | 75 | 6 | 12 | 5.6% (2.6%–11.5%) | 11.2% (6.6%–18.2%) |
| Nov 17–23 | 1,400 | 532 | 25 | 2 | 3 | 3.0% (0.8%–9.5%) | 4.6% (1.6%–11.4%) |

Of the 8 sampled regression repairs, 3 are in `compiler`, 3 in `psi` and 2 in `build`. The `compiler` cases are test files whose code stopped compiling after an API in another crate changed. The keyword filter also matches a much smaller share of commits late in the window: 85% of commits on Nov 11 but 20% on Nov 19. The later bodies probably use different words, so the drop from the first week to the second may come from the filter rather than from less rework.

The raw per-lane and per-day counts are printed by `python3 rework.py`.

### How long regressions lived on main, and what caught them (sample R cases)

The time on main runs from the landing of the cause PR to the landing of the repair. Both dates come from commits.tsv, and the body names the cause PR.

| Repair | Cause named in body | Time on main | What caught it (as the body says) |
| --- | --- | ---: | --- |
| `91d86f1ab00e` (#2964) | #2923 | 1.0 h | post-merge CI job: the test target failed to compile (`E0063 missing field supplies`) |
| `4809e8389dce` (#3349) | #3309 (my inference from "could no longer") | 2.0 h | a later agent's program was refused |
| `1a6b9b5f1460` (#5221) | #5150 | 5.4 h | a later agent working on Build files |
| `730180f801c0` (#3453) | #3350 | 5.8 h | a later agent: 8 tests could not start because of retired imports |
| `ade0aabb9799` (#2442) | #2370 (for one of three faults) | 8.1 h | corpus ledger run: 41 rows recorded as checking failed on main |
| `dfe21346cb04` (#5111) | #4837 | 43.1 h | test suite: 26 of 42 tests failed, and a bisect of first-parent main named #4837 |
| `44c61768aaa8` (#3206) | not named ("after provider plans stopped using separate declaration wrappers") | — | the test target no longer compiled |
| `6fe546efc941` (#2535) | #1818, which landed before the window | — | corpus fixtures were rejected |

Across the 6 cases with both dates in the window, the time on main ranges from 1.0 h to 43.1 h, with a median of 5.6 h. Outside the sample, three more cause-and-repair pairs give 1.2 h (#3000 → #3019), 1.4 h (#4861 → #4872), and 0.2 h for a follow-up (#3917 → #3918). The owner caught none of the 8 sampled regressions: 5 were caught by tests or CI, and 3 by a later agent who hit the failure.

### Ten clear examples

| Hash | Class | What the body says |
| --- | --- | --- |
| `91d86f1ab00e` | R | #2923 added a test initializer without the `supplies` field that #2915 had made required. The test target did not compile on main, and a post-merge CI job caught it about 1 h later. |
| `1a6b9b5f1460` | R | "This restores the whole-file check that #5150 removed." Build files with unknown types ran their Build before being checked. |
| `dfe21346cb04` | R | A Build with `suspend Build::prepare_dependency` was refused. A bisect between #4817 and #4998 named #4837 as the first bad commit, 43 h earlier. |
| `730180f801c0` | R | Eight `access_view_calls` tests could not start because their fixture imported names that #3350 retired. |
| `44c61768aaa8` | R | The compiler's package-review test target stopped compiling after the provider-plan migration. |
| `a799ad13633a` | R | "Every test in `compiler --test domain_qualified_types` has failed on main since #3000", fixed 1.2 h later. Found through the phrase filter, not the sample. |
| `bb3d4d0b02fb` | R | A test still read the `Refusal.machine` field that #4861 removed, so the whole stage-05 unit target failed to compile. Fixed 1.4 h later. Found through the phrase filter. |
| `ade0aabb9799` | R | Two borrow-checking faults "made 41 corpus programs that the ledger records as checking fail on main". A third fault dated from #2370. |
| `c4da5c01c8c1` | C | A follow-up to #3917 that routes eight more error conversions through the cause-preserving constructor #3917 introduced. It includes a control that "fails on the merged #3917 implementation". |
| `cc1c3bd01d72` | C | "#3524 meant to carry this edit, but the narrowed entry was left out of its squashed commit." |

### Secondary check: a phrase filter over all 3,016 bodies

A narrower phrase filter (`STRONG` in the script: "first bad commit", "bisect", "fails/failed/broken/red on main", "#N broke/introduced/removed/regressed", and similar) matches 109 of 3,016 bodies (3.6%). I read a random 25 of the 109 (seed 7). 10 of the 25 describe repairing a regression that an earlier landed change caused. Most of the other 15 mention, in passing, that some test is "already red on main" without repairing it. This check is a keyword match plus a 25-commit sample, not an estimate of its own. It is consistent with a regression-repair rate of a few percent.

## How this was computed

- **Data.** I used `commits.tsv` (3,016 first-parent commits on `origin/main`, 2026-11-10 to 2026-11-23). I read the commit bodies with a single `git -C /home/user/Omega log --first-parent origin/main --since=2026-11-09 --format='%x01%H%x02%B'`, which reads commit objects and fetches no file contents. I joined the bodies to the rows on the first 12 hex digits of the hash. I did not use `files.tsv`.
- **Candidate filter** (`is_cand` in `rework.py`). A commit is a candidate when its title or its body (without the `Agent:`, `Gate:`, `Spec:`, `Proof:` and attribution lines) matches any of these:
  - `\b(revert\w*|restor\w*|repair\w*|regress\w*|unbreak\w*|follow[- ]?up|again|re-?land\w*|undo\w*|undoes|broke|broken|breaks)\b` (case-insensitive);
  - a title of the form `lane: fix|repair|restore|revert …`;
  - a `#NNN`–`#NNNNN` reference other than the commit's own PR number.

  1,664 of 3,016 commits match.
- **Sample.** `random.seed(20261123); random.sample(cands, 100)` in Python 3. I classified each commit after reading its body: the opening paragraph, then each sentence that contains a keyword or a PR reference, and the full text for unclear cases. The classes are R, V, C, O and N as defined in the first table. The class list is hard-coded as `CLASS` in `rework.py`, in sample order, and the cause and how it was caught are in `CAUSE`. Some rules I applied:
  - A capability the compiler had refused as "not supported yet" counts as N, not as a repair.
  - An old defect that a recent change exposed rather than introduced counts as O (for example `a1308fdb7098`, `f9b49891e6ad`).
  - A migration that an earlier change began and this one finishes counts as C.
  - Commits that repair a regression in passing while doing something else (for example `bee91f6a3899`, which restores Windows Clippy while it removes dead xtask commands) count by their main purpose.
- **Scaling.** The rate for the window is the sample share × 1,664 ÷ 3,016. The 95% interval is the Wilson score interval (z = 1.96) on the sample share, with the same scaling. The lane and period estimates scale each group's sample share by that group's own candidate count and commit count.
- **Time on main.** This is the landing time of the repair minus the landing time of the cause PR named in the body. Both times come from the `date` column, and I matched the cause on the `pr` column.
- **Rerun.** Run `python3 rework.py` (counts, groups, time on main) and `python3 rework.py strong` (the 25-commit phrase-filter sample). Both read `commits.tsv` and `reports/bodies.raw`. `show.py` prints the sampled bodies the way I read them.

## What this does not show

- **One reader.** I classified every commit, once, with no second reader. The line between R, O and C is a judgment call. Several R cases name their cause only implicitly (`4809e8389dce`'s cause is my inference), and one cause (#1818) predates the window.
- **Missed repairs.** The 1,352 commits that match no keyword are counted as non-repairs. I did not sample them, so the rates are lower bounds for repairs worded without these words. The drop in candidate share late in the window (85% on Nov 11, 20% on Nov 19) suggests that the wording changed, and the filter may miss more repairs in the second week.
- **Small cells.** Each lane and day cell has 9 to 75 sampled commits, and most single days have 2 to 16. The lane and period rows cannot rank lanes or days with confidence. The higher `compiler` share rests on 4 of 9 sampled commits.
- **Regressions nobody repaired.** The data counts repairs that landed. It does not count regressions that are still on main, or regressions that were fixed inside a larger change whose body does not mention them.
- **Time on main.** Only 6 sampled cases, plus 3 outside the sample, can be dated. A cause that landed before the window cannot be dated from this table.
- **Size of the rework.** The data does not say how much work a repair took. I did not read diffs.
- **"Already red on main".** Many bodies say that some test was already failing on main. I did not measure how long main stayed red or how many tests were red.
