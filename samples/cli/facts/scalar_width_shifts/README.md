# Scalar Width Shifts

A bounded shift count `shift < 4` on a `Wrapping` `u32` value preserves
in-width shift validity and recovers the original scalar across all nine places.

Every place performs the same operation: shift a `u32 in Wrapping` value (`21`) left
by a bounded shift count (`1`), verify the shifted scalar is `42`, shift right by the
same count to recover the original scalar `21`, and exit 0 only when both hold.
Only the transport of the operands changes.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | both the Wrapping u32 value and bounded shift count are copied into locals |
| field | `field/` | bounded record fields carry the Wrapping u32 value and shift count |
| parameter | `parameter/` | the shift machine reads its parameters directly |
| call result | `call_result/` | calls with declared types and bounds supply both operands |
| match join | `match_join/` | both arms keep the operands inside Wrapping and bounds, so the joined operands shift safely |
| const name | `const_name/` | `const SHIFT: u32 = 1` provides the named shift count |
| another state | `other_state/` | a successor state receives the operands and shifts there |
| another machine | `other_machine/` | a forwarding machine transports operands to the shift machine |
| another target body | `target_body/` | each of four target bodies shifts the Wrapping u32 value and shift count |

Each positive place and the root `main.omg` binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`),
declares `Expected exit: 0`, and passes `omega run --both` — Terminal Psi and
native execution agreeing:

```sh
omega run --both samples/cli/facts/scalar_width_shifts/main.omg
omega run --both samples/cli/facts/scalar_width_shifts/local/main.omg
omega run --both samples/cli/facts/scalar_width_shifts/field/main.omg
omega run --both samples/cli/facts/scalar_width_shifts/parameter/main.omg
omega run --both samples/cli/facts/scalar_width_shifts/call_result/main.omg
omega run --both samples/cli/facts/scalar_width_shifts/match_join/main.omg
omega run --both samples/cli/facts/scalar_width_shifts/const_name/main.omg
omega run --both samples/cli/facts/scalar_width_shifts/other_state/main.omg
omega run --both samples/cli/facts/scalar_width_shifts/other_machine/main.omg
omega run --both samples/cli/facts/scalar_width_shifts/target_body/main.omg
```

Each place has an `invalid/` child that breaks the shift/width contract by using an
unqualified `u32` value without `in Wrapping`; that twin differs from its positive
only in the construct's own domain. It carries its authored rejection expectation
in `invalid/sample.json` (`rejection_contains`) and `invalid/expected.txt`, so the
control rejects with `may overflow `u32`` rather than for an unrelated reason.
Check a twin with `omega --check <place>/invalid/main.omg`.

## Governance and Evidence

- Owning requirement: `wiki/spec/language/numeric_values.md#shift-counts`
- Parent task: `CONSERVATION-CONTRACT` (`TASKS.md`)
- Reserved ticket: `8bfead69f1114e37831c300710f126ca`

### Measured Observations

Producer `target/debug/omega` built at `opt-level: 1` from base `f98259e86dd2`,
host `macos_arm64`.

- `omega run --both` on the root and all nine places: `native exit 0` and
  `interp exit: 0` for every one of the ten positives.
- `omega --check <place>/invalid/main.omg` on all nine twins: exit 1 with the
  pinned `may overflow `u32`` diagnostic.
- Discrimination: corrupting the shift amount (`1` -> `3`, so `21 << 3 = 168`
  is no longer `42`) and, separately, corrupting the expected shifted value
  (`42` -> `84`) each produce `native signal 5` together with
  `interp crash: Trap (host presentation agrees)`. The oracle therefore
  discriminates, and both engines trap on the corrupted result.
- `tools/sample_native_gate.py --facts`: 10/10 samples meet their documented
  oracle; 9/9 intended-invalid controls match their authored rejection.
- `tools/corpus_gate.py --native --filter facts/scalar_width_shifts`: executed 9,
  exit codes `{'0': 9}`; the nine `built ... exit:0` rows are recorded in
  `tests/omega/corpus_native_macos_arm64.txt`.
- `tools/corpus_gate.py --filter facts/scalar_width_shifts`: 0 structural moves,
  18 fixtures (`checked` 9, `rejected` 9), 0 fail fixtures did not reject.

### Acceptance Demonstrated

1. **Establishment & Identity**: A `u32 in Wrapping` operand preserves in-width shift validity across arithmetic and bit-shift expressions, enabling left and right shifts without triggering exact arithmetic overflow obligations. The exact left-shift result-representability obligation of the [shift count law](wiki/spec/language/numeric_values.md#shift-counts) is discharged by the Wrapping domain rather than by a caller guard.
2. **Composition**: each `invalid/` twin differs from its positive only in the construct's own domain — an unqualified `u32` without `in Wrapping` — and never in a caller guard, so the reject twins exercise the fact itself.
3. **Transport across Nine Places**: The Wrapping u32 scalar value and shift count survive transport across local bindings, struct fields, machine parameters, call return results, match join bindings, named constants, state transitions, machine forwarding, and target-scoped bodies (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`).
4. **Soundness & Reject Twin Enforcement**: In each corresponding `invalid/` twin, omitting `in Wrapping` causes exact arithmetic verification to reject the unproven representability of the left shift (`may overflow `u32``).
5. **Terminal Psi and carrier lowering**: each place executes through Terminal Psi, and the interpreter's observed `interp exit: 0` agrees with the native exit for all ten positives.
6. **Native realization and image emission**: the nine `_exit` fixtures in `tests/omega/pass/facts/` are built and executed for `macos_arm64`, recording `built ... exit:0`; the `target_body` place additionally carries both shifts in all four target-scoped bodies.

### What Remains Open

- Full interval-set proof propagation and theorem propagation for non-wrapping arithmetic on dynamic counts.
- Cross-carrier shift count reduction proofs on composite structures.
- Native execution rows are recorded for `macos_arm64` only; the three other
  hosted targets are bound and checked but not executed on this host.
