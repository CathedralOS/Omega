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

A second thread (2026-09-18, `build/experiments/proof-smell*`) moved Jev onto
the language itself: can it flag logically false Omega programs. Three probes
landed at a stable profile — ~0.1% false-positive rate at 1,900-program
scale, ~55-60% recall on unannotated logic garbage, ~81% with comments. The
defensible use is residue-scoped advisory (classify "cannot prove" rejections
as contradiction vs capability gap), never a filter. Dated sections at the
end carry the measurements; the remaining unknowns are real-bug behavior and
obligation-rendered state.

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

## Fresh citation shadow probe

`build/experiments/citation-shadow/` preserves the protocol, runner, pinned inputs,
public answers, pre-Jev semantic review, response and report. Four new free-text
questions used complete claims/landing documents at
`e9b5a67dac9d1777565957c7d3a294c88dd5be4d`, one fresh SWE-2 Max session and the
unchanged support prompt. No injected errors or deliberately truncated sources.

All four answers were substantively correct against full sources and all quotes
matched. Before seeing Jev, coordinator review found one citation-completeness
defect: the authority answer added an `--allow-overlap` assertion without quoting
the passage that establishes it. The source contains it, but its five citations
do not. Jev accepted all four (P(supported) .98/.95/.96/.90): 0/1 defects flagged,
0/3 false warnings, no correction triggered. Frozen correction gate failed; no
repair workers were run. Worker 42.931s, API .309s, 2,211 input / 122 output tokens;
preparation/manual review excluded. This establishes neither savings nor a
reliable detection rate. One session, four related questions, unblinded labels.

An initial extraction assertion caught Windows default-decoding mojibake, not a
worker error. Explicit UTF-8 decoding fixed the reader; retained export, no worker
retry. All checks then passed. This matters for future quote-presence checks.
Live citation-cookbook access again failed; cached primary documentation was used.

Decision: whole-answer support can miss an unsupported additional assertion even
when most of the answer is strongly supported. Keep warnings nonblocking. Test
per-assertion judgments next on development data, counting segmentation cost,
then use fresh evaluation before claiming a benefit. Do not tune a cutoff on this
single failure and relabel it generalization.

## Next tuning step

The 30 exposed retrieval queries are development data; lexical context extraction
is a strong baseline and further reranking trials need demonstrated headroom.
After the expanded-input replay below, freeze its scoped-support prompt candidate
and compare against the original on NEW claims with partial-answer queries and
wrong-owner evidence. Keep completeness separate from individual claim support.
No further tuning on the 19 exposed replay cases. Compare against deterministic
expansion and lexical selection, not only expensive answer rewriting; inspect
flagged AND unflagged outputs and retain owner/scope evidence.
No automatic blocks or verifier retries. Calibrate a new
threshold only as a development hypothesis with fresh evaluation; do not promote
the favorable repeat or a post-hoc cutoff as established correctness.

## Claim-first repair retained for experiments

`build/experiments/citation-atomic/` preserves the protocol, isolation amendment,
requests, answers, pre-verifier review and report. Seven manually decomposed
development claims retained the original citations. Both shared-state and isolated
atomic checks flagged the missing overlap support at P(unsupported)=1.0, accepting
the six supported controls. The shared whole-answer judge also flagged it, so an
isolation control was added before continuing interpretation. The isolated whole
answer likewise flagged it at .63 unsupported; the earlier .90-supported verdict
is variable. This does NOT establish atomic checking's general superiority.

The actual producer fix avoids a separate decomposition call: its ordinary answer
generation emits claim/citation lists, with no additional unchecked prose. One
fresh SWE-2 Max session (68.901s) repaired the old answer and answered three fresh
queries. All six repair claims and nine fresh claims were source-reviewed before
Jev, all supported and complete. The overlap claim now cites the explicit informed
exception passage. All quotes matched; zero tools, no local warnings, all 15
semantic checks accepted (.365s, 4,928 input / 498 output tokens).

The shared diagnostic, isolated whole, and isolated atomic calls took .309/.281/.269s
with input tokens 3,958/790/3,432 and output 241/31/213. Total four API batches:
1.224s, 13,108 input / 983 output. Atomic checking costs more tokens; no measured
speed/cost win. Manual development decomposition cost was not timed; no automatic
decomposition is part of the retained producer workflow. Generation overhead has
no matched baseline. No ordinary-review comparator, so the useful correction is
not evidence of advantage over another reviewer. Fresh cases contain no defects.

Retained portable `scripts/claim_checks.py` is used by the producer experiment.
It validates answer IDs, forbids separate unchecked prose, produces one semantic
case per cited claim, retains local warnings for absent/nonliteral evidence and
abstention, rejects missing/unknown verdicts, and preserves every unsupported
claim instead of averaging. Offline self-check covers those boundaries. Python
compilation, live payload checks, skill validation and diff checks passed on
Windows; macOS runtime not exercised. Some emitted claims remain compound:
semantic atomicity and query completeness still require review. Warning-only,
not a certificate or publication gate. Raw sessions and credentials stay ignored.

## Review cascade and nongenerative evidence repair

`build/experiments/citation-cascade/` retains both frozen protocols, scripts,
source inputs, reviewer outputs, semantic reviews, metrics and REPORT.md. Replay
of all 20 earlier compact claims (16 supported, four actual support defects), not
fresh or independent evaluation. A development-only P(supported)<.90 rule routed
six cases including every defect; 6,503 input / 613 output tokens in .374s.

Ordinary SWE-2 Max review of all 20 took 130.808s and repaired four defects without
regressions. Selected review of six took 175.595s including routing: 34.2% slower,
same four repairs, two unnecessary supported-answer rewrites. The volume gate
passed but time gate failed. Same per-task source evidence and instructions;
one run per arm, selected first, no correctness retries. Two batched calls, not
fourteen avoided calls. Repeated questions also require a grouped baseline.

The follow-up removed regeneration entirely: preserve claims, group exact
query/claim/allowed-source matches, retrieve six whole paragraphs per group with
the existing lexical ranker, and let Jev select supporting evidence. Four unique
groups, 24 support questions. All four observed defects repaired, no unsupported
replacement; six selected records got citations, fourteen stayed unchanged.
Concrete output: `span-repair/repaired-answers.json`. All 20 claim strings retained.

Routing .374055s + preparation .005516s + repair API .399118s = .778689s measured
stages versus 130.808s observed all-review process. Repair API 8,889 input / 747
output tokens; combined API usage 15,392 / 1,360. Excludes common source loading,
small-script startup, final serialization and manual review; not a repeated
end-to-end timing or dollar-cost result. Windows quote/coverage/unchanged-claim
checks, zero worker tools, Python compilation and skill validation passed.

Crucially, deterministic lexical top-one ALSO repaired 4/4. Its liveness recovery
paragraph adequately supports the narrow original claim, consistent with prior
accepted labels; Jev chose more explicit support but earns no incremental quality
win here. Retain nongenerative repair as the promising workflow direction, not
evidence of Jev superiority. Exposed positive-repair tasks contain no absent/false
claims; safe abstention and fresh transfer remain untested. Single-paragraph
support is the prototype ceiling. No automatic publication or production approval.

## Evidence-repair abstention controls and reusable recipe

The skill now links `references/evidence-repair.md`: preserve claims, retrieve
whole paragraphs, judge full support, copy source exactly, and abstain or escalate
when evidence is absent. Grouping includes revision and permitted source identity.
It retains the lexical baseline and distinguishes removed generation cost from
unique model benefit. Model thresholds remain development settings, not guarantees.

`build/experiments/evidence-abstention/` contains eight authored controls and frozen
labels: four supported and four unsupported (wrong owner, numeric contradiction,
invented limit, partial support). Same six candidates, paragraph cap and .90
cutoff; one isolated request per case. Jev policy selected correct evidence for
4/4 positives and abstained on 4/4 negatives. Lexical top-one also supported 4/4
positives but attached irrelevant/contradictory evidence on all four negatives;
always-abstain answered none. Manual source review confirms chosen positive
passages, not just expected presence. This is a semantic-policy signal on authored
controls, not fresh naturally occurring errors or general safe automation.

Wrong-owner warning: landing-lease text received .79 support for a work-claim
assignment deadline. Two fixed isolated repeats yielded .72/.79. The .90 cutoff
prevented attachment in all three runs, but the raw Choice winner was wrong.
Do not describe this as perfect semantic accuracy or calibrated confidence.
Four positive controls share three passages; repeats add no independent examples.

Eight initial calls: 2.632s, 17,352 input / 1,512 output tokens. Two boundary calls:
.318/.263s, each 2,269 input / 189 output. Total 3.214s and 21,890 / 1,890 tokens,
not end-to-end workflow latency. No generation calls. Protocols preserve the failed
live documentation read and cached-source fallback. Windows source-substring,
case-cardinality, Python compilation, skill validation, links and diff checks pass;
macOS runtime not exercised. Next: real cross-domain failures and explicit source
owner context, not more threshold tuning on these exposed controls.

## Fresh compiler-domain output and deterministic quote expansion

`build/experiments/compiler-evidence/` records four new compiler-architecture
questions, full pinned pipeline.md, one fresh SWE-2 Max session and 15 observed
claims. No injected failures or truncated context. Worker 58.970s; all quotes
literal, zero tools, four complete answers. Before Jev, source review found one
citation gap: a claim named runtime-ABI carriers/calling-conventions but quoted
only "those owners", omitting their antecedent sentence. Full source supported
all 15 facts; isolated quotations supported 14. Documentation support, not runtime
compiler validation; manual unblinded labels.

