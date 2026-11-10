# What the Terminal interpreter checks at runtime

The Terminal interpreter does check overflow, zero divisors, shift counts,
index and view bounds, case activity, integer width and loan/claim state on
the operations it runs. But almost every failed check stops the run with an
interpreter error, `TerminalInterpretError::VerifiedOperationMalformed`. That
error means "this verified artifact is malformed". It does not stop the
program with a crash. Only four things produce a program crash
(`TerminalExecutionStatus::Crashed`):

- a `TrappingInteger` operation,
- a `FloatResultPolicy::Trapping` operation,
- an explicit `Crash` terminator that lowering placed in the program,
- a crash returned by a host boundary effect.

So for `stage_trust.md` rule 2: the interpreter does catch an exact add that
overflows, an exact divide by zero and an index out of bounds when native code
has no guard. It reports them as a malformed artifact, not as the program's
own trap. A `--both` run therefore shows a disagreement as an interpreter
failure, not as a matching crash. I did not trace how the CLI reports that
error.

Code: `<checkout>/` at
`80ce6c9c461`. Paths are relative to
`omega-rust/psi/interpreter/terminal-interpreter/src/` unless stated. The
integer semantics live in
`omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs` (called
`proposition.rs` below). `malformed()` is `boundary_values.rs:696-698`.

## Table

| Runtime check | Does the interpreter check it? | file:line | On violation | Verification |
|---|---|---|---|---|
| Overflow: `ExactIntegerAdd/Subtract/Multiply` | yes | `engine_execution.rs:3621-3690`; `proposition.rs:250-262` (`checked_*` then `admits`), `308-330` (`exact_add_values`) | `Err(VerifiedOperationMalformed)`. This is an interpreter error, not a crash. | opened: matches (b1_012) |
| Overflow: `WrappingInteger*` add/sub/mul | no, by design: the result is reduced modulo 2^bits | `proposition.rs:266-292`, `662-664`, `755-761` | none (the value wraps) | opened: matches |
| Overflow: `SaturatingInteger*` add/sub/mul | no, by design: the result is clamped to the type's range | `engine_execution.rs:3375-3396`, `3697-3701`; `proposition.rs:942-1047` | none (the value is clamped) | opened: matches |
| Overflow: `TrappingInteger` add/sub/mul | yes | `engine_execution.rs:3751-3807`; trap built at `3733-3748` | `Crashed(CrashCause::Trap)` at the operation | opened: matches (b1_012) |
| Integer casts | yes | `IntegerExactCast`/`IntegerWiden`: `engine_execution.rs:3494-3536`. `TrappingInteger::Convert`: `3799-3801` | exact cast or widen: `malformed`. Trapping convert: trap | opened: matches |
| Division or remainder by zero | yes, for every policy | `proposition.rs:666-671` (`exact_div` = `checked_div`), `673-690`, `692-753`; dispatch `engine_execution.rs:3669-3684`, `3778-3784` | Exact, Wrapping and Saturating: `malformed`, so a zero divisor is never given a value. Trapping: trap | opened: matches. b1_012 inferred this without reading `proposition.rs`; now confirmed |
| Minimum ÷ −1 | yes for Exact and Trapping. Wrapping and Saturating define a result | Exact divide: the quotient fails `admits`, or `checked_div` returns None at 128 bits (`proposition.rs:250-262`). Exact remainder: explicit None at `682-684`. Wrapping divide → MIN (`699-704`). Wrapping remainder → 0 (`716-721`). Saturating divide → MAX (`737-742`). Saturating remainder = wrapping remainder → 0 (`752`) | Exact: `malformed`. Trapping: trap. Wrapping and Saturating: the defined value | opened: corrected. The notes left this UNDECIDED; it is now decided from the code |
| Shift counts | yes for Exact and Trapping. Wrapping reduces the count | Exact: `proposition.rs:806-818` rejects count ≥ width or negative; `exact_shift_left` also rejects bits shifted out (`909-911`, `918-920`). Wrapping: count is `rem_euclid(width)` (`1200-1209`). Dispatch `engine_execution.rs:3573-3611`, `3786-3797` | Exact: `malformed`. Trapping: trap. Wrapping: no error (the count is taken modulo the width) | opened: corrected. The notes said "UNDECIDED / inferred"; the wrapping count is reduced, not checked |
| Float trapping result policy | yes | `engine_execution.rs:3305-3312` | `Crashed(Trap)` | opened: matches (b1_012) |
| Runtime array index (fixed array, bounded or borrowed byte view, element view) | yes, before the operation runs | `engine_execution.rs:2664-2669` → `storage_primitives.rs:4655-4743` → `terminal-psi/semantics/src/structural.rs:2688-2694` (`selected < extent`) | `malformed` | opened: matches (b1_019 left the callee UNDECIDED) |
| Literal index into a fixed array on a case-payload path | yes | `boundary_inputs.rs:2049-2059` | `malformed` | opened: matches (b1_003) |
| `ElementViewRead` / `ElementViewWrite` index | yes: the index must be below the length, and the operand length must equal the view length | `storage_bytes_and_views.rs:4819-4821`, `4785`, `4702-4707` | `malformed` | opened: matches (b1_017) |
| `ByteSequenceRead` / `ByteSequenceWrite` index | yes | `engine_structural.rs:2267-2269`; `storage_bytes_and_views.rs:2860-2862`; the array-backed write uses `checked_add` (`2884-2886`) | `malformed` | opened: matches (b1_013, b1_016) |
| Region or view bounds (subslice) | yes: start ≤ end ≤ length, and the operand length equals the source length | element view: `storage_bytes_and_views.rs:3811-3819`, `4864`, `4906-4908`. Byte sequence: `2303-2342`. Byte view `subslice`: `3102-3116` | `malformed` | opened: matches |
| Case or tag validity | yes: every `Case` segment of every operation's projection paths is checked before dispatch | `engine_execution.rs:2670` → `boundary_inputs.rs:1946-1967` → `1772-1774`; payload projection `2004-2012`; field read `engine_structural.rs:2104-2113`; `StructuralCase` terminator `engine_terminators.rs:2354-2368` | `malformed` | opened: matches (b1_004, b1_014) |
| Domain: integer value fits its type width | yes, on most writes and reads | `engine_execution.rs:3826-3832` (`terminal_scalar_belongs_to_type`). Used at `storage_primitives.rs:2160`, `5372`, `5392`, `engine_structural.rs:922`, `storage_bytes_and_views.rs:4682`, `4801` | `malformed` | opened: matches |
| Domain: bounded-integer field restriction (for example `0..=127`) | only when a value is established, crosses the boundary or is loaned. Not on the field-store path I traced | checked: record establishment `engine_structural.rs:374`; primitive local `storage_primitives.rs:3878-3884`; boundary replace or loan `boundary_effects.rs:3024-3037`, `3059-3072`; boundary inputs `boundary_values.rs:1653`. Not checked: `execute_structural_scalar_field_store` `engine_structural.rs:920-956` → `store_primitive_storage` `storage_primitives.rs:4473-4482` → `stage` `2160` → `ScalarCell::write` `5381-5400` (width only) | `malformed` or `TerminalEffectRejection` where checked. Not detected on the field-store path | opened: new lead, in no note. I did not check whether lowering puts an explicit check before such stores |
| Domain: structural qualification ids | partly: the domain id must be declared for the carrier type. The predicate itself is not re-evaluated | `storage_records_and_arrays.rs:3555-3580`; field-store "preserves domain content" is a static test on declarations (`engine_structural.rs:823-893`) | `malformed` | opened: matches (b1_001, b1_004) |
| Loan or ownership state | yes, as access-mode and custody bookkeeping. This is not a dynamic borrow stack | access cover: `storage_references.rs:2188-2240`, `4451-4461`. Live claims block a subslice: `storage_bytes_and_views.rs:4884-4885` via `engine_custody.rs:509-511`. Affine frontier: `engine_custody.rs:105-116`. Nominal cleanup with a live loan: `engine_custody.rs:731-745`. Release needs a live referent: `storage_references.rs:3194-3197`. A crash frontier must equal the live claims: `engine_terminators.rs:2491-2493` | `malformed`, `AffineFrontierMismatch` or `NominalCleanupHasLiveLoans`. All are interpreter errors | opened: matches (b1_010, b1_025, b1_026) |

