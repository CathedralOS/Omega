# Host boundary readiness

A checked suspending call retains its arguments and continuation until its
admitted host adapter is ready. The native and ordinary Terminal paths select
the authored `Immediate` implementation and both must exit 0; they exercise the
same source contract with inline completion.

`cargo nextest run -p compiler --test terminal_effect_readiness` exercises actual
parking using source-produced Terminal artifacts, both retained and independently
decoded. It checks a nested scalar-result call and a mutable scalar loan: three
repeated readiness waits preserve the preceding effect, original argument/storage,
fuel charge and unfinished result, then resume to the authored result without
replay. The mutable loan begins at 7 and returns to the caller as 42. A
non-suspending call refuses a pending host response before invoking it, and retry
preserves that refusal. Closing a live wait refuses; normal return is repeatable
without repeating effects.

The component entry reserves `Readiness::wait`, parks at the distinct
`ProviderGate::wait`, then installs the explicitly selected `Immediate`
implementation into that same receiver. Its result comes from a real guest
component invocation; host observations are exactly `[1, 0, 42]`. Retained and
loaded forms also reject ordinary missing imports, premature provider calls,
host-adapter substitution, installation at a fuel pause, and rebinding.

This supports provider readiness under **PIPELINE-ARCHITECTURE-ENFORCEMENT** and
**TERMINAL-BUILD-SEED**. It does not establish dependency acquisition,
task migration, or cancellation settlement. Build's
explicit backing selection and application-owned preparation/resumption remain
open.