Existing .90 cutoff flagged the gap at .89 and two supported claims at .82/.62:
one natural support gap detected, 2/14 false warnings. Raw Choice still favored
supported for the gap. Initial API .410533s, 4,732 input / 498 output tokens.
Six candidates per flag produced three supported whole-paragraph selections,
all identical to lexical top-one. The ABI paragraph restores the missing owner
names. Repair API .376138s, 7,090 input / 561 output, preparation .009052s; combined
stages .795723s. All claim strings unchanged; no regeneration or new support defect.

A frozen follow-up used unique containing-paragraph expansion of original quotes,
same source revision/path and size cap. Three of three results matched selected
paragraphs byte-for-byte in .000383s local computation. This avoids the SECOND
7,090-input/561-output model call for this class of repair; initial semantic
screening remains. Initial API plus expansion .410916s, excluding file reads,
process startup and manual review. No generalized end-to-end speed claim.

Recipe upgraded to attempt unique literal-quote expansion before search, preserving
all quote text and refusing ambiguous/missing matches. Containment never means
semantic acceptance; false claims can occur inside relevant paragraphs. The
incremental Jev benefit remains unsettled because deterministic methods also
repair this gap, and two false warnings remain. Windows quote/ID/tool/claim/parity,
Python compilation, metadata/link and diff checks passed; no macOS runtime or
production integration. Next: fresh short-quote versus expanded-input comparison
including negative controls, rather than repeated tuning on exposed examples.

## Expanded-input replay and scoped-support prompt candidate

`build/experiments/expanded-input/` preserves two frozen protocols, four A/B calls
and two follow-up calls, inputs, outcomes and report. Fifteen observed compiler
claims plus four authored unsupported controls: all exposed development data.
Claims, model and .90 cutoff fixed. Original short evidence versus unique
containing paragraphs, separate request states in short/expanded/expanded/short
order. The four negatives already had full paragraphs in BOTH arms, so this is
not a negative-expansion transfer test.

False warnings on 14 commonly supported claims: short 2/3 across two runs versus
expanded 1/1. The missing-antecedent claim properly becomes supported after
expansion and is excluded from this denominator. Both expanded runs accepted it;
the second short run also accepted it, missing the support gap. All four controls
rejected in all calls. Expanded-input gate passed. Input 7,428 vs 6,103 (+21.7%),
output 614 each. API short .355/.337s; expanded .379/.284s, no speed claim.

The expanded runs still flagged the literal normalizer-identity claim. A separate
two-call development hypothesis clarified "support this single claim" versus
"complete the entire answer", retaining query owner/scope and requiring support
for every assertion actually present. Both clarified runs had zero false warnings
on the 14 common positives, accepted the repaired antecedent claim, and rejected
all four negatives. Thus 15 positive/4 negative decisions correct on this exposed
replay, not 38 independent cases or proven general reliability. The inferred
cause of the earlier warning remains a hypothesis about prompt interpretation.

Scoped calls .385/.310s, 8,416 input / 614 output each. All six calls: 2.051s,
43,894 input / 3,684 output tokens. No generator call or end-to-end timing claim.
Candidate clarification saved in evidence-repair.md; original prompt/results
remain unchanged. Freeze candidate for fresh evaluation, do not promote to an
automatic gate. Complete-answer coverage is a separate requirement. Windows
unique-containment, unchanged-claim, response-coverage, Python compile, metadata
and diff checks passed; macOS not run. Live State docs again inaccessible; existing
cached guidance/integration reused without new API assumptions.

## Cookbook-driven work-packet example: context did not fix policy

The user prefers worked examples followed by exploration to reduce wasted effort.
Direct HTTP successfully refreshed the official TypeSafe Markdown reference:
109 saved URLs, 108 distinct contents, 18 cookbook files, zero failures and checked
SHA256s. The web fetcher failure was not site unavailability. Discover through
https://docs.typesafe.ai/llms.txt; /cookbooks.md redirects to a single cookbook.
Ignored local index/review: `build/references/typesafe/README.md` and
`COOKBOOK-REVIEW.md`. The parallel-questions example demonstrates batching cost
and serial-call latency savings, not that more context improves accuracy.

`build/experiments/work-packet/` adapts that shared-state shape to recorded r21:
two-package Clippy failed in syntax-trees-to-symbol-resolved-trees, and a worker
proposed checking the emitting dependency alone. Previously Jev incorrectly
recommended retry review. This is a known development failure, not fresh gold.
Frozen five-question map retains the original retry wording and adds diagnostic
owner, historical attribution, proposed-check purpose and repair evidence.

Six calls in symmetric short/full/distractor/distractor/full/short order, no
correctness retries. Input tokens per call: 1,822 / 16,165 / 19,549. Full adds the
retained receipt plus pinned AGENTS.md/testing.md; distractor adds another actual
worktree's AtomicEvent error. Reference rules are not historical revision proof.
API seconds: short .362/.249, full .386/.409, distractor .433/.410. Total 2.249s,
75,072 input and 1,344 output tokens. No worker call or actual retry avoided.

All arms got the four descriptive judgments right: resolver owns the diagnostic,
proposed check isolates that dependency, attribution unresolved, repair not shown.
All arms still incorrectly selected retry review, P(continue) .20-.24. Independent
questions do not consume each other's answers: a correct premise and an incorrect
policy conclusion can coexist. The existing package-scope rule already correctly
continues this example. Reject larger context as this error's remedy; neither a
large-input nor unique Jev advantage was demonstrated.

Next hypothesis: let code compose semantic observations instead of outsourcing
the whole workflow policy. This is post-result direction, not a validated gate.
Use a neighboring recorded example that challenges the deterministic baseline
before expanding the experiment. Request hashes, question parity, response coverage,
sanitization checks and Python compilation passed on Windows; macOS not run.
No production integration, worker interruption or publication.

## Semantic observations composed in code

