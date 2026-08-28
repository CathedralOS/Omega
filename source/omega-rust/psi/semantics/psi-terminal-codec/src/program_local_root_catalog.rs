//! Owned, non-authoritative program-local root producer schemas projected
//! from one successfully verified terminal-Psi artifact.
//!
//! This catalog deliberately stops before installation. It carries no slot,
//! occurrence, cardinality, artifact-instance, lifecycle, lineage, or grant
//! state, and therefore cannot introduce authority. Consumers use its exact
//! terminal identity and resolved semantic names as the portable side of a
//! later Omega-owned installation join.

use psi_terminal::{ProgramLocalRootIntroductionSchema, TerminalPsiIdentity};
use psi_terminal_verifier::VerifiedTerminalModule;

use crate::{CodecError, terminal_psi_identity};

/// One exact verified producer schema with module-local references resolved to
/// their stable source-free semantic identities.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VerifiedProgramLocalRootProducerSchema {
    boundary_requirement_identity: String,
    qualification_identity: String,
    carrier_identity: String,
    schema: ProgramLocalRootIntroductionSchema,
}

impl VerifiedProgramLocalRootProducerSchema {
    pub fn boundary_requirement_identity(&self) -> &str {
        &self.boundary_requirement_identity
    }

    pub fn qualification_identity(&self) -> &str {
        &self.qualification_identity
    }

    pub fn carrier_identity(&self) -> &str {
        &self.carrier_identity
    }

    pub const fn schema(&self) -> &ProgramLocalRootIntroductionSchema {
        &self.schema
    }
}

/// Canonical owned projection of every program-local producer schema in one
/// exact verified terminal-Psi artifact.
///
/// The terminal-Psi identity is the catalog identity: this is a deterministic
/// projection of the verified semantic artifact, not a separately authored or
/// independently replaceable manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProgramLocalRootProducerCatalog {
    terminal_psi: TerminalPsiIdentity,
    terminal_entry: psi_core::MachineId,
    schemas: Vec<VerifiedProgramLocalRootProducerSchema>,
}

impl VerifiedProgramLocalRootProducerCatalog {
    /// Reconstruct and own the portable producer catalog from verifier-owned
    /// evidence. There is intentionally no constructor from `TerminalModule`.
    pub fn from_verified(
        verified: &VerifiedTerminalModule<'_>,
    ) -> Result<Self, ProgramLocalRootProducerCatalogError> {
        let module = verified.module();
        let terminal_psi = terminal_psi_identity(module)
            .map_err(ProgramLocalRootProducerCatalogError::TerminalIdentity)?;
        let mut schemas = Vec::new();

        for boundary in &module.boundary_machines {
            for schema in &boundary.program_local_root_introductions {
                let qualification = module
                    .structural_domains
                    .iter()
                    .find(|domain| domain.id == schema.qualification)
                    .ok_or(ProgramLocalRootProducerCatalogError::MissingQualification {
                        requirement_identity: boundary.identity.clone(),
                        schema_identity: schema.identity,
                    })?;
                let carrier = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == schema.carrier)
                    .ok_or(ProgramLocalRootProducerCatalogError::MissingCarrier {
                        requirement_identity: boundary.identity.clone(),
                        schema_identity: schema.identity,
                    })?;

                // Replay the most important cross-table invariant instead of
                // assuming that the verified wrapper can never be widened by a
                // future vocabulary revision.
                if qualification.carrier != schema.carrier {
                    return Err(
                        ProgramLocalRootProducerCatalogError::QualificationCarrierMismatch {
                            requirement_identity: boundary.identity.clone(),
                            schema_identity: schema.identity,
                        },
                    );
                }
                let Some(owner_projection) = qualification.content_projection.as_ref() else {
                    return Err(
                        ProgramLocalRootProducerCatalogError::MissingOwnerContentProjection {
                            requirement_identity: boundary.identity.clone(),
                            schema_identity: schema.identity,
                        },
                    );
                };
                if schema.projection != owner_projection.identity
                    || schema.algebra != owner_projection.algebra
                    || schema.capacity != owner_projection.expression
                {
                    return Err(
                        ProgramLocalRootProducerCatalogError::OwnerContentProjectionMismatch {
                            requirement_identity: boundary.identity.clone(),
                            schema_identity: schema.identity,
                        },
                    );
                }

                schemas.push(VerifiedProgramLocalRootProducerSchema {
                    boundary_requirement_identity: boundary.identity.clone(),
                    qualification_identity: qualification.identity.clone(),
                    carrier_identity: carrier.identity.clone(),
                    schema: schema.clone(),
                });
            }
        }

