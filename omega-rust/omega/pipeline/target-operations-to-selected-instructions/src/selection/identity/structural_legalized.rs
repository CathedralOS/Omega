//! Optimizer module role: identity leaf. Reconstructs the structural legalized fingerprint embedded in selected-plan identity.

use super::*;

pub(super) fn identity(
    plan: &SelectedInstructionPlan,
    schema: StructuralLegalizedIdentitySchema,
) -> LegalizedOperationPlanIdentity {
    let legalized = LegalizedOperationPlan {
        psi: plan.psi,
        optimization_unit: optimization_core::OptimizationUnitIdentity::from_canonical_bytes(
            b"omega.selected-structural-legalized-fingerprint.v1",
        ),
        fuel_schedule: plan.fuel_schedule,
        target: plan.target,
        entry: plan.entry,
        functions: Vec::new(),
        scalar_functions: Vec::new(),
        structural_unit_functions: plan
            .structural_unit_functions
            .iter()
            .map(|function| SourceStructuralUnitFunction {
                machine: function.machine,
                attachment: function.attachment,
                provenance: function.provenance.clone(),
                recipe: if !function.boundary_settlements.is_empty() {
                    legalized_operations::StructuralUnitLegalizationRecipe::ClaimCompletionSettlementsThenReturnUnitV1
                } else {
                    match function.call.as_ref().map(|call| &call.source) {
                        Some(legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit) => {
                            legalized_operations::StructuralUnitLegalizationRecipe::AuthoredCallThenReturnUnitV1
                        }
                        Some(legalized_operations::LegalizedCallUnitSource::InstalledProvider { .. }) => {
                            legalized_operations::StructuralUnitLegalizationRecipe::InstalledProviderCallThenReturnUnitV1
                        }
                        None => legalized_operations::StructuralUnitLegalizationRecipe::ReturnUnitV1,
                    }
                },
                structural_types: function.structural_types.clone(),
                call_plan: function.abi.call_plan.clone(),
                parameters: function
                    .abi
                    .parameters
                    .iter()
                    .map(|parameter| LegalizedCallUnitParameter {
                        semantic: parameter.semantic.clone(),
                        target: parameter.target.clone(),
                    })
                    .collect(),
                structural_places: function.structural_places.clone(),
                entry_claims: function.entry_claims.clone(),
                published_service_ceiling: function.published_service_ceiling.clone(),
                entry_block: function.source_entry_block,
                boundary_settlements: function.boundary_settlements.clone(),
                call: function.call.as_ref().map(|call| LegalizedCallUnit {
                    source: call.source.clone(),
                    operation: call.operation,
                    callee: call.callee,
                    arguments: call
                        .arguments
                        .iter()
                        .map(|argument| LegalizedCallUnitArgument {
                            semantic: argument.semantic.clone(),
                            target: argument.target.clone(),
                        })
                        .collect(),
                    claim_transfers: call.claim_transfers.clone(),
                    requirement_obligations: call.requirement_obligations.clone(),
                    crash_continuations: call.crash_continuations.clone(),
                    fuel: call.provenance.fuel.clone(),
                    effect: call.effect,
                    ownership: call.ownership.clone(),
                }),
                return_edge: function.terminator.psi_return_edge,
                return_fuel: function.terminator.instruction.provenance.fuel.clone(),
                return_effect: function.terminator.effect,
                return_ownership: function.terminator.ownership.clone(),
            })
            .collect(),
        projected_structural_call_returns: Vec::new(),
    };
    match schema {
        StructuralLegalizedIdentitySchema::V9 => {
            legalized_operations::legalized_operation_plan_identity_v9_legacy(&legalized)
        }
        StructuralLegalizedIdentitySchema::V12 => {
            legalized_operations::legalized_operation_plan_identity_v12_legacy(&legalized)
        }
        StructuralLegalizedIdentitySchema::V13 => {
            legalized_operations::legalized_operation_plan_identity_v13_legacy(&legalized)
        }
        StructuralLegalizedIdentitySchema::V14 => {
            legalized_operations::legalized_operation_plan_identity_v14_legacy(&legalized)
        }
    }
}
