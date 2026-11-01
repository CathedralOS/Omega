# Machine tuple dispatch

A complete machine-valued static tuple supplied through generic dispatch is
those selected declarations. The call records that result, and the caller
reads the record.

Owning task: **OPERATOR-MACHINE-SUPPLY**. The requirement is admission of a
complete machine-valued static callable tuple through generic dispatch.
`pair<left, right>` supplies both machine arguments. `apply` selects that
tuple as `Observe` and calls it. `left` returns `kept` and `right` returns
`true`, so the true input takes `B(kept)` and yields true. `hold` requires
`direct == true`. The false arm of `pair` returns false, and `hold`'s false
arm crashes.

Before admission, a nested machine application whose static tuple contained a
machine operand was not a completely admitted callable tuple.

Each twin changes only `left`'s body from `kept` to `false`.

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
subject arrives. They do not re-establish the tuple result after the call.
Reject fragments are `direct == true` and `hold`. An incomplete tuple
`pair<left>` still refuses specialization.

On Linux x86_64, all nine positives report native exit 0 and interpreter exit 0
from `omega run --both`. All nine twins reject. Windows, macOS, and QEMU were
not run.

- **Establishment:** `left` returns the subject and `right` returns true. The
  tuple does not establish itself.
- **Identity:** the two selected machines are `left` and `right`, in that
  order, under the one subject.
- **Composition:** `apply` calls the selected tuple. `hold` reads the result.
- **Soundness:** a `left` body of `false` leaves `true` unable to satisfy `hold`.
- **Psi 05–07 through Omega 07–09:** the nine positives agree on this host.
  The other hosted targets are bound and not executed here.
