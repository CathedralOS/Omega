# Crash-entry Boolean view

Owning requirement: **CRASH-CONTRACT**, entry-operand normalization.

Fact: a checked Boolean view of an entry `u8` equal to `1` discharges `maybe_clear`'s `value == false` `Trap` crash guard without a caller crash ceiling.

An integer-to-Boolean view is not a total representation view. Structural access keeps a proved 0/1 landing on the scalar observation route. The entry byte's proved value is that observation. The view records it, and a local copy, a call whose result is ensured equal to that observation, and a match whose arms agree all read the same observation. `true == false` is closed false, so the callee's Trap route does not survive. A missing entry equality, and an entry byte proved equal to `0`, still leave the route uncovered.

On main `d6a282e29dad11509f280c3f4bc2ddbe028c922b`, before the local repair, the root rejected with `call from read_flag to maybe_clear at statement 2 call 0 has an uncovered Trap crash route`. On main `ce4681e434c766c619ebaebc2b372510d7b92ac8`, before the call and match transport, `call_result` and `match_join` rejected with `call from read_flag to maybe_clear at statement 3 call 0 has an uncovered Trap crash route`. After that transport, `omega run --both` on Windows x86-64 reports native exit 0 and interpreted exit 0 for every place.

## Witnesses

| Place | Positive | Missing entry equality | Entry byte proved 0 |
| --- | --- | --- | --- |
| local | [main.omg](main.omg) | [invalid/main.omg](invalid/main.omg) | [proved_false/main.omg](proved_false/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | [field/proved_false/main.omg](field/proved_false/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | [parameter/proved_false/main.omg](parameter/proved_false/main.omg) |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | [call_result/proved_false/main.omg](call_result/proved_false/main.omg) |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | [match_join/proved_false/main.omg](match_join/proved_false/main.omg) |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | [const_name/proved_false/main.omg](const_name/proved_false/main.omg) |
| other state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | [other_state/proved_false/main.omg](other_state/proved_false/main.omg) |
| other machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | [other_machine/proved_false/main.omg](other_machine/proved_false/main.omg) |
| target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | [target_body/proved_false/main.omg](target_body/proved_false/main.omg) |

Corpus pins: `pass/facts/crash_entry_boolean_view_exit` and `pass/facts/crash_entry_boolean_view_<place>_exit`; `fail/facts/crash_entry_boolean_view_rejected`, `fail/facts/crash_entry_boolean_view_false_rejected`, and the same pair for each place.

## Requirements and Evidence

- **Establishment:** `requires byte.value == 1` (or `packet.flag.value == 1`, `byte.value == ONE`, or the successor state's same requirement) establishes the entry byte before `&...value as &bool` observes it. Each reject twin omits only that requirement.
- **Identity and lifetime:** the observation is the proved scalar value of the entry byte, not a total representation-view identity. The source place keeps its `u8` carrier. Field nests `Byte` in `Packet.flag`.
- **Composition:** the observed `bool` reaches `maybe_clear` as a local, a direct parameter, a call result ensured equal to the view, or a match join of that view. `other_machine` and `target_body` pass it to `forward`, which publishes the same `value == false` ceiling and calls `maybe_clear`. No caller crash ceiling is added on `read_flag`.
- **Soundness boundaries:** a byte outside `{0, 1}` establishes no Boolean. An entry proved `0` keeps the false Trap route uncovered, including through a call result and a match join.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** each positive executes with exit 0 in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement through native emission (`Omega 00–09`):** `Build::bind` settles `ProgramEntry` for all four hosted targets, and the Windows x86-64 image exits 0 in agreement with Terminal Psi.
- **Remaining open under `CRASH-CONTRACT`:** policy casts, IEEE representation views, and the rest of `CRASH-CONTRACT` remain on `TASKS.md`.
