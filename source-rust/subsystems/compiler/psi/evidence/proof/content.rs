use crate::StructuralPlaceDeclaration;
use semantic_vocabulary::{
    ClaimId, ContentAlgebra, ContentConservation, ContentProjectionIdentity,
    ContentStructuralPlace, OperationId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaimContentProjection {
    pub projection: ContentProjectionIdentity,
    pub algebra: ContentAlgebra,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentEntryClaim {
    pub claim: ClaimId,
    pub input: ContentStructuralPlace,
    pub projections: Vec<ClaimContentProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentIdentityReshuffle {
    pub claim: ClaimId,
    pub input: ContentStructuralPlace,
    pub output: ContentStructuralPlace,
    pub projections: Vec<ClaimContentProjection>,
}

impl ContentIdentityReshuffle {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentPlaceSubstitution {
    pub source: ContentStructuralPlace,
    pub target: ContentStructuralPlace,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentPartitionComposition {
    pub producer_operation: OperationId,
    pub source_report_fingerprint: u64,
    pub source_structural_places: Vec<StructuralPlaceDeclaration>,
    pub source: ContentConservation,
    pub input_claims: Vec<ClaimId>,
    pub substitutions: Vec<ContentPlaceSubstitution>,
    pub derived: ContentConservation,
}

impl ContentPartitionComposition {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentClaimProjection {
    pub claim: ClaimId,
    pub projection: ContentProjectionIdentity,
    pub subject: ContentStructuralPlace,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentTransformationDerivation {
    pub conservation: ContentConservation,
    pub lineage_roots: Vec<ClaimId>,
    pub ancestry: Vec<ContentLineageStep>,
    pub inputs: Vec<ContentClaimProjection>,
    pub outputs: Vec<ContentClaimProjection>,
    pub custody: ContentTransformationCustody,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentTransformationCustody {
    Derived,
    Boundary {
        requirements: Vec<ContentBoundaryBodyRequirement>,
        addresses: Vec<ContentBackingAddressTransport>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentBoundaryBodyRequirement {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub guarantee: crate::ContentConservationGuarantee,
    pub substitutions: Vec<ContentBodyProjectionSubstitution>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentBodyProjectionSubstitution {
    pub source: semantic_vocabulary::ContentTerm,
    pub target: semantic_vocabulary::ContentTerm,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentBackingAddressTransport {
    pub input: ContentClaimAddress,
    pub output: ContentClaimAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentClaimAddress {
    pub claim: ClaimId,
    pub subject: ContentStructuralPlace,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentLineageStep {
    pub producer_operation: OperationId,
    pub inputs: Vec<ClaimId>,
    pub outputs: Vec<ClaimId>,
}

impl ContentTransformationDerivation {
    pub fn map_places(
        &mut self,
        map: &mut impl FnMut(semantic_vocabulary::PlaceId) -> semantic_vocabulary::PlaceId,
    ) {
        fn term(
            value: &semantic_vocabulary::ContentTerm,
            map: &mut impl FnMut(semantic_vocabulary::PlaceId) -> semantic_vocabulary::PlaceId,
        ) -> semantic_vocabulary::ContentTerm {
            match value {
                semantic_vocabulary::ContentTerm::Projection {
                    projection,
                    subject,
                } => {
                    let mut subject = subject.clone();
                    subject.root = map(subject.root);
                    semantic_vocabulary::ContentTerm::Projection {
                        projection: *projection,
                        subject,
                    }
                }
                semantic_vocabulary::ContentTerm::Separate(children) => {
                    let mut children = children
                        .iter()
                        .map(|child| term(child, map))
                        .collect::<Vec<_>>();
                    children.sort();
                    semantic_vocabulary::ContentTerm::Separate(children)
                }
            }
        }
        self.conservation = ContentConservation::new(
            self.conservation.algebra().clone(),
            term(self.conservation.left(), map),
            term(self.conservation.right(), map),
        );
        for binding in self.inputs.iter_mut().chain(&mut self.outputs) {
            binding.subject.root = map(binding.subject.root);
        }
        if let ContentTransformationCustody::Boundary {
            requirements,
            addresses,
        } = &mut self.custody
        {
            for requirement in requirements.iter_mut() {
                for substitution in &mut requirement.substitutions {
                    substitution.target = term(&substitution.target, map);
                }
                requirement.substitutions.sort();
            }
            requirements.sort();
            for address in addresses.iter_mut() {
                address.input.subject.root = map(address.input.subject.root);
                address.output.subject.root = map(address.output.subject.root);
            }
            addresses.sort();
        }
        self.inputs.sort();
        self.outputs.sort();
    }
}
