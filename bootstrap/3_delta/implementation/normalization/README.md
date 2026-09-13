# Gamma body-height normalization

Start at [program.gamma](program.gamma). Its `normalize_program` entrance sits
between complete [Gamma lowering](../lowering/README.md) and
[serialization](../emission/README.md). It consumes and produces the same
[Gamma program representation](../representation/README.md). It does not parse
Delta, repeat frontend judgments, or write receipt bytes.

[expressions.gamma](expressions.gamma) coordinates height budgets and the common
visit/resume machine. [arguments.gamma](arguments.gamma) and
[bindings.gamma](bindings.gamma) retain pending call and let children.
[helpers.gamma](helpers.gamma) owns extraction and generated definitions;
[capture.gamma](capture.gamma) collects free bindings under their lexical scopes.

The selected Gamma evaluator admits at most 255 nested expression lists per
function body. Lowering can exceed that height even when Delta syntax stays
within its separate 1,024-level expression-depth profile. Normalization moves
whole over-height fragments into generated functions, retaining their position
in the original expression's evaluation.

## Height budget

`normalization_height_limit` in [program.gamma](program.gamma) owns the selected
255-list budget. Each function body starts with that budget. A subtree whose
recorded height fits the remaining budget is reused unchanged. Otherwise traversal
descends through its Gamma call or let structure with one less level available
to each expression child.

Rebuilt nodes refresh both expanded height and canonical byte extent through
the ordinary Gamma constructors. Extracted helper calls retain their own
extents. Capture collection leaves the completed body unchanged, preserving
both summaries on its immutable nodes.

At budget one, an over-height fragment reserves a helper name and restarts
normalization under a fresh budget of 255. Descendant helpers finish first.
Capture then collects free bindings from the completed, height-bounded body,
including arguments to descendant helpers, before appending its definition.
The replacement call passes only already-bound values. Those arguments have height zero, so
the replacement call has height one. The initial program can also contain lowering's
shared projection definition for wide constructors. Its height-three body
already fits; it is not an extraction helper or an authored Delta function.

No work is moved into a sibling expression or before an enclosing initializer.
The helper call occupies the extracted fragment's exact evaluation position.
Branches remain conditional, earlier arguments still precede later arguments,
and authored computations and traps remain inside the fragment. A call that
replaces a tail-position fragment remains in tail position; the fragment's
original result becomes the helper's result.

### Height and completion argument

The [representation constructors](../representation/gamma/expressions.gamma)
assign atoms height zero and calls/lets `1 + max(child heights)`.
The [expression serializer](../emission/expressions.gamma) emits precisely that
expression-list nesting; declaration and parameter-list wrappers are outside
the body. Immutable reuse preserves the summary, and capture collection does
not change the body.

These height additions cannot overflow for admitted source. Let `N` be source
bytes, at most 4,194,304. There are at most `N` authored expression starts on
any path. Lowering adds at most one level for an ordinary call/let, at most
`N` for a constructor product, seven for arithmetic, and `4 + 3*N` for a
match: three wrapper lets, at most `N` arm selectors plus their comparison
level, and at most `2*N` pattern lets and field projections. Each is bounded
by `16*N` for nonempty source, giving the deliberately loose bound
`16*N*N <= 2^48`, below signed
64-bit overflow. Shared projection prefixes do not increase path height.

For each visit, order `(height, 255 - budget)` lexicographically. Descending
to an expression child decreases height. Extraction at budget one retains the
original fragment but restarts at budget 255, decreasing the second component.
Finite argument lists and pending frames account for the remaining
visits. Thus the traversal terminates in an unbounded-resource model. By the
same induction, each returned body fits its requested budget: a reused node
already fits, rebuilt children fit budget minus one, and an extracted call has
height one. A helper body is normalized before its definition is appended,
so the bound covers all generated helpers, not only authored functions.
Collecting its free bindings preserves the completed body's height.

This is a source-level audit argument, not a machine-checked certificate or
a guarantee that the selected evaluator has enough allocation/work resources
to finish every admitted compilation. Byte-extent arithmetic is a separate
summary and is not bounded by this height argument.

## Captured bindings

[capture.gamma](capture.gamma) visits the completed fragment and coordinates
three counted lists: direct references, binders owned inside the fragment, and
argument batches from completed extraction helpers. It leaves the body and
binding atoms unchanged. Global call heads and constants are not captures.

Binding identity makes one final set difference sufficient. Within a produced
function, each identity has one lexical binder and every reference resolves to
it. A free identity therefore cannot denote an unrelated internal binder in a
sibling or initializer. Source identities are declaration starts; generated
identities are marker/coordinate pairs. Equal spellings in disjoint scopes
remain distinct identities. For example,
`(let value Int (let value Int 1 value) value)` owns both identities and has
no free reference. This argument applies to produced plans, not synthetic trees
that reuse an identity for unrelated binders.