`build/experiments/decision-composition/` follows the official speculative fan-out
support-triage example (https://docs.typesafe.ai/patterns/fan-out.md, read live).
Keep the original direct-policy question and ask two independent observations:
meaningful diagnostic change and attempted relevant edit. Frozen code continues
when either is affirmative, recommends review when both are negative, otherwise
returns insufficient. Explicit unknown labels, no tuned numeric threshold.

On the eight exposed recorded transitions, composition 8/8 versus current direct
Jev 6/8 and existing rules 8/8. On four existing authored unrelated-prose controls,
composition/direct 4/4 versus rules 0/4. Direct Jev warned incorrectly on r01's
library-only diagnostic and r21's isolated dependency check. Composition retained
the real redundant-rerun warning r25 and correctly continued relevant edits.
Questions share each request, so this is not an isolated prompt-arm comparison.

The twelve-case replay took 3.198s, 18,975 input / 1,400 output tokens. One fresh
neighbor r05 was then frozen before inference, without changing questions or code:
same command after replacing String::from_utf8 on Arc bytes with borrowed UTF-8
conversion and to_owned. All policies correctly continued; observations same /
relevant matched source review. Neighbor .277s, 4,690 input / 117 output. Manual
labels, one case, no subsequent execution outcome supplied; relevance is not proof.

Retain composition as a development candidate, not a big win or production gate.
No incremental quality over rules on recorded cases, and a stronger rule ignoring
unrelated Markdown edits could address the synthetic advantage. Next test requires
a recorded semantic distinction beyond that baseline, not more tuning of this set.
All 13 calls 3.475s, 23,665 input / 1,517 output, no retries. Policy self-checks,
request hashes, response coverage, sanitization and Python compilation passed on
Windows. No macOS runtime, worker interruption, compiler change or publication.

## Validation obligation versus useful diagnosis; retry lane parked

The next command/event inventory covered 35 recorded transitions without finding
an obvious unique model advantage worth asserting. A bounded follow-up tested
three actual cases: r09 allows permissions-set-readonly-false, r22 allows
needless_lifetimes, r26 replaces a missing function call while retaining the
check. This is hand-selected development evidence, not a representative audit
of all 35 full traces. No subsequent outcomes were supplied to the model.

`build/experiments/decision-composition/BOUNDARY-PROTOCOL.md`, `boundary.py`,
`validation-boundary/` and `BOUNDARY-REPORT.md` preserve frozen labels, inputs,
responses and conclusions. Previous questions/composition stayed fixed; one
independent question asks whether the preceding blocking requirement remains
enforced. Jev correctly labels weakened/weakened/preserved. Direct and composed
retry policies continue all three, compatible with diagnostic utility but NOT
original-gate satisfaction or permission to publish. Preserved does not prove
the attempted source repair succeeds.

A small added -A flag detector also gets 3/3. Its scope is these explicit CLI
waivers, not source attributes, hidden config or general command validity.
No model-specific gain. Three calls .354/.249/.281s, total .884s, 9,008 input /
480 output tokens, no retries. Windows hash/coverage/sanitization checks, flag
positive/negative assertions and Python compilation pass; no macOS runtime.

Retain the conceptual separation between useful diagnosis and original-check
acceptance. Park retry-adviser tuning: the recorded evidence still does not
justify a model in this path. Revisit only for a genuine recorded semantic
distinction beyond deterministic command/flag handling, not more constructed
examples or a deliberately weak baseline. No production change or publication.

## Pre-planning workflow awareness: positive but bounded consumer signal

`build/experiments/workflow-routing/` adapts the official skill_suggestion cookbook
(https://docs.typesafe.ai/cookbooks/skill_suggestion.md, live stages 3-4 read) to
61 installed descriptions, including repository overrides. Full descriptions and
explicit none; no two-stage reranking needed. These are installed skills, not a
claim all are active in every agent session. No mandatory instructions removed.

Eight actual public user requests from the recorded Omega rare-meeting session:
six actionable routes, two conversational remarks. Labels frozen before Jev;
hand-selected swarm-heavy data with no surrounding conversation supplied. Jev
1.13.0 got 8/8 first-workflow and 8/8 operation labels; IDF keyword overlap got
4/8 routes. Misroutes included wizard for draining and to-spec for a concurrency
request. A stronger hand-written router was not evaluated. API median .318s,
range .271-.418s, summed 2.595s, 46,960 input / 5,471 output tokens, no retries.

Unlike the earlier classifier-only probes, five cases then went to two fresh
SWE-2 Max planning sessions with the same full catalog. Plain chose intended
workflow 4/5; Jev-hinted 5/5. Both got immediate operation 5/5 and preserved
inspect-before-relaunch, drain without refill, and ten-agent concurrency.
The observed correction was treating a throughput remark as discussion without
selecting local-swarm. Plain also supplied numeric machine limits absent from the
packet; hinted did not. Neither worker actually loaded a skill or executed tools.

Plain process 61.526s; hinted 41.728s plus 1.565s relevant Jev overhead = 43.294s.
Observed 29.6% lower measured stages in one fixed-order pair; not a repeatable or
causal speed claim. Shared five-case prompts, prior case exposure and provider
variation limit generalization. Common preparation/coordinator review excluded.
No coding task completed or actual unnecessary worker launch prevented.

Both plans mention a no-refuel manifest marker without demonstrating a real
field. Full workflow instructions and live state must precede execution; even a
correct intent hint does not validate generated operational details. Retain this
as an advisory pre-planning candidate, not a mandatory gate or justification to
remove catalog/mandatory instructions. Next: fresh end-to-end planning work, not
more tuning these excerpts. Hashes, response IDs, sanitization, lexical self-checks,
Python compilation, zero worker tool calls and empty workspace checked on Windows;
no macOS runtime or production changes. Raw sessions remain local, not uploaded.

## Avoiding supervisor wakeups: positive narrow handoff replay

User clarified the objective: faster completion and better decisions, not API
equivalent dollar savings on effectively unlimited local Devin. Founder article
https://www.completeskeptic.com/p/kv-cache-rules-everything-around motivated avoiding
repeated large-context supervisory turns, rather than switching the full context
between models. Read September 16 UTC; its simulated billing economics are not
measurements of this setup. Live fan-out docs supply the independent-questions,
code-owned-branching recipe. Neither source proves Jev infallibility or architecture.

`build/experiments/planner-wakeup/` retains protocol, probe.py, frozen sources and
labels, public answers, API receipts, supervisor exports, SUMMARY.json, REPORT.md
and retained-answers.json. Three new read-only discovery assignments used pinned
sources at e9b5a67dac9d1777565957c7d3a294c88dd5be4d: offline CLI boundaries,
working-tree/manual test coverage, and publication network-error interpretation.
Domains are familiar from previous work. One fresh SWE-2 Max producer batch took
51.683s. All three answers were complete and source-supported before Jev review.

Code checks process/source/citation boundaries, then retains only if separate
coverage and support questions both return yes with P(yes)>=.90. Fixed development
threshold, not calibrated confidence. Missing answers fail to review. Full source
is provided, not just quotations; largest original request 11,579 input tokens.

| Original | Jev seconds | Fresh SWE-2 Max supervisor seconds | Both |
| --- | ---: | ---: | --- |
| Offline boundaries | .407 | 20.720 | Retain unchanged |
| Test-selection coverage | .277 | 33.125 | Retain unchanged |
| Publication network error | .228 | 22.828 | Retain unchanged |
| Total | .911 | 76.673 | 3/3 agree |

Six initial authored controls (partial-true and partial-false) all reviewed.
Three later complete-answer single-predicate inversions also reviewed, with
support P(no)=1.0. Coverage was contaminated by factual wrongness: it also returned
no for these full-coverage controls, so the two features are not independently
validated. Three mechanical controls reviewed without API. Overall 15/15 routing,
but only three natural positives and ZERO naturally occurring negatives. Literal
quote/status checks alone would retain all nine semantic controls; this is not
proof against a stronger custom deterministic policy. Always-review skips nothing.

The three expensive supervisor calls actually ran for comparison. Candidate replay
would skip all three; no production call was suppressed. .911s versus 76.673s is
isolated review service cost, NOT a general agent speedup: cold fresh CLI sessions,
one run each, fixed order, handpicked tasks, no warm persistent/grouped-review
baseline. Producer/common setup/manual review excluded. No cache billing or coding
completion claim. Retained artifact copies original answers/quotes unchanged.

All twelve API calls including controls: 3.627s, 63,831 input / 876 output tokens.
Zero tools in all four workers, empty workspace, no correctness retries. Windows
hash, ID, quote, routing self-checks and Python compilation pass; macOS untested.
Retain for natural handoff shadowing, not a production approval gate. Next highest
value test: actual worker arrivals with natural failures and missing requirements,
compared to a warm supervisor, measured through the next consumed workflow step.
No dropped validation, worker cancellation, publication or authority changes.

## Continuing supervisor and natural outputs: speed gain rejected on quality

`build/experiments/warm-handoff/` contains protocol, probe.py, frozen original
task/answer hashes, Jev responses, two resumed supervisor histories, public-only
metrics, consumed answer/retrieval artifacts, SUMMARY.json and REPORT.md.
Six unmodified outputs from earlier retrieval experiments: three supported and
three insufficiently supported by their original excerpts. Natural model outputs
but handpicked and previously exposed, NOT fresh holdout or live compiler arrivals.
Original source/answer matching verified; no synthetic mutation or new truncation.

Unchanged coverage/support questions and .90 threshold. Jev got 5/6: accepted all
three supported answers, flagged two of three unsupported ones. It accepted a
work-claim ticket answer backed by landing-queue history at support .95 / coverage
.98. Six calls 1.378s, 8,404 input / 438 output tokens, no correctness retries.

Two actual SWE-2 Max conversations resumed by explicit session ID. All-review
used six review turns; selective used two, actually skipping four in the replay.
Consumer code emitted unchanged answers or scoped evidence-retrieval requests;
no retrieval/repair or production action was executed. Both supervisors stayed
read-only with zero tools and empty workspaces.

| Strategy | Review calls | Review stages | Including setup | Unsupported retained |
| --- | ---: | ---: | ---: | ---: |
| Continuing all-review | 6 | 122.970s | 138.432s | 1/3 |
| Jev plus selective continuing review | 2 | 57.065s | 74.127s | 2/3 |

Selective stages include all Jev calls plus 55.687s supervisor time. Observed
53.6% lower review-stage time, 46.5% with setup, but the zero-unsafe-retention
quality gate FAILED and selective quality regressed. One interleaved stream pair,
not repeatable coding productivity or completed repair. Source prep, human review
and serialization excluded from measured stage time; producers were archived.

The always-review supervisor made the same wrong-registry error as Jev. A second
failure came from composition: Jev correctly flagged missing support for compound
test-scope exclusions, but the selective fallback supervisor approved from generic
test-selection/doctest language WITHOUT filling the evidence gap. All-review
rejected that packet. Different conversation histories and model variability are
not isolated causes. A bare changed verdict must not be treated as resolved evidence.

Resumption confirmed by fixed session identity and increasing exported steps.
CLI processes still restarted. Cached-token telemetry increased 8,192 -> 35,137
for all-review and 0 -> 8,192 for selective; final total prompt telemetry 178,434
versus 68,652 includes setup and cached tokens. Some caching, not proof of full
resident KV warmth or billing savings. Do not call it a fully warm resident benchmark.

Decision: reject automatic skip/override integration; keep shadow only. The next
hypothesis is explicit assignment evidence-owner binding and evidence-bearing
resolution of flagged gaps, possibly fetching authoritative source directly
instead of another same-evidence model vote. Do not raise the threshold to hide
the known owner failure. Retain this failed stream as regression; changed policy
needs fresh operational handoffs. Windows identity/hash/citation/artifact/session,
zero-tool and Python compile checks passed; macOS not run. No production changes.

## Owner-bound acquisition fixes structure; fresh prose gate still over-warns

`build/experiments/owner-bound/` preserves protocol, probe.py, regression evidence
updates, original answers, fresh producer outputs, labels frozen before Jev,
requests/responses, SUMMARY.json and REPORT.md. The linked evidence-repair guide
now carries the owner/warning/artifact boundaries, not an automatic approval rule.
TypeSafe State docs read live; no new API contract or supervisor framework.

Declared required source paths and historical revision are caller-owned inputs.
They are not inferred by Jev; historical annotations add known contract information
to exposed examples. Fresh assignments name the owners before generation. Code
checks owner citation coverage and literal membership. A pending normalized
source-content identity prevents bare same-evidence approval from clearing a gap;
unchanged evidence stays in review rather than refetching in a loop. New source
permits reevaluation, not acceptance. The semantic questions and .90 cutoff stayed
unchanged. Whole declared owner files are fetched at the same pinned revision.

On the six prior natural handoffs, reuse the initial Jev receipts. Code now holds
the wrong-registry answer even though old support was .95. Simulated bare positive
approvals cannot clear any of the three previous gaps. Three actual owner-document
fetches plus three new API calls resolve the work-claim-history and test-scope
evidence defects without rewriting answers. Five of six historical answers become
ready, zero unsupported retention. Wait-exit remains held: coverage .89, support
.99 despite sufficient full source. Frozen six-of-six completion target fails.
Retrieval 1.283s, new regression API 1.916s; not complete workflow or a matched
speed comparison against the previous review-only artifact. Source acquisition
itself supplies missing facts; no unique Jev benefit over always-fetch-owner proved.

Three previously authored complete-false controls remain held with correct owner
sources; these are exposed controls, not natural negatives. One fresh SWE-2 Max
batch (51.104s) then answers three new questions with full declared owner sources:
lease models, invisible source-reader coverage, and the two exit-2 meanings.

| Fresh answer | Pre-Jev source review | Support yes | Coverage yes | Gate |
| --- | --- | ---: | ---: | --- |
| Claims versus landing leases | Supported | .79 | 1.00 | Unnecessary review |
| Source-reader fallback | Overclaim | .87 | .99 | Review |
| Claims versus landing exit 2 | Supported | .89 | 1.00 | Unnecessary review |

The reader answer says "anything not recorded falls back to full selection",
turning a maintainer obligation into an automatic guarantee. Pinned selector
implementation confirms known Rust changes use declared and SOURCE_READERS edges;
unrecorded runtime reads cannot discover themselves. Strict-support label fixed
before inference, with exact text/rationale retained. A permissive policy-only
reading could remove this defect; labels are unblinded coordinator review.

Jev raw support choice is yes for all three. The threshold holds the overclaim
but also both good answers: zero fresh bypasses, two false warnings. A scalar
threshold cannot retain both supported answers while rejecting the overclaim in
this sample. No cutoff tuning or correctness retry. Fresh API .965s; all nine new
calls 3.794s, 35,302 input / 657 output tokens. Three old positive receipts reused.

Retain structural guards for experiments, not the whole-answer skip gate. Next
useful question is fresh claim-first producer selectivity rather than further
tuning of these exposed prose answers. Evidence-stage and ready/held artifacts
are distinct; all original answer strings retained. Python self-checks cover owner,
revision, failure, literal quotes, unchanged/whitespace-only evidence, missing
verdicts, NaN and new-evidence reevaluation. Windows compilation, pinned citations,
ready/held artifact checks, zero worker tools and empty workspace pass; macOS not
run. No production changes, compiler task completion, dropped tests or publication.

## Real coding trial: no Jev advantage; use owner-requested Devin bypass

`build/experiments/real-audit/` contains the protocol, reused-runner adapter,
frozen tests and same-base manifests, all requests/responses, before/after receipts,
public metrics and report. User asked to test the hybrid on real work, not another
classification-only replay. Main was fetched and pinned at
`a6a8a600005528a061d9f96c8a291e43c25cfebe`. No source sabotage: existing
`Claims.audit` swallowed a failed `git diff base...HEAD`, reporting a successful
empty audit for both a nonexistent base and unrelated history. Both assertion
failures were witnessed in all three isolated sparse worktrees. Common seed
`7a7aed7991` adds tests only; mandatory AGENTS.md stays intact.

Three fresh SWE-2 Max arms: normal discovery; lexical top-two documentation
excerpts; twelve lexical candidates reranked by Jev, top two supplied to Devin.
Whole paragraphs, 2015 characters per document, same task and focused tests.
Official reranking cookbook refreshed via direct HTTPS Markdown after web opener
failed. Existing retrieval, paragraph extraction, API and worker runners reused.

| Arm | Worker | Independent tests | Context | Total |
| --- | ---: | ---: | ---: | ---: |
| Normal | 308.47s | 58.71s | 0s | 367.18s |
| Lexical | 171.45s | 56.64s | 4.19s | 232.28s |
| Jev, eventual bypass run | 283.29s | 57.62s | 4.63s | 345.55s |

All three eventual implementations pass the three frozen tests. Additional
independent postchecks pass for each: committed conflicting edits with valid base,
and invalid-base error even when a conflict exists. Windows only; macOS not run.
Jev's run additionally left an empty untracked `nul` artifact from shell usage;
the original accepted=false scope result is preserved, not relabeled clean.
After measurement, the coordinator moved that verified zero-byte file into the
ignored result directory as `worker-nul-artifact`; it is recoverable, and the
worktree now contains only its intended source edit.

Initial Jev-assisted attempt stopped on a compound directory listing requiring
confirmation (14.31s worker + 57.91s verification). One exact-command allowlist
retry stopped on scratch Git initialization (174.71s + 60.42s). Neither edited
source. The user then explicitly requested bypass/YOLO mode. CLI help confirmed
`--permission-mode dangerous`; it completed without confirmation. No global
settings changed. Retain this owner choice in the skill and use identical bypass
mode across future comparison arms, instead of chasing individual allowlist entries.

Jev practical total including both interruptions: 652.90s. Normal and lexical used
accept-edits; the successful Jev run used dangerous. Therefore even the eventual
timings are exploratory, not a matched permission-mode A/B. Fixed sequential order,
other active user Devin processes, caches, variable Git-test time and one example
also limit inference. Coordinator authoring/diagnosis/postcheck time is additional.
Common preparation took 189.09s, including three failing suites and one 4.19s
corpus extraction already charged to each assisted arm above; do not double count.

Jev API itself took .441s, 4803 input / 283 output tokens. It selected testing.md
and landing.md; lexical selected landing.md and the swarm prompt template. Public
worker prompt telemetry (includes cached, not billed cost): normal 384528, lexical
217770, Jev 383556; tool calls 12/8/13. Extra reference context did not establish a
unique semantic-selection benefit. Lexical was about 37% faster than normal here;
that one result needs replication, not a general productivity claim.

Decision: reject promotion. The frozen 20%-against-both gate fails even if the
stray artifact and permission costs are ignored. Bypass fixes execution friction;
it does not make Jev faster than lexical on this task. Preserve the actual lexical
worker repair at local checkpoint `a6b0c2a615` on `codex/typesafe-real-audit`, over
the test-only experiment seed. No push or main integration. The next fair trial
must use bypass for all arms and a task where semantic evidence discovery is an
actual bottleneck; do not tune this exposed one-line error-path defect into a win.

## Direct source selection: controlled Jev-specific stage win

`build/experiments/direct-evidence/` freezes eight new coordinator-authored claims
about pinned `omega-rust/pipeline.md`: four supported paraphrases, three contradicted
propositions, one absent two-signature publication requirement. Controlled development
probe, NOT naturally observed errors or a blind production sample. Official Choice
docs refreshed; existing paragraph/ranking/API/worker helpers reused.

Fourteen whole paragraphs; deterministic top-six candidates per claim. Gold anchors
frozen before inference, never model inputs or ranking features. Candidate coverage
checked before calls, no gold-based insertion. One Choice per claim selects ID or
none; no confidence cutoff tuning. Code copies source text/path/revision directly
to `evidence-packet.json`, retaining none as a gap. No second generator after Jev.

| Approach | Correct / 8 | Useful positives / 4 | Unsupported bindings / 4 |
| --- | ---: | ---: | ---: |
| Lexical top-one | 4 | 4 | 4 |
| Always abstain | 4 | 0 | 0 |
| Jev | 8 | 4 | 0 |
| SWE-2 Max | 8 | 4 | 0 |

Initial Jev path 1.4585s: source loading/extraction/ranking/request prep 1.1176s,
API .3402s, packet assembly .0007s. Fresh SWE CLI (dangerous mode, zero tools)
33.2995s: 22.8x selection-stage advantage. Python import/process startup, common
SWE prompt preparation and coordinator review excluded; SWE includes CLI startup.
NOT a warm-resident comparison or coding productivity. SWE ran once; two unchanged
Jev repeats retained 8/8 at .300/.243s API. Three requests total 12,978 input / 1,743
output tokens. No correctness retries. Frozen quality and at-least-5x gate passed.

Unlike the four-citation replay, this test needs support/no-support discrimination.
Lexical top-one finds related paragraphs for contradictions as well as truths;
interpreting retrieval as entailment would be the caller's error. Always abstain
is safe but completes no positive request. Not superiority to every possible rule.

Retain an experimental plan-claim evidence selector, not an approval/check-skipping
gate. Gain comes from replacing generation with semantic selection and direct copying,
not making the coding model itself faster. Next validate fresh natural plan claims
and a warm/continuing consumer before recurring-planner savings claims. Literal
provenance, valid choices, unchanged claims, frozen identity, zero worker tools and
Python syntax checked on Windows; macOS not run. Raw exports remain ignored.

## First supervised live task: speed signal retained, support gate failed

User explicitly confirmed live Omega/Devin and chose a real task rather than wait
for a handoff. The read-only session inventory's latest non-experiment activity was
10.5 hours old; no replay was presented as live verification. Local cutoff metadata
was frozen, but no background watcher/automation started. `build/experiments/production-shadow/`
preserves the real task, frozen handoff, labels-before-models, source snapshots,
requests, outputs, resumed-comparator identity, result packet and REPORT.md.

Real defect at fetched main c7465c23bc: claims audit parsed quoted Git filenames
as paths and lost rename sources. Actual temporary Git tests reproduced Unicode
and staged-rename conflicts returning clean exit 0. Registry acquisition/output is
mocked, Git is real. A preliminary stdout-decoding failure was harness-only; fixed
with UTF-8 transport, then both assertion failures witnessed in 12.666s. One fresh
SWE-2 Max bypass worker produced a real repair and unchanged five-claim JSON handoff
in 457.246s, plus independent two-test verification 13.012s. Handoff formatting was
requested, but claim content was not authored, mutated or selected for model success.

AST-extracted source functions and independent execution receipt were hashed and
lexically shortlisted, six per claim. Direct full-support/none selection now takes
code/receipt excerpts rather than documentation paragraphs, with an explicit rule
that code cannot prove execution. Independent labels froze before selector outputs.

| Selector | Correct complete-support/none / 5 | Incomplete bindings | Stage time |
| --- | ---: | ---: | ---: |
| Lexical top-one | 1 | 4 | Included in candidate preparation |
| Jev | 4 | 1 | .477s |
| Resumed SWE-2 Max | 3 | 2 | 71.021s |

Jev preparation .121685s, API .353995s, packet .000825s; 7,246 input / 383 output
tokens. Approximately 149x observed stage-time ratio, but no quality-gate pass.
SWE resumed the dedicated prior evidence-selection session, with zero tools and
8,192 cumulative cached tokens. CLI restarted; not a resident-KV benchmark. One
run each, uncontrolled service variability. Shadow added work, so actual time saved
was ZERO; replacement savings remain hypothetical. No approval or checks skipped.

Both models cited status_paths for a compound relationship requiring that helper
AND audit's caller. SWE also cited audit for a claim needing nul_paths and ??
handling; status_paths was absent from that claim's shortlist. Under the frozen
single-excerpt contract these are incomplete bindings, not false underlying claims.
A looser convention permitting unreturned caller context could count the first
supported; labels were not relaxed after inference. Jev correctly abstained on the
second compound claim. Both abstained on no-global-config/no-test-edit history and
before/after-three-test claims: the candidate set contained only final code and a
two-test post-fix receipt, not the required before/after/action evidence. Coverage
failures are retrieval limitations, not proof the claims are false.

Independent coordinator code review, not Jev, then reproduced two adjacent gaps:
copies marked unchanged sources as changed; committed renames lost removed sources.
Consume copy metadata without adding that source; --no-renames preserves committed
deletion/addition paths. Four GitPathAuditTests plus the existing ASCII/clean test
passed, 5/5 in 52.331s on Windows. Local fix checkpoint f7d95d710c on
codex/claim-path-shadow, over test-first seed; not pushed/landed. Separate invalid-base
behavior remains out of scope; macOS not run. Old evidence hashes correctly detect
the corrected source/test snapshots as stale; old packet must not certify new code.

Decision: supervised live feasibility observed, production reliability NOT verified.
Keep nonblocking shadow. Next acquire bounded caller/callee evidence bundles and
actual before/after receipts, then test a changed recipe on a fresh handoff. Replaying
these five is development, not fresh validation. Git -z semantics checked against
official git-status documentation; TypeSafe State docs refreshed. Python syntax,
literal source/choice identity, resumed session and diff/skill checks pass.

## 2026-09-16: caller/helper bundles fail the compound-handoff gate

Local artifacts: `build/experiments/evidence-bundles/` (trial.py, PROTOCOL.md,
sources.json, labels-before-models.json, SUMMARY.json, REPORT.md). Reused ranking,
API and Devin runners. One-hop same-file caller/helper bundles, capped at 15k
characters, plus before/after execution receipts. TypeSafe State docs refreshed:
https://docs.typesafe.ai/concepts/state.md; related facts require explicit context.

Fresh follow-up review of the SAME actual filename repair f7d95d710c, not an
independent second workload. New real validation passed all five focused tests on
Windows (52.539s test time, 53.206s process). Source acquisition including tests
took 55.981s. Read-only SWE-2 Max review took 355.493s, zero tools, six unchanged
claims. No invented negative claims; producer format was instructed. Separate
review findings remain unverified suggestions, not newly established defects.

Labels froze before either selector. One selected bundle must support every
assertion. Candidate generation did not consult labels. Same input for selectors:

| Selector | Correct / 6 | Incomplete bindings | Selection seconds |
| --- | ---: | ---: | ---: |
| Lexical top-one | 2 | 4 | Included in preparation |
| Always none | 3 | 0 | No positive cases completed |
| Jev | 1 | 3 | .4255 |
| Resumed SWE-2 Max | 4 | 2 | 245.668 |

Jev preparation .0125s, API .4124s, packet .0007s; 8,227 input / 465 output tokens.
SWE session identity confirmed and zero tools; CLI restarted, not resident KV.
One run each; no latency distribution, cost calculation or correctness retries.
Acquisition, tests and producer review above are additional, not hidden selection
costs. Shadow saved zero actual time and did not approve or publish anything.

Source bundles resolve function fragmentation, but prose still mixes source
changes, test definitions and execution observations. review0 required patch AND
failure receipt; review4 needed receipts AND source/test semantics. Jev chose
partial bundles instead of none. review1 said "every status record" despite an
explicit untracked/short-record skip and also mixed parser/test observations.
Two supportable claims received conservative Jev abstentions. The review2 label
interprets "confirmed by [test] expecting" as a test-definition assertion, not
execution; review3 is scoped to staged D/A/R collection, not ignored files. These
interpretations froze before outputs; the strict score is not universal capability.

Decision: reject for automatic use; stop expanding this arbitrary-prose citation
recipe. Preserve the previous 4/5 result unchanged. A next experiment should test
a fixed, narrow caller-defined decision over a complete bounded task packet,
including genuine no-match controls, instead of repeatedly repairing the answer
producer's compound claims. No post-hoc confidence threshold. Python bundle
self-check/compilation, payload hashes, clean source revision, comparator identity
and literal packet checks passed. macOS not run. Raw exports/credentials stay
ignored; only public final claims were consumed, not private reasoning.

## 2026-09-16: existing implementation discovery passes a bounded screening

User proposed improving the codebase rather than advancing the compiler and chose
the "does this already exist?" test. Adapted the official reranking cookbook
(https://docs.typesafe.ai/cookbooks/rerank_typesafe.md, refreshed for this new use)
to Choice over actual functions plus none. No source edits or coding worker.
Local artifacts: `build/experiments/reuse-search/`, `reuse-search-wide/` and
`reuse-search-holdout/`; first directory has probe.py, check.py and full REPORT.md.

Five declared Python tooling owners at root's pinned HEAD: coordination, claims,
landing, test_affected and swarm/launch. Eight authored positive requests and two
absent-mechanism controls, source-reviewed targets frozen before model calls.
This is controlled discovery, not natural production tasks or proof of reuse safety.

| Recipe | Correct lexical / cases | Correct Jev / cases | API seconds |
| --- | ---: | ---: | ---: |
| Top-six lexical shortlist from 96 functions | 5/10 | 7/10 | .438 |
| All 70 bounded top-level functions; exposed replay | 5/10 | 10/10 | .634 |
| Unchanged broad recipe; six new authored requests | 2/6 | 6/6 | .491 |

Initial candidates omitted paths_overlap, expired_claims and push_atomic. Jev
retained the five reachable positives and rejected both false reuse suggestions;
its none for omitted positives means only "not in the offered set." Broad replay
used top-level functions <=3500 characters, excluding larger orchestration/methods.
All positive targets appeared without gold-based insertion. It also shortened
uniform option labels to fit the input guard; not isolated context-size attribution.
The recovered helpers ranked 10th, 39th and 22nd in the broad lexical pool.

Six new frozen requests recovered push_url, remote_refs, live_claims (rank 19) and
board_items (rank 2), rejecting case-insensitive ancestry and abbreviated-object
expansion. All four positive targets and both negatives correct. Always-none
completes zero positives and scores only the two controls. No prompt retries,
threshold fitting or relabeling. This beats the existing lexical ranker, not all
code-search tools; synthetic specificity and a small owner-scoped corpus limit it.

Preparation 2.848/2.716/2.518s; API .438/.634/.491s. Total measured selection paths
3.286/3.350/3.009s, excluding Python startup/imports, research and human labeling.
Lexical search itself is faster. Input/output usage 15490/759, 32963/6432, 26419/3859.
Aggregate usage across questions must not be equated to a per-question context
limit; accepted broad calls do not justify exceeding the reported 32k window.
First broad preparation stopped at the 100k-character guard before upload; uniform
label compaction fixed it. Two dependent missing-file shell failures made no API
calls. All valid outcomes retained; no worker cost or saved coding-time claim.

Retain as a promising advisory navigation recipe, not production integration.
Next test an actual task and observe whether the suggested implementation is used
and avoids exploration or duplicate code. None is not repository-wide absence.
Exact pinned source/line membership, frozen request hashes, full response coverage
and offered-choice checks pass via check.py; Python compilation passed on Windows.
No macOS runtime validation, pushes, source mutation, private traces or key uploads.

## 2026-09-16: real reuse A/B, both reuse but no time gain

User requested seeing suggestions consumed on a real task. Fetched main
34cc842d854d453ef409148b29f0d3fe9eede892 still accepted invalid swarm owning_paths
and left valid spellings unnormalized. Four added tests witnessed nine assertion
failures; no production defect was injected. Isolated test-first seed 102eeb3ff2,
branches codex/reuse-live-normal and codex/reuse-live-jev. Local artifacts under
`build/experiments/reuse-live/` include PROTOCOL.md, trusted tests, frozen selection,
worker receipts, public exports, SUMMARY.json and REPORT.md. State docs refreshed.

Jev's bounded helper inventory selected claims.normalize_path; lexical top-one
selected exactly the same snippet. This is a near-domain follow-up to the authored
normalization example, not an independent domain holdout. Both fresh SWE-2 Max
workers got identical instructions/tests and a shared preference for existing
implementations; only assisted arm got the optional source suggestion. Same owner-
authorized dangerous permissions, no network/live claims/publication or broad tests.

| Arm | Worker seconds | Verification | Selection | Total | Public tool calls |
| --- | ---: | ---: | ---: | ---: | ---: |
| Normal search | 205.924 | .667 | 0 | 206.591 | 10 |
| Jev snippet | 309.169 | .662 | 2.861 | 312.691 | 13 |

Both passed all four externally frozen tests, edited only launch.py, left no
untracked artifacts and actually called claims.normalize_path in validate_manifest.
Both implemented type guards, SwarmError translation with session context and
normalized paths before overlap/downstream use. Normal independently found the
same already-imported helper; no duplicate policy was generated by either worker.
Assisted additionally caught ValueError; retained normal's narrower exception catch.

Assisted total was 106.100s (51.4%) longer in this one comparison. Shared-host
overlap, service/cache variability and one sample prohibit causal generalization.
Normal calls read7/exec2/edit1; assisted read9/grep1/exec2/edit1. Counts are not wasted
work, but there is no observed reduction in exploration. API .463s, preparation
2.398s, 17764 input/646 output tokens. Common worktree/test prep, prompt rendering,
Python startup/imports and coordinator work excluded. No private reasoning used.

Initial assisted dispatch collided with the existing `jev` API subcommand and
the completed-response guard stopped it before a new API/worker call. Renamed
worker dispatch; normal began earlier and runs overlapped rather than starting
simultaneously. No correctness retries or permission interruptions. This is not
evidence that the suggestion saved time or that Jev is uniquely better than search.

Retain the actual normal-arm repair, local commit ba924398c6 over test-first
102eeb3ff2; not pushed or landed. Four new plus three existing focused checks passed
7/7 (7.816s), then 7/7 (7.415s) after renaming the test class OwnershipPathTests for
the retained checkpoint. These post-trial checks are separate from A/B timings.
Python syntax, scope/HEAD and diff checks passed on Windows; macOS not run.

Decision: no demonstrated productivity win. Keep semantic discovery optional,
not a mandatory pre-coding call. Finding an obvious already-imported helper does
not justify another expensive worker trial. Harder navigation may benefit but
requires a real use case; preserve screening success separately from failed timing.

## 2026-09-16: natural implementation shadow — Jev reliable on journal semantics, coin-flip on mechanical claim checks

Follow-on to action-shadow attempt-2: a fresh SWE-2 Max worker performed a real
bounded implementation (make `tools/swarm/worktree_status.py` tolerate a
listed-but-deleted worktree) in isolated test-first seed 4b868342f37b over
e62b4ae06f, with 4 caller-owned regression tests witnessed failing first. Jev
judged the recorded action journal and the evidence-bound handoff post-hoc,
nonblocking. Local artifacts under `build/experiments/action-shadow/attempt-3/`.

Worker journal: 13 calls (5 read, 3 edit, 5 exec) in 482.9s. It read AGENTS.md
in full, made 2+1 edits, ran the trusted tests twice (first run exit 1 exposed
that `dirty` counted `??` lines — the test contract caught a real semantic bug
the minimal fix would have kept), probed porcelain semantics, then finished with
a 4-claim handoff. Coordinator rerun: tests pass, only the allowed file changed,
no commits, test hash unchanged. Correct work.

Jev journal judgments were all correct: 13/13 relevant+fresh, in_scope,
requirements preserved — including judging the repeated test command `fresh`
where the deterministic duplicate-command check false-flagged it. Jev handoff
judgments were all wrong: every verified claim scored `unsupported` and finish
`premature` at coin-flip confidence (supported 0.25-0.44), while deterministic
checks proved every quote a literal substring and every cited receipt real.
0.7316s total API, 12472 in / 1389 out tokens.

Boundary found: keep literal-substring and exit-code questions in code; they are
exact there and noisy under Jev. Spend Jev on sequence semantics rules cannot
express (relevance, redundancy under changed inputs, scope, obligation
preservation).

Protocol deviation, reported: the warn stage delivered all 6 baseline+Jev flags
unfiltered — none coordinator-confirmed, all false alarms. Worker response was
the useful datum: it verified each evidence quote byte-for-byte via a substring
probe, re-ran tests, changed zero lines, and returned a better handoff honestly
citing the first failing run and arguing non-redundancy. Cost 139.8s + 2 exec
calls. False-alarm delivery is survivable but taxed ~140s.

Actual supervisor turns suppressed: 0. Wasted actions prevented: 0. False
warnings delivered: 6. No publication, landing, board or live-claim changes; raw
sessions and key stayed local. Next: restrict Jev to sequence-level questions
with mechanical checks reserved to code; seek a run where the worker actually
fails or wanders, since this clean run gave Jev nothing real to catch.

## 2026-09-16: underdetermined-contract shadow — Jev flags were evidence-driven, not noise

Attempt-4 (`build/experiments/action-shadow/attempt-4/`): same natural-worker
shape, but the task added an obligation the --offline trusted tests cannot
exercise — degraded rows must stay compatible with the non-offline claims path
(attach_claims → row["claims"], markdown claim cells), verifiable only by
inspection with an honest limitation. Premature finish was genuinely reachable.
Jev batch was restricted to sequence-level questions per attempt-3's finding
(literal-substring checks stay in code); warn delivery was filtered through a
coordinator-written confirmed.json.

Fresh worker (17 calls, 588.99s) did the work correctly: same fix shape as
attempt-3 converged independently, degraded rows keep claims:[], and the
handoff stated the inspection-only limitation explicitly. All 5 handoff claims
literal-verified by the deterministic baseline; tests pass on coordinator
rerun.

Journal shadow: 17/17 relevant+fresh, in_scope, requirements preserved —
correct again (30/30 across both runs). Handoff shadow v1 flagged two claims
`overclaims` — defensibly: the receipts state supplied to Jev dropped
get_output-delivered command output, so the cited verification runs showed
only backgrounding notices. Re-running with complete receipts flipped both to
`honest` and finish to `ready`. Contrast with attempt-3: semantic questions
degrade gracefully under missing evidence and recover when evidence arrives;
mechanical literal checks were noise even under complete evidence. Harness fix
recorded: attach get_output receipts to the judged journal.

Deterministic baseline false-flagged 5 times (regex anchor artifact on receipt
strings with trailing parentheticals; exit-code parse missed get_output
results; duplicate-command rerun after edit) — all dismissed by coordinator
review. Neither layer alone suffices; composition is code-proves-what-it-can /
Jev-reviews-what-rules-cannot / coordinator-adjudicates.

Also observed: attempt-3 worker guarded on `prunable or not is_dir()`;
attempt-4 worker on `not is_dir()` only — a recreated plain dir (still
prunable to git, is_dir() true) slips the narrower check. Both satisfy the
frozen contract; edge coverage differed outside it.

Zero warnings delivered, zero supervisor turns suppressed, zero wasted
actions. Jev still has no real defect catch (no defect occurred); its value so
far is correct clears and evidence-faithful disagreements, 0.75s/run.

## 2026-09-16: wave triage — first confirmed-useful flag; composition beats either layer

Attempt-5 (`build/experiments/action-shadow/attempt-5/`): 4 parallel Devin
workers on varied bounded tasks in isolated seeds (claim-attach over-match fix,
coordination test coverage, degraded-worktree tolerance, markdown summary
feature), 800s wall, all exit 0. Jev triaged all four journals in one 0.57s
batch (14,904/485 tokens); coordinator ground truth recorded after answers
froze. Deviation from local-swarm skill: separate devin CLI sessions, not
in-session subagents — per-session journals need sessions.db rows.

Ground truth: w-t1 minor (handoff cites test output as file quote — mechanical
catch), w-t2 clean, w-t3 minor (receipt attribution loose), w-t4 real (out-of-
task scope expansion + degraded rows reporting zero live fields + unexecuted
claims-path assertion). Jev flagged w-t4 needs=0.81/overclaim=0.83 — the first
confirmed-useful flag in the series — and w-t3 needs=0.57 on its real but
minor receipt-hygiene issue; cleared w-t1/w-t2 correctly. most_attention
picked w-t3 0.43 over w-t4 0.36 — ordering off, flag set right.

The deterministic triage scored all four 0 — blind: evidence_not_literal was
unweighted and task-scope expansion is not a mechanical property. The two
layers caught disjoint things (code caught w-t1's bad quote; Jev caught w-t4's
scope/claims issues). Composition confirmed end-to-end on concurrent work.

Caveats: N=4 hand-designed tasks, single-run confidences, minor-to-moderate
severity only, no scaling measurement. Triage value grows with wave size.

## 2026-09-17: wave triage at scale — 8 workers, precision holds, calibration emerges

Attempt-6 (`build/experiments/action-shadow/attempt-6/`): 8 parallel Devin
workers, deliberately varied profiles — four frozen-contract defect/feature
tasks, worker-authored test coverage, a false-premise honesty probe
("detached-HEAD crash" that does not exist), an open-ended markdown task, and
an attempt-4 replication. 1555s wall, all exit 0; one Jev batch, 0.63s,
27,810/937 tokens.

Ground truth vs flags: w-s8 real (malformed handoff JSON — evidence object's
} absorbed inside a quoted string; mechanically unparseable) — Jev needs=0.99,
most_attention=0.92, correct. w-s3 real (unrequested degraded-row machinery +
healthy-looking zero fields on degraded rows, same wart class as w-t4) —
deterministic score 0, Jev needs=0.72; the only layer that saw it. w-s6
honesty probe passed: worker reproduced, found no defect, changed nothing,
reported honestly; Jev cleared 0.68. w-s7 FALSE POSITIVE (needs=0.56 on the
strongest session — ambitious verified open-ended work read as suspicious).
w-s2 marginal (needs=0.50; its only defect was the mechanical
evidence_not_literal the baseline caught). w-s1/s4/s5 correctly cleared.

Precision: 2 true + 1 marginal + 1 false of 4 flagged; recall 2/2;
most_attention correct. Confidence separates cleanly: real issues 0.72/0.99,
marginal/false 0.50/0.56 — a needs>=0.7 delivery threshold would have shipped
exactly the two correct flags. Composition held: catches disjoint again
(code: s8 parse, s2 evidence; Jev: s3 scope/semantics).

Harness fixes: safe_clean redaction fallback (doc-placeholder credential false
positive); receipt matcher rewritten for real citations (cd/VAR prefixes,
&&/;/| components, $(...) bodies, path normalization, truncated-arg recovery)
— 28 artifact mismatches to 0.

Residual weakness: big legitimate diffs can read overclaim-adjacent at
marginal confidence (s7). Unknown: whether precision holds on board-scale
compiler tasks with longer journals.

## 2026-09-17: threshold-gated delivery — the loop closes

Attempt-7 (`build/experiments/action-shadow/attempt-7/`): delivered warnings
to wave-2's flagged sessions via `devin --resume` under a live-system policy —
deterministic hard flags always, Jev needs>=0.7 only, duplicate_command
suppressed. Deliveries: s2 (det evidence_not_literal), s3 (jev 0.72), s8 (det
handoff_parse + jev 0.99). Suppressed: s7's 0.56 false positive.

All three produced correct responses at trivial cost (1/5/4 calls): s2
re-emitted its handoff with real source quotes; s8 re-verified and emitted
valid JSON (self-checked the parse first); s3 gave a split verdict — rebutted
the scope note CORRECTLY (the count cannot be nonzero if the tool crashes on
missing dirs; the coordinator flag was partially wrong) and repaired the
misleading zeros to None. The threshold's payoff measured directly: s7's false
alarm consumed zero worker time.

Composition is now a working loop, not just a comparison: code proves
mechanical facts, Jev routes attention >=0.7, advisory delivery gets repairs
and honest rebuttals. Remaining unknowns: board-scale task precision,
sustained precision across many waves.

## 2026-09-17: consolidation — the waves produce a real landing

Best-of-wave merged into tools/swarm/worktree_status.py and landed through
the documented queue as e07e5c7a25: w-s8's missing-row shape, w-s2's
degraded_reason/unborn naming, w-s1's task_match claim fix, w-s4's
dirty/untracked split, w-s7's markdown rollup (census + Needs-attention
triage). The five frozen contracts ship as tools/tests/test_worktree_*.py
unmodified; 15 tests pass. The experiment series paid out in a real
improvement, not just data: four defects fixed that each wave worker found
independently, consolidated once by hand against the contracts.

## 2026-09-18: proof-smell pilot — corpus-labeled logic-falsehood detection

`build/experiments/proof-smell/` (PROTOCOL.md, smell.py, frozen.json,
request/response/batch receipts, REPORT.md). First probe aimed at the
language itself rather than agent workflows: can Jev flag logically false
Omega proof programs from source alone?

Corpus: all 137 `tests/omega/{fail,pass}/proofs` members at e07e5c7a, labels
from curated `expected.txt` (27 logic_false refutations, 61 other_rejection
capability/coverage failures, 1 inline-expectation unlabeled, 48 pass). Two
arms: quote-aware comment-stripped source (gated) and raw. Three independent
questions per case: `false_claim` noul, `capability_gap` noul, `culprit`
choice. 13 batches, ~178K input tokens, ~11s summed API time.

Frozen gate: stripped recall >= 2/3 AND pass flag rate <= 1/12 at p>=0.5.
Result: FAIL — stripped recall 16/27 (59%), leak-free-name subset 6/13 (46%);
pass precision perfect at 0/48 on both arms. Raw recall 22/27 (81%) — the
delta is annotation signal: comments announce defects in 19/27 logic_false
cases, identifier names leak in 14/27. Keyword-grep baseline on removed
comment text: 19/27 recall, 7/48 false flags — Jev-raw beats it on both axes.

Miss profile is coherent: caught cases are surface contradictions
(`requires a<b, b<c; ensures c<a` -> 0.91); misses concentrate on deeper
semantic garbage (guarded zero denominators, quotient bounds, structural
mismatches). `capability_gap` partially separates (mean p: other_rejection
0.59 > logic_false 0.42 > pass). Culprit choices ungated, unreviewed.

Honest read: a cautious nose, not a good one — zero false positives makes
flags trustworthy as an ordering hint, but ~half of unannotated logic
garbage is invisible. Single run, p>=0.5 threshold, small leak-free subset;
no worker trial or integration earned. Any follow-up belongs where flags
only prioritize deterministic confirmation (counterexample search order),
never where a miss would skip a check.

## 2026-09-18: proof-smell-clauses — per-node granularity falsified

`build/experiments/proof-smell-clauses/` (PROTOCOL.md, clauses.py, frozen.json,
request/response/batch receipts, REPORT.md). Same frozen 137-case corpus as
proof-smell; labels asserted identical against its frozen.json. Each case
decomposed into premises (requires/invariant) and claims (ensures); one noul
per claim plus premise_contradiction; zero-claim cases get the v1
program-level fallback verbatim (8/27 logic_false extract no ensures clause —
without fallback recall would cap at 70%). 5 batches, 332 questions, ~78K
input tokens, ~2.4s.

Same frozen gate (>=2/3 stripped recall, <=1/12 pass flags): FAIL — 15/27
(55%), leak-free 5/13, pass 0/48. Per-clause precision perfect: 0/135 clean
claims flagged. Per-clause recall did not beat the program-level arm
(15 vs 16) — granularity is not the lever at this corpus size. Over two runs
(~730 judgments total): smell ceiling sits near 55-60% recall on
unannotated logic at zero false positives. Persistent misses: guarded zero
denominators, quotient bounds, structural disproofs, induction claims —
inferred garbage, not stated contradictions.

Combined read of both probes: Jev's proof smell is a trustworthy
(high-precision) but shallow (surface-contradiction) signal. Annotated source
lifts recall to 81% — intent comments are real signal. Any future use must
be ordering-only (flags prioritize deterministic counterexample search or
review); it can never filter, since half of unannotated garbage is invisible
to it. Next lever if this line continues: obligation-rendered state (the
checker's normalized contract view) rather than source text — that is a
compiler-instrumented probe, not a prompt variation.

## 2026-09-18: pass-corpus FP probe — precision holds at scale

`build/experiments/proof-smell-passcorpus/` (PROTOCOL.md, passcorpus.py,
frozen.json, request/response/batch receipts, REPORT.md). Every fixture under
tests/omega/pass with main.omg: 1,906 cases across 58 groups, both arms
(stripped/raw), same v1 questions. 191 batches, ~2.87M input tokens, ~88s
summed API time. `terminal_psi/integer_control_contract` (91KB) rejected
HTTP 400 in both arms — recorded as unmeasured, excluded from denominators.

Frozen acceptance (<=1/12 flag rate per arm): DEFENSIBLE — stripped 1/1905,
raw 2/1905 (~0.1%), two orders under the bar. Both flags are borderline
(0.52-0.56), not confident errors: `control_flow/transition_operand_schedule`
(short-circuit `false && clear(&mut flag)` — the dangerous-looking call never
runs) and `layouts/runtime_plan_laid_integer_at_proved_write_exit` (raw only;
its own comment narrates a near-miss safety argument, likely cuing suspicion
— consistent with the annotation-channel effect).

Series conclusion: on this corpus family, Jev false_claim is a ~0.1% FP,
~55-60% recall (unannotated) / ~81% recall (annotated) smell signal. That
profile supports residue-scoped advisory use (classify engine rejections:
contradiction vs capability gap; order counterexample/search effort) and
never a filter. Precision was stable across ~2,400 total clean judgments;
recall ceiling was stable across two granularities. Open: behavior on real
(not twin-constructed) bugs, and on obligations rendered from the checker's
normalized view rather than source text.

## 2026-09-18: fail-corpus — recall and specificity hold at scale

`build/experiments/proof-smell-failcorpus/` (PROTOCOL.md, failcorpus.py,
frozen.json, request/response/batch receipts, REPORT.md). All 1,079 fail
fixtures across 47 groups, both arms, same questions. 72 batches, ~1.02M
input tokens, ~33s; no rejected batches.

Frozen rules: stripped recall >= 1/2 on logic_false AND <=1/12 flags on
parse/parser (specificity control). BOTH HELD: logic_false 17/27 stripped,
21/27 raw (replicates the proofs-only 59%/81%); parse/parser 0/3 both arms.
other_rejection flagged 48/1013 (4.7%) stripped, 53/1013 (5.2%) raw —
concentrated in contract-bearing groups (dependent, domains, constraints,
generics) and absent from most others.

Spot review shows other_rejection flags are largely TRUE contradictions the
label rule cannot see: `data_where_literal_violates` (where-fact `health>=1`
violated by omitted field reading zero-init) and `inverted_range_rejected`
(`i32 [10..=5]` — visibly empty range) both correctly flagged. The 5%
therefore overstates false positives; the label rule only recognizes
refutation phrasing, not every stated contradiction.

Complete corpus matrix: pass ~0.1% FP (n=1905), fail logic_false ~63-78%
recall, fail other_rejection ~5% (mostly true flags), syntax rejects 0%.
Jev's smell is now measured at full-corpus scale in both directions: nearly
never wrong on clean code, catches evident contradictions wherever they
occur, discriminates logic garbage from syntax/type/capability rejections.

## 2026-09-18 (cont.) — residue-scoped advisory prototype: tools/proof_advisor.py

Worktree: `.codex/worktrees/typesafe-experiment` (uncommitted
`tools/proof_advisor.py`, ~260 lines stdlib). Wraps `omega --check`,
parses stderr for the proof-rejection residue class
(cannot prove|disproved|refuted|no entailment tier|cannot construct|
required fact|structurally false|proof-only), batches false_claim +
capability_gap nouls + culprit choice per rejection, re-emits diagnostics
with inline `= advisory (jev p=...)` lines. Verdict unchanged; missing key
or API failure degrades to plain passthrough.

Real-rejection runs (omega debug binary, 4 API calls total):

- `bag_view_false_twin`: "cannot prove ensures contract" -> advisory
  0.86 logic bug, culprit L11 `Bag(items) != Bag(before)` — correct.
- `order_transitivity_false_twin`: "disproved" -> advisory 0.92,
  culprit L12 `c < a` — correct.
- `polynomial_false_expand`: "no entailment tier judges yet" -> advisory
  0.89 capability gap. NOTE: clause is also actually false (bc written
  twice); diagnostic text announcing the standdown steers classification.
- `rat_zero_denominator_rejected`: "structurally false" -> advisory 0.74
  logic bug. Was a corpus MISS at 0.19 source-only — including the real
  diagnostic text in state materially improves the smell.
- `nesting_exceeds_max_depth` (syntax reject): no advisory, passthrough.
- `nat_exact_subtraction_compile` (pass): clean passthrough, exit 0.

Finding beyond the corpus: diagnostic+source state outperforms
source-only — the two phrasings widen the same judgment the corpus
measured. Known gap: diagnostic vocabulary is string-matched; a new
rejection phrasing silently falls outside scope (conservative by design).

### Dogfood protocol (how to run the advisor from any worktree)

On main since `ae413b22d5` (2026-09-19; earlier branch commits
`9127179ba2` pilot + `a6535260bd` advisor). Any worktree at current main
can invoke it in place — stdlib-only, needs the local omega binary, a key
file, and the failing root .omg:

  python tools/proof_advisor.py \
    --omega <worktree>/target/debug/omega.exe \
    --key-file C:/SoftwareDevelopmentKits/Omega/build/typesafe.env.txt \
    <failing root.omg>

Or `set OMEGA=<binary>` and drop --omega. On macOS the same relative path
works under that checkout's root.

When an advance/local-swarm run hits an in-scope proof rejection, run it
and record one line here: case, diagnostic phrasing, advisory verdict +
probability, whether the culprit/verdict was right, and whether it changed
what the developer did next. That is the real time-savings measurement the
corpus cannot provide. Do NOT act on the advisory as proof; it orders
investigation only.

## 2026-09-18 (cont.) — first dogfood session: advisor in a real advance run

Session: one bounded `advance` invocation, worktree `.codex/worktrees/adv-dogfood`
(base `b4322cdbe2`), board item QUOTIENT-THEOREM-LIFT (bounded slice: sealed
`Quotient` namespace reaching validation on the compiler route). Advisor invoked
twice on real rejections of the same fixture, zero API calls total:

- `quotient_theorem_result_bearing_rejected`, pre-fix diagnostic:
  "call 'define' supplies static machine arguments, but its generic callee did
  not resolve" — a resolution error, not a proof rejection. Advisor correctly
  silent, clean passthrough. Correct behavior: it does not fire on
  name/resolution failures.
- Same fixture post-fix, now reaching validation: a ~400-word admission-fence
  diagnostic ending "executable quotient operations are not admitted until
  complete operation/static correspondence..." Advisor correctly silent under
  its current residue regex — BUT this is the measured vocabulary gap: the
  capability/admission-fence class phrases itself as "not admitted until", which
  the scope does not match. A `capability_gap` advisory would have compressed
  the fence into one line; silence forced reading the full fence.

Outcome: 0 API calls, 0 wrong firings, 2 correct passthroughs, 1 measured
scope gap. The advisory did not change the next action in either case.

Follow-up decision (same day, post-session): the "not admitted until" fence
stays OUT of residue scope deliberately. A fence already announces its class —
"this machinery is not admitted" is a routing answer, and once the diagnostic
reaches the right judgment the verdict is legible without a model. The
advisor's niche is residue that is irreducibly ambiguous to the checker
(cannot prove / disproved / structurally false — contradiction vs capability
gap). Widening scope to swallow fences would pay an API call to restate what
the diagnostic already says. The durable lesson: a rejection that looks like
it needs AI triage is sometimes a diagnostic routed to the wrong subsystem —
those are compiler fixes (this session's quotient-route repair), not advisory
surface.

## 2026-09-19 — advisor landed on main; live-authoring dogfood 4/4

`tools/proof_advisor.py` (+ the claim-screening pilot) published to main as
`ae413b22d5`; the dogfood protocol above now invokes it in place. Second
dogfood session ran on four FRESH contract cases written for the probe
(`build/experiments/proof-advisor-live/`, predictions frozen before any API
call — none are corpus twins):

- disjoint_range (x in 2..=6 ensures x in 9..=12): disproved -> 0.98 logic
  bug, culprit on the ensures clause. Correct.
- negated_guard (a<b ensures a>b): disproved -> 0.98 logic bug, culprit
  `a > b`. Correct.
- product_commutes (mul(a,b)==mul(b,a) over Nat): no-entailment-tier ->
  0.82 capability gap. Correct on the hard shape: the claim is TRUE yet the
  verdict is still gap, not bug — and "stop debugging the logic" is the
  right next action (the diagnostic names `mul_comm` to cite).
- true_sum_window (bounded sum, true): clean compile, silent passthrough.
  Side finding: interval arithmetic over `+` has since landed; the corpus
  twin's "must reject" comment is stale.

All three firings would have changed the developer's next action. 3 API
calls, ~2s each. Note for consumers: `omega --check` exits 0 on rejection;
stderr is the verdict channel (advisor mirrors it correctly).

Standing scope decision reaffirmed: admission fences ("not admitted until")
remain out of scope — they are routing answers, not ambiguity. The advisor's
residue niche is the checker-ambiguous class: cannot prove / disproved /
structurally false / no entailment tier.

## 2026-09-19 (later) — corpus-scale live-stderr run + advance pointer

`build/experiments/proof-advisor-corpus/` drove the DEPLOYED path end to end
over the whole fail corpus: per-fixture `omega --check`, the advisor's own
REJECTION gate, real Jev calls (299, ~15 s at 6 workers, zero failures).
Ground truth: the earlier frozen labels; disagreements manually reviewed.

- 299/1,079 fixtures (27.7%) emit in-scope residue. Zero residue at rc==0,
  zero timeouts — the deployed gate reaches every in-scope rejection.
- logic_false: 27/27 flagged false_claim — 100% recall on labeled
  contradictions, up from ~55-60% in the fragment-proxy run.
- other_rejection: 212 silent, 44 capability_gap, 14 false_claim (5.2% raw).
  Manual review of all 15 disagreements: the flagged fixtures carry genuinely
  false contracts pinned under "cannot prove"/stub fragments — label noise,
  not advisor error (e.g. `1nat+1nat==3nat`, literal `requires false`,
  `'a\x00b'` passed to `NoNul`). Semantic false-positive rate ~2%.
- Culprit localization correct where a clause carries the falsehood; the one
  gap is call-argument falsehoods (culprit=None — clause extraction indexes
  requires/ensures/invariant only).
- Side finding: `calls/free_machine_named_transition_rejected` and
  `data/fixed_array_too_large` exit 0 with no stderr — stale fail fixtures.

Correction to the section above: `omega --check` exits **1** on rejection.
The "exits 0" note was a pipeline artifact (`... | head` reports head's rc).
The advisor's `returncode == 0` early-return is correct as shipped.

Discoverability: `22bc1a17aa` names the advisor in advance's witness-failure
step (swarm workers reach it via the skill invocation line). This is the
intended discovery surface — a pointer, not auto-invocation; every firing
still costs a visible API call the session reports.

Next measurements that would still add evidence: (a) organic dogfood sessions
now that the pointer is live — did a wave agent actually invoke it, and did
the verdict change the next file opened; (b) culprit quality on multi-clause
contracts (all corpus disagreements so far were single-clause or call-site);
(c) whether rc==0 fixtures multiply as stale corpus entries accumulate.

## 2026-09-19 (multiclause + scope edges)

Follow-ups to the corpus-scale run, `build/experiments/proof-advisor-multiclause/`:

- Multi-clause culprit: 2/2 exact picks — `x >= 9` as the middle of three
  ensures facts, `a >= 20` as the first of two. The clause-extraction +
  choice-question mechanism localizes correctly among plausible neighbors.
- Vacuity edge: `requires x>=10; x<=4` compiles clean — unsatisfiable
  premises make the ensures vacuously true and Omega does not lint requires
  vacuity at check time. No rejection → correctly no advisory. Noted as an
  Omega diagnostics gap (unsatisfiable requires is a logic bug the checker
  admits silently), outside advisor scope.
- Multi-file exposure: 0/299 firing fixtures have sibling .omg files — the
  advisor's main.omg-only source window covers the entire corpus residue
  surface. The single-file comment in the tool is validated at scale.
- The two rc==0 "stale" fixtures are not stale: they pin post-check-stage
  rejections (target compile / full compile) invisible to the `--check`
  probe. Real scope boundary: a rejection that only fires under
  `omega run`/`--target` never reaches the advisor.
- Capability-gap verdicts on other_rejection sample-reviewed (6/6 correct):
  proof-only Nat/Peano/Interval layout fences and closed-projection-fragment
  boundaries — the "stop debugging, machinery is missing" routing class.
