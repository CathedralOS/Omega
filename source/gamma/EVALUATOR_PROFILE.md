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
from `0x10000000`. The evaluator checks its explicit request, function,
environment, value-stack, activation, and nested-call limits where currently
implemented.

The selected implementation is
[`evaluator/gamma_evaluator.beta`](evaluator/gamma_evaluator.beta), a 1,314-line
addressed Beta program assembling to a 6,880-byte Alpha tape. Its current
SHA-256 identities are:

```text
Beta source  1b943437dece551712ae3a1406dff27804b4004f090613d8343c39fa833b14b2
Alpha tape   69c2247323d226799ba9c5ee5697240d1b67a6fcc35173b545b04a2802429a83
```

A complete capacity proof remains required before this profile can be admitted.
Proper-tail execution, static validation of unreachable bodies, bounded output,
and provenance-tagged immutable-pair allocation are implemented and pinned by
selected gates.
