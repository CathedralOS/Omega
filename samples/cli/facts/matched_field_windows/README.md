# Matched Field Windows

The shared fact and operation in every place: a match arm treats its integer
pattern as the subject's value, so in the arm `which == 0` the window
`items[0..which + 2]` has length 2, the same length as the other arm (`items[2..4]`),
and the returned window publishes length 2. Every place copies that sentence and writes
the returned window through `write_pair`, which requires length at least 2.

Only the transport of the matched coordinate changes across the nine places. Each place
is an application that exits 0:

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the pattern subject is a local copy |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the pattern subject is a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the pattern subject is an explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the window end is a call whose ensured result is 2 |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | the window end joins two spellings of that coordinate |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the window end adds the named constant `SPAN` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the coordinate |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine receives `which + 2` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies publishes the same contract |

Root witness: [`main.omg`](main.omg) with [`build.omg`](build.omg).

## Execution

Expected exit: 0 for all ten positive entries (root plus 9 places). Every positive place
and root `main.omg` binds `ProgramEntry` for all four hosted targets (`windows_x86_64`,
`linux_x86_64`, `macos_arm64`, `linux_arm64`) and passes `omega run --both`: the Terminal
Psi leg reports `interp exit: 0` in exact agreement with native `exit 0`.

```sh
omega run --both samples/cli/facts/matched_field_windows/main.omg
omega run --both samples/cli/facts/matched_field_windows/local/main.omg
omega run --both samples/cli/facts/matched_field_windows/field/main.omg
omega run --both samples/cli/facts/matched_field_windows/parameter/main.omg
omega run --both samples/cli/facts/matched_field_windows/call_result/main.omg
omega run --both samples/cli/facts/matched_field_windows/match_join/main.omg
omega run --both samples/cli/facts/matched_field_windows/const_name/main.omg
omega run --both samples/cli/facts/matched_field_windows/other_state/main.omg
omega run --both samples/cli/facts/matched_field_windows/other_machine/main.omg
omega run --both samples/cli/facts/matched_field_windows/target_body/main.omg
```

## Reject Twins

Each place has one reject twin under `<place>/invalid/` with authored `sample.json`
that breaks only this fact and still rejects during compile-time proof checking.
Seven of them keep the pinning arm at length 2 and change the arm that does not
pin the pattern — `items[2..3]` instead of `items[2..4]` — so the two arms
disagree on published length. The remaining two break the coordinate transport
itself: `call_result` makes the call's ensured result 3, and `other_machine`
hands `open_end` the coordinate 3 while that machine requires `end == 2`.

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove ensures contract` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `cannot prove ensures contract` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `cannot prove ensures contract` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `cannot prove ensures contract` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `cannot prove ensures contract` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `cannot prove ensures contract` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove ensures contract` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `violates required fact \`end == 2\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove ensures contract` |

The target-body twin breaks every one of the four target bodies, so it rejects
on every host rather than only where a body is realized.

`tests/omega/fail/slices/matched_open_window_end` is the related wildcard-arm
control: an arm that does not learn the coordinate cannot prove the open end.

## Pinned Fixtures

The positive side is `tests/omega/pass/facts/matched_field_windows_through_<place>_exit`,
one fixture per place, each a byte-copy of that place's `main.omg` with
`expected_native_exit.txt` of 0. The negative side is
`tests/omega/fail/facts/matched_field_windows_through_<place>_rejected`, each
with the pinned `expected.txt` fragment above.

`tests/omega/corpus_outcomes.txt` records all nine positives as `checked` and
all nine twins as `rejected` with their fragment matched.
`corpus_native_macos_arm64.txt` records `exit:0` for all nine positives, and
`corpus_both_macos_arm64.txt` records `interpreter-exit:0 interpreter:agrees`.
All three were measured on a fresh producer built from the rebased candidate.

## Requirements and Evidence

- **Establishment:** Matching `which` against `0` refines the arm's static value of `which` to `0`, so the subslice expression `items[0..which + 2]` has statically refined bounds `0..2` and establishes slice length `2` matching the other arm (`items[2..4]`). The construct satisfies the enclosing machine's contract (`ensures result.len == 2`) at introduction without requiring any caller post-hoc equality guard (`transition ... == 2`).
- **Identity and lifetime:** The borrowed mutable slice window `&mut [u8]` (and `&mut [i64]` generally) preserves its exact backing storage identity (`bytes`) and published length `2` across the match arm join and return loan. Writing through `write_pair(window)` directly mutates the backing array in place (`bytes[0] = 9; bytes[1] = 8; bytes[2] = 3`), demonstrating that mutable subslice projection maintains loan identity and exclusive mutable access without aliasing or unexpected copy.
- **Composition:** The pattern-refined window coordinate composes across all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into `write_pair(window)`, which requires `window.len >= 2`. Each positive place `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in the construct's own arm range (`items[2..3]` vs `items[2..4]`) or coordinate contract (`end == 2`), confirming clean compositional modularity without caller guard divergence.
- **Soundness boundaries:** All 9 reject twins under `<place>/invalid/main.omg` (and `tests/omega/fail/slices/matched_open_window_end`) reject during compile-time proof checking whenever an arm's slice length disagrees (`cannot prove ensures contract ... result.len == 2`) or the coordinate precondition is violated (`violates required fact \`end == 2\``). Exclusive mutable borrow rules, loan scopes, and slice safety invariants are preserved throughout; an unproved or mismatched slice length cannot escape to runtime.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Pattern-refined subslice construction lowers without semantic loss into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing static length proofs while retaining runtime pointer+length view carriers, borrow restoration windows (`reborrow restored call uses`), match joins, and state transitions. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native execution.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Backing array `[u8; 4]` and mutable slice view descriptors (`(ptr, len)`) validate target layout geometry (two-word fat pointers of 16 bytes with 8-byte alignment). `CallPlan` and `StatePlan` lower two-word slice view returns and parameters across target ABI register and stack slots according to target calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for subslice base-pointer offset arithmetic (`base + start * elem_size`, here `start * 1` for `u8` and `start * 8` for 64-bit carriers) and slice length calculation (`end - start = 2`). The backend assigns register homes and disjoint stack-frame activation storage for the backing array and slice descriptor, maintaining worst-case stack bounds and inserting standard entry prologues and epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve for the array backing storage, match dispatch and basic blocks lay out with short/near branch relaxation and offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across `windows_x86_64` and `macos_arm64`.

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/matched_field_windows --jobs 4
```