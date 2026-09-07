# Bootstrap cost review

Initial measurements use `59e73ffef81b550c0eac4946b0e40970dc90942b` on macOS arm64;
follow-through revisions are identified below.
This is engineering evidence, not an owner ruling or admitted proof artifact.
The objective remains the [smallest human-auditable chain](bootstrap_minimization.md).

## Findings and disposition

| Area | Evidence | Recommendation |
| --- | --- | --- |
| Delta height normalization | Current Epsilon customer lowers to height 69; normalization adds no helpers. | Freeze expansion. Do not raise Gamma depth for this customer. Resolve deep-source conformance before deleting the transform. |
| Delta name construction | Shared-prefix cursors save 275,352 pairs/11,014,080 bytes over root-rebuilding seeks on the current Epsilon subject; both produce the same complete receipt. | Retain the existing implementation without expansion. Customer capacity alone does not require it; preserve separate conformance obligations before considering removal. |
| Generic Gamma checker | Capacity checks at 547,817 work; the normalized forward state-fold recipe costs another 681,724 even with identity byte steps. | Separate capacity and scanner passes do not fit the current provision. Cost the combined encoder and a coherent resource provision before adopting either representation. |
| Partial Beta theory | Lexical and word helpers exist; full encoder, independent owner root, and source-owned certificate producer do not. | Do not treat helper completion as full-proof feasibility or trust reduction. |
| Retained concatenative route | Eleven bootstrap files, 236,347 bytes; eight remaining test entrypoints after experiment retirement below. | Candidate for further retirement, not dead code. Map unique coverage and preserve useful findings before coordinated removal. |

The resource follow-through below changes one private checker limit after an
allocation audit; no language contract, evaluator capacity, or trusted assumption
changes. Diagnostics are feasibility evidence, not an accepted artifact.

## Metrics that determine the next step

1. Exact remaining trusted programs, definitions, and admissions; which named
   obligation a completed certificate would remove, and which it would not.
2. Human audit burden across the whole route: source, semantic cases, state,
   invariants, proof rules, resource arguments, and permanent validation.
3. Actual next-compiler demand, with exact input identities and owned outcomes.
4. Full-certificate bytes, retained state and cumulative allocation, depth,
   checking time, and producer cost. Distinguish measured peaks from bounds.

File/line totals are inventory, not a proxy for correctness or audit difficulty.
Passing small equations does not establish any of these whole-chain results.

## Delta: current customer versus conformance envelope

The existing source-owned
[normalization diagnostic](../../tests/delta/normalization/normalization_driver.gamma)
was composed with the selected Delta implementation manifest. Its 157,030 bytes
match the committed diagnostic pin. Input was the exact 87-member Epsilon closure
(610,428 bytes), one LF, and the existing 2,565-byte
[execution entry](../../tests/epsilon/interpreted-omega-experiment/execution_driver.delta).
The combined 612,994-byte subject has SHA-256
`251a97366c26c4356e4c573e353f8998d3c7d26985bc6052deafc4fe28a43f94`.
Standard Gamma framing and the unchanged selected evaluator produced status 0,
empty stderr, and these five u32 observations plus the scalar terminator:

```text
authored definitions / maximum lowered Gamma height: 724 / 69
normalized definitions / maximum Gamma height:       724 / 69
maximum generated-helper parameters:                0
stdout hex: d402000045000000d4020000450000000000000000
elapsed: 86.747 seconds
```

This establishes customer demand, not complete receipt equivalence or Epsilon
execution correctness. A separate host-only source census found 720 authored
Epsilon functions and maximum Delta expression depth 23; the source-owned lowered
height 69 is the relevant result. Neither measurement covers future customers.

The normalizer owns ten Gamma files: 303 lines/14,326 bytes. Its README and five
gate files add 408 lines/22,121 bytes. Its obligations include capture identity,
scope, fresh parameter mapping, evaluation/trap order, tail position, height
termination, refreshed emission extents, and generated-helper resources.
Those obligations, rather than 303 lines alone, are the cost of retaining it.

