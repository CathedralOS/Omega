# Omega Canaries

Canaries are tiny programs that isolate one compiler capability at a time.
They are not samples, tutorials, or end-user examples.

## Layout

- `pass/<feature>/main.omg`: the compiler should accept this program.
- `fail/<feature>/main.omg`: the compiler should reject this program.
- `fail/<feature>/expected.txt`: the diagnostic fragment that must appear.

Feature names should describe the compiler behavior being pinned down, not the
sample that happened to need it first. Prefer names like
`guarded_transition_dispatch` or `bounded_float_call_unproven` over
story-shaped names like `dungeon_step_01`.

## Build Output

Canaries may emit a local `build/` directory when run through the real CLI.
Those artifacts are intentionally ignored. They are useful scratch evidence,
not source.

If a canary needs a permanent expected output, keep that expectation as a small
checked-in text file beside the canary instead of preserving the generated
build directory.

## Relationship To Samples

Samples are miniature projects that should read like user code.
Canaries are compiler tripwires. When a sample exposes a missing feature, add
or extend the smallest canary that proves that feature in isolation, then return
to the sample once the tripwire is green.
