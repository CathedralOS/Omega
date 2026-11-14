# Domain carrier identity

A domain declared over `u16` classifies `u16` values only. Seven fits in both
`u8` and `u16`, but `7u8` is not a member of `u16::Small`; a program converts
it first. Each place below carries a `u16` value into a `u16::Small`
membership and subtracts it from ten, which is exact only because the
membership supplies `self <= 10`. Each reject twin changes the carrier at that
place to `u8` and nothing else.

One sentence: `A u16 value is a member of the u16 domain Small (self <= 10) only on its own u16 carrier, so 10 - value is exact.`

Owning task: **DOMAIN-CARRIER-FORMATION** in [TASKS.md](../../../../TASKS.md),
specified by
[carrier-exact membership](../../../../wiki/spec/language/domains.md#carrier-exact-membership).

| Place | Positive | Reject twin | Corpus pass | Corpus fail |
| --- | --- | --- | --- | --- |
| Local | [local](local/main.omg) | [invalid](invalid_local/main.omg) | `pass/facts/domain_carrier_identity_through_local_exit` | `fail/facts/domain_carrier_identity_through_local_rejected` |
| Field | [field](field/main.omg) | [invalid](invalid_field/main.omg) | `pass/facts/domain_carrier_identity_through_field_exit` | `fail/facts/domain_carrier_identity_through_field_rejected` |
| Parameter | [parameter](parameter/main.omg) | [invalid](invalid_parameter/main.omg) | `pass/facts/domain_carrier_identity_through_parameter_exit` | `fail/facts/domain_carrier_identity_through_parameter_rejected` |
| Call result | [call_result](call_result/main.omg) | [invalid](invalid_call_result/main.omg) | `pass/facts/domain_carrier_identity_through_call_result_exit` | `fail/facts/domain_carrier_identity_through_call_result_rejected` |
| Match join | [match_join](match_join/main.omg) | [invalid](invalid_match_join/main.omg) | `pass/facts/domain_carrier_identity_through_match_join_exit` | `fail/facts/domain_carrier_identity_through_match_join_rejected` |
| Const name | [const_name](const_name/main.omg) | [invalid](invalid_const_name/main.omg) | `pass/facts/domain_carrier_identity_through_const_name_exit` | `fail/facts/domain_carrier_identity_through_const_name_rejected` |
| Nested machine | [other_state](other_state/main.omg) | [invalid](invalid_other_state/main.omg) | `pass/facts/domain_carrier_identity_through_other_state_exit` | `fail/facts/domain_carrier_identity_through_other_state_rejected` |
| Other machine | [other_machine](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) | `pass/facts/domain_carrier_identity_through_other_machine_exit` | `fail/facts/domain_carrier_identity_through_other_machine_rejected` |
| Selected implementation | [target_body](target_body/main.omg) | [invalid](invalid_target_body/main.omg) | `pass/facts/domain_carrier_identity_through_target_body_exit` | `fail/facts/domain_carrier_identity_through_target_body_rejected` |

Where the membership sits at each place: a `requires` on the parameter, on a
record field (`reading.level`), on a constant (`LEVEL`), on a nested machine's
parameter, and on a boundary-trait requirement selected per target by Build;
an `ensures result in Small` on a call result, on a two-arm join, and on a
forwarding helper machine; and, for the local, an executable `level in Small`
test whose true arm carries the fact into the call. Twins reject with
``domain membership `u16::Small` expects subject carrier `u16`, got `u8` ``,
or for the local with the matching `executable domain test` wording.

Focused formation controls beside the row, under `tests/omega/fail/domains/`:
`carrier_exact_suffixed_literal`, `carrier_exact_unrelated_declaration`,
`carrier_exact_data_predicate`, `carrier_exact_domain_predicate`,
`carrier_exact_trait_contract`, `carrier_exact_ensures_result`,
`carrier_exact_signedness`, `carrier_exact_nominal`,
`carrier_exact_indexed_application` and `carrier_exact_specialized_generic`.
`pass/domains/carrier_exact_same_carrier_exit` keeps the accepted forms:
same-carrier memberships, an unsuffixed literal landing in `u16`, an explicit
`as u16` conversion before an executable test, a closed `AtMost<10>`
application, a nominal carrier, and a generic contract specialized at `u16`.

## Requirements and evidence

- **Establishment:** the membership is established by the caller's proof of
  `self <= 10` for the actual (7 and 2), by a checked `ensures` on the
  producing machine, or by the executable test's true arm. Carrier formation
  runs before the predicate is substituted, so a mismatched subject never
  reaches integer reasoning.
- **Identity and lifetime:** carrier correspondence uses the subject's
  resolved type after the owning declaration's type equations; a generic
  declaration is judged on each specialized copy once its binder is closed.
- **Composition:** every place's consumer subtracts directly from the
  membership it receives; positives and twins differ only in that place's
  carrier.
- **Runtime falsifiability:** `main` checks two inputs (7 gives 3, 2 gives 8),
  the join selects a different value on each arm, the field neighbours keep
  41 and 43, and the selected implementations check a per-target tag (17 for
  x86-64, 29 for AArch64), so a wrong lowering or Build row traps.
- **Lowering and native realization:** every positive binds `ProgramEntry`
  on the four hosted targets and passes `omega run --both`. A scalar
  `ensures result in Domain` used to fail Terminal lowering with
  `MissingUnitDomainPlan`; it is now kept as a checked source guarantee, as
  other scalar ensures are.

Observed here: Linux x86-64 native and Terminal Psi agreement for every
positive, and source rejection for every twin. The other hosted targets keep
their authored rows, unexecuted here.

## Remaining

`(narrow as u16) in Small` inside a `requires` clause is now well formed (it
used to fail typing with `cast target type lowering requires the enclosing
resolved program`), but a caller cannot yet discharge such a precondition:
`converted(4u8)` reports `cannot prove requires contract ... narrow as u16 in
u16::Small`. The same conversion in an executable test works. Carrier-generic
domain families over a type-parameter carrier were not exercised.
