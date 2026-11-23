# Cost, speed and human steering per landed change

## Answer

1. About 215 commits landed on `origin/main` per day: 3,016 over 14 days, ranging from 135 to 351 on the 13 full days. Codex wrote 1,945 of them, Claude Code 951, other agents 47, and 73 carry no provenance line.
2. After correcting for audit lines that report a running session total (explained below), the median change used 11.6M input tokens, 98.8% of them served from cache (about 100k uncached), plus 29k output tokens, 59 steps and 18 minutes of agent time. The 90th percentile is 44.9M input tokens and 90 minutes, and the costliest 10% of commits hold 54% of all input tokens (n = 1,910 commits with usable numbers).
3. Since the audit tool was fixed on 2026-11-15 to count only messages a person typed, 76% of measurable commits record zero steers. The mean is 3.5 steers per commit, and 33 of 717 commits record more than 20. Before the fix, Claude Code lines also counted tool results and notifications as steers, so steers before 2026-11-15 19:27 cannot be compared with steers after it.
4. Per-commit cost shows no clear trend across the 14 days: the median corrected input is 10.9M in the first week and 13.1M in the second, and the rank correlation with time is +0.06. The large fall in steers comes from the counting fix, not from any change in human behaviour.
5. Only 1,910 of the 3,016 commits (63%) have usable numbers. From 2026-11-17 to 2026-11-21, most Codex lines read "unavailable", so the second week rests on 536 of its 1,400 commits.

## Measurements

The tables call **set A** the 1,910 commits whose provenance line has all six numbers, after removing 136 placeholder lines (0 tokens, 1 step, 1 s) and 8 unsplittable running totals. **Corrected** values subtract the previous running total from the same session (see "How this was computed"). **As reported** values are the raw numbers on the line.

### Coverage of the provenance line

| Condition | Commits | Share of 3,016 |
|---|---|---|
| No provenance line at all | 73 | 2.4% |
| Line present, numbers "unavailable" or unparsed (870 Codex, 19 others) | 889 | 29.5% |
| All six numbers present | 2,054 | 68.1% |
| of which placeholder (0 tokens, 1 step, 1 s) | 136 | |
| of which running total with no earlier row to subtract, over 24 h | 8 | |
| **Set A** | **1,910** | **63.3%** |
| Rows in the 2,054 that continue an earlier row's running total | 514 | |

### Per commit (set A, corrected) compared with as reported (2,054 rows)

| Measure | Median (as reported) | Mean (as reported) | p90 (as reported) | Median (corrected) | Mean (corrected) | p90 (corrected) | Total (corrected, 14 days) |
|---|---|---|---|---|---|---|---|
| Tokens in | 18.4M | 550M | 799M | 11.6M | 25.7M | 44.9M | 49.0B |
| Tokens cached (part of tokens in) | 18.3M | 543M | 785M | 11.5M | 25.4M | 44.5M | 48.5B |
| Tokens in minus cached | 150k | 6.8M | 7.5M | 100k | 266k | 471k | 0.51B |
| Tokens out | 43k | 1.3M | 2.0M | 29k | 67k | 132k | 129M |
| Steps | 83 | 2,866 | 2,056 | 59 | 114 | 215 | 218k |
| Minutes | 24.4 | 1,055 | 781 | 17.8 | 39.8 | 90 | 76k (1,267 h) |
| Steers | 0 | 164 | 312 | 0 | 21.3 | 64 | 41k |

The raw mean is about 20 times the corrected mean because some lines carry cumulative counters. One Codex session reported 721 hours, 19.7B tokens and 4,572 steers on a single commit, which is more than the 336 hours in the window.

### Per day

"Agent-hours" is the sum of minutes on that day's commits divided by 60. Dividing corrected agent-hours by 24 gives the average number of agents working at once, but only for commits that carry numbers.

