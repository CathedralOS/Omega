# Value-location alignment

Fact: align_of(value) observes the storage location of a live u64 place, the referent of a &u64, or the first element of a nonempty [u64] slice, and reports that location's guaranteed alignment, at least 8u64 on every hosted target, discharging exact subtraction alignment - 8u64 across all nine places.

Before this row the compiler refused every value argument to `align_of`
("takes no value arguments; the queried type is its single `<T>` operand"),
so a program could ask for the alignment of a type but never of the storage it
actually holds. This row supports the `align_of(value)` bullet of
`CHECKED-ASM-LABEL-CONTRACTS` in [TASKS.md](../../../../TASKS.md), governed by
[location alignment](../../../../wiki/spec/language/counts_and_addresses.md#location-alignment)
and [layout queries](../../../../wiki/spec/layouts/plans.md#size-and-alignment-queries).

For example, `align_of(cell)` on a local `cell: u64` reports 8, and
`align_of(value)` on a parameter `value: &u64` reports the alignment of the
`u64` it refers to, not of the 8-byte reference. Each positive subtracts 8
from that result; the subtraction is accepted only because the query proves
the result is at least 8.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [main](local/main.omg) | [invalid](invalid_local/main.omg) |
| field | [main](field/main.omg) | [invalid](invalid_field/main.omg) |
| parameter | [main](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| call result | [main](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| match join | [main](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| const name | [main](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| nested machine | [main](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| another machine | [main](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| another target implementation | [main](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

A second production of the same query observes a borrowed slice:
[nonempty slice](compositions/nonempty_slice/main.omg), with a
[twin](compositions/nonempty_slice/invalid/main.omg) that removes only
`requires items.len >= 1`. A slice's location is its first element's, which
exists only for a nonempty view, so the twin rejects with "needs the nonempty
fact `items.len >= 1`". A `&[u8]` companion reports 1, not the 8 of the
slice descriptor.

A third production observes a parameter from its own contract:
[contract requirement](compositions/contract_requirement/main.omg) states
`requires align_of(value) >= 8u64` on a `&u64` parameter, and every caller
must satisfy it. Its [twin](compositions/contract_requirement/invalid/main.omg)
changes only the referent to `u32`, so the clause is false for every caller
and rejects with "scalar predicate is false: requires contract". Because each
answer is the exact validated minimum, the body's own `align_of(value)` already
proves its subtraction; the contract adds the caller-side obligation, not a
premise the body depends on. A weaker clause therefore cannot make the body's
subtraction fail, which is why the twin changes the storage, not the bound.

The same contract production also observes a field path below a parameter:
[contract field requirement](compositions/contract_field_requirement/main.omg)
states `requires align_of(holder.cell) >= 8u64` on a `&Holder` parameter, and
a companion clause observes the neighbouring `u8` and `u32` fields, whose
results (1 and 4) the program checks at run time. Its
[twin](compositions/contract_field_requirement/invalid/main.omg) changes only
the field to `u32` and rejects with "scalar predicate is false: requires
contract".

Each twin changes only the observed location's storage to `u32`. Its
guaranteed alignment is 4, so the unchanged `alignment - 8u64` is rejected with
bounds `[-4, -4]`. Positives and twins are mirrored under
`tests/omega/{pass,fail}/facts/value_location_alignment_through_*`.

## How the query works

Typing keeps the authored operand on the `align_of` call. Typed preparation
then resolves that operand as a place in its state: it must be rooted at a
local or parameter, a reference is followed to its referent, and the operand
leaves the executable call. A borrowed slice is followed to its element
type, and the request keeps a proof-only `view[0]` index that range checking
discharges with the same length facts as an ordinary first-element index; it
is never evaluated, so no element is read and no implicit `&view[0]` loan is
made. From there the query is the same portable
selected-layout alignment request as `align_of<T>()` over the observed
storage type, so checking, Terminal Psi, the interpreter and native code all
answer it from the selected target's validated layout. The operand is never
loaded, moved or borrowed by the query.

The reported value is the validated minimum for that storage type under the
default storage rule. The specification allows this: the result must be a
guarantee for the location, not its greatest power-of-two divisor.

## Runtime falsifier

Every positive also observes narrower neighbours and checks the exact
results at run time: a `u8` location must report 1 and a `u32` field 4. A
query that measured the reference descriptor (8), the whole record, or the
wrong field would trap. A `&u64` parameter alone cannot tell referent from
descriptor, since both align to 8; the `&u8` companion in the parameter and
call-result places does, because its referent reports 1. The match join calls both arms with distinct results
(8 and 16). The selected-implementation place checks its Build tag (17 for
x86-64, 29 for AArch64) so a wrong provider row traps.

Every positive binds `ProgramEntry` for Windows x86-64, Linux x86-64, Linux
ARM64 and macOS ARM64 and declares exit zero. Native and Terminal Psi
agreement was observed on Linux x86-64 only; the other targets keep their
authored rows, unexecuted here.

## Requirements accounted for

- Establishment: the query itself establishes the bound from the observed
  storage type's validated layout; no caller guard precedes the subtraction,
  and the runtime checks follow it.
- Identity: the subject is the exact place the operand names, through
  references to the referent. A value leaf, a constant, a slice not proved
  nonempty, or a contract query that is not a parameter or a field path below one is rejected rather than given a fabricated location.
- Composition: the result is an ordinary `u64` whose layout-observation fact
  survives copies, fields, call returns, joins and machine arguments, through
  the existing layout-observation route.

## What remains open

- Contract positions: `requires` on an ordinary machine accepts a whole
  parameter, its referent, or a field path below either. Still rejected:
  `ensures` clauses (a true clause cannot yet be proved at exit), contracts on
  trait requirements (so a per-target implementation cannot carry the clause),
  and slices, elements or case payload fields in any contract.
- Case payload fields: a body query on `s.x` where `x` is a case payload is
  rejected, because the query does not carry the active-case proof an
  ordinary read owes; it would otherwise report the first matching case's
  field type.
- Stronger-than-type alignment from placement or allocator contracts, and
  runtime computation of a location's alignment, are not implemented; the
  result is always the type minimum.
- Location validity after the view is replaced, its offset changes or its
  backing moves is not modelled beyond the type minimum, which holds for any
  live location of that type.
- Interpretation reports selected-target geometry, but there is no admitted
  storage mapping for a location with stronger placement.
