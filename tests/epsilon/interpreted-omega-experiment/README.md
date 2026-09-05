# Interpreted Omega bootstrap experiment

This gate tests where Alpha tape production should resume after the selected
Gamma evaluator begins interpreting higher compiler sources.

The selected experiment keeps Epsilon execution interpreted and makes
`alpha_bootstrap` an ordinary Omega product target:

```text
Delta-written Epsilon evaluator + exact Epsilon-written Omega D
  -> interpreted Omega compiler D
  -> Omega C for alpha_bootstrap
  -> omega compiler Alpha tape
```

The gate requires the Omega product build to bind exactly one
`alpha_bootstrap::ProgramEntry`, requires Omega D to retain its Alpha tape
construction, and rejects any `EpsilonAlpha`/`epsilon_alpha_` backend residue in
the Delta-written Epsilon implementation. The evaluator is currently 11,732
lines / 597,945 bytes, authored in 84 explicitly manifested members.
The complete gate checks 141 ordinary fixtures, five D customers, and seven
framing controls against the exact reconstructed evaluator receipt.

The executable slice runs the current checking pipeline, locates `Main::main`,
and executes an empty entry, scalar `let` and local/parameter assignment,
zero-initialized scalar receiver
fields and fixed arrays of `i32` or `u8`, scalar and indexed assignment/read,
grouped/local scalar reads, `assert`, all scalar operators, unary negation,
equality/order comparisons, and direct
`Console.write_byte` and `Console.exit_process` statements with scalar expression
arguments. Locals, machine parameters, and state parameters update only their established checked
declaration home; right sides evaluate against the old values, without changing
other bindings. Console argument traps precede the write's byte-range check or
the process exit. It preserves output
before exit, overflow, `ByteRange`, and `Assertion` traps; non-Boolean assertions
trap separately. Indexed access evaluates the receiver field before its index,
uses zero-initialized per-index homes, and traps as `Bounds` before a read or
right-side evaluation. Byte reads zero-extend to `i32`. Byte stores check
`0..255` after evaluating the right side and before committing any update;
out-of-range values trap as `ByteRange`, without truncation or wrapping.
Entry and states share a block evaluator. Scalar transitions consume checked
subject/pattern identities and execute only the selected continuation. State
arguments evaluate against the old locals, then install their bindings
simultaneously; transfers discard old block locals and preserve current machine
parameter values and receiver storage. A tail-driven invocation loop resumes states without retaining the
previous block's call frame. Resultless return, state falloff, and supported
Console write/exit continuations are executable. Unqualified machines and
receiver methods support nested and recursive calls, scalar and aggregate
parameters/returns, and resultless return/falloff. Callee locals are separate from caller locals;
committed field and output effects return with the result. The expression engine
threads those effects through left-to-right operands, arguments, index checks,
right sides, and returns, retaining exact trap/exit prefixes. Only the outer
entry adapter maps ordinary completion to process exit zero.
Stored records and fixed arrays use instance-specific places, including nested
fields and array elements. Receiver calls update their selected place; ordinary
record/array locals, parameters, returns, and state arguments are copied values.
An argument snapshot is taken before evaluating later arguments. Whole-value
assignment replaces descendants rather than retaining previously written
children absent from the copied value. Index expressions evaluate once against
live storage, so their writes are visible to the selected read; a final indexed
assignment preserves sibling writes made while evaluating its right side.
Sum values retain their checked nominal owner, case identity, and immutable
payload snapshots. Sparse sum defaults select the first declared case and
zero payload homes. Transitions evaluate their subject once, select only the
matching checked case or final wildcard, and establish independent arm-local
payload homes. Those homes support ordinary receiver mutation and array views;
state transfers retain backing required by captured views.
Views and all four Console operations execute with the controls below.
Any retained `Unsupported` staging outcome is not an Epsilon observation and
cannot survive in the final evaluator.

