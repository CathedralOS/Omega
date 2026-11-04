# Read a field owned by a selected method's input

Owning task: **FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../TASKS.md), specifically structural field reads rooted in entry content claims.

Fact: A finite-family call reads the original nested scalar beneath its owned input's content claim.

Each program passes a packet by value through a runtime-selected method at widths 16 and 32. The packet contains a linear `Token` qualified by `Token::Owned`. Its content projection declares one counted logical unit. This establishes a nonempty claim for the nested token and grants no physical storage authority. The surrounding scalar fields are 17 and 23; the token values are 103 and 107.

The method receives a selector, an ordinary scalar and the packet. The compiler must associate the nested claim with the packet despite the preceding scalar arguments. The packet's default multiplicity cannot erase the linear obligation in its token. The implementation reads `input.owned.value`, explicitly drops the token, and returns the saved scalar.

The field's declared range proves the public result is at least 100. The caller subtracts 100 before any result guard. Runtime checks require differences 3 and 7 for both widths. A constant answer, stale input or incorrect field offset fails these checks. Both specializations perform the same read. The empty cleanup hook supplies no observable cleanup count.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| Local | [program](local/main.omg) | [twin](local/invalid/main.omg) | A local packet supplies the call. |
| Field | [program](field/main.omg) | [twin](field/invalid/main.omg) | An enclosing record stores the packet. |
| Parameter | [program](parameter/main.omg) | [twin](parameter/invalid/main.omg) | The checking machine receives the packet as a parameter. |
| Call result | [program](call_result/main.omg) | [twin](call_result/invalid/main.omg) | A constructor call returns the packet. |
| Match join | [program](match_join/main.omg) | [twin](match_join/invalid/main.omg) | Both arms construct the same packet shape. |
| Const name | [program](const_name/main.omg) | [twin](const_name/invalid/main.omg) | Copyable constant seeds supply the qualified scalar; runtime construction establishes the token's claim. |
| Nested machine | [program](other_state/main.omg) | [twin](other_state/invalid/main.omg) | A transition transfers the packet to a nested machine. |
| Another machine | [program](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) | A separate machine forwards the packet to dispatch. |
| Target body | [program](target_body/main.omg) | [twin](target_body/invalid/main.omg) | Build selects the provider that performs dispatch. |

Every twin changes only the field domain's lower bound from 100 to 0. The same inputs still form, but an arbitrary field of the weaker type cannot prove the method's unchanged result guarantee. The intended diagnostic is `cannot prove ensures contract`. No caller guard substitutes for the guarantee. Sample and corpus Main/Build files match.

The nearest rows are [returned descriptors](../finite_family_returned_descriptor/README.md), which check public borrowing origins, and [exclusive results](../finite_family_exclusive_result/README.md), which check consumption of a returned counter owner. Neither establishes this scalar read under a nested nonempty input claim after scalar arguments.

The governing rules are [dynamic call and claim paths](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#call-and-forwarding-roles), [content custody](../../../../wiki/spec/resources/content_custody.md), [finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary), [constants](../../../../wiki/spec/language/constants.md) and [nominal cleanup](../../../../wiki/spec/language/ownership.md).

Construction, the declared field domain and the exact content projection establish the fact. Identity requires the selected method to read each original value. Composition requires the public result bound to justify subtraction without relying on a private guarantee. Checking must preserve ownership and cleanup while rejecting the weakened-domain twin. Terminal execution, layouts, call argument positions and native execution must retain that behavior.

All nine pairs are authored and pinned under `tests/omega/{pass,fail}/facts/finite_family_owned_field/`. Qualification remains pending. The first local probe uses optimized producer source `972297978286bb346cb46a4a7979286f22c6920f`; it claims no result until its raw observations are inspected. The other eight places require the same validation, followed by all four matching hosts, independent review and normal landing. Parent acceptance remains open. No language-design decision is needed.
