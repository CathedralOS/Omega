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

## Follow-up: query-centered excerpt coverage

`build/experiments/context-windows/` preserves `windows.py`, `PROTOCOL.md`,
the frozen labels/request, API responses and `REPORT.md`. Ten new authored queries
used the same frozen 13-document corpus, with eight repository questions and two
self-contained literal transformations containing misleading repository terms.
They deliberately challenge buried details; this is not a representative random
sample or unseen-domain evaluation. One API run per query/arm, no accuracy retries.

The broad question and 0.5 cutoff stayed fixed. Centered excerpts use the existing
lexical ranker to select two nonoverlapping 1,000-character windows at 500-character
strides, reassembled in source order and capped at 2,015 characters. Short documents
are not duplicated. Query-window construction took approximately 0.05s total.
Labels include a required owner document and an exact evidence sentence, neither
used in extraction or sent to Jev. First/last excerpts exposed 1/8 evidence anchors;
centered excerpts exposed 7/8 before inference.

| Policy | Required documents | Selected exact evidence anchors | Correct abstention | References | Characters |
| --- | ---: | ---: | ---: | ---: | ---: |
| Lexical, first/last | 6/8 | 1/8 | 0/2 | 20 | 40,300 |
| Jev cutoff, first/last | 6/8 | 1/8 | 2/2 | 12 | 24,180 |
| Lexical, centered | 7/8 | 6/8 | 0/2 | 20 | 38,574 |
| Jev cutoff, centered | 8/8 | 7/8 | 2/2 | 13 | 26,126 |

Frozen promotion signal passed: no loss of required-document recall, both
abstentions, at least two additional anchors versus old Jev, and more anchors than
centered lexical. Most improvement comes from deterministic excerpt construction;
Jev adds one retained owner/anchor over centered lexical plus selective abstention.
Lexical forced top-two cannot abstain by design. No wholesale gain is attributed to
Jev alone. Joint API evaluation: 20 calls, 6.784s, 150,143 input and 10,174 output
tokens. No worker-speed claim follows from these retrieval scores.

Exact-anchor coverage is deliberately narrow. The live-worker question lacks its
frozen anchor but another selected recovery paragraph may support its answer. Keep
the frozen score; separately review semantic support rather than moving the label.

## Consumer follow-up: evidence-backed answers

The cutoff and excerpt probes are complete. The consumer probe used three
fresh SWE-2 Max sessions on the exposed ten-query batch: old Jev excerpts, centered
lexical excerpts, centered Jev excerpts. Require supported answer choices and exact
per-query citations; unknown is safe noncompletion. This tests downstream use, not
fresh generalization or coding productivity. `CONSUMER-PROTOCOL.md`, `consume.py`,
and `consumer-*` artifacts in the same local directory preserve the trial.
`CONSUMER-REPORT.md` and `semantic-review.json` retain the result and per-case
source judgments. All three workers made zero tool calls and every quotation
matched the per-query supplied text after whitespace normalization.

| Context | Correct options | Supported correct | Safe unknown | Incomplete/wrong-owner support | Worker seconds | Selection/window seconds |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Jev first/last | 6/10 | 3/10 | 4 | 3 | 136.01 | 3.501 |
| Lexical centered | 10/10 | 9/10 | 0 | 1 | 237.90 | 0.049 |
| Jev centered | 10/10 | 10/10 | 0 | 0 | 56.66 | 3.332 |

The concrete lexical miss is useful: a question about removed **work-claim**
tickets received the **landing-queue** document. Devin selected the expected
answer but cited a different registry's retained-history rule. Jev supplied
`tools/claims.md`, and Devin cited the requested registry's actual rule. Exact
quote matching and answer-key accuracy both missed this provenance error.

The old excerpts yielded four safe unknowns plus three correct choices with
incomplete evidence: exit-code semantics from generic queue text, liveness from
advisory ownership, and a compound exclusion claim from doctest-only guidance.
The centered live-worker passage supported its answer despite lacking the frozen
anchor. This is why literal-anchor recall and semantic support remain separate.

Support review was coordinator-authored, unblinded, and conservative about full
answer entailment. Ten questions shared each worker prompt; per-question isolation
was instructed, not enforced. No code was repaired. Fixed arm order, provider
cache variation, and one run per arm prohibit claiming a repeatable 4x speedup.
Prompt telemetry was 27,978 / 31,572 / 28,576 tokens respectively, including cached
tokens; common preparation and coordinator review were not timed. The quality
signal is promising on this constructed batch, not compiler productivity proof.

## Fresh isolated owner-confusion replication

`build/experiments/context-owner-holdout/` contains the frozen protocol, six fresh
queries and owner rubrics, `owners.py`, API responses, twelve worker exports,
`semantic-review.json` and `REPORT.md`. Each question had its own fresh SWE-2 Max
session per arm, alternating which arm ran first. The same broad question,
centered extraction and 0.5 cutoff were retained. Six Jev calls took 2.088s with
45,003 input and 2,877 output tokens. No answers were retried.

