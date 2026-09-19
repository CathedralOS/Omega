//! Binding scalar and structural call results to checked unit locals.

use crate::execution::terminal_unit::{
    CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralResultBindingPlan, ExpressionNode, Multiplicity, ShapeCollector,
    StatementNode, SymbolHandle, TypeReferenceHandle, TypedTrees, is_reference, is_unit,
    parameter_qualifications, type_graph_requires_nominal_drop,
};

pub(crate) fn checked_unit_scalar_result_local(
    program: &TypedTrees,
    statements: &[StatementNode],
) -> Option<CheckedUnitScalarResultBindingPlan> {
    let StatementNode::LocalData(local) = statements.first()? else {
        return None;
    };
    if local.is_mutable || !local.initial_value.is_valid() {
        return None;
    }
    let primitive_type = program.primitive_type_reference(local.type_reference)?;
    if !matches!(
        program.expression_table.expression(local.initial_value),
        ExpressionNode::Call(_)
    ) {
        return None;
    }
    Some(CheckedUnitScalarResultBindingPlan {
        statement_index: 0,
        binding_ordinal: 0,
        primitive_type,
    })
}

pub(crate) fn bind_scalar_call_result(
    facts: &CheckFacts,
    operation: CheckedUnitEffectOperationPlan,
    result: CheckedUnitScalarResultBindingPlan,
    allow_boundary: bool,
) -> Option<CheckedUnitEffectOperationPlan> {
    match operation {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            source_site,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            scalar_arguments,
            structural_arguments,
            completion_receipts,
        } if allow_boundary => Some(CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            source_site,
            result,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            scalar_arguments,
            structural_arguments,
            completion_receipts,
        }),
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            scalar_arguments,
            erased_scalar_arguments,
            structural_arguments,
            claim_transfers,
        } => Some(CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            result,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            target_contract_commitment: facts
                .contract_plans
                .for_machine(target_machine)?
                .commitment,
            service_reach,
            scalar_arguments,
            erased_scalar_arguments,
            structural_arguments,
            claim_transfers,
        }),
        _ => None,
    }
}

/// Keep the checked call's exact target, source coordinate, operands and
/// completion receipts while attaching its independently checked result local.
pub(crate) fn bind_structural_call_result(
    operation: CheckedUnitEffectOperationPlan,
    result: CheckedUnitStructuralResultBindingPlan,
) -> Option<CheckedUnitEffectOperationPlan> {
    Some(match operation {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            source_site,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            scalar_arguments,
            structural_arguments,
            completion_receipts,
        } => CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            source_site,
            discard_result_on_return: result.multiplicity == Multiplicity::Affine,
            result,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            scalar_arguments,
            structural_arguments,
            completion_receipts,
        },
        operation @ CheckedUnitEffectOperationPlan::StructuralCall { .. } => operation,
        _ => return None,
    })
}

pub(crate) fn checked_unit_structural_result_local(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    statements: &[StatementNode],
    binders: &[(SymbolHandle, String)],
) -> Option<(CheckedUnitStructuralResultBindingPlan, SymbolHandle)> {
    let StatementNode::LocalData(local) = statements.first()? else {
        return None;
    };
    if !local.initial_value.is_valid()
        || !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        )
    {
        return None;
    }
    Some((
        checked_structural_result_type(program, shapes, local.type_reference, binders)?,
        local.symbol,
    ))
}

/// Result shape is independent of whether the source binds or discards it.
/// Linear value classification does not establish custody: the ordinary call
/// producer must retain the independently reconstructed transfer/return record.
/// Nominal cleanup still requires its checked settlement plan.
/// Primitive arrays use their complete-shape classifier, including dimensions
/// with no leaves; the older owned-storage classifier excludes empty arrays.
pub(crate) fn checked_structural_result_type(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    result_type: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<CheckedUnitStructuralResultBindingPlan> {
    if validation::reference_result_custody::parts(program, result_type).is_some() {
        return Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_reference_type(result_type, binders)?,
            multiplicity: Multiplicity::Affine,
        });
    }
    let multiplicity = crate::checks::type_multiplicity(program, result_type);
    let qualifications = parameter_qualifications(program, shapes, result_type, binders)?;
    if is_unit(program, result_type)
        || program.primitive_type_reference(result_type).is_some()
        || is_reference(program, result_type)
        || type_graph_requires_nominal_drop(program, result_type)
        || (multiplicity != Multiplicity::Linear
            && (!(validation::has_plain_owned_contents_with_numeric_constraints(
                program,
                result_type,
            ) || validation::is_closed_primitive_array_type(program, result_type)
                || validation::reference_result_custody::is_reference_record(
                    program,
                    result_type,
                ))
                || !qualifications.is_empty()))
    {
        return None;
    }
    Some(CheckedUnitStructuralResultBindingPlan {
        statement_index: 0,
        binding_ordinal: 0,
        type_identity: shapes.add_type(result_type, binders, &[])?,
        multiplicity,
    })
}
