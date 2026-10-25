# Temporary case cleanup

**Bonus.value == 2 survives each place into an affine case common field with
omitted zero storage and normal shared-call cleanup.**

Every place supplies the same copyable `Bonus { value: 2 }` to `consume`, which
constructs an affine `Event::Treasure` with an omitted zero common field and
borrows it for a scalar-result call. Each `invalid/` twin changes one supplied
`Bonus.value` from 2 to 1 and retains the required bound and operation.

## Places

The nine places carry the `Bonus.value == 2` fact into an affine case temporary and verify its normal cleanup:

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | original witness: short-circuiting, once-only ordered child calls, and case temporary cleanup |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | Local binding |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | Enclosing record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | Required parameter fact |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | Guaranteed result fact |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | Both selected result arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | Named copyable `Bonus` constant |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | Nested machine parameter |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | Separate producer and consumer |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | Target-selected producer |

Run a positive with `omega run --both <place>/main.omg`; both native and Terminal
execution must exit 0. Check its `invalid/main.omg` and compare the authored
`expected.txt` fragment. The const place transports `Bonus`, not an affine
`Event`.

The unchanged root `main.omg` additionally checks once-only ordered child calls:
short-circuiting skips the first construction, then two child calls increment a
counter before the constructed case is borrowed and cleaned up. It must also
exit 0 in native and Terminal execution. The unchanged full regression fixture
`tests/omega/pass/data/runtime_mixed_case_nested_common_fields_exit` retains its
arrays, erased fields, active payload and short-circuit paths.

Cleanup uses exact checked dying-owner identities. A contained omitted zero
field may share its constructor's source expression; it is not another owner.
The focused drop fixtures also check nested backing, linear claims, projected
residuals, and observable reverse completed-owner hook order. This bounded repair
does not close the broader CML4 ownership acceptance.

## Requirements and Evidence

Owning spec: [ownership and borrowing](../../../../wiki/spec/language/ownership.md), [layouts and plans](../../../../wiki/spec/layouts/plans.md), and [nominal cleanup](../../../../wiki/spec/terminal-psi/ownership.md#nominal-cleanup).
This row establishes deterministic affine cleanup of temporary tagged case values
with omitted zero common fields across call expressions and control-flow boundaries.

Full 8-layer specification and lowering evidence:

- **Establishment:** Constructing or matching an affine temporary tagged case value (`Event::Treasure { gold: 1, details: value }`) whose common field `omitted == 0` is omitted from storage and whose payload owns `Bonus { value: 2 }` establishes deterministic cleanup of unconsumed/temporary case payloads when the expression or enclosing scope ends, without any post-hoc caller equality guard (`transition x == 100`).
- **Identity and lifetime:** The temporary case payload's resource/loan identity remains live for the temporary's exact lifetime across the shared borrower call (`observe(&Event)`), releasing in deterministic reverse establishment order upon call completion and statement end.
- **Composition:** Temporary case payload cleanup and its prerequisite fact compose across all nine standard places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`). In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the supplied fact (`value: 1` vs `value: 2`) violating the cleanup or call prerequisite while leaving the construct, bound, and operation identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time (`--check`) when the temporary case payload's prerequisite is not satisfied (`value.value == 2`), rejecting with their authored diagnostic (e.g. `cannot prove requires contract for call consume` or `violates required fact `value.value == 2``). Unsound or unproven payload values cannot enter the temporary case constructor or bypass cleanup obligations.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Temporary case payload scope exit lowers into explicit variant-conditional `DiscardResidual` / `InvokeNominal` / `RetireHome` affine cleanup actions and borrow restoration windows in canonical target-neutral Terminal Psi (`PSITERM\0`), erasing omitted zero-byte fields while preserving runtime tag and payload layout, and executes identically in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Tagged sum/case layout geometry validates: omitted zero-sized common fields (`omitted == 0`) consume no physical storage, while variant tags and active payloads (`gold: u64`, `details: Bonus`) align correctly according to target layout plans. `CallPlan`/`StatePlan` lower case values, loans, and cleanup actions across registers and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects tag checks, payload field projections, and cleanup routines, assigning register homes and disjoint stack-frame homes for temporary case slots so temporary payloads do not clobber live caller or callee variables across calls. Entry prologues and multi-return restoration epilogues strictly honor computed frame ceilings.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve for temporary case storage and projected payload references, basic blocks and cleanup branches relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

## What remains open

Still open under CML4: nested dynamic variant payloads, arbitrary runtime-sized
union payloads, unselected case drop dispatch, and cross-host native qualification
for non-hosted targets.

```sh
omega run --both samples/cli/facts/temporary_case_cleanup/local/main.omg
omega --check samples/cli/facts/temporary_case_cleanup/local/invalid/main.omg
python3 tools/sample_gate.py --filter cli/facts/temporary_case_cleanup --cli <omega>
python3 tools/sample_native_gate.py --filter cli/facts/temporary_case_cleanup --cli <omega>
python3 tools/corpus_gate.py --filter temporary_case_cleanup
python3 tools/corpus_gate.py --native --filter temporary_case_cleanup
python3 tools/corpus_gate.py --both --filter temporary_case_cleanup
```
