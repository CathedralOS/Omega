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
the Delta-written Epsilon implementation. The evaluator is currently 10,092
lines / 507,516 bytes, authored in 59 explicitly manifested members.

The executable slice runs complete checking, locates the fixed `Main::main`,
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
Console write/exit continuations are executable. Unqualified scalar machines and
direct-self receiver methods support nested and recursive calls, scalar returns,
and resultless return/falloff. Callee locals are separate from caller locals;
committed field and output effects return with the result. The expression engine
threads those effects through left-to-right operands, arguments, index checks,
right sides, and returns, retaining exact trap/exit prefixes. Only the outer
entry adapter maps ordinary completion to process exit zero.
Arbitrary receiver places, sum transitions, aggregate parameters, nested storage,
views, and remaining Console operations
stay explicitly `Unsupported`; that staging outcome is not an Epsilon
observation and cannot survive in the final evaluator.

The exact evaluator plus eight-line driver compiles through the selected Delta
route to a pinned 599,984-byte Gamma receipt. Seventy-four ordinary controls cover
success, local, receiver-field, and fixed-array values, repeated mutation,
output, comparisons, bitwise/shift/division behavior, short-circuiting, bounds
ordering, byte-storage range boundaries, and trap prefixes. State controls
cover simultaneous swaps, left-to-right argument traps, local-scope reuse,
wildcard selection, unmatched non-Boolean scalar subjects, grouped transfers,
return/Console continuations, and a 1,024-transfer countdown. Mutation controls
interleave local and parameter homes and transfer their updated values; Console
controls cover computed arguments and traps before the effect. The explicit
`fixtures.tsv` inventory pins each fixture's bytes, digest, and expected
observation; the gate rejects missing or unlisted fixtures. Call controls cover
recursive frame isolation, machine-parameter mutation across states, ordinary
return versus process exit, grouped receiver applications, recursive entry on
existing storage, short-circuit suppression of calls, left-to-right effects and
traps, and indexed-store effects before bounds and before the final update.

One additional customer concatenates the whole, unchanged Omega D
`representations.epsilon` and `lexical_classification.epsilon` members with
[`customers/omega_lexical/main.epsilon`](customers/omega_lexical/main.epsilon).
Its 29 assertions exercise four actual D lexical helpers, including nested
machine calls and machine parameters read across state transitions. Every
member and the 34,911-byte packed source is pinned. The host performs no
function extraction, source rewriting, or semantic substitution. Its actual
260-field `OmegaParser` also exercises bounded-stack member census and type
formation. Two static 260-field negative fixtures require ordinary Epsilon
rejection for a final duplicate or unknown field type, not Gamma stack
exhaustion. The Gamma resource profile is unchanged.

This is executable boundary evidence, not a completed interpreter edge.
Acceptance still requires execution of all Epsilon statements, expressions,
state transfers, traps, and Console observations, followed by exact composition
with D.
