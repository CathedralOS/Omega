# Beta theory component profile

[Theory and ownership](README.md) | [Executable gate](../../../tests/gamma/beta-encoding-theory/README.md)

This profile covers the current source-owned theory emitter and finite
diagnostic equations. It is not a profile for the complete Beta encoder,
certificate producer, or accepted whole-source artifact.

## Emitter

The marked test entry plus the exact twenty-one-member source closure is 442 lines,
21,516 bytes, SHA-256
`c38a34fe1ea560b250f58de795452f2d3ea7478eb31b31b4b94ffb020fb49172`.
Its only admitted input is empty; framing occupies 21,520 bytes. Nonempty input
returns status 1 without publishing output. The fixed emitted section is
93,140 bytes, SHA-256
`daec17f40c9d02002cad2ed870e7da5e8a9cb31d94fe91b71fa53f3a01027614`.
The test-owned pins bind these exact bytes; they are diagnostic custody checks,
not arithmetic operations, checker rules, or artifact authority.

The composition has 58 Gamma functions, maximum arity four, twelve nested
expression-body lists, and eleven simultaneously active bindings per function.
All constructor, clause, function, variable, cell, and administrative-word loops
are tail calls with fixed finite iteration counts. The emitter allocates no
pairs; its marked entry allocates one outcome pair.

A source call-path audit allows fourteen contexts and fifteen frames, including
pending calls while arguments are evaluated. Ignoring tail-call savings, the
longest acyclic function route has twelve functions, through the lexical
definition/hexadecimal-clause/constant-clause/template/word writers. Pending
classification while a clause call is being prepared is also included. Tail-only
self loops do not accumulate contexts. Thus 165 binding rows suffice. Allow
sixteen argument/helper slots per expression level and thirteen levels per
frame: at most `15 * 13 * 16 + 32 = 3152` temporary entries, below the evaluator's
524,288.
These are conservative source bounds, not measured runtime peaks. A changed
closure or entry requires a fresh audit.

## Checked finite requests

The generic checker and its diagnostic source are unchanged: 63,504 bytes under
the [existing checking profile](../derivation_checker/CHECKING.md).
The emitted theory has `S=8, C=284, A=12, F=57, W=23284`; its formation work
estimate is 111,996. Every supplied clause and proof row is checked, including
ones not used by the final root.

| Positive batch | Proof rows | Required cumulative checking work |
| --- | ---: | ---: |
| All lexical equations | 1,024 | 137,781 |
| All nibble joins | 768 | 10,757 |
| All high/low nibble splits | 512 | 68,869 |
| All composed split/join round trips | 1,792 | 84,741 |
| Thirteen fixed-width Word cases | 13 | 928 |
| Counter byte helpers, each 128-case half-table | 128 | 9,029 or 25,413 |
| Nineteen separate checked successor cases | 21..84 | 290..3,341 |
| All ordered nibble pairs | 768 | 10,757 |
| Twelve separate byte comparisons | 19 | 204..1,284 |
| Thirty separate word comparisons | 177 | 2,004..10,644 |

The [gate's work derivations](../../../tests/gamma/beta-encoding-theory/README.md)
count clause selection, indexing, every visit/resumption, ground comparisons,
explicit rules, and final-root checks independently of observed output. A Word
unfolding includes seventeen template visits, seventeen resumptions, and sixteen
ground-comparison transitions; none of its eight fields is an unchecked byte copy.

The largest request is 165,128 bytes and the largest ground table has 1,552
rows, both in the round-trip batch. Including checker source and framing gives
228,636 bytes. Across these finite families, the generic cumulative pair bound
is at most `51506 + 137781 * 96 + 128 = 13278610`, below the selected arena's
40,265,318 pairs. Each vector is a separate evaluator invocation; these figures
do not claim that unrelated certificates can reset accounting mid-request.
Malformed Word arities and semantic corruptions must publish exact owned
rejections. A timeout, evaluator failure, or short observation is not a verdict.

The counter cases explicitly compose carry selection and increment through
ordinary proof rules. The maximum-Word case proves Overflow, not a resource
refusal or a wrapped successful value. These finite requests fit the unchanged
checking profile; they do not establish the cost of a full-source certificate.

Ordering cases compare all 256 nibble pairs, literal byte priority examples,
and complete words including every highest-differing byte position and exact/
adjacent Beta capacity words. The word recipe explicitly normalizes all eight
byte comparisons before composing the ordering chain; even a decisive high byte
does not hide unchecked fixture rows. The new ordering requests use at most
121,144 bytes, 531 ground terms, 768 proof rows, and 10,757 checking work.

On macOS arm64, the full gate passed two identical emissions, three nonempty-input
producer refusals, and all 181 exact checker diagnostics. Emission took 1.041
seconds; the exhaustive nibble-ordering batch took 5.681 seconds. The twelve
byte-comparison cases took 3.203..3.255 seconds and the thirty word-comparison
cases took 3.636..3.852 seconds. A preceding five-case ordering preflight also
passed. These are scoped observations, not semantic limits or portable
performance guarantees.

Windows runtime validation is unavailable in this session. The gate documents
the same Git Bash/Python entrypoint for Windows x64 and macOS arm64.
Full-source certificate storage, cost, and artifact acceptance remain open.
