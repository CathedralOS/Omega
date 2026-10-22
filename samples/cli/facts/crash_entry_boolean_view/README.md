# Crash-entry Boolean view

Owning requirement: **CRASH-CONTRACT**, entry-operand normalization.

Fact: a checked Boolean view of an entry `u8` equal to `1` discharges `maybe_clear`'s `value == false` `Trap` crash guard without a caller crash ceiling.

An integer-to-Boolean view is not a total representation view. Structural access keeps a proved 0/1 landing on the scalar observation route. The entry byte's proved value is that observation. `true == false` is closed false, so the callee's Trap route does not survive. A missing entry equality, and an entry byte proved equal to `0`, still leave the route uncovered.

On main `d6a282e29dad11509f280c3f4bc2ddbe028c922b`, before this repair, the positive rejected with `call from read_flag to maybe_clear at statement 2 call 0 has an uncovered Trap crash route`. After the repair, `omega run --both` on Windows x86-64 reports native exit 0 and interpreted exit 0.

The other eight transport places, policy casts, and IEEE representation views remain open.

## Witnesses

| Witness | Path | Oracle |
| --- | --- | --- |
| Positive | [main.omg](main.omg) | `pass/facts/crash_entry_boolean_view_exit` |
| Missing entry equality | [invalid/main.omg](invalid/main.omg) | `fail/facts/crash_entry_boolean_view_rejected` |
| Entry byte proved 0 | [proved_false/main.omg](proved_false/main.omg) | `fail/facts/crash_entry_boolean_view_false_rejected` |

## Requirements and Evidence

- **Establishment:** `read_flag`'s `requires byte.value == 1` establishes the entry byte before `&byte.value as &bool` observes it. The reject twin omits only that requirement.
- **Identity and lifetime:** the observation is the proved scalar value of the entry byte, not a total representation-view identity. The source place keeps its `u8` carrier.
- **Composition:** the observed `bool` is the actual of `maybe_clear`. No caller crash ceiling is added.
- **Soundness boundaries:** a byte outside `{0, 1}` establishes no Boolean. An entry proved `0` keeps the false Trap route uncovered.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the positive executes with exit 0 in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement through native emission (`Omega 00–09`):** `Build::bind` settles `ProgramEntry` for all four hosted targets, and the Windows x86-64 image exits 0 in agreement with Terminal Psi.
- **Remaining open under `CRASH-CONTRACT`:** the other eight Boolean transport places, policy casts, IEEE representation views, and the rest of `CRASH-CONTRACT` remain on `TASKS.md`.