The gate compiles the exact evaluator plus the 56-line / 2,621-byte
`execution_driver.delta` (SHA-256
`d2b2ce68e4c8afa71f3d096d9069f4c7258a98140d7c828311239de39b85a0f5`) through the
selected Delta route and pins the measured 701,464-byte receipt, SHA-256
`c2fe7c09dac2faea8baedf7e42b8c2715062889b174fc94f856018c6c13d4f2f`.
The ordinary controls cover
success, local, receiver-field, and fixed-array values, repeated mutation,
output, comparisons, bitwise/shift/division behavior, short-circuiting, bounds
ordering, byte-storage range boundaries, and trap prefixes. State controls
cover simultaneous swaps, left-to-right argument traps, local-scope reuse,
wildcard selection, unmatched non-Boolean scalar subjects, grouped transfers,
return/Console continuations, and a 1,024-transfer countdown. Mutation controls
interleave local and parameter homes and transfer their updated values; Console
controls cover computed arguments and traps before the effect. The explicit
`fixtures.tsv` inventory pins each fixture's bytes, digest, expected observation,
and sealed input in its trailing `stdin_hex` column. Empty input is written as
the quoted empty TSV field `""`. The gate rejects missing or unlisted fixtures.
Call controls cover
recursive frame isolation, machine-parameter mutation across states, ordinary
return versus process exit, grouped receiver applications, recursive entry on
existing storage, short-circuit suppression of calls, left-to-right effects and
traps, and indexed-store effects before bounds and before the final update.
Aggregate controls cover same-type sibling and nested places, independent array
elements, local receiver mutation, copied parameters/returns, recursive local
storage, receiver selection before argument effects, snapshot timing, overwrite
clearing, and simultaneous aggregate state arguments with retained machine
parameters. A 300-iteration state control establishes fresh scalar parameter
and local bindings on every iteration while retaining receiver storage. It
checks that workload's tail execution and binding behavior; it does not prove
all resource bounds, root-release paths, or identifier-exhaustion behavior.
A standalone byte-array control passes and returns a fixed array by value,
indexes a returned non-place value, reads its `.len`, and mutates a local copy
without changing the original array.
The contextual-field control combines array/view lengths with nested authored
record fields named `len` and `as_slice`. Effectful receiver indexes run once,
and a retained array view observes a later update. Runtime array/view `.len`
selection skips the unrelated record-field ledger; named records still consume
their exact checked projection identities.
Checked local, record-field, callable, state-application, transition-subject,
and completed-pattern references are indexed once at entry and shared by calls
and state transfers. Their separate source-start bucket ledgers retain original
kind/span matching and progress precedence, so nested postfix projections with
the same start cannot answer for one another. Callable grouping normalizes
before bucket selection; invocation-local values and storage remain separate.
Missing Complete pattern facts remain missing. Invalid construction preserves
all original query ledgers for linear lookup, including incomplete pattern
records. The [direct reference controls](../runtime-references/README.md)
compare indexed and original lookup on synthetic checked facts independently
of this source-program execution gate.

The development driver receives a four-byte little-endian source length,
exactly that many Epsilon source bytes, and all remaining bytes as sealed stdin.
The host only frames bytes; ordinary Delta code separates the two inputs.
Source bytes remain in a validated bounded view over the request; only sealed
stdin is rebuilt into balanced byte trees. The host diagnostic timeout is 300 seconds
per compilation or execution, not an Epsilon observation or resource verdict.
Gamma's published resource profile is unchanged.
This is private test framing, not the final evaluator request/observation
envelope. Six malformed-frame controls expect the single tag byte `05`,
including a declared source length of `0xffffffff` without its body. A
valid zero-length source reaches checking and produces a tagged rejection with
its exact reason and source offset. These framing observations are counted
separately from the language/customer inventory.

## Private execution observations

The diagnostic result starts with an explicit tag; subsequent bytes have the
following layout. Integers and offsets use little-endian encoding.

