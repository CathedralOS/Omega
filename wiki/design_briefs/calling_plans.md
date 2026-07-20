# Design Brief: Calling And Machine-State Plans

Current as of 2026-07-18. Boundary conventions are normalized policy artifacts;
Omega's internal calling convention remains compiler-sovereign. This brief now
includes inbound machine-state preservation, which ordinary calls do not expose.
Engineering is incomplete. The normalized compiler model and initial policy
evaluators are implemented; source-policy evaluation and authoritative lowering
remain.

## One boundary entry plan, two independent facets

An ordinary ABI is a layout over registers and a stack. It does not, by itself,
describe hardware entering while another activation is live.

```omega
data CallPlan {
    params: [Placement];
    result: Placement;
    ordinary_clobbers: RegisterSet;
    stack_align: count;
    shadow_bytes: count;
    entry_control: EntryControl;
}

data StatePlan {
    initial_regime: MachineRegime;
    interrupted_state: MachineStateSet;
    saved_state: MachineStateSet;
    restored_state: MachineStateSet;
    permitted_transitive_use: MachineStateSet;
}

data BoundaryEntryPlan {
    call: CallPlan;
    state: StatePlan;
}
```

`CallPlan` owns parameter/result placement and ordinary ABI behavior.
`StatePlan` owns the state that already belonged to an interrupted activation,
the state an entry stub preserves, and the state the handler and its callees may
use. Their projections coincide for many ordinary calls; that is not a reason to
fuse their identities.

## Policies are ordinary trait relationships

A boundary requirement pins a target policy through ordinary trait composition:

```omega
trait Calling<C> {
}

boundary trait TimerInterrupt:
    Calling<X86InterruptConvention>
{
    machine handle(frame: &mut X86InterruptFrame, ack: LapicAck);
}
```

`C` is a calling-policy type, not the frame data type. Evaluating it against the
requirement signature produces the complete normalized `BoundaryEntryPlan`.
The evaluated result, not merely the policy symbol, enters the public contract
identity. A target-specific requirement may layer over a portable semantic
service trait; Omega does not need an entry-slot refinement mechanism merely to
reuse the semantic API across targets.

Boundary-trait parents and policy parents have different established meanings:
boundary service parents contribute service reach; ordinary core policy parents
contribute contract identity and no reach.

The exported machine remains ordinary and keeps `boundary` bare:

```omega
boundary machine Kernel::on_timer(
    frame: &mut X86InterruptFrame,
    ack: LapicAck,
) satisfies TimerInterrupt::handle {
    ...
}
```

The old `boundary(InterruptFrame)` / `boundary(MsX64)` modifier spelling is
retired. It fused the boundary marker with deployment policy and duplicated the
requirement's identity.

## Plan derivation and validation

One evaluated plan drives both directions:

```text
calling policy + signature
          |
          v
validated BoundaryEntryPlan
       /             \
outbound encoder   inbound entry/exit stub
```

Plan validation checks, at minimum:

- every parameter and result is placed exactly once and compatibly;
- stack ranges, alignment, shadow space, and register classes are coherent;
- ordinary clobbers match the stable ABI regime;
- saved/restored state covers the `StatePlan` commitment;
- entry/exit control is valid for the initial regime; and
- target and provider applicability match the requirement.

Regime-changing instructions do not turn one calling plan into a multi-mode
blob. Checked instruction contracts require regime R and establish R'. Stable
regions on either side use their own plans.

## Contract identity versus implementation evidence

The authored, evaluated `CallPlan + StatePlan` is published contract identity.
The emitted register/machine-state footprint is provider evidence.

The backend must honor a state ceiling while selecting instructions and
allocating registers, then emit checkable footprint evidence. The final placed
artifact is independently validated after inlining, specialization,
link relaxation, veneers/thunks, generated stubs, and admitted indirect leaves:

```text
actual_transitive_footprint subset_of permitted_transitive_use
actual_clobbers intersect unsaved_interrupted_state = empty
```

A legal change in register allocation or implementation evidence does not alter
caller contract identity. It revalidates the provider artifact only.

Checked Omega leaves produce derived footprint evidence. Raw/admitted leaves
carry accepted footprint claims under receipt. The trust report must distinguish
the two.

A no-SIMD interrupt root may require a callee clone compiled under a no-SIMD
state ceiling. This is contextual codegen specialization, not generic type
monomorphization, although both may share backend cloning and cache machinery.

## Selection and bindings

Plans exist only at boundaries. Most callers do not name one: an external
`Binding` and satisfied requirement determine the pinned policy. Explicit policy
identity is authored on the requirement, never inferred from a DLL name,
syscall number, or friendly target string.

Provider plans remain derived from explicit `satisfies` declarations and `via`
leaves. Admission proves or accepts that a realization refines the complete
boundary plan. See
[`extern_boundary_and_format_domains.md`](extern_boundary_and_format_domains.md).

## External roots

An inbound stub installed for hardware or foreign callbacks is an external root
because it has no Omega caller. The artifact root ledger records its evaluated
boundary plan, provider/artifact/receipt identities, service reach, stack
domain, nesting/preemption relation, and liveness/version pins.

This ledger is also where WCSU composes same-stack interrupt demand and where a
dynamic installation is checked against the artifact-wide bound. Per-machine
validation alone cannot answer those questions.

## Compiler-owned pieces

Policies choose from closed placement and machine-state vocabularies. Compiler
derivers own instruction emission, entry/exit stubs, contextual specialization,
footprint production, and final-artifact validation. Omega's internal convention
is not expressible and may change between releases.

## Engineering order

Generic trait-parent composition used by `Calling<C>` is implemented. Header
parents and body-level `requires` share one validated graph; boundary parents
contribute service reach and ordinary policy parents do not.

