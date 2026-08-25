# Inline Assembly

Inline assembly is the checked low-level operation surface for OS and driver
work. It is not an opaque text or byte escape from Omega's control-flow,
ownership, effects, authority, or machine-state rules.

The full catalog and contract surface remain incremental. The implemented pilot
accepts strict blocks containing known `hlt`, port `in`/`out`, x86
`lfence`/`sfence`/`mfence`, x86 `cli`/`sti`, structured x86
`pushfq`/`popfq`, structured x86 `rdmsr`/`wrmsr`, structured x86
control-register reads/writes, and `jmp state(...)`
instructions. Multiple instructions use `;` as an explicit separator because
newlines are not grammar; empty blocks and a control transfer followed by
another instruction reject.
Catalogued entry/exit operations such as `iretq` and `eret` and the x86 IDT
load operation `lidt` refuse as **deriver only** rather than falling through an
unknown-mnemonic path. `lidt` has a real closed contract: consumer-supplied
CPU/table publication authority, an exact descriptor place, and the realized
scratch clobber. Only an admitted provider holding the consumer-established
table and root records may consume it. Omega owns this instruction contract,
not an IDT lifecycle or IDT-specific public carrier.
Returns, calls, and indirect branches refuse as hidden exits, while recognized
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
realized sequence: the current `out` lowering uses `rax`, `rdx`, `r10`, `r11`,
and `r15`, while `in` uses `rax`, `rdx`, `r10`, and `r15`.

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

`cli` and `sti` are zero-operand x86 instructions with `MachineControl` reach,
an explicit `MachineOwner` authority requirement, and no general-purpose
register clobbers. Their contracts record that `cli` clears RFLAGS.IF before
the next instruction, while `sti` does not recognize maskable interrupts until
after the following instruction. The current authority discharge admits them
only in a freestanding boot root; listing `reaches machine_control` in hosted
code does not mint authority. Higher-level interrupt-control providers must
still expose save/restore as the ordinary linear token described below rather
than leaking a bare unmask operation into application code.

`pushfq <destination>` and `popfq <source>` are structured x86 value
operations over exact `u64` places. They do not expose the architectural
stack effect directly: snapshot lowers to `pushfq; pop scratch; store`, and
restore lowers to `load; push scratch; popfq`, leaving RSP unchanged in both
cases. Snapshot has no service reach or authority requirement. Restore carries
`MachineControl`, requires `MachineOwner`, and records that RFLAGS.IF is
restored from the operand. A literal cannot stand in for a saved-flags place.
The higher-level provider will wrap this value flow in the ordinary linear
`InterruptMask` protocol; the instruction contract itself does not invent
special-purpose linearity.

`rdmsr <destination>, <index>` and `wrmsr <index>, <value>` expose the
architectural EDX:EAX pair as one exact `u64` value and the ECX selector as an
exact `u32` index. The read stores into an explicit writable place; the write
splits its source into the architectural low/high halves. Both are x86-only,
carry `MachineControl`, require `MachineOwner`, and declare every scratch or
architectural register changed by their realized sequences. A higher-level
MSR provider therefore cannot hide reach or authority by choosing direct
assembly.

`read_cr0`/`read_cr2`/`read_cr3`/`read_cr4` each copy the named x86 control
register into an exact writable `u64` destination. `write_cr0`/`write_cr3`/
`write_cr4` accept one exact `u64` source; CR2 is read-only on this surface.
Every form is x86-only, carries `MachineControl`, requires `MachineOwner`, and
declares the scratch registers used to materialize or store its value. These
operations expose register value flow only: broader regime transitions and
address-translation invariants remain obligations of the provider that
uses them.

The same `where` surface accepts boolean `requires` and `ensures` facts:

```omega
asm where
    requires self.ready
    clobbers rax, rdx, r10, r11, r15
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
machine critical(control: &mut InterruptMaskControl)
reaches machine_control
{
    let guard: InterruptMaskGuard = control.save_and_mask();
    // Checked work while the prior mask state is held.
    guard.restore();
}
```

