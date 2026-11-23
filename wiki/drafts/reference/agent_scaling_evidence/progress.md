# Is the compiler measurably closer to done? (origin/main, 2026-11-10 to 2026-11-23)

## Answer

No recorded number in these 14 days shows the compiler getting steadily closer to finished: none of the six release gates was ever reported closed, and nothing that measures the whole compiler was recorded more than once in a form that can be compared. The board went the other way. TASKS.md grew from 232 to 355 lines and from 55 to 77 top-level items: 34 item identifiers were added and 12 removed. The nearest thing to a progress measure was the "red rows" count in Gate lines. Commits reported a net 286 fewer failing corpus rows on 2026-11-10 to 2026-11-12, but every nonzero value falls in those three days, the corpus it counted was deleted on 2026-11-13, and the field last appears on 2026-11-16. The two whole-program sweeps with category totals are one Windows sweep and one partial Linux table, both from 2026-11-14 and never repeated. Every pass count in Proof lines counts the tests of a crate or suite, not the compiler as a whole.

## Measurements

### 1. Remaining-work files at the last commit of each day

"Items" counts lines that start with `- **ID.**`, where ID is bold capitals with at least one hyphen. "Distinct IDs" counts every such bold identifier anywhere in the file. Open questions are `###` headings under `## Open questions`. Concerns are `###` headings under `## Entries`.

| Day | Last commit | TASKS lines | TASKS items | Distinct IDs | Bullets | IDs added / removed | OWNER_QUESTIONS lines | Open questions | OWNER_CODE_CONCERNS lines | Concern entries |
|---|---|---|---|---|---|---|---|---|---|---|
| 11-10 | de317c676878 | 232 | 55 | 112 | 85 | – | 205 | 2 | (absent) | – |
| 11-11 | 18b5c9f40fb2 | 234 | 57 | 114 | 89 | +5 / −3 | 970 | 18 | (absent) | – |
| 11-12 | c6e3a086198a | 252 | 61 | 117 | 102 | +5 / −1 | 148 | 2 | 451 | 55 |
| 11-13 | 4e6a78253e42 | 278 | 67 | 123 | 114 | +6 / 0 | 163 | 3 | 298 | 32 |
| 11-14 | 6f775ca83a86 | 288 | 74 | 130 | 162 | +9 / −2 | 67 | 0 | 179 | 15 |
| 11-15 | a73cfac4de99 | 336 | 76 | 132 | 207 | +2 / 0 | 67 | 0 | 111 | 5 |
| 11-16 | 6169f5568dd8 | 346 | 77 | 133 | 214 | +1 / 0 | 316 | 6 | 219 | 20 |
| 11-17 | 14595a0db336 | 361 | 78 | 134 | 227 | +1 / 0 | 67 | 0 | 136 | 8 |
| 11-18 | 9eb9772d796f | 353 | 78 | 134 | 219 | 0 / 0 | 71 | 1 | 198 | 16 |
| 11-19 | 8edb668d35ca | 359 | 78 | 134 | 224 | 0 / 0 | 71 | 1 | 212 | 18 |
| 11-20 | acf8fc4f8bee | 330 | 77 | 136 | 194 | +2 / −3 | 67 | 0 | 130 | 6 |
| 11-21 | 02a8afccc3eb | 319 | 75 | 134 | 183 | 0 / −2 | 112 | 3 | 90 | 1 |
| 11-22 | 0cfaba88e56a | 339 | 75 | 134 | 202 | +1 / −1 | 86 | 3 | 86 | 1 |
| 11-23 | f0d028a4a512 | 355 | 77 | 136 | 217 | +2 / 0 | 72 | 1 | 94 | 2 |

The 12 item identifiers that left TASKS.md were:
- 11-11: SUBSTITUTABLE-SERVICES, W100-CONNECTED, WIRE-CODEC-LIBRARY
- 11-12: BUILD-TEST-GROUPS
- 11-14: CALLER-CHOSEN-RESULT-LIFETIMES, MUTABLE-PARAMETER-CONTRACT-NAME
- 11-20: FACT-SAMPLE-DEPENDENCIES, LOC-VALUE-CUSTODY, REQUIRES-UNIFICATION
- 11-21: ITERATION-PERFORMANCE, VALIDATION-ENVIRONMENT
- 11-22: NO-IMPLICIT-COMPILER-STORAGE

