# Bounded byte input

This is the settled library and boundary contract for bounded line input, not
a claim of complete compiler or provider support. [Bounded growth](bounded_growth.md)
owns container invariants; [boundary calls](../terminal-psi/boundary_calls.md)
owns exact provider identity, calls, and result custody.

## Destination and ownership

Input fills a caller-supplied mutable byte slice. Its existing length is the
maximum writable extent, not a request to grow storage. An ordinary fixed array
may lend its full range; its length remains fixed. The operation neither resizes
an allocation nor changes the slice extent or an owner's live-length metadata.
There is no new output descriptor, resizable-container primitive, or implicit
allocator selection.

The caller supplies valid writable storage and the required exclusive access
for the invocation's lifetime. The provider must obey that range and the
declared write and completion contract. Memory accessibility checks by an OS
do not establish the allocation's language-level extent. A selected external
provider retains its ordinary admission obligations; caller bounds proofs do
not prove that an external implementation respects them.

On normal return, the reported prefix contains exactly the newly read bytes,
in order. The operation writes only that prefix and leaves the remaining
destination unchanged. The result's bound refers to the exact supplied slice,
not another buffer or a hidden capacity. A caller may borrow the reported prefix
or explicitly update its own container length under that owner's contract.
An ordinary slice exposes existing initialized elements, not spare capacity or
uninitialized storage. Any owner-specific writable-window operation must
independently establish its storage, initialization, and lifetime obligations.

## Line result and zero initialization

The library result in `source/library/std/console.omg` has this shape; native
provider and complete caller support remain implementation obligations:

```omega
data LineReadResult {
    case Invalid;
    case LineComplete(count: u64);
    case EndOfInput(count: u64);
    case Full(count: u64);
}
```

`Invalid` is the first, ZII-valid case. It means no completed read result is
established, not EOF, a successful empty line, or a recoverable OS error. Every
normal return from `read_line` establishes one of the other three cases. Their
counts satisfy `count <= destination.len` and bind the written prefix to that
invocation. Constructing a same-shaped result independently establishes no I/O
observation. Non-Unit results retain ordinary strict-use and case checking.

## Greedy stopping rules

`read_line` fills the destination from the logical byte stream until the first
applicable completion below. It may wait for further input; it is not a
nonblocking query for currently available bytes.

| Condition | Result and input consumption |
| --- | --- |
| Destination length is zero | `Full(0)`, without reading. |
| LF is read while room remains | Store LF and return `LineComplete(count)`, including LF in the positive count. |
| EOF is observed while room remains | `EndOfInput(count)`, retaining any partial final line; zero means observed EOF without bytes. |
| Destination fills without LF | `Full(count)`, where count equals destination length; do not read another byte to discover what follows. |

LF in the last available position produces `LineComplete`, not `Full`.
Reaching capacity before observing EOF produces `Full`, even when EOF would be
the next observation. `Full` means line completion has not been established;
it does not prove that the line is too long. The next call resumes the remaining
input rather than discarding or draining it. Repeated calls with larger or reused
storage can assemble a line under ordinary checked length and resource rules.

Only byte `0x0A` is the delimiter. Preserve bytes exactly, including NUL, CR,
and LF. There is no terminator byte added beyond the input, no CRLF normalization,
silent truncation, or lossy character conversion. A full prefix may end inside a
multibyte encoding. A text wrapper may strip delimiters or normalize explicitly.

The result does not add an I/O-error case. The adapter preserves the selected
byte-input operation's declared failure contract; the existing hosted byte leaf
traps on failed reads. Such failures must be represented honestly through
admission and caller crash coverage, not hidden by `Invalid`. A recoverable
I/O-result boundary is a separate contract. Failure after input consumption
does not imply rollback of the stream or destination.

## Library composition and target realization

Use shared checked library code over bounded byte input, initially the existing
`read_byte` leaf. Target-specific implementations may satisfy the same line
contract directly if they preserve its byte sequence, stopping reason, logical
input consumption, bounds, and operational envelope under ordinary provider
verification and admission. A richer target does not redefine portable meaning.

POSIX `read(fd, buffer, count)` on Linux/macOS and Windows
`ReadFile(handle, buffer, count, ...)` accept a maximum byte count. They need
not return a complete requested range or a line. Short reads alone are not EOF;
the adapter interprets each target's actual status and continues as necessary.
Terminal modes and redirected input do not justify assuming one OS call has
the portable line semantics. Native console encoding conversions, when used,
belong to an explicitly selected text contract rather than this raw-byte API.

Byte-at-a-time input avoids surplus read-ahead but is not a performance mandate.
A buffered provider must retain bytes read beyond a delimiter or completed
prefix for subsequent logical reads. It needs explicit storage and stream
custody; surplus bytes cannot be discarded or made unavailable when switching
operations. No hidden allocation or repeated external effect after suspension
is permitted. Blocking/suspension and progress premises compose normally: finite
destination size bounds successful appends, not how long an input operation waits.

## Encoding and higher-level owners

UTF-8 is an imported library domain with checked recognition and preservation,
not a compiler-known predicate spelling or a per-sample capacity-specific
declaration. Predicate-only membership requires proof, not a privileged minter.
Empty bytes satisfy UTF-8, but arbitrary input mutation does not preserve that
fact. Validate the exact written prefix before qualifying it; neither the result
tag nor the rest of the destination establishes its encoding. See
[encoding domains](../language/domains.md#byte-containers-and-encoding-domains).

Keep the raw destination unqualified when its contents need not remain UTF-8.
Higher-level text APIs may return validated views or explicit encoding failures.
Resizable containers, growth, owner-length updates, and allocator authority
belong to ordinary wrappers, not the low-level reader. A bounded destination
needs no advance proof that the complete input line fits: the checked full-buffer
outcome prevents out-of-range writes.

Size queries are optional source-specific protocols, not a prerequisite for
reading. Available-byte counts are hints unless exact item identity and stability
are guaranteed. Discovering a stream line's length can require consumption and
explicit buffering. A stronger query-then-read guarantee needs a stable item or
reservation; it cannot silently replace bounded-read outcomes.
