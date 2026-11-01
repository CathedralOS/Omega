# Generic requirement statement call

A statement-position call of the generic boundary requirement `Mark::record<T>`
from a generic caller applies the caller's open `T` to the requirement's
formals, so the provider's write of `label` into `hits` is observed after the
call.

Owning task: **TOP-LEVEL-BOUNDARY-REQUIREMENTS**. The requirement is call
validation of a generic requirement that belongs to a boundary trait. A
carrier-owned `boundary requirement Task::request_cancel<T>(...)` inferred its
type operand. A boundary-trait requirement states its complete static
telescope, so the generic caller writes `Mark::record<T>(...)` with its own
open `T`.

Before the repair, statement-call validation admitted only `machine` binders
of a selected requirement. A call whose operand was the caller's open `T` found
no signature and refused with ``qualified call `record` has no resolved
callable declaration``. The same call with a closed operand (`record<i32>`) and
the same call in value position were accepted, so a generic helper could use
`finish<T>` and not `request_cancel<T>`.

`notify<T>` calls `Mark::record<T>(value, label, hits)`. `main` calls it for
`i32` with label 9 and for `u8` with label 5, then crashes unless the two
counters read 9 and 5. `Counting::record` ignores the borrowed value and stores
`label` through `hits`, so the observation proves the call reached the selected
provider with its own arguments and with each instantiation of `T`.

Each twin changes only the explicit operand from `T` to `u64`. The substituted
formal is then `&u64` and the argument is `&T`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [call result](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [match join](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const name | [const name](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [other state](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| another machine | [other machine](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| another selected implementation | [target body](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The match-join place picks the borrowed argument and the label in the same
`match`, and the two instantiations take different arms, so a join that lost an
arm reads the wrong counter. The target-body place selects four named
implementations of `Mark`, one per hosted target, through `Build` rows; the
bodies differ in their binder spellings and write the same label.

Each twin passes `&label` where the substituted formal is `&T`. The repaired
checker rejects it in the generic template with ``argument `value` for state
`record` expects `&T` ``. Before the repair the twin reported only the two
instantiated copies, ``expects `&i32` `` and ``expects `&u8` ``, so the open-`T`
fragment pins the repair.

On Windows x86_64, with the optimized producer built from `0402685c2f2` plus
this change, all nine positives report native exit 0 and interpreter exit 0
from `omega run --both`, and all nine twins reject. At the same base without the
change all nine positives refused ``qualified call `record` has no resolved
callable declaration`` at the statement call. Pinned fixtures:
`tests/omega/pass/facts/generic_requirement_statement_call_through_<place>_exit`
and `tests/omega/fail/facts/generic_requirement_statement_call_through_<place>_rejected`.
Linux, macOS, and QEMU were not run; the other hosted targets are bound, not
executed.

- **Establishment:** the explicit operand `T` establishes the requirement's
  type application, and `Counting::record` establishes `hits` by storing
  `label`. No caller guard re-establishes either.
- **Identity:** the requirement's `T` and the caller's `T` are distinct binders
  that share a spelling. The call substitutes one for the other, so the formal
  `&T` of the requirement reads as the caller's `&T`.
- **Composition:** every place hands the same borrow to the call through a local,
  a field, a parameter, a call result, a match join, a named constant, a nested
  machine, another machine, or a Build-selected implementation.
- **Soundness:** an argument whose type is not the substituted formal is refused
  before execution, in the template and in each instantiation.
- **Psi 05–07 through Omega 07–09:** the two instantiations (`i32`, `u8`) lower
  to separate closed applications, and the native and interpreter engines agree
  on the stores. A dropped or misrouted call leaves `hits` at 0 and `main`
  crashes.

Open: a bare `Mark::record(value, ...)` still has to state its type operand,
because a static requirement application requires its complete invocation
telescope. A data-owned helper that repeats its owner's binder spelling (`machine
Task::settle<T>` on `Task<T>`) still refuses at specialization with `applied
callable has an unrepresented static type operand`; the Task customers use free
machines, and the data-owned spelling is retired by **STATIC-MACHINE-MIGRATION**.
