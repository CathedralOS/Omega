# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`.

Last pruned: 2026-07-20.

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

## 2. How does a source `Calling<C>` policy evaluate?

Generic trait composition can now retain `Calling<X86InterruptConvention>`,
and the compiler has normalized `CallPlan + StatePlan` records and validators.
The source model still lacks the relationship that qualifies `C` as a policy
and evaluates it against the satisfied requirement's signature.

The decision must preserve these settled constraints:

- never infer a policy from `C`'s spelling, target nickname, import library, or
  binding mechanism;
- validate the evaluated `BoundaryEntryPlan` before it can contribute identity;
- hash the normalized evaluated plan, not merely `C`'s symbol, into the
  requirement contract; and
- keep placement and machine-state vocabularies closed/compiler-validated
  without making Omega's internal calling convention programmable.

Recommendation: make `C` satisfy one sealed core policy requirement whose
compile-time machine computes `BoundaryEntryPlan` from the requirement
signature. Calling layout is genuine computation, so this does not revive an
imperative plan-builder API; the machine chooses from closed plan data and the
compiler validates its result. A smaller alternative is a compiler-owned
closed policy value selected explicitly by `C`, but it needs an honest source
relationship rather than friendly-name recognition. Decide the source/core
spelling and whether user packages may define new policies subject to the same
validator, or only platform packages may do so.

## 3. What is Cathedral's first x86 interrupt state policy?

The normalized `CallPlan + StatePlan` vocabulary and validator are ready, but
the first timer profile cannot be derived until Cathedral chooses the hardware
entry policy that both the stub and installed-root/WCSU analysis must enforce.
Cathedral's own open-question ledger still leaves its initial x86 stack classes,
masking/preemption graph, and WCSU composition unresolved.

Decide, for the first timer root:

- whether it uses the interrupted stack or a dedicated IST stack, and which
  exception/root classes may share or preempt that stack;
- whether the gate masks interrupts for the whole handler, where nesting may be
  re-enabled, and whether the timer root may re-enter itself;
- the exact interrupted machine-state set the stub saves/restores and the
  transitive state ceiling exposed to checked handler code; and
- whether the first acknowledgement token represents legacy PIC EOI, LAPIC EOI,
  or a target-selected protocol with distinct concrete providers.

Recommendation: start with a non-reentrant interrupt gate on a dedicated IST
stack, interrupts masked until deriver-owned exit, a full integer/control-state
save with no SIMD use permitted transitively, and a protocol-neutral linear
acknowledgement requirement refined by PIC/LAPIC providers. This is deliberately
conservative and can later admit nesting or a broader state ceiling, but it is
still an OS policy choice because it fixes stack demand and preemption edges.
