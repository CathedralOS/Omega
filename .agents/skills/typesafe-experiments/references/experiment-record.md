# Omega TypeSafe experiment record

Distilled from the September 15–16, 2026 conversation and local reports. This is
not a verbatim transcript. User messages and observable results are preserved as
decisions and findings; credentials and private model reasoning are excluded.

## Intent and direction

Jarod uses Codex and local Devin SWE-2 Max, considers the local Devin allowance
effectively unlimited, and wants high-yield uses for fast atomic judgments. Do not
invent a paid-call budget from that assumption; API requests and elapsed time still
have costs. The repo is intended to be open source, and the user approved sending
sanitized repo content, completion claims, and command/output receipts for these
experiments. That historical approval is not permission to transmit secrets or
unrelated future data.

The conversation progressed from classifying outputs and checking completion
claims, through full-code traces and claim/evidence linking, to decision support
before and after coding-model calls. The useful distinction: Jev may help choose
what to read or review; tests, revision binding, and independent checks establish
execution facts. A saved rationale or commit message is itself a claim, not proof.

The user supplied an external anecdote about fast pre/post-generation decisions
and independent questions. Its latency and quality claims were motivation, not
our measurements. The latest direction is to keep tuning, preserve the learning,
and test whether it transfers rather than treat one failed gate as a model ceiling.

## Verified retained results

These reports were reread when creating this record. Earlier exploratory results
not reproduced here can be recovered from local artifacts; do not reconstruct
missing exact scores from conversational recollection.

### Evidence navigation: useful pipeline efficiency, not verification

`build/experiments/handoff-shadow/hybrid/REPORT.md`:

- Input tokens per selected historical workload: 96,560 → 17,976 (81.4% less).
- Median summed sequential API time: 4.663s → 0.871s (5.35x).
- Five source-reviewed narrow links retained in each of three repeats; four came
  from Jev, one exact test-count/timing link from code.
- Compaction, batching, deterministic filtering and resolution all contributed.
  No competitor superiority, human review-time saving, or general correctness was
  measured. A revision overclaim became a review flag, not an approval.

The earlier batching-only replication retained 4/5, 5/5, 4/5 links and reduced input
by 69.5%, narrowly missing its 70% target. Do not conflate it with the later hybrid.

### Retry adviser: apparent aggregate advantage came from synthetic cases

`build/experiments/handoff-shadow/retry-probe/evaluation-reviewed/REPORT.md`:

- Eight recorded transitions: Jev 7/8, simple rules 8/8.
- Four counterfactual unrelated-Markdown edits: Jev 4/4, rules 0/4.
- Aggregate 11/12 versus 8/12 hides a false warning on useful dependency diagnosis.
- Summed API time 3.036s; 14,931 input and 520 output tokens.
- Gate failed. No real retry was avoided or worker automatically stopped.

### Three-condition coding-context pilot

Snapshot `e9b5a67dac9d1777565957c7d3a294c88dd5be4d`, Windows, SWE-2 Max
(`3000.10.21` CLI), Jev `jev-1.13.0`. Deliberately seeded defects, not discovered
production bugs. Three tasks, two in the same Python module; one clean run per arm.

| Task | Normal discovery | Lexical context | Jev context |
| --- | ---: | ---: | ---: |
| Path overlap (`tools/claims.py`) | 405.73s | 552.15s | 430.57s |
| Audited docs (`tools/test_affected.py`) | 200.05s | 119.40s | 114.33s |
| Source readers (same module) | 134.34s | 123.73s | 97.03s |
| Total | 740.12s | 795.27s | 641.93s |
| Tool calls | 28 | 27 | 21 |
| Worker prompt tokens, including cached | 761,720 | 922,967 | 655,417 |

All nine trusted acceptance suites passed; 144 additional path-pair checks per arm
also passed. Jev selection added 1.118s, 22,969 input and 1,392 output tokens.
Times include selection and independent verification, but not common setup.

Frozen gate: all accepted and at least 20% lower aggregate time than BOTH baselines.
Result: failed (13.3% versus normal, 19.3% versus lexical). Mandatory AGENTS.md was
retained. Both selectors supplied two documents capped at 5,500 characters each;
candidate excerpts used first/last 1,000 characters. These caps did not equalize
actual context lengths. The docs case selected `tools/testing.md` under Jev but
not lexical ranking; its near-equal assisted times do not isolate that document's
causal benefit.

Operational failures matter: an empty sparse checkout invalidated the first normal
attempt; two Jev attempts stopped before edits on missing command permissions.
They were preserved and excluded from clean-run scoring, not hidden. Their worker
times were 31.80s normal and 168.85s Jev. Including them yields approximately
771.92s normal versus 810.77s Jev, before coordinator repair time. The second Jev
attempt also violated the separate-shell-command instruction.

