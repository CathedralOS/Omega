# Qualified foreign input

On macOS ARM64, `omega run main.omg` calls libc `abs` through a boundary trait
with a predicate-qualified scalar argument. Expected exit: **0**. The calls
cover a literal, local, field, forwarded parameter and later state; the domain
excludes the unrepresentable `abs(INT_MIN)` case.
An additional literal-subject requirement checks that scalar propositions do not
need a structural argument slot.

The interpreter does not supply a trusted handler for this libc import, so its
decline is not an execution oracle. The checked-provider counterpart belongs
to the provider corpus and runs in both engines. This sample does not claim
Windows or Linux runtime coverage.
