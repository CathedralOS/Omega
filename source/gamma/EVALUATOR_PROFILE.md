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

Status meanings are:

| Status | Meaning |
| ---: | --- |
| 0 | Complete; stdout is the authoritative artifact. |
| 1 | Invalid request or Gamma source. |
| 2 | Authored trap while evaluating reached code. |
| 3 | Incomplete bounded storage. |
| 4 | Internal evaluator contradiction. |

Only status-zero output is an artifact. Invocation plumbing must discard output
from every nonzero status. The evaluator writes immediately, so a late trap can
leave bytes on the process stream before plumbing discards them.

## Private partition

The current evaluator uses these Alpha memory regions:

```text
0x00100000..0x01100000   request bytes
0x01200000..0x01300000   function rows
0x01300000..0x01500000   lexical environment rows
0x01500000..0x01d00000   temporary value stack
0x01e00000..0x01f00000   function activation rows
0x01f00000..0x02000000   nested-call context rows
0x02000000..0x0e000000   immutable pair nodes
```

The Alpha tape occupies low memory and the hidden Alpha call stack grows down
from `0x10000000`. Every evaluator-owned extent is preflighted before use; the
hidden Alpha stack is discharged by the containment argument below.

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
| immutable pairs | five-word `(marker, left, left kind, right, right kind)` nodes | 5,033,164 pairs |
| successful output | bytes written to stdout | 67,108,864 bytes |

The request maximum includes its four-byte length. Consequently a source with
no sealed input is at most 16,777,212 bytes. An exact-size request receives one
EOF probe and proceeds; one additional byte selects status 3 before framing.
A declared source extent outside the retained request is malformed framing and
selects status 1 instead.

Function and environment insertion preflight the next count before deriving a
row address. The value stack preflights the complete 16-byte next entry. Pair
allocation preflights the complete 40-byte node; its arena has 32 unusable tail
bytes after the maximum whole-node count. Output preflights each byte, including
the final byte returned by `main`. A successful computation may therefore emit
at most 67,108,863 bytes with `write` before `main` appends the last byte.

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

The evaluator's Alpha call graph has no other recursive cycle. A conservative
bound of 16 live Alpha return addresses per expression level per active Gamma
frame, plus 512 fixed helper slots, is
`16 * 256 * 257 + 512 = 1,053,184` return addresses, or 8,425,472 bytes. The
pair heap stops at `0x0e000000`, leaving 33,554,432 bytes below Alpha's initial
`0x10000000` stack pointer. The hidden stack therefore remains more than 25 MiB
above every evaluator-owned allocation even at the conservative bound.

Every other memory access is based on one of the preflighted extents above, a
source/input span within the retained request, a fixed table inside the tape,
or a fixed-size row selected by an already bounded count. Runtime `/` and `%`
precheck zero and `INT64_MIN / -1`, translating both Alpha arithmetic traps to
status 2. Thus no admitted evaluator execution relies on Alpha's undefined
out-of-range memory behavior or exposes an Alpha arithmetic trap as a Gamma
outcome. Gamma has no time or fuel bound; a nonterminating program diverges.

The selected implementation is
[`evaluator/gamma_evaluator.beta`](evaluator/gamma_evaluator.beta), a 1,325-line
addressed Beta program assembling to a 6,934-byte Alpha tape. Its current
SHA-256 identities are:

```text
Beta source  02618b10cf275e82b11821d7dfb0bb3bd2120410677bf4bcd2b6b555aa0d5e54
Alpha tape   a72bc962c99eb5cec80ffd9246d19c8e7bee3c23229c47e25d846f1080d0cac2
```

Proper-tail execution, static validation of unreachable bodies, exact resource
outcomes, bounded output, profile-owned arithmetic traps, and provenance-tagged
immutable-pair allocation are implemented. The selected gate pins exact source
and tape identities plus exact/adjacent function, syntax-depth, and call-context
boundaries; the fixed identity makes the remaining arithmetic extent arguments
above reviewable against one immutable subject rather than a host model.
