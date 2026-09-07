# Gamma evaluator profile v2

This document fixes the first Beta-authored implementation of concatenative
Gamma. The trusted Beta compiler translates its source into platform-independent
Alpha tape. This profile does not extend [`LANGUAGE.md`](LANGUAGE.md).

## Request

The evaluator receives one exact-ended request:

```text
0..3   Gamma source length, little-endian u32
4..    exact Gamma source bytes, then all remaining bytes as sealed input
```

Source and input remain immutable for the complete invocation. Paths, caller-
selected capacities, filenames, clocks, environment, and other host facts never
enter the request.

## Memory

The evaluator selects this `AlphaBootstrapV2` partition:

```text
0x00000000..0x00100000  evaluator tape, 1 MiB
0x00100000..0x00900000  request, 8 MiB
0x00900000..0x01100000  definition rows, 8 MiB
0x01100000..0x02100000  data stack, 16 MiB
0x02100000..0x0C000000  fixed cells, 159 MiB
0x0C000000..0x0D000000  word-continuation stack, 16 MiB
0x0D000000..0x0E000000  reserved, 16 MiB
0x0E000000..0x10000000  Alpha hidden call stack, 32 MiB
```

Every extent is half-open and checked before access. There are at most 262,144
32-byte definition rows, 2,097,152 simultaneous data-stack words, 20,840,448
addressable cells, and 1,048,576 retained 16-byte word continuations. The
request's four-byte source length and both bodies must fit its region. Regions
never borrow spare capacity.

## Outcomes

```text
status 0  Complete: stdout is the emitted bytes
status 1  InvalidRequestOrSource
status 2  AuthoredTrap
status 3  Incomplete
status 4  InternalFailure
```

Invalid textual source, malformed top-level definitions, duplicate/builtin
names, absent `main`, and missing reached `jump`/`branch` operands produce
status 1. Reached unknown tokens or targets, stack underflow, input/cell/byte bounds,
failed assertions, and division traps produce status 2. Exhausting a fixed
request, definition, stack, continuation, cell, or output extent produces
status 3. Impossible private state produces status 4.

Gamma has no fuel policy. Infinite execution remains divergence. A host timeout
produces no evaluator result or semantic evidence.

## Artifact publication

`output-byte` and `output-word` write immediately. A later trap or exhaustion may
therefore leave an arbitrary stdout prefix. That prefix is an honest execution
observation but is never an artifact.

Invocation plumbing publishes stdout if and only if the evaluator returns
status zero. It writes to a temporary destination, atomically publishes after
success, and removes the temporary destination after every other status. The
successful generic output ceiling is 64 MiB (`0x4000000`). Gamma customers may
emit readable lower-language source as well as Alpha candidates. A compiler
that claims an Alpha artifact must independently enforce Alpha's exact
16,777,212-byte raw-tape maximum before publication.

## Private representation

The evaluator retains source bytes and records one 32-byte row per definition:
name start, name length, body start, and body end. Exact source-order linear
scans resolve names; one NUL-row table resolves builtins. There is no AST, token
array, hash, cache, interning, pre-resolved occurrence, or generated threaded
code.

Data-stack words and cells are raw 64-bit bit patterns. Definition rows and
continuations contain private memory coordinates that are never Gamma values.
`output-word` stores one raw word at `0x0D000000` in the reserved region and
emits its eight byte-addressed contents; no Gamma operation can name that address.
`jump` and `branch` replace the current body span directly. Ordinary calls push
only the caller cursor and body end. All other evaluator control uses bounded
Alpha subroutines whose maximum nesting is independent of Gamma execution.

The trusted Beta compiler compiles `evaluator/gamma_evaluator.beta`. Exact Beta
compiler reconstruction and byte-identical evaluator rebuilding bind readable
source to execution; the evaluator tape is not separately admitted as opaque
bytecode.