Questions contrasted work-claim renewal versus landing nonrenewal, assignment
recovery versus active-reservation cancellation, and test-selection dirty-input
coverage versus publication's clean-worktree requirement. These are new questions
in familiar domains, not a broad independent task sample.

| Strategy | Correct choices | Strictly supported | Partial support | Total seconds | Worker prompt tokens including cached |
| --- | ---: | ---: | ---: | ---: | ---: |
| Centered lexical | 6/6 | 5/6 | 1/6 | 161.96 | 133,861 |
| Centered Jev | 6/6 | 5/6 | 1/6 | 142.24 | 133,516 |

Primary quality replication failed: a tie. Aggregate time was 12.2% lower with Jev,
but median paired Jev/lexical time ratio was 0.951, missing the frozen 0.8 target.
The earlier dramatic latency gap did not replicate. Totals include window
construction, selection and worker process time, not common snapshot loading,
prompt construction or coordinator review. All quotations matched supplied text,
all workers made zero tool calls, and the working directory remained empty.

Both cancellation answers inferred the prohibition from the separate examples
`cancel --ticket <waiting-ticket>` and `release --claim <active-claim>`. Their
identical excerpts cut off before the explicit prohibition. The frozen strict
rubric marks that incomplete; a permissive reading would give BOTH 6/6, still
no advantage. This common failure belongs to excerpt boundaries, not Jev ranking.

Lexical retrieval's renewal answer is supported by a swarm template explicitly
invoking `claims.py renew`. Its missing preferred `claims.md` document is not a
semantic failure. Grade the rule's actual owner rather than requiring one filename.
Coordinator support review remains unblinded; no coding productivity or generic
speed advantage has been established.

## Paragraph extraction and citation-support pivot

`build/experiments/context-paragraphs/` preserves eight fresh queries, their frozen
anchors, extractor self-checks and report. Both extractors found 8/8 required files.
Whole paragraphs retained 8/8 full evidence anchors versus character windows' 7/8,
with no lost anchor. Selected text grew from 31,063 to 31,950 characters under the
same 2,015-character cap per document. The gate required two additional anchors;
it failed, and no Jev calls were spent on the now-saturated 8/8 lexical baseline.
Oversize indivisible paragraphs are omitted, not silently truncated. This is a
small deterministic input improvement, not a model or coding-productivity win.

The next probe targeted semantic citation support, reusing public final-answer and
quotation fields from observed workers. `build/experiments/citation-support/`
contains the frozen protocol, `support.py`, request, labels, per-run results and
`REPLICATION-REPORT.md`. Raw sessions/private reasoning were not sent. The unchanged
Choice prompt asks whether quotations jointly support the complete claim for the
entity and scope requested. A secondary guide is allowed if it explicitly describes
the right subsystem. Wrong-owner support, missing conditions and partial support
are warning candidates. This does not establish whether an unsupported claim is false.

The initial source-reviewed development subset had 20 cited answers: 16 supported,
four insufficiently supported. Later recorded queries supplied ten clear positives
and two debatable command-example cases. Six inverted claims retained their observed
citations as explicit synthetic negatives. Unknown answers and self-contained
transformations were routed deterministically, not scored by Jev.

| Run | Observed support issues flagged | False warnings on 26 clear supported outputs | Synthetic inversions rejected | Three-batch seconds |
| --- | ---: | ---: | ---: | ---: |
| Initial | 3/4 | 0/26 | 6/6 | 1.106 |
| Fixed repeat 1 | 4/4 | 0/26 | 6/6 | 0.876 |
| Fixed repeat 2 | 3/4 | 0/26 | 6/6 | 1.026 |

Stable catches: exit-code/resumption inferred from generic queue text, full
validation scope inferred from doctest-only support, and a work-claim answer citing
landing-queue history. The weak liveness justification flipped at P(supported)
0.61 / 0.48 / 0.61. Both ambiguous cancellation examples were accepted every time.
The original catch-all gate failed; the favorable middle repeat does not replace it.
Every run used 13,168 input and 1,209 output tokens for 38 judgments. This is summed
batch latency, not per-answer service latency. Repeats are not independent cases.

Mechanical quotation matching had passed for the original answers; it is not a
strong competing semantic verifier. Labels remain unblinded coordinator judgments.
The later clear recorded cases are positives only, so there is no independent real
negative holdout. No worker was interrupted, answer automatically changed, or
publication authorized. This is a candidate cheap warning layer, not a certificate.

## Next tuning step

The 30 exposed retrieval queries are development data; lexical context extraction
is a strong baseline and further reranking trials need demonstrated headroom.
The more promising next experiment is a nonblocking citation-support warning on
fresh source-backed answers. Freeze the current prompt; inspect flagged AND
unflagged outputs, retain owner/scope evidence, and measure whether a warning leads
to a useful correction. No automatic blocks or verifier retries. Calibrate a new
threshold only as a development hypothesis with fresh evaluation; do not promote
the favorable repeat or a post-hoc cutoff as established correctness.
