//! Installed replay for the bounded Unit Boolean split with two descriptor leaves.
//!
//! The installation format retains the general Unit scalar ABI, forwarded-call
//! projections, and semantic attribution already owned by the image. This
//! validator rejoins those carriers without introducing a join-specific row.

use calling_conventions::ValueShape;
use machine_code::SemanticCodeSite;
use semantic_vocabulary::ScalarType;

use super::{InstallationError, InstallationRecord};

pub(super) fn validate_installed_unit_dynamic_descriptor_joins(
    record: &InstallationRecord,
) -> Result<(), InstallationError> {
    for function in &record.functions {
        let calls = record
            .forwarded_dynamic_descriptor_calls
            .iter()
            .filter(|call| call.machine == function.machine)
            .collect::<Vec<_>>();
        let attributions = record
            .semantic_code_attribution
            .iter()
            .filter(|attribution| attribution.machine == function.machine)
            .collect::<Vec<_>>();
        let ordinary_unit_call_count = record
            .internal_unit_calls
            .iter()
            .filter(|call| call.machine == function.machine)
            .count();
        let boolean_parameter = function.unit_scalar_abi.as_ref().is_some_and(|abi| {
            matches!(
                abi.parameters.as_slice(),
                [parameter] if parameter.scalar_type == ScalarType::Boolean
            )
        });
        let joined_shape_hint = calls.len() == 2 && attributions.len() == 5;
        // Boolean is an ordinary scalar type, not a dynamic-family tag. Let
        // the disjoint installed store/call validators own their exact rows.
        let primitive_store_shape = function.unit_write_only_primitive_stores.len() == 1
            && calls.is_empty()
            && attributions.len() == 2;
        let projected_store_shape = function.unit_structural_scalar_field_stores.len() == 1
            && calls.is_empty()
            && attributions.len() == 2;
        let ordinary_unit_call_shape =
            ordinary_unit_call_count == 1 && calls.is_empty() && attributions.len() == 2;
        if primitive_store_shape || projected_store_shape || ordinary_unit_call_shape {
            continue;
        }
        if !boolean_parameter && !joined_shape_hint {
            continue;
        }
        let invalid = || InstallationError::InvalidUnitDynamicDescriptorJoin(function.machine);
        let Some(abi) = function.unit_scalar_abi.as_ref() else {
            return Err(invalid());
        };
        let [parameter] = abi.parameters.as_slice() else {
            return Err(invalid());
        };
        let [first, second] = calls.as_slice() else {
            return Err(invalid());
        };
        let [
            condition,
            first_call,
            true_return,
            second_call,
            false_return,
        ] = attributions.as_slice()
        else {
            return Err(invalid());
        };
        let [structural_parameter] = function.unit_parameters.as_slice() else {
            return Err(invalid());
        };
        let [structural_home] = function.unit_parameter_homes.as_slice() else {
            return Err(invalid());
        };
        let edge_identity =
            |attribution: &crate::ObjectCodeAttribution| match attribution.attribution.site {
                SemanticCodeSite::Edge(edge) => Some(edge),
                SemanticCodeSite::Operation(_) => None,
            };
        let condition_edge = edge_identity(condition);
        let true_return_edge = edge_identity(true_return);
        let false_return_edge = edge_identity(false_return);
        if parameter.scalar_type != ScalarType::Boolean
            || parameter.placement.shape != ValueShape::integer(1, 1)
            || abi.call_plan.result.is_some()
            || abi.call_plan.parameters.as_slice()
                != [parameter.placement.clone(), structural_home.source.clone()]
            || structural_parameter.place != structural_home.place
            || structural_parameter.structural_type != structural_home.structural_type
            || structural_parameter.multiplicity != structural_home.multiplicity
            || structural_parameter.access != structural_home.access
            || structural_parameter.shape != structural_home.shape
            || function.unit_stack.is_none()
            || first.operation == second.operation
            || first.source == second.source
            || first.callee != second.callee
            || !matches!(
                (
                    first.semantic_result.map(|result| result.scalar_type),
                    second.semantic_result.map(|result| result.scalar_type),
                    first.result.is_some(),
                    second.result.is_some(),
                ),
                (
                    Some(ScalarType::Boolean),
                    Some(ScalarType::Boolean),
                    true,
                    true
                ) | (None, None, false, false)
            )
            || condition.attribution.operation_ordinal != 0
            || first_call.attribution.operation_ordinal != 1
            || true_return.attribution.operation_ordinal != 2
            || second_call.attribution.operation_ordinal != 3
            || false_return.attribution.operation_ordinal != 4
            || first_call.attribution.site != SemanticCodeSite::Operation(first.operation)
            || second_call.attribution.site != SemanticCodeSite::Operation(second.operation)
            || first_call.text_offset != first.text_offset
            || first_call.attribution.byte_count != first.byte_count
            || second_call.text_offset != second.text_offset
            || second_call.attribution.byte_count != second.byte_count
            || condition_edge.is_none()
            || true_return_edge.is_none()
            || false_return_edge.is_none()
            || condition_edge == true_return_edge
            || condition_edge == false_return_edge
            || true_return_edge == false_return_edge
            || !contiguous(condition, first_call)
            || !contiguous(first_call, true_return)
            || !contiguous(true_return, second_call)
            || !contiguous(second_call, false_return)
            || false_return
                .text_offset
                .checked_add(false_return.attribution.byte_count)
                != function.text_offset.checked_add(function.byte_count)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn contiguous(left: &crate::ObjectCodeAttribution, right: &crate::ObjectCodeAttribution) -> bool {
    left.text_offset.checked_add(left.attribution.byte_count) == Some(right.text_offset)
}
