# Crash-entry signed zero

Owning requirement: **CRASH-CONTRACT**, entry-operand normalization
(`TASKS.md`). The board sentence this row closes: an entry `f32` proved equal to
`-0.0` discharges `maybe_nonzero`'s `value != 0.0` Trap guard across all nine
places of `facts/crash_entry_signed_zero`, because the guard reads recorded IEEE
equality and the two signed zeros compare equal.

Fact: an entry `f32` equal to `-0.0` discharges `maybe_nonzero`'s
`value != 0.0` Trap crash guard without a caller crash ceiling, because the
guard reads recorded IEEE equality and `-0.0` compares equal to `0.0`.

`maybe_nonzero(value: f32) -> f32` declares `crashes Trap value != 0.0`.
`-0.0 != 0.0` is false under IEEE equality, so the `Trap` route is never
entered, the machine returns `value`, and `main`'s `observed == 0.0` selects
`done`. Every positive program completes with `Expected exit: 0`, and each place
binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64` and
`linux_arm64` in its own `build.omg`. `read_zero` publishes no crash ceiling;
the entry `requires value == -0.0` (or `packet.value == -0.0` /
`value == NEGATIVE_ZERO` / the successor state's own requirement) is what
establishes the operand.

Each reject twin proves the entry equal to `1.0` instead: `1.0 != 0.0` stays
true, so `maybe_nonzero`'s `Trap` route stays live and uncovered and the
program rejects with `has an uncovered Trap crash route`. A bit-identical
compare is the wrong record — it would keep `-0.0 != 0.0` live and reject the
positive; NaN unorderedness is not this sentence, it belongs to
`facts/crash_entry_nan_equality`.

## Nine-Place Coverage

| Place | Positive witness | Reject twin |
| --- | --- | --- |
| Local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| Field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| Parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| Call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| Match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| Const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| Other state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| Target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

`local` copies the entry into `observed`, `field` reads `Packet.value`,
`parameter` passes the entry straight to `maybe_nonzero`, `call_result` carries
`identity`'s return, `match_join` joins two arms that agree, `const_name` uses
`public const NEGATIVE_ZERO: f32 = -0.0`, `other_state` moves the value through
a successor state that restates the requirement, `other_machine` forwards
through a second machine, and `target_body` repeats the forwarding machine for
each of the four targets. `main` passes `-0.0` in every place and asserts
`observed == 0.0`.

Pinned corpus fixtures are `pass/facts/crash_entry_signed_zero_through_<place>_exit`
and `fail/facts/crash_entry_signed_zero_through_<place>_rejected` for the nine
places. The twin's authored fragment lives in `<place>/invalid/sample.json`
(`role: intended-invalid`, `rejection_contains`), and
`<place>/invalid/main.omg` differs from `<place>/main.omg` only in the entry
value (`1.0` for `-0.0`), the literals that carry it, and its `Control:` header
line.

The local place additionally pins its writable carrier, because the equality
route reads the shared entry-provenance local-carrier rule:
`pass/facts/crash_entry_signed_zero_through_local_writable_exit` records the entry
equality in a writable local that is never written, and
`fail/facts/crash_entry_signed_zero_through_local_written_rejected` overwrites the
local with `1.0`, which makes `value != 0.0` live, so the route stays uncovered
(`call from `read_zero` to `maybe_nonzero` at statement 2 call 0`).


## Requirements and Evidence

- **Establishment:** the entry's `requires value == -0.0` (or
  `packet.value == -0.0`, `value == NEGATIVE_ZERO`, or the successor state's
  restated requirement) establishes the signed zero before `maybe_nonzero`
  reads it, and the guard's recorded IEEE equality is what the crash route
  consumes. `main` does not re-establish the fact afterwards; it only asserts
  `observed == 0.0`.
- **Identity and lifetime:** the record is the entry operand's own IEEE
  equality, read at the guard. Each carrier transports that one record — the
  local copy, the field read, the parameter itself, the `identity` result, the
  agreeing match arms, the named const, the successor state's parameter and the
  forwarding/target-qualified machines — and nothing writes that storage
  between entry and guard.
- **Composition:** every place copies the one fact sentence; the consuming
  operation is `maybe_nonzero`'s guard and `main`'s `observed == 0.0`. The nine
  twin pairs differ only in the construct's own value, with no caller-side
  equality guard added after the call.
- **Soundness boundaries:** equality is IEEE equality, not bit identity. A
  bit-identical compare would keep `-0.0 != 0.0` live and lose the positive; an
  entry proved equal to `1.0` keeps the route and rejects. Proof-only terms
  cannot authorize executable guards.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the recorded equality
  lowers into the crash guard's float predicate, the crash entry and its
  uncovered route lower unchanged, and `omega run --both` executes the same
  program in the Terminal Psi interpreter and natively with agreement
  (`interpreter exit: 0`, `native exit 0`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
  each place binds `ProgramEntry` for the four hosted targets; `target_body`
  carries a target-qualified `forward` machine per target; the `f32` entry
  scalar, the `f32` const carrier and the `Packet { value, marker }` field
  placement come from the layout plan.
- **Instruction selection, register homes, and activation frame realization
  (`Omega 03–06`):** IEEE float comparison selection for `value != 0.0` and
  `observed == 0.0` (unordered vs. ordered outcomes preserved), `f32` register
  arguments across `identity`/`forward`, and the callee's activation frame.
- **Form encoding, branch relaxation, relocations, and native image emission
  (`Omega 07–09` + emission):** direct images execute and agree with the
  interpreter across the four hosted targets; `macos_arm64` is measured on the
  recording host, `windows_x86_64` is retained from the row's own recording.

## Recorded runs

Corpus rows measured at the landed revision with an opt-level 1 corpus runner on
macOS arm64 (`python3 tools/corpus_gate.py --both --record --filter
facts/crash_entry_nan_equality,facts/crash_entry_signed_zero`), ten pass rows and
ten fail rows for this row:

| Leg | Rows | Result |
| --- | --- | --- |
| check | `pass/facts/crash_entry_signed_zero_through_<place>_exit` and the writable-carrier pin | `checked`, `check-target:macos_arm64`, and 0 structural moves against the pinned rows |
| native | the same ten in `corpus_native_macos_arm64.txt` | `built`, `exit:0` |
| both | the same ten in `corpus_both_macos_arm64.txt` | `built`, `exit:0`, `interpreter-exit:0`, `interpreter:agrees` |
| reject twins | `fail/facts/crash_entry_signed_zero_through_<place>_rejected` and the written-local pin | `rejected`, `expected`; 0 rejected without the expected fragment |

Earlier direct runs at `9ff26a87021` with an opt-level 1 CLI and stdin closed:
9 of 9 positives `omega run --both` exited 0 (native exit 0, interpreter exit 0),
9 of 9 twins rejected with `has an uncovered Trap crash route`, and each twin's
direct diagnostic equals its pinned continuation line. The psi flow machinery
that reads entry origins moved after that measurement (`114906953e1`,
`539552bc218`), so the corpus rows above are the re-measurement.

The twin diagnostics name the transport the twin interrupts, for example
`` call from `read_zero` to `maybe_nonzero` at statement 2 call 0
has an uncovered Trap crash route; publish a same-cause route whose guard covers
this invocation `` for `local`, `statement 3` for `field`, and
`` call from `read_zero` to `forward` `` for `other_machine`.

## Remaining open under CRASH-CONTRACT

`TASKS.md`'s entry-operand normalization keeps nonempty-path lengths and policy
casts open; this row establishes the signed-zero equality sentence and its
reject twins, not those.
