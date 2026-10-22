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
| Other state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | Arrival state reads the same path |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | `forward` keeps the zero-crash route |
| Target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | Target-qualified `forward` keeps the route |

Each reject twin deletes only `requires packet.leaf.bits == 7` (or `== SEVEN`) and must reject with an uncovered Trap crash route.

## Requirements and Evidence

- **Establishment:** `requires packet.leaf.bits == 7` (or `== SEVEN`) establishes the nested entry value before `maybe_zero` consumes it. The twin omits only that path equality.
- **Identity and lifetime:** the operand stays the entry field `leaf.bits`, two steps under `packet`, and a caller crash ceiling is not added on `read_bits`.
- **Composition:** the same path equality is carried through a local, the field operand, a parameter, a call result, a match join, a const name, another state, another machine, and another target body.
- **Soundness boundaries:** deleting the path equality leaves the Trap route uncovered. The witness does not use a proof-only binding as an executable guard.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the nested read and guarded call lower to Terminal Psi and execute with exit 0 under `omega run --both`.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `Build::bind` settles `ProgramEntry` for the four hosted targets. `Packet` and `Leaf` are ordinary records.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** the nested `u64` read, comparison, and call select native instructions inside the program entry.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** the native image exits 0 in agreement with Terminal Psi.
- **Remaining open under `CRASH-CONTRACT`:** policy casts, Boolean and IEEE representation views, entry provenance, selected-use joins, executed outcomes, and ordered completion remain on `TASKS.md`.
