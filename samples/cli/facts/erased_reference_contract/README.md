# Erased shared-reference contracts

Erased primitive and record references discharge their value-7 requirements
at `keep` without runtime witness arguments, reads, storage or cleanup.

The primitive witness still requires `witness == 7`. The record witness has
one `i32` field and requires `record.value == 7` at the same call. Every
place carries both references; only how they reach that operation changes.
Both Boolean paths run and must return their distinct ordinary results, 41
and 42. A wrong result traps; the authored exit oracle is 0.

Owning task: **PROOF-KERNEL-CORE**. The governing rules are
[explicit erased bindings](../../../../wiki/spec/proofs/contracts.md#explicit-erased-bindings)
and the [mathematical foundation](../../../../wiki/spec/proofs/foundation.md).
Erasure changes runtime relevance, not type identity, lifetime, authority or
which precondition must be proved.

## Places and controls

| Place | Positive | Record-only reject twin |
| --- | --- | --- |
| a local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| a record field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| an explicit parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| a completed call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| a covered match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| a named constant | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| a Build-selected implementation | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

The root [`main.omg`](main.omg) repeats the local witness. Positives retain
`ProgramEntry` bindings for all four hosted targets. The selected-body place
also checks distinct x86-64 and ARM64 tags to detect a wrong Build selection.

Every sample twin keeps the primitive referent at 7 and changes only the
record field to 8, including its producer postcondition when that place uses
one. It must reject at the contract consuming that record, with
`record.value == 7` in the diagnostic. An unrelated early refusal does not
establish this control.

The existing `tests/omega/pass/facts/erased_reference_contract_through_*_exit`
files mirror the positives. Each existing
`tests/omega/fail/facts/erased_reference_contract_through_*_rejected` file
retains its scalar-only control and adds the corresponding record-only twin
under distinct declaration names. Thus the old primitive rejection remains
independent of the new record rejection. The target-body scalar control uses
the sample's ordinary named implementations rather than retired target prefixes.
No Omega files or fact folders are added.

## What the witnesses require

- Construction, a declared producer guarantee, an entry precondition or every
  incoming match arm establishes the field value 7. The caller adds no equality
  guard after the transport to manufacture the required fact.
- Reference arguments retain their exact declarations, referents and scopes.
  Type, borrow, access, ownership and write-validity checks still apply.
- Calls and nested machines substitute the complete record subject. The
  contract-term path carries its declared fields, rather than replacing the
  record with a scalar or inferring a value from its name.
- Checked lowering retains erased contract terms while excluding erased
  bindings from executable parameter plans. Psi/native exit agreement checks
  behavior; the lowered and calling-plan evidence separately establishes that
  erased arguments have no executable lanes.

Run an existing place and its twin through the ordinary CLI:

```sh
omega run --both samples/cli/facts/erased_reference_contract/local/main.omg
omega --check samples/cli/facts/erased_reference_contract/local/invalid/main.omg
```

## Observations and remaining scope

The primitive-reference row predates this extension; its historical observations
retain their original producer identities. The record baseline at
[`b85c3aef217b`](https://github.com/CathedralOS/Omega/actions/runs/42658306975)
uses compiler inputs identical to published `4a194ff`. Both local programs
refuse before contract checking because the erased record parameter is neither
a scalar lane nor an admitted contract-term carrier. Neither engine ran, and
the twin did not reach its intended field-value refusal. The original archive,
source snapshots, six input blobs and both optimized producer profiles are
retained in the claim evidence record.

Record acceptance is in progress in this draft. Array referents, dynamic
dispatch and transitive assumptions beyond the direct construction lane remain
open. This row does not close the broader PROOF-KERNEL-CORE acceptance.
