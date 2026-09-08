# Symbolic hardware materialization

Runtime-known addresses remain data requiring separate
[range authority](../resources/extents.md). Toolchain-known code identities do
not become source-visible numeric addresses. A closed symbolic vocabulary
distinguishes sealed data symbols and entry-stub identities. A normalized layout
places those sources, including split fragments, without user address arithmetic.

## Resolution and phases

Each source declares its consumption phase. Resolve at the last legal phase:
fixed-image layout, native relocation, loader relocation, or a generated runtime
writer. A field consumed by the loader before Omega executes must fit the
format's native relocation vocabulary; a post-handoff table may use a writer.

The materialization plan binds range, alignment, consumption phase, machine
regime, and scoped artifact-installation authority. Policy and layout alignment
must agree; concrete-site validation checks the entire occupied range before
consumption. A valid layout alone establishes none of those authorities.

## Generated writers and publication

A checked writer derives from the normalized layout and sealed sources. It
receives one exact mapped, pinned, writable unpublished placement and a resolver
restricted to the admitted artifact. It validates placement and resolves each
sealed target once before writing fragments directly to the destination. No
numeric entry address or arbitrary-offset writer is exposed. Target lowering
retains the complete plan, placement, source identities, final-content validation,
and derived footprint; compact report/cache keys are not authority.

Failure establishes no consumer value. Partial bytes cannot become
hardware-visible merely by occupying storage. Publication is atomic; restoring
the original destination bytes transactionally is not required. Consumer
validation compares the complete current bytes with the exact writer output
before observation; a compact digest alone is insufficient for that join.

The consumer package defines its semantic validator and resulting established
value. For an IDT, the OS checks admitted roots, selectors, gate kinds, privilege,
IST choices, reserved bits, and canonical base/limit. Layout validity is not
hardware-table admissibility, and these policies are not compiler-owned types.

Materialization and installation have separate authorities and receipts.
The writer can write the destination and use its sealed resolver, but cannot
publish a table. The installer requires the consumer-established value and
independent publication authority; it cannot manufacture the bytes. It records
[external roots](external_roots.md) before the checked publication instruction
makes them reachable. Shared establishment/content/linearity infrastructure
does not imply a universal hardware-table/executable typestate algebra.

## Bootstrap safety

A post-firmware writer's software-fault-free certificate combines mapped/pinned/
writable destination and stack facts, WCSU provisioning, checked offsets and
fragment tiling, admitted CPU support, bounded work, and no suspension, blocking,
allocation, dynamic dispatch, or unsupported instruction path. Under those
premises deterministic software faults are excluded. NMI, machine check, and
physical failure remain explicit boot-envelope assumptions, not proved absence.

## Checked instructions

[Checked assembly](../language/assembly.md) owns source syntax, exact block
assertions/clobbers, instruction requirements, regime changes and control flow.

Entry/exit operations such as `iretq` and `sysret` are deriver-only, not ways for
source to manufacture control exits. Provider-only checked `lidt` requires
consumer CPU/table publication authority and accounts for its descriptor read,
scratch clobber, state change, and reach. The compiler owns that instruction
contract, not the OS's IDT lifecycle. [Machine-state evidence](machine_state_evidence.md)
checks the final artifact; prebuilt foreign code uses provider admission.
