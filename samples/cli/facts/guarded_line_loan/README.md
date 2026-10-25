# Guarded line loan

A byte owner written through a mutable loan and read through a shared loan
after a selected case edge observes the written byte. Every place copies that
sentence and runs the same operation: `receive` writes 37 into byte 0 through
an exclusive loan and returns a `ReadOutcome` case carrying 5, the selected
case edge runs, and the shared read after the edge observes `37 + 5`. The
program exits 0 only when the observation is 42.

The original root `main.omg` and all nine places bind `ProgramEntry` for all
four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`)
and declare `Expected exit: 0`.

## Places

The nine places carry the guarded line loan across each language construct into the consuming operation:

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | original witness: local byte owner stays live across selected case edge |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | the owner is a local byte array |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | the owner is a record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | the shared loan is passed as a parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | the owner is returned by a call |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | either match arm yields the same shared loan |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | a named constant selects the written and observed byte |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | a successor machine receives the shared loan |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | a forwarding machine performs the write |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | each target body writes through the loan |

Every place carries an `invalid/` reject twin (mirrored in `controls/held_loan/`) that
adds one construct: a competing exclusive loan of that place's owner, held live
across the loan site. That breaks only the exclusive-liveness this fact rests
on, so the owner can no longer be written through one loan and read after the
edge through another. The field twin's diagnostic names `holder.line`, and the
another-machine twin's names the forwarding machine `forward`.

## Requirements and Evidence

Owning spec: [borrows and aliases](../../../../wiki/spec/language/ownership.md#borrows-and-aliases) and [loan resources and compatibility](../../../../wiki/spec/terminal-psi/loans.md).
This row establishes the transport and carrier lowering of guarded line loans across the nine places:
a byte owner written through a mutable loan and read through a shared loan after a selected case edge observes the written byte.

Full 8-layer specification and lowering evidence:

- **Establishment:** The machine `receive(&mut line)` writes 37 into byte 0 and returns `ReadOutcome::Complete { count: 5 }`. The case transition guard `ReadOutcome::Complete { count }` and subsequent state transition guard on the shared loan `observe(&line, count)` establish the guarded length and value fact (`line[0] + count == 42`) directly from the store and payload without any post-hoc caller equality guard (`transition x == 100`).
- **Identity and lifetime:** The guarded fact on the borrowed line remains attached to the exact loan and backing storage version of `[u8; 64]` while active. Any conflicting exclusive loan or mutation to the underlying storage while `held` or the shared observation is active invalidates the fact and is statically refused.
- **Composition:** The guarded line loan and its bound compose across all nine standard places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming operation. In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` (and `<place>/controls/held_loan/main.omg`) differ strictly in introducing a competing exclusive loan (`held: &mut [u8; 64] = &mut line`) held live across the `receive` call site, keeping the construct, bound, and operation identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins (and `<place>/controls/held_loan/main.omg`) fail closed at compile time (`--check`) with `state \`receive\` receives \`line\` while local borrow \`held\` is still active` (or `receives \`holder.line\`` / `state \`forward\` receives \`line\``), proving that competing loans or weakened guards cannot evade loan exclusivity rules.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Guarded loans, array references, and case transitions lower into canonical target-neutral Terminal Psi (`PSITERM\0`) operations with borrow restoration windows (`reborrow restored call uses`) while erasing static proof obligations, executing with agreement in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Fixed array `[u8; 64]` layout geometry (size 64, align 1) and pointer-to-array references validate, and `CallPlan`/`StatePlan` lower pointer and enum case values across register and stack slots according to target calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 8-bit memory byte stores (`line[0] = 37`), case tag branch dispatch, and 64-bit wrapping additions. It assigns register homes and disjoint stack-frame activation storage for the 64-byte array home and `ReadOutcome` case payloads, inserting standard entry prologues and multi-return restoration epilogues honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements for local array storage resolve to concrete offsets, function-relative basic blocks and case branch targets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

## What remains open

Still open on the ownership and loans board: multi-borrow slice partitioning, runtime-sized slice view reslicing, and foreign non-hosted target execution.

Canonical authority: [borrows and aliases](../../../../wiki/spec/language/ownership.md#borrows-and-aliases)
gives `&mut T` exclusive observation and mutation and `&T` shared observation
under its loan contract, and requires borrowed arguments to continue denoting
the original storage rather than a caller-side snapshot; its starting blob is
`7e6f9a8ad935edc4e66324dd6ced07fe0c5be7ad`.
[Loan resources and compatibility](../../../../wiki/spec/terminal-psi/loans.md)
requires a live exclusive loan to carry its exact captured place, access and
restoration obligation and rejects an incompatible overlapping use; its starting
blob is `3e2e33d7f04f0dc0e4e144a9ccf94ab045af61f3`. This row establishes only
the transport and carrier lowering of this one guarded-line-loan fact across the
nine places; the board's other acceptance remains open.

```sh
omega run --both samples/cli/facts/guarded_line_loan/local/main.omg
omega --check samples/cli/facts/guarded_line_loan/local/invalid/main.omg
python3 tools/sample_gate.py --filter cli/facts/guarded_line_loan --cli <omega>
python3 tools/sample_native_gate.py --filter cli/facts/guarded_line_loan --cli <omega>
python3 tools/corpus_gate.py --filter facts/guarded_line_loan
python3 tools/corpus_gate.py --native --filter facts/guarded_line_loan
python3 tools/corpus_gate.py --both --filter facts/guarded_line_loan
```