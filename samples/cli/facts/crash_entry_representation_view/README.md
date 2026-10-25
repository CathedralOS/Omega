# Crash-entry integer representation views

Owning requirement: **CRASH-CONTRACT**, entry-operand normalization.

Fact: a checked equal-width integer view of an entry `u64` equal to `7` discharges `maybe_zero`'s `value == 0` `Trap` crash guard without a caller crash ceiling.

A checked equal-width integer view preserves the authenticated source operand's
bits through a guarded scalar helper. The source's entry equality must discharge
the helper's zero-crash route without adding a caller crash ceiling or replacing
the helper's published guard.

`main.omg` retains the frozen failing witness's semantics (with the current
`public` spelling): an entry `u64` equal to 7 is viewed as
`i64`, copied, and passed to `maybe_zero`. On main `53cd2938437a29356a48d6eaf29f14de07a4bb99`
it rejected with an uncovered `Trap` route at statement 2. The same-carrier
control executed native/Terminal exit 0.

The acceptance command is `omega run --both <place>/main.omg`; every positive declares
exit 0 and binds all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`).

## Nine-Place Coverage

| Place | Positive witness | Fact-sensitive reject twin | Pinned corpus fixtures |
| --- | --- | --- | --- |
| Local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | `pass/facts/crash_entry_representation_view_through_local_exit`, `fail/facts/crash_entry_representation_view_through_local_rejected` |
| Field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | `pass/facts/crash_entry_representation_view_through_field_exit`, `fail/facts/crash_entry_representation_view_through_field_rejected` |
| Parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | `pass/facts/crash_entry_representation_view_through_parameter_exit`, `fail/facts/crash_entry_representation_view_through_parameter_rejected` |
| Call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | `pass/facts/crash_entry_representation_view_through_call_result_exit`, `fail/facts/crash_entry_representation_view_through_call_result_rejected` |
| Match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | `pass/facts/crash_entry_representation_view_through_match_join_exit`, `fail/facts/crash_entry_representation_view_through_match_join_rejected` |
| Const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | `pass/facts/crash_entry_representation_view_through_const_name_exit`, `fail/facts/crash_entry_representation_view_through_const_name_rejected` |
| Nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | `pass/facts/crash_entry_representation_view_through_other_state_exit`, `fail/facts/crash_entry_representation_view_through_other_state_rejected` |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | `pass/facts/crash_entry_representation_view_through_other_machine_exit`, `fail/facts/crash_entry_representation_view_through_other_machine_rejected` |
| Target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | `pass/facts/crash_entry_representation_view_through_target_body_exit`, `fail/facts/crash_entry_representation_view_through_target_body_rejected` |

Additional mechanism sub-witnesses and controls retained in this folder and pinned under `tests/omega/{pass,fail}/facts/crash_entry_integer_view_*`:

- Root (`main.omg`): parameter → primitive view → immutable copy → guarded helper.
- `high_bit`: `u64::MAX` reads as `i64(-1)`, preserving bit interpretation.
- `reverse_signed`: `i32(-1)` reads as `u32::MAX`.
- `snapshot_after_write`: the copied view stays 7 after its original storage is overwritten.
- `surviving_guard`: a nonclosed route remains permitted and executes normally for input 7.
- `view_then_widen`: signed interpretation happens before value-preserving widening.
- `controls/changed_guard`, `controls/stale_storage`, `controls/incompatible_view`: reject uncovered call routes and incompatible `u8` → `bool` representation views.

## Requirements and Evidence

- **Establishment:** `read_bits`'s entry precondition (`requires value == 7`, `requires packet.bits == 7`, or `requires value == SEVEN`) establishes the exact bit-pattern fact on the entry storage before `&value as &i64` observes it; each `<place>/invalid/main.omg` omits only that entry precondition and rejects with `has an uncovered Trap crash route`.
- **Identity and lifetime:** `EntryOperands::scalar` retains both the source `u64` and viewed `i64` carriers while recursively authenticating the entry root; `snapshot_after_write` proves that completed scalar snapshots retain their earlier meaning after storage mutation, while `controls/stale_storage` proves writes invalidate current storage provenance.
- **Composition:** the equal-width integer view fact composes across local copy, shared record field projection, direct call parameter, `ensures result == value` call result, `match` arm join, `const SEVEN` binding, authenticated state transition arrival, inter-machine call with propagated `crashes Trap value == 0`, and target-qualified machine bodies (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to discharge `maybe_zero`'s `crashes Trap value == 0` guard without a caller crash ceiling.
- **Soundness boundaries:** checked recast admission enforces equal size, alignment, authority, and representation validity (`controls/incompatible_view` rejects `u8` → `bool`), and no proof-only binding authorizes an executable read.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** representation views and guarded calls lower to canonical target-neutral Terminal Psi (`PSITERM\0`) and execute with exit `0` in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `Build::bind` settles `ProgramEntry` across all four hosted targets and validates 64-bit integer layout geometry and calling plans across scalar and `&Packet` parameters.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** bit-preserving 64-bit integer view reads, comparisons, and calls select native `x86_64` and `arm64` instructions with bounded stack-frame activation storage.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** native images emit and execute with exit `0` on `windows_x86_64` and `linux_x86_64` in agreement with Terminal Psi (`omega run --both`).
- **Remaining open under `CRASH-CONTRACT`:** nonempty-path lengths and policy casts remain open on `TASKS.md`. The entry-byte Boolean observation is `facts/crash_entry_boolean_view`. The IEEE bit view is `facts/crash_entry_ieee_view`.
