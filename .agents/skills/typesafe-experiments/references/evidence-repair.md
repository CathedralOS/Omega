# Evidence-only repair

Use when an existing factual claim lacks adequate citations and the relevant source
material is available. First ask whether finding evidence can replace rewriting
the answer. Do not assume the claim is true: absence or contradiction must leave
it unresolved, not encourage searching until something seems to agree.

## Smallest useful path

1. Retain the exact claim, question, source revision and permitted source paths.
2. Group only identical claim/question/source-set/revision requests. Different
   owners or snapshots are not duplicates just because their words match.
3. For an existing literal quote, first try its uniquely containing paragraph in
   the same source revision/path, within the size cap. Preserve all original
   quotes; missing or multiple matches mean no automatic expansion. This repairs
   omitted antecedents such as "those owners" without searching or generating.
   Containment is NOT semantic support: retain source review/checking. Otherwise
   retrieve whole paragraphs with the existing lexical ranker. Keep deterministic
   top-one as a baseline; never use expected labels or handpicked answer anchors.
4. Ask whether each candidate independently supports the complete claim for the
   requested owner and conditions. Distinguish support for that claim from whether
   it answers every part of the broader question. Check answer completeness
   separately; do not demand missing assertions within a claim-support judgment.
   Batch independent judgments within a budget;
   isolate different experimental arms to avoid giving one a hint from the other.
5. Copy a sufficiently supported candidate verbatim; keep the claim unchanged.
   Verify source path, revision and literal membership before retaining the patch.
No suitable candidate means unresolved and eligible for normal review.

The current local prototype uses six candidates, a 2,400-character paragraph cap,
and P(supported)>=.90. These are development settings, not universal confidence
guarantees. Oversized paragraphs and multi-passage claims can fall outside this
prototype. A single paragraph must establish the whole claim; partial matches do
not accumulate into proof. Missing/invalid API results are errors, not acceptance.

Save citation replacements separately from the original answers so a reviewer can
see what changed. A source-support judgment is neither source truth nor compiler
correctness. This recipe does not authorize publication, suppress checks, or turn
an unreviewed response into a trusted answer. Do not append unchecked prose.

## Owner-bound handoff repair

For a structured assignment with known evidence owners, carry those owners and
the source revision in the assignment contract. Check them before semantic review;
similar statements about different registries are not interchangeable evidence.
Distinguish required owners from merely allowed references. Caller-supplied owner
bindings are additional information, not a demonstrated model-routing capability.

When a handoff is held for missing evidence, another model's approval of the same
packet does not supply the missing facts. Preserve the warning and its evidence
identity. Acquire relevant authoritative material, then reevaluate; new evidence
permits review but does not guarantee acceptance. If the complete pinned material
is already present, keep unresolved cases for review rather than repeatedly fetching
the same file. A reviewer may also resolve a warning by identifying specific support
the first pass missed; a bare changed verdict is insufficient.

Keep evidence-updated intermediate artifacts separate from ready answers. Preserve
the original claim and show which sources changed. Owner binding and literal text
checks do not establish truth, and conservative false warnings can erase the speed
benefit. Compare against simply acquiring the declared owner documents without AI.

## Measure the advantage honestly

The current development prompt candidate adds this clarification to the original
support instructions (preserving their owner, scope and partial-support rules):

> Evaluate support for this single claim, not completeness of the entire answer.
> The claim need not answer every part of the query. Use the query to resolve
> intended entity and scope, not to demand additional assertions. Every assertion
> actually present in the claim still requires support.

This passed the exposed compiler/control replay, not a fresh holdout. Freeze it
for the next comparison; retain the original prompt as baseline and do not relabel
earlier results as using this candidate.

Compare against lexical selection, ordinary review and safe abstention. Include
wrong-owner, absent, contradictory and conditional evidence alongside positives;
report authored controls separately from naturally observed defects. Measure
successful repairs and unsafe replacements, not merely fewer reviewer inputs.
Count retrieval, API and fallback work, and label stage timing separately from
end-to-end timing. Provider probabilities are not calibrated guarantees.

The recorded four-defect replay was repaired by both lexical selection and Jev.
Removing regeneration was the demonstrated opportunity; unique Jev benefit was
not established. See [the result ledger](experiment-record.md) for subsequent
controls. Local replay entrypoint: `build/experiments/citation-cascade/repair.py`;
its ignored dependencies are experiments, not an installed production service.
Reuse them when available rather than creating another orchestration framework.

## Direct selection instead of a generator

For a caller that already has a claim and needs one supporting source, trial a
Choice over retrieved paragraph IDs plus `none`. Code copies the selected paragraph
and provenance; it does not ask a second model to rewrite the claim. Unlike ranking
alone, the question must distinguish full support from similar words, contradiction
and extra unsupported conditions. Preserve no-match as a gap, not failure proof.

The direct-evidence controlled probe matched SWE-2 Max on eight authored cases and
was 22.8x faster in measured selection stages. Lexical completed the four positive
cases but could not supply the required no-support judgments; always abstain
completed none. See the ledger for cold-start, sample and timing limits. This earns
fresh operational validation, not automatic approval or a general reliability claim.

The first supervised live task exposed a boundary: natural implementation claims
span callers, helpers and execution history. One function can be relevant without
supporting the whole claim. Retrieve a bounded provenance-bearing evidence bundle
when that relationship is required, or keep the gap; do not silently weaken the
complete-support rule. Source code is not an execution receipt. A before/after claim
needs both observations, not a test definition or final-only run. Preserve source
hashes and reject reuse after the code changes. The live trial was fast but failed
the zero-incomplete-binding gate; its packet is not an automatic approval artifact.
