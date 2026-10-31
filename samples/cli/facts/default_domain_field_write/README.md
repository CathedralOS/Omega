# Default-domain field write

A field write that preserves `Interval`'s default domain keeps `self.hi <= 4`,
so `4 - hi` is exact.

Parent: `DOMAIN-DEFAULT-PREDICATE-UNIFICATION` in
[TASKS.md](../../../../../TASKS.md). The same disjunction construction proves
must hold again after a field store, or the later exact use must reject.

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

Each positive binds the four hosted targets. `omega run --both` requires
native and Terminal Psi agreement on exit 0. Each twin changes only the
preserving write of `3` to `9`.

`match_join` writes `3` and `2` on two locals, joins them, and also consumes
`Empty`. The twin changes only the `3` write to `9`.

Still open on the parent item: formation, construction, calls, matching,
coverage, and erasure of the general predicate representation. These nine
places are the write of that predicate.
