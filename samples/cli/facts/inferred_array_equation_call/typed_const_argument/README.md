# Computed array sizes through calls and stored values

Fact: the checked call `extent()` establishes index 2 before
`IndexedPacket<[u8; 2], extent()>` discharges `Value == [u8; Capacity]`.

The program builds a packet with that exact two-element array type, carries it
through `Envelope<Value>`, and passes its array field to `capacity`. That call
must infer 2 from the field's type. Array element values do not supply the size.
The reject twin changes only `extent()` from 2 to 3; it must refuse the conflict
with the declared two-element array.

This production belongs to **STRUCTURAL-GENERIC-MATCHING** and
**BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../../TASKS.md).
Neither parent is closed by this row.

## Places and checks

The root program and the nine places each declare exit zero and bind all four
hosted entry targets. Each place has a matching reject twin and pinned corpus
inputs under `tests/omega/{pass,fail}/generics/inferred_equation_typed_const_*`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [main.omg](local/main.omg) | [invalid/main.omg](local/invalid/main.omg) |
| field | [main.omg](field/main.omg) | [invalid/main.omg](field/invalid/main.omg) |
| parameter | [main.omg](parameter/main.omg) | [invalid/main.omg](parameter/invalid/main.omg) |
| call_result | [main.omg](call_result/main.omg) | [invalid/main.omg](call_result/invalid/main.omg) |
| match_join | [main.omg](match_join/main.omg) | [invalid/main.omg](match_join/invalid/main.omg) |
| const_name | [main.omg](const_name/main.omg) | [invalid/main.omg](const_name/invalid/main.omg) |
| other_state | [main.omg](other_state/main.omg) | [invalid/main.omg](other_state/invalid/main.omg) |
| other_machine | [main.omg](other_machine/main.omg) | [invalid/main.omg](other_machine/invalid/main.omg) |
| target_body | [main.omg](target_body/main.omg) | [invalid/main.omg](target_body/invalid/main.omg) |

The target place uses one ordinary named implementation selected by four
explicit Build rows. Positive and twin keep the same declarations and choices.
Other places retain their existing copies, fields, parameters and transitions.
The type equation grants no ownership, reference, calling or effect permission.
The existing `other_state` path is the nested-machine place.

The [non-integer call control](controls/non_integer_call/main.omg) changes only
`extent` to return a Boolean. Postponing evaluation cannot make that Boolean a
valid integer capacity. The [computed satisfaction check](controls/computed_satisfaction/main.omg)
places the call in `satisfies SizedHook<extent()>::read`; its twin changes only
the call's result to a Boolean. The valid program must run at zero, and each
invalid program must reach its own type or index obligation.
The [mixed-readiness check](controls/mixed_destination_readiness/main.omg) also
places a ready value-qualified constant beside the unfinished array type. Its
twin changes that constant from 7 to 8 and must still reject its value predicate.

## Current evidence

Earlier notes recorded Linux success for this production. That remains
historical evidence, not execution of the current source. The fixed original
Linux baseline at `83e3d72bad8bd79a17111831811aaf3b975af8ab` actually refused
`const_name`: it treated the pending call in the constant's type as a completed
non-integer argument. The constant's initializer and consuming call remain fixed.

The first focused repair at `3233d55f3e63407b334adeda6c5e80dec323dbe8`
executed local and field at zero in both engines. The named constant still
refused at a literal-only argument check, and the migrated target requirement
header retained an unsolved equation. Those results remain incomplete. The
next focused source is independently reviewed; actual execution and all four
matching-host qualification remain pending. New control fragments are authored
obligations until their actual diagnostics are observed.

At the older fixed base `264fd279a7bb2755041cc795ef68e9c8f557747e`, an
explicit `TinyBytes<[u8; 256], 256>` application was wrongly treated as size zero.
That earlier repair stopped private checking probes from replacing unrelated
array sizes with temporary zeroes. Completed values still need their exact
declared carrier and final type equations checked. The result-field repair
also kept the selected declared field type rather than deriving it from a
callee's body. These historical repairs do not establish current execution.

The [parent qualification notes](../README.md#current-migration-qualification)
retain all four existing productions, their complete selection and remaining
acceptance. Structured constant formation, exact type equations, ordinary
Build selection, Terminal Psi execution and native execution must all succeed.
No successful intermediate stage completes this row.
