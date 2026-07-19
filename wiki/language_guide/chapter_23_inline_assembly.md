# Inline Assembly

Inline assembly is the checked low-level operation surface for OS and driver
work. It is not an opaque text or byte escape from Omega's control-flow,
ownership, effects, authority, or machine-state rules.

The full catalog and contract surface remain incremental. The implemented pilot
accepts strict blocks containing known `hlt`, port `in`/`out`, and
`jmp state(...)` instructions. Multiple instructions use `;` as an explicit
separator because newlines are not grammar; empty blocks and a control transfer
followed by another instruction reject. Catalogued entry/exit operations such
as `iretq` and `eret` already refuse as **deriver only** rather than falling
through an unknown-mnemonic path. Returns, calls, and indirect branches refuse
as hidden exits, while recognized load/store spellings refuse until they carry
structured provenance and permission contracts.

## Parsed instructions with contracts

`asm { ... }` is parsed for the selected target. Every accepted instruction has
a compiler-owned contract that can contribute:

- operand type/width and target-feature requirements;
- address provenance, bounds, alignment, initialization, and permission;
- boundary-service reach and required authority;
- register, flag, memory, and machine-state changes;
- ordering, atomicity, and cache/TLB effects;
- required and established machine regime; and
- all possible control exits.

The surrounding Omega facts discharge those obligations exactly as they do for
ordinary operations. An instruction with an unsatisfied contract rejects. A
package cannot silence the obligation by moving the instruction into a helper.

```omega
machine Interrupts::save_and_mask(
    authority: &mut InterruptControl,
) -> InterruptMask
{
    asm {
        pushfq;
        cli
    }
}
```

This sketch does not make `cli` safe by spelling it. Its contract requires the
appropriate authority, contributes the normalized interrupt-control reach,
records flag/state changes, and participates in construction of the linear
restore token.

## No quiet spelling

Direct assembly and an abstract boundary operation for the same mechanism must
contribute the same normalized reach and authority requirement. `asm { wrmsr }`
cannot bypass a package policy that rejects `MachineControl` merely because the
author avoided a wrapper trait.

Containing assembly does not automatically make a machine a provider. If all
instruction obligations are discharged from checked Omega facts and held
capabilities, the machine remains checked. Claims that cannot be proved cross
the ordinary provider-admission spine and appear in receipts/reports.

Unknown instructions and raw emitted bytes are rejected. An admitted prebuilt
blob is a foreign provider artifact, not inline assembly.

## Control flow remains Omega control flow

Known jumps and branches map to declared Omega states/transitions. Hidden labels,
loops, returns, unwinds, and control transfers are invalid. For example:

```omega
asm {
    jmp next_state()
}
```

is a low-level spelling of an ordinary transition whose target, arguments,
invariants, and live obligations remain checked.

The instruction catalog records an availability class:

- **user checked**: an author may spell the instruction when its complete
  contract can be discharged; or
- **deriver only**: generated entry/exit machinery may use it, but ordinary
  assembly may not.

Return-from-interrupt and similar operations are deriver-only because allowing a
handler to spell them would create an unmodeled exit around its entry contract.
Direct branches remain final-artifact-validated targets. Indirect calls and tail
calls must consume sealed, requirement-compatible entry references rather than
numeric addresses. Protected returns and the complete final-artifact CFI
certificate remain the separate owner question; admitted-artifact installation
does not discharge them.

## Machine-state regimes

Regime-changing instructions state transitions directly. A far jump or control
register write may require one mode and establish another. Calling policies
describe stable entry regimes; they do not pretend a real-to-protected-to-long
transition is one exotic ABI.

The complete boundary entry policy also carries a `StatePlan` describing
interrupted state, save/restore behavior, and the register/machine-state classes
the transitive handler may use. Source syntax cannot prove that a handler is
SIMD-free: optimization, register allocation, and callees may introduce SIMD.
The backend therefore emits an actual footprint certificate and final-artifact
validation checks it against the state ceiling.

## Memory and hardware instructions

Assembly loads/stores do not manufacture authority from an integer address.
Their contracts require an authorized extent/view or another specific provider
grant. MMIO field access normally uses plan-derived operations rather than
hand-written loads, but the underlying instruction is checked by the same rules.

Cache maintenance, admitted-artifact installation, TLB invalidation/shootdown,
DMA completion, and cross-core instruction-fetch visibility are target-specific
contracted sequences. Public APIs should expose the semantic operation; checked
provider implementations own the instruction sequence. Assembly cannot create
an executable mapping from ordinary bytes: relevant instructions require the
same admitted-artifact provenance, scoped installation authority, and reach as
the page-table/provider path.

## Required initial catalogs

The freestanding x86 vertical slice needs contracts for:

- `cli`, `sti`, `hlt`, flags save/restore;
- `in`/`out` port I/O;
- `lidt`/`lgdt`, control-register and MSR access;
- atomics and fences;
- cache/TLB maintenance and invalidation;
- mode-transition operations; and
- generated interrupt/syscall entry and return sequences.

The AArch64 slice must include the corresponding system-register, barrier,
cache-maintenance, exception-entry, and regime-transition operations. This is
incremental catalog engineering over one model, not a new feature for every
instruction family.

## Working rules

- Assembly is parsed and target-checked; it is never an opaque byte block.
- Every accepted instruction emits a complete normalized contract.
- Effects, authority, and trust cannot be laundered through assembly helpers.
- Hidden control flow and user-authored entry/exit protocol bypasses are invalid.
- Memory operations preserve Omega provenance, permission, and invariant rules.
- Machine-state footprints are validated against the complete boundary
  `StatePlan` after final code realization.
- `Binding::Instruction` retires as the checked catalog covers its customers.

See
[`OS Memory And Hardware Foundation`](../design_briefs/os_memory_and_hardware_foundation.md)
and [`Calling And Machine-State Plans`](../design_briefs/calling_plans.md).
