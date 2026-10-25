# Static boundary request origins

A fresh request's zero field survives transport to the static boundary call's precondition.

This **R5** parser-admission prerequisite follows [zero initialization](../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization) and [invocation contracts](../../../../wiki/spec/language/state_contracts.md). `Request` has an ordinary zero-constructible `u64` field. `Io::acquire` requires it to equal `ZERO`; the selected source provider actually writes `9` and returns `7`. Every positive checks the returned status at runtime. Every invalid twin changes the request field to `1` before the same call and must reject with `cannot prove requires contract`.

The compiler uses the boundary write owner's exact signature and runtime-receiver classification. A static namespace has no receiver storage to poison the completed operand prefix. Explicit mutable arguments remain writable, and a provider object's runtime receiver retains its ordinary origin checks. No boundary implementation body is used to strengthen the public contract.

| Place | Positive | Changed-field reject |
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

All positive projects select the source provider and bind `ProgramEntry` for the four hosted targets. `omega run --both` on Linux x86-64 must report native and Terminal Psi exit `0`. Matching fixtures under `tests/omega/{pass,fail}/facts/boundary_zero_origin_through_*` preserve byte-identical sources and build declarations. No foreign host execution is claimed.

1. **Establishment.** Fresh zero construction, a proved returned record or the receiving helper's explicit entry contract establishes the field equation. The changed-field twin cannot consume it.
2. **Identity and lifetime.** The precondition observes the same live request field at invocation. It grants no post-call preservation through `&mut Request`; a focused stale post-call consumer rejects.
3. **Composition.** The request reaches the boundary through a local, record field, helper parameter, returned record, match join, named zero, successor state, forwarding machine or target body. The consumer supplies no new guard proving the field zero.
4. **Soundness.** Boundary signature selection remains exact; argument effects and mutable writes remain visible. Focused changed-before-call and effectful-argument controls reject at acquire, and post-call stale-field use rejects at its later consumer.
5. **Checked-to-Terminal Psi (Psi 05–07).** The precondition is checked before invocation; Terminal executes the selected source provider's real mutation and return.
6. **Provider, layout and ABI (Omega 00–02).** `Build::select_provider<Io, Actual>` supplies the ordinary checked provider, and the mutable record uses its actual reference lane.
7. **Instructions, registers and activations (Omega 03–06).** Native execution invokes that provider and consumes its returned status in each transport variation.
8. **Encoding and emission (Omega 07–09).** Every positive native image exits zero, agreeing with Terminal Psi.

The row qualifies one general origin-frame repair required by ordinary parser admission. It does not replace the remaining R5 may-write matrices or parser convergence gate.