The 11-23 file still holds LOWERED-FORM-CHECKER, the item the owner marked highest priority, and WINDOWS-ARM64-TARGET, which added a fifth hosted target on 11-23.

### 2. Release gate named in each commit's Gate line

The gate is the first `|`-separated field of the Gate line, matched to the six gate names. Plain spellings such as "source semantics" count too. "Compiler lanes" are psi, omega, backend, compiler, build, packages and library: 2,024 of 3,016 commits.

| Gate | All commits (3,016) | Compiler-lane commits (2,024) |
|---|---|---|
| RC-SOURCE-SEMANTICS | 1,218 | 1,159 (57%) |
| RC-BUILD-AND-PACKAGES | 296 | 273 (13%) |
| RC-NATIVE-MATRIX | 230 | 203 (10%) |
| RC-REPOSITORY | 172 | 140 (7%) |
| RC-DIAGNOSTICS | 125 | 115 (6%) |
| RC-REPRESENTATIVE-PROGRAMS | 92 | 71 (4%) |
| text naming no gate | 66 | 33 |
| no Gate line | 817 | 30 |

Compiler-lane commits per day, by gate:

| Day | Total | SOURCE-SEM | BUILD-PKG | NATIVE | REPO | DIAG | REPR-PROG | none or other |
|---|---|---|---|---|---|---|---|---|
| 11-10 | 24 | 20 | 0 | 2 | 0 | 1 | 1 | 0 |
| 11-11 | 148 | 113 | 9 | 4 | 9 | 8 | 5 | 0 |
| 11-12 | 110 | 80 | 11 | 4 | 11 | 3 | 1 | 0 |
| 11-13 | 206 | 146 | 23 | 22 | 11 | 4 | 0 | 0 |
| 11-14 | 285 | 174 | 41 | 20 | 22 | 27 | 1 | 0 |
| 11-15 | 252 | 139 | 38 | 33 | 10 | 13 | 19 | 0 |
| 11-16 | 135 | 64 | 12 | 22 | 11 | 4 | 22 | 0 |
| 11-17 | 100 | 66 | 3 | 8 | 8 | 3 | 12 | 0 |
| 11-18 | 156 | 68 | 28 | 22 | 26 | 3 | 4 | 5 |
| 11-19 | 150 | 56 | 13 | 23 | 17 | 0 | 0 | 41 |
| 11-20 | 123 | 34 | 29 | 8 | 6 | 28 | 3 | 15 |
| 11-21 | 122 | 75 | 20 | 16 | 5 | 4 | 0 | 2 |
| 11-22 | 105 | 64 | 23 | 2 | 4 | 11 | 1 | 0 |
| 11-23 | 108 | 60 | 23 | 17 | 0 | 6 | 2 | 0 |

The mix has no trend. RC-SOURCE-SEMANTICS leads on every day, between 28% (11-20) and 83% (11-10). RC-REPRESENTATIVE-PROGRAMS, the gate that the unfiltered sample gate decides, appears in 0 to 22 compiler-lane commits a day and in 71 in total. A Gate line names the gate a commit says it serves. It does not say whether that gate is any closer to closing.

### 3. "red rows" in Gate lines: the one recorded progress measure

The AGENTS.md of 11-12 (snapshot c6e3a086198a) required compiler PRs to write `Gate: <gate> | red rows: <change>`: the net change in failing rows of the tests/omega corpus ledger, negative when rows turned green. By 11-13 (4e6a78253e42) AGENTS.md no longer mentions red rows.