Optional full claims suites took 286.09s normal, 381.78s lexical, and 315.57s Jev.
That runtime variation confounds speed attribution. There is no established
compiler-wide productivity improvement and no landed code from the pilot.

## Local recovery map

Paths below are relative to the original Omega checkout, not necessarily the
current worktree. They are ignored and machine-local; the tables above are durable.

- `build/experiments/context-pilot/`: `PROTOCOL.md`, `pilot.py`, frozen manifests,
  request/response JSON, `RESULTS.json`, `REPORT.md`, trusted tests and per-arm logs.
  Commands: `python build/experiments/context-pilot/pilot.py report` and
  `python build/experiments/context-pilot/pilot.py audit-paths` (`python3` on macOS).
  The harness was only run on Windows and depends on older local pilot helpers.
- `.codex/worktrees/ts-context-{paths,docs,readers}-{normal,rules,jev}`: disposable
  sparse worker checkouts. Seeds are orphan commits to prevent history-based fixes.
- `build/experiments/handoff-shadow/`: earlier linking, routing, holdout, batching,
  diagnostics and retry trials. Read their reports before reusing conclusions.
- `build/references/typesafe/pages/`: saved documentation fallback. Read relevant
  API, question and cookbook pages; prefer live docs when available.
- Raw `*-session.json` exports can contain private reasoning. Inspect only named
  public fields needed for metrics/tool observations; never bulk-upload them.

Known harness traps: `worktree add --no-checkout` plus sparse configuration did not
populate files; an explicit checkout population step was needed. Preflight must
require assertion failures, not just a nonzero exit (missing-file errors fooled the
initial check). Headless command permissions must be smoke-tested; do not solve
permission friction by globally enabling dangerous mode. Execute frozen trusted
tests outside worker control and check seed HEAD plus edited/untracked path scope.

## Follow-up: selection policy versus prompt wording

`build/experiments/context-tuning/PROTOCOL.md`, `tune.py`, `frozen.json`,
`request.json`, `response.json`, and `REPORT.md` retain the next bounded trial.
Six fresh coordinator-authored queries used the same frozen 13-document corpus:
four required-owner-document labels and two explicit self-contained controls.
One query reused the test-selector domain. Labels were frozen before inference;
this was not independent gold or a coding-worker trial.

| Policy | Required docs retained | Correct no-context answers | References | Supplied characters |
| --- | ---: | ---: | ---: | ---: |
| Lexical top-two | 3/4 | 0/2 | 12 | 60,296 |
| Broad relevance, top-two | 4/4 | 0/2 | 12 | 62,269 |
| Broad relevance, cutoff 0.5, up to two | 4/4 | 2/2 | 6 | 29,269 |
| Task-needed wording, top-two | 4/4 | 0/2 | 12 | 62,269 |
| Task-needed wording, cutoff 0.5, up to two | 3/4 | 2/2 | 4 | 18,269 |

The stricter prompt missed its promotion signal. The simple cutoff on the original
prompt retained all required documents and both abstentions while removing 53.0%
of would-be supplied context versus forced top-two. Both forced policies necessarily
fail no-context cases; do not present that constructed contrast as broad intelligence.
Required-document recall does not label every extra selected document as irrelevant.
The probability cutoff is a feasibility setting, not a calibrated universal threshold.

The stricter question scored the bootstrap owner 0.38 useful versus broad relevance's
0.61. Inspection found that first/last excerpts omitted the middle paragraph stating
the required manifest membership/order rules, though the full document contained it.
This is an input-coverage confound, not evidence that strict questions are inherently
worse. Ask whether an excerpt *locates* needed context versus *contains* the answer.

Joint evaluation: six calls, 2.283 seconds, 55,414 input and 5,918 output tokens.
Both variants shared each request, so this is not a single-policy deployment cost.
No worker ran, and no worker token/time savings were measured. Request identity was
checked after normalizing platform newlines; no answers were retried for correctness.

## Next tuning step

The two-reference rule always fills both slots, even for weak or irrelevant matches.
The cutoff probe above is complete; do not rerun it as fresh evidence. Carry the
simpler broad-relevance cutoff as the candidate. Check omitted-middle evidence on
new cases using query-centered excerpt windows, retaining deterministic selection
as a baseline. If that transfers, compare worker completion with a fixed validation
scope and preflighted permissions. Do not turn the 53% character reduction into a
worker-speed claim before measuring it. The six exposed queries are now development
examples, not a reusable blind holdout.