The collection owners are:

- [capture/calls.gamma](capture/calls.gamma) traverses ordinary arguments.
  Only normalization creates generated marker-104 (`$h`) heads. Descendants
  finish before their parent's capture, so these calls already carry sorted
  unique binding arguments. Collection retains their existing counted lists;
  it does not traverse, sort, or copy their elements.
- [capture/lets.gamma](capture/lets.gamma) records the binder and visits both
  children. Identity-based subtraction replaces saved lexical scope spines.
- [capture/bindings.gamma](capture/bindings.gamma) orders generated bindings
  first by marker/coordinate, then source bindings by declaration start.
  It never compares pair provenance or source spelling.
- [capture/sets.gamma](capture/sets.gamma) sorts only newly encountered references
  and owned binders, unions sorted batches, and subtracts owned identities.
  Merge and difference copy consumed prefixes and reuse untouched tails.
  Counted-list merge sort uses logarithmic non-tail depth; its element walks
  are tail calls. It introduces no tree, map, allocator, or profile.
- [capture/result.gamma](capture/result.gamma) composes those operations.
  With no owned binders, it returns the merged batch unchanged.

One sorted unique list supplies both parameters and replacement-call arguments.
The same permutation is applied to both; all arguments are already-bound atoms,
so reordering them moves no computation or trap. Initializers remain exactly
where lowering placed them. Pair-bearing values keep their provenance.

Original spelling is safe too. A free binding was active throughout the extracted
fragment. Delta forbids active shadowing, so internal binders cannot conflict
with its spelling, and simultaneously active captures have distinct spellings.
Lowering's source-forbidden `$` names use distinct markers and coordinates.
Each Gamma function is validated independently; capture does not follow global
helper definitions. Only helper names receive fresh `$hN` identities.
Names are allocated in extraction order; definitions are appended in deterministic
completion order, with inner helpers possibly preceding their parents.

### Capture allocation ownership

Splitting descendants before collection avoids rebuilding oversized tails.
Reusing original atoms and bodies avoids fresh parameter atoms and renaming
maps. Batching additionally avoids traversing and rediscovering every reference
in a completed helper's argument list. These are separate changes; none alone
establishes a whole-producer linear bound.

For the current counted-list implementation, in pairs:

- Starting a collection allocates three; adding a direct reference allocates
  three; adding an owned binder or a completed batch allocates four.
- A nonfinal ordinary argument allocates three continuation/payload pairs;
  a let continuation allocates two.
- Union consuming `q` prefix entries allocates `2q + 1`; difference copying
  `p` surviving entries allocates `2p + 1`. Unconsumed tails stay shared.
- Sorting `n >= 1` direct entries allocates at most
  `(2.5 * ceil(log2(n)) + 1) * n` pairs. Existing helper batches are not sorted.