| Tag | Result | Remaining bytes |
| --- | --- | --- |
| `00` | Exit | full signed `i32` exit code in four bytes, then exact stdout |
| `01` | Trap | one closed language trap-kind byte, then exact stdout prefix |
| `02` | Reject | one closed language rejection-reason byte, then four-byte source offset |
| `03` | Internal | four-byte source offset |
| `04` | Unsupported | none |
| `05` | MalformedRequest | none |

Negative exit codes retain their full two's-complement `i32` bit pattern; they
are not reduced modulo 256. Tags separate an exit from a trap or staging
failure even when their final bytes would otherwise coincide. Rejections retain
both the language reason and its exact coordinate. Stdout and trap prefixes
follow their headers without a terminator or an implicit extra exit byte.

These bytes are a private diagnostic payload returned by the driver through
`ConformanceBytesV1`, not operating-system process statuses or a new Epsilon
observation contract. A successful diagnostic invocation has outer status zero.
Gamma evaluator failures remain raw failures; the runner must not synthesize a
tagged result from them. `Unsupported`, `Internal`, and malformed private input
remain distinct from an authored Epsilon exit or trap. The transport does not
establish final evaluator framing or resource closure.

Thirteen additional controls pin this transport: twelve exit programs retain
stdout `A` while exercising zero, 128, 129, 133, 250 through 253, 256, minus one,
and both signed `i32` endpoints. The thirteenth writes all six observation-tag
bytes followed by `80 ff` as ordinary stdout. These cases distinguish exits
from the former trap/staging sentinels, prevent modulo-256 truncation, and keep
tag-like payload bytes distinct from the header.

View controls cover closed literal escapes, NUL/high bytes, empty views,
single-evaluation `.as_slice`, omitted and explicit slice bounds, nested views,
bounds/effect order, live backing updates, immutable-element and descriptor rejection,
record-field reads, returned-array snapshots, and local backing retained across
state transfers and recursive calls. Console controls pin exact `write_line`
bytes and LF, trap prefixes, ordered reads of `0`, `128`, and `255`, and stable
EOF through nested calls and states.

One customer concatenates the whole, unchanged Omega D
`representations.epsilon` and `lexical_classification.epsilon` members with
[`customers/omega_lexical/main.epsilon`](customers/omega_lexical/main.epsilon).
Its 29 assertions exercise four actual D lexical helpers, including nested
machine calls and machine parameters read across state transitions. Every
member and the 34,911-byte packed source is pinned. The host performs no
function extraction, source rewriting, or semantic substitution. Its actual
260-field `OmegaParser` also exercises bounded-stack member census and type
formation. Two static 260-field negative fixtures require ordinary Epsilon
rejection for a final duplicate or unknown field type, not Gamma stack
exhaustion. A wide Main control declares 260 distinct `i32` locals followed by
one local with an unknown type; it also requires ordinary Epsilon rejection,
not Gamma stack exhaustion during statement-type checking. The Gamma resource
profile is unchanged.

The second customer concatenates the whole, unchanged D
`representations.epsilon` and `alpha_tape.epsilon` members with
[`customers/omega_alpha_tape/main.epsilon`](customers/omega_alpha_tape/main.epsilon).
Every member and its 63,530-byte packed source is pinned. Its two distinct
`AlphaTapeBuffer` receivers execute D's actual `initialize`,
`write_reserved_word`, and `payload_length` machines, including their nested
calls. It checks separate byte storage and lengths, little-endian word writes,
the four `255` bytes written for `-1`, and reinitialization of one buffer without
changing the other. Its expected observation is tagged `Exit(0)` with stdout
`ABCDEFGH`. No D functions or types are extracted or replaced.
The third customer concatenates whole `representations.epsilon` and
`request_and_utf8.epsilon` members with
[`customers/omega_request/main.epsilon`](customers/omega_request/main.epsilon).
Its 40,542-byte packed source exercises the actual `OmegaRequestEnvelope` and
`OmegaUtf8Validation` receiver machines using literals and a Main-owned array
view. It checks empty/nonempty outer OCREQ frames, hostile lengths and trailing
bytes, valid multibyte UTF-8, and malformed continuation, surrogate, out-of-range,
and truncated encodings. Its expected observation is tagged `Exit(0)` with
stdout `A` followed by LF.
The fourth customer concatenates whole `representations.epsilon` and
`lexical_classification.epsilon` with
[`customers/omega_numeric_base/main.epsilon`](customers/omega_numeric_base/main.epsilon).
Its 34,904-byte source exercises all four cases in D's actual
`omega_digit_in_base` machine and the first-case default. Eighteen assertions
cover admitted and rejected digits; the expected result is tagged `Exit(0)`
with stdout `A`.

