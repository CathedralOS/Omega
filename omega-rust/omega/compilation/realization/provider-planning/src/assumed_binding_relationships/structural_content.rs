//! Portable content of the relationship's produced structural correspondence.
//!
//! Preparation checks carriers, passing modes, lifetime partitions, multiplicity,
//! operational ceilings and native geometry independently of semantic assumptions.
//! Retain that decision against the two original application/public-contract
//! commitments and the already evaluated calling applications. A later consent
//! join must not rerun compatibility or replace it with equal byte sizes.
//! Parameter/result positions are portable coordinates; source symbols and local
//! type handles remain queries into the producing owner, never encoded identity.
//! These components supply neither domain issuance nor receiving acceptance.

use super::declarations::DeclaredRequirement;
use super::{
    AssumedBindingApplicationContent, AssumedBindingCallingCompatibility,
    AssumedBindingSubjectCorrespondence,
};
use crate::digest_encoder::DomainDigestEncoder;
use checked_trees::CheckedTrees;
use language_semantics::{BindingRelevance, Multiplicity};

/// Construction belongs to preparation after all independent compatibility
/// checks. Missing original application content remains unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingStructuralContent {
    subject_mapping: [u8; 32],
    compatibility: [u8; 32],
}

impl AssumedBindingStructuralContent {
    pub const fn subject_mapping(self) -> [u8; 32] {
        self.subject_mapping
    }

    pub const fn compatibility(self) -> [u8; 32] {
        self.compatibility
    }
}

pub(super) fn capture(
    program: &CheckedTrees,
    required: &DeclaredRequirement<'_>,
    supplied: &DeclaredRequirement<'_>,
    subjects: &[AssumedBindingSubjectCorrespondence],
    applications: Option<AssumedBindingApplicationContent>,
    public_contracts: ([u8; 32], [u8; 32]),
    calling: AssumedBindingCallingCompatibility,
) -> Option<AssumedBindingStructuralContent> {
    let applications = applications?;
    let mut mapping = DomainDigestEncoder::new(b"omega.assumed-binding-subject-mapping.v1\0");
    mapping.raw(&applications.required());
    mapping.raw(&applications.supplied());
    mapping.raw(&public_contracts.0);
    mapping.raw(&public_contracts.1);
    // Original applications preserve captured regions and invocation families;
    // public contracts preserve the positional partition throughout the types.
    // Compatibility has already established this one-to-one formal mapping.
    mapping.len(required.lifetimes.len());
    for ordinal in 0..required.lifetimes.len() {
        mapping.len(ordinal);
        mapping.len(ordinal);
    }
    mapping.len(subjects.len());
    for subject in subjects {
        match subject {
            AssumedBindingSubjectCorrespondence::Parameter { ordinal, .. } => {
                mapping.byte(1);
                mapping.u32(*ordinal);
                mapping.u32(*ordinal);
            }
            AssumedBindingSubjectCorrespondence::Result => mapping.byte(2),
        }
    }
    let subject_mapping = mapping.finish();

    let mut compatibility =
        DomainDigestEncoder::new(b"omega.assumed-binding-structural-compatibility.v1\0");
    compatibility.raw(&subject_mapping);
    compatibility.raw(&calling.required().as_bytes());
    compatibility.raw(&calling.supplied().as_bytes());
    // Published operational ceilings are in the original public contracts;
    // evaluated geometry/layout/lifecycle applications are in the calling pair.
    // Retain the source-checking custody decisions that were also compared.
    for declaration in [required, supplied] {
        compatibility.len(declaration.parameters.len());
        for parameter in declaration.parameters {
            compatibility.byte(u8::from(parameter.is_const));
            compatibility.byte(u8::from(parameter.is_mutable));
            compatibility.byte(match parameter.relevance {
                BindingRelevance::Relevant => 1,
                BindingRelevance::Erased => 2,
            });
            let custody = program
                .facts
                .reference_custody
                .recorded_custody(parameter.type_reference)?;
            encode_multiplicity(&mut compatibility, custody.binding_multiplicity);
            encode_multiplicity(&mut compatibility, custody.result_multiplicity);
        }
        compatibility.byte(u8::from(declaration.result.is_valid()));
        if declaration.result.is_valid() {
            encode_multiplicity(
                &mut compatibility,
                program
                    .facts
                    .reference_custody
                    .recorded_custody(declaration.result)?
                    .result_multiplicity,
            );
        }
    }
    Some(AssumedBindingStructuralContent {
        subject_mapping,
        compatibility: compatibility.finish(),
    })
}

fn encode_multiplicity(encoder: &mut DomainDigestEncoder, multiplicity: Multiplicity) {
    encoder.byte(match multiplicity {
        Multiplicity::Unrestricted => 1,
        Multiplicity::Affine => 2,
        Multiplicity::Linear => 3,
    });
}
