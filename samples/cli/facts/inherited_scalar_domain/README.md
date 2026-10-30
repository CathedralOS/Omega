# Inherited scalar domain

Nested domain `Nested` requires `self in Small`, and inherited domain
`Nested::Inherited` establishes `self < 5`, so `5 - held` is exact.

Parent: `DOMAIN-DEFAULT-PREDICATE-UNIFICATION` in
[TASKS.md](../../../../../TASKS.md). The qualified constant, and the same
value carried through the other eight places, is the destination that must
establish the nested parent predicate.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

- Establishment: `Small` proves `self < 5`. `Nested` requires that membership.
  `Nested::Inherited` adds the parent membership, so a destination qualified
  by it establishes the bound. The initializer `3` is inside the bound; `9`
  is the reject twin.
- Identity: `held` is that exact qualified scalar. The twin changes only the
  initializer.
- Composition: each place transports `held` into `5 - held`. `match_join`
  also carries `1`, and `5 - 1` is `4`.
- Soundness: the bound is the domain predicate, not a caller guard after the
  qualifier is erased.
- Psi through native: `omega run --both` requires Terminal and native
  agreement on exit 0. This host executes `linux_x86_64`.

`const_name` is not closed. A qualified constant destination still fails
in the private `@const-qualification` probe with `cannot prove scalar
result domain` for both a member (`3`) and a non-member (`9`). The other
eight places establish the same nested inherited bound on an ordinary value.

Still open on the parent item: qualified constant destinations, and one
general predicate representation through formation, construction, calls,
matching, coverage, writes, and erasure.
