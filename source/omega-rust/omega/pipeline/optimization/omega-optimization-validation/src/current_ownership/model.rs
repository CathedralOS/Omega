use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LiveClaim {
    pub(super) input: Option<PlaceId>,
    pub(super) path: Vec<StructuralPathSegment>,
    pub(super) multiplicity: Option<StructuralMultiplicity>,
}

/// Executable ownership reconstructed from current operations and signatures.
/// Immutable source snapshots and cached `OwnershipEvent` rows are not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CurrentOwnership {
    pub(super) claims: BTreeMap<ClaimId, LiveClaim>,
    pub(super) owned_places: BTreeMap<PlaceId, StructuralMultiplicity>,
    pub(super) partial_custody_paths: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>>,
}

pub(super) fn reconstruct_entry_ownership(function: &PsiOptimizationFunction) -> CurrentOwnership {
    let mut claims = BTreeMap::<ClaimId, LiveClaim>::new();
    for claim in &function.entry_claim_declarations {
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input)
            .expect("structural signature validation precedes current ownership replay");
        claims.insert(
            claim.claim,
            LiveClaim {
                input: Some(claim.input),
                path: claim.path.clone(),
                multiplicity: Some(if claim.path.is_empty() {
                    parameter.multiplicity
                } else {
                    StructuralMultiplicity::Linear
                }),
            },
        );
    }
    for claim in &function.content_entry_claims {
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input.root);
        claims.entry(claim.claim).or_insert(LiveClaim {
            input: parameter.map(|_| claim.input.root),
            path: Vec::new(),
            multiplicity: parameter.map(|parameter| parameter.multiplicity),
        });
    }
    CurrentOwnership {
        claims,
        owned_places: function
            .structural_parameters
            .iter()
            .filter_map(|parameter| {
                (parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                    .then_some((parameter.place, parameter.multiplicity))
            })
            .collect(),
        partial_custody_paths: BTreeMap::new(),
    }
}
