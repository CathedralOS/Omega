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
| a local | [local/main.omg](local/main.omg) | [invalid_local/main.omg](invalid_local/main.omg) |
| a record field | [field/main.omg](field/main.omg) | [invalid_field/main.omg](invalid_field/main.omg) |
| an explicit parameter | [parameter/main.omg](parameter/main.omg) | [invalid_parameter/main.omg](invalid_parameter/main.omg) |
| a completed call result | [call_result/main.omg](call_result/main.omg) | [invalid_call_result/main.omg](invalid_call_result/main.omg) |
| a covered match join | [match_join/main.omg](match_join/main.omg) | [invalid_match_join/main.omg](invalid_match_join/main.omg) |
| a named constant | [const_name/main.omg](const_name/main.omg) | [invalid_const_name/main.omg](invalid_const_name/main.omg) |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [invalid_other_state/main.omg](invalid_other_state/main.omg) |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [invalid_other_machine/main.omg](invalid_other_machine/main.omg) |
| a Build-selected implementation | [target_body/main.omg](target_body/main.omg) | [invalid_target_body/main.omg](invalid_target_body/main.omg) |

The root [`main.omg`](main.omg) calls `keep` as a statement with an erased-only
parameter list: no runtime argument and no result. Positives retain
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
- Calls and nested machines substitute each scalar field of the record through
  its own erased scalar lane; no value is inferred from a name.
- Checked lowering retains erased contract terms while excluding erased
  bindings from executable parameter plans. Psi/native exit agreement checks
  behavior; the lowered and calling-plan evidence separately establishes that
  erased arguments have no executable lanes.

Run an existing place and its twin through the ordinary CLI:

```sh
omega run --both samples/cli/facts/erased_reference_contract/local/main.omg
omega --check samples/cli/facts/erased_reference_contract/invalid_local/main.omg
```

## How the record is carried

An erased closed record, by value or behind a shared reference, has one erased
scalar lane per scalar field. `record.value` in `keep`'s contract names that
lane, exactly as `witness` names the primitive's lane, so the record has no
runtime parameter, field, address, read or cleanup. At a call the lane's actual
is the caller's own erased lane (parameter, nested machine, other machine), the
scalar written in an immutable construction (local, field, constant, selected
body), or the field observed at the place of a record local (call result,
match join) or of a runtime record formal. The two requirements are separate `requires` clauses so that a
twin's refusal names `record.value == 7` alone.

The constant place declares `data Witness [copy]` because a
[constant](../../../../wiki/spec/language/constants.md) needs a type that
permits copying.

## Remaining scope

Array referents, sums, records with erased or address fields, a record with
several scalar fields passed through a nested-machine transition, and a `mut`
formal or a temporary used as an erased record actual are refused.
This row does not close the broader PROOF-KERNEL-CORE acceptance.
