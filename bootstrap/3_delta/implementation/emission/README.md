# Expanded Gamma serialization

Start at [program.gamma](program.gamma). It serializes the counted, ordered
definitions in a completed [Gamma plan](../representation/README.md), emits
their separators, and selects the existing profile adapter at program end.
[declarations.gamma](declarations.gamma) writes each definition's name,
parameters, and body. [expressions.gamma](expressions.gamma) prints the plan's
ordinary Gamma calls and lets; [atoms.gamma](atoms.gamma) prints its atoms.

The pipeline finishes the complete frontend, selected-profile schema check,
and [lowering](../lowering/README.md) and
[normalization](../normalization/README.md) of every authored body before
writing the first receipt byte. A count-only serialization then measures the
complete payload using cached expression extents. Emission does not classify
Delta expressions, resolve locals, expand constructors or patterns, or choose
checked-arithmetic guards. Those
decisions are already explicit Gamma structure.

## Atom and expression custody

Source atoms retain exact admitted source spans through retained-node
accessors. Serialization may copy those bytes without interpreting their
Delta expression structure. Source binding references reuse the established
binding atom; generated names retain their marker and coordinate. Function
atoms receive the existing injective naming treatment, and fixed words and
integer atoms have dedicated textual encodings.

Expression serialization uses explicit continuations for pending sibling
arguments and let initializers. Final arguments and let bodies tail-enter with
a scalar pending-close count; unary projection chains allocate no continuation
per projection. This traversal is over Gamma plan nodes, not retained
Delta expression children. Counts govern list projections; continuation depth
is neither source expression depth nor generated Gamma expression-list height.
The plan's height summaries do not change serialization or silently select a
different representation.

## Fixed publication text

[text.gamma](text.gamma) owns textual primitives. [bytes.gamma](bytes.gamma)
retains the existing fixed byte-runtime and application-adapter text. The
pipeline's existing runtime selection remains separate from generic expression
serialization. These helpers, definition order, whitespace, hygienic spellings,
and the final publication byte retain the established receipt format.

Fixed runtime text uses ordinary Gamma character literals in groups of eight.
One shared writer replaces packed decimal constants, byte-unpacking arithmetic,
and a separate comment-agreement checker. Count mode advances by the complete
fixed extent; publication sequences writes with balanced addition, then advances
the count. Gamma's left-to-right evaluation preserves output order. Each leaf
returns a byte or a sum of eight bytes; the largest fixed body writes 660 bytes,
so every intermediate sum is nonnegative and at most `660 * 255`. No per-chunk
bindings or pair allocations are needed. The
[staged compiler gate](../../../../tests/delta/staged-compiler/run.sh) pins exact
runtime bytes, receipt counts, and execution. No new Gamma syntax or host source
generator is involved. Generic packed-word serialization remains separate.

Emission entrypoints receive a count/publication flag and running byte count.
The expression entrance chooses cached counting or publication once; recursive
publication does not carry or recheck that flag. Byte loops only write, and
their callers advance the count once for each completely written extent.
[extents.gamma](extents.gamma) supplies the canonical expression-size summary
retained by each Gamma node constructor. Its count-only helpers share atom
spellings and call/let prefixes with publication, adding cached child extents
instead of unfolding shared children. Every rebuilt node recomputes its summary;
reused immutable nodes keep theirs. Count mode adds span and packed-text lengths
without reading or writing their bytes; it does not build a byte rope.
The coordinator includes the entry-owned final LF in both totals. If the full
count exceeds 16,777,212, it returns DCOUT `Incomplete` resource 12 in payload
coordinate space 2, at byte 16,777,212, with the limit and exact complete count.
No application marker or partial program precedes that refusal. Admitted
publication checks its returned count against the preflight count; disagreement
remains an internal invariant failure, not a resource refusal.

Preflight accumulation and node summaries use checked nonnegative addition, never
saturation. Negative metadata propagates the private failure sentinel `-1`
through every subsequent count operation. Before publication, the coordinator
turns that result into canonical `InternalFailure` code 2
(`emission_metadata_contradiction`), internal-row space 3, row 0, with zero
limit/requested. Row 0 identifies the singleton complete-program emission
record; it does not claim to locate the first corrupt child or a source byte.
This is a retained-metadata invariant, not a source-admission refusal.

Positive signed overflow in private metadata still fails before any wrapped
count can be admitted or published. The source argument below shows why admitted
Delta cannot construct that demand; no additional refusal code is needed for it.
Malformed private projections and late replay disagreement retain raw evaluator
failures. The latter cannot append a failure frame after already written receipt
bytes; its existing raw evaluator failure discards publication.
Serialization summaries cost two additional immutable pairs per Gamma node:
one for the extent and one for the optional unary-word prefix described below.
Full Epsilon recompilation remains a storage regression check.

