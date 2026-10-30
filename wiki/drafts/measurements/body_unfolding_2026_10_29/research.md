# Body unfolding investigation

2026-10-29 (America/Los_Angeles). User-assigned continuation of ITERATION-PERFORMANCE
in the retained contract-query-publication worktree. Fixed source
`c38d69bb14f8773414312e15388f7776ef966379`; compiler inputs still match baseline
producer `1c7645a93c087116c69bb5951cc940709cdadecfeae847d38f9de957d3ad4d60`.
The preceding [iteration](../remaining_first_checking_2026_10_30/README.md) rejected
an empty-equation shortcut below its predefined benefit threshold. Do not include
that patch in this candidate.

Measured uncertainty: body unfolding accumulated 128.05 s inclusive across
1,388,873 calls. This includes recursive predicate proving. Split setup, locals,
tail/target construction and scalar guards before attributing the cost to body
substitution or proposing an answer cache. The diagnostic adds observation scopes
only; it must finish ordinary package review and actual host native/Terminal
execution on the unchanged operand-order control.

Coordination status now shows prior ticket `1a92afc0764b427fa2e71bf095de2b2b`
registered despite its earlier timeout. Updated its note for this continuation.
Other active work overlaps contract entailment; all edits stay in this worktree
and would be reconciled on publication. Shared build slots serialize local work.

## Changed research question

Query: `site.lean-lang.org type checker cache whnf transparency unfolding memoization`.
Read [Lean transparency documentation](https://lean-lang.org/doc/api/Init/MetaTypes.html)
and [kernel type checker implementation](https://raw.githubusercontent.com/leanprover/lean4/master/src/kernel/type_checker.cpp),
especially `lazy_delta_reduction_step`, `failed_before` and `cache_failure`.
Lean distinguishes speculative unfolding from checking likely-valid input; its
kernel avoids expanding an expensive term when a cheaper projection or matching
application argument comparison suffices. It remembers failed argument comparisons
to avoid trying the same route repeatedly.

Transferable question: is Omega spending time repeating a question before it
knows unfolding can produce a useful answer? Lean's transparency modes and caches
cannot be copied directly: Omega's proof-context storage epochs, exact captured
payloads, selected preconditions and active recursion chain constrain answer reuse.
No source reduction limit or refusal behavior may change merely to improve timing.
Do not infer a speedup from this literature. Existing local memoization has an
exact-payload key and batch lifetime; inspect those before adding another cache.

## Decision rule before measurements

If the diagnostic identifies a small safe mechanism, compare fixed baseline and
candidate in A-B-B-A order using fresh independent project/cache roots and profiling
disabled. Require at least 10% lower first-update mean and separated ranges,
byte-identical native images, actual host engine agreement, and focused unfolding
and refusal controls before retention. Preserve every valid trial. Reject or mark
inconclusive when the screen fails; do not broaden tests after a rejected screen.
Inclusive scopes are attribution clues, not additive or exclusive savings.

## First diagnostic

Optimized diagnostic build: 3m59s. Fresh update: 151.77 s, ordinary review resume:
5.02 s, actual host native/Terminal comparison: 9.14 s, both engines exit 0.
Unfold total: 150.047 s inclusive; scalar guards: 147.954 s across 43,130 entries.
Setup: 1.302 s; callee term construction: 1.048 s (including 0.199 s preparation).
Locals/tails/targets are smaller. Do not optimize environment allocation based
on the large parent timer. The next diagnostic splits observed predicate
resolution, equation judgment, float and integer fallback.

Source inspection found a possible reuse boundary if integer fallback dominates:
`integer_predicate_observations` builds one goal and its premises under the same
immutable judge but disables the exact-payload operand-answer vector already
used by prepared goal batches. Reusing answers only within this private query
would leave every operation's formation/admission and atom registration intact.
The active application chain is constant for that judge; recursive queries own
separate observation records. This remains a hypothesis pending attribution.

## Guard diagnostic and candidate selection

The second optimized diagnostic build took 3m37s. Fresh update completed in
124.07 s, resume in 4.55 s and actual host comparison in 8.51 s (both engines 0).
Integer observation preparation accumulated 124.655 s inclusive across 44,985
queries, while predicate normalization was 3.160 s and structural equation
judgment only 0.052 s. The separate diagnostic timings are not an A/B speedup.

Selected candidate: enable the existing exact-payload operand-answer vector only
inside `integer_predicate_observations`. The sole immutable judge owns goal and
premise projection; recursive judge queries allocate independent observations.
Every miss still resolves at depth zero, and every occurrence still registers
its atoms, carriers and operations for the same admission algorithm. The vector
leaves with the private query result and is never installed into prepared premise
storage. No proof conclusion is cached and no active recursion boundary changes.
This is one field initialization plus comments, without the preceding rejected
empty-equation shortcut or diagnostic instrumentation.

The A/B harness uses independent fresh package caches. Focused correctness checks
reuse a candidate cache after its timed trial to avoid paying first preparation
again; they are behavior observations, not timing comparisons. If the performance
screen fails, run only selected-contract recursion positive/refusal controls.
If it passes, run the selected guard/snapshot/formation controls and the existing
exact-payload/context-isolation Rust unit test. Preserve every raw result.
