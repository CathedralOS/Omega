# Projection receiver call

A field of a namespace-qualified call inside a domain predicate is that field
of the declaration's body under the subject. The predicate records that result,
and the caller reads the record.

Owning task: **OPERATOR-MACHINE-SUPPLY**. The requirement is declaration-owned
call formation for receiver and projection terms. `marks::project(self).kept`
selects `project`. The body builds `Flag { kept: kept, other: true }`. The
predicate reads `kept` and leaves `other` unread. Qualifying `bool in Held`
records that field. `hold(direct)` requires `direct == true` and returns
`direct`, so a value other than true takes `bad()` (`crash Trap`).

Before this projection, checking accepted the domain fact and lowering rejected
the qualification with `scalar qualification requires non-vacuous or unsupported
evidence`.

Each twin changes only the selected field from `kept` to `false`. The neighbor
`other: true` stays. A projector that read `other` would accept the twin.

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
| target body | [target body](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Parameter and another-machine transports require `kept == true` so the true
subject arrives. They do not re-establish `Held` after the qualification.
Reject fragments are `bool::Held` and `cannot prove initializer`.

On Linux x86_64, all nine positives report native exit 0 and interpreter exit 0
from `omega run --both`. All nine twins reject. Windows, macOS, and QEMU were
not run.

- **Establishment:** `marks::project(self).kept` is the domain obligation. The
  selected field of the body supplies the result.
- **Identity:** the subject is one `bool` per place. The call's formal is that
  subject. `other` is a neighbor of the selected field.
- **Composition:** `hold` reads the qualified value. No post-hoc guard replaces
  the domain call.
- **Soundness:** a selected field of `false` leaves `true` outside `bool::Held`.
- **Psi 05–07 through Omega 07–09:** the nine positives agree on this host.
  The other hosted targets are bound and not executed here.
