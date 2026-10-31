# Domain predicate call

A call inside a domain predicate is the declaration's body under the subject.
The predicate records that result, and the caller reads the record.

Owning task: **OPERATOR-MACHINE-SUPPLY**. The requirement is declaration-owned
call formation in domain and default predicates. `domain bool::Held requires
premise(self)` selects `premise`, whose body returns `kept`. Qualifying
`bool in Held` records that result. `hold(direct)` requires `direct == true`
and returns `direct`, so a value other than true takes `bad()` (`crash Trap`).

A data default-domain predicate uses the same declaration-owned call at
construction: `requires premise(self.kept)` admits `Mark { kept: true }` and
rejects `Mark { kept: false }` because the call evaluates false. That gate
already ran. This row is the scalar domain projection that lowering previously
dropped as unsupported qualification evidence.

Each twin changes only `premise`'s body from `kept` to `false`.

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

On Linux x86_64, after projecting the selected body into the scalar domain
predicate, all nine positives report native exit 0 and interpreter exit 0 from
`omega run --both`. All nine twins reject. Windows, macOS, and QEMU were not run.

- **Establishment:** `premise(self)` is the domain obligation. The body supplies
  the result; the annotation does not establish itself.
- **Identity:** the subject is one `bool` per place. The call's formal is that
  subject.
- **Composition:** `hold` reads the qualified value. No post-hoc guard replaces
  the domain call.
- **Soundness:** a body of `false` leaves `true` outside `bool::Held`.
- **Psi 05–07 through Omega 07–09:** the nine positives agree on this host.
  The other hosted targets are bound and not executed here.
