# Witnessed compiler gap and bounded repair

The independently frozen application checked on unmodified main base
`b4008bb3c7894aebad47eaccf8af12bb4a8006b1`, then failed native production with
`RegisterAllocation(RuntimeSpill(Homes(NoCommonCandidate { function: 10, register: 6 })))`.
The preserved base compiler SHA256 is
`31a3c7997e31bcce4c09d9f112e29f3d49b99af602beda55e68bba4d573318b0`.
The complete application is the source witness; no numerical expectation was
relaxed and no alternate numerics library was imported.

Terminal inspection and temporary selected-instruction tracing identified the
decimal writer's state-carried i64 value: its console call requires a surviving
callee-saved view, while later signed remainder requires the fixed accumulator.
Each point has legal views; their whole-interval intersection is empty. Existing
runtime split/spill admission already supports these scalar block parameters
and checks incoming paths, rewrites, rebuilt analyses and final strict homes.
The recovery entrance recognized only `NoCompatibleHome`. The repair generalizes
that entrance and its composing routes to `NoCommonCandidate`; it grants no
view, storage authority or acceptance bypass. Other malformed class/view errors
remain errors. Temporary diagnostics were removed.

The durable arithmetic fixture `runtime_call_live_remainder_recovery_exit`
carries positive and negative scalars across console calls into signed remainder,
requiring exactly two `carry` lines, empty stderr and exit 70. Both native and
Terminal observations matched. Existing trapping-add pressure and borrowed-array
pressure customers also completed both engines with exit 70. The allocator's
14 source-free unit/route tests passed, including an incompatible-view control.
Scoped crate check and Clippy passed; existing warnings remain visible in logs.

One regression package proposal became stale when its `.gitignore` was added.
It was discarded without changing accepted project files. Its end-to-end test
uses the documented repository corpus fixture acceptance policy. The application
itself completed ordinary package review and accepted-lock publication. There
was no automatic approval or package denial.
