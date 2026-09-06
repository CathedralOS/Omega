# Gamma evaluator profile

## Request

The evaluator consumes one exact byte stream:

```text
0..4   little-endian u32 Gamma source length
4..    exact Gamma source bytes
...    all remaining bytes as sealed input
```

The complete request is capped at 16 MiB. Source accepts HT, LF, CR, and
printable ASCII only.

## Observation

Every `write` is buffered inside the evaluator. A scalar `main` preserves the
Gamma transformer convention: its value must be `0..255`, is appended to the
buffer, the complete buffer is published, and status zero is returned.

An application source begins with the exact first declaration
`(def $application () Int 1)` and returns `(pair status publish)` from `main`.
The validated marker selects application failure ownership before execution.
Both result fields must be scalar, `status` must be `0..254`, and `publish` must
be zero or one. Status zero requires publication. A nonzero published result
requires a nonempty buffer, making publication reconstructible from the process
observation. A discarded result must have nonzero status. The evaluator
validates the complete result before either flushing or discarding the buffer.

Before a valid application result is returned, evaluator-owned statuses are:

| Status | Meaning |
| ---: | --- |
| 0 | Complete scalar transformer. |
| 1 | Invalid request or Gamma source. |
| 2 | Authored trap while evaluating reached code. |
| 3 | Incomplete bounded storage. |
| 4 | Internal evaluator contradiction. |

Evaluator failures expose no stdout. A valid application result selects its own
declared status. Status-zero stdout, including empty stdout, and nonempty
nonzero stdout are authoritative publication; empty nonzero stdout is a
discarded outcome. Invocation plumbing applies only that generic predicate and
does not decode application statuses or bytes.

After a validated application marker, evaluator failures map to the generic
generated-program block: internal or unclassified evaluator failure is 248,
authored trap is 249, value/environment/call-stack exhaustion is 250, immutable
pair exhaustion is 252, request extent is 253, and buffered-output extent is
254. Status 251 is reserved for a detected memory-containment violation; no
checked evaluator path currently produces it. Invalid Gamma source remains
status 1 before application execution.

## Private partition

The current evaluator uses these Alpha memory regions:

```text
0x00100000..0x01100000   request bytes
0x01200000..0x01300000   function rows and lookup index
0x01300000..0x01500000   lexical environment rows
0x01500000..0x01d00000   temporary value stack
0x01e00000..0x01f00000   function activation rows
0x01f00000..0x02000000   nested-call context rows
0x0e000000..0x0efffffc   buffered output bytes
0x10000000..0x70000000   immutable pair nodes
```

The selected AlphaBootstrapV4 realization provides 1.75 GiB of memory. The Alpha
tape occupies low memory and the hidden Alpha call stack still grows down
from `0x10000000`. Pairs grow upward from that boundary, without overlapping
the stack or moving the output buffer. The former pair region
`0x02000000..0x0e000000` is unused. Every evaluator-owned extent is preflighted
before use; the hidden Alpha stack is discharged by the containment argument
below.

## Exact capacities

All counts below belong to this evaluator profile, not to Gamma's language
semantics. An operation that would exceed one of them halts with status 3
before the offending store or write. Equality with an end address is admitted;
only an extent beyond the end is refused.

| Resource | Retained representation | Maximum |
| --- | --- | ---: |
| complete request | bytes at `0x00100000` | 16,777,216 bytes |
| function census | five-word rows, with an explicit logical cap | 4,096 functions |
| active lexical environment | four-word `(name span, value, kind)` rows | 65,536 bindings |
| temporary values and arguments | two-word `(value, kind)` entries | 524,288 values |
| nested expression lists | evaluator recursion, prechecked during census | 255 lists |
| nested call contexts | three-word rows, slot zero reserved | 256 contexts |
| active function frames | six-word rows | 257 reachable frames |
| immutable pairs | five-word `(marker, left, left kind, right, right kind)` nodes | 40,265,318 pairs |
| buffered output | bytes published after result validation | 16,777,212 bytes |

The request maximum includes its four-byte length. Consequently a source with
no sealed input is at most 16,777,212 bytes. An exact-size request receives one
EOF probe and proceeds; one additional byte selects status 3 before framing.
A declared source extent outside the retained request is malformed framing and
selects status 1 instead.

