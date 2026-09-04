# Delta abstraction-boundary experiment

This experiment asks whether the 8,733-line Delta-authored Epsilon compiler is
large because Delta is missing reusable mechanisms, or because Epsilon owns a
large amount of irreducible language and diagnostic policy.

The analyzer parses complete top-level Delta forms from the current compiler and
measures five candidate families. Reported ceilings are deliberately generous:
they assume every identified declaration and helper disappears and charge zero
lines for the corresponding Delta language/compiler implementation.

| Candidate | Exact current family | Free-feature ceiling | Result |
| --- | --- | ---: | --- |
| Generic option/result | 8 optional declarations plus 25 parse outcomes | 99 lines | Reject for now |
| Generic immutable list | 24 ordinary lists, 22 template reverses, 3 template counts | 245 exact lines | Reject standalone elaboration |
| Generic catalog/map | 8 catalog result types plus 11 lookup traversals | 245 lines | Reject generic map |
| Source span wrapper | 29 start/end helpers | 164 lines | Reject wrapper alone |
| Candidate minimum fold | 3 candidate types plus 6 merge helpers | 77 lines | Reject generic fold |

Even the impossible combined ceiling is only 844 lines, 9.7% of the Epsilon
evaluator. The five proposals therefore cannot explain most of the source-size
explosion.

## Generic sums

Current option payloads have both `0/1` and `0/3` constructor arities. Parse
outcomes have `1/2` and `2/2` shapes. A single `Option<T>` or `Result<T,E>` is
insufficient without generic product/record payloads. Constructor matches and
phase-specific rejection logic remain at every use site.

The declaration-only ceiling is 96 lines. Supporting this requires parametric
types, generic type checking, instantiation or uniform representation, and
recursive generic references in the Gamma-authored Delta compiler. The
hypothetical syntax is retained in `generic_option.delta-plus` and must reject
under current monomorphic Delta. This does not earn that language expansion.

## Generic lists

This was the strongest candidate. Twenty-four declarations have the ordinary
`Empty | More(item, tail)` shape; the excluded trie has a three-field node. Of
the 23 reverse functions, 22 are the exact list template and one reverses a
four-list control ledger. Three count functions are exact templates. The real
replaceable family is therefore 49 forms, 245 lines, and 11,409 bytes.

`list_elaborator.gamma` implements a complete two-pass source transformation
for this derived form:

```text
(list Type Empty More Element reverse_name count_name)
```

`_` omits either helper. One pass emits every data declaration; the second emits
helpers and ordinary definitions, preserving Delta's required top-level order.
The 24-line Epsilon family specification expands to 49 forms that are
alpha-equivalent to the existing declarations and helpers. A
smoke program then passes through the ordinary selected Delta compiler and
executes generated reverse/count functions with result 2.

The measured authored cost loses:

```text
explicit Epsilon family          245 lines / 11,409 bytes
Gamma list elaborator            292 lines / 13,200 bytes
derived Epsilon specifications    24 lines / 3,474 bytes
derived route total              316 lines / 16,674 bytes
net                               +71 lines / +5,265 bytes
```

The standalone pass also introduces another transformation relation. The
trie-backed global census is fast; remaining whole-source rescans own the
continued cost. Development timings are diagnostic, not semantics.

A fused implementation could reuse Delta's scanner, but it must cost fewer than
221 Gamma lines merely to tie raw line count, before charging proof complexity
or the greater audit weight of lower-rung code. Direct virtual-list support must
also modify type, constructor, arity, match, and helper-function resolution. The
standalone implementation is therefore a favorable lower bound, not an unfairly
expensive generic system. Reopen only if another independently justified Delta
feature supplies most of that machinery for free.

## Catalog/map

Catalog results have four different constructor shapes: `0/1`, `0/2`, `0/1/2`,
and `0/2/2`. Traversals use unqualified, owner-qualified, nested-member, and
state-local keys. Their payloads preserve fields versus cases, boundary versus
data owners, owner custody, and shape facts.

A generic `Map<K,V>` would require generic key equality, heterogeneous value
sums, construction/update semantics, and either hashing/tree ordering or an
explicit bounded linear representation. Most lookup policy would remain. The
206-line ceiling is therefore unattainable. If scale later fails, Epsilon should
first test a source-owned indexed catalog; Delta should not acquire a generic map
from this evidence.

## Source spans

Two executable kernels compare repeated `(start, end)` fields with a nominal
`SourceSpan` wrapper under current Delta:

```text
                         Lines  Source bytes  Gamma receipt bytes
flat fields                 15           597                1,235
nominal SourceSpan          19           759                1,153
```

Both produce result 11. Wrapping saves 82 generated bytes but costs four authored
lines and 162 source bytes because current Delta must destructure the span through
ordinary functions. A span type alone moves representation complexity without
improving source auditability. Reopen only with a separately justified record
field/projection facility.

## Candidate minimum

Census chooses the left candidate on equal offsets. Type formation creates a
conflict when equal-offset kinds differ. Final diagnostics union reason sets on
equal offsets. The equality behavior is policy, not generic mechanism.

An executable kernel extracting a common three-way `offset_order` helper grows
from 54 to 59 lines, from 2,533 to 2,770 source bytes, and from 3,837 to 4,050
Gamma receipt bytes. Both produce result 33. Keep the three phase-specific folds
explicit.

## Verdict

The Epsilon compiler is certainly verbose, but all five tested abstractions fail
their current earned-feature test. They either save too little, require larger
Delta semantics, move representation complexity, or erase Epsilon-specific
policy that should remain visible.

The language explosion appears to be mostly real Epsilon work: parsing its full
surface, exact diagnostic ordering, identity custody, type formation, resolution,
control checking, and Alpha encoding. Future abstraction proposals should use
the same test: exact family accounting, a free-feature ceiling, implementation
cost below that ceiling, and an executable source-level win.