The public contract above does not make `cli` safe by hiding it. The selected
provider's checked implementation still uses `pushfq`/`cli`; those instruction
contracts require the appropriate authority, contribute the normalized
interrupt-control reach, and record flag/state changes. The public
`InterruptMaskGuard` carrier is opaque boundary data; compact settlement
identities remain provider-owned representation. Its routed `Active`
qualification records valid issuance and forces the caller to consume
`restore`; package code cannot inspect or reconstruct fields, copy the linear
value, or directly restore the saved state.

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
numeric addresses.

Backward-edge return integrity needs no separate source property. In checked
Omega, return and continuation state is compiler-owned, not addressable as
ordinary data, and protected by memory safety; WCSU provisioning additionally
proves the physical stack bound is not exhausted. The instruction catalog must
therefore describe every stack/control mutation exactly. An instruction or
sequence that cannot preserve a modeled exit is deriver-only or rejected; an
author cannot override or omit the catalog's effects.

Forward-edge integrity remains distinct: runtime indirect calls require sealed
entry references or descriptors retaining requirement/satisfier identity.
Opaque providers must supply an admitted `CallPlan + StatePlan` including their
permitted exits or remain behind adequate hardware isolation; missing evidence
fails closed. An independent final-byte transfer certificate and CET, PAC, or
shadow-stack hardening are deferred PCC/TCB-reduction work, not language
semantics.

External-root admission enforces that distinction directly. Provider body
footprint evidence is checked against the state-use ceiling, while a separate
exit realization must match the plan's exact return-control mechanism and
restored-state set. An opaque execution can substitute adequate hardware
isolation only through a trust receipt already reported by that root; missing,
unreported, or plan-drifted evidence cannot mint `ProviderExecution`.

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

A compiler-selected implicit freestanding program entry is the admitted boot
root rather than a hosted caller. Its normalized ceiling additionally permits
the instruction-pointer, balanced-stack, and control-state use required by the
checked machine-control catalog. This compatibility entry rule never widens an
explicit source-selected boundary `StatePlan`; authored interrupt, firmware,
or callback plans remain exact authority.

Floating arithmetic instruction contracts additionally require the target's
canonical masked semantic-control state. For binary32/binary64 this includes
the selected rounding rule and gradual-underflow controls such as x86 FTZ/DAZ
or their AArch64 equivalents; sticky status flags remain outside the invariant.
An admitted instruction satisfier is valid only under that precondition.
Checked assembly that changes the relevant controls must establish the matching
state transition and cannot return to ordinary Omega code without restoring the
canonical state.

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
any other address-translation provider path.

Assembly participates in contracts in two independent directions. Each
instruction emits requirements on target features, authority, state, and
surrounding placement; every requirement must be discharged or the assembly
rejects. Separately, a checked block may provide a conformance to callers. That
evidence is derived when the instruction model proves the conformance and may
come from an admitted provider only when the selected profile permits that
trust class. Admission never means leaving an emitted requirement open.

## Required catalogs

The freestanding x86 vertical slice needs contracts for:

- `cli`/`sti`, `hlt`, and structured flags save/restore;
- `in`/`out` port I/O;
- deriver-only `lidt` and `lgdt`, plus structured control-register and MSR
  access;
- atomics and x86 fences;
- cache/TLB maintenance and invalidation;
- mode-transition operations; and
- generated interrupt/syscall entry and return sequences.

The AArch64 slice must include the corresponding system-register, barrier,
cache-maintenance, exception-entry, and regime-transition operations. This is
incremental catalog engineering over one model, not a new feature for every
instruction family.

## Working rules

- Assembly is parsed and target-checked; it is never an opaque byte block.
- Every accepted instruction emits complete normalized requirements, all of
  which must be discharged, plus its modeled effects and provided facts.
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
