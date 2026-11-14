# Read a field owned by a selected method's input

Owning task: **FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../TASKS.md), specifically structural field reads rooted in entry content claims.

Fact: A finite-family call reads the original nested scalar beneath its owned input's content claim.

Each program passes a packet by value through a runtime-selected method at widths 16 and 32. The packet contains a linear `Token` qualified by `Token::Owned`. Its content projection declares one counted logical unit. This establishes a nonempty claim for the nested token and grants no physical storage authority. The surrounding scalar fields use 17 and 23; the token values are 103 and 107.

The method receives a selector, an ordinary scalar and the packet. The compiler must associate the nested claim with the packet despite the preceding scalar arguments. The packet's default multiplicity cannot erase the linear obligation in its token. The implementation reads `input.owned.value`, explicitly drops the token, and returns the saved scalar.

The field's declared range proves the public result is at least 100. The caller subtracts 100 before any result guard. Runtime checks require differences 3 and 7 for both widths. A constant answer, stale input or incorrect field offset fails these checks. Both specializations perform the same read. The empty cleanup hook supplies no observable cleanup count.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| Local | [program](local/main.omg) | [twin](invalid_local/main.omg) | A local packet supplies the call. |
| Field | [program](field/main.omg) | [twin](invalid_field/main.omg) | An enclosing record stores the packet. |
| Parameter | [program](parameter/main.omg) | [twin](invalid_parameter/main.omg) | The checking machine receives the packet as a parameter. |
| Call result | [program](call_result/main.omg) | [twin](invalid_call_result/main.omg) | A constructor call returns the packet. |
| Match join | [program](match_join/main.omg) | [twin](invalid_match_join/main.omg) | The arms construct the same packet shape from different lawful field values. Choosing the wrong arm changes the observed difference. |
| Const name | [program](const_name/main.omg) | [twin](invalid_const_name/main.omg) | Each constant initializer constructs a token, performs the selected read and consumes the token; only the resulting scalar becomes a constant. The same reads also run at runtime. |
| Nested machine | [program](other_state/main.omg) | [twin](invalid_other_state/main.omg) | A transition transfers the packet to a nested machine. |
| Another machine | [program](other_machine/main.omg) | [twin](invalid_other_machine/main.omg) | A separate machine forwards the packet to dispatch. |
| Target body | [program](target_body/main.omg) | [twin](invalid_target_body/main.omg) | Build selects the provider that performs dispatch. |

Every twin changes only the field domain's lower bound from 100 to 0. The same inputs still form, but an arbitrary field of the weaker type cannot prove the method's unchanged result guarantee. The intended diagnostic is `cannot prove ensures contract`. No caller guard substitutes for the guarantee. Sample and corpus Main/Build files match.

The nearest rows are [returned descriptors](../finite_family_returned_descriptor/README.md), which check public borrowing origins, and [exclusive results](../finite_family_exclusive_result/README.md), which check consumption of a returned counter owner. Neither establishes this scalar read under a nested nonempty input claim after scalar arguments.

The governing rules are [dynamic call and claim paths](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#call-and-forwarding-roles), [content custody](../../../../wiki/spec/resources/content_custody.md), [finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary), [constants](../../../../wiki/spec/language/constants.md) and [nominal cleanup](../../../../wiki/spec/language/ownership.md).

Construction, the declared field domain and the exact content projection establish the fact. Identity requires the selected method to read each original value. Composition requires the public result bound to justify subtraction without relying on a private guarantee. Checking must preserve ownership and cleanup while rejecting the weakened-domain twin. The match expects a different field value from each arm and exercises both values at both widths. Constant evaluation performs the whole read and cleanup within the admitted invocation; no constant holds a linear token or storage authority. Terminal execution, layouts, call argument positions and native execution must retain that behavior.

All nine pairs are pinned under `tests/omega/{pass,fail}/facts/finite_family_owned_field/`. Each measured host checks the nine positives, rejects all nine twins with `cannot prove ensures contract`, and runs the nine positive fixtures at exit 0 in both Terminal Psi and native code. The eighteen public sample projects have the same nine executions and nine intended rejections.

| Host | Run | Actual optimized compiler | Program source | Result |
| --- | --- | --- | --- | --- |
| Linux x86-64 | [41880969570](https://github.com/CathedralOS/Omega/actions/runs/41880969570) | `83e3d72bad8bd` | `17be13e1a9ff` | All cases pass. |
| Linux ARM64 | [41887635893](https://github.com/CathedralOS/Omega/actions/runs/41887635893) | `17be13e1a9ff` | `17be13e1a9ff` | All cases pass. |
| macOS ARM64 | [41887658330](https://github.com/CathedralOS/Omega/actions/runs/41887658330) | `17be13e1a9ff` | `17be13e1a9ff` | All cases pass. |
| Windows x86-64 | [41887643715](https://github.com/CathedralOS/Omega/actions/runs/41887643715) | `17be13e1a9ff` | `17be13e1a9ff` | All cases pass. |

Linux x86-64 reuses the retained optimized compiler from published `83e3d72bad8bd79a17111831811aaf3b975af8ab`. Its compiler, library, harness and configuration Git objects equal those at program source `17be13e1a9ff1489ac3a0422c1e011cef9591898`; the original producer identity is preserved. The other measured hosts build that program source directly. Compiler profiles, captured input hashes, exact rejection causes and both actual engine exits are verified. The Mac run also contains a separately audited atomic scope; this table describes only the field-read cases.

The [earlier full-row run](https://github.com/CathedralOS/Omega/actions/runs/41866026571) used producer `972297978286bb346cb46a4a7979286f22c6920f` and program source `b59f21a7b83951962e44e4d104342ff4c413eaa4`. Eight places and their twins passed. The target pair stopped at an authoring error: its boundary method omitted the explicit empty `reaches` declaration. PR #1827 published that earlier failure; this follow-up corrects the declaration without changing the field, ownership, result guarantee or runtime checks.

The shared call-effect repair in [PR #1826](https://github.com/CathedralOS/Omega/pull/1826) is a prerequisite. A known call with no required write slots now records that empty list explicitly, while unknown call effects still refuse. The [focused Linux run](https://github.com/CathedralOS/Omega/actions/runs/41860200412) exercised the local field case and retained the relevant refusal controls.

The Source records update only the corrected target pair. The other sixteen Source entries retain their earlier observations. Host execution remains in the linked reports; no duplicate per-host record rows are added. The parent task remains open for target eligibility and const-only bridges. No language-design decision is needed.
