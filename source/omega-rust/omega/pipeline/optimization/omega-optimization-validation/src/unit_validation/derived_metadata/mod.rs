//! Derived operation-metadata coordination.
//!
//! The entrance owns complete place/claim admission. Dominance, scalar value
//! metadata, provenance, successor edges, ownership, and terminator
//! classification descend into independently named evidence leaves.

use super::*;

mod control_flow;
mod edges;
mod ownership;
mod places;
mod provenance;
mod values;

pub(crate) use control_flow::{dominators, is_terminator};
pub(crate) use edges::{expected_edges, successors_match_operation};
pub(crate) use ownership::expected_ownership;
pub(crate) use places::{function_has_claim, reconstruct_declared_places};
pub(crate) use provenance::{expected_provenance, provenance_matches_operation};
pub(crate) use values::{expected_definitions, expected_uses};

pub(crate) fn validate_places_and_claims(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let known_places = reconstruct_declared_places(function)?;
    for parameter in &function.structural_parameters {
        if !function.declared_places.contains(&parameter.place) {
            return Err(OptimizationUnitValidationError::UnknownPlace {
                machine: function.machine,
                place: parameter.place,
            });
        }
    }
    for block in &function.blocks {
        for node in &block.nodes {
            for event in &node.ownership {
                let claims: &[ClaimId] = match event {
                    omega_optimization_unit::OwnershipEvent::ClaimTransfer(claims)
                    | omega_optimization_unit::OwnershipEvent::ClaimCompletion(claims)
                    | omega_optimization_unit::OwnershipEvent::StructuralReturn(claims)
                    | omega_optimization_unit::OwnershipEvent::CrashFrontier(claims) => claims,
                    omega_optimization_unit::OwnershipEvent::Cleanup(_) => continue,
                };
                for claim in claims {
                    if !function_has_claim(function, *claim) {
                        return Err(OptimizationUnitValidationError::UnknownClaim {
                            machine: function.machine,
                            claim: *claim,
                        });
                    }
                }
            }
        }
    }
    if known_places != function.declared_places {
        let place = known_places
            .symmetric_difference(&function.declared_places)
            .next()
            .copied()
            .expect("different place sets have a difference");
        return Err(OptimizationUnitValidationError::UnknownPlace {
            machine: function.machine,
            place,
        });
    }
    Ok(())
}
