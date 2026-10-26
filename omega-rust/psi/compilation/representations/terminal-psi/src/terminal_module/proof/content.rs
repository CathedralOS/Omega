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
    /// Strictly ordered by `(projection, algebra)` in canonical modules.
    pub projections: Vec<ClaimContentProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentIdentityReshuffle {
    /// The same claim may have rows for mutually exclusive result cases.
    /// All such rows retain one input and the same complete projection set.
    pub claim: ClaimId,
    pub input: ContentStructuralPlace,
    pub output: ContentStructuralPlace,
    /// Strictly ordered by `(projection, algebra)` in canonical modules.
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
    /// Exact call operation whose successful normal completion establishes the
    /// source theorem used by this composition. Merely carrying this row is
    /// never semantic authority.
    pub producer_operation: OperationId,
    /// Non-authoritative compact coordinate for reporting and caches. The
    /// exact source equation below is independently reconstructed and replayed.
    pub source_report_fingerprint: u64,
    /// Structural-place declarations for the source callable's theorem. They
    /// live in a namespace local to this witness rather than the wrapper.
    pub source_structural_places: Vec<StructuralPlaceDeclaration>,
    pub source: ContentConservation,
    /// Dense machine-local claims whose exact entry projections participate.
    pub input_claims: Vec<ClaimId>,
    /// Strictly ordered by `source`; every source projection has exactly one
    /// substitution and all rows are used by replay.
    pub substitutions: Vec<ContentPlaceSubstitution>,
    pub derived: ContentConservation,
}

impl ContentPartitionComposition {}

/// One live claim's exact content projection at a transformation occurrence.
/// A claim identity is independent of its current storage and qualification:
/// equal geometry alone cannot establish this association.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentClaimProjection {
    pub claim: ClaimId,
    pub projection: ContentProjectionIdentity,
    pub subject: ContentStructuralPlace,
}

/// One equation of an atomic content transformation. All rows of the operation
/// are checked jointly, including every independent qualification of each
/// consumed parent. Outputs keep their existing nominal claim identities;
/// a same-owner equation derives membership, not a fresh root or replacement
/// claim. A conditional boundary-body relation transports custody only.
/// Derivations and their input/output rosters are unordered sets encoded in
/// strict derived `Ord` order. Remapping restores that order before publication.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentTransformationDerivation {
    pub conservation: ContentConservation,
    /// The existing account from which every consumed claim descends.
    pub lineage_anchor: ClaimId,
    /// Ordered completed replacements of the anchor's frontier. Each producer
    /// must independently authenticate these exact input/result occurrences;
    /// this roster alone is not authority, nor is numeric conservation.
    pub ancestry: Vec<ContentLineageStep>,
    pub inputs: Vec<ContentClaimProjection>,
    pub outputs: Vec<ContentClaimProjection>,
    pub custody: ContentTransformationCustody,
}

/// Qualification derivation and conditional boundary-body custody transport
/// have different authority. A boundary body retains its raw result's actual
/// memberships; only a later admitted invocation may establish its result D.
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
    /// Exact declaration-local source law, not its compact report digest.
    pub guarantee: crate::ContentConservationGuarantee,
    /// Every source projection occurs exactly once. A current descendant
    /// frontier may replace one original input with an explicit separate term.
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
    /// Rebind exact occurrence roots alongside the operation's operand/result
    /// namespace. Qualification, projection and claim identities are unchanged.
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