## What remains unknown

- How `omega run --both` and the CLI report a `TerminalInterpretError`
  compared with a `Crashed` status. Not traced.
- Whether Terminal Psi lowering puts an explicit check before every
  bounded-integer field store. If it does not, the interpreter accepts an
  out-of-range value on that path.
- Whether any operation family I did not open skips a check. I opened the
  dispatch table (`engine_execution.rs:2672-2886`) and the arithmetic, index,
  view, case and loan paths above. I did not open the atomic-access,
  boundary-call or `PrimitiveScalarRead` bodies; those come from the notes.
- b1_024 inferred a possible Rust panic from division by zero at
  `storage_records_and_arrays.rs:3683-3684` (`stride /= length`). I opened
  it. When a dimension length is 0, `count` is presumably 0, so the loop body
  never runs. I did not prove that `count` always equals the product of the
  dimensions.

## Notes review

There are 26 `b1` notes. Every claim behind the rows above was checked against
the code in 10 of them (001, 003, 004, 010, 012, 013, 014, 016, 017, 019).
None had a wrong verdict on a row. Their errors were omissions:

- They left the semantics of `IntegerType` as UNDECIDED, even though that is
  where the minimum ÷ −1 and shift behaviour is defined.
- No note found the gap on the bounded-integer field-store path.
- One note (b1_024) gave an unproved inference about a host panic.
