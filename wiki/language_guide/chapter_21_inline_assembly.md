# Chapter 21: Inline Assembly

Assembly is a checked low-level operation surface, not an escape from Omega's
ownership, authority, effects or control flow. The [assembly specification](../spec/language/assembly.md)
defines the contract; the [instruction catalog note](../../omega-rust/psi/foundation/language-core/inline_assembly.md)
records current source and target coverage.

## Parsed instructions with contracts

Each accepted instruction has structured operands and a compiler-owned contract.
For x86 port output, for example:

```omega
asm {
    out self.port, self.value
}
```

The port is exactly `u16`, the byte value exactly `u8`; a wider place does not
narrow implicitly. Required authority and service reach remain obligations of
the enclosing machine. Several instructions use explicit `;` separators.

A block may state assertions and its exact realized register footprint:

```omega
asm where
    requires self.ready
    clobbers none
    ensures self.ready
{
    lfence; sfence
}
```

Here the x86 fences change ordering, not general-purpose registers. `requires`
is proved on entry and `ensures` on fallthrough after invalidating stale facts.
Neither clause manufactures evidence or overrides an instruction contract.
An authored clobber set must be exact: both omitted and invented changes reject.
`hlt` and `jmp` blocks have no local post-state for `ensures`.

## No quiet spelling

Direct assembly cannot hide a mechanism's service reach or authority by avoiding
a wrapper. A helper containing `wrmsr` still owes machine-control authority.
Conversely, a machine remains checked when its complete instruction obligations
are proved; assembly does not automatically make it an opaque provider.

High-level interrupt APIs normally use an opaque linear guard: save and mask,
perform checked work, then consume the guard to restore the prior state. The
instruction supplies register/state behavior, not special-purpose linearity or
permission for callers to reconstruct saved-state tokens. The
[interrupt contract](../spec/build/interrupt_obligations.md#mask-and-acknowledgement-custody)
owns the guard's opaque representation, routed qualification and exact restore.

Unknown instructions and raw bytes reject. Admitted prebuilt code uses the
ordinary foreign-provider path.

## Control flow remains Omega control flow

```omega
asm {
    jmp next_state()
}
```

This is a checked transition: its target, arguments, invariants and live
obligations remain visible. Control transfer must be the final instruction.
Hidden labels, loops, returns and unwinds are not another control-flow system.

Some catalog operations are user-checked; entry/exit operations such as
return-from-interrupt are deriver-only. A handler cannot use them to bypass its
entry protocol. Indirect calls require sealed entry identity, not an integer
that happens to resemble a code address.

## Machine-state regimes

A control-register write may change the machine regime. Its contract must name
the required prior state and established state; a mode switch is not merely an
unusual ABI.

The boundary `StatePlan` constrains the complete realized handler and its callees.
Source code that appears SIMD-free may acquire SIMD through optimization or
allocation, so [machine-state evidence](../spec/build/machine_state_evidence.md)
checks the final artifact. Assembly changing floating controls must restore the
canonical Omega controls before ordinary checked code resumes.

## Memory and hardware instructions

An integer address grants no memory authority. Assembly memory operations need
the same provenance, bounds, alignment and permissions as other operations.
MMIO normally uses plan-derived accessors; cache/TLB operations, DMA completion
and executable publication need their exact target contracts.

For example, a checked table writer does not acquire publication authority merely
by producing valid bytes. The consumer establishes the table's semantic validity
and the installer separately publishes it. See [hardware materialization](../spec/build/hardware_materialization.md).

## Required catalogs

Target support grows by adding complete instruction contracts and their checked
realization, not by granting arbitrary mnemonic access. A catalog entry, parser
test or emitted byte sequence alone is not evidence of complete native execution.
Current families and rejected forms are listed beside the catalog.

## Working rules

Keep the semantic operation in the public API and the checked instruction
sequence in its selected implementation. Preserve exact effects, authority,
control exits and state restoration. If those obligations cannot be discharged,
the assembly rejects; a helper or admission receipt cannot leave them open.
