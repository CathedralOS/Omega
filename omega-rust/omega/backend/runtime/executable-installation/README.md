# Executable-installation foundation

Contract: [admitted executable installation](../../../../../wiki/spec/build/executable_installation.md).
[lib.rs](src/lib.rs) owns artifact/placement admission and the linear lifecycle.
[container.rs](src/container.rs) and [container_bytes.rs](src/container_bytes.rs)
own container validation; [materializer.rs](src/materializer.rs) resolves sealed
sources; [replacement_quarantine.rs](src/replacement_quarantine.rs) retains
incompletely drained installed realizations.

Keep exact bytes, relocation/proof payload, placement lineage, final-byte
snapshot, footprint, audience, and provider receipts behind report identities.
Content, proof, and final-byte digest domains are separate. Container-v1 compact
content values do not replace those acceptance premises.

The compiler's frozen text plus immutable dynamic-conformance-table consumer
does not establish a mutable-data or BSS installation model: its section gap is
canonical zero padding and the whole extent remains read-only. A successful
post-handoff writer similarly produces unpublished bytes, not an established
hardware table or publication authority. Physical invocation, consumer semantic
validation, source linear integration, and PCC/final-code admission must each
be supplied by their actual consumers; structural lifecycle tests do not attest
those operations. [TASKS.md](../../../../../TASKS.md) owns unfinished work.
