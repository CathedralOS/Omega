//! Independent receiving checks for case metadata; never calls construction.
use super::*;

pub(super) fn validate(
    actual: &LegalizedScalarTerminator,
    node: &optimization_unit::OptimizationNode,
    function: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    let invalid = Error::NonCanonicalLegalizedPlan;
    let (
        LegalizedScalarTerminator::StructuralCase {
            source: actual_source,
            layout,
            cases,
            effect,
            ownership,
        },
        AbstractOperation::StructuralCase {
            source,
            cases: expected,
        },
    ) = (actual, &node.operation)
    else {
        return Err(invalid);
    };
    let produced = scalar_graph_input::structural_case::source_owner(function, *source)?;
    if *actual_source != produced
        || *layout
            != scalar_graph_input::aggregate_results::sum_type_layout(
                produced.structural_type(),
                plan,
            )?
        || *effect != node.effect
        || *ownership != node.ownership
        || cases.len() != expected.len()
        || cases.len() != node.successors.len()
    {
        return Err(invalid);
    }
    let declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == produced.structural_type())
        .ok_or(invalid.clone())?;
    let terminal_psi::StructuralTypeShape::Sum { cases: declared } = &declaration.shape else {
        return Err(invalid);
    };
    if cases.len() != declared.len() || cases.len() != layout.cases.len() {
        return Err(invalid);
    }
    for (case_position, (((actual, source), edge), declared)) in cases
        .iter()
        .zip(expected)
        .zip(&node.successors)
        .zip(declared)
        .enumerate()
    {
        let destination = function
            .blocks
            .iter()
            .find(|block| block.id == source.target)
            .ok_or(invalid.clone())?;
        if actual.edge != source.psi_edge
            || actual.target != source.target
            || actual.case != source.case
            || actual.case != declared.id
            || Some(actual.case_tag) != i32::try_from(case_position).ok()
            || actual.trivial_affine_discards != source.trivial_affine_discards
            || actual.fuel != edge.fuel
            || actual.payloads.len() != source.payloads.len()
            || actual.payloads.len() != destination.parameters.len()
        {
            return Err(invalid);
        }
        let fields = declared
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if fields.len() != layout.cases[case_position].fields.len() {
            return Err(invalid);
        }
        for ((actual, source), parameter) in actual
            .payloads
            .iter()
            .zip(&source.payloads)
            .zip(&destination.parameters)
        {
            let field_position = fields
                .iter()
                .position(|field| field.id == source.field)
                .ok_or(invalid.clone())?;
            if actual.field != source.field
                || actual.field_byte_offset
                    != u32::from(layout.cases[case_position].fields[field_position].byte_offset)
                || actual.parameter.value != source.parameter
                || actual.parameter.value != parameter.value
                || actual.parameter.scalar_type != source.scalar_type
                || actual.parameter.scalar_type != parameter.scalar_type
                || actual.parameter.definition_site != parameter.site
            {
                return Err(invalid);
            }
        }
    }
    Ok(())
}