Function and environment insertion preflight the next count before deriving a
row address. The value stack preflights the complete 16-byte next entry. Pair
allocation preflights the complete 40-byte node; its arena has 16 unusable tail
bytes after the maximum whole-node count. Output preflights each buffered byte.
A scalar transformer may emit at most 16,777,211 bytes with `write` before its
final byte. An application result may publish all 16,777,212 buffered bytes.

The 4,096 five-word function rows occupy `0x01200000..0x01228000` in
authored declaration order. A separate sorted index of 4,096 eight-byte row
pointers occupies `0x01228000..0x01230000`, inside the same function partition.
Lookup compares name length and then exact name bytes by binary search; it
does not change declaration order, the retained `main` row, or first-declaration
application-marker ownership. Census checks duplicates before the existing
function-count preflight. After admitting and completing a row, insertion
shifts only initialized index entries and stores its pointer at an index no
greater than 4,095. Lookup reads only the initialized prefix. There is no new
capacity, source representation, or AST.

The 256-context cap is stronger than the physical function-frame arena: `main`
owns one frame and each live non-tail context can own one more, so at most 257
frames are reachable. Proper-tail calls release their temporary context and
reuse the current frame. The physical frame-end check remains defensive and
also selects status 3, but cannot be the first exhausted resource under this
profile.

## Containment argument

Declaration census bounds every expression body to 255 nested lists before
validation or evaluation. Validation recurses only over that bounded syntax;
it does not enter called bodies. During evaluation, ordinary call depth is
bounded by the 256 live contexts, while tail `if`, tail `let`, and tail calls
loop or reuse the current activation.

Function-index search, comparison, and insertion use bounded loops, not
recursive calls. The evaluator's Alpha call graph has no other recursive cycle.
A conservative bound of 16 live Alpha return addresses per expression level
per active Gamma frame, plus 512 fixed helper slots, is
`16 * 256 * 257 + 512 = 1,053,184` return addresses, or 8,425,472 bytes. The
output buffer stops at `0x0efffffc`, leaving 16,777,220 bytes below Alpha's
initial `0x10000000` stack pointer. The hidden stack therefore remains more than
8 MiB above every lower-memory allocation even at the conservative bound.
The pair arena starts at the initial stack pointer and grows upward, while
every live return address is strictly below it. Its complete-node preflight
keeps all pair stores below Alpha's `0x70000000` memory end. Pair projection
requires the retained pair kind, an allocated aligned node in this same upper
arena, and its marker; integer values do not acquire pointer provenance by
matching a relocated address.

Every other memory access is based on one of the preflighted extents above, a
source/input span within the retained request, a fixed table inside the tape,
or a fixed-size row selected by an already bounded count. Runtime `/` and `%`
precheck zero and `INT64_MIN / -1`, translating both Alpha arithmetic traps to
status 2. Thus no admitted evaluator execution relies on Alpha's undefined
out-of-range memory behavior or exposes an Alpha arithmetic trap as a Gamma
outcome. Gamma has no time or fuel bound; a nonterminating program diverges.

The selected implementation is
[`evaluator/gamma_evaluator.beta`](evaluator/gamma_evaluator.beta), a 1,632-line,
46,482-byte addressed Beta program assembling to an 8,355-byte Alpha tape. Its
current SHA-256 identities are:

```text
Beta source  9ccd93e07a3baa00bba34133e91d15df6f3cc4d670688d06ff3febf82b304904
Alpha tape   f08544faee5ee3a7aa5969f17004fa708326c38f9fb8ab27dfa9c97cb44ac2e8
```

Proper-tail execution, static validation of unreachable bodies, exact resource
outcomes, bounded output, profile-owned arithmetic traps, and provenance-tagged
immutable-pair allocation are implemented. Buffered scalar transformation and
generic application publication share this one selected evaluator. The gate
pins exact source
and tape identities plus exact/adjacent function, syntax-depth, and call-context
boundaries; the fixed identity makes the remaining arithmetic extent arguments
above reviewable against one immutable subject rather than a host model.

The separate [heap-boundary gate](../../tests/gamma/heap-boundary/README.md)
executes ordinary Gamma allocation loops at the exact whole-node maximum and
one beyond it, under scalar and application publication. It also crosses the
previous 20,132,659-pair ceiling. This profile increase is driven by the actual
complete-D parser customer: the prior evaluator exhausted its immutable heap
with raw application status 252 before publishing the twelve-invocation result.
It does not change Gamma meaning, allocate an unbounded heap, or convert an
outer evaluator failure into an Epsilon observation.