| Day | Commits | In set A | Median tokens in (corr.) | Tokens in, as reported | Tokens in, corrected | Tokens out, corr. | Agent-hours as reported | Agent-hours corrected | Agents at once (corr. h / 24) | Median minutes (corr.) | Zero-steer commits (A) | Steers as reported | Steers corrected |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 11-10 (partial) | 58 | 47 | 26.9M | 9.9B | 3.6B | 13.0M | 146 | 63 | 2.6 | 41 | 4/47 | 16k | 6,316 |
| 11-11 | 259 | 240 | 13.6M | 12.0B | 6.7B | 16.0M | 262 | 170 | 7.1 | 23 | 10/240 | 22k | 13k |
| 11-12 | 211 | 164 | 10.5M | 15.5B | 5.6B | 18.1M | 239 | 102 | 4.2 | 15 | 30/164 | 15k | 6,497 |
| 11-13 | 249 | 186 | 5.7M | 378.8B | 2.0B | 4.0M | 14,235 | 65 | 2.7 | 8.5 | 71/186 | 139k | 1,309 |
| 11-14 | 351 | 307 | 6.3M | 99.7B | 5.3B | 12.1M | 3,263 | 112 | 4.7 | 6.4 | 172/307 | 44k | 5,211 |
| 11-15 | 319 | 297 | 16.4M | 49.5B | 7.7B | 21.6M | 1,211 | 217 | 9.0 | 25 | 172/297 | 18k | 6,102 |
| 11-16 | 169 | 133 | 18.0M | 146.1B | 4.3B | 11.7M | 5,004 | 156 | 6.5 | 40 | 98/133 | 19k | 653 |
| 11-17 | 135 | 63 | 12.1M | 73.3B | 1.5B | 5.0M | 2,555 | 54 | 2.2 | 33 | 45/63 | 12k | 328 |
| 11-18 | 225 | 77 | 21.7M | 132.2B | 2.5B | 5.4M | 4,589 | 57 | 2.4 | 21 | 58/77 | 23k | 305 |
| 11-19 | 257 | 27 | 4.5M | 7.4B | 0.3B | 0.7M | 192 | 7 | 0.3 | 11 | 15/27 | 1,718 | 151 |
| 11-20 | 309 | 78 | 11.4M | 53.5B | 2.5B | 4.8M | 1,094 | 58 | 2.4 | 18 | 47/78 | 8,048 | 348 |
| 11-21 | 180 | 45 | 15.9M | 42.9B | 1.1B | 4.1M | 943 | 50 | 2.1 | 44 | 29/45 | 5,316 | 68 |
| 11-22 | 152 | 134 | 14.1M | 102.4B | 2.8B | 5.6M | 2,218 | 63 | 2.6 | 18 | 110/134 | 13k | 80 |
| 11-23 (to 15:10 -08:00) | 142 | 112 | 11.9M | 7.1B | 3.1B | 6.6M | 171 | 95 | 3.9 | 24 | 108/112 | 17 | 17 |

As reported, agent-hours reach 14,235 h on one day, which no group of sessions can log in 24 hours. These values are cumulative session totals, not the time spent on one change. After correction, 50 to 217 agent-hours fall on each full day, which means 2 to 9 agents were working at once on average. That is a lower bound, because 37% of commits contribute no time.

### Steers per commit

| Steers | As reported (n = 2,054) | Set A, corrected (n = 1,910) | After the counting fix, set A (n = 717) |
|---|---|---|---|
| 0 | 1,051 (51%) | 969 (51%) | 542 (76%) |
| 1 | 69 (3%) | 122 (6%) | 56 (8%) |
| 2–5 | 100 (5%) | 145 (8%) | 52 (7%) |
| 6–20 | 190 (9%) | 256 (13%) | 34 (5%) |
| more than 20 | 644 (31%) | 418 (22%) | 33 (5%) |

In set A, the 10% of commits with the most steers hold 70% of all corrected steers. After the fix (2026-11-16 to 2026-11-23), set A records 1,950 corrected steers over 669 commits, or about 244 a day for the commits that have numbers.

### Before and after the provenance-counting fix (PR #3707, merged 2026-11-15 19:26:49 -08:00)