| Day | Gate lines with red rows | Nonzero | Sum |
|---|---|---|---|
| 11-10 | 27 | 10 | −102 |
| 11-11 | 161 | 36 | −171 |
| 11-12 | 121 | 33 | −13 |
| 11-13 | 7 | 0 | 0 |
| 11-14 | 69 | 0 | 0 |
| 11-15 | 87 | 0 | 0 |
| 11-16 | 5 | 0 | 0 |
| 11-17 to 11-23 | 0 | – | – |

Over all 477 values, the negative ones add up to −348 and the seven positive ones (+41, +7, +5, +3, +3, +2, +1) add up to +62, for a net of −286. The commits report these numbers themselves. This analysis did not check them against a ledger.

### 4. Whole-program sweeps and sample-gate totals in commit bodies

These were found by searching all 3,016 bodies for lines that mention "sweep" or "sample-gate" and also contain a number, then reading the matches. wiki/development/testing.md records how to run a sweep ("about 9,000 programs and about an hour on one machine") but no results.

| Date | Commit | What it records | Whole-compiler? |
|---|---|---|---|
| 11-10 | d30fccc7aae6 | A sweep of 1,314 corpus rows; 154 `pass/` rows flagged red in shards 41–50 | Part of the corpus |
| 11-11 | 574477f5b606, 4de61edc4fbf | A full corpus sweep at 759c80404428 found 1,296 rows worse than the ledger recorded | Whole corpus, but counts regressions against the ledger, not a total |
| 11-11 | b4080e938ae1 | All 8,325 `fail/` fixtures | Only the negative half |
| 11-13 | e90504ab19fa | At 386e5df75d, 1,906 programs refused with "root binding main is ambiguous", 654 with "duplicate const" and 93 with "ambiguous membership" | Counts of causes, not a total |
| 11-13 | 4e5a0ecc674f | Adds `cargo xtask sweep`, covering about 9,000 programs | No results |
| 11-14 | 87d447aa4a61 | Linux sweep at c195e1ef3c. 881 programs the old sweep called "Rejected as expected" split into 601 pinned error matched, 250 different error, 30 nothing pinned. Two machines each checked all 9,097 programs | Partial table only |
| **11-14** | **97df27b7f63a** | **Full windows_x86_64 sweep at b3b8010cec: 660 Checks, 884 Rejected as expected, 254 Rejected for another reason, 7,285 Blocked, 18 Accepted a wrong program (9,101 in all)** | **Yes, the only complete one, recorded once** |
| 11-14 | dde4eeba67e9 | 176 programs re-checked from "the 14 November sweep" | A follow-up subset |
| 11-23 | e95a95a1e7f0 | A sweep with `--changed` that selected 0 rows and then 1 | A selection |

The 24 sample-gate lines without `--filter` all describe filtered or named subsets: 18 of 18 programs, 28 projects, 68 directories, 160 showcase samples (d7ac56cff569, 11-15), and 13 probes. None reports an unfiltered sample-gate total, which is the evidence RC-REPRESENTATIVE-PROGRAMS calls for.

A quick sweep is running on this machine now (`sweep_quick.tsv.status`: 2,336 programs at 1987cfa875 for linux_x86_64, 0 done when read). It is not part of the record and was not used.

### 5. Pass counts in Proof lines

2,039 of 3,016 commits have a Proof line. The pass-count pattern matched 183 times in 166 of those. By the noun counted: tests 168, cases 8, samples 4, programs 3.