## Reachable byte-count bound

This is a source-level audit of the selected producer, not a checked refinement
certificate or a promise of sufficient compiler memory/time. Let `N` be admitted
source bytes (`3 <= N <= 4,194,304`); any well-formed complete Delta program is
longer than three bytes. Counts below refer to expanded expression occurrences,
not unique immutable pairs. Thus sharing cannot conceal a larger printed tree.

The [lowering templates](../lowering/README.md) place each authored expression
child once. Checked arithmetic binds operands instead of copying their trees;
match selectors retain each arm body once. An inlined
[pattern projection spine](../lowering/matches/bindings.gamma) with `k` binders
prints at most a quadratic number of `first`/`second` calls. Only widths through
256 use that inline form; wider patterns use linear call-site structure and
one shared height-three projection definition. The following conservative
quadratic envelope still bounds both forms, including that fixed definition.
All pattern binders together consume distinct source tokens, so
`sum(k) <= N` and `sum(k*k) <= N*N`. A conservative inventory allows 128
node/atom occurrences per remaining source token/expression and `4*k*k` per
pattern for expanded projections. Therefore the original program has at most
`M = 128*N + 4*N*N <= 64*N*N` occurrences, including call heads and binders.

Two smaller counts matter for capture. Local-reference occurrences are bounded
by `R = 32*N`: an arithmetic template uses at most eleven generated references,
match wrappers at most two per match, selectors one per arm, and each pattern
initializer one payload reference regardless of projection-chain length.
Authored references add at most `N`. A path has height at most `L = 16*N`:
charge the constant expression wrappers, constructor fields, arm selectors,
pattern lets and projection steps to their distinct source occurrences along
that path. With `E` expression nodes, `F` constructor fields, `A` match arms,
and `B` pattern binders on those constructs, `7*E + F + A + 2*B` covers the
path: seven covers each expression's constant wrappers/comparisons, while the
other terms cover the variable-length spines. Each count is at most `N`,
so this fits the deliberately loose `16*N` allowance.
This strengthens the separate height audit's looser overflow bound.

Normalization moves original subtrees into helpers; it never copies their
composite structure into both caller and callee. Each extraction starts at a
different original call/let occurrence, so helper count `J <= M`. Charge each
distinct captured parameter to one free local-reference occurrence below its
extraction point. Normalization completes descendant helpers before capturing
the enclosing helper, so this charge must include forwarded call arguments.
Initially each reference has its original expanded-occurrence origin. When an
inner helper captures a binding, assign its replacement-call argument an origin
from one of that binding's references in the extracted subtree. Outer capture
can forward that origin again, but only across an ancestor cut. Its deduplication
selects at most one parameter for each binding at that cut; different captured
bindings cannot claim the same original reference. Thus `(origin, ancestor cut)`
is an injective charge for capture incidences, including forwarded arguments.
Replacement calls have height one and do not introduce further extraction
points. A reference crosses at most `L` ancestor cuts, giving
`C <= R*L <= 512*N*N` without assuming the old capture order. This bounds retained
and intermediate capture incidences, not the work of looking them up or the
cumulative allocation of traversal frames. These counts also bound the
program-wide fresh-identity counter, which now advances only for helper names:
`J <= 64*N*N <= 2^50`; generated names consequently fit within 21 bytes,
without assuming that counter arithmetic was already safe.

Long source names need a weighted bound, not `N` bytes for every generated
atom. Original source-span spellings total at most `N`: a local reference reuses
its declaration's spelling, but its equal-length use also occupies source bytes.
Across extraction cuts, the original binding spelling is printed as both a
helper parameter and a call argument. Added long-name copies therefore total
at most `2*N*L`. Other generated atoms are short; unchanged body references are
included in the original-occurrence allowance below. Counting fixed syntax generously gives
the following complete-payload envelope:

```text
64*M + 128*J + 128*C + 2*N*L + N + 4,096 < 2^18 * N^2 <= 2^62
```

The 64-byte allowance covers each original occurrence's short atom spelling,
punctuation, annotation, and spacing. Each 128-byte helper allowance covers its
definition and replacement-call framing and names; each capture allowance covers
its parameter declaration and argument. Authored declarations and parameters
are included in the original inventory. Fixed marker, byte runtime, adapter,
and final LF total less than 4,096 bytes. This is deliberately not a tight size
estimate or a proposed payload provision.

