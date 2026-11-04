# Read a field owned by a selected method's input

Owning task: **FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../TASKS.md), specifically structural field reads rooted in entry content claims.

Fact: A finite-family call reads the original nested scalar beneath its owned input's content claim.

The [local program](local/main.omg) passes a packet by value through a runtime-selected method at widths 16 and 32. The packet contains a linear Token qualified by Token::Owned. Its owner declares one counted unit of content, so the nested entry claim is nonempty. These are logical units; the qualification grants no access to physical storage. The surrounding scalar fields are 17 and 23, and the actual nested values are 103 and 107.

The selector and an ordinary scalar prefix precede the packet. The public slot must retain the exact nested claim at structural argument zero; neither scalar occupies that index. The packet itself has default multiplicity, so its root cannot replace the roster of its linear descendants. The implementation reads input.owned.value and explicitly disposes the token through its declared nominal cleanup.

The field's declared range proves the public result is at least 100. The caller subtracts 100 directly, before any result guard. Runtime checks require differences 3 and 7 for both widths. A constant answer, stale input or missing nested field offset fails those checks. Both specializations perform the same read; this fact does not establish different width-dependent behavior. The empty cleanup hook makes no runtime once-only cleanup claim.

The [reject twin](local/invalid/main.omg) changes only the field domain's lower bound from 100 to 0. The same authored inputs still form, but reading an arbitrary field of that weaker type cannot prove the method's unchanged result guarantee. A precondition or caller guard is not added. Sample and corpus Main/Build files match.

The nearest rows are [returned descriptors](../finite_family_returned_descriptor/README.md), which check public borrowing origins, and [exclusive results](../finite_family_exclusive_result/README.md), which check explicit once-only consumption of a returned counter owner. Neither proves this scalar read under a nested nonempty input claim after scalar arguments.

The governing rules are [dynamic call and claim paths](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#call-and-forwarding-roles), [content custody](../../../../wiki/spec/resources/content_custody.md), [finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary) and [nominal cleanup](../../../../wiki/spec/language/ownership.md).

Establishment comes from construction, the declared field domain and the exact Content projection. Identity requires the selected method to read each original nested value. Composition requires the public result bound to justify subtraction without using a private guarantee. Source checking must preserve ownership and actual cleanup while rejecting the weakened-domain twin. The checked program, Terminal execution, layouts, argument/claim positions and matching-host native result must agree.

Only the first local pair is authored. Its optimized fixed-source baseline is pending; no compiler defect, successful check, runtime result or intended refusal is claimed yet. The remaining eight places, complete reject pins, four-host execution, exact producer evidence, independent review and normal landing remain required. Parent acceptance stays open; no language-design decision is needed.