The `omega-calling-conventions` foundation now owns normalized
`CallPlan`, `StatePlan`, and `BoundaryEntryPlan` compiler records. It evaluates
MS-x64, SysV-x64, AAPCS64, x86-64 Linux-syscall, and AArch64 Linux-syscall
policies for the currently classified scalar/HFA shapes. Validation rejects
unclassified aggregates, incomplete or overlapping placements, incompatible
regimes, unsaved permitted state, and footprints above the state ceiling.
Register use derives its machine-state class, so evidence cannot hide SIMD use
by omitting a self-reported class. Contract and evidence fingerprints are
separate by type.

Existing compatibility bindings select this normalized policy as an independent
oracle. The process-entry prologue now evaluates the target's normalized native
policy and carries every classified scalar argument's exact register or
ABI-relative stack offset and width through abstract operations, target
operations, layout, and x86-64 or AArch64 emission. Target encoders alone add
the entry return-address or function-frame bias. This removes both the former
backend convention that interpreted an abstract argument index as the
Microsoft x64 register sequence and the global four-register entry limit.
Incoming scalar `f32`/`f64` parameters follow XMM/V locations as well.
Integer process-entry terminal values likewise carry the normalized plan's
exact result register through abstract/target operations and x86-64/AArch64
emission; ISA encoders no longer invent `rax` or `x0`. On AAPCS64, flat records
of one to four contiguous, same-width `f32` or `f64` members classify as HFAs
from the normalized data layout and arrive through the plan-selected vector
register fragments. Nested and general aggregate entry classification and
source-selected policies remain. Generic Linux
syscall leaves are the first outbound path to make the normalized plan
authoritative: emission evaluates the x86-64 or AArch64 syscall policy for the
operand signature, then passes its exact parameter registers, number register,
and supervisor-call immediate to the ISA encoder. The legacy binding's
`number_register`/`supervisor_call` fields no longer select those facts on that
path. Composite runtime-text byte and line syscalls now use the same evaluated
placements. AArch64 emits the plan-selected registers and supervisor-call
immediate; the current fixed x86-64 sequences fail closed when asked to realize
a different normalized plan rather than silently overriding it.
Register-resident AArch64 C/import emission now also evaluates AAPCS64 from the
selected operand shapes and passes the exact planned X/V argument and result
registers to the ISA encoder. Stack-resident and fragmented outbound placements
fail closed until their lowering exists. The Darwin variadic `open` compatibility
seam still handles its anonymous trailing stack argument specially, while its
named arguments and result consume the normalized plan. The general Microsoft
x64 import encoder now receives its policy from the concrete target,
evaluates selected scalar/pointer operand shapes, and consumes the plan's exact
RCX/RDX/R8/R9, shadow-relative stack, and RAX-result placements. A non-Microsoft
x86 target fails closed at this Win64 compatibility encoder. Microsoft x64
vtable and firmware service-table calls now reuse the same plan-driven
marshaller; receiver arguments remain on the wire, dispatch-only table pointers
do not, and result-bearing field calls validate the plan-selected RAX placement
before storage. `GetStdHandle`, `ExitProcess`, and `Sleep` now route through the
same plan-driven general marshaller without changing bytes or relocation sites.
`GetAsyncKeyState` likewise consumes planned RCX/RAX placements while retaining
its compatibility-specific 16-bit zero-extension transform. The composite
Windows time calls now plan their actual native one-pointer signatures: QPC/QPF
also carry an ignored RAX `BOOL` result, while `GetSystemTimePreciseAsFileTime`
is void. Their temporary out slot remains an encoder materialization detail.
Composite `ReadFile`/`WriteFile` sequences now evaluate their actual five-value
native signature and ignored RAX `BOOL` result. Their four register arguments,
shadow-relative fifth argument, and scratch-slot reservation consume that plan;
the scratch slot itself remains an encoder materialization detail. Dedicated
runtime line/byte Windows sequences now reuse the same file layout and validate
the actual one-DWORD/RAX `GetStdHandle` plan without changing their fixed bytes
or relocation sites. AArch64 fragmented calls and concrete firmware
machine-state policy remain.

Scalar AAPCS64 outbound stack arguments now consume normalized stack offsets:
the encoder reserves a 16-byte-aligned outgoing area, materializes integer,
pointer, and float values through caller-saved X/V scratch registers, stores
them at the planned offset, and restores SP after `BL`. Width calculation plus
call/data relocation walkers consume the same stack-prefix/store/restore
accounting. Fragmented outbound placements still fail closed.

Remaining order:

1. Evaluate the policy selected by `Calling<C>` against the requirement
   signature and hash the evaluated pair into requirement identity.
2. Complete plan-driven outbound calls and their results;
   differential-check every supported compatibility encoder against the plan,
   add the concrete firmware/interrupt state policies, and make the plan
   authoritative.
3. Derive outbound encoders and inbound stubs from the same plan.
4. Add state-ceiling-aware instruction selection/register allocation and
   contextual specialization.
5. Emit object-level footprint evidence and validate the final artifact.
6. Add external-root reporting and the x86 interrupt vertical slice.

## Still open

- core-data spelling and source evaluation of the policy selected by
  `Calling<C>`;
- register/machine-state vocabulary extensions beyond the implemented x86-64
  and AArch64 foundation;
- object-certificate composition and final-image validation format;
- admitted indirect-call footprint contracts;
- unwind/non-local-exit representation; and
- the concrete x86 interrupt `StatePlan`, stack classes, and acknowledgement
  requirement used by Cathedral's timer slice.

These are plan/checker/backend questions. They do not justify reviving
`boundary(<Plan>)`, adding an interrupt machine species, or exposing code
addresses as integers.
