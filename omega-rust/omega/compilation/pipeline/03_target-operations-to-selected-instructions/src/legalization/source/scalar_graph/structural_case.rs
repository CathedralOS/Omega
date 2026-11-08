//! Source construction of exact case destinations and structural-home layout.
use super::{AbstractOperationPlan, Error, scalar_graph_input};
use crate::LegalizationError;
use abstract_operations::AbstractOperation;
use legalized_operations::{
    LegalizedScalarTerminator, LegalizedStructuralCasePayload, LegalizedStructuralCaseSuccessor,
    LegalizedValueDefinition,
};

pub(super) fn project(
    node: &optimization_unit::OptimizationNode,
    function: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
) -> Result<LegalizedScalarTerminator, LegalizationError> {
    let AbstractOperation::StructuralCase {
        source,
        path,
        cases,
    } = &node.operation
    else {
        return Err(Error::custody());
    };
    let (selected, byte_offset, root_shape, layout) =
        scalar_graph_input::structural_case::dispatch_projection(function, *source, path, plan)?;
    let source = scalar_graph_input::structural_case::case_source(function, *source)?;
    let declaration = plan.structural_type(selected).ok_or(Error::custody())?;
    let declared: &[terminal_psi::StructuralCaseDeclaration] = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Sum { cases }
        | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return Err(Error::custody()),
    };
    if cases.len() != declared.len()
        || cases.len() != node.successors.len()
        || cases.len() != layout.cases.len()
    {
        return Err(Error::custody());
    }
    let mut successors = Vec::with_capacity(cases.len());
    for (case_position, ((case, edge), declared)) in
        cases.iter().zip(&node.successors).zip(declared).enumerate()
    {
        let destination = function.block(case.target).ok_or(Error::custody())?;
        let fields = declared
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if case.payloads.len() != destination.parameters.len()
            || fields.len() != layout.cases[case_position].fields.len()
        {
            return Err(Error::custody());
        }
        let mut payloads = Vec::with_capacity(case.payloads.len());
        for (payload, parameter) in case.payloads.iter().zip(&destination.parameters) {
            let field_position = fields
                .iter()
                .position(|field| field.id == payload.field)
                .ok_or(Error::custody())?;
            payloads.push(LegalizedStructuralCasePayload {
                field: payload.field,
                field_byte_offset: byte_offset
                    .checked_add(layout.cases[case_position].fields[field_position].byte_offset)
                    .ok_or(Error::custody())?,
                parameter: LegalizedValueDefinition {
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                    definition_site: parameter.site,
                },
            });
        }
        successors.push(LegalizedStructuralCaseSuccessor {
            edge: case.psi_edge,
            target: case.target,
            case: case.case,
            case_tag: i32::try_from(case_position).map_err(|_| Error::custody())?,
            payloads,
            structural_bindings: super::terminator::successor(edge, function, plan, unit)?
                .structural_bindings,
            cleanup_actions: case.cleanup_actions.clone(),
            cleanup_calls: Vec::new(),
            fuel: edge.fuel.clone(),
        });
    }
    Ok(LegalizedScalarTerminator::StructuralCase {
        source,
        path: path.clone(),
        root_shape,
        byte_offset,
        layout,
        cases: successors,
        effect: node.effect,
        ownership: node.ownership.clone(),
    })
}