PR #3707 (`tools: make provenance audit a Claude subagent's own work and count only typed steers`) states that, before it landed, Claude Code lines counted tool results, notifications and peer messages as steers. They also counted tokens two or three times and could report the coordinator's numbers instead of the subagent's.

| Agent | Period | Commits | In set A | Median tokens in (corr.) | Mean tokens in (corr.) | p90 tokens in | Median minutes | Median steps | Zero-steer share | Mean steers | p90 steers |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Codex | before | 599 | 463 | 9.8M | 25.5M | 41.5M | 15 | 48 | 82% | 1.1 | 2 |
| Codex | after | 1,346 | 507 | 13.2M | 28.6M | 49.4M | 23 | 86 | 69% | 3.8 | 8 |
| Claude Code | before | 728 | 721 | 10.6M | 24.5M | 42.9M | 14 | 40 | 6% | 52.0 | 116 |
| Claude Code | after | 223 | 206 | 14.6M | 22.8M | 47.7M | 26 | 73 | 93% | 2.6 | 0 |
| All agents | before | 1,395 | 1,193 | 10.2M | 24.9M | 42.4M | 15 | 43 | 36% | 32.1 | 95 |
| All agents | after | 1,621 | 717 | 14.0M | 27.0M | 49.3M | 24 | 80 | 76% | 3.5 | 4 |

### By agent (set A, corrected)

| Agent | Commits | In set A | Median tokens in | Median tokens out | Median steps | Median minutes | Zero-steer share | Mean steers | Share of corrected tokens in |
|---|---|---|---|---|---|---|---|---|---|
| Codex | 1,945 | 970 | 11.9M | 27k | 65 | 18 | 75% | 2.5 | 54% |
| Claude Code | 951 | 927 | 11.4M | 30k | 48 | 17 | 25% | 41.0 | 46% |
| Others (Cursor 23, Antigravity 14, Human with GitHub Copilot 8, Devin 1, ChatGPT 1) | 47 | 13 | 6.0M | 46k | 32 | 17 | 62% | 18.7 | 1% |
| No provenance line | 73 | 0 | – | – | – | – | – | – | – |

Claude Code's steer figures here mix the two counting rules: 721 of its 927 set-A commits come from before the fix.

### By model (set A, corrected; models with 20 or more commits)

| Agent / model | Commits | In set A | Median tokens in (as reported) | Median tokens in (corr.) | Median tokens out | Median minutes | Zero-steer share |
|---|---|---|---|---|---|---|---|
| Codex / "unavailable" | 870 | 0 | – | – | – | – | – |
| Codex / gpt-6.1-sol | 576 | 554 | 18.8M | 13.4M | 32k | 22 | 78% |
| Claude Code / claude-opus-5-5 | 546 | 534 | 24.5M | 14.7M | 44k | 23 | 25% |
| Codex / gpt-6.2-astra | 318 | 317 | 46.1M | 10.3M | 20k | 15 | 73% |
| Claude Code / claude-fable-5-2 | 276 | 276 | 13.4M | 8.1M | 13k | 10 | 12% |
| Claude Code / "unavailable" | 66 | 60 | 6.4M | 6.4M | 19k | 16 | 95% |
| Codex / gpt-6-astra | 53 | 51 | 7.3M | 7.3M | 15k | 16 | 57% |
| Codex / gpt-6 | 50 | 1 | 0 (placeholders) | – | – | – | – |
| Codex / gpt-6.2-sol | 33 | 33 | 206M | 28.2M | 48k | 23 | 64% |
| Claude Code / "withheld" | 30 | 30 | 161M | 4.2M | 33k | 15 | 3% |
| Codex / "unknown" | 24 | 0 | 0 (placeholders) | – | – | – | – |

### By lane (set A, corrected; lanes with 40 or more commits)

