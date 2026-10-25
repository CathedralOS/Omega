# Forwarded runtime indices

A runtime limit from 9 to 10 binds `Index<limit>`, retaining payload 8 below the exact captured limit.

This is another production of the parent fact. Each place constructs the value
through `forward<limit>(8)`, whose generic result comes from `create<Forwarded>`.
The receiving `Index<limit>` annotation must agree with that exact application.
A same-index mutable loan then crosses an ordinary `preserve` call. Call carrier
checking retains its nominal declaration and static argument roles while the
occurrence checker proves the captured Value coordinate.
`observe<limit>(indexed.value)` then constructs and reads the same indexed value
through an inferred member-call temporary. The distinct binder names deliberately
exercise declaration identity. Each twin changes only payload 8 to 10 at the
forwarding call and must reject `10 < limit`; it does not change a caller guard.

The places are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [another machine](other_machine/main.omg),
and [target body](target_body/main.omg). Every place has `invalid/main.omg`.
Identical corpus pairs are named
`facts/runtime_index_application_forwarded_through_<place>_{exit,rejected}`.

The owning outcome is `RUNTIME-VALUE-GENERICS`. The accepted contracts are
[value binders and runtime identity](../../../../../wiki/spec/language/generics.md)
and [dependent-value composition](../../../../../wiki/spec/language/dependent_values.md).
Construction establishes `value < Created`; each forwarding call substitutes its
own bound subject. Ordinary value coordinates join ordinary formal operands in
the result-index substitution. Mutation does not retag an existing value;
unknown operand writes still require the existing captured-operand proof path.
A missing proof remains a rejection.

Checking and proof erase into Terminal Psi without a proof object. The normal
record descriptor, call/result layout and ABI carry the scalar index. The normal
instruction, register-home, frame, encoding and image stages realize those calls;
`omega run --both` observes payload 8 and the descriptor limits 9 and 10. All four hosted target
entries are authored, while current execution evidence covers Linux x86-64 only.
It does not claim execution on unavailable Windows, macOS or Linux AArch64 hosts.

At fresh main `036605466510`, the reduced explicit forwarded-result witness
rejected with `cannot prove runtime index subject equality`; the nested inferred
control also retained a foreign callee binder and rejected closed-value admission.
Raw source, producer identities and before/after observations are retained in
`build/slot7-evidence` and the claim `eb4a17c20bd044858d0d087b992335c3`.
The parent outcome remains open until every required hosted execution has been
observed. General nested aggregate-call realization and flow-sensitive computed
type-recipe formation have separate preserved baseline failures; these runs do
not establish those behaviors. Snapshot-order and retag controls remain part of
the focused qualification.
