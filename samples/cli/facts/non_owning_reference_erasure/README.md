# Explicit non-owning reference erasure

Fact: explicit non-owning reference erasure writes 73 to the original record
without minting routed membership.

Parents: `BUILD-OWNED-TARGET-SELECTION` and `DOMAIN-ACCESS-AUTHORITY` in
[TASKS.md](../../../../TASKS.md).

The [root](main.omg) and nine places carry an issued record through an explicit
bare write-only view, write 73, and read the original backing afterward. Nonzero
17/23 fields surround the written slot and remain unchanged. The matching
reject twin changes only the view contract to unrelated `Reissued` membership;
erasure cannot establish a new route. The fact declares no domain access
policy and makes no promise that an input `Issued` qualification survives a write.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [control](invalid_local/main.omg) |
| Field | [program](field/main.omg) | [control](invalid_field/main.omg) |
| Parameter | [program](parameter/main.omg) | [control](invalid_parameter/main.omg) |
| Call result | [program](call_result/main.omg) | [control](invalid_call_result/main.omg) |
| Match join | [program](match_join/main.omg) | [control](invalid_match_join/main.omg) |
| Const name | [program](const_name/main.omg) | [control](invalid_const_name/main.omg) |
| Nested machine | [program](other_state/main.omg) | [control](invalid_other_state/main.omg) |
| Other machine | [program](other_machine/main.omg) | [control](invalid_other_machine/main.omg) |
| Selected implementation | [program](target_body/main.omg) | [control](invalid_target_body/main.omg) |

The selected place uses an ordinary `ErasedWrite` requirement and checked
`ErasedWriteCode` implementation. [Build](target_body/build.omg) records four
explicit provider selections and four hosted entries. The public incoming
contract and write are identical to the former target-prefixed bodies. The
match place executes both inputs against distinct original homes and verifies
that each spare stays untouched. The const place supplies the same 73 through
`WRITTEN`; it creates no compile-time exclusive reference.

```sh
omega run --both samples/cli/facts/non_owning_reference_erasure/main.omg
```

## Requirements and controls

- Establishment and authority: `issue` establishes the exact incoming routed
  membership. A checked erasure preserves access and storage identity without
  issuing `Reissued`. A checked alternative cannot supply non-observation
  evidence for an opaque Native implementation selected by Build: that exact
  implementation still needs its admitted judgment and receipt.
- Identity and composition: the same write reaches original backing through
  every transport. A qualification-only view retains declared selectors;
  physical views retain their separate geometry checks. The caller observes
  73 and unchanged neighbors after the call. Match counterfactuals discriminate
  either constant selector, and a changed selected write value traps.
- Restoration: input root qualifications are preconditions, distinct from
  explicit `ensures` and declared lender field/element obligations. Field
  restoration uses the exact declaration carrier, including nested reference
  shells, and exact path/domain instance. Call summaries retain each performed
  assignment's checked destination type; a bare writer cannot borrow a
  qualified writer's restoration merely because their storage coordinates match.
- Soundness: existing `fail/recast/non_owning_erasure_preserves_*` controls cover
  routed/scalar fields, arrays, projected/stored/reborrowed aliases and helper
  writes. Unproved explicit ensures still reject. A typed whole write without
  ensures remains valid, while a typed write followed by an erased bare write
  cannot preserve the lender's routed field. Shared references gain no store
  permission. Existing opaque checked-forwarding controls remain refusals.
- Checked lowering, ABI and realization: ordinary write-only reference
  descriptors preserve the original home, selectors and field offset through
  checked calls and selected supply. No runtime permission bits or new opaque
  authority carrier is introduced. The surrounding fields test physical store
  width and placement. Native product inspection corroborates original-home
  address transport independently of the runtime canary oracle.

## Observations and remaining acceptance

The retained optimized producer `a13bf162097` and its immutable SDK completed all ten
positive authored programs (root plus nine places) with native 0 / Terminal 0,
and all nine routed-membership twins refused for the intended missing fact.
The focused corpus records 44 Source/admission outcomes: 18 checked and 26
intended refusals. Its BOTH leg has 18 actual native 0 / Terminal 0 agreements
and one intended opaque Native admission refusal; that control was never run.
Five existing physical-view, shared/stored-reference and access-opposite CLI
controls retained their outcomes. The unmodified match counterfactual exits 0;
forcing either branch or changing the selected write to 74 traps. All four
hosted target selections prepare and check; fresh execution is macOS ARM64.

After reconciling actual Main write-coordinate and mutation-summary changes,
optimized producer `1f01fe627c9` with its own SDK repeated the 44 Source/admission
outcomes and 18 BOTH executions plus the intended admission refusal. Six affected
normal CLI projects passed (four native 0 / Terminal 0, two proper membership
refusals), alongside seven existing reference/array-window preservation cases,
the same four Terminal counterfactuals and four-target preparation/checking.
The array-window positive and missing-establishment opposite exercise the
retained fixed-range physical translator. Authored Source and pin inputs did
not change during this reconciliation. Current root, returned-pointer, selected
body and match native images are byte-identical to the retained images; their
Terminal products and producer identities remain separately recorded.

The call-result lowering repair rejoins the unique checked reference backing
using its registered stored structural identity. It preserves the distinct
logical target, reference access, returned loan, storage path and final view
checks. Native product inspection follows the original 12-byte record home
through the returned pointer, the store at offset 4 and the caller's value and
17/23 canary reads; the erasure helper returns that pointer without a record copy.

The raw records remain in the work claim's
`build/non-owning-build-migration-20261102` evidence directory. A pin wrapper
initially counted 18 total BOTH outcomes rather than 18 executions plus its one
admission refusal; the complete raw gate outcomes were retained and the tally
corrected without replay. The former recorded acceptance of the invalid helper
field is now its intended rejection, not a lost valid program. An unrelated
replaced-reference local-establishment probe already refused on its baseline
and is excluded from the passing preservation set.

The former baseline refusals and both stopped captures remain retained with
actual source/profile identities. One capture stopped at the invalid helper's
wrong acceptance before runner construction; no invalid control was executed.
The final producer is separately identified, with its own immutable SDK.
Other-host execution and admitted opaque write-only receipt production are not
proved here. Domain-owned access policies, the broader invariant-window
capability and remaining Build migration obligations remain open on their tasks.
