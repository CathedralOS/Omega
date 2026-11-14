# Method array operand inference

Owning task: **STRUCTURAL-GENERIC-MATCHING** in [TASKS.md](../../../../TASKS.md),
requirement "infer later method type operands through const-dependent array
carriers": `first<const Count: u64, T>(bytes: &[T; Count])` accepts
`first<3u64>(&bytes)` for `bytes: [u8; 3]`, keeping the matching-count positive
and the wrong-count reject together.

Fact: with Count supplied as 3, the omitted element T of Bytes::last is inferred
as u8 from the actual &[u8; 3], and the selected implementation's [T; Count]
carrier is the caller's copyable [u8; 3].

Every place copies that sentence. The consuming operation is the selected
boundary method `Bytes::last<3u64>(&bytes)`, whose body reads `bytes[Count - 1]`.
Only how the `[u8; 3]` actual reaches the call changes.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [positive](local/main.omg) | [Count 4](invalid_local/main.omg) | array copied into a second local |
| field | [positive](field/main.omg) | [Count 4](invalid_field/main.omg) | `Packet::items` between `before`/`after` canaries |
| parameter | [positive](parameter/main.omg) | [Count 4](invalid_parameter/main.omg) | `consume(bytes: [u8; 3])` makes the call |
| call_result | [positive](call_result/main.omg) | [Count 4](invalid_call_result/main.omg) | `make_bytes()` result |
| match_join | [positive](match_join/main.omg) | [Count 4](invalid_match_join/main.omg) | both arms join one `[u8; 3]`; each arm has its own last element |
| const_name | [positive](const_name/main.omg) | [Count 4](invalid_const_name/main.omg) | `const BYTES: [u8; 3]` |
| other_state | [positive](other_state/main.omg) | [Count 4](invalid_other_state/main.omg) | nested `arrived(bytes: [u8; 3])` |
| other_machine | [positive](other_machine/main.omg) | [Count 4](invalid_other_machine/main.omg) | `forward` relays to `consume` |
| target_body | [positive](target_body/main.omg) | [Count 4](invalid_target_body/main.omg) | Build selects `X86Bytes` or `ArmBytes`, each behind a tag check (17 / 29) |

The [root](main.omg) and its [twin](invalid/main.omg) repeat the operation.
Each positive binds `ProgramEntry` for Windows x86-64, Linux x86-64, macOS ARM64
and Linux ARM64, selects its `Bytes` provider per target, and declares exit 0.
The nine places are pinned as
`tests/omega/pass/facts/method_array_operand_inference_through_<place>_exit` and
`tests/omega/fail/facts/method_array_operand_inference_through_<place>_rejected`.

## Baseline and repair

At `c79bb826816`, an optimized producer refused every positive, and the
explicit full tuple `Bytes::last<3u64, u8>(&bytes)` the same way:

```text
scalar computations construct structural type `array(named(name(u8)),literal(3))`
with a shape the ordinary Unit roster does not carry
```

The partial tuple itself already formed. The Unit roster then registered the
selected implementation's formal `[T; Count]`, under its substitutions, as the
identity `[u8; 3]` with `copyable: false`, because the copy question read the
open formal `T` without those substitutions. The caller's constructed `[u8; 3]`
registered the same identity with `copyable: true`, and the roster refused the
disagreement. Free generic machines are specialized before this point, which
is why `first<3u64>(&bytes)` on an ordinary machine already ran.

The shape collector now asks the declared-property owner through the same
substitutions that named the shape's identity
(`substituted_type_satisfies_copy`), so the formal's copy permission is its
actual element's. The cleanup and descriptor judgments already read those
substitutions.

## Reject twins

Each `invalid/` twin changes only the explicit prefix to `4u64`. The actual is
still `[u8; 3]`, and every twin rejects with
``argument `bytes` for state `last` expects `&[u8; 4]` ``. The
[element_not_copy](element_not_copy/main.omg) twin removes the copy requirement
the fact names: a non-copy `Token` element cannot satisfy `T [copy]`, and it
rejects with ``generic machine `SelectedBytes::last` has a concrete
specialization tuple that does not satisfy its authored type bounds``
(pinned as `method_array_operand_inference_element_not_copy_rejected`).

## Nearest row

The nearest row is [inferred_array_equation_call](../inferred_array_equation_call/README.md),
which also transports a byte array into a generic call. It infers a const
index through a `where` structural equation on a free machine, and its twins
reject in preparation (`requires equation binds ...`). This row has no equation:
it completes a partial explicit tuple of a Build-selected boundary method, and
its repair is in checked execution planning (the Unit structural roster), a
different layer. Its twins name the argument carrier and the copy bound, which
that row's twins do not.

## Requirements and evidence

- **Establishment:** the explicit `3u64` and the actual `&[u8; 3]` together form
  the tuple `<3, u8>` at the call edge. No caller guard restates Count or T.
- **Identity and lifetime:** the array is read through a shared borrow only; no
  write intervenes between its construction and the call. The field place
  checks the neighbouring `before`/`after` bytes after the call.
- **Composition:** each place delivers the same `[u8; 3]` carrier; the match
  join returns 13 or 29 by arm, and `main()` checks both.
- **Soundness boundaries:** a wrong explicit Count and a non-copy element both
  reject before Terminal Psi.
- **Checked-to-Terminal Psi lowering (`Psi 06–08`):** the selected
  implementation's substituted array carrier and the caller's array share one
  roster plan; Terminal execution reports exit 0.
- **Provider settlement, layout and ABI (`Omega 00–02`):** Build rows select the
  `Bytes` provider per target (two implementations in `target_body`); the
  borrowed array argument and the `u8` result use the ordinary call plan.
- **Instruction selection through emission (`Omega 03–09`):** `bytes[Count - 1]`
  depends on the specialized Count at runtime; a wrong extent or element width
  reads another value and traps.
- **Runtime falsifier:** elements 7, 11, 13 are distinct, so a wrong index or
  width does not produce 13; the match arms produce 13 and 29.

Observed on macOS ARM64 (`omega run --both`, native and Terminal exit 0) for the
root and all nine places with the repaired producer. The other hosted targets
keep their authored rows, unexecuted here.

## What remains open

Borrowing a named array constant directly (`&BYTES`) refuses with
``array constant projection currently requires unborrowed literal integer
selectors``; the const place therefore binds the constant to a local first.
The rest of `STRUCTURAL-GENERIC-MATCHING` (open schemas, retained extensions,
late receivers, operator/conformance supply, binders, lifetimes and finite
type alternatives) stays open on the board.
