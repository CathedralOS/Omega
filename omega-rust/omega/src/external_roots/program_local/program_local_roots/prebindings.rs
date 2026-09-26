//! Program-local root schema digests and installed prebindings.

use crate::executable_installation::{ArtifactId, InstalledCodeId};
use crate::external_roots::{
    ExternalRootId, InstalledExternalRoot, ProviderExecutionId, RootAdmissionId, RootSlotId,
    RootSlotOwnerId,
};
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentProjectionExpression, ContentProjectionScalar,
};
use sha2::Digest;
use sha2::Sha256;
use std::num::NonZeroU64;
use terminal_codec::VerifiedProgramLocalRootProducerSchema;
use terminal_psi::TerminalPsiIdentity;

/// Collision-resistant commitment to every exact, resolved producer-schema
/// field consumed by program-local root installation.
///
/// The digest is deliberately separate from terminal Psi's compact FNV schema
/// identity. It is the identity used for ledger uniqueness, cohort grouping,
/// and replay; the compact value remains available only as a compatibility
/// report coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramLocalRootSchemaDigest([u8; 32]);

impl ProgramLocalRootSchemaDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact, opaque address of one non-authoritative installed prebinding.
///
/// The tuple includes the strong exact-schema commitment. The compact schema
/// report identity is retained for compatibility diagnostics only and never
/// selects a ledger row, cohort group, or lifecycle family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramLocalRootPrebindingId {
    pub(crate) installed_code: InstalledCodeId,
    pub(crate) root: ExternalRootId,
    pub(crate) slot: RootSlotId,
    pub(crate) schema_digest: ProgramLocalRootSchemaDigest,
}

impl ProgramLocalRootPrebindingId {
    pub const fn installed_code(self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn root(self) -> ExternalRootId {
        self.root
    }

    pub const fn slot(self) -> RootSlotId {
        self.slot
    }

    pub const fn schema_digest(self) -> ProgramLocalRootSchemaDigest {
        self.schema_digest
    }
}

/// Non-authoritative prebinding of one portable producer schema to one exact
/// installed environment-to-program slot occurrence.
///
/// This record deliberately does not carry a lifecycle epoch and cannot mint
/// content. It closes the installation facts already available today so a
/// later lifecycle join can consume one typed occurrence instead of repeating
/// requirement, provider, artifact, and slot matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLocalRootInstalledPrebinding {
    pub(crate) identity: ProgramLocalRootPrebindingId,
    pub(crate) psi: TerminalPsiIdentity,
    pub(crate) installed_root_evidence:
        crate::external_roots::installed_root_ledger::InstalledRootEvidence,
    pub(crate) owner: RootSlotOwnerId,
    pub(crate) artifact: ArtifactId,
    pub(crate) admission: RootAdmissionId,
    pub(crate) provider_execution: ProviderExecutionId,
    pub(crate) requirement_identity: String,
    pub(crate) argument_index: u32,
    pub(crate) source_parameter_position: u32,
    pub(crate) qualification_identity: String,
    pub(crate) carrier_identity: String,
    pub(crate) projection: semantic_vocabulary::ContentProjectionIdentity,
    pub(crate) schema_compatibility_report_identity: u64,
    pub(crate) algebra: ContentAlgebra,
    pub(crate) per_occurrence_capacity: ContentProjectionExpression,
}