Generic multiple-batch unions can still repeatedly copy prefixes. Earlier
checking/lowering and normalizer frames and rebuilt nodes also allocate or
perform work; emission preflight allocates no pairs, and its publication
traversal is separately bounded below the pair arena by the admitted payload
extent (see [emission](../emission/README.md#publication-traversal-pairs)).
The lexical trie reused by lowering is another
construction pass, not free reuse of checking's stored environment.
These formulas are source-level accounting, not measured arena peaks or a
complete resource proof.

#### Full-width payload refusal

The regression keeps the full-width control's declaration, pattern, and identity
entry, but changes `select` to reconstruct `(Wide field00000 ... field65534)`.
Its 1,704,033 bytes have SHA-256
`c69598944c34dc0f37187fb67bcf5624b021ac393a8cd8d91f7b967ab84a0945`.
Expression depth is three and active locals are 65,536. Source and syntax fit
their independent provisions. The selected outcome is exact payload refusal,
not publication of a larger Gamma artifact.

The preceding unbatched collector's nonfinal argument continuation allocated
**five** pairs (two frame, three payload), not six. Counting only product cuts
2 through 257 gives
`5 * sum(j=2..257, 65,535 - 254*j - 1) = 41,780,480` pairs, already above
the 40,265,318-pair arena. Its first 257 product cuts also required at least
183,609,879,296 binding comparisons. These are source-derived bounds, not
observed evaluator exhaustion or measured durations.

In the batched route, product helpers prepend small lower-coordinate field
prefixes to shared suffix batches. Generated-first ordering permits payload
bindings to join at the head. Pattern helpers subtract their owned suffix
once, copying the retained field prefix once rather than once per binder.
This removes the demonstrated capture recurrences without a source-specific
accelerator. Lowering separately reuses the existing exact-name trie instead
of making 2,147,450,880 linear binding comparisons for this source.

The exact requested payload count is independently derived from serialization,
not from a resource lower bound or compiler-produced count:

| Component | Bytes |
| --- | ---: |
| Original unnormalized payload, including fixed runtime and adapter | 4,446,892 |
| 774 helper wrappers and decimal helper-name digits | 19,904 |
| 516 captures of the eight-byte payload binding | 12,384 |
| 16,909,062 captures of ten-byte field bindings | 473,453,736 |
| Total requested payload | 477,932,916 |

Three match-wrapper lets leave budget 252. Projection helpers have even IDs
0..514; binding-chain helpers have odd IDs 1..515 and capture field prefixes
of length `251 + 254*j`. The final chain starts at field 65,529; its six lets
leave the constructor budget 249. Product helpers 516..773 capture suffixes
starting at `247 + 254*j`. Each family has 258 members (`j = 0..257`).
A nonempty helper adds
`16 + 2*name_length + 2*sum(binding_name_lengths) + 8*arity` bytes,
including its definition LF. Capture order changes none of these lengths.

The [opt-in resource control](../../../../tests/delta/resource-boundary/README.md)
uses this exact source and a literal 40-byte DCOUT resource-12 expectation:
coordinate and limit 16,777,212, requested 477,932,916. Its host watchdog is
not a language limit. A successful refusal still does not close allocation
containment for all admitted programs or the Delta refinement edge.

Canonical compiler SHA-256
`7b39266be43a7459a717f6624cc6e128579869398eae3e3ecef5a08006183df5`
completed this exact DCREQ/profile-1 fixture on macOS arm64 in 4,855.704 seconds:
status 2, exactly the expected 40-byte frame, and empty stderr. The frame's
SHA-256 is `e968f867c7a64e64a9340320dafb7b925487e7d7aad6567c38b10ae51966d155`.
This is completion evidence for one stress source, not a measured arena peak,
bootstrap-chain duration, or controlled speedup comparison.

### Static validation-environment bound

The selected Delta frontend permits at most 65,536 simultaneously active
parameters, let bindings, and pattern bindings. Lowering preserves their scopes.
Only [checked arithmetic](../lowering/arithmetic.gamma) and
[matches](../lowering/matches.gamma) add local binders: three per arithmetic
expression, three per payload match, or one per nullary match. Products, calls,
and field projections introduce no binders; pattern lets correspond to authored
bindings. Each active generated wrapper belongs to an expression on one source
nesting path, whose admitted depth is at most 1,024. Thus an unnormalized body's
active environment is conservatively bounded by
`65,536 + 3 * 1,024 = 68,608` bindings.

Extraction preserves that bound. Its distinct parameters replace a subset of
the bindings already active outside the fragment; they do not accompany a
second copy of that environment. Bindings introduced inside the fragment retain
their scopes and are not parameters. This mapping is injective and composes
through nested extraction. An inner helper collects original bindings before
the outer helper is closed; outer collection consumes its argument batch, not its
global head or completed definition. Sibling scopes and let initializers do not retain
bindings introduced only in another child.

Gamma validates each function independently, restoring the environment after
each let and resetting it between functions. Every authored or extracted body
therefore fits its 131,072-row validation environment. This is a conservative
source-level audit argument, not a claim that 68,608 is attainable or a checked
edge certificate. It does not bound aggregate bindings during non-tail runtime
calls. Fixed runtime and adapter definitions are separate from the transform
and must fit independently. Lowering's shared projection definition likewise
fits independently with two parameters and no local binders.

## Phase and receipt boundaries

`prepare_admitted_source` continues to return the pre-normalization plan.
The private lowering-height diagnostic therefore observes the same expanded
heights, including heights above 255. Normal compilation calls
`normalize_program` before `emit_checked_program` publishes any receipt bytes.
The fixed byte helpers and profile adapters retain their existing text.

If every authored body already fits 255, no helper or rewritten subtree is
needed and the existing receipt stays byte-identical. Over-height programs
receive generated definitions as part of the same ordinary Gamma program;
this is not another evaluator, runtime representation, or application profile.

## Remaining resources

Body-height normalization does not establish complete Gamma-profile admission.
Generated helpers are ordinary declarations. The evaluator's function table
fits every source admitted by its request extent; payload preflight therefore
also bounds generated function storage. Calls
outside tail position can add live contexts to its separate 256-context limit,
and plan construction, captures, and execution consume finite immutable storage.
The transform does not introduce a new language refusal, increase a selected
profile bound, or manufacture compiler-owned resource evidence.

Exact body heights, capture behavior, evaluation and trap order, existing
receipts, and actual Epsilon reconstruction remain validation obligations.
The [normalization gate](../../../../tests/delta/normalization/README.md)
compares production-plan observations with authored expectations, then compiles
and executes the generated Gamma. The separate
[lowering-plan gate](../../../../tests/delta/lowering-plan/README.md)
retains its pre-normalization measurements.
Compiler-owned resource/internal DCOUT publication and the complete Delta edge
remain open even when every generated body satisfies the nesting bound.