Capture retains normalized bodies and their original atoms unchanged;
partially normalized programs add subsets of the helper/capture
incidences above. The envelope therefore covers intermediate cached extents as
well as final payloads, not the cumulative sizes of all discarded immutable
copies. Counts are nonnegative sums of those extents and syntax
bytes, so partial sums also fit signed 64-bit arithmetic. The actual publication
path runs only after exact preflight admits at most 16,777,212 bytes; its byte
increments and pending closes follow the same immutable formatting structure.
No saturation, fabricated requested count, or new source refusal is necessary.
These counts do not establish canonical outcomes for underlying evaluator
exhaustion; serialization's own traversal pairs are bounded
[below](#publication-traversal-pairs).

### Unary fixed-word prefixes

[unary_words.gamma](unary_words.gamma) caches a compact publication prefix for
one-argument calls whose head is a fixed word of one through seven bytes. The
cache does not select a particular primitive or inspect authored source names.
It encodes the visible low bytes and their length in one nonzero scalar below
2^59; ignored high bytes are masked before encoding. All other call shapes use
ordinary call publication. Zero means no cached prefix, not an empty spelling.

The expression visitor writes a cached prefix and tail-enters the sole argument
with one more pending close. This skips repeated atom dispatch and argument
dispatch when immutable call nodes occur many times in the expanded output.
The same byte writer emits fixed words on both paths. Counts continue to come
from canonical extents, and the final publication total must still agree.
Every reconstructed node refreshes both summaries; no cache is keyed by pair
address, compiler identity, or source pattern.

No source-dependent lowering template remains in this directory. Calls,
constructors, bindings, arithmetic, and matches belong under `lowering/`;
durable Gamma plan nodes belong under `representation/`.

### Publication traversal pairs

The counts above bound emitted bytes; this bound covers the immutable pairs
serialization itself allocates while producing them. The only pair
allocations in this directory are the pending-work frames in
[expressions.gamma](expressions.gamma): a nonfinal-argument frame costs four
pairs (two frame cells plus a two-pair counted payload), and a pending
let-body frame costs three (two plus one). Final children, unary-word calls,
and the scalar pending-close count allocate none.

The count-only pass allocates zero pairs: every expression answers through
its cached extent and each fixed or atom spelling advances by its measured
length, so preflight never descends the plan. Publication's byte loops,
decimal, marker, definition, and adapter writers likewise use only `write`
effects, lexical rows, and scalar arithmetic, not pairs. The two summary
pairs per Gamma node belong to plan construction, not to this traversal.

Publication runs only after exact preflight admits at most `P = 16,777,212`
bytes including the final LF. Charge each frame to distinct emitted bytes: a
nonfinal argument owns its separator byte and that argument's first emitted
byte, and a `let` owns at least twelve bytes after its opening parenthesis
(`let `, a binder, ` `, `Int`, ` `, the initializer-body separator, and `)`).
An argument's first byte can be another let's opening parenthesis, which is
why the let charge excludes it. Every other charged byte sits inside its own
frame's extent, so the charged sets are disjoint. A shared immutable child
revisited per occurrence simply charges that occurrence's own emitted bytes.
Therefore

```text
traversal pairs <= 4 * floor(P / 2) + 3 * floor(P / 12)
                <= 33,554,424 + 4,194,303 = 37,748,727 < 40,265,318
```

Serialization alone therefore cannot exhaust the selected pair arena: it adds
at most 37,748,727 cumulative pairs to a compilation whose earlier phases are
separately owned. This bounds emission's own traversal, not generated-program
execution; it manufactures no DCOUT outcome and changes no provision.

## Validation and remaining boundaries

The [emission gate](../../../../tests/delta/emission/README.md) exercises
count/publication agreement, prefix eligibility and fallback, continuation
ordering, and capture preservation on private Gamma-plan controls. These
controls do not claim source admission or executable program semantics.

The staged gate compares exact receipts, executes generated programs, and
exercises nested expressions and wide payloads. Exact Epsilon checking and
execution receipts remain separate full-customer reconstruction gates. A
changed receipt requires explanation, not a relaxed expectation.

The [lowering-plan gate](../../../../tests/delta/lowering-plan/README.md)
checks authored expectations against the pre-normalization plan's expanded
body heights. The separate normalizer handles the
[selected Gamma profile](../../../2_gamma/EVALUATOR_PROFILE.md#exact-capacities)
limit of 255 nested expression lists per generated function body. Serialization
does not make extraction or capture decisions and does not alter those budgets.

Plan and continuation pairs consume the selected evaluator's finite immutable
arena. The traversal bound above covers serialization's own frames; earlier
phases' cumulative allocation remains separately owned. Stack-safe compiler
traversal, complete-before-write planning, and exact
receipt preservation do not close compiler-owned resource/internal outcomes,
generated-profile admission, or the full Delta bootstrap edge.
