# Design Brief: Privileged Instructions And Binary Trust

Current as of 2026-07-18. The instruction-contract and admitted-executable
installation rules are settled. Concrete manifest encoding, authority-token
implementation, protected-return CFI, and trusting-trust/PCC engineering
remain open.

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
by hardware permission maps; MMIO authority can be represented by page-table
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
prevents injection; protected returns and final-artifact CFI remain a separate
gate over all code, including the boot-admitted installer.

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

## Still open

- concrete executable/component manifest encoding;
- the first-class owns-the-machine authority type and attenuation rules;
- protected returns and the final-artifact CFI certificate;
- admission policy for prebuilt third-party binaries without checkable PCC;
- diverse compilation/trusting-trust defenses; and
- proof-carrying code for checking an untrusted binary without rebuilding it.
