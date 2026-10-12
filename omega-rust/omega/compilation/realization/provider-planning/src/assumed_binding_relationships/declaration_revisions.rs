//! Seal-owned source revisions of the two original public requirements.
//!
//! A source unit or location alone cannot distinguish declarations. Preparation
//! retains the ordinary schema producer's canonical requirement coordinate and
//! declaration kind; sealing joins that coordinate, original provenance and
//! checked public contract to the already produced consumed source unit. Local
//! handles, physical roots and diagnostic names never enter portable bytes.
//! Application correspondence and complete dependency identity remain separate.

use package_compilation::CheckedSourceCustody;
use symbols::SymbolHandle;

use super::PreparedAssumedBindingRelationship;
use super::declarations::{DeclaredRequirement, PublicRequirement};
use crate::digest_encoder::DomainDigestEncoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssumedBindingDeclarationKind {
    TraitRequirement,
    TopLevelRequirement,
}

/// Preparation's original schema coordinate, before later declaration pruning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DeclarationCoordinate {
    kind: AssumedBindingDeclarationKind,
    canonical_requirement: String,
}

impl DeclarationCoordinate {
    pub(super) fn capture(requirement: &DeclaredRequirement<'_>) -> Self {
        Self {
            kind: match requirement.declaration {
                PublicRequirement::Trait { .. } => AssumedBindingDeclarationKind::TraitRequirement,
                PublicRequirement::Machine { .. } => {
                    AssumedBindingDeclarationKind::TopLevelRequirement
                }
            },
            canonical_requirement: requirement.requirement_identity().to_owned(),
        }
    }
}

/// One source-backed public declaration revision. Construction consumes only
/// original producers; this grants neither a runtime binding nor consent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingDeclarationRevision {
    kind: AssumedBindingDeclarationKind,
    source_revision: [u8; 32],
    public_contract: [u8; 32],
    digest: [u8; 32],
}

impl AssumedBindingDeclarationRevision {
    pub const fn kind(self) -> AssumedBindingDeclarationKind {
        self.kind
    }
    pub const fn source_revision(self) -> [u8; 32] {
        self.source_revision
    }
    pub const fn public_contract(self) -> [u8; 32] {
        self.public_contract
    }
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingDeclarationRevisions {
    pub required: AssumedBindingDeclarationRevision,
    pub supplied: AssumedBindingDeclarationRevision,
}

/// Retain revisions after the ordinary source seal has produced exact units.
/// Package inputs and original captured consuming roots supply distinct source
/// owners through the same seal. Missing original custody stays unavailable;
/// captured roots are never relabeled as packages or given receiving authority.
/// Receiving collection must require both revisions.
pub fn retain_assumed_binding_declaration_revisions(
    relationships: &mut [PreparedAssumedBindingRelationship],
    custody: &CheckedSourceCustody,
) {
    for relationship in relationships {
        relationship.declaration_revisions = capture(relationship, custody);
    }
}

fn capture(
    relationship: &PreparedAssumedBindingRelationship,
    custody: &CheckedSourceCustody,
) -> Option<AssumedBindingDeclarationRevisions> {
    if !custody.owns_sources(&relationship.declaration_owner().symbols) {
        return None;
    }
    Some(AssumedBindingDeclarationRevisions {
        required: revision(
            relationship,
            custody,
            relationship.required().symbol,
            &relationship.required_declaration_coordinate,
            relationship.required_public_contract(),
        )?,
        supplied: revision(
            relationship,
            custody,
            relationship.supplied().symbol,
            &relationship.supplied_declaration_coordinate,
            relationship.supplied_public_contract(),
        )?,
    })
}

fn revision(
    relationship: &PreparedAssumedBindingRelationship,
    custody: &CheckedSourceCustody,
    symbol: SymbolHandle,
    coordinate: &DeclarationCoordinate,
    public_contract: [u8; 32],
) -> Option<AssumedBindingDeclarationRevision> {
    if !symbol.is_valid()
        || coordinate.canonical_requirement.is_empty()
        || public_contract == [0; 32]
    {
        return None;
    }
    let symbols = &relationship.declaration_owner().symbols;
    let source = custody.declaration_source(symbols, symbol)?;
    let source_revision = source.unit().revision_identity();
    let mut encoder = DomainDigestEncoder::new(b"omega.assumed-binding-declaration-revision.v1\0");
    encoder.byte(match coordinate.kind {
        AssumedBindingDeclarationKind::TraitRequirement => 0,
        AssumedBindingDeclarationKind::TopLevelRequirement => 1,
    });
    encoder.raw(&source_revision);
    encoder.bytes(coordinate.canonical_requirement.as_bytes());
    encoder.u64(source.start());
    encoder.u64(source.end());
    encoder.raw(&public_contract);
    Some(AssumedBindingDeclarationRevision {
        kind: coordinate.kind,
        source_revision,
        public_contract,
        digest: encoder.finish(),
    })
}
