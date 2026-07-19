# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`.

Last pruned: 2026-07-18.

## 1. Control-flow integrity and protected returns

Executable installation is settled separately and prevents code injection.
Control-flow integrity is an independent gate over every executable artifact,
including the boot-admitted installer itself.

The forward edge is substantially shaped: direct branches are fixed and
final-artifact validated; indirect calls and tail calls use sealed,
requirement-compatible entry references rather than numeric addresses; dynamic
descriptors retain satisfier/contract identity; checked assembly cannot add an
unmodeled control exit; interrupt/exception exits are deriver-owned under
`CallPlan + StatePlan`.

The remaining owner decision is the backward edge and enforcement contract:

- What normalized fact proves a return corresponds to its legitimate call or
  continuation state?
- Which guarantees come from software proof, protected control storage,
  shadow stacks/CET, PAC, or another target mechanism?
- How do suspension, cancellation, exceptions, interrupts, tail calls, and
  component/provider crossings preserve the return discipline?
- What final-artifact certificate lets an independent validator check every
  indirect call, return, entry stub, veneer, and thunk after placement?
- Must admitted foreign providers supply accepted CFI claims, run behind
  hardware isolation, or both according to policy?

Recommendation: one normalized CFI plan/certificate consumed by the final
validator, with checked Omega producing evidence and opaque leaves either
receipt-gated or isolated. Keep target mechanisms as realizations of the plan,
not source attributes or a new `unsafe` escape.

Detailed surrounding context and engineering residue are in
[`wiki/design_briefs/os_memory_and_hardware_foundation.md`](wiki/design_briefs/os_memory_and_hardware_foundation.md).
