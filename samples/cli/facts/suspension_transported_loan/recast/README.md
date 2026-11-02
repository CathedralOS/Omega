# Recast loans across suspension

A stored reference can be recast to a compatible record view and carried across
a possibly suspending call. The view must keep the original backing and its carry
restrictions, so the transported record still reads 73 afterward.

This is another production of the parent fact, under
`STATIC-MACHINE-MIGRATION` in [TASKS.md](../../../../../TASKS.md). It uses the
same fact sentence, `Message` fields, consuming read, canaries, and rejection
obligation as the [direct-loan production](../README.md). Each cast starts from
an already stored reference, rather than an explicit borrow expression. The
cast changes the checked view of that storage; it grants no new lifetime,
access, or suspension permission.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [source](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| constant | [source](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [source](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other machine | [source](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target body | [source](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each positive binds all four hosted targets and declares exit 0. Every twin
changes only `Backing`'s suspension policy to forbidden. The backing stays live
through the recast even though no source statement reads it after suspension.
The match place executes both choices, observing 73 and 11; the target-body
place checks the distinct selected implementation tag. `read` returns zero if
either neighboring field changes, and the entry traps on any wrong observation.

The [recast contract](../../../../../wiki/spec/layouts/recasts.md) requires
compatible geometry and representation validity while preserving address,
authority, provenance, and lifetime. The [carry contract](../../../../../wiki/spec/resources/carry.md#borrows-across-suspension)
requires the original referent, parent loans, access restrictions, and live
resource demands to survive the call. These are established separately: equal
sizes or the target record's default carry policy supply no authority.

At fixed base `3009cab5a41` on macOS ARM64, the optimized compiler accepts the
direct-loan local place and an explicit-borrow recast. The otherwise equivalent
stored-reference local place rejects at `park`: "the loan lifetime resource is
unavailable". Its twin also reports the original `backing`'s forbidden carry
policy. These results were captured before the compiler repair; candidate
execution and rejection evidence remains pending.

The inline scheduler can establish retention across a possibly suspending call;
it does not demonstrate an actual parked activation or another host's runtime.
