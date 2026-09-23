# Canonical compiler execution storage

This source-level audit covers the **Gamma program implementing Delta**, not
the Gamma programs it emits. It narrows the remaining
[resource obligation](README.md#resource-ownership-in-the-selected-producer);
it is not a checked refinement certificate or a cumulative pair-allocation bound.

The subject is the canonical [`delta_compiler.gamma`](../../delta_compiler.gamma)
prefix plus the ordered [`implementation.gamma.sources`](../implementation.gamma.sources)
closure: 147,840 bytes, SHA-256
`fbcb9e17b7ce0c75849136086bc5a4b6df4264054be72b5aae6d70325f9d0929`.
It executes under the exact source/tape and provisions in the
[Gamma evaluator profile](../../../2_gamma/EVALUATOR_PROFILE.md).
Changes to either executable subject require rechecking the corresponding
argument; a new source manifest digest alone does not preserve this evidence.

Runtime emission is support-member custody, not fixed text encoded in the
emitter. [`support.gamma`](../emission/support.gamma) validates the sealed
input's trailing 2,998-byte support section with one tail-recursive wrapping
byte-sum pass per member before any Delta-source phase, then copies each
member verbatim during publication through a tail-recursive `write`/`read`
walk while count mode advances by the same bound extent. The three support
loops carry at most three locals each and no non-tail user calls; the
fixed-source maxima below need no larger provision.

Sequential binding groups replace 94 nested-let chains containing 309 bindings.
Expanding each group to nested lets recovers the preceding compiler's expression
trees exactly: initializer order, binding scope, tail positions, and user-call
edges are unchanged. The purpose is to expose the binding sequence and decision
body without a wrapper per local. This does not remove pair-layout conventions
or the explicit typing and lowering continuations. No preprocessor is retained;
the selected Gamma evaluator validates and executes the grouped syntax directly.

## Fixed source and call inventory

The compiler contains 377 Gamma definitions. Inspecting every body gives these
maxima, including bodies not reached from the canonical `main`:

| Fixed-source quantity | Maximum | Owning body |
| --- | ---: | --- |
| Formal parameters | 11 | `typing_match_bindings` |
| Nested authored lists, including binding groups | 17 | `resolve_collected_constructors` |
| All authored local bindings in one body | 12 | `publish_compiler_failure` |
| Formal parameters plus all body bindings | 16 | `lowering_match_arms` |
| Pending user calls within one body's expression syntax | 5 | `lowering_projection_definition`, `typing_match_body` |

These are compiler-source counts, independent of Delta input depth, width,
identifier length, or generated-helper count. The fixed 17-list maximum fits
Gamma's 255-list validation provision. Its validator visits bodies separately,
resets the lexical environment at each definition, and never executes callees.

For execution, a call's arguments are non-tail. An `if` condition and a `let`
initializer are non-tail; their branches/body inherit the containing tail
position. Other primitive operands are non-tail. A user call temporarily owns
one context while evaluating its arguments, even when the eventual call is tail.
The evaluator releases that tail context before reusing the current activation.

Weight each caller-to-callee edge by the pending enclosing user calls plus one
for a non-tail callee activation. Include each call's temporary argument context
as a local peak. The complete inventory has 67 recursive components. Only three
call sites on recursive cycles have positive weight: two in `capture_sort` and
one in `emit_decimal`. The capture merge passes (`capture_merge_all`,
`capture_merge_pass`) and the support-member loops (`support_sum_region`,
`support_check_members`, `emit_support_bytes`) are zero-weight tail edges.

| Non-tail recursive owner | Decreasing quantity | Conservative additional contexts |
| --- | --- | ---: |
| [`capture_sort`](../normalization/capture/sets.gamma) | For `count >= 2`, both children are at most `ceil(count / 2)` and run sequentially. Any positive signed 64-bit count reaches the base case in at most 63 halvings. | 63 |
| [`emit_decimal`](../emission/text.gamma) | Recursive calls divide a value at least ten by ten. A positive signed 64-bit value has at most 19 decimal digits. | 18 |

The sort allowance does not depend on a capture-allocation estimate. Values
below two do not recurse. Trie descent across name bytes, sibling-row scans,
and ancestor rebuilding are tail-driven; name length does not multiply the
sibling-rebuild allowance. The remaining parser, grammar, typing, lowering,
normalization, capture, emission,
list, and cursor cycles have only zero-weight tail edges.

Remove those three bounded edges, collapse zero-weight recursive components,
and propagate maximum weighted demands from local peaks through the resulting
acyclic graph. The previous 18-context fixed overhead from `main` remains a
conservative bound: the support-member rewrite removes the counted fixed-text
writers and adds tail-recursive sum and copy loops whose calls sit in tail
position, not additional nested user calls. Reserving 64 for
that overhead and adding both recursion allowances, even though their
deepest paths do not coexist, gives:

```text
live call contexts <= 64 + 63 + 18 = 145 < 256
live function frames <= one main frame + live contexts = 146
```

This inventory was checked with disposable source inspection and direct review
of the recursive routines and evaluator. The method above, not a retained host
analyzer or its output, defines the audit. It does not execute Delta input,
manufacture a certificate, or grant a new checker rule.

## Lexical and temporary storage

In [`gamma_evaluator.beta`](../../../2_gamma/gamma_evaluator.beta),
`enter_function` installs parameters above the caller's environment;
`bind_let_initializer` adds a binding only after its initializer returns.
Ordinary scope/function completion restores the saved environment count, and
`enter_tail_restart` resets it to the current activation's base. An activation
therefore retains at most its parameters plus all its body's local bindings,
including every binding in a grouped `let`:

```text
live lexical rows <= 146 * 16 = 2,336 < 131,072
```

Every expression-list level retains at most eleven temporary-value entries:
an ordinary call holds at most its eleven arguments; primitive/`let` handling
uses at most three entries for saved operands, operator, environment count, or
binder coordinates. Validation's `let` handling also uses three. Nested child
evaluation is charged to the next expression level, not hidden in that count.
`prepare_call` records the argument base; parameter installation resets the
value pointer to it, ordinary calls restore it on return, and tail replacement
does not retain previous argument blocks. Allow one extra result entry per
activation for `enter_function`'s return handling:

```text
temporary entries <= 146 * (17 * 11 + 1) = 27,448 < 524,288
```

The validator executes no Gamma bodies, so its one-body requirements are smaller
than these execution bounds. Gamma's existing hidden-Alpha-stack containment
argument continues to apply; no native stack or memory partition is changed.

## Remaining obligation

The canonical compiler does not need more call contexts, lexical rows, or
temporary-value storage for source depth or width. Explicit worklists and
immutable plans still allocate **pairs cumulatively**; returning from a call or
dropping a worklist does not reclaim them. Every producer phase now carries a
per-occurrence charge derived in the same style:

- [Serialization's own traversal](../emission/README.md#publication-traversal-pairs)
  is bounded below the pair arena by the admitted payload extent.
- The [normalizer's frames and rebuilt nodes](../normalization/README.md#traversal-and-rebuild-pairs)
  are charged per plan-node occurrence with capture merges bounded per
  collection by `(2T + 2) * ceil(log2(k + 1)) + 2 * (k + 1) + 1`, the
  owned-binder sort and difference adding one further pass at most
  `2 * T + 1` plus the owned sort.
- The [checking audit](../checking/README.md#traversal-and-rebuild-pairs)
  charges census metadata, resolution rows, typing continuations, and
  environment binds per source occurrence — at most `39*S + 36` site pairs —
  and the [lowering audit](../lowering/README.md#traversal-and-rebuild-pairs)
  charges continuation frames, plan construction, and the produced `G` at
  `<= 200*S + 91` with `G <= 40*S + 15`.
- The shared [name-trie and cursor audit](../checking/names/README.md#pair-accounting)
  charges lookups, descents, fresh suffixes, and immutable rebuilds; departed
  ancestor levels amortize to descended name bytes through the cursor zipper
  identity, and each rebuilt branch level now prepends one fresh row at five
  pairs rather than copying up to 63 sibling rows.

The cumulative question those per-occurrence products left open — whether
the coarse envelope (at most `295*S + 34*N + 149` pairs for checking,
lowering, and shared names, before the normalizer's `45*G + 7*F + 1`) plus
the closed capture envelope `60,672*N*N + 5,680*N + 2,980*S + 1,118` fits the
selected arena — remains open as a general containment proof: the
envelope exceeds the 3,422,453,760-pair arena from `N = 238`, so it cannot
serve as the containment proof. The
[measured worst-shape study](../../../../tests/delta/resource-boundary/README.md#measured-worst-shape-pair-containment)
runs the canonical closure under an instrumented reference interpreter that
counts every immutable-pair allocation, measures selected scaled shapes on
each admitted extent axis, and projects 417,063,339 pairs at the full
extents — 8.2× under the arena. This supports the selected provision but does
not establish a bound for every admitted composition. The capture-chain regime at full width is
bounded independently by the exact `(origin, ancestor cut)` incidence count
and witnessed by canonical full-extent completions. This audit neither
supplies a DCOUT heap refusal nor converts an outer Gamma failure into one.

The same aggregate question under the retired 40,265,318-pair arena was
answered by measurement on the unchanged canonical closure: the products
did not stay below it. The closed envelope already exceeded that arena at
maximum source extents, and the measured study below shows real admitted
sources reached the exhaustion that envelope anticipated.

### Whole-producer pair study (measured)

The measurement reads the evaluator's own cursor, not an instrumented
allocator. In the retired-profile
[`gamma_evaluator.beta`](../../../2_gamma/gamma_evaluator.beta) the
persistent register `ra0` was the immutable-pair heap cursor: initialized to
`0x10000000`, preflighted against `ra1` = `0x70000000`, advanced by exactly
40 bytes per pair, and never decreased. Its halt-time value is therefore the
compilation's exact cumulative allocation and lifetime high-water. The Alpha
arm64 seed exposes `h_halt` and keeps `vregs` in `x19`, so an lldb batch
session — breakpoint on `h_halt`, then `((unsigned long*)$x19)[0xa0]` and
`[0xa1]` — reports both registers before exit without modifying the tape,
request bytes, compiler closure, or evaluator source. Observed statuses and
outputs match direct runs; a one-line control source measured 207 pairs and
produced the identical 3,058-byte receipt as the uninstrumented run. All runs
below are macOS arm64 against the canonical 147,840-byte compiler closure,
the 2,998-byte support section, and the retired profile's 8,575-byte
evaluator tape.

Each admitted extent was driven to its retired-profile boundary with the
shape that maximizes cumulative pairs for that extent. Sources below are
named by content; digests and generators match the
[resource-boundary](../../../../tests/delta/resource-boundary/README.md)
fixture families except where stated. Arena share is of the retired
40,265,318-pair arena.

| Profile extent | Worst-shape source | Bytes | Outcome | Cumulative pairs | Arena share |
| --- | --- | ---: | --- | ---: | ---: |
| request/source bytes | one 4,000,000-byte identifier | 4,000,047 | publish | 12,000,333 | 29.8% |
| syntax-arena bytes | balanced `(+ …)` tree, 129,000 nodes | 774,045 | payload refusal | 30,960,310 | 76.9% |
| type rows | 65,536 nominal types | 1,507,329 | reject | 4,902,112 | 12.2% |
| constructor rows | 65,536 constructors | 720,953 | reject | 2,320,158 | 5.8% |
| function rows | 32,768 functions | 720,923 | reject | 2,241,502 | 5.6% |
| active environment rows | 65,536 generated bindings | 852,006 | publish | 5,552,375 | 13.8% |
| coverage rows | 65,536 match arms | 1,310,762 | reject | 5,033,374 | 12.5% |
| expression parse depth | 1,020 nested `let`s | 17,281 | publish | 155,567 | 0.4% |
| payload bytes | exact 16,777,212-byte receipt | 172,478 | publish | 2,974,113 | 7.4% |
| capture aggregate | 65,535-field reconstruction | 1,704,033 | payload refusal | 36,033,367 | 89.5% |
| real customer | Epsilon closure + canonical entry | 628,304 | publish | 1,864,697 | 4.6% |

The extracted rates are linear in their drivers: each checked `+` node costs
240 pairs before publication — and another 69 in publication traversal when
the receipt fits — and each name byte costs three pairs across its census,
resolution, and typing events. The syntax ledger charges exactly 22 pairs per
`(+ x y)` node — ten list pairs (three open-frame, three node, one
parent-spine, three child-spine) plus twelve atom pairs — and is the binding
cap on arithmetic density: 129,000 `+`
nodes complete the ledger while 131,071 refuse at resource 7. Name bytes are
nearly ledger-free — an atom is four pairs regardless of length — so the
pair-maximizing corner of the admitted extent box composes ledger-maximal
arithmetic with byte-maximal name text in one request. Count-mode
serialization allocates no pairs, so a payload refusal contributes nothing;
the spend must cross the arena before publication to exhaust.

That corner was measured directly. A 4,194,288-byte admitted source — one
3,420,227-byte identifier declaration followed by a 129,000-node arithmetic
`main`, SHA-256
`0b588a5376cefaecd8b7556d61464f857f0326df248607f8b6bc0285d605d89a`,
inside every authored-row, depth,
source-extent, and syntax-ledger provision — drove the cursor to exactly
40,265,318 pairs and ended in halt 252 with empty stdout: the evaluator's
`application_heap_failure` path, not a DCOUT row and not a generated
application's resource exhaustion. A second corner source — the same
identifier plus a helper function whose 129,000-node tree takes a bound
`Int` reference at every leaf (SHA-256
`b531678b2e1fc236a6df7c90a289f4ba532b8748274de5c32473f96a15b04e12`) —
halted identically at the same cursor. A tighter-margin sibling (118,000
nodes plus a 3,486,227-byte identifier) completed all producer phases and
refused only at payload count, measuring 38,779,117 pairs — confirming the
additive rates and showing the boundary sits between the two compositions.

The finding was therefore negative for that selected profile: admitted
Delta sources existed whose compilation allocated past the immutable-pair
arena, and the boundary contract has no resource row for cumulative pair
allocation — the [arithmetic probe](README.md#arithmetic-allocation-probe)
separately rules out inventing a general DCOUT heap code. The evaluator has
since adopted the 3,422,453,760-pair extent. The
[`delta-compiler-pair-arena-profile` rule](../../../MINIMIZATION.md#private-capacity-changes)
delegates further coherent capacity tuning without owner approval; complete
containment and customer validation remain engineering obligations. The old
exhaustion does not establish a fundamental limit of the selected design.
Keep the per-occurrence accounting above, and escalate only if feasible
reprovisioning cannot preserve the contract or a changed observation/trust
guarantee is proposed. Raw status 252 remains an evaluator failure, not DCOUT.

Generated Delta applications are different programs. Their recursion and live
storage can still exhaust the selected evaluator or diverge. The compiler's
fixed-source bounds above must not be applied to those executions, the Epsilon
evaluator, or an arbitrary Gamma source.
