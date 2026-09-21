use crate::OptimizationUnitValidationError;
use crate::unit_validation::references::LiveReference;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::ClaimId;
use semantic_vocabulary::PlaceId;
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeDeclaration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LiveClaim {
    pub(super) input: Option<PlaceId>,
    pub(super) path: Vec<StructuralPathSegment>,
    pub(super) multiplicity: Option<StructuralMultiplicity>,
}

/// Executable ownership reconstructed from current operations and signatures.
/// Immutable source snapshots and cached `OwnershipEvent` rows are not read.
/// `live_references` replays each outstanding loan's carrier location, root
/// origin, and parent; joins compare it exactly like the owned roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CurrentOwnership {
    pub(super) claims: BTreeMap<ClaimId, LiveClaim>,
    pub(super) owned_places: BTreeMap<PlaceId, StructuralMultiplicity>,
    pub(super) partial_custody_paths: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>>,
    pub(super) live_references: Vec<LiveReference>,
}

pub(super) fn reconstruct_entry_ownership(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<CurrentOwnership, OptimizationUnitValidationError> {
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
    Ok(CurrentOwnership {
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
        live_references: super::references::entry_references(function, structural_types)?,
    })
}
