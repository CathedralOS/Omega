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
[`evaluator/gamma_evaluator.beta`](evaluator/gamma_evaluator.beta), a 1,472-line
addressed Beta program assembling to an 8,119-byte Alpha tape. Its current
SHA-256 identities are:

```text
Beta source  8a32405571e939f90d3c6c562561bd389bcee998b0c1f4b397e14a8a9c97aeb4
Alpha tape   a2d79be0de063d409683b3bbdf1d27cba9d5b845dbbfe0635b5029e0ff729ef2
```

A complete capacity proof remains required before this profile can be admitted.
Proper-tail execution, static validation of unreachable bodies, bounded output,
and immutable-pair allocation are implemented and pinned by selected gates.
