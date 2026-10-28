# Prefix case identities

Numbered mixed data preserves the active payload bound, common field and neighbor canaries; schema case 90 is authored first and remains the ordinary zero case.

This row belongs to **PREFIX-CASE-IDENTITIES** in [TASKS.md](../../../../TASKS.md).
It follows [construction and case identity](../../../../wiki/spec/language/data_and_literals.md#construction-and-case-identity)
and [layout identity scopes](../../../../wiki/spec/layouts/plans.md).

The mixed carrier has common field #1, first-authored case #90 Sample with
payload field #1 and payload tombstone #2, then case #7 Empty. Common and payload
numbering are independent. The payload uses the ordinary scalar domain
PayloadValue, requiring value <= 50; there is no case-local where clause.

The consumer checks the transported common field and matches the active case.
Sample adds 10 and proves result <= 60. Empty returns 3. Every entry checks
Sample 32/common 91 gives 42, Empty/common 73 gives 3, and an ordinary zero home gives
first-authored Sample 0/common 0 and result 10. The neighboring 305419896 and
1515863100 values also survive. These distinct results detect case/payload/common
field corruption and confusing schema identity with the zero discriminator.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [main](local/main.omg) | [invalid](local/invalid/main.omg) | local value binding |
| field | [main](field/main.omg) | [invalid](field/invalid/main.omg) | record field with two canaries |
| parameter | [main](parameter/main.omg) | [invalid](parameter/invalid/main.omg) | declared Reading parameter |
| call result | [main](call_result/main.omg) | [invalid](call_result/invalid/main.omg) | returned Reading value |
| match join | [main](match_join/main.omg) | [invalid](match_join/invalid/main.omg) | distinct Sample and Empty values through both arms |
| const name | [main](const_name/main.omg) | [invalid](const_name/invalid/main.omg) | named Sample and Empty constants |
| nested machine | [main](other_state/main.omg) | [invalid](other_state/invalid/main.omg) | explicit Reading parameter to nested consume |
| another machine | [main](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) | separate consuming machine |
| selected target body | [main](target_body/main.omg) | [invalid](target_body/invalid/main.omg) | one ordinary SelectedEntry implementation selected by Build |

Every build binds all four hosted ProgramEntry targets. Each twin changes only
the PayloadValue bound from 50 to 80, preserving the numbers, ordering, tombstone,
constructed values, canaries and consuming result <= 60 requirement. All nine twins reject with "cannot prove ensures contract" for the consuming
result bound; their original source and runtime checks remain pinned.

The const-name place transports real named Sample/Empty values. Its ordinary
zero home is a shared control; this does not claim an uninitialized const form.

## Scoped controls

- [Pure sum retirement](controls/pure_sum_retirement/main.omg) keeps a case
  tombstone and a same-numbered payload tombstone in independent scopes, with
  distinct First/Later outcomes and a first-authored zero check.
- [Empty reference array](controls/empty_reference_array/main.omg) establishes
  a zero-length payload without inventing a reference element.
- [Contextual field](controls/contextual_case_field/main.omg) preserves a
  numbered ordinary field named case and its neighbor.
- [Old order](controls/retired_case_order/main.omg) and
  [case block](controls/case_block/main.omg) reject the retired declaration forms.
  The old-order negative is intentionally the only new source using that spelling.
- [Partial numbering](controls/partial_case_identities/main.omg),
  [retired payload reuse](controls/retired_payload_reuse/main.omg), and
  [ambiguous mixed retirement](controls/ambiguous_mixed_retirement/main.omg)
  preserve the existing scope restrictions. The mixed row itself places no
  ambiguous outer tombstone.

The nine positives and twins are pinned under
tests/omega/{pass,fail}/facts/prefix_case_identity_<place>. Controls are pinned
under tests/omega/{pass,fail}/data/prefix_case_*.

## Verification frontier

Optimized Rust producer `22662ce1c2593ab013a93819659a2fb125beaa48` checks all
nine places and the three positive controls, rejects all nine twins and five
negative controls for their authored reasons, and runs all twelve positive
programs with native exit 0 and Terminal exit 0 on macOS arm64. This repairs the
initial empty zero-payload home and named mixed-constant failures. The four
Build bindings do not claim execution on the other hosts.

The parent remains open until the Omega-written parser/resolver gates run.
Their current harness clears source checking but native lowering refuses the
structural result of `Console::read_byte`; that compiler prerequisite is under
repair. No product-parser runtime acceptance is claimed yet.

| Requirement | Witness and remaining verification |
| --- | --- |
| Establishment | Ordinary payload domain and complete mixed construction; twin widens only that domain. |
| Identity and lifetime | Independent common/case/payload scopes and exact active case; no extra lifetime claim. |
| Composition | Same mixed carrier and bound across the nine places above. |
| Soundness | Twin result-bound refusal and scoped syntax/identity rejects. |
| Psi lowering and Terminal | Retain common field, active payload and zero's authored case; all nine Terminal executions pass. |
| Layout and ABI | Mixed carrier and canaries pass through calls/results; stable IDs must not become runtime ordinals. |
| Instruction selection | All nine native executions meet their authored branch/common/payload oracles. |
| Encoding and emission | Actual native/Terminal agreement on matching hosts is required; four Build bindings alone are not execution evidence. |
