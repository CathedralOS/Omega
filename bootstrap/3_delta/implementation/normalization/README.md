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
- Union emitting `q` entries allocates `2q + 1`; difference copying
  `p` surviving entries allocates `2p + 1`. Unconsumed tails stay shared.
- Sorting `n >= 1` entries allocates at most
  `2.5*n*ceil(log2(n)) + 3*n - 2` pairs: at most `2.5*n` per recursion level
  for emitted entries, reversals, and split prefixes, plus at most two pairs
  per internal node for the split result and union join and one per leaf for
  the base result. An earlier statement of this bound used a `+1` linear
  coefficient; the split-prefix pair and the union join make the linear term
  `3*n - 2` (for example `n = 4` allocates 30 pairs, not 24). Existing helper
  batches are not sorted, and an empty list still allocates its one-pair
  counted result.

`capture_finish` prepends the fragment's sorted direct references to its `k`
completed helper batches and merges the `k + 1` sorted lists through
`capture_merge_all`, then subtracts owned binders once. Each tail pass unions
adjacent sorted lists pairwise; a surviving list feeds exactly one union in the
next pass, so an entry occurrence is emitted at most once per pass and at most
`ceil(log2(k + 1))` passes run before one sorted union remains. Union is
associative and commutative on sorted unique lists, so the result is the same
sorted batch the earlier left fold produced; every receipt byte is unchanged.
With `T = R + E` total entry occurrences across the fragment's sorted
references and `k` batches, one collection's merges and input prepend
therefore allocate at most
`(2*T + 2)*ceil(log2(k + 1)) + 2*(k + 1) + 1` pairs: two per emitted entry
plus join, one survivor cons and pass record per pass, and the two-pair input
prepend. When the fragment owns `O` binders, sorting them and running the
final difference add at most `(2.5*ceil(log2(O)) + 3)*O + 2*T + 1` pairs: the
difference is one further pass emitting at most `T` survivors. The previous
left fold's `k*d` product — many batches each rescanning an accumulated union
of up to `d` distinct bindings — is replaced by this per-occurrence
logarithmic term. `d <= T` remains unbounded by the 68,608
simultaneously-active-environment allowance: fragment-owned binders can be
numerous even though the difference removes them once.

