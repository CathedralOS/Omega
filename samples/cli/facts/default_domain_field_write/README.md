# Default-domain field write

A field write that preserves `Interval`'s default domain keeps `self.hi <= 4`,
so `4 - hi` is exact.

Parent: `DOMAIN-DEFAULT-PREDICATE-UNIFICATION` in
[TASKS.md](../../../../../TASKS.md). The same disjunction construction proves
must hold again after a field store, or the later exact use must reject.

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

Each positive binds the four hosted targets. `omega run --both` requires
native and Terminal Psi agreement on exit 0. Each twin changes only the
preserving write of `3` to `9`.

`match_join` writes `3` and `2` on two locals, joins them, and also consumes
`Empty`. The twin changes only the `3` write to `9`.

Still open on the parent item: formation, construction, calls, matching,
coverage, and erasure of the general predicate representation. These nine
places are the write of that predicate.

## Mathematical coupling during allocator restoration

[mathematical_coupling](mathematical_coupling/main.omg) exercises the same
field-update operation with `embed(used) + embed(free) == embed(capacity)`.
The exclusive call temporarily breaks and then restores that complete domain;
the caller uses exact `capacity - used` before its runtime oracle. This is the
allocator's `BUMP-ALLOCATOR-CANARY` restoration obligation, not a new row for
another scalar width or field count. Source checking proves the mathematical
predicate; store geometry must not reject it merely because a second bounded
Terminal predicate spelling cannot express it. The twin changes only the final
free count from 16 to 17 and must reject unclosed restoration. The runtime
oracle checks both mutated fields, the unchanged capacity, and subtraction.
Pins: `default_domain_mathematical_coupling_{exit,rejected}` under the facts
corpus. Four hosted targets are declared; host execution is recorded with the
submitted repair.
