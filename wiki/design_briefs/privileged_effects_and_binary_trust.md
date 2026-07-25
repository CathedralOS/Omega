# Design Brief: Privileged Instructions And Binary Trust

Current as of 2026-07-22. The instruction-contract, admitted-executable
installation, and checked-Omega return-integrity rules are settled. Concrete
manifest encoding, authority-token implementation, and trusting-trust/PCC
engineering remain open.

## Core rule

Inline assembly is a checked intrinsic surface, not opaque bytes. Every accepted
instruction contributes a complete normalized contract:

- service reach;
- required authority;
- register, flag, and memory changes;
- control/failure behavior; and
- target applicability.

An instruction without a compiler-known contract is rejected. Raw `db`/byte
escape hatches do not compile merely by declaring a conservative effect row.

## Separate service families

Do not create one catch-all `Privileged` service. At minimum:

- `MachineControl`: `hlt`, interrupt enable/disable, control/MSR operations;
- `PortIo`: x86 port input/output; and
- `Mmio`: volatile access to mapped device memory.

These services have different enforcement substrates. Port I/O can be mediated
by hardware permission maps; MMIO authority can be represented by admitted
mapping capabilities; machine-control operations generally require execution
inside the trusted kernel/boot domain.

The names are boundary-trait/service identities under decision 22, not global
lowercase effect keywords.

## Reach, authority, and trust

The three ledgers remain independent:

- The normalized effect row reports which privileged service may be reached.
- Capability/evidence values prove that this call possesses the required
  authority.
- Provider/admission receipts report why the implementation contract is
  trusted.

Listing `MachineControl` in a row never mints owns-the-machine authority.
Possessing authority never allows a caller whose published ceiling omits the
service to reach it.

## Binary trust

Hardware can mediate some operations from an untrusted ring, but a native
binary's self-reported manifest is still only a claim. Arbitrary machine code
can omit the declaration and execute bytes directly when the CPU permits it.
Omega therefore never converts raw bytes into host code. A prebuilt artifact
must acquire the sealed `Artifact::AdmittedExecutable` fact through admission;
installation then borrows that reusable artifact and consumes scoped linear
placement authority. The final placed bytes are validated before the provider
establishes execute permission.

The sound baseline is source-distributed, host-compiled code:

1. the trusted compiler derives contracts from accepted instructions;
2. it validates authority and the caller's published ceiling;
3. it emits normalized reach/authority/trust artifacts; and
4. the host treats the resulting binary as its artifact, not as an untrusted
   statement about itself.

This remains conditional on trusting the compiler or independently checking
the admitted proof/certificate. Diverse compilation, bootstrap verification,
and proof-carrying code are separate upgrades to that TCB story. Installation
prevents injection. Backward-edge return integrity in checked Omega follows
from memory safety plus non-addressable compiler-owned live and parked control
state; WCSU proves sufficient stack capacity. Forward-edge indirect targeting
instead depends on sealed entry references and descriptors retaining
requirement/satisfier identity.

The boundary of that derivation is explicit. Checked assembly cannot omit the
instruction catalog's stack/control effects. An opaque provider must supply an
admitted `CallPlan + StatePlan` covering its exits or remain hardware-isolated;
unknown evidence fails closed. DMA receives only lent extents and therefore
cannot address task/control storage it was not granted. An independent
final-byte transfer certificate and CET, PAC, or shadow-stack hardening are
future PCC/TCB-reduction layers, not prerequisites for v1 semantics.

## Cathedral M3 slice

The first useful slice is:

- compiler-known `hlt`, `in`, and `out` intrinsics;
- contracts that distinguish `MachineControl` from `PortIo`;
- rejection of raw/unknown instruction bytes;
- an explicit boot-root owns-the-machine authority until the general
  capability lifecycle lands; and
- artifacts showing service reach, authority use, and accepted compiler trust
  separately.

The subsequent x86 catalog now also carries `cli`/`sti` as `MachineControl`
operations with explicit `MachineOwner` authority, target applicability, and
RFLAGS.IF transitions (including STI's delayed recognition rule). Structured
`pushfq <destination>`/`popfq <source>` operations preserve RSP, use exact
`u64` places, and give restore the same `MachineControl`/`MachineOwner`
contract. Their higher-level saved-mask/restore protocol remains an ordinary
provider-minted linear token. Its source contract is now live as the opaque
`InterruptMaskControl`/`InterruptMaskGuard` pair in
`omega::language::core::interrupt`; provider minting/lowering remains.

`omega::language::core::assembly` now defines the canonical empty boundary
traits `MachineControl` and `PortIo`. Compiler-known asm calls resolve those
symbol-backed identities into the ordinary recursive service-reach fixed
point. Direct emission sites must import and publish the matching service, and
every known checked caller on a path to the instruction must publish it as
well; diagnostics retain the normalized call path. This admission consumes
the same canonical rows as all other service-reach checks; no lowercase/u64
projection remains.

Authority remains orthogonal: the same catalog entry independently carries
`MachineOwner`, `IdtControl`, or no authority, and listing a service row cannot
discharge it. Cathedral's UEFI source now names firmware boundary reach as
`SimpleTextOutput`/`BootServices` and direct instruction reach as
`PortIo`/`MachineControl`, rather than collapsing either into `device_io`.

Structured `rdmsr <destination>, <index>` and `wrmsr <index>, <value>` now
model the implicit ECX and EDX:EAX registers as explicit exact `u32`/`u64`
value flow. Both carry `MachineControl`, require `MachineOwner`, enumerate the
realized architectural and scratch clobbers, and refuse non-x86 targets.

Structured `read_cr0`/`read_cr2`/`read_cr3`/`read_cr4` and
`write_cr0`/`write_cr3`/`write_cr4` likewise expose exact `u64` value flow,
carry `MachineControl`, require `MachineOwner`, and refuse non-x86 targets.
CR2 intentionally has no write form. The instruction contract does not by
itself claim that a new paging or execution regime is valid; provider-level
facts must discharge those transitions.

`lidt` is now a contracted but deriver-only x86 operation. It requires the
distinct `IdtControl` authority, consumes a private `IdtDescriptor` place
through scratch R10, declares that exact clobber, and has the pinned
`41 0f 01 1a` (`lidt [r10]`) encoding. User assembly rejects before operand
lowering. Only the installed-IDT provider may issue it after content-bound
materialization and root-record preparation; the catalog entry therefore does
not reopen a raw table-installation escape.

## Still open

- concrete executable/component manifest encoding;
- the first-class owns-the-machine authority type and attenuation rules;
- admission policy for prebuilt third-party binaries without checkable PCC;
- optional independent final-byte control-transfer certificates and
  target-hardware CFI hardening;
- diverse compilation/trusting-trust defenses; and
- proof-carrying code for checking an untrusted binary without rebuilding it.