The aggregate is now closed by the same incidence charge the
[emission audit](../emission/README.md#reachable-byte-count-bound) derives.
A collection runs once per extraction; authored definitions are never
captured. Collections visit disjoint fragment interiors: each collection's
direct references are distinct local-reference occurrences, so
`sum(R) <= 32*N`, and fragment-owned binders are visited only by their
innermost collection, so `sum(O) <= G <= 40*S + 15`. Each completed helper
batch is prepended into exactly one parent collection, so `sum(k) <= J`, and
`sum(E)` is the nested helpers' parameter totals — forwarded capture
incidences bounded by the injective `(origin, ancestor cut)` charge at
`C <= R*L <= 512*N*N`. Hence

```text
sum(T) = sum(R) + sum(E) <= 32*N + 512*N*N
```

Summing the traversal bullets and the complete finish bound over at most
`J <= 64*N*N <= 2^50` collections — with `ceil(log2(R)) <= 27`,
`ceil(log2(G)) <= 25`, and `ceil(log2(J + 1)) <= 51` under the admitted
extents — bounds program-wide capture allocation by

```text
capture pairs <= 60,672*N*N + 5,680*N + 2,980*S + 1,118
```

This is a closed admitted-source envelope for the previously open `sum(T)`
aggregate, not a demonstration that the total fits the 3,422,453,760-pair
arena: the envelope already exceeds the arena at `N = 238` even with `S = 0`.
The capture
term is therefore no longer an unbounded expression, and the remaining
allocation question reduces to whether real admitted-source allocation can be
shown — by a sharper structural argument or by measured evidence — to stay
below the selected pair arena.

Earlier checking/lowering frames and rebuilt nodes also allocate or perform
work; the [checking audit](../checking/README.md#traversal-and-rebuild-pairs)
and [lowering audit](../lowering/README.md#traversal-and-rebuild-pairs) charge
each of those pairs per source occurrence, including the produced plan size
`G <= 40*S + 15` that this section's `45*G + 7*F + 1` consumes. Emission
preflight allocates no pairs, and its publication traversal is separately
bounded below the pair arena by the admitted payload extent (see
[emission](../emission/README.md#publication-traversal-pairs)). The
[traversal and rebuild accounting](#traversal-and-rebuild-pairs) below charges
the normalizer machine per plan-node occurrence. The lexical trie reused by
lowering is another construction pass, not free reuse of checking's stored
environment; its cumulative inserts are charged in the
[name-trie audit](../checking/names/README.md#pair-accounting). These
formulas are source-level accounting, not measured arena peaks or a complete
resource proof.

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
the retired 40,265,318-pair arena (the current profile allocates
3,422,453,760 pairs). Its first 257 product cuts also required at least
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
| Original unnormalized payload, including bound support members | 4,448,550 |
| 774 helper wrappers and decimal helper-name digits | 19,904 |
| 516 captures of the eight-byte payload binding | 12,384 |
| 16,909,062 captures of ten-byte field bindings | 473,453,736 |
| Total requested payload | 477,934,574 |

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
coordinate and limit 16,777,212, requested 477,934,574. Its host watchdog is
not a language limit. A successful refusal still does not close allocation
containment for all admitted programs or the Delta refinement edge.

Canonical compiler SHA-256
`fbcb9e17b7ce0c75849136086bc5a4b6df4264054be72b5aae6d70325f9d0929`
completed this exact DCREQ/profile-1 fixture on macOS arm64 under an lldb
halt breakpoint: status 2, exactly the expected 40-byte frame, and empty
stderr. The frame's SHA-256 is
`c95d55ef5b5c741bad050d7b870bb66fa9989c44e8e4b5a1e15f6276e74d47d5`. The same
observation read the evaluator's immutable-pair cursor at halt — 36,033,367
cumulative pairs, 89.5% of the 40,265,318-pair arena — recorded in the
[whole-producer study](../boundary/execution_storage.md#whole-producer-pair-study-measured).
The instrumented run took about 30.5 hours on a heavily loaded host, so it is
completion and allocation evidence for one stress source, not a
bootstrap-chain duration or a controlled speedup comparison. The earlier
direct run of the previous canonical closure finished the identical fixture
in 4,855.704 seconds; the support-member manifest change moved the expected
frame, so that duration does not carry to this compiler.

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

## Traversal and rebuild pairs

The visit/resume machine's frames and rebuilt nodes are charged separately
from the capture collections above. Each plan node is visited at most twice
and descended at most once: `normalization_visit` resumes a node whose
recorded height fits its remaining budget and otherwise descends it once; a
node still over height at budget one is revisited once under a fresh 255
budget as an extraction root, which replaces descent at the shallow budget
rather than adding to it. The extracted node's children are first visited
during that fresh-budget traversal, the replacement call's height one can
never descend, and rebuilt nodes are returned through frames without being
revisited. So every frame, cons, spine cell, and rebuild below is allocated at
most once per plan-node occurrence, and each extraction root is a distinct
occurrence.

| Site | Pairs | Charged per |
| --- | ---: | --- |
| Kind-1 remaining-argument frame | 7 | argument edge of a descended call |
| Argument result cons | 1 | argument edge of a descended call |
| Rebuilt argument spine cell | 1 | argument edge of a descended call |
| `gamma_call` rebuild | 6 | descended call |
| Kind-2 pending-let frame | 4 | descended let |
| Kind-3 completed-initializer frame | 3 | descended let |
| `gamma_let` rebuild | 6 | descended let |
| Helper frame, generated name atom, two state records, helper definition, replacement call, helper cons | 22 | extraction |
| Authored definition rebuild, cons, and depth-zero resume pair | 6 | authored definition |
| Helper-list and definition-spine reversal | 1 | helper / authored definition |
| `gamma_program` root | 1 | program |

A descended call with `a` arguments therefore allocates `9*a + 6` pairs, a
descended let allocates 13, and an atom allocates none. With `G` node
occurrences in the completed lowering plan, `J <= G` extraction helpers, and
`F <= 32,768` authored function rows, the machine allocates at most

```text
9*A + 6*C + 13*L + 22*J + 6*F + J + F + 1 <= 45*G + 7*F + 1
```

pairs, where `A`, `C`, and `L` are argument edges and descended call and let
occurrences, with `A + C + L <= 2*G`. This charges every normalizer frame and
rebuild to the plan being traversed. `G` is produced by lowering, so the bound
reduces the normalizer term to the earlier-phase plan size; it is not a fixed
byte budget. The capture merge term's aggregate `T` is separately closed by
the incidence charge in the
[capture allocation ownership](#capture-allocation-ownership) section above.

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

Generated helpers are ordinary declarations, so the emitted program's
admission bounds are the same bounds the rest of the receipt already carries:
the 16,777,212-byte payload extent places the whole source under the
evaluator's request extent and, at least eight bytes per completed
declaration, under its 2,097,152-row function table; this transform enforces
the 255-list body bound; and the
[static environment audit](#static-validation-environment-bound) holds every
generated body under the 68,608-binding allowance, below the evaluator's
131,072-row validation environment. Calls
outside tail position can add live contexts to the separate 256-context limit,
and plan construction, captures, and execution consume finite immutable
storage; those are resources of the generated program's own run, not compiler
outcomes. The transform does not introduce a new language refusal, increase a
selected profile bound, or manufacture compiler-owned resource evidence.

Exact body heights, capture behavior, evaluation and trap order, existing
receipts, and actual Epsilon reconstruction remain validation obligations.
The [normalization gate](../../../../tests/delta/normalization/README.md)
compares production-plan observations with authored expectations, then compiles
and executes the generated Gamma. The separate
[lowering-plan gate](../../../../tests/delta/lowering-plan/README.md)
retains its pre-normalization measurements.
The complete Delta edge remains open even when every generated body satisfies
the nesting bound.
