# Runtime index application

A runtime limit from 9 to 10 binds `Index<limit>`, retaining payload 8 below the exact captured limit.

The operation is `let indexed: Index<limit> = Index{value:8}` under
`limit >= 9 && limit <= 10`. Construction must prove `value < Limit`.
The bound belongs to the runtime subject; it does not select a const
specialization. Every invalid twin changes only the constructed payload from
8 to 10, retaining the bound, consuming operation and runtime oracle.

The original root is preserved. Its baseline at
`fd367363d34c9c1e082bd51eafad81c57311a006` rejected at the construction with
"data `Index` literal cannot prove the default domain". The const-binder and
ordinary-record controls checked; supplying the descriptor explicitly did not
repair the runtime-indexed construction. All nine places reproduced this refusal.
The same baseline incorrectly accepted payload 10 when a global constant named
`Limit` supplied 100 after ambiguous local lookup. The repaired scope rejects
that construction against its actual captured runtime limit.

## Places

| Place | Positive | Reject twin | Transport / Carrier |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | root runtime parameter entry requirement |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | local copy of the admitted runtime subject |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | `Bounds` field domain, read inside a separate machine |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | runtime parameter's entry requirement |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | completed call's result guarantees |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | either bounded input; both arms execute |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named bounds 9 and 10 on a runtime parameter |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | checked arrival and state entry requirement |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | helper call substitutes its exact actual |
| target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | each of four target bodies proves construction |

Mirrored corpus pairs live at
`tests/omega/pass/facts/runtime_index_application_through_<place>_exit` and
`tests/omega/fail/facts/runtime_index_application_through_<place>_rejected`.
Their sources and builds match the sample pairs byte for byte. Every positive
and root binds all four hosted `ProgramEntry` targets (`windows_x86_64`, `linux_x86_64`,
`macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`.
The final `first == 8 && second == 8` check observes the result; it supplies no
premise to the earlier construction. No caller guard repairs a missing fact.

## Requirements and Evidence

- **Parent Outcome:** `RUNTIME-VALUE-GENERICS` (`TASKS.md`).
- **Requirement Established:** Default-domain construction of fixed-representation data using a directly named runtime scalar index (`Index<limit>` with payload `8 < Limit` where `limit >= 9 && limit <= 10`), transported through all nine places and agreeing under Terminal Psi and native execution.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** The admitted lower bound (`requires limit >= 9 && limit <= 10`) establishes that payload 8 satisfies the runtime index default domain (`value < Limit`) directly upon construction (`let indexed: Index<limit> = Index { value: 8 };`), without any post-hoc caller equality guard (`transition ... == 8`). The constructor's domain is satisfied by the captured runtime parameter's precondition and never serves as its own premise.
- **Identity and lifetime:** The runtime index fact stays bound to the exact scalar subject and generic binder identity across local copy, field projection, parameter passing, call results, match joins, const bounds, state transitions, forwarding machines, and target bodies. In type positions, the generic binder retains its type parameter identity; in data predicates, the generated leading descriptor field supplies the captured scalar value. Reassignments and unrelated writes do not invalidate the captured index fact.
- **Composition:** The runtime index application fact composes across all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming value extraction (`indexed.value`). Each `<place>/main.omg` vs. `<place>/invalid/main.omg` differs only in the construct's bound or index argument (payload 8 in positive witnesses vs. payload 10 in invalid twins).
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed with default-domain violations (`error: data \`Index\` literal violates the default domain: a \`where\` fact evaluates FALSE at this construction`) because payload 10 cannot satisfy `10 < Limit` for any admitted `Limit <= 10`. The scope resolution prevents a same-named global constant `Limit = 100` from shadowing or bypassing the captured descriptor.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Runtime index application lowers without semantic loss into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof obligations while supplying the captured scalar as a leading descriptor field in the record carrier without introducing runtime proof witnesses, boxing, or per-index code specialization, executing with full agreement in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), record layout geometry validates (`Index<Limit: u32>` with `value: u32` 32-bit scalar layout, 4-byte size and alignment), and `CallPlan`/`StatePlan` lower arguments and return values across target ABI register and stack placements.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for scalar parameter passing, register-to-register moves, and comparison/transition branching. Backend assigns register homes and disjoint stack-frame storage, composing bounded activation frames and inserting entry prologues and epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve, basic blocks and branches relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across hosted targets including `windows_x86_64` and `macos_arm64`.

### Verification Evidence

- `python3 tools/sample_native_gate.py --cli <shared producer> --filter cli/facts/runtime_count_index,cli/facts/runtime_index_application` at base `5e6e565d` on `macos_arm64` (2026-10-22): 20/20 samples meet their documented oracle (this row's root and nine places with native exit 0); 27/27 intended-invalid controls match their authored rejection, including this row's 9/9 (`data \`Index\` literal violates the default domain`); `omega run --both` reports Terminal and native agreement (exit 0) on all ten positives of both rows.
- The nine pass fixtures are recorded in `tests/omega/corpus_native_macos_arm64.txt` (native `exit:0`) and `tests/omega/corpus_both_macos_arm64.txt` (`exit:0 interpreter-exit:0 interpreter:agrees`); the nine reject fixtures are recorded in `tests/omega/corpus_outcomes.txt` with their default-domain diagnostic.
- Focused controls additionally challenge a second runtime descriptor and a same-named global constant: `tests/omega/pass/generics/runtime_index_predicate_second_descriptor_exit` and `tests/omega/fail/generics/runtime_index_predicate_global_shadow`.
- The oracle discriminates: mutating the payload or expectation breaks the `first == 8 && second == 8` guard, transitioning to `bad()` and trapping with `crash Trap`, proving the exit 0 oracle actively verifies the execution.
- Remaining open: runtime argument forms, general parameter/result/field qualification forwarding through reassignment and guards, and linear custody coverage beyond fixed scalar constructors under `RUNTIME-VALUE-GENERICS`.

### Canonical Contracts

- [Value binders and const requirements](../../../../wiki/spec/language/generics.md#value-binders-and-const-requirements)
- [Runtime index identity and storage](../../../../wiki/spec/language/generics.md#runtime-index-identity-and-storage)
- [Establishment and witness loans](../../../../wiki/spec/language/dependent_values.md#establishment-and-witness-loans)
- [Default domains and zero initialization](../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization)


The [forwarded production](forwarded/README.md) carries this same fact through
runtime-valued generic results and inferred member-call temporaries. Its nine
places and reject twins extend this folder's production coverage; they do not
close the parent outcome's remaining computed-index or effectful compositions.

The same fact also covers [direct computed results](computed_result/README.md), [qualified computed carriers](qualified_computed/README.md), and [mutating selected operators](mutating_operator/README.md). Their nine-place productions retain the bound and payload above. The named direct-result probe originally refused its constructor proof on optimized source `219b21966eb`; the qualified recipe originally lost its resolved cast context, and the explicit-reference token probe checked but had no executable body. These are implementation gaps under the existing generics and evaluation contracts. Current candidate-bound reports retain the actual source, producer and host observations.