The selected Delta profile still admits 1,024-level expressions, and existing deep
controls use this transform. Deleting it now would leave that contract uncovered.
The smaller alternative is **not currently to enlarge Gamma**: its published
[containment argument](../../bootstrap/2_gamma/EVALUATOR_PROFILE.md#containment-argument)
allows at most 508 lists in the existing hidden-stack gap with unchanged context
bounds. A 511-list provision needs 16,846,848 bytes versus 16,777,220 available.
A larger coherent layout or tighter argument is possible, but has its own audit
cost and no demonstrated need from this customer.

Next useful simplification evidence: compare complete canonical emission with a
scratch source-owned path serializing the original plan for this exact customer.
Do not remove shared height/extent metadata: emission also consumes it. Any
contract narrowing remains an owner decision, not a local optimization.

## Delta names: customer allocation versus boundary necessity

At `fe0b48924aebc7bd4818f1a75d870f98ba17e8b4`, the complete canonical Delta
compiler compiled the same 612,994-byte Epsilon-plus-entry subject used above.
That subject includes one separating LF; it is not the unseparated byte sequence
in the ordinary Epsilon gate. The untouched selected evaluator returned status
0, empty stderr, and a 711,597-byte Gamma receipt in 103.129 seconds, SHA-256
`f28040ac9c4bee83e6487fe604c0e93a84265db0e92e430d9f2677c759784e5a`.

A scratch comparison changed only `identity_cursor_seek` to finish its previous
root before descending from depth zero:

```text
(def identity_cursor_seek ((cursor Int) (start Int) (end Int)) Int
  (identity_cursor_descend (identity_cursor_finish cursor) 0 0 start end))
```

The sparse trie, insertion/replacement, admission order, caller state, and other
definitions stayed unchanged. This disables shared-prefix reuse; it is not a
complete cursor-free compiler. The composed variant was 155,284 bytes, SHA-256
`ade7b4124f8057110e195dc1111234554e3f054cb1698ad31a5a9a1d80b02e70`.

| Whole-compilation observation | Current prefix reuse | Root-rebuilding seek |
| --- | ---: | ---: |
| Cumulative allocated pairs | 2,242,373 | 2,517,725 |
| Allocated pair storage, 40 bytes/node | 89,694,920 bytes | 100,709,000 bytes |
| Instrumented wall time, one run | 105.654 s | 108.347 s |

Both instrumented receipts matched the untouched evaluator's bytes exactly.
Telemetry came from a scratch Beta evaluator: the two publication calls invoke
an appended routine at `0x20a3`, which calls the original `flush_output`, then
writes `(ra0 - 0x10000000) / 40` as eight little-endian bytes, preserving status
register `r4`. The existing monotonic pair cursor and allocation checks are
unchanged. Its 8,441-byte tape has SHA-256
`af2790b6aec4d5e062dd38a05bc3da8f9b41691d2712b72fba81ffb33251183d`.
Five controls checked zero/one/three allocations and scalar/application
publication, including nonzero status. Counts measure allocated pair storage,
not live reachable values or host RSS; the instrumented evaluator is not admitted.

**Disposition:** the existing three cursor files (123 lines/5,879 bytes) avoid
10.9% of the alternative's allocations on this actual customer. Both remain far
below Gamma's 40,265,318-pair provision. These single-run times do not establish
a reliable throughput improvement. Retain the current implementation, freeze
expansion, and do not describe it as necessary merely to fit this customer.
Full constructor/parameter/match limits remain separate requirements, including
the [65,536-arm controls](../../tests/delta/resource-boundary/README.md#match-coverage-fixture-inventory).
The historical `36bdfe173c` capacity repair changed several mechanisms together;
it is not an isolated measurement of cursor necessity. No new name index,
representation, language, or private-capacity change follows from this review.

On macOS arm64, both compiler variants also passed the existing cursor,
parameter, and catalog-replacement controls: 14 exact diagnostic rejections and
seven identical positive receipts that preserved binary input when executed by
selected Gamma. Full resource-boundary suites and Windows were not run. This
establishes the measured compilation and focused controls, not complete Epsilon
execution or proof-chain closure. Scratch variants and telemetry are not retained
as permanent tooling; selected source, artifacts, and test expectations stay unchanged.

## Checker: full-size representation is only a prerequisite

Run `sh tests/gamma/beta-encoding-theory/run.sh --subject-shape`.
The following observations used the original 262,144-unit work provision;
the later capacity follow-through below uses the larger current profile.
It uses the unchanged 63,504-byte checker diagnostic and 93,140-byte source-emitted
partial theory. The subject is the entire 46,484-byte evaluator Beta source and
8,355-byte Alpha tape, with their existing profile identities printed by the gate.
Source and tape retain separate list roots. All byte values share 256 constructor
rows and one Nil; duplicate witness list spines force structural traversal rather
than same-reference shortcuts. Both supplied proof rows are checked, even though
only tape reflexivity is the final diagnostic root.

| Observed positive request | Result |
| --- | ---: |
| Request bytes | 2,729,608 |
| Owner / witness ground rows | 55,096 / 54,839 |
| Proof rows | 2 |
| Cumulative checking work | 219,369 of 262,144 |
| Wall time | 229.060 seconds |

The companion request changes the witness source's first byte and must reject
at the first proof row's right field. It returned tag 1, code 12, coordinate
2,729,588, zero limit/requested, in 125.442 seconds. Both invocations returned
process status 0 and empty stderr; the complete owned bytes determined the
verdict. Windows execution was unavailable. This is scoped diagnostic evidence,
not a fresh whole-checker conformance run.

This does **not** prove Beta encoding, authenticate the owner, or prove evaluator
correctness. It shows that the current representation fits this preliminary
whole-subject check and that duplicate endpoint traversal is expensive.
No runtime allocation peak was instrumented. The then-published ledger gives the
conservative bound `2*23284 + 3*109935 + 32*8 + 26 + 96*219369 + 128`
= 21,436,207 cumulative pairs for this request, not measured allocation or RSS.

There is a concrete sharing opportunity without new rules. Using the same
validated root identities would reduce these two reflexivity rows to 13 work
units, by the existing accounting, but ground admission would still traverse the
input. That is a derived comparison, not a measured full encoding certificate.
Similarly, the raw source has 76 distinct byte values across 46,484 occurrences.
Repeated `source_byte` unfolding per occurrence would cost 3,581,951 work under
the current literal recipe; one closed equation per distinct value costs 6,477.
These are source-derived recipe costs (`sum(byte+7)+5`), excluding traversal,
state transitions, and composition. Reuse helps; it does not prove the missing
encoder will fit. Raising the work provision alone also changes its cumulative
allocation argument and cannot be assessed from a timeout alone.

The missing dependency sequence remains:

```text
complete error-valued Beta definitions
  -> independently constructed exact source/tape/limits proposition
  -> source-owned untrusted producer with shared terms and closed equalities
  -> complete checked encoding certificate and artifact custody
```

Definitions must cover malformed input and limits as well as successful opcodes,
operands, trivia, assertions, and EOF. The owner must fix the theory and proposition
independently; the producer cannot choose them. All of this is additional to the
current generic checker (1,365 Gamma lines/62,349 bytes) and partial theory emitter
(435 Gamma lines/21,305 bytes), excluding their docs and tests.
Even a successful encoding certificate leaves the evaluator's implementation of
Gamma trusted, as the [checker contract](../architecture/bootstrap_chain/proof_kernel.md)
states. It is not a proof of the whole compiler chain.

Before another helper family, specify the state/recursion and proof-composition
route for a complete encoder and cost a representative contiguous sequence,
including a word operand, trivia, an assertion, and state/endpoint composition.
Reuse existing ground identities and closed facts first. Report extrapolation
separately from complete-subject measurement. If this cannot plausibly improve
the total audit burden, return for a design decision instead of expanding it.

## Counter strategy: sharing does not establish feasibility

A follow-up at `e128a74dbba34b651ac7c36e782e66ec4a0c3eeb` tests the concrete
proposal to maintain a Word source position by proving one existing
`word_successor` equation per byte. Run
`sh tests/gamma/beta-encoding-theory/run.sh --counter-cost`.
This reuses the existing diagnostic recipe, globally interns identical ground
terms, and shares identical proof rows after remapping their premises. It does
not search for shorter proofs, interpret the theory, or implement a Beta encoder.

| Constructed consecutive increments | Ground rows | Proof rows | Work | Request bytes |
| --- | ---: | ---: | ---: | ---: |
| 1 | 15 | 15 | 266 | 93,988 |
| 24 | 268 | 314 | 6,637 | 108,616 |
| 257 | 2,834 | 3,350 | 130,186 | 257,116 |
| 46,484: one per raw source byte | 373,550 | 513,467 | 11,204,359 | 25,285,216 |

The first three requests are checked by the unchanged Gamma implementation,
with exact row/work observations in 3.157, 4.863, and 25.661 seconds on macOS
arm64. Changing the final endpoint of the 257-increment request rejects with
tag 1, code 12, coordinate 257,104 and zero limit/requested in 25.932 seconds.
All four invocations return process status 0 and empty stderr; the complete
owned observation determines the verdict. Windows execution was unavailable.
The full-sized table is **constructed only**:
it exceeds both the checker's 8 MiB input and Gamma's 16 MiB enclosing frame.
Its work is derived from the existing recipe ledger, not an executed verdict.
Before sharing, the same full-sized recipe would retain 790,878 term rows,
977,793 proof rows, and 25,098,413 work. Sharing materially helps but still leaves
source counting alone at about 43 times the current checking-work provision.
Neither table includes a proof that these numbers count the source, a scanner,
operand parsing, output construction, assertions, or the final encoding root.
This is a cost for this recipe, not a lower bound on every possible certificate.

The 24-byte literal example is:

```text
imm r0 0x1 ; x
0xa:
ret
```

It includes a word operand, comment, exact address assertion, and final LF/EOF.
Its expected tape from the written Beta rules is opcode 1, register 0, the eight
little-endian bytes of 1, then opcode 20: eleven bytes. This is a hand-derived
example, not a checked encoding result. The probe establishes only the cost of
its source-position increments; token handling and their connection to those
eleven output bytes are deliberately still missing. Small-example success must
not conceal the full-subject failure of the proposed counter strategy.

### Candidate complete construction and remaining decision

The complete transparent encoder needs three ordinary-definition phases:

1. Validate the entire source envelope and its capacity, including comment bytes.
   This cannot be delegated to an unchecked producer-supplied length. The
   per-byte Word-successor recipe above is not a viable baseline under the current
   profile. A grouped/binary length derivation is an engineering alternative to
   cost before adding helpers; a larger coherent profile is another, not a
   change to source semantics. Neither alternative is established by this probe.
2. Consume the immediate source tail in one structurally decreasing scanner.
   Its state distinguishes trivia, comments, an unfinished token, expected item
   or operand, emitted-byte count, output accumulator, and failure. Completed
   tokens select only the closed mnemonic/operand table, `dw`, or an assertion.
   EOF must finish the last token and reject missing operands. Failure values
   remain explicit; no helper-returned cursor substitutes for structural decrease.
3. Finalize an error-or-success value, exact output ordering and provision, and
   full exhaustion. Assertions compare against emitted length at their precise
   position. The owner fixes the complete `encode_Beta(S, limits) = Success(T)`
   root from the exact raw bytes and independently fixed definitions.

For a forward state-fold formulation, one byte step would unfold the scanner,
use a closed state-transition equality, rewrite its recursive-call argument by
congruence, and compose with transitivity. Shared source suffix identities avoid
copying source lists; shared byte classifications and closed arithmetic facts
avoid repeating those derivations. Equalities still have to establish every
transition and finalization. This specifies the dependency/recursion route, not
an implemented or fully costed theory. Output ordering and resource/error
precedence must be audited against Beta before promoting particular equations.

**Disposition:** do not extend the encoder around per-byte Word counter proofs.
The source-capacity follow-through below costs cheaper alternatives; their
composition with this complete scanner route still needs a costed contiguous
example. An 8 MiB limit increase alone cannot fit the per-byte recipe; Gamma
framing and the cumulative-allocation argument also need consideration. No new
trusted arithmetic, assembler primitive, unchecked length, chunk-level budget
reset, or narrowed root follows from this engineering result. The complete
certificate remains open, and its auditability advantage remains unproven.

### Binary trees: smaller representation and a coherent work provision

A scratch measurement at `c6b832590d7c36a2f9ded6a49ccf6180c9f0d37c`
uses the same complete source/tape subjects and unchanged 63,504-byte checker.
Split each nonempty byte sequence longer than one byte at the largest power of
two strictly smaller than its length; recursively intern identical subtrees.
Empty and individual bytes are leaves. This operates on raw bytes, not Beta
tokens or trusted producer-supplied chunk boundaries.

With a scratch free-tree vocabulary (256 byte constructors, Empty, binary Join),
the two-Ref comparison request has 15,636 owner terms and 15,379 duplicate witness
joins. It checks in 65.243 seconds with 61,529 work units and 745,508 request bytes.
Changing a witness byte reference rejects with tag 1, code 12, coordinate 745,488,
zero limit/requested, in 33.536 seconds. The final root is still tape reflexivity,
not encoding. The vocabulary is only 3,120 bytes, versus the partial Beta theory's
93,140; the whole input reduction cannot be attributed to trees alone. Term
storage accounts for 1,894,080 bytes of the reduction. This vocabulary is not a
replacement Beta theory.

A second scratch vocabulary separates Byte, Source, and Shape sorts and defines
one structurally recursive function:

```text
shape(Empty) = EmptyShape
shape(Leaf(byte)) = Unit
shape(Join(left, right)) = ShapeJoin(shape(left), shape(right))
```

Both recursive arguments are unchanged immediate constructor children, as the
existing calculus requires. Each distinct leaf uses one Unfold row; each distinct
join uses Unfold, binary Cong, and Trans, sharing child equalities. The owner-section
diagnostic root is `shape(source) = expected_shape`. This connects an equation to
the raw source structure, but proves neither its capacity nor Beta encoding.
The typed vocabulary is 3,376 bytes. The complete source contains 76 distinct byte
leaves and 11,865 distinct joins, height 16, and only 23 distinct erased shapes.

| Shape-fold subject bytes | Request bytes | Ground terms | Proof rows | Work |
| --- | ---: | ---: | ---: | ---: |
| 0 | 3,500 | 3 | 1 | 12 |
| 1 | 3,520 | 4 | 1 | 13 |
| 2 (`ab`) | 3,760 | 11 | 5 | 66 |
| 4 (`abab`) | 3,924 | 15 | 8 | 111 |
| 256 (all byte values) | 58,792 | 1,542 | 1,021 | 13,528 |
| 46,484 (complete source; constructed cost) | 1,670,848 | 35,846 | 35,671 | 534,538 |

The first five check with exactly these row/work observations; the 256-byte
case takes 4.711 seconds. Changing the two-byte proof's final endpoint to Unit
rejects at coordinate 3,748 with code 12 and zero limit/requested. The full request
was also executed: it returns owned Incomplete (tag 2, code 4, coordinate
1,203,828, limit 262,144, requested 262,145) in 111.570 seconds. Its 534,538 work
is the recipe ledger, not an observed successful check: each distinct nonempty
leaf costs eight units including proof indexing, each join 45, plus five setup
units. All observed invocations return process status 0 and empty stderr on macOS
arm64; Windows was not run. No allocation peak was measured.

Applying the then-published conservative ledger to the full fold gives
`7,864,346 + 534,538*96 + 128 = 59,180,122` pairs, above Gamma's 40,265,318-pair
provision. The follow-through audited all comparison/substitution branches:
memo insertions are paired with cheaper traversal transitions, so charging each
unit as a worst-case insertion overstates cumulative allocation. The
[amortized argument](../../bootstrap/2_gamma/derivation_checker/COMPARISON.md#amortized-allocation-argument)
bounds allocation at 48 pairs per unit plus a fixed allowance covering unfinished
traversals and terminal failures. Raising work to 655,360 therefore bounds the
entire request at 39,321,754 pairs without enlarging Gamma's arena.

With only that production constant changed, the same 1,670,848-byte shape request
checks successfully: 35,671 proof rows, exactly 534,538 work, in 208.152 seconds.
The five smaller folds and changed two-byte endpoint control also retain their
exact observations. The 3,376-byte emitted theory retains SHA-256
`18dfb59785f754692d47256f57a18746f646c0753a9026c54fb4010687488c52`;
the checker diagnostic remains 63,504 bytes, now SHA-256
`8c91eefacbbd4517fa58e83da2fc96e35adb005bed6fc0f2296858ceb46537ce`.
The full fold ran alongside regression checks on macOS arm64; this is not a
controlled timing comparison or measured allocation peak. No Windows run is claimed.

**Disposition:** the structural-fold resource obstacle is resolved; tree sharing
still needs a full encoder cost case before adoption. A balanced tree's depth
alone is not a source-length proof: capacity
requires a checked relation to its full shape and the owner's exact bytes.
The numeric bound is costed below; the ordered tree scanner remains open. No checker rule,
production representation, or trusted boundary changed; the
temporary emitters and fixed proof recipes are not retained as a new framework.

### Exact source capacity: compare the simpler alternative too

At `12090b9803e8c91732f94f18f8582874068cd2ca`, a scratch ordinary-Gamma emitter
extends the preceding vocabulary with binary naturals, Boolean/order constructors,
and transparent arithmetic. `Zero`, `Bit0(n)`, and `Bit1(n)` denote zero, twice
`n`, and twice `n` plus one. Addition and comparison recurse on the selected first
operand's immediate child; the second operand may become its half. Comparison
treats redundant zero forms numerically, not as identical constructor trees.
No arithmetic operation or new rule is added to the checker.

The first route defines `size(EmptyShape)=0`, `size(Unit)=1`, and
`size(ShapeJoin(a,b))=add(size(a),size(b))`. It explicitly lifts the checked
`shape(S)` equation through size, numeric comparison, and a Boolean result:

```text
leq(compare(size(shape(S)), binary(0x4000000))) = True
```

This counts every raw byte occurrence, including trivia and repeated references
to a shared subtree. It is valid for arbitrary finite trees, not just balanced
ones. There is no unchecked length or height premise. Arithmetic facts share
across the current source's 23 erased shapes.

The simpler alternative directly defines source byte count: Empty is zero,
Leaf is one, and Join adds its child counts. It shares the same closed arithmetic
facts without an intermediate Shape sort. Its raw joins need four linking rows
instead of three: 53 work units including indexing versus 45. Leaf constants
cost 11 versus eight. Counting 11,865 distinct joins and 76 byte leaves therefore
adds 95,148 raw-fold units, offset partly by removing the separate shape-size
derivation. Simpler definitions do not imply a smaller certificate.

| Complete 46,484-byte source-capacity request | Via shapes: checked | Direct: checked |
| --- | ---: | ---: |
| Request bytes | 1,718,316 | 1,999,560 |
| Ground terms | 36,635 | 36,566 |
| Proof rows | 36,887 | 48,661 |
| Cumulative work | 547,817 | 641,773 |
| Work left under 655,360 | 107,543 | 13,587 |

Both complete requests check at exactly their predicted counts: 205.491 seconds
via shapes and 246.402 seconds directly. The shape route's
5,620-byte emitted theory has SHA-256
`11b8a4704e37a8163ebdf3f52f95bb477711b97c63d87df641fb454abecb1884`.
The direct comparison vocabulary is 5,836 bytes, SHA-256
`c60a231d46c3e0370b8460ba51d1f9baba128fed049d04ee505b99bdf03a7e27`;
it deliberately retains the unused shape definitions for this comparison, so
these bytes are not a minimized direct-only theory. Both use the unchanged
checker identity from the preceding subsection and one cumulative request budget.
Changing only the shape route's owner root to a 46,483-byte bound, without
regenerating proof rows, rejects with code 12 at coordinate 1,718,404 in
216.053 seconds (1,718,420 request bytes). The old certificate cannot establish
the changed proposition.

Controls checked exact raw-source bounds for lengths zero, one, two, four, and
256; 64 arithmetic pairs included redundant zeros, carries, and unequal widths.
A height-100 unbalanced shape counts exactly 101 leaves. A 27-node shared shape
representing `2^26` occurrences fits the exact Beta source bound; appending one
leaf proves False, and changing that proof to claim True rejects with code 12
at coordinate 43,060. Two raw bytes against a one-byte bound also prove False
through both routes; changing the direct proof to claim True rejects at 7,928.
Storage-node counts cannot substitute for represented byte occurrences.

**Disposition:** numeric capacity has a feasible checked route. The direct route
removes a representation relationship; the shape route saves 93,956 work units
and 281,244 request bytes in these unminimized diagnostics. Select neither from
file count or limit fit alone. Cost textual-ASCII validation and the contiguous
scanner/operand/assertion example together before retaining a production route.
The existing erasure loses byte values and cannot prove ASCII validity; any
validity-preserving erasure needs new checked equations that retain invalid-byte
positions and the envelope-before-tokenization behavior. Complete Beta encoding,
independent production root reconstruction, and the source-owned certificate
producer remain missing. These scratch recipes are not production authority or
a new retained framework. Measurements are macOS arm64 with overlapping checks,
not controlled timing comparisons or allocation peaks; Windows was not run.

### Forward state threading: reject the recipe's budget premise

At `06b76d3eb68963c4e4c8ed8903b686ebc921fc34`, a temporary ordinary-Gamma
emitter isolates traversal from byte semantics with this total definition:

```text
scan(Empty, state) = state
scan(Leaf(byte), state) = state
scan(Join(left, right), state) = scan(right, scan(left, state))
```

The two recursive Source arguments are unchanged immediate children; changing
the state argument is legal under the existing calculus. The scratch State sort
has one nullary constructor, Unit. This deliberately does **no scanning**: it
proves neither byte classification, tokenization, encoding, nor capacity. The
diagnostic root is only `scan(S, Unit) = Unit`, using the entire raw subject and
the same binary splitting/interning rule as above. Identity byte steps permit
maximal normalized-state reuse for this recipe, so a resource failure is useful
before authoring real scanner helpers.

Each distinct Leaf uses one Unfold (10 units including proof indexing). Each
distinct Join uses Unfold (26), binary Cong (11), two Trans (14), and four
indexing units: 55 total. Cong rewrites `scan(right, scan(left, Unit))` to
`scan(right, Unit)` using the left equality and a shared Ref for the right
Source. Each distinct right-source Ref adds four units including indexing;
request setup and final-root comparisons add five.

| Raw subject | Request bytes | Terms | Proof rows | Recipe work |
| --- | ---: | ---: | ---: | ---: |
| Empty | 3,464 | 3 | 1 | 14 |
| `a` | 3,484 | 4 | 1 | 15 |
| `ab` | 3,748 | 10 | 7 | 84 |
| `abab` | 3,932 | 13 | 12 | 143 |
| All 256 byte values | 70,804 | 1,534 | 1,531 | 17,610 |
| Complete 46,484-byte source | 2,116,340 | 35,824 | 54,632 | 681,724 |

The source has 11,865 distinct joins, 76 distinct leaves, and 7,096 distinct
right-source references, giving `55*11865 + 10*76 + 4*7096 + 5 = 681724`.
This is a constructed recipe cost, not an observed successful full check.
The first five requests check at exactly the stated row/work counts; the
256-byte case takes 5.733 seconds. Changing the two-byte proof's final endpoint
to its initial scan application rejects with code 12 at coordinate 3,736.
The emitted theory is 3,336 bytes, SHA-256
`6822f1a2766b177986e92ccacb23d8a3564350fa278a74e648681cbaf517387f`;
the checker diagnostic retains the preceding subsection's identity.

The complete request returns owned Incomplete (tag 2, code 4, coordinate
2,063,720, limit 655,360, requested 655,361) in 233.215 seconds. Thus the actual
checker confirms exhaustion, not success at the constructed 681,724 cost. All
listed observations return process status zero with empty stderr on macOS
arm64. The initial scratch emitter attempt returned raw evaluator status 250,
not a checker verdict: its 256-step non-tail constructor-writing loop exceeded
the call-context provision once its callers were included. Making that loop
tail-recursive fixed the emitter without changing any provision.
No allocation peak or Windows runtime result is claimed.

This is **not a lower bound for all certificates**. A different fixed recipe
can unfold the right subtree with the symbolic state `scan(left, state)`, then
compose two Trans without Cong or Ref. Its local Join cost is 43, but memoization
now needs both source and state identities. A raw-term census on this subject
finds 46,483 Join instances and 46,484 Leaf instances: 2,463,614 work with five
setup/final units. That alternative was counted, not checker-executed; other
mixed or fused recipes are not ruled out.

**Disposition:** do not promote the normalized forward-fold recipe as fitting
the present whole-request budget. Capacity via shapes leaves only 107,543 units;
the scanner's Join rows alone require 652,575. Sharing physical source terms
does not share equality rows headed by different functions. Adding the separate
diagnostic totals is not an exact combined-certificate cost: setup, shared
facts, root composition, and a fused definition may change it. Nevertheless,
these separate-pass recipes cannot fit together unchanged.

Before another permanent helper family, cost a contiguous real encoder example
with operands, trivia, assertions, and endpoints, and estimate **one combined**
ASCII/capacity/encoding request. Compare fused traversal against separate passes
under a coherent larger work/input/allocation provision; the private limits
are adjustable, not language laws. Raising work merely enough for this no-op
fold would not establish encoding feasibility. Keep the complete error-valued
root, raw-byte coverage, and cumulative budget intact. This measurement adds
no production definition, checker rule, runtime provision, or retained probe
framework. Complete encoding and its source-owned producer remain unimplemented.

## Legacy route: retirement must follow its consumers

`bootstrap/2_gamma/bootstrap/concatenative/` and
`bootstrap/3_delta/bootstrap/concatenative-compiler/` retain four language-source/
receipt files totaling 6,658 lines/195,831 bytes, one 26,674-byte tape, and six docs.
These are outside the selected execution route; their size is maintenance and
review-discovery cost, not automatically additional trusted chain material.

The six remaining Gamma entrypoints are `evaluator-slice.sh`, `evaluator-reconstruction.sh`,
`compiler-fixed-point.sh`, `state-machine-customer.sh`, and the `run.sh` files in
`gamma1-augmentation-experiment/` and `gamma-to-beta-experiment/`.
The two remaining Delta entrypoints are the `run.sh` files in `compiler-slice/`
and `state-machine-experiment/`.
The role registry, two artifact helpers, and
chain-hygiene gate also retain the old route. Searches must include helper calls
and role variables, not only literal directory names.

Their [retention inventories](../../tests/delta/README.md) include historical
architecture discriminators and conformance evidence. Some old comparisons
explicitly lack current typing, arithmetic, and Bytes coverage, so their line
counts do not justify replacing the selected implementation. Audit each unique
assertion against selected-route gates; keep useful findings in existing design
documentation and retire obsolete execution paths together with their adapters
and hygiene requirements. Do not preserve them merely to make inventory checks
pass, or delete them while leaving their consumers broken.

### Completed topology discriminators

The direct-Beta feasibility and direct-Delta evaluator experiments meet their
documented retirement conditions. D88 selected the implemented direct functional
Gamma evaluator; D92 rejected moving Delta's recursive-data machinery into its
Beta trust root. Those ratified records and the selected evaluator's
[measurement discussion](../../tests/gamma/evaluator-development/README.md)
preserve the reasons: generated-call expansion was expensive, but removing Gamma
by enlarging the low-level evaluator did not reduce matched audit cost.

The retired directories are `tests/delta/direct-beta-feasibility-experiment/`
and `tests/delta/direct-beta-evaluator-experiment/`: six files, 2,430 lines,
62,896 bytes, including the 2,019-line symbolic direct-Delta prototype.
Their source, measurement assertions, and reconstruction recipes remain
recoverable from Git at `0ac643b7237ec7d667dff4868c3fc8ebee542e83`.
They are not accepted artifacts or sources used by another executable gate.

| Former obligation | Retained evidence or selected-route coverage |
| --- | --- |
| Generated tokenizer/call/helper size comparison | D88 and the selected evaluator measurement discussion retain the topology finding; old generated-code sizes are not selected-language conformance. |
| Exact Nat, List, and recursive rope observations | `tests/delta/staged-compiler/{recursive_match,list_match,bytes_rope}.{delta,gamma}` and `run.sh` compile, compare exact receipts, and execute results. |
| 100,000-node proper-tail construction/traversal | `proper_tail` in that same selected gate exercises tail calls through `if`, `let`, and `match`. |
| 3,001-function witness | `stress` in that gate retains the same source and result 199. |
| Unknown nominal field and missing match arm | The selected gate's `malformed` cases reject without emitted bytes. |
| Rejection of reordered exhaustive arms | Not retained: it contradicts current Delta. D110 and the selected `reordered_match` case require acceptance and correct dispatch. |
| Prototype tape/source hashes and old evaluator size pins | Identify an unselected prototype, not a selected-chain obligation. They remain historical Git data. |

At the retirement review's base revision above, a focused selected-route run
compiled and executed all six positive programs in this mapping and rejected
the two malformed sources. The three stored receipts remained exact; no legacy
compiler, host translator, or direct-Delta prototype participated. This is not
a full Delta conformance claim. The existing selected Gamma augmentation gate
also reproduced its exact receipt and result 42. Checks ran on macOS arm64;
Windows execution was unavailable. The selected source, artifacts, and permanent
tests were not changed to make retirement pass.

At that checkpoint, no shared resolver, concatenative compiler, artifact helper,
fixture, or hygiene requirement was removed: their other consumers remained.
This retired completed comparison work without claiming the complete
Gamma-to-Delta edge had closed or every retained experiment was disposable.

### Forth experiment

The owner subsequently directed retirement of
`tests/delta/forth-gamma-experiment/`, rather than extending it into a statically
checked replacement language. D93/D94 retain the comparison and its limitations:
the smaller interpreter came with 555 compiler words, absent whole-program
name/stack-effect checks, and a 3,001-function case exceeding 600 seconds.
Those historical decisions are unchanged; the prototype is no longer maintained.

The eight experiment files and their now-unused
`tests/gamma/evaluator-development/resolve.py` total 3,006 lines/101,739 bytes.
They are recoverable at `284559298ec7dc0a7c3de029026918e112dfc197`.
A repository-wide consumer search found no remaining executable dependency on
these files or symbolic Beta inputs. Shared fixtures, the concatenative route,
artifact helpers, and hygiene requirements still have other consumers and stay.

Nat/List/rope, 100,000-node tails, and function-scale behavior map to the selected
controls above (the selected scale case uses 3,001 functions, versus Forth's 301).
The selected `staged-compiler/run.sh` also owns missing payload argument/binder
rejections and empty-rope traps. The Forth-only `value`/`text` rules, its acceptance
of unreachable unknown words, and its private receipt/size pins retire with the
dialect. Reordered exhaustive arms must remain accepted under current Delta;
the contrary Forth rejection is not a regression to preserve.

At the Forth retirement base above, focused macOS arm64 checks rejected all four
mapped malformed sources without output, reproduced the exact rope receipt and
result `B`, compiled an empty-rope lookup that trapped without output, and ran
the former 301-function source to 99 through selected Gamma/Delta. Earlier
selected-route tail and 3,001-function results above have unchanged inputs.
Chain hygiene and remaining-consumer checks passed; Windows was unavailable.
No selected compiler, evaluator, artifact, or permanent control changed.

### Scalar compiler experiments

`tests/delta/functional-compiler-experiment/` and
`tests/delta/streaming-compiler-experiment/` no longer need executable prototypes.
D80/D82 preserve the direct-Alpha versus scalar-elaboration comparison; D88
selected direct functional Gamma after measuring the streaming compiler's
call expansion and manually managed state. Declaration-only retention and
validation by rescan avoided a universal expression table, but the 193-line
Gamma1 lowerer raised the authored streaming route to 859 lines without removing
its manual frame protocol. Extending that prototype no longer distinguishes the
architecture choice. The selected compiler now covers its scalar customers.

Six prototype/compiler/gate/documentation files totaling 67,322 bytes are removed,
recoverable at `a8b46e48e498259f83cbf0757320378b7ddbaefa`. The shared 202-byte
recursive and 951-byte scalar-surface fixtures move unchanged to
`tests/delta/staged-compiler/`. Its gate now compiles and executes both, covering
recursion, all seven operators, lexical binding, forward/nested calls, and
thirteen ordered parameters. The Gamma evaluator and remaining legacy comparison
read the moved fixtures; neither reads a retired compiler implementation.

Existing selected malformed controls own missing main, duplicate functions,
unknown locals, call arity, result-type disagreement, and initializer scope.
The old rejection of fourteen parameters is not retained: Delta has arbitrary
arity. Direct-Alpha halt statuses, experiment tape/receipt hashes, and agreement
between two executions of an obsolete compiler are not selected-route contracts.
The Gamma1 lowerer and concatenative tools remain used by other gates; their
retirement still requires a separate consumer review.

Focused macOS arm64 validation ran the newly registered selected-gate loop,
six malformed sources, and an admitted fourteen-argument call yielding 14.
The selected Gamma evaluator also ran the moved recursion fixture directly.
All three remaining fixture bindings resolve to byte-identical pre-move sources;
shell syntax, consumer searches, documentation links, and chain hygiene were
checked. Full legacy/capacity suites and Windows execution were not rerun for
these fixture moves; production source and artifact inputs are unchanged.
