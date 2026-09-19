# Bootstrap cost investigation

Temporary evidence for P1 encoder feasibility and bounded Delta simplification
on [the bootstrap board](../../TASKS_BOOTSTRAP.md). This is engineering evidence,
not an owner ruling, admitted artifact, or complete-chain proof. Both named
comparisons are now decided; each section records its decision above the
retained measurements. The Delta section persists because its numbers back the
retention records at the mechanism owners and an inbound
`#delta-simplification` link. The P1 ledger persists as the live feasibility
account for the paused consolidated candidate: delete it when the complete
definition package and integrated recipe replace its extrapolations with actual
full-subject costs, or when an owner ruling retires the route. Delete this
draft when both sections are absorbed or superseded.
The [minimization contract](../../bootstrap/MINIMIZATION.md) owns the review rules.

## Delta simplification

Question: can the existing normalization or name-construction machinery be
simplified without losing selected conformance? The current customer alone is
not the full admitted source envelope.

**Decision — resolved: retain all three mechanisms; no simplification is
taken.** Each measured mechanism has a live conformance consumer, and removing
any of them narrows admitted source or removes a consumer's evidence rather
than simplifying the chain:

- Body-height normalization stays. Delta admits expression depth 1,024 while
  the selected Gamma evaluator admits at most 255 nested expression lists per
  function body, and lowering can exceed the evaluator bound inside that
  envelope; the [normalization gate](../../tests/delta/normalization/run.sh)
  exercises the obligation at depth 1,024 and width 2,048. On this customer the
  pass is already minimal: `normalization_visit` compares each body root's
  recorded height against the budget and returns fitting nodes unchanged, so
  the 724 fitting definitions cost one comparison and one definition rewrap
  each, generate no helpers, and leave the emitted receipt byte-identical.
- Both node summaries stay. The cached byte extent feeds emission's count-only
  preflight, which produces the exact 16,777,212-byte admission and the DCOUT
  resource-12 refusal carrying the complete count; recorded height feeds
  normalization's per-node budget decision and the lowering-plan gate's
  pre-normalization expectations. Removing either summary removes its
  consumer's evidence, not only private metadata.
- The shared-prefix name cursor stays without expansion, as its
  [owner README](../../bootstrap/3_delta/implementation/checking/names/README.md)
  already records; the measured removal cost below bought no conformance or
  failure-behavior improvement.

The required exact-emission comparison is discharged on this customer: with no
over-height body, normalization adds no helpers and the published receipt stays
byte-identical. For over-height admitted sources removal cannot preserve
emission at all — the unnormalized body exceeds the evaluator's census bound —
so no further deletion comparison remains to run.

