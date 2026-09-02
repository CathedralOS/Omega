//! Canonical legalized-plan roster encoding shared by current and legacy identities.

use super::projected_structural_call_return::encode_projected_structural_call_return;
use super::{
    plan::encode_structural_unit_function,
    scalar::{encode_bindings, encode_definition_site, encode_leaf, encode_register},
    shared::*,
};

pub(super) fn identity(
    plan: &LegalizedOperationPlan,
    domain: &[u8],
    retain_call_contract: bool,
) -> LegalizedOperationPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&plan.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.entry.get().to_le_bytes());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_option_id(
            &mut bytes,
            function.attachment.map(|attachment| attachment.get()),
        );
        encode_ids(
            &mut bytes,
            function
                .provenance
                .operations
                .iter()
                .map(|operation| operation.get()),
        );
        encode_ids(
            &mut bytes,
            function.provenance.edges.iter().map(|edge| edge.get()),
        );
        bytes.push(match function.recipe {
            LegalizationRecipe::ReturnU64ImmediateConditionalV1 => 0,
            LegalizationRecipe::ReturnU64EntryParameterConditionalV1 => 1,
            LegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1 => 2,
            LegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1 => 3,
            LegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1 => 4,
            LegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1 => 5,
            LegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1 => 6,
            LegalizationRecipe::ReturnU64ActiveResidentExactAddBridgeChainConditionalV1 => 7,
            LegalizationRecipe::ReturnU64ActiveResidentExactAddOriginalVictimChainConditionalV1 => {
                8
            }
            LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1 => 9,
            LegalizationRecipe::ReturnU64IntegerLessThanParametersConditionalV1 => 10,
            LegalizationRecipe::ReturnU64IntegerLessOrEqualParametersConditionalV1 => 11,
        });
        bytes.extend_from_slice(&function.condition_source.get().to_le_bytes());
        match &function.condition {
            LegalizedCondition::DirectParameter {
                parameter_index,
                register,
                definition_site,
            } => {
                // Preserve the exact pre-V13 byte layout for every existing
                // direct-condition recipe.
                bytes.extend_from_slice(&(*parameter_index as u64).to_le_bytes());
                encode_register(&mut bytes, *register);
                encode_definition_site(&mut bytes, *definition_site);
            }
            LegalizedCondition::IntegerEqualParametersV1 {
                operation,
                result_definition_site,
                fuel,
                left,
                right,
            } => {
                bytes.push(0xff);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
                encode_definition_site(&mut bytes, *result_definition_site);
                encode_fuel(&mut bytes, fuel);
                for parameter in [left, right] {
                    bytes.extend_from_slice(&parameter.source_value.get().to_le_bytes());
                    bytes.extend_from_slice(&(parameter.parameter_index as u64).to_le_bytes());
                    encode_register(&mut bytes, parameter.register);
                    encode_definition_site(&mut bytes, parameter.definition_site);
                }
            }
            LegalizedCondition::IntegerLessThanParametersV1 {
                operation,
                result_definition_site,
                fuel,
                left,
                right,
            } => {
                bytes.push(0xfe);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
                encode_definition_site(&mut bytes, *result_definition_site);
                encode_fuel(&mut bytes, fuel);
                for parameter in [left, right] {
                    bytes.extend_from_slice(&parameter.source_value.get().to_le_bytes());
                    bytes.extend_from_slice(&(parameter.parameter_index as u64).to_le_bytes());
                    encode_register(&mut bytes, parameter.register);
                    encode_definition_site(&mut bytes, parameter.definition_site);
                }
            }
            LegalizedCondition::IntegerLessOrEqualParametersV1 {
                operation,
                result_definition_site,
                fuel,
                left,
                right,
            } => {
                bytes.push(0xfd);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
                encode_definition_site(&mut bytes, *result_definition_site);
                encode_fuel(&mut bytes, fuel);
                for parameter in [left, right] {
                    bytes.extend_from_slice(&parameter.source_value.get().to_le_bytes());
                    bytes.extend_from_slice(&(parameter.parameter_index as u64).to_le_bytes());
                    encode_register(&mut bytes, parameter.register);
                    encode_definition_site(&mut bytes, parameter.definition_site);
                }
            }
        }
        bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
        bytes.extend_from_slice(&function.true_block.get().to_le_bytes());
        bytes.extend_from_slice(&function.false_block.get().to_le_bytes());
        bytes.extend_from_slice(&function.branch_true_edge.get().to_le_bytes());
        bytes.extend_from_slice(&function.branch_false_edge.get().to_le_bytes());
        encode_fuel(&mut bytes, &function.branch_true_fuel);
        encode_fuel(&mut bytes, &function.branch_false_fuel);
        encode_bindings(&mut bytes, &function.branch_true_bindings);
        encode_bindings(&mut bytes, &function.branch_false_bindings);
        encode_leaf(&mut bytes, &function.when_true);
        encode_leaf(&mut bytes, &function.when_false);
    }
    encode_len(&mut bytes, plan.unit_functions.len());
    for function in &plan.unit_functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_option_id(
            &mut bytes,
            function.attachment.map(|attachment| attachment.get()),
        );
        encode_ids(
            &mut bytes,
            function
                .provenance
                .operations
                .iter()
                .map(|operation| operation.get()),
        );
        encode_ids(
            &mut bytes,
            function.provenance.edges.iter().map(|edge| edge.get()),
        );
        bytes.push(match function.recipe {
            UnitLegalizationRecipe::ReturnUnitV1 => 0,
        });
        bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
        bytes.extend_from_slice(&function.return_edge.get().to_le_bytes());
        encode_fuel(&mut bytes, &function.return_fuel);
    }
    encode_len(&mut bytes, plan.structural_unit_functions.len());
    for function in &plan.structural_unit_functions {
        encode_structural_unit_function(&mut bytes, function, retain_call_contract);
    }
    encode_len(&mut bytes, plan.projected_structural_call_returns.len());
    for closure in &plan.projected_structural_call_returns {
        encode_projected_structural_call_return(&mut bytes, closure);
    }
    LegalizedOperationPlanIdentity::from_canonical_bytes(&bytes)
}
