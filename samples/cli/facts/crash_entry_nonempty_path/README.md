# Nonempty crash-entry field paths

Owning requirement: **CRASH-CONTRACT**, entry-operand normalization, nonempty-path lengths.

Fact: a crash-entry u64 equal to 7 reached through a field of a field discharges maybe_zero's value == 0 Trap crash guard without a caller crash ceiling.

`packet.leaf.bits` is two field steps. The entry equality on that path is the operand `maybe_zero` guards with `crashes Trap value == 0`. `read_bits` publishes no crash ceiling. A single field and a dynamic index are other rows (`crash_entry_representation_view`, `indexed_crash_guard`).

The acceptance command is `omega run --both <place>/main.omg`. Every positive declares exit 0 and binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`. No place depends on `std`.

Windows x86-64 measured 2026-10-21 at `ba1d8505b883b1e956ad907068da2a34e8967bae` with `omega run --both`, stdin closed, on an opt-level 1 producer. Root and all nine places exited 0 natively and interpreted. Each reject twin rejected with an uncovered Trap crash route. The compiler was not edited.

## Nine-Place Coverage

| Place | Positive witness | Fact-sensitive reject twin | Transfer |
| --- | --- | --- | --- |
| Local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | Nested field copied to a local |
| Field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | Nested field path is the call operand |
| Parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | Nested field passed as `take`'s parameter |
| Call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | `identity` returns the nested field |
| Match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | Both arms read the same nested field |
| Const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | `SEVEN` names the entry equality |
| Nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | Arrival state reads the same path |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | `forward` keeps the zero-crash route |
| Target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | Target-qualified `forward` keeps the route |

Each reject twin deletes only `requires packet.leaf.bits == 7` (or `== SEVEN`) and must reject with an uncovered Trap crash route.

The local place also pins its writable carrier, because this row's consumer is the entry/call-substitution path that reads the shared entry-provenance local carrier rule: `pass/facts/crash_entry_nonempty_path_local_writable_exit` records the entry value in a writable local that is never written, and `fail/facts/crash_entry_nonempty_path_local_written_rejected` overwrites it with the caller's `replacement` before the call, so the guard is live and the route stays uncovered (`at statement 2 call 0`). Both were measured on the host's executing form: the positive reports native exit 0 and interp exit 0, and the written control rejects.

This change also records the family's check-leg golden rows: the 18 existing fixtures and the two writable-carrier pins now carry 20 rows in `tests/omega/corpus_outcomes.txt` (pure addition, previously none).

Known precision difference, shared with the view rows: a statement that reads a writable local as argument 0 and lends the same local `&mut` as argument 1 is admitted here (`maybe_zero(observed, &mut observed)` exits 0 natively and in Terminal Psi) while the three view routes refuse it at the call's own statement. Named arguments evaluate left to right ([expressions](../../../../wiki/spec/language/expressions.md#evaluation-schedule)), so the read precedes the lend; the admission is sound and is not produced by the shared local carrier rule. The same note sits in `crash_entry_boolean_view`'s README.

## Requirements and Evidence

- **Establishment:** `requires packet.leaf.bits == 7` (or `== SEVEN`) establishes the nested entry value before `maybe_zero` consumes it. The twin omits only that path equality.
- **Identity and lifetime:** the operand stays the entry field `leaf.bits`, two steps under `packet`, and a caller crash ceiling is not added on `read_bits`.
- **Composition:** the same path equality is carried through a local, the field operand, a parameter, a call result, a match join, a const name, a nested machine, another machine, and another target body.
- **Soundness boundaries:** deleting the path equality leaves the Trap route uncovered. The witness does not use a proof-only binding as an executable guard.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the nested read and guarded call lower to Terminal Psi and execute with exit 0 under `omega run --both`.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `Build::bind` settles `ProgramEntry` for the four hosted targets. `Packet` and `Leaf` are ordinary records.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** the nested `u64` read, comparison, and call select native instructions inside the program entry.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** the native image exits 0 in agreement with Terminal Psi.
- **Remaining open under `CRASH-CONTRACT`:** policy casts, Boolean and IEEE representation views, entry provenance, selected-use joins, executed outcomes, and ordered completion remain on `TASKS.md`.
