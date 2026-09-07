# Beta theory component profile

[Theory and ownership](README.md) | [Executable gate](../../../tests/gamma/beta-encoding-theory/README.md)

This profile covers the current source-owned theory emitter and finite
diagnostic equations. It is not a profile for the complete Beta encoder,
certificate producer, or accepted whole-source artifact.

## Emitter

The marked test entry plus the exact sixteen-member source closure is 319 lines,
15,154 bytes, SHA-256
`e423cf265f2f7c4ded9a552db28500738078cbb1ec558b776a489a1d93d25588`.
Its only admitted input is empty; framing occupies 15,158 bytes. Nonempty input
returns status 1 without publishing output. The fixed emitted section is
82,480 bytes, SHA-256
`6f8616c15ac9a9cc7e418a3aac78c4d3bd965388a7421b21756840a038cb2819`.
The test-owned pins bind these exact bytes; they are diagnostic custody checks,
not arithmetic operations, checker rules, or artifact authority.

The composition has 45 Gamma functions, maximum arity four, ten nested
expression-body lists, and ten simultaneously active bindings per function.
All constructor, clause, function, variable, cell, and administrative-word loops
are tail calls with fixed finite iteration counts. The emitter allocates no
pairs; its marked entry allocates one outcome pair.

A source call-path audit allows fourteen contexts and fifteen frames, including
pending calls while arguments are evaluated. Ignoring tail-call savings, the
longest acyclic function route has twelve functions, through the lexical
definition/hexadecimal-clause/constant-clause/template/word writers. Pending
classification while a clause call is being prepared is also included. Tail-only
self loops do not accumulate contexts. Thus 150 binding rows suffice. Allow sixteen argument/helper
slots per expression level and eleven levels per frame: at most
`15 * 11 * 16 + 32 = 2672` temporary entries, below the evaluator's 524,288.
These are conservative source bounds, not measured runtime peaks. A changed
closure or entry requires a fresh audit.

## Checked finite requests

The generic checker and its diagnostic source are unchanged: 63,504 bytes under
the [existing checking profile](../derivation_checker/CHECKING.md).
The emitted theory has `S=7, C=281, A=12, F=36, W=20619`; its formation work
estimate is 94,943. Every supplied clause and proof row is checked, including
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

The [gate's work derivations](../../../tests/gamma/beta-encoding-theory/README.md)
count clause selection, indexing, every visit/resumption, ground comparisons,
explicit rules, and final-root checks independently of observed output. A Word
unfolding includes seventeen template visits, seventeen resumptions, and sixteen
ground-comparison transitions; none of its eight fields is an unchecked byte copy.

The largest request is 154,468 bytes and the largest ground table has 1,552
rows, both in the round-trip batch. Including checker source and framing gives
217,976 bytes. Across these finite families, the generic cumulative pair bound
is at most `46144 + 137781 * 96 + 128 = 13273248`, below the selected arena's
40,265,318 pairs. Each vector is a separate evaluator invocation; these figures
do not claim that unrelated certificates can reset accounting mid-request.
Malformed Word arities and semantic corruptions must publish exact owned
rejections. A timeout, evaluator failure, or short observation is not a verdict.

The new counter cases explicitly compose carry selection and increment through
ordinary proof rules. The maximum-Word case proves Overflow, not a resource
refusal or a wrapped successful value. These finite requests fit the unchanged
checking profile; they do not establish the cost of a full-source certificate.

On macOS arm64, the gate passed two identical emissions, three nonempty-input
producer refusals, and all 106 exact checker diagnostics. Emission took 0.947
seconds. The five original positive batches took 8.089, 5.125, 5.327, 9.584,
and 3.128 seconds in table order. The four byte-helper batches took
3.111..3.430 seconds; the nineteen successor cases took 2.643..2.962 seconds,
including 2.928 seconds for maximum overflow. These are scoped observations,
not semantic limits or portable performance guarantees.

Windows runtime validation is unavailable in this session. The gate documents
the same Git Bash/Python entrypoint for Windows x64 and macOS arm64.
Full-source certificate storage, cost, and artifact acceptance remain open.
