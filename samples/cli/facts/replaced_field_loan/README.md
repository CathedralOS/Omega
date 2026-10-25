# Replaced field loan

A reborrow after stored descriptor replacement writes 41 to incoming backing while preserving original 17 and sibling 7.

Owner: **STATE-LOCAL-VALUE-FRONTIER**, aggregate replacement and reads.
**MATCH-SELECTIVE-LOWERING** supplies the ordinary owned match join. The same
completed descriptor must survive all nine places; its reference-shaped type
alone supplies neither a backing address nor exclusive access.

## Witnesses

All positives declare expected exit 0 and bind all four hosted targets. Each
reject twin changes only `Message.body` from mutable to shared access. The
consuming mutable store remains unchanged.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [shared descriptor](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [shared descriptor](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [shared descriptor](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [shared descriptor](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [shared descriptor](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [shared descriptor](const_name/invalid/main.omg) |
| A nested machine | [program](other_state/main.omg) | [shared descriptor](other_state/invalid/main.omg) |
| Another machine | [program](other_machine/main.omg) | [shared descriptor](other_machine/invalid/main.omg) |
| Another target body | [program](target_body/main.omg) | [shared descriptor](target_body/invalid/main.omg) |

The pairs are pinned in `tests/omega/{pass,fail}/facts/` as
`replaced_field_loan_through_<place>_{exit,rejected}`.

## Requirement evidence

| Requirement | What this row establishes |
| --- | --- |
| Establishment | Construction establishes the old descriptor; the completed replacement installs the incoming descriptor. The consuming store uses its existing exclusive access. The final guard observes results and creates no loan authority. |
| Identity and lifetime | Descriptor identity and pointee contents have separate immutable storage histories. Replacing `body` retires its old occurrence; a captured parent pointer still observes the new child. An inline copied carrier keeps its own earlier descriptor. |
| Composition | Locals, fields, parameters, call results, owned match joins, const names, states, machines and target bodies carry the same replacement into the store. Every incoming storage predecessor must supply complete evidence. |
| Soundness boundaries | Shared access cannot supply the exclusive store. Outstanding loans still exclude moving their backing; a completed temporary call borrow does not become an eternal loan. Receiving output attribution uses the exact storage owner, path and producing statement. |
| Psi 05–07 | Checked capture, current-slot observation, borrowed cleanup and selected ownership lower through Terminal Psi. `omega run --both` checks its execution against native execution. |
| Omega 00–02 | Provider settlement, record layout and ordinary call plans preserve the actual descriptor and its distinct old/new backing. This row does not qualify every layout or ABI lane. |
| Omega 03–06 | Native instruction, register and frame realization preserves the replacement write and untouched backing on macOS AArch64. |
| Encoding/emission | The emitted Mach-O executes with exit 0 and agrees with Terminal Psi. Windows native execution is owner-deferred; Linux native execution is unverified. Four hosted bindings do not establish runtime acceptance on those hosts. |

The [lifetime replacement rule](../../../../wiki/spec/language/lifetimes.md),
[evaluation schedule](../../../../wiki/spec/language/expressions.md),
[saved-subject semantics](../../../../wiki/spec/language/patterns.md) and
[storage fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md)
govern this row.

Related controls in [case_match_payload_loan](../case_match_payload_loan/README.md)
exercise guard-time replacement, a held parent pointer, constructed child
borrows, callee-local aliases, an owned argument carrying a pointer, and private
replacement of a copied argument. Live call-installed carriers, including a
forwarded alias after an older loan's last use, still reject an owned move;
using the installed carrier before its last use permits the later move. Returned
descriptor controls also distinguish sibling storage, conditional origins and
a nonzero window inside a nested array. Qualified helper calls retain the
explicit argument roster independently of their namespace receiver, for both
value-producing calls and explicit statement discards. An inactive case payload
in unrelated completed storage does not create a phantom outstanding loan.

## Scope

This row discharges current-field descriptor replacement and reborrow transport.
The parent capabilities retain their other aggregate, opaque-input, borrowed
join and theorem/quotient correspondence obligations. Producer, source, target,
engine and retained product identities belong to the pinned qualification and
claim records; source rejection is distinct from runtime agreement.