| Lane | Commits | In set A | Median tokens in | Median tokens out | Median steps | Median minutes | Zero-steer share | Mean steers | Share of corrected tokens in |
|---|---|---|---|---|---|---|---|---|---|
| psi | 1,326 | 1,027 | 12.8M | 31k | 66 | 21 | 53% | 21.2 | 58% |
| compiler | 266 | 153 | 13.9M | 42k | 80 | 24 | 50% | 18.1 | 8% |
| docs | 241 | 135 | 5.0M | 13k | 24 | 7.5 | 47% | 36.0 | 11% |
| omega | 123 | 92 | 19.9M | 46k | 86 | 30 | 59% | 15.3 | 5% |
| repo | 122 | 111 | 6.2M | 18k | 24 | 5.4 | 36% | 17.3 | 3% |
| build | 114 | 46 | 12.8M | 27k | 77 | 16 | 76% | 14.6 | 2% |
| tools | 99 | 75 | 9.2M | 26k | 41 | 11 | 52% | 31.5 | 3% |
| backend | 93 | 32 | 7.4M | 20k | 57 | 18 | 69% | 4.5 | 1% |
| tests | 83 | 80 | 13.0M | 35k | 64 | 22 | 21% | 35.6 | 3% |
| bootstrap-omega | 80 | 3 | – (3 rows) | | | | | | 0% |
| bootstrap | 66 | 1 | – (1 row) | | | | | | 0% |
| packages | 54 | 33 | 9.1M | 28k | 69 | 18 | 61% | 7.1 | 1% |
| samples | 49 | 38 | 10.1M | 23k | 58 | 14 | 26% | 30.7 | 1% |
| library | 48 | 31 | 10.0M | 20k | 43 | 16 | 68% | 14.1 | 1% |
| no lane prefix | 125 | 10 | 15.2M | 30k | 132 | 26 | 20% | 3.0 | 1% |
| other lanes (V2, epsilon, cli, native, source, ...) | 127 | 43 | 12.2M | 17k | 62 | 13 | 49% | 3.0 | 1% |

The bootstrap lanes have almost no usable numbers: 77 of their 80 and 65 of their 66 commits lack numbers or carry placeholders. Lane differences in steers mostly reflect how many of each lane's commits came from Claude Code before the counting fix.

### Trend over the 14 days (set A, corrected)

| Period | Commits | In set A | Median tokens in | Mean tokens in | Median minutes | Zero-steer share | Mean steers |
|---|---|---|---|---|---|---|---|
| 2026-11-10 to 11-16 | 1,616 | 1,374 | 10.9M | 25.6M | 16.6 | 41% | 28.7 |
| 2026-11-17 to 11-23 | 1,400 | 536 | 13.1M | 25.8M | 21.3 | 77% | 2.4 |

The table gives Spearman rank correlations with commit time over set A (n = 1,910):

| Measure | All agents | Codex (n = 970) | Claude Code (n = 927) |
|---|---|---|---|
| Corrected tokens in | +0.06 | +0.08 | −0.02 |
| Corrected minutes | +0.07 | | |
| Corrected steps | +0.18 | | |
| Corrected steers | −0.55 | +0.04 | −0.53 |

The fall in steers comes from Claude Code alone and lines up with the counting fix. Codex steers show no trend.

### Concentration of input tokens

| Basis | Top 1% of commits | Top 10% of commits | Bottom 50% of commits |
|---|---|---|---|
| As reported (n = 2,054) | 36.2% | 92.4% | 0.6% |
| Set A, corrected (n = 1,910) | 24.3% | 53.8% | 9.2% |

## How this was computed

The script `<tables>/reports/cost.py` reads `commits.tsv` (3,016 rows) and prints every table. It uses no sampling and no random seed, and needs only python3. The chain-ratio sensitivity runs are `python3 cost.py <commits.tsv> 0.5` and `... 0.9`.

