# Crash-entry NaN equality

Owning requirement: **CRASH-CONTRACT**, entry-operand normalization
(`TASKS.md`). The board sentence this row closes: an entry `f32` that is NaN
makes `value == value` false, so `maybe_ordered`'s Trap guard drops across all
nine places of `facts/crash_entry_nan_equality`; the guard reads that unordered
IEEE equality. The `1.0` twin keeps the route.

Fact: an entry `f32` that is NaN makes `value == value` false, so
`maybe_ordered`'s Trap guard drops without a caller crash ceiling, because the
guard reads recorded IEEE equality and NaN is unordered.

`maybe_ordered(value: f32) -> f32` declares `crashes Trap value == value`. NaN
is unordered, so that guard is false: the `Trap` route is never entered and the
machine returns `value`. Every positive program completes with
`Expected exit: 0`, and each place binds `ProgramEntry` for `windows_x86_64`,
`linux_x86_64`, `macos_arm64` and `linux_arm64` in its own `build.omg`.
`read_nan` publishes no crash ceiling.

Each reject twin proves the entry equal to `1.0`, where `value == value` stays
true, so `maybe_ordered`'s `Trap` route stays uncovered and the program rejects
with `has an uncovered Trap crash route`. `const_name` cannot divide by the
sealed float operator in a const initializer, so it names the quiet-NaN bits
`2143289344` and reads them as `f32` (`&value as &f32`); the twin names the bits
of `1.0`, `1065353216`.

## Nine-Place Coverage

| Place | Positive witness | Reject twin |
| --- | --- | --- |
| Local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| Field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| Parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| Call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| Match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| Const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| Nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| Target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

`local`, `field`, `other_state`, `other_machine` and `target_body` form the NaN
with `0.0f32 / 0.0f32`, the sealed `float_operations` call; `parameter` carries
it into the entry's own argument; `call_result` returns it from `identity`;
`match_join` joins two arms that agree on the value; `const_name` reads the
`2143289344` bits; `target_body` repeats the forwarding machine for each of the
four targets and `main` observes NaN through all four.

Pinned corpus fixtures are `pass/facts/crash_entry_nan_equality_through_<place>_exit`
and `fail/facts/crash_entry_nan_equality_through_<place>_rejected` for the nine
places. The twin's authored fragment lives in `<place>/invalid/sample.json`
(`role: intended-invalid`, `rejection_contains`), and
`<place>/invalid/main.omg` differs from `<place>/main.omg` only in the entry
value and its `Control:` header line.

The local place additionally pins its writable carrier, because the equality
route reads the shared entry-provenance local-carrier rule:
`pass/facts/crash_entry_nan_equality_through_local_writable_exit` records the
unordered entry in a writable local that is never written, and
`fail/facts/crash_entry_nan_equality_through_local_written_rejected` overwrites the
local with `1.0`, which makes `value == value` true, so the route stays uncovered
(`call from `read_nan` to `maybe_ordered` at statement 3 call 0`).


## Requirements and Evidence

- **Establishment:** the entry `f32` arrives NaN from the sealed
  `0.0f32 / 0.0f32` call (`const_name`: the `2143289344` bits read as `f32`),
  and `maybe_ordered`'s guard `value == value` reads that unordered IEEE
  equality. Nothing re-establishes the fact after the construct completes:
  `main` only observes `observed != observed`, which is true exactly while the
  value stays unordered.
- **Identity and lifetime:** the record is the entry operand's own IEEE
  equality. Each carrier transports that one record to the guard — the local
  copy (`local`), the `Packet.value` read (`field`), the entry parameter itself
  (`parameter`), the `identity` return (`call_result`), two agreeing match arms
  (`match_join`), the const's bits (`const_name`), the successor machine's
  parameter (`other_state`), `forward`'s parameter (`other_machine`) and the
  four target-qualified `forward` bodies (`target_body`). No storage holding
  that record is written between entry and guard.
- **Composition:** every place copies the one fact sentence, and the consuming
  operation is the guard plus `main`'s `observed != observed`. The nine twin
  pairs differ only in the entry value, never in a caller guard.
- **Soundness boundaries:** NaN equality stays false and is never proved
  reflexively; a guard that cannot be entered leaves the route uncovered, so
  each twin is refused rather than silently accepted, and proof-only terms
  cannot authorize executable guards.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the guard lowers as a
  float equality predicate over the recorded entry observation, the crash entry
  and its uncovered route lower unchanged, and `omega run --both` executes the
  same program in the Terminal Psi interpreter and natively with agreement
  (`interpreter exit: 0`, `native exit 0`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
  each place's `build.omg` binds `ProgramEntry` for the four hosted targets;
  `target_body` carries a target-qualified `forward` machine per target; the
  `f32` entry scalar and the `Packet { value, marker }` field placement come
  from the layout plan.
- **Instruction selection, register homes, and activation frame realization
  (`Omega 03–06`):** IEEE float comparison selection for `value == value` and
  `observed != observed`, `f32` arguments across the `identity`/`forward` calls
  (including the live-across-call `call_result` carrier), and the callee's
  activation frame.
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
| check | `pass/facts/crash_entry_nan_equality_through_<place>_exit` and the writable-carrier pin | `checked`, `check-target:macos_arm64`, and 0 structural moves against the pinned rows |
| native | the same ten in `corpus_native_macos_arm64.txt` | `built`, `exit:0` |
| both | the same ten in `corpus_both_macos_arm64.txt` | `built`, `exit:0`, `interpreter-exit:0`, `interpreter:agrees` |
| reject twins | `fail/facts/crash_entry_nan_equality_through_<place>_rejected` and the written-local pin | `rejected`, `expected`; 0 rejected without the expected fragment |

Earlier direct runs at `9ff26a87021` with an opt-level 1 CLI and stdin closed:
9 of 9 positives `omega run --both` exited 0 (native exit 0, interpreter exit 0),
9 of 9 twins rejected with `has an uncovered Trap crash route`, and each twin's
direct diagnostic equals its pinned continuation line. The psi flow machinery
that reads entry origins moved after that measurement (`114906953e1`,
`539552bc218`), so the corpus rows above are the re-measurement.

The twin diagnostics name the transport the twin interrupts, for example
`` call from `read_nan` to `maybe_ordered` at statement 2 call 0
has an uncovered Trap crash route; publish a same-cause route whose guard covers
this invocation `` for `local`, `statement 3` for `field`, and
`` call from `read_nan` to `forward` `` for `other_machine`.

`crash_entry_signed_zero` owns the signed-zero equality sentence; this row is NaN unorderedness only.

## Remaining open under CRASH-CONTRACT

`TASKS.md`'s entry-operand normalization keeps nonempty-path lengths and
policy casts open; this row establishes the NaN-equality sentence and its
reject twins, not those.