| Kind of count | Example (day, commit) | Repeated across days? |
|---|---|---|
| One crate or suite's tests | "292 terminal-semantics library tests" (11-23, caa1c50a1bec), "126 resolver tests" (11-20, 985bf70ff52e) | Each suite has its own number; they cannot be summed or compared |
| Lowered-form checker suite | "1,227 cases: 1,186 passed, 39 failed" (11-20, 84002df5e710); "1,228 … 1,198 passed, 28 failed" (11-20, b8fa1b4f53f2) | Twice, on one day. It counts the checker's tests, not programs. |
| V2 workspace "L0–L5 readiness" | 1,892 / 1,881 / 1,842 tests and "37 → 34 blockers" (11-21, 049ff1c2a917); 2,128 (11-21), 2,139 (11-21), 2,044 → 2,047 → 2,088 (11-22), "four total blockers" (11-22, eab3284034b1), 2,201 and 2,276 (11-22) | Yes, for 11-21 and 11-22 only. It measures a separate omega-rust-v2 workspace, added on 11-18 (#4218). #5070 on 11-22 says V2 takes no new work, and V2 is gone from the tree at the 11-22 and 11-23 snapshots. |
| Bigger local totals | 2,752 tests on Windows x86-64 (11-23, 4ea9fb880e23); 1,542 unit tests (11-23, 870d1b40e71a) | Once each, with different scopes |

A random sample of 30 of the 183 matches (Python `random.seed(20261123)`, `random.sample`) was read by hand. All 30 count tests of one crate, stage or suite: for example "64 sample gates agree" over the samples a change touched, "5 corpus tests" and "2,128 Rust tests" in the V2 workspace. None counts every program or every test in the compiler.

## How this was computed

- Input: commits.tsv (3,016 first-parent commits on origin/main, 2026-11-10 to 2026-11-23). Days come from its `day` column. The last commit of a day is the one with the latest `date`, compared as an absolute timestamp.
- Board files: `git show <hash>:TASKS.md`, `:OWNER_QUESTIONS.md` and `:OWNER_CODE_CONCERNS.md`, read once each per day.
  - Item regex: `^- \*\*([A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+)\.?\*\*` (multiline). Distinct-ID regex: the same pattern without the line anchor.
  - "Added / removed" compares the item-ID sets of consecutive days.
- Gate: the first `|` field of the `gate` column (300 characters), upper-cased and matched by prefix to the six `RC-` names or to their lower-case plain spellings. Red rows: `red rows:\s*([+-]?\d+)`.
- Proof counts: regex `(?:all\s+)?(\d[\d,]*)\s+(?:[\w-]+\s+){0,3}?(tests?|samples?|programs?|fixtures?|cases?)[^.;|]{0,40}?\b(pass(?:ed)?|ok|succeeded|exited 0|agree[ds]?)`, case-insensitive, over the `proof` column (first 600 characters). An "N of M" pattern was also tried and matched nothing.
- Full bodies came from one `git log --first-parent --format=%h%x00%B%x1e -n 3400 origin/main`, and all 3,016 hashes matched. Sweep and sample-gate lines are body lines matching `sample[- ]gate|xtask sweep|\bsweep\b` that also contain a digit. Each was then read. Readiness and checker totals came from the further regexes in the script.
- Script: `progress.py` in this directory. `python3 progress.py > progress.out 2> progress.err` reproduces every number above. progress.err holds the gate, red-rows and readiness tables.

## What this does not show

- **Whether removed TASKS.md items were finished.** The 12 identifiers that left the board might have been renamed, merged or dropped. The diffs were not read. The growth to 77 items may also reflect newly found work rather than lost ground.
- **OWNER_QUESTIONS.md and OWNER_CODE_CONCERNS.md move both ways.** Questions rise to 18 on 11-11 and fall to 0 after rulings; concern entries fall from 55 to 1–2. Their size tracks how fast the owner rules and how agents file and clear entries, not how much compiler is left. OWNER_CODE_CONCERNS.md is absent at the 11-10 and 11-11 snapshots.
- **Gate counts show what agents said they worked on, not results.** No commit claims a gate closed, and the closure rule needs all six on one revision on five hosts.
- **The red-rows total is self-reported and covers three days.** Its corpus no longer exists, so the −286 cannot be continued or checked here.
- **One complete sweep is a single point, not a trend.** The 11-14 Windows sweep left 7,285 of 9,101 programs Blocked, but no later full sweep was recorded to compare with. Sweeps may have run with results kept only on the machines (`build/sample-gate-runs/`, `--out` files), which this record cannot see.
- **The keyword searches may miss totals worded differently.** For example, a result with no "sweep" or "sample-gate" in its line would not be found.
- No build, test or sweep was run for this report.