- **Days** use the `day` column as extracted. It takes the date in the commit's own timezone offset: 2,759 commits are at -08:00 and 257 at +00:00. 2026-11-10 starts at 00:43 UTC, so it is a partial day, and 2026-11-23 ends at 15:10 -08:00.
- **Usable numbers** means all six numeric cells (`tokens_in`, `tokens_cached`, `tokens_out`, `steps`, `minutes`, `steers`) are present. I checked the extraction by hand against the raw lines from `git show -s --format=%B` for 11 commits: 6 random (seed 7) and the 5 largest. All 11 matched.
- **Placeholders** are 136 rows with 0 tokens, 1 step and a time of 1 s. They are excluded from set A.
- **Running totals (chain correction).** Many lines report counters that grow from commit to commit within one session. One Codex series climbs from 42,668 to 43,299 minutes across about 30 commits. The script links row B to an earlier row A when all of these hold:
  - A and B have the same agent, and A is at most 72 h earlier.
  - A reports at least 50M tokens in, and A's tokens in is at least 0.7 × B's.
  - A's tokens in, tokens out, steps and steers are each ≤ B's.
  - B's minutes exceed A's by no more than the wall-clock gap plus 30 minutes, and are not more than 30 minutes below A's.

  Among the candidates, A is the one with the largest tokens in. Each corrected value is B − A, floored at 0, and an unlinked row keeps its own values. This linked 514 of 2,054 rows. Rows with no link and more than 24 h of reported time (8 rows) are running totals with nothing to subtract, so they are excluded from set A. Of the 184 rows reporting more than 24 h, 176 were linked.
