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

## Native container codec

`container_bytes.rs` owns the little-endian `OMEGAXE!` codec: a 64-byte header,
32-byte section-directory records, opaque code/proof spans, 16-byte entry rows,
32-byte relocation rows, and a 64-byte placement record. The ordinary encoder
emits version 2 with eight required semantic sections, including four separate
strong authority commitments (contracts, footprint, regime, installation scope).
Version-1 compatibility encoding is retained separately; those candidates cannot
pass executable admission.

Length arithmetic and limits are checked before slicing/allocation. Ranges must
tile the bytes after the header/directory without overlap, gaps, or unreferenced
suffixes. Reserved fields are zero; unknown required sections reject. Directory
and payload identities agree. Informational sections remain opaque and confer
no semantic authority, but their report identity is derived from kind and exact
bytes rather than trusted from the directory. The encoder validates its own
completed output through the hostile-input decoder before returning it.

Normalized content identity includes exact code, architecture, contracts,
footprint, placement, entries, and canonical relocations. Proof bytes have a
separate strong identity. All four imported-authority commitments participate
in admission, materialization, and final-byte identity; fixed compact report
values cannot hide changed evidence. Signed relocation addends remain retained,
and shared relocation kinds do not permit architecture substitution.

The artifact writer accepts a normalized artifact and exact proof payload, not
an arbitrary final native-image buffer, and installs the encoded file atomically.
This packaging seam is not a complete compiler-to-container evidence translator
or independent PCC verifier. Firmware envelopes are separate from this format.
