# Design Brief: Freestanding Boot And Hardware Facts

Current direction as of 2026-07-18. The model is settled enough to guide
Cathedral/UEFI slices; exact entry, MMIO, and interrupt grammar remains open.

## Freestanding target

Freestanding is a build/image property:

```omega
machine build(b: &mut Build, fs: &mut Filesystem) {
    b.freestanding = true;
    b.entry = Main::run;
}
```

It means no ambient hosted provider set. Every firmware, device, memory-map,
clock, scheduler, or exit service used by the program must be supplied by an
explicit boundary provider admitted for the target.

## Typed entry handoff

The image exports a boundary callable. A target-specific entry stub decodes the
firmware/boot calling convention into a typed handoff and enters it:

```omega
data UefiHandoff {
    image: EfiHandle;
    system_table: &mut EfiSystemTable;
}

boundary machine Main::run(
    &self,
    handoff: UefiHandoff
) -> BootOutcome
    effects Firmware + MemoryMap;
```

The stub/provider is accepted trust. Its contract must state calling-plan,
pointer provenance, alignment, lifetime, paging/CPU assumptions, stack budget,
and any other facts the checker cannot derive from Omega code. These facts are
visible in the boundary report; they are not implicit target folklore.

## Facts, commitments, and authority

Hardware claims use the ordinary semantic homes:

- value/layout propositions are domains or contract facts;
- semantic interpretation is an explicit domain commitment;
- authority is a capability/evidence value;
- service reach is a boundary-trait member in the decision-22 row; and
- trust in firmware/imported behavior is an admission receipt.

Do not invent fake values merely to write an axiom such as “CPU is in long
mode.” A target/provider contract can publish that environmental assumption.
Concrete page tables, memory maps, descriptors, and handles become typed values
once the boundary materializes them.

## UEFI memory-map handoff

UEFI's map/exit protocol is a linear state transition:

1. query the required map capacity;
2. obtain explicit storage;
3. read the map and its `MapKey`;
4. validate/materialize descriptors using firmware-provided stride/version;
5. call `ExitBootServices(image, key)`; and
6. on success, mint a linear `FinalMemoryMap` token.

```omega
data FinalMemoryMap [linear] {
    descriptors: Vec<MemoryDescriptor>;
}

boundary trait FirmwareMemoryMap {
    machine read(
        firmware: &mut FirmwareAuthority,
        storage: &mut MapStorage
    ) -> ReadMapResult;

    machine exit_boot_services(
        firmware: FirmwareAuthority,
        key: MapKey,
        map: Vec<MemoryDescriptor>
    ) -> ExitBootResult;
}
```

The successful outcome creates one `FinalMemoryMap` obligation and consumes
the firmware authority. Retry consumes neither and returns the new required
capacity/key information. No drop path pretends to perform the fallible exit.

Descriptor stepping uses the runtime descriptor stride returned by firmware,
not `sizeof(MemoryDescriptor)`. The format/layout policy must prove each view is
within the supplied map bytes before exposing a typed descriptor.

## Minting memory authority

The final map is the origin for physical-memory authority:

```omega
machine mint_regions(
    map: FinalMemoryMap,
    metadata: &mut RegionMetadataBudget
) -> MintRegionsResult
    requires metadata.remaining >= region_metadata_bound(map)
    ensures forall r in result.regions: r.exclusive and r.backed_by_ram;
```

`Region` construction is owner-controlled. Regions split/attenuate through the
future resource algebra; they are not recreated from integer addresses. The
metadata budget is explicit. Allocator service reach, authority, capacity, and
failure remain separate contract axes.

## MMIO and device access

Device memory is exposed only after a memory-management provider maps a
physical range under suitable authority:

```omega
boundary trait DeviceMemory {
    machine map<T>(
        mapper: &mut MapDeviceAuthority,
        physical: PhysicalRange,
        out: &mut Mmio<T>
    ) -> MapDeviceResult;
}
```

`Mmio<T>` has explicit volatile/ordered operations; it does not coerce to
ordinary `&T`. Revocation removes the mapping/authority according to the
component/resource contract. Reaching `DeviceMemory` never substitutes for
possessing `MapDeviceAuthority`.

## Interrupt entry

An interrupt enters through a target calling plan and restricted boundary
machine. Its pinned contract must fit the interrupt context's floor:

- forbidden `Suspend`/`Block` members are absent;
- stack/frame and execution budgets are explicit;
- reentrancy and shared-state contracts are satisfied;
- acknowledgement/EOI authority is linear where required; and
- the exit restores the target-required machine state.

Interrupt handlers normally post bounded events to scheduler/device queues and
return. They do not gain a second concurrency or wait model.

## Required artifacts

A freestanding build report should include:

- image entry and calling-plan identity;
- accepted target/firmware assumptions;
- boundary service reach and provider receipts;
- physical/virtual memory and stack budgets;
- minted authority roots and attenuation paths;
- interrupt entry registrations; and
- any opaque hardware behavior the checker cannot model.

## Still open

- exact build/entry declaration grammar;
- UEFI provider and descriptor-layout source packages;
- the first `Region` split/merge resource algebra;
- MMIO ordering and volatile operation spelling;
- interrupt calling plans and acknowledgement-token types; and
- section/physical-address placement in image emission.
