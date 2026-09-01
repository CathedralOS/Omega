use crate::declarations::PackageKey;
use crate::review::{
    FreshPackageRootPolicyAcceptance, FreshPackageRootPolicyError, ReviewOnlyRootPolicyResolution,
};
use omega_build_evaluation::{BuildEvaluationUsage, BuildObservationSummary};
use omega_package_compilation::{
    AcceptedSemanticBinding, PackageGeneratedSourceBundle, PackageSourceConsumptionCommitment,
};
use omega_package_evidence::ledger::{
    OrdinaryPackageObligationLedger, OrdinaryPackageObligationResultSet,
    OrdinaryPackageObligationSchemaIdentity,
};
use omega_package_source::{ImmutableSourceResolution, SourceResolveError};

pub const ACCEPTED_ORDINARY_EVIDENCE_SCHEMA_VERSION: u16 = 3;

/// Closed identity for the exact accepted-evidence vocabulary represented by
/// this module. This is distinct from the obligation schema and any future
/// accepted-lock framing version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AcceptedOrdinaryEvidenceSchemaIdentity {
    version: u16,
}

impl AcceptedOrdinaryEvidenceSchemaIdentity {
    pub const fn current() -> Self {
        Self {
            version: ACCEPTED_ORDINARY_EVIDENCE_SCHEMA_VERSION,
        }
    }

    pub const fn version(self) -> u16 {
        self.version
    }
}

/// One exact package artifact and its local derivation provenance.
///
/// Construction is private to the complete closure gate. The ordinary
/// artifact is the complete locally reconstructed obligation ledger, not a
/// compiler verdict, review fingerprint, or native executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedOrdinaryPackageEvidence {
    pub(super) package: PackageKey,
    pub(super) resolution: ImmutableSourceResolution,
    pub(super) source_consumption: PackageSourceConsumptionCommitment,
    pub(super) build_evaluation_usage: Option<BuildEvaluationUsage>,
    pub(super) build_observation: Option<BuildObservationSummary>,
    pub(super) semantic_bindings: Vec<AcceptedSemanticBinding>,
    pub(super) generated_sources: PackageGeneratedSourceBundle,
    pub(super) artifact: OrdinaryPackageObligationLedger,
    pub(super) results: OrdinaryPackageObligationResultSet,
}

impl AcceptedOrdinaryPackageEvidence {
    pub const fn package(&self) -> &PackageKey {
        &self.package
    }

    pub const fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    pub const fn source_consumption(&self) -> PackageSourceConsumptionCommitment {
        self.source_consumption
    }

    pub const fn build_evaluation_usage(&self) -> Option<BuildEvaluationUsage> {
        self.build_evaluation_usage
    }

    pub const fn build_observation(&self) -> Option<&BuildObservationSummary> {
        self.build_observation.as_ref()
    }

    /// Exact consumer semantic roles used to derive this package evidence.
    /// The root policy still owns acceptance of every blocking result exposed
    /// by those roles.
    pub fn semantic_bindings(&self) -> &[AcceptedSemanticBinding] {
        &self.semantic_bindings
    }

    pub const fn generated_sources(&self) -> &PackageGeneratedSourceBundle {
        &self.generated_sources
    }

    pub const fn artifact(&self) -> &OrdinaryPackageObligationLedger {
        &self.artifact
    }

    pub const fn obligation_schema(&self) -> OrdinaryPackageObligationSchemaIdentity {
        self.artifact.schema()
    }

    pub const fn results(&self) -> &OrdinaryPackageObligationResultSet {
        &self.results
    }
}

/// Authority-bearing in-memory acceptance for one exact ordinary package
/// closure.
///
/// The only public construction path reruns source custody, obligation
/// reconstruction, transitive composition, conflict derivation, and root
/// policy replay. This value has no codec, lock mutation route, audit receipt,
/// or `PackageInstance` constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedOrdinaryClosureEvidence {
    pub(super) schema: AcceptedOrdinaryEvidenceSchemaIdentity,
    pub(super) packages: Vec<AcceptedOrdinaryPackageEvidence>,
    pub(super) acceptance: FreshPackageRootPolicyAcceptance,
}

impl AcceptedOrdinaryClosureEvidence {
    pub const fn schema(&self) -> AcceptedOrdinaryEvidenceSchemaIdentity {
        self.schema
    }

    pub fn packages(&self) -> &[AcceptedOrdinaryPackageEvidence] {
        &self.packages
    }

    pub const fn acceptance(&self) -> &FreshPackageRootPolicyAcceptance {
        &self.acceptance
    }

    pub const fn root_policy(&self) -> Option<&ReviewOnlyRootPolicyResolution> {
        self.acceptance.root_policy()
    }
}

#[derive(Debug)]
pub enum AcceptedOrdinaryEvidenceError {
    SourceCustody {
        package: PackageKey,
        error: SourceResolveError,
    },
    SourceSelectionCustody {
        package: PackageKey,
        error: crate::resolution::source::PackageSourceSelectionEvidenceError,
    },
    RootPolicy(FreshPackageRootPolicyError),
    MissingReview(PackageKey),
    ReviewAssociationMismatch(PackageKey),
    AllocationFailed,
}

impl std::fmt::Display for AcceptedOrdinaryEvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceCustody { package, error } => write!(
                formatter,
                "accepted ordinary evidence source custody changed for {package:?}: {error}"
            ),
            Self::SourceSelectionCustody { package, error } => write!(
                formatter,
                "accepted ordinary evidence source selection changed for {package:?}: {error}"
            ),
            Self::RootPolicy(error) => {
                write!(
                    formatter,
                    "accepted ordinary evidence replay failed: {error}"
                )
            }
            Self::MissingReview(package) => write!(
                formatter,
                "accepted ordinary evidence is missing compiler review for {package:?}"
            ),
            Self::ReviewAssociationMismatch(package) => write!(
                formatter,
                "accepted ordinary evidence review does not match reconstructed package {package:?}"
            ),
            Self::AllocationFailed => {
                formatter.write_str("accepted ordinary evidence allocation failed")
            }
        }
    }
}

impl std::error::Error for AcceptedOrdinaryEvidenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceCustody { error, .. } => Some(error),
            Self::SourceSelectionCustody { error, .. } => Some(error),
            Self::RootPolicy(error) => Some(error),
            Self::MissingReview(_)
            | Self::ReviewAssociationMismatch(_)
            | Self::AllocationFailed => None,
        }
    }
}
