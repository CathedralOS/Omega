//! Source construction of exact case destinations and structural-home layout.
use super::*;

pub(super) fn project(
    node: &optimization_unit::OptimizationNode,
    function: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<LegalizedScalarTerminator, LegalizationError> {
    let invalid = Error::SourceCustodyMismatch;
    let AbstractOperation::StructuralCase { source, cases } = &node.operation else {
        return Err(invalid);
    };
    let (defining_operation, result) =
        scalar_graph_input::structural_case::source_result(function, *source)?;
    let layout = scalar_graph_input::read_byte::layout(result, plan)?;
    let declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .ok_or(invalid.clone())?;
    let terminal_psi::StructuralTypeShape::Sum { cases: declared } = &declaration.shape else {
        return Err(invalid);
    };
    if cases.len() != declared.len()
        || cases.len() != node.successors.len()
        || cases.len() != layout.cases.len()
    {
        return Err(invalid);
    }
    let mut successors = Vec::with_capacity(cases.len());
    for (case_position, ((case, edge), declared)) in
        cases.iter().zip(&node.successors).zip(declared).enumerate()
    {
        let destination = function
            .blocks
            .iter()
            .find(|block| block.id == case.target)
            .ok_or(invalid.clone())?;
        let fields = declared
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if case.payloads.len() != destination.parameters.len()
            || fields.len() != layout.cases[case_position].fields.len()
        {
            return Err(invalid);
        }
        let mut payloads = Vec::with_capacity(case.payloads.len());
        for (payload, parameter) in case.payloads.iter().zip(&destination.parameters) {
            let field_position = fields
                .iter()
                .position(|field| field.id == payload.field)
                .ok_or(invalid.clone())?;
            payloads.push(LegalizedStructuralCasePayload {
                field: payload.field,
                field_byte_offset: u32::from(
                    layout.cases[case_position].fields[field_position].byte_offset,
                ),
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
            case_tag: i32::try_from(case_position).map_err(|_| invalid.clone())?,
            payloads,
            trivial_affine_discards: case.trivial_affine_discards.clone(),
            fuel: edge.fuel.clone(),
        });
    }
    Ok(LegalizedScalarTerminator::StructuralCase {
        defining_operation,
        result: result.clone(),
        layout,
        cases: successors,
        effect: node.effect,
        ownership: node.ownership.clone(),
    })
}
