# Captured float meaning result

The minimum call's checked IEEE meaning survives transport and discharges `require_min` for its exact captured result.

This row qualifies the named-operator write-origin prerequisite of **R5** in `TASKS.md`. The source imports the selected float operator roster explicitly. `forward` returns `F32::minimum(left, right)` and proves its declared `FloatMeaning` equation. The caller transports that captured result, then consumes the equation in `require_min`. Every invalid twin changes only the implementation expression from `F32::minimum` to `F32::maximum`, retaining the minimum guarantee and its consumer. It must reject with `ensures contract proof fact`.

The repair shares the ordinary exact operator selection, formal operand types, chronological origin capture and signature write ceiling. It adds no float-specific proof rule. The declaration namespace is not runtime receiver storage; treating it as an unknown receiver had made the empty completion frame opaque and discarded the equation's input observations. Mutable explicit arguments and operand effects still contribute their full writes.

| Place | Positive | Invalid twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

All positive projects bind `ProgramEntry` for all four hosted targets. On the available Linux x86-64 host, `omega run --both` must report native and Terminal Psi exit `0`; the runtime branch checks the returned value is `1.0`. Each matching pass/fail fixture under `tests/omega/{pass,fail}/facts/float_meaning_result_through_*` retains byte-identical source and build declarations.

1. **Establishment.** The selected minimum operation supplies its checked public normal-return equation; `forward` must prove it. Maximum cannot establish that equation for arbitrary inputs.
2. **Identity and lifetime.** The equation names the captured result and operand observations. An empty completion write frame preserves those observations without making runtime calls repeatable or granting Float substitution identity.
3. **Composition.** The same equation reaches a local, record field, parameter, call result, match join, named constants, successor state, forwarding machine and target body. `require_min` consumes it without a new guard establishing the fact.
4. **Soundness boundaries.** The negative twins retain the claimed minimum equation but select maximum. Focused mutable-argument and effectful-operand controls retain invalidation independently of this scalar row.
5. **Checked-to-Terminal Psi (Psi 05–07).** The equation justifies the consumer; the actual minimum and result branch execute under the interpreter.
6. **Provider settlement, layout and ABI (Omega 00–02).** Hosted entry bindings and ordinary f32 value lanes follow the selected target program.
7. **Instructions, register homes and activations (Omega 03–06).** Native execution computes minimum and transports the returned scalar through each source place.
8. **Encoding and native image emission (Omega 07–09).** Native and interpreter exit zero agree for every positive. No execution is claimed for unavailable foreign hosts.

This is a parser-admission prerequisite witness, not a claim that this row alone completes R5. The finite-cycle and other may-write rows remain required, as does ordinary parser package admission and convergence.
