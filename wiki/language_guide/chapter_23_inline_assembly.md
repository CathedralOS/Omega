# Inline Assembly

Inline assembly is the checked low-level operation surface for OS and driver
work. It is not an opaque text or byte escape from Omega's control-flow,
ownership, effects, authority, or machine-state rules.

The full catalog and contract surface remain incremental. The implemented pilot
accepts strict blocks containing known `hlt`, port `in`/`out`, x86
`lfence`/`sfence`/`mfence`, and `jmp state(...)` instructions. Multiple
instructions use `;` as an explicit separator because newlines are not grammar;
empty blocks and a control transfer followed by another instruction reject.
Catalogued entry/exit operations such as `iretq` and `eret` already refuse as
**deriver only** rather than falling through an unknown-mnemonic path. Returns,
calls, and indirect branches refuse as hidden exits, while recognized
load/store spellings refuse until they carry structured provenance and
permission contracts.

The port-I/O pilot already uses structured operand constraints. `out` accepts
an exact `u16` port place (or a fitting literal) and an exact `u8` value place
(or a fitting literal); `in` accepts an exact writable `u8` destination and the
same `u16` port class. These constraints are target-register constraints, so a
wider integer place never narrows implicitly. Each catalog operand records its
source role, read/write access, exact primitive class, literal policy, and
architectural register: port values bind to `dx`, byte values and destinations
to `al`. The shared catalog also records the registers clobbered by each
realized sequence: the current `out` lowering uses `rax`, `rdx`, `r10`, and
`r11`, while `in` uses `rax`, `rdx`, `r10`, and `r15`.

An author may restate a block's exact realized register footprint with
`asm where clobbers ... { ... }`. The declaration is checked against the union
of the compiler-owned instruction contracts: omitting a changed register and
inventing an unchanged register both reject, while order and duplicates carry
no meaning. A block with no general-purpose register changes spells
`clobbers none`.

The three x86 fences are zero-operand instructions with explicit load, store,
or full memory-ordering metadata and an empty service-reach effect set. They do
not invent a boundary service reach or a register clobber. Their x86-only
target gate is enforced at realization, and the backend emits the exact
`0f ae /5`, `/7`, and `/6`
encodings; selecting an AArch64 target refuses instead of substituting a
differently named barrier.

The same `where` surface accepts boolean `requires` and `ensures` facts:

```omega
asm where
    requires self.ready
    clobbers rax, rdx, r10, r11
    ensures self.ready
{
    out self.port, self.value
}
```

A `requires` clause is proved at block entry. An `ensures` clause is proved at
the falling-through block exit after instruction writes have invalidated stale
facts. These clauses are assertions only: neither mints a fact, admits an
unknown instruction, nor overrides its compiler-owned contract. An `ensures`
clause rejects on `hlt` and `jmp` blocks because those instructions have no
local post-state.

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
- atomics and the completed x86 fence slice;
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
- The retired `Binding::Instruction` path has no compatibility spelling;
  parsed checked assembly is the only source-level instruction surface.

See
[`OS Memory And Hardware Foundation`](../design_briefs/os_memory_and_hardware_foundation.md)
and [`Calling And Machine-State Plans`](../design_briefs/calling_plans.md).