impl ProgramLocalRootInstalledPrebinding {
    pub const fn identity(&self) -> ProgramLocalRootPrebindingId {
        self.identity
    }

    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn owner(&self) -> RootSlotOwnerId {
        self.owner
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn admission(&self) -> RootAdmissionId {
        self.admission
    }

    pub const fn provider_execution(&self) -> ProviderExecutionId {
        self.provider_execution
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn argument_index(&self) -> u32 {
        self.argument_index
    }

    pub const fn source_parameter_position(&self) -> u32 {
        self.source_parameter_position
    }

    pub fn qualification_identity(&self) -> &str {
        &self.qualification_identity
    }

    pub fn carrier_identity(&self) -> &str {
        &self.carrier_identity
    }

    pub const fn projection(&self) -> semantic_vocabulary::ContentProjectionIdentity {
        self.projection
    }

    pub const fn schema_digest(&self) -> ProgramLocalRootSchemaDigest {
        self.identity.schema_digest
    }

    pub const fn schema_compatibility_report_identity(&self) -> u64 {
        self.schema_compatibility_report_identity
    }

    pub const fn algebra(&self) -> &ContentAlgebra {
        &self.algebra
    }

    pub const fn per_occurrence_capacity(&self) -> &ContentProjectionExpression {
        &self.per_occurrence_capacity
    }

    pub(crate) fn matches_root(&self, root: &InstalledExternalRoot<'_>) -> bool {
        self.installed_root_evidence == root.evidence
            && self.identity.installed_code == root.installed_code.identity()
            && self.identity.root == root.root
            && self.identity.slot == root.slot
            && self.owner == root.owner
            && self.artifact == root.installed_code.artifact()
            && self.admission == root.evidence.admission
            && self.provider_execution == root.evidence.provider_execution.identity
            && self.requirement_identity == root.evidence.root.candidate.requirement_identity
    }
}

/// Exact installed-slot count derived for one schema and one installed
/// artifact occurrence. It remains prebinding evidence, not an aggregate root:
/// lifecycle epoch and authority introduction are intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLocalRootInstalledPrebindingCount {
    pub psi: TerminalPsiIdentity,
    pub installed_code: InstalledCodeId,
    pub artifact: ArtifactId,
    pub requirement_identity: String,
    pub argument_index: u32,
    pub source_parameter_position: u32,
    pub qualification_identity: String,
    pub carrier_identity: String,
    pub schema_digest: ProgramLocalRootSchemaDigest,
    pub schema_compatibility_report_identity: u64,
    pub algebra: ContentAlgebra,
    pub per_occurrence_capacity: ContentProjectionExpression,
    pub installed_slot_count: NonZeroU64,
    pub prebinding_identities: Vec<ProgramLocalRootPrebindingId>,
}

pub(crate) type CountKey = (u16, [u8; 32], InstalledCodeId, ProgramLocalRootSchemaDigest);

pub(crate) type LifecycleFamilyKey = (InstalledCodeId, ProgramLocalRootSchemaDigest);

pub(crate) fn program_local_root_schema_digest(
    verified: &VerifiedProgramLocalRootProducerSchema,
) -> ProgramLocalRootSchemaDigest {
    program_local_root_schema_fields_digest(
        verified.boundary_requirement_identity(),
        verified.qualification_identity(),
        verified.carrier_identity(),
        verified.schema(),
    )
}

pub(crate) fn program_local_root_schema_fields_digest(
    boundary_requirement_identity: &str,
    qualification_identity: &str,
    carrier_identity: &str,
    schema: &terminal_psi::ProgramLocalRootIntroductionSchema,
) -> ProgramLocalRootSchemaDigest {
    fn field(digest: &mut Sha256, bytes: &[u8]) {
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }

    fn scalar(digest: &mut Sha256, value: &ContentProjectionScalar) {
        match value {
            ContentProjectionScalar::SubjectField(path)
            | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
                digest.update([
                    if matches!(value, ContentProjectionScalar::SubjectField(_)) {
                        1
                    } else {
                        2
                    },
                ]);
                digest.update((path.len() as u64).to_le_bytes());
                for segment in path {
                    field(digest, segment.as_bytes());
                }
            }
            ContentProjectionScalar::Natural(value) => {
                digest.update([3]);
                field(digest, value.as_bytes());
            }
            ContentProjectionScalar::Successor(inner) => {
                digest.update([4]);
                scalar(digest, inner);
            }
            ContentProjectionScalar::Add(left, right)
            | ContentProjectionScalar::Subtract(left, right)
            | ContentProjectionScalar::Multiply(left, right) => {
                digest.update([match value {
                    ContentProjectionScalar::Add(_, _) => 5,
                    ContentProjectionScalar::Subtract(_, _) => 6,
                    ContentProjectionScalar::Multiply(_, _) => 7,
                    _ => unreachable!(),
                }]);
                scalar(digest, left);
                scalar(digest, right);
            }
        }
    }

    let mut digest = Sha256::new();
    digest.update(b"omega.program-local-root-schema.sha256.v1\0");
    field(&mut digest, boundary_requirement_identity.as_bytes());
    field(&mut digest, qualification_identity.as_bytes());
    field(&mut digest, carrier_identity.as_bytes());
    digest.update(schema.argument_index.to_le_bytes());
    digest.update(schema.source_parameter_position.to_le_bytes());
    digest.update(schema.qualification.get().to_le_bytes());
    digest.update(schema.carrier.get().to_le_bytes());
    digest.update(schema.projection.domain.get().to_le_bytes());
    digest.update(
        schema
            .projection
            .projection_report_fingerprint
            .to_le_bytes(),
    );
    digest.update([match schema.algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    }]);
    field(&mut digest, schema.algebra.parameter.as_bytes());
    match &schema.capacity {
        ContentProjectionExpression::IntervalSet(members) => {
            digest.update([1]);
            digest.update((members.len() as u64).to_le_bytes());
            for (start, end) in members {
                scalar(&mut digest, start);
                scalar(&mut digest, end);
            }
        }
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            digest.update([2]);
            scalar(&mut digest, magnitude);
        }
    }
    ProgramLocalRootSchemaDigest(digest.finalize().into())
}
