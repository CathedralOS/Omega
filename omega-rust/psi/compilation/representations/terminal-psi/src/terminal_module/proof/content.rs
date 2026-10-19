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
/// the equation derives membership, not a fresh root or a replacement claim.
/// Derivations and their input/output rosters are unordered sets encoded in
/// strict derived `Ord` order. Remapping restores that order before publication.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentTransformationDerivation {
    pub conservation: ContentConservation,
    /// The retained relation authenticates this current input identity only.
    /// Equal geometry does not establish shared backing with another claim;
    /// multi-parent reunion needs additional evidence not represented here.
    pub lineage_anchor: ClaimId,
    pub inputs: Vec<ContentClaimProjection>,
    pub outputs: Vec<ContentClaimProjection>,
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
        self.inputs.sort();
        self.outputs.sort();
    }
}
