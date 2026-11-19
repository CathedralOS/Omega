# Inherited scalar domain

Nested domain `Nested` requires `self in Small`, and inherited domain
`Nested::Inherited` establishes `self < 5`, so `5 - held` is exact.

Parent: `DOMAIN-DEFAULT-PREDICATE-UNIFICATION` in
[TASKS.md](../../../../../TASKS.md). The qualified constant and the same
value carried through the other eight places establish the nested parent
predicate. `const HELD: u64 in Nested::Inherited = 3` exits 0. Initializer
`9` rejects.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](invalid_local/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](invalid_field/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

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

This host executed all nine places with `omega run --both`: native and
Terminal Psi both exited 0. Each initializer-`9` twin rejected.

Still open on the parent item: one general predicate representation through
formation, construction, calls, matching, coverage, writes, and erasure.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Observed evidence is
Linux x86-64 native and Terminal Psi agreement for the positive and source
rejection for the twin; the other hosted targets keep their authored rows,
unexecuted here.
