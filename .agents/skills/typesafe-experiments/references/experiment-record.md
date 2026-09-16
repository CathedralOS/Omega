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
