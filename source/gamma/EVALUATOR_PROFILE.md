# Gamma evaluator profile v1

This document fixes the first Beta-authored implementation of Gamma. The
trusted Beta compiler translates its source into platform-independent Alpha
tape. This profile defines request framing, observable implementation outcomes,
resource bounds, and private representation constraints. It does not extend
the Gamma language defined by [`LANGUAGE.md`](LANGUAGE.md).

## Request

The evaluator receives one exact-ended request:

```text
0..8    [47 41 4D 4D 41 52 45 51 01]  (`GAMMAREQ`, version 1)
9..12   Gamma source length, little-endian u32
13..16  sealed input length, little-endian u32
17..    exact Gamma source bytes, then exact sealed input bytes, then EOF
```

The evaluator checks both lengths and its fixed memory profile before reading
either body. Paths, filenames, caller-selected profiles, capacities, and other
host facts never enter the request. Source and input remain immutable in
disjoint fixed regions for the complete invocation.

## Memory

The evaluator selects this `AlphaBootstrapV2` memory map:

```text
0x00000000..0x00100000  evaluator tape, 1 MiB
0x00100000..0x00900000  GAMMAREQ source plus input, 8 MiB
0x00900000..0x01100000  declaration tables, 8 MiB
0x01100000..0x02100000  reusable evaluator stack, 16 MiB
0x02100000..0x0E000000  immutable value arena, 191 MiB
0x0E000000..0x10000000  reserved Alpha hidden call stack, 32 MiB
```

Every extent is half-open and checked before cursor advancement. The request's
source and input lengths must fit together in their region. The evaluator does
not borrow unused capacity across boundaries. Changing an extent requires an
explicit evaluator-profile revision and renewed tape audit.

## Outcomes

The complete terminal status set is:

```text
status 0  Complete: stdout is the exact returned Bytes
status 1  InvalidRequestOrSource
status 2  AuthoredTrapOrReject
status 3  Incomplete
status 4  InternalFailure
```

There is no failure frame, stable reason vocabulary, source coordinate, limit
report, or human-readable diagnostic in the audited evaluator. Development
tools may provide richer diagnostics without entering Gamma meaning or artifact
authority.

An authored arithmetic, bytes, value-kind, name, arity, local-binding, or
wrong-family match trap yields status 2. Returning `Reject` also yields status
2. Malformed private evaluator state or a returned value outside the reserved
outcome family yields status 4. Exhausting a fixed source, input, declaration,
frame, value-arena, or output extent yields status 3 and makes no Gamma judgment.
The output extent is exactly `AlphaBootstrapV2`'s 1,048,572-byte raw tape
maximum. Every status-0 result is therefore directly stampable into an audited
Alpha seed; profile v1 admits no separate larger tool-output class.

The evaluator has no fuel or call-count policy. Every primitive traversal is
finite over already bounded source or values; recursive execution may diverge.
A host timeout may stop a development invocation, but it produces no evaluator
result and supplies no semantic or trust premise.

## Artifact publication

`Complete` traverses its immutable byte rope once, validating each private node
and the running output extent while immediately writing logical bytes. A late
invalid node or exceeded extent may leave an arbitrary stdout prefix before
status 3 or 4. That prefix is an honest execution observation but is never an
artifact.

The canonical artifact boundary accepts stdout if and only if the evaluator
returns status 0. Invocation plumbing writes stdout to a temporary destination,
atomically publishes it only after status 0, and discards it after every other
status. This status check does not interpret Gamma values or compiler
diagnostics. Parsing, validation, and ordinary evaluation perform no output.

No magic suffix, sentinel bytes, or in-band panic sequence distinguishes
failure. Alpha tapes and compiler output may contain arbitrary bytes, and an
unreachable suffix cannot authenticate completeness.

## Private representation

The evaluator retains exact source bytes rather than constructing an AST,
token array, or bound occurrence graph. A complete structural pass records only
fixed-capacity function, constructor-family, constructor, and entry rows
containing source `(start, length)` spans and required arities/body coordinates.
Reserved forms dispatch directly. Global and local names resolve by source-
order linear scan, length comparison, and exact source-byte equality. The
initial evaluator uses no hash, fingerprint, cache, interning table, or pre-
resolved occurrence.

Private table and arena identities are one-based. Declaration row zero,
constructor-family zero, and arena handle zero mean absent or unresolved. Value
tag zero means uninitialized or invalid. A `Bytes` value combines its explicit
value tag with a private handle; handle zero is canonical empty `Bytes`, so it
cannot be confused with `Int(0)`.

The profile represents nonempty `Bytes` as immutable literal/input leaves,
single-byte leaves, concatenation ropes, and slices/views. Concatenation does
not copy either operand, slicing retains a logical view, and all traversal is
iterative over the reusable evaluator stack.

Persistent constructor values and byte rope/view nodes use one invocation-local
bounded bump arena with no reclamation or garbage collector. Parameters, `let`
bindings, pattern bindings, pending primitive work, and non-tail continuations
use a separate reusable bounded stack. A non-tail call pushes its continuation;
a return pops it; a tail call replaces the current function frame without
growing the stack. These mutable regions are private evaluator mechanics and
are not Gamma values or source mutation.

The trusted Beta compiler compiles `evaluator/gamma_evaluator.beta` to the
Gamma evaluator tape. Exact Beta compiler reconstruction and byte-identical
evaluator rebuilding bind the readable source to the executed Alpha program;
the evaluator tape is not separately admitted as opaque bytecode.