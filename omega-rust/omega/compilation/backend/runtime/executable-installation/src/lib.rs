/*
We model normalized executable-artifact admission, placement, installation, and
post-handoff writer context resolution through linear state carriers (`Artifact`,
`ValidatedPlacement`, `InstalledCode`, and
`ResolvedPostHandoffEntryWriterContext`).

Decoding a container or materializing bytes at a destination never establishes
execute authority on its own: each transition consumes linear evidence from the
previous step and returns its inputs inside `InstallationError` or
`DestinationClaimError` when a gate refuses.
*/

/*
Why separate 64-bit report identities (`ArtifactId`, `InstalledCodeId`, etc.)
from 32-byte SHA-256 authority commitments (`ImportedContractSetDigest`,
`DeclaredFootprintDigest`, `MachineRegimeDigest`, `InstallationScopeDigest`)?
Compact 64-bit values are convenient for diagnostic tables and compatibility
reports, but relying on them for admission or installation would collapse the
trust boundary to a 64-bit FNV/compatibility space. We therefore keep report
identities strictly for indexing and diagnostics while requiring full
domain-separated SHA-256 digests and exact placement geometry on every
state-changing receipt.
*/

/*
The crate is organized into five domain modules:
- `authority_digests.rs`: domain-separated SHA-256 authority digests, non-zero
  report IDs, and compatibility fingerprints.
- `artifacts.rs`: decoded executable artifacts, entry stubs, normalized
  relocations, and admitted artifact custody.
- `placement.rs`: linear W+NX to R+NX placement states (`CodePlacement`,
  `FrozenPlacement`, `ValidatedPlacement`) and materialized byte evidence.
- `installation.rs`: installation authorities, provider receipts, W^X
  enforcement status, and linear `InstalledCode`.
- `post_handoff_writer.rs`: destination mapping validation and once-resolved
  opaque writer contexts (`ResolvedPostHandoffEntryWriterContext`).
*/

mod artifacts;
mod authority_digests;
mod installation;
mod placement;
mod post_handoff_writer;

pub use artifacts::Artifact;
pub use authority_digests::{
    AccessPlanId, AdmissionReceiptId, ArtifactContentDigest, ArtifactId, CodePlacementId,
    DeclaredFootprintDigest, DestinationPreparationReceiptId, EntryContractDigest,
    EntryReferenceFactDigest, EntrySetId, FinalBytesDigest, FinalValidationId,
    ImportedContractSetDigest, InstallationFactDigest, InstallationScopeDigest,
    InstallationScopeId, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MachineRegimeDigest, MappingQuarantineId, NonAuthoritativeContainerFingerprint64,
    NonAuthoritativeInformationalFingerprint64, NonAuthoritativeWriterContextFingerprint64,
    RelocationSetId, ReplacementFactDigest, RetirementFactDigest,
};
pub use installation::{InstallationError, InstalledCode};
pub use post_handoff_writer::{
    ResolvedPostHandoffEntryWriterContext, ValidatedPreparedPostHandoffWriterDestination,
};
