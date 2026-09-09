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
the Delta-written Epsilon implementation. The evaluator is currently 12,097
lines / 617,354 bytes, authored in 87 explicitly manifested members.
The complete gate checks 143 ordinary fixtures, five D customers, and seven
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
The obsolete `Unsupported` staging cases are absent from execution outcomes.
This removes no language observation and does not close the resource profile.

The gate compiles the exact evaluator plus the 55-line / 2,565-byte
`execution_driver.delta` (SHA-256
`ba509602e6873117e59ffc544ada6c8aa16e20b08311e69a01b7cb3897199b38`) through the
selected Delta route and pins the measured 719,826-byte receipt, SHA-256
`dd4985c0eb6e1f30bc2178f90dd30e25ae7b842fb544137f606a44e622000f22`.
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

All nine [closed trap identities](../../../bootstrap/4_epsilon/LANGUAGE.md#9-closed-rejection-and-trap-identities)
have an ordinary-source control that preserves output written before the trap:

| Code | Trap | Prefix control |
| --- | --- | --- |
| 1 | Overflow | [`add_overflow.epsilon`](add_overflow.epsilon) |
| 2 | DivisionByZero | [`state_argument_order.epsilon`](state_argument_order.epsilon) |
| 3 | SignedDivisionOverflow | [`division_overflow.epsilon`](division_overflow.epsilon) |
| 4 | ShiftCount | [`shift_count.epsilon`](shift_count.epsilon) |
| 5 | ByteRange | [`byte_range.epsilon`](byte_range.epsilon) |
| 6 | Bounds | [`bounds_read.epsilon`](bounds_read.epsilon) |
| 7 | NonBoolean | [`nonboolean.epsilon`](nonboolean.epsilon) |
| 8 | Assertion | [`assertion.epsilon`](assertion.epsilon) |
| 9 | NonExhaustiveTransition | [`state_nonexhaustive.epsilon`](state_nonexhaustive.epsilon) |

The division-overflow, shift-count, and non-Boolean controls require the binary
prefix `00 ff` and suppress a following write of `B`. Their prior empty-prefix
observations could not detect dropped output. These cover trap identity,
prefix preservation, and stopping after the fault, not every route to each
trap or resource exhaustion.

The existing [`full_scalar.epsilon`](full_scalar.epsilon) control also checks
successful `i32` endpoint arithmetic, signed quotient/remainder combinations,
sign-bit and alternating-bit operations, and shift counts 0 and 31. Its literal
expectations distinguish 32-bit wrapping left shifts from overflow and arithmetic
right shifts from truncating division. In particular, `-1 << 31` exercises the
largest intermediate product in the current 64-bit shift implementation. These
are source-level execution checks, not an exhaustive scalar refinement proof.

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
| `04` | unassigned (retired staging case) | never emitted |
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
tagged result from them. `Internal` and malformed private input
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
Every member and its 69,011-byte packed source is pinned. Its two distinct
`AlphaTapeBuffer` receivers execute D's actual `initialize`,
`write_reserved_word`, and `payload_length` machines, including their nested
calls. It checks separate byte storage and lengths, little-endian word writes,
the four `255` bytes written for `-1`, and reinitialization of one buffer without
changing the other. It also exercises actual symbolic label allocation,
forward fixup resolution, instruction-boundary replay, and sealing. A jump to
the following `ret` finalizes to exactly `0c 09 00 00 00 00 00 00 00 14`.
Post-seal writes and emission cannot change that payload; a second finalization
returns zero. An undefined registered label instead retains the placeholder,
reports D's internal-misuse state, and cannot finalize.

All four direct address-writing paths admit offset 16,777,211 and reject
16,777,212 before appending bytes. The jump control also crosses the retired
one-MiB target ceiling at 1,048,572. These short unfinished forward-target buffers
exercise encoding guards, not full-size tape realization or successful
finalization at those high offsets. Finalization still requires a target inside
the actual payload at a reconstructed instruction start. The address-admission
controls failed the old four guards at `07e8c1df86` with diagnostic `01 08` and stdout
`ABCDEFGH`; their one-MiB ceiling contradicted the selected Alpha profile.

The successful observation is tagged `Exit(0)` with stdout `ABCDEFGH`, the
actual ten-byte sealed payload above, and a second sealed 79-byte program.
The host compares the complete observation against literal bytes before
extracting the actual final 79 bytes, stamping them with the existing
`stamp_seed` route, and executing the result. It never stamps the expected
literal or a failed diagnostic's partial stdout.

The second program echoes input until EOF and exits with its byte count. D's
ordinary emitters establish three labels and four fixups across address-only,
register/address, and register/register/address shapes. Its read loop begins
at offset 30, its halt at 74, and its write/return subroutine at 76. A nonzero
byte takes `jnz` back to the loop; a zero byte takes the following `jmp`.
The forward `jeq` distinguishes a zero-extended `0xff` input byte from the
64-bit all-ones EOF sentinel. Counter initialization and increment are explicit.
The same emitted tape runs on sealed inputs empty, `00`, `80 ff`, and
`00 80 ff`, requiring exact echo and process statuses 0, 1, 2, and 3. These
small statuses avoid platform exit-width ambiguity. Each target execution has
a 30-second host watchdog, not a language or profile bound.
On macOS arm64, the expanded customer at base `497e21fb9a` produced the exact
observation in 135.413 seconds; all four emitted-tape executions passed. The
existing shell/seed route also selects Windows x64, but that host has not been
validated for this customer.

This connects interpreted D tape construction to native Alpha execution. The
first jump/return payload remains structural evidence, not a terminating
standalone program. Neither payload establishes complete Omega compilation,
full-size storage realization, or a checked refinement proof. No D functions
or types are extracted or replaced. Run the customer and emitted program with:

```sh
sh tests/epsilon/interpreted-omega-experiment/run.sh --customer 'Omega D Alpha tape buffers'
```

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
On macOS arm64, base `564b45e205` ran this unchanged customer in 145.273
seconds. Reusing the exact current callable fact during checking reduced the
measured invocation to 120.518 seconds; that 712,070-byte receipt was
reconstructed separately in 107.401 seconds. Both customer runs used the same
selected evaluator and 300-second watchdog, without another bootstrap job.
These are single-run measurements, not a controlled benchmark series, an
Epsilon execution bound, or a complete-D parser speed claim. Earlier runs at
different checkpoints took 183.898 seconds without another bootstrap job and
293.095 seconds at `3d3c033f8d` with overlapping fixtures; they are not a matched
comparison for this change.
No scanner function is extracted, rewritten, or replaced; this contract is not
evidence of complete Omega parsing or compilation.

With compact runtime field coordinates, a paired macOS arm64 comparison on
the same evaluator and unchanged customer observed 136.905 seconds for the
712,070-byte baseline receipt and 126.065 seconds for the 713,259-byte receipt
pinned above (135.839 and 125.229 child CPU seconds). Other bootstrap checks
were running. This single pair is not a benchmark series or a full-parser
speedup; the canonical selected-customer command independently returned the
same observation in 132.815 seconds. The baseline is the receipt at
`844e450838`, unchanged by subsequent folder moves.
The separate [complete-D parser comparison](../../bootstrap/omega-parser/README.md#field-identity-comparison)
now records both full-customer outcomes, times, and cumulative allocations.

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
[immediate payload establishment](../../../bootstrap/4_epsilon/LANGUAGE.md#epsilon-constructor-payload-establishment-order):
`ByteRange` preserves the preceding output, including output from evaluating
the failing argument, and suppresses later mutation/output, Assertion, or exit.
Existing successful snapshot and later-argument-trap controls ensure valid
earlier fields do not suppress later argument evaluation.

The record-field identity control uses two nominal owners with matching field
spellings. It checks independent nested writes, unwritten zero defaults, sibling
preservation, and a whole-record snapshot across subsequent updates. The private
[runtime controls](../runtime-invariants/README.md) separately isolate each
identity coordinate and malformed projection admission.

The inventory specifies 148 language/customer judgments (143 ordinary fixtures
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
