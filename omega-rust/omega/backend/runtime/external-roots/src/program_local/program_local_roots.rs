//! Program-local roots: prebindings, installed occurrences, the
//! installation ledger, epoch cohorts and establishment.
//!
//! `prebindings.rs` carries schema digests and installed prebindings,
//! `occurrences.rs` the installed occurrence, subject and scalar bindings,
//! `activation.rs` the live generated-entry activation token,
//! `installation_ledger.rs` the ledger and capacity evaluation,
//! `epoch_cohorts.rs` cohort members, aggregates and coexistence reports,
//! `establishment.rs` established roots, epoch runtimes and retirement, and
//! `capacity_evaluation_tests.rs` the capacity tests.

mod activation;
#[cfg(test)]
mod capacity_evaluation_tests;
mod epoch_cohorts;
mod establishment;
mod installation_ledger;
mod occurrences;
mod prebindings;

pub use activation::{ProgramLocalEntryActivation, ProgramLocalEntryActivationLeaveError};
pub use epoch_cohorts::{
    InstalledProgramLocalRootEpochCohortId, ProgramLocalRootCoexistenceReport,
    ProgramLocalRootCohortMember, ProgramLocalRootEpochAggregate,
    ProgramLocalRootEpochAggregateCapacity, ProgramLocalRootEpochAggregateSnapshot,
    compose_program_local_root_coexistence_report,
};
pub use establishment::{
    EstablishedProgramLocalRoot, EstablishedProgramLocalRootCapacity,
    InstalledProgramLocalRootEpochCohort, ProgramLocalRootBatchEstablishmentError,
    ProgramLocalRootCohortSealError, ProgramLocalRootEpochRuntime,
    ProgramLocalRootEstablishmentError, ProgramLocalRootLineageId,
    ProgramLocalRootOccurrenceRetirementError, ProgramLocalRootRetirementError,
    RetiredProgramLocalRootOccurrence,
};
pub use installation_ledger::ProgramLocalRootInstallationLedger;
pub use occurrences::{
    InstalledProgramLocalRootOccurrence, InstalledProgramLocalRootOccurrenceId,
    InstalledProgramLocalRootSubject, ProgramLocalRootEntryInvocationId,
    ProgramLocalRootScalarBinding, ProgramLocalRootScalarSource, ProgramLocalRootSubjectPlaceId,
};
pub use prebindings::{
    ProgramLocalRootInstalledPrebinding, ProgramLocalRootInstalledPrebindingCount,
    ProgramLocalRootPrebindingId, ProgramLocalRootSchemaDigest,
};