        // Canonicalize independently of declaration traversal. All semantic
        // fields participate through the derived total order.
        schemas.sort();
        if let Some(duplicate) = schemas
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| &pair[0])
        {
            return Err(ProgramLocalRootProducerCatalogError::DuplicateSchema {
                requirement_identity: duplicate.boundary_requirement_identity.clone(),
                schema_identity: duplicate.schema.identity,
            });
        }

        Ok(Self {
            terminal_psi,
            terminal_entry: module.entry,
            schemas,
        })
    }

    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    /// Exact verified terminal entry whose emitted function offset must later
    /// be rebound to the installed entry. This is retained beside, not folded
    /// into, the artifact identity so installation can replay both facts.
    pub const fn terminal_entry(&self) -> psi_core::MachineId {
        self.terminal_entry
    }

    pub fn schemas(&self) -> &[VerifiedProgramLocalRootProducerSchema] {
        &self.schemas
    }
}

#[derive(Debug)]
pub enum ProgramLocalRootProducerCatalogError {
    TerminalIdentity(CodecError),
    MissingQualification {
        requirement_identity: String,
        schema_identity: u64,
    },
    MissingCarrier {
        requirement_identity: String,
        schema_identity: u64,
    },
    QualificationCarrierMismatch {
        requirement_identity: String,
        schema_identity: u64,
    },
    MissingOwnerContentProjection {
        requirement_identity: String,
        schema_identity: u64,
    },
    OwnerContentProjectionMismatch {
        requirement_identity: String,
        schema_identity: u64,
    },
    DuplicateSchema {
        requirement_identity: String,
        schema_identity: u64,
    },
}

impl std::fmt::Display for ProgramLocalRootProducerCatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TerminalIdentity(error) => {
                write!(
                    formatter,
                    "verified terminal-Psi identity replay failed: {error}"
                )
            }
            Self::MissingQualification {
                requirement_identity,
                schema_identity,
            } => write!(
                formatter,
                "verified program-local root schema {schema_identity:#018x} for `{requirement_identity}` has no resolved qualification"
            ),
            Self::MissingCarrier {
                requirement_identity,
                schema_identity,
            } => write!(
                formatter,
                "verified program-local root schema {schema_identity:#018x} for `{requirement_identity}` has no resolved carrier"
            ),
            Self::QualificationCarrierMismatch {
                requirement_identity,
                schema_identity,
            } => write!(
                formatter,
                "verified program-local root schema {schema_identity:#018x} for `{requirement_identity}` disagrees with its qualification carrier"
            ),
            Self::MissingOwnerContentProjection {
                requirement_identity,
                schema_identity,
            } => write!(
                formatter,
                "verified program-local root schema {schema_identity:#018x} for `{requirement_identity}` has no owner content projection"
            ),
            Self::OwnerContentProjectionMismatch {
                requirement_identity,
                schema_identity,
            } => write!(
                formatter,
                "verified program-local root schema {schema_identity:#018x} for `{requirement_identity}` disagrees with its owner content projection"
            ),
            Self::DuplicateSchema {
                requirement_identity,
                schema_identity,
            } => write!(
                formatter,
                "verified program-local root schema {schema_identity:#018x} for `{requirement_identity}` is duplicated"
            ),
        }
    }
}

impl std::error::Error for ProgramLocalRootProducerCatalogError {}
