# Beta theory component profile

[Theory and ownership](README.md) | [Executable gate](../../../tests/gamma/beta-encoding-theory/README.md)

This profile covers the current source-owned theory emitter and finite
diagnostic equations. It is not a profile for the complete Beta encoder,
certificate producer, or accepted whole-source artifact.

## Emitter

The marked test entry plus the exact ten-member source closure is 207 lines,
9,185 bytes, SHA-256
`3a37c2a3a9683184324a0de1cd92ed27994d66f85fbe54e247badcfd28fa40dd`.
Its only admitted input is empty; framing occupies 9,189 bytes. Nonempty input
returns status 1 without publishing output. The fixed emitted section is
62,836 bytes, SHA-256
`ed5f3425ddc1b5482687e048cb4f0f01456a73da784244a411eefbf9b163d53d`.
The test-owned pins bind these exact bytes; they are diagnostic custody checks,
not arithmetic operations, checker rules, or artifact authority.

The composition has 32 Gamma functions, maximum arity three, nine nested
expression-body lists, and eight simultaneously active bindings per function.
All constructor, clause, function, variable, cell, and administrative-word loops
are tail calls with fixed finite iteration counts. The emitter allocates no
pairs; its marked entry allocates one outcome pair.

A source call-path audit allows twelve contexts and thirteen frames, including
pending calls while arguments are evaluated. Ignoring tail-call savings, the
longest acyclic function route has eleven functions, through the lexical
definition/constant-clause/template/word writers. Pending classification while a
clause call is being prepared is also included. Tail-only self loops do not
accumulate contexts. Thus 104 binding rows suffice. Allow sixteen argument/helper
slots per expression level and ten levels per frame: at most
`13 * 10 * 16 + 32 = 2112` temporary entries, below the evaluator's 524,288.
These are conservative source bounds, not measured runtime peaks. A changed
closure or entry requires a fresh audit.

## Checked finite requests

The generic checker and its diagnostic source are unchanged: 63,504 bytes under
the [existing checking profile](../derivation_checker/CHECKING.md).
The emitted theory has `S=6, C=279, A=11, F=24, W=15708`; its formation work
estimate is 71,564. Every supplied clause and proof row is checked, including
ones not used by the final root.

| Positive batch | Proof rows | Required cumulative checking work |
| --- | ---: | ---: |
| All lexical equations | 1,024 | 137,781 |
| All nibble joins | 768 | 10,757 |
| All high/low nibble splits | 512 | 68,869 |
| All composed split/join round trips | 1,792 | 84,741 |
| Thirteen fixed-width Word cases | 13 | 928 |

The [gate's work derivations](../../../tests/gamma/beta-encoding-theory/README.md)
count clause selection, indexing, every visit/resumption, ground comparisons,
explicit rules, and final-root checks independently of observed output. A Word
unfolding includes seventeen template visits, seventeen resumptions, and sixteen
ground-comparison transitions; none of its eight fields is an unchecked byte copy.

The largest request is 134,824 bytes and the largest ground table has 1,552
rows, both in the round-trip batch. Including checker source and framing gives
198,332 bytes. Across these finite families, the generic cumulative pair bound
is at most `36290 + 137781 * 96 + 128 = 13263394`, below the selected arena's
40,265,318 pairs. Each vector is a separate evaluator invocation; these figures
do not claim that unrelated certificates can reset accounting mid-request.
Malformed Word arities and semantic corruptions must publish exact owned
rejections. A timeout, evaluator failure, or short observation is not a verdict.

The macOS arm64 gate passed two identical emissions, three nonempty-input
producer refusals, and all 51 exact checker diagnostics. Observed elapsed times
were 0.822 seconds for emission and 7.471, 4.519, 4.813, 8.851, and 2.550 seconds
for the five positive batches in table order. These are observations of this
run, not semantic limits or portable performance guarantees.

Windows runtime validation is unavailable in this session. The gate documents
the same Git Bash/Python entrypoint for Windows x64 and macOS arm64.
Full-source certificate storage, cost, and artifact acceptance remain open.
