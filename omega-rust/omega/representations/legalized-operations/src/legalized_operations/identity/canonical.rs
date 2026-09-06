//! Canonical legalized-plan roster encoding shared by current and legacy identities.

use super::condition::encode_condition;

use super::projected_structural_call_return::encode_projected_structural_call_return;
use super::scalar_call_unit::encode_scalar_call_unit_function;
use super::{
    plan::encode_structural_unit_function,
    scalar::{
        encode_bindings, encode_definition_site, encode_immediate, encode_leaf, encode_scalar_type,
    },
    shared::*,
};

pub(super) fn identity(
    plan: &LegalizedOperationPlan,
    domain: &[u8],
    retain_call_contract: bool,
    retain_scalar_call_unit_roster: bool,
    retain_scalar_body: bool,
    retain_ordered_calls: bool,
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
        let function = match function {
            LegalizedFunction::Conditional(function) => {
                if retain_scalar_body {
                    bytes.push(0);
                }
                function
            }
            LegalizedFunction::Leaf(function) => {
                // A legacy identity cannot claim leaf custody. The leaf tag
                // and complete payload are retained even under legacy domains.
                bytes.push(1);
                super::scalar_leaf::encode(&mut bytes, function);
                continue;
            }
            LegalizedFunction::SharedReturnConditional(function) => {
                bytes.push(2);
                encode_shared_return(&mut bytes, function);
                continue;
            }
        };
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
            LegalizationRecipe::ReturnU64ExactIntegerSequenceConditionalV1 => 17,
            LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1 => 9,
            LegalizationRecipe::ReturnU64IntegerLessThanParametersConditionalV1 => 10,
            LegalizationRecipe::ReturnU64IntegerLessOrEqualParametersConditionalV1 => 11,
            LegalizationRecipe::ReturnU64IntegerNotEqualParametersConditionalV1 => 12,
            LegalizationRecipe::ReturnU64I64LessThanParametersConditionalV1 => 13,
            LegalizationRecipe::ReturnU64EqualZeroParameterConditionalV1 => 14,
            LegalizationRecipe::ReturnU64NotEqualZeroParameterConditionalV1 => 15,
            LegalizationRecipe::ReturnU64I64LessOrEqualParametersConditionalV1 => 16,
        });
        encode_condition(&mut bytes, function.condition_source, &function.condition);
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
    if retain_scalar_call_unit_roster {
        encode_len(&mut bytes, plan.scalar_call_unit_functions.len());
        for function in &plan.scalar_call_unit_functions {
            encode_scalar_call_unit_function(&mut bytes, function, retain_ordered_calls);
        }
    }
    LegalizedOperationPlanIdentity::from_canonical_bytes(&bytes)
}

fn encode_shared_return(bytes: &mut Vec<u8>, function: &LegalizedSharedReturnConditionalFunction) {
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    encode_option_id(bytes, function.attachment.map(|value| value.get()));
    encode_ids(
        bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        function.provenance.edges.iter().map(|value| value.get()),
    );
    super::scalar_leaf::encode_abi(bytes, &function.abi);
    encode_condition(bytes, function.condition_source, &function.condition);
    bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
    for arm in [&function.when_true, &function.when_false] {
        bytes.extend_from_slice(&arm.block.get().to_le_bytes());
        encode_len(bytes, arm.parameters.len());
        for parameter in &arm.parameters {
            bytes.extend_from_slice(&parameter.value.get().to_le_bytes());
            encode_scalar_type(bytes, parameter.scalar_type);
            encode_definition_site(bytes, parameter.site);
        }
        bytes.extend_from_slice(&arm.branch_edge.get().to_le_bytes());
        encode_bindings(bytes, &arm.branch_bindings);
        encode_fuel(bytes, &arm.branch_fuel);
        encode_immediate(bytes, &arm.constant);
        bytes.extend_from_slice(&arm.transfer_edge.get().to_le_bytes());
        encode_bindings(bytes, &[arm.transfer_binding]);
        encode_fuel(bytes, &arm.transfer_fuel);
    }
    bytes.extend_from_slice(&function.return_block.get().to_le_bytes());
    bytes.extend_from_slice(&function.return_parameter.value.get().to_le_bytes());
    encode_scalar_type(bytes, function.return_parameter.scalar_type);
    encode_definition_site(bytes, function.return_parameter.site);
    bytes.extend_from_slice(&function.return_edge.get().to_le_bytes());
    encode_fuel(bytes, &function.return_fuel);
}