Measurements used the 612,994-byte Epsilon-plus-entry subject with SHA-256
`251a97366c26c4356e4c573e353f8998d3c7d26985bc6052deafc4fe28a43f94`.
It includes one separating LF; do not substitute an unseparated closure.
At `59e73ffef81b550c0eac4946b0e40970dc90942b`, normalization left 724
definitions at maximum Gamma height 69 and generated no helpers. This does
not establish that normalization is unnecessary for Delta's deeper admitted
sources. The [normalization gate](../../tests/delta/normalization/run.sh) and
[Gamma containment argument](../../bootstrap/2_gamma/EVALUATOR_PROFILE.md#containment-argument)
own that separate obligation. Compare exact complete emission from the
original plan before proposing removal; do not remove height/extent metadata
also consumed by emission or narrow the language to fit this customer.

At `fe0b48924aebc7bd4818f1a75d870f98ba17e8b4`, disabling only shared-prefix
seek reuse changed cumulative allocation from 2,242,373 to 2,517,725 pairs
on that subject. Both receipts matched the selected evaluator's exact
711,597-byte output. The difference is 275,352 pairs at 40 bytes each;
this is cumulative allocation, not live memory or host RSS. Both fit the
selected provision. Single-run timings do not establish a throughput gain.
The decision above retains the
[existing cursor](../../bootstrap/3_delta/implementation/checking/names/README.md)
without expansion; neither customer fit nor
this isolated comparison closes full parameter, constructor, or match limits.
Both investigations ran on macOS arm64; no Windows result is claimed.

## P1 complete-route feasibility

The next unit and scope pause belong to the
[complete encoder candidate](../../bootstrap/proofs/beta_encoding/ENCODER_CANDIDATE.md).
Its customer is the exact selected evaluator source/tape in
[the Gamma profile](../../bootstrap/2_gamma/EVALUATOR_PROFILE.md), not another
literal instruction proof. The
[complete encoding subject](../../bootstrap/proofs/beta_encoding/ACCEPTANCE.md)
requires complete error-valued Beta definitions, independently constructed
source/tape/limits/theory ownership, and a source-owned untrusted producer.
An encoding certificate would still leave evaluator correctness separately
trusted. Source-shape, capacity, and component checks do not close P1.

**Decision — the named comparison is resolved.** The consolidated candidate's
state architecture is the required shape and supersedes every earlier
baseline; the following are retired as evaluation units, not merely deprioritized:

- the history-bearing scan state carrying accumulated output and completed
  tokens (~10-20GB-class ground table, physically impossible);
- the repeated output-successor counter recipe as a route baseline
  (4,611,614 unshared / 2,122,796 shared work for that one component);
- the normalized linked-list recipe (required-record floor 8,553,056 bytes,
  already above the 8,388,608-byte request before theory and states); and
- per-byte Word-successor counting as a source counter (25,285,216 request
  bytes diagnostic; not a baseline).

The candidate remains the proposed evaluation unit, with two priced design
levers: adopt the permitted concat-collapse flatten (~10 times on that
component) and price count-out-of-state (~11.3 times on leaf-state
distinctness) before fixing the state record. No measured or derivable recipe
family reaches acceptance under the selected provisions, so the strategy pause
stands. Residual routes — recipe restructuring toward the ~675,017-work
physical ceiling, checked closed-lemma composition across bounded requests
(owner escalation), or more native backing (owner decision) — are not
selectable from this review: restructuring needs the complete definition
package owed by the P1 item, and the other two are owner rulings.

The following retained discriminators concern the 46,484-byte source and
8,355-byte tape. They are not estimates of the plan's complete token-at-a-time
encoder. Recorded executions ran on macOS arm64; no Windows execution or
allocation peak was measured.

| Discriminator | Evidence and limitation |
| --- | --- |
| Per-byte Word-successor counting | At `e128a74dbba34b651ac7c36e782e66ec4a0c3eeb`, the full constructed recipe cost 25,285,216 request bytes and 11,204,359 work after sharing. It was not executed full-size and omits encoding. Do not use it as the baseline source counter. |
| Exact source capacity | At `12090b9803e8c91732f94f18f8582874068cd2ca`, shape-mediated and direct counting checked at 547,817 and 641,773 work respectively. Their requests were 1,718,316 and 1,999,560 bytes. Neither proves ASCII or scanning. |
| Normalized identity state fold | At `06b76d3eb68963c4e4c8ed8903b686ebc921fc34`, constructed cost was 681,724 work; execution returned Incomplete at 655,361. This no-op byte step is not an encoder or a universal lower bound. |
| Connected literal components | At `802320b216f7dfa42cbe87ba44d3b1e47d3bd2e6`, history-bearing versus factored local transition/collection checked at 24,489 versus 27,673 work under the same theory. Local reuse alone did not make factoring cheaper. Literal opcodes left general dispatch, output limits, and full finalization unproved. |
| Normalized linked-list recipe | The source-specific required-record floor is `184 * 46484 = 8,553,056` bytes, already above the checker's 8-MiB request before theory and states. This rules out that unchanged recipe, not every certificate. |
| Stateful binary-tree traversal | At `efe1898f290ac9b8220139f6331c7af74f57218f`, 42,081 Join/incoming-state keys and 7,096 right-source references give `55*42081 + 4*7096 = 2,342,839` recipe work. Adding the separate shape-capacity work gives 2,890,656, still excluding byte transitions, collection, ASCII, encoding, output, and finalization. It is not a checked combined certificate. |

The last trace retained completed tokens; the proposed streaming encoder does
not. Its actual states, fragment results, projection/composition rows, and
sharing must be counted afresh. Adding unrelated partial totals or scaling a
small literal cannot establish full fit. Temporary host-authored recipes and
instrumented evaluators are diagnostic witnesses, not semantic stages or
production authority. Do not recreate local scratch paths as required tooling.

### Integrated ledger for the consolidated candidate

Derived against the current selected subject (47,748-byte source, 8,575-byte
tape; the discriminators above used the earlier 46,484/8,355 subject). The
subject now has 3,751 tokens in 595 distinct values, 3,493 emitting tokens,
218 assertions, 19,495 comment bytes, 15,305 separator bytes, 76 distinct
bytes, a maximum token of 18 bytes, and 994 distinct pending-token prefixes.
A mechanical binary source partition has exactly 47,747 Joins, of which
12,121-13,549 subtrees are distinct under midpoint and power-of-two splits.
These are source census facts, not a checked certificate.

**What the candidate removes from incoming state.** The pre-candidate scan
state carried an accumulated output ByteList and a completed-token history
beside the emitted-byte count. The output history is the load-bearing
removal: every closed scan equation would carry the output prefix (about
4,287 bytes average here, ~103KB of Cons records), and ~95k distinct state
terms alone would need a ~10-20GB ground table — three orders over the 8-MiB
request before one proof row. The token history measured 29,780 distinct
transition keys against 1,685 local (byte, comment, pending) keys on the
earlier subject. Removing both is a feasibility precondition, not an
optimization. The retained state — comment mode, reversed pending token,
operand expectation, checked output count, sticky failure — stays a small
bounded term on the success path; malformed pending tokens may span the
source but only cost rejection paths, not this certificate.

**What it keeps and adds.** The checked output count remains in incoming
state for assertions and output capacity, so the named counter cost remains:
the repeated output-successor recipe, reproduced from its existing algebra,
projects 4,611,614 work unshared at 8,575 increments (the board's 4,494,082
was the earlier 8,355-byte tape) and 2,122,796 work with the measured global
DAG sharing — still 3.2 times the work provision, with 95,136 proof rows,
69,538 ground terms, and 4,667,320 owner+witness bytes, ~56% of the request
limit for that one component. The candidate adds the fragment sort,
per-result fragments, Join composition, and a final flatten.

**Component ledger (work).** Measured recipes where they exist, honest
ranges otherwise; nothing below is a full-subject check.

| Component | Derived work | Basis |
| --- | ---: | --- |
| Theory formation, complete package | ~0.2-0.5M | Estimate; the equations do not exist. Current 57-function arithmetic package: 111,996 |
| Envelope/ASCII + source capacity | ~0.6M | Shape-mediated count checked 547,817 on the earlier subject; 76 distinct byte classifications share |
| Leaf byte transitions | ~0.4-0.9M count-free; ~4.5-9.5M with count in state | Measured census: 1,521 vs 17,130 distinct leaf states; ~60-150 per key plus ~200-400 per shared transition derivation |
| Join/state transitions | ~2.4-2.8M | Existing 55+4/key algebra over ~43-48k estimated keys; count pushes keys toward the 47,747 ceiling |
| Token completions | ~0.8-2.2M | 595 distinct token values (~1-3k each) plus ~3,751 uses |
| Output count | ~0.9-2.1M per-emission bounded add (unimplemented); 2,122,796-4,611,614 successor recipe | ~3,494 distinct emission-granularity count values; 8,575 per-byte |
| Flatten | ~0.5-0.9M concat-collapsed; ~3-5M raw mirror | ~7k vs ~95.5k fragment nodes plus 8,575 appends; the collapse is permitted, not required |
| Assertions, finalize, root | ~0.15-0.5M | 218 word-compare Equal plus tape-length and owner-root comparisons |
| **Total** | **~6-9.5M optimistic; ~15-27M as written** | |

**Request bytes.** The shared counter component alone measures 4,667,320
owner+witness bytes. Adding ~17,130 distinct state terms (~150-400B each),
~95-200k scan/flatten rows, distinct subtrees, token derivations, the larger
theory, and the owner tape puts a plausible total at ~13-26MB — 1.6-3.1
times the 8,388,608-byte request. The producer's own buffered-output limit
(16,777,212) is also exceeded at the top of that range.

**Storage.** The allocation ledger admits `7,864,346 + 48W + 128` pairs for
work `W`; the physical ceiling under the selected 40,265,318-pair arena is
W = 675,017 — the 655,360-work provision is already ~97% of that physical
ceiling in the 1.75GiB realization. At the measured counter-only work the
ledger needs
~110M pairs (~4.4GB); at the integrated totals, ~0.3-1.3G pairs (~12-52GB).
"Larger work provision" is therefore not a counter change but a native-backing
decision, the option already named below.

**Encoder-state sharing (measured).** At every source boundary the full
incoming-state tuple takes 17,130 distinct values with the count field and
1,521 without it — an 11.3 times multiplier from count carriage alone.
Distinct count values are 3,494 at emission granularity (8,575 per-byte).
The distinctness is semantic, not a sharing failure: no interning merges
states that differ only in the count. Moving count out of incoming state is
the largest identified lever inside the candidate's latitude (~11 times on
the leaf component's distinct-key work); it relocates the ~2M-class counting
into a separate
discharge pass rather than removing it, and it raises the assertion-discharge
design question the equations must answer.

**Verdict on the named comparison.** Against the repeated output-successor
family the candidate plausibly does reduce total audit burden: it converts
a physically impossible route (~10-20GB request) into a bounded one, and the
4,494,082-class figure was one component's unshared cost, not a rival total.
Against acceptance it does not close: every derivable scenario lands ~9-41
times over the work provision and ~9-40 times over the physical pair
ceiling, ~1.6-3.1 times over request bytes, and ~7-32 times over the pair
arena. The residual gap is structural (RAM-bound work, per-distinct-fact
recipe costs), not a constants problem.

**Missing quantities (now measured).** The complete package and an explicit
full-subject producer now exist, replacing the extrapolations above. The
emitted theory is 116,992 bytes (18 sorts, 361 constructors, 108 functions;
encoder functions 58..108 occupy 22,816 bytes). A pure-Python stepper
(`tests/gamma/beta-encoding-theory/stepper.py`) replays every ground
application through its stated clause and emits the corresponding
unfold/congruence/transitivity certificate rows. On the exact selected
subject — the 47,748-byte evaluator source as a midpoint-split Source tree
(22,339 interned owner terms) and the 8,575-byte tape — it independently
reconstructed the owner proposition
`encode_Beta(S, 0x4000000, 0xfffffc) = Success(T)` and produced the complete
derivation in 24.1 seconds:

- owner proposition 534,208 bytes over 22,339 terms;
- certificate 134,800,268 bytes over 2,130,039 witness terms and
  3,182,484 proof rows (1,018,573 unfold, 1,157,485 transitivity,
  936,597 congruence, 69,829 reflexivity; maximum proof depth 204);
- total request 135,451,492 bytes — 16.1 times the 8,388,608-byte
  provision;
- checker work not executed (the request cannot be admitted); at the
  measured ~13.9-16.5 work/row across the checked encoder batches the
  derivation projects ~45-52M work — ~70-80 times the 655,360 provision
  and the ~675,017 physical pair ceiling.

Producer peak RSS was ~2.8GB on macOS arm64. These are measured actuals for
one straightforward producer shape, not a lower bound: the named levers
(count-out-of-state, concat-collapse flatten, denser row sharing) could
shrink it, but even a tenfold reduction leaves ~13.5MB request and ~5M
work — still over both provisions and the physical pair ceiling.

**Disposition.** Retire the repeated output-successor recipe and the
history-bearing state design as baselines; the candidate supersedes both on
its own terms. The complete equations now exist and the produced
full-subject certificate confirms this ledger's direction: measured at
16.1 times the request provision and ~70-80 times the work provision /
physical pair ceiling — reproduced exactly at `6a1751fe08` on macOS arm64 —
the single-request route does not reach acceptance under the selected
provisions. The remaining routes are recipe
restructuring toward ~675k work (the measured 3.18M-row / 2.13M-term
certificate would need ~two orders of magnitude of further reduction —
not a plausible constants gap), checked closed-lemma composition across
bounded requests (a checker addition needing owner escalation), or more
native backing (an owner-level realization decision already named below).
That residual choice is filed as owner decision
`beta-encoding-certificate-admission`. Splitting into multiple
requests changes nothing without that composition rule: the checker
validates premises only as earlier rows in one table.
Do not select a larger profile from these costs.

## Coherent provisions

Compare the integrated recipe against the
[checker checking/allocation ledger](../../bootstrap/proofs/checker/CHECKING.md),
[comparison argument](../../bootstrap/proofs/checker/COMPARISON.md#amortized-allocation-argument),
and Gamma's enclosing frame and arena, not a work-limit constant alone.
The current request/work provisions are 8 MiB and 655,360 work; larger private
provisions are candidates, not demonstrated fits or changes to language laws.

**Decision — superseded by owner decision `beta-encoding-certificate-admission`,
which selects the native-backing route; the provisions this section declined to
change are now GAMMA-DERIVATION-CHECKER's work, and the reasoning below records
why no smaller edit reaches them.** Every derivable integrated
scenario lands ~9-41 times over the 655,360-work provision and ~1.6-3.1 times
over the 8-MiB request, while the allocation ledger admits at most
675,017 work under the selected 40,265,318-pair arena — the current provision
already consumes ~97% of that physical ceiling. A larger work provision is
therefore not a private-capacity edit but the native-backing route, which is
an owner-level realization decision; a larger request likewise requires
rederived ground/index/memo and allocation bounds before it is a candidate.
No still-partial ledger justifies selecting either, and no profile change is
recorded at the owners.

A larger request requires rederived ground/index/memo and allocation bounds.
A larger Gamma frame requires a coherent memory layout and exact/adjacent
failure controls. Tighter allocation accounting must cover accepted, rejected,
and interrupted requests, not only cheap successful branches. If full costs
justify more native backing, compare a fixed zeroed startup allocation against
the [static image constraints](../../bootstrap/0_alpha/README.md), preserving
the same monotone arena and auditing both host realizations and allocation
failure. No new allocator, opcode, or accelerator is authorized by this option.

Resolution requires one full-subject ledger of definition cost, theory/root
ownership, request bytes, actual sharing, work, allocation provision, producer
and checker depth/time, and remaining uncertainty. The integrated ledger above
now derives the measurable share of that bill: every derivable scenario lands
~9-41 times over the work provision, whose physical pair ceiling is ~675,017,
and ~1.6-3.1 times over request bytes. The remaining unknowns are the actual
equations, the integrated full-subject recipe, and the Join-level state census.
Until the complete definition package and integrated recipe exist, the pause
on isolated helper/provision expansion remains; the derived shortfall is
structural evidence for the routes above, not authorization to pick one.
Independent bootstrap work can continue; small proof successes are not new
authorization.