- **Sensitivity of the correction.** At a link ratio of 0.5, the script links 550 rows. The corrected median of tokens in is 11.6M, the total is 44.4B, and the top 10% hold 49%. At a ratio of 0.9, it links 391 rows: the median is 11.8M, the total 67.0B, and the top 10% hold 61%. Medians and zero-steer shares barely move. Means, totals and concentration move by about ±20–35%.
- **Agents at once** = corrected minutes per day / 60 / 24.
- **The counting-fix split** uses the merge time of commit `8b2f3c251c` (PR #3707), read with `git show -s`. I found it with `git log --first-parent origin/main --since=2026-11-09 --format=...`, filtered by the keyword "provenance|steer". Commit `e5371514c9` (PR #3167, 2026-11-14) changed Codex transcript selection to refuse a missing match.
- **Lane** is the extracted prefix of the title. Lanes with fewer than 40 commits are grouped as "other lanes", and an empty prefix is shown separately.
- **Percentiles** use linear interpolation between order statistics.

## What this does not show

- **The 37% of commits without numbers.** These are 73 with no line, 889 "unavailable" and 136 placeholders. Nearly all "unavailable" lines are Codex commits from 2026-11-14 to 2026-11-23, peaking at 226 of 309 commits on 11-20. If those commits differ in cost, the second-week figures are biased. A plausible cause is the Codex thread-binding change (#3167), which refuses to report when no transcript matches, but this is my inference and I did not verify it.
- **Exact per-commit cost.** The chain correction is a heuristic. Two independent large sessions by the same agent within 72 h can be wrongly linked, which understates cost. A running total whose earlier commits fall outside the rules is not linked, which overstates it. When one session lands several commits, the corrected values divide its cost among them by the order of their counters, not by the work each one took. 41 set-A rows have a corrected input of exactly 0 because two commits carry the same totals.
- **Steers as a measure of human intervention before 2026-11-15 19:27.** By PR #3707's own account, Claude Code steers before then counted non-human messages. After the fix, the line counts typed messages "after the first one", but whether every one of them was a redirection is not checked. Codex's counting rule was not examined.
- **Whether time is wall time of one session.** Corrected minutes behave like active time per change (median 18 minutes). Raw values above 24 h, and daily sums of up to 14,235 h, are cumulative session counters and cannot be wall time. Corrected agent-hours give a lower bound of 2–9 agents working at once per day. The data does not record how many sessions actually ran.
- **Money.** Tokens are not priced. The split into cached and uncached input is reported, but different models price these differently.
- **Work that never landed.** Abandoned or unmerged sessions, and sessions whose PRs merged after 2026-11-23 15:10, are not counted. Tokens spent by a coordinator that did not itself land a commit are also missing.
- **Accuracy of self-reported numbers.** The lines come from the agents' own `cargo xtask provenance` audit. PR #3707 says Claude Code tokens were counted two or three times before the fix, so Claude Code token figures before 2026-11-15 are likely inflated. The correction above does not remove that inflation.

## Check

A second analyst checked this report. The counts and the raw figures reproduce exactly. The corrected medians move by 10–15% when the running-total correction uses a different rule. Two statements do not hold as written. The range "50–217 agent-hours per full day" leaves out 11-19, which has 7 h. The statement "no clear trend" in per-commit cost hides a rise in steps and minutes after 2026-11-15 that both agents show. I used my own scripts (`chk2.py`, `chk3.py` in the scratchpad), python3 and `git show -s` only. I did not rerun `cost.py`.

### Recomputed and matched

- **Commits per day.** The day counts match: 3,016 rows, with 58 on 11-10 and 135–351 on the other days. 11-23 ends at 15:10, so it is also partial. That leaves 12 full days, not 13. The range does not change.
- **Commits by agent.** Codex 1,945, Claude Code 951, no line 73, Cursor 23, Antigravity 14, Human/Copilot 8, ChatGPT 1, Devin 1. All match.
- **Commits by model.** The report gives no model table. In the model column, 945 rows read "unavailable", 33 "unknown" and 30 "withheld". Of the 945 "unavailable" rows, 68 still carry token numbers. A count by model therefore covers only about two thirds of commits.
- **Coverage.** 962 rows have empty numbers: 73 with no line and 889 with a line. 136 rows have 0 tokens. 184 rows report more than 1,440 minutes, and the largest reports 43,299. The raw daily sum peaks at 14,235 h on 11-13. All match.
- **"Unavailable" lines.** I drew a sample of 30 of the 889 rows that have a line but no numbers (seed 4242) and read each commit body. All 30 are Codex lines that literally read "unavailable" (30/30). Codex rows with empty numbers number 870, with none before 11-14 and 226 of 303 on 11-20. Both match.
- **Raw per-commit figures (n = 2,054).** Median tokens in 18.39M, mean 550M, p90 799M. Out 42.7k. Steps 83. Minutes 24.35. The top 10% hold 92.4% of tokens in. All match.
- **Steers after PR #3707.** I confirmed the merge time, 2026-11-15T19:26:49-08:00, with `git show`. The PR body confirms that before the fix, steers counted tool results and notifications, tokens were counted two or three times, and subagents reported the coordinator's numbers. Over my corrected set:
  - 545 of 717 commits have zero steers (76%), against 542 in the report.
  - The mean is 3.30, against 3.45.
  - 29 commits record more than 20 steers, against 33.
  - Claude Code's zero-steer share is 6% before the fix (n = 721) and 93% after (n = 205).
  - Codex's zero-steer share is 82% before and 69% after.
- **Steer trend.** The Spearman correlation between commit time and steers is −0.55 overall, −0.51 for Claude Code and +0.03 for Codex. This matches. The fall in steers is a change in how the audit tool counts them.

### Sensitivity of the corrected figures

I replaced the running-total rule with my own:
- the earlier row must have the same agent **and the same model**;
- it must be at most 72 h earlier and report at least 50M tokens in;
- all five counters must be at most the later row's, including minutes;
- the later row's minutes may grow by no more than the wall gap plus 30 minutes;
- there is **no 0.7 ratio**, and the rule picks the **latest** qualifying earlier row.

This rule linked 538 rows and gives a set of 1,912 commits. The report's rule gives 1,910. Under my rule:

| Measure | Report | My rule |
|---|---|---|
| Median tokens in | 11.6M | 13.1M |
| p90 tokens in | 44.9M | 50.6M |
| Mean tokens in | 25.7M | 31.3M |
| Total tokens in | 49.0B | 59.8B |
| Median uncached | 100k | 100k |
| Median out | 29k | 30k |
| Median steps | 59 | 62 |
| Median minutes | 17.8 | 18 |
| Top 10% share of tokens in | 53.8% | 56.9% |
| Top 1% share of tokens in | 24.3% | 29.8% |

My total lies inside the report's 44.4–67.0B sensitivity range. On the 1,325 rows that neither link to an earlier row nor serve as one, the median is 10.8M tokens and 14.1 minutes. The medians in the headline should therefore be read as about ±15%, not to three significant figures.

The same holds for agents and lanes. Under my rule, the Codex median is 14.0M and the Claude Code median is 12.4M, against 11.9M and 11.4M in the report. The ordering of agents and lanes is the same: omega 22.9M, compiler 18.9M, psi 15.3M, repo 6.5M, docs 5.3M. psi holds 54% of corrected tokens in, against 58% in the report. The bootstrap and bootstrap-omega lanes have usable numbers on 4 of 146 commits, which matches. Another 41 commits fall in other bootstrap-family lanes (epsilon, gamma, delta, Alpha, bootstrap-tests).

### What differed

- **The range of agents at once is wrong.** The report says "50–217 agent-hours per full day, 2–9 agents at once". Its own per-day table gives 7 corrected hours on 11-19, which is 0.3 agents; my rule gives 15 h. The correct range of corrected hours on the full days is 7–217, which is 0.3–9 agents at once. 11-19 has usable numbers on only 27 of 257 commits. That supports the report's own caveat: this is a lower bound and says little about how many sessions actually ran.
- **The headline understates the trend.** It says "per-commit cost shows no clear trend" and quotes only tokens (Spearman +0.06; mine +0.09). Splitting set A at the PR #3707 merge gives a rise for both agents. For Codex, whose counting the fix did not change, I confirmed this only for tokens, steps and minutes:

  | Group | Before the fix | After the fix |
  |---|---|---|
  | All set A: median tokens in | 11.6M (n = 1,195) | 16.8M (n = 717) |
  | All set A: median steps | 44 | 89 |
  | All set A: median minutes | 14.9 | 26.0 |
  | Codex: median tokens in | 10.7M | 17.3M |
  | Codex: median steps | 50 | 99 |
  | Codex: median minutes | 15.7 | 25.7 |
  | Claude Code: median tokens in | 11.8M | 15.0M |
  | Claude Code: median steps | 42 | 73 |
  | Claude Code: median minutes | 13.9 | 26.3 |

  My half-window medians are 12.1M and 16.3M, against 10.9M and 13.1M in the report. Daily medians swing from 5M to 30M, so the day-level series is noise. The before/after rise is larger than that noise for steps and minutes.

  This rise cannot be called a trend in real cost. After 11-16, most Codex commits have no numbers, so the measured Codex commits may not be typical. Before the fix, Claude Code lines also double-counted tokens and gave subagents the coordinator's near-empty numbers. Both effects push in unknown directions. The supportable statement is narrower: on commits with numbers, steps and minutes per commit roughly doubled after 2026-11-15, and the data cannot say whether the work changed or the sample did.
- **Claude Code cost before the fix is not a measurement.** 721 of Claude Code's 926 set-A rows are dated before PR #3707. PR #3707 says those lines double- or triple-counted tokens and that subagents reported the coordinator's numbers ("Steps: 1"). The headline median and the agent comparison both pool these rows. The report states this only under "What this does not show". The Claude Code median and its 46% share of tokens are therefore unsupported as measurements of Claude Code's cost.
- **Zero steers after correction depends on the correction.** On the corrected values, 76% of post-fix commits have zero steers. On the raw values of all 732 post-fix rows that have a steers cell, the share is 70%. Some corrected zeros come from subtracting an equal running total. They mean "no new steers since the previous commit in the session", not that the session had none.

### Not checked

- I did not reproduce the claim that 176 of the 184 rows over 24 h link to an earlier row, or the exact distribution of steers. My rule differs from the report's, and both are heuristics.
- I did not check how Codex counts steers.