The fifth customer combines the unchanged `representations.epsilon`,
`request_and_utf8.epsilon`, `lexical_classification.epsilon`, and `lexer.epsilon`
members with [`customers/omega_lexer/main.epsilon`](customers/omega_lexer/main.epsilon).
The four production members total 85,458 bytes. The 160-line / 6,771-byte Main
has SHA-256 `e4a262f1b011402970f958afbc6c950882bb75906fc7244b3ea19c8d489a0e06`;
the complete 3,377-line / 92,229-byte customer has SHA-256
`d53f8f57eb7963c1a3126d206edc9b3b6c2bd4c2fd19c0989cf68053c7abf4bd`.
Its 32 assertions and 14 checked sum cases call D's actual `scan_at` and
`validate_lexical` entries. They require exact token kinds, spans, cursors,
keyword/punctuation/base metadata, escaped-string length, nested comments,
whole-view completion, UTF-8 priority, trailing out-of-profile rejection,
unterminated comments, unsupported escapes, and recovery after a previous
failure. The required observation is tagged `Exit(0)` with stdout `A`.
The current receipt produced that exact observation in 183.898 seconds in the
complete gate, with no other bootstrap evaluator job running during this
customer. Checkpoint `3d3c033f8d` took 293.095 seconds through the
selected-customer gate, with other fixtures running concurrently for part of
that earlier measurement. Both used the unchanged 300-second watchdog. This
pair provides gate completion and local timing evidence, not a controlled
benchmark series, an Epsilon execution bound, or a portable speed claim.
No scanner function is extracted, rewritten, or replaced; this contract is not
evidence of complete Omega parsing or compilation.

Run that customer alone with:

```sh
sh tests/epsilon/interpreted-omega-experiment/run.sh --customer 'Omega D lexer'
```

Selection retains all fixture and member identity checks, exact evaluator
receipt reconstruction, and seven private-framing controls. It then executes
only the named customer; a missing, empty, or unknown name cannot produce an
empty passing selection. Omitting the option runs the complete inventory. A
selected-customer result does not replace full-suite evidence.

Nineteen sum controls cover nullary spellings, first-case defaults, nested sums
in records/arrays, payload ByteRange and argument traps, once-only subjects,
wildcards, snapshots before later argument effects, binder copies and receiver
places, ordinary calls/returns, recursion, state transfers, and retained views
of binder-owned arrays. The three `sum_byte_before_*` controls exercise
[immediate payload establishment](../../../bootstrap/epsilon/LANGUAGE.md#epsilon-constructor-payload-establishment-order):
`ByteRange` preserves the preceding output, including output from evaluating
the failing argument, and suppresses later mutation/output, Assertion, or exit.
Existing successful snapshot and later-argument-trap controls ensure valid
earlier fields do not suppress later argument evaluation.

The inventory specifies 146 language/customer judgments (141 ordinary fixtures
and five whole-member D customers). Seven private-framing controls are counted
separately. The companion
[checking gate](../checking/README.md) pins exact checker reasons and coordinates
without executing Epsilon programs; this gate retains execution and whole-D
customer evidence.

This is executable boundary evidence, not a completed interpreter edge.
Acceptance still requires execution of all Epsilon statements, expressions,
state transfers, traps, and Console observations, followed by exact composition
with D. Executing these D members does not establish a complete D compiler or
the final D-to-Omega bootstrap edge.
