# UEFI bootstrap implementation

Contract: [UEFI entry and handoff](../../../../../wiki/spec/build/uefi_entry.md).

`src/uefi_bootstrap.rs` owns invocation-private returning-application ledger
authority and phase leases. `provider_projection.rs` and
`handle_protocol_provider/` under that module join exact input provenance to
selected native operands and execution receipts. Equal public report IDs cannot
mint another ledger's authority. The separate `os_handoff.rs` models the bounded
map/exit cycle and returns complete linear custody on stale keys or exhaustion.
`os_handoff_cycle.rs` is the caller-side composition that drives that ledger
across the `get_memory_map` and `exit_boot_services` provider edges in the only
legal order — acquire the freshest map, bind its key into the exit attempt,
execute and admit, then apply — returning complete live custody on every
rejection. Both edges consume the `program-entry-plan` OS-handoff invocation
plan rather than local target constants: each planned invocation retains its
`GetMemoryMap` or `ExitBootServices` leg (service-table row, Microsoft-x64
call shape, closed status table with custody roles), the join checks the
sealed row against that leg, preparation and every later custody transition
replay the retained leg against a freshly derived plan and the provider's row,
status classification reads the leg's roles, and the ledger admits an
exhaustion status only through the plan's error predicate. That state model
alone does not qualify final-map physical memory, and the generated
custody-transfer adapter that `source/library/std/targets/uefi_x86_64/handoff.omg`
names as its realization is not built: the authored surface stays planned and
non-invoked.

## Target evidence and execution

The [target representation](../../../representations/target/src/uefi_system_table/mod.rs)
owns the known 120-byte x64 system-table prefix: eighteen ordered rows,
24-byte header, explicit revision padding, `ConOut` at 64, and Boot Services
at 96. Occurrence integrity validates signature, covered prefix, zero Reserved,
and CRC over the complete declared header with its CRC field zeroed. Covered
forward-compatible suffixes remain valid. Revision is retained for later
capability policy. Neither layout nor integrity projects a service pointer;
phase and occurrence provenance are separate prerequisites.

The bounded HandleProtocol executor consumes a retained service/handle/GUID/
output-slot carrier under the UEFI ABI. Its non-clone receipt seals actual status
and output. Success with unchanged non-null output may enter exact Loaded Image
decoding (96-byte layout, revision `0x1000`, base at 64, size at 72). Errors,
unknown status, drift, or bad geometry retain executed provider custody for
release. The call does not itself establish an image extent or semantic root.

`program-entry-plan` owns target-stack closure and exact physical/semantic entry
matching. The address-free readiness join retains the optimized semantic entry,
both physical occurrences, private firmware lease, and stack plan. It validates
the receiver-free two-root Unit slice and returns all inputs on rejection.
Readiness is neither generated-shell execution nor source invocation, complete
WCSU producer evidence, provider installation, or a returned `EfiStatus`.

The [execution board](../../../../../TASKS.md) tracks these missing consumers;
passing layout, ledger, or state-transition tests does not close them.
