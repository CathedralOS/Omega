# Executable-installation foundation

Contract: [admitted executable installation](../../../../../../wiki/spec/build/executable_installation.md).
Start at [lib.rs](src/lib.rs) and [installation.rs](src/installation.rs).
These modules expose admission, placement validation, installation, and
post-handoff writer context resolution in lifecycle order. Each transition
consumes linear inputs and returns them on failure; `lib.rs` re-exports the
public carriers and digests.

Follow the domain modules as needed:

- [Artifacts](src/artifacts.rs) owns decoded artifacts, normalized relocation
  records, and admitted artifact custody.
- [Placement](src/placement.rs) owns linear W+NX and frozen R+NX placement
  states, materialized artifact bytes, and validated placement evidence.
- [Installation](src/installation.rs) owns install authorities, provider
  receipts, W^X enforcement status, and linear `InstalledCode`.
- [Post-handoff writers](src/post_handoff_writer.rs) own destination mapping
  validation and once-resolved writer contexts.
- [Authority digests](src/authority_digests.rs) owns domain-separated SHA-256
  commitments, normalized report identities, and compatibility fingerprints.

Keep exact bytes, relocation payload, placement lineage, final-byte
snapshot, footprint, audience, and provider receipts behind report identities.
Content and final-byte digest domains are separate. Container-v1 compact content
values do not replace those acceptance premises.

The compiler's frozen text does not establish a mutable-data or BSS
installation model: its section gap is canonical zero padding and the whole
extent remains read-only. A successful post-handoff writer similarly produces
unpublished bytes, not an established hardware table or publication authority.
Physical invocation, consumer semantic validation, source linear integration,
and final-code admission must each be supplied by their actual consumers;
structural lifecycle tests do not attest those operations.
[TASKS.md](../../../../../../TASKS.md) owns unfinished work.
