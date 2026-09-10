//! Array temporaries belong to the exact actual at this call's formal position.
//! Replay the recursive shape even with no scalar leaves; a typed empty payload
//! cannot substitute another same-typed occurrence or conceal a stale carrier.

use checked_trees::expression::ExpressionHandle;
use checked_trees::signature::StateParameter;
use checked_trees::{
    CheckedScalarComputationHandle, CheckedScalarComputationStructuralArgument, CheckedTrees,
};
use symbols::SymbolHandle;

use crate::{LoweringError, unsupported};

pub(super) fn rejoin(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    parameter: &StateParameter,
    expression: ExpressionHandle,
    argument: &CheckedScalarComputationStructuralArgument,
) -> Result<Vec<(ExpressionHandle, CheckedScalarComputationHandle)>, LoweringError> {
    let CheckedScalarComputationStructuralArgument::Array {
        expression: retained_expression,
        type_reference,
        elements,
    } = argument
    else {
        return unsupported("computed array argument has no retained construction");
    };
    if parameter.is_mutable
        || *retained_expression != expression
        || *type_reference != parameter.type_reference
        || !validation::is_closed_primitive_array_type(checked, *type_reference)
    {
        return unsupported("computed array differs from its exact authored actual or type");
    }
    let source = validation::scalar_array_elements(checked, machine, expression, *type_reference)
        .ok_or(LoweringError::Unsupported(
        "computed array has no exact recursive source shape",
    ))?;
    // Projection folding may select closed leaves, but cannot erase a changed
    // operator selection at the authored array expression.
    for projection in source.projections {
        if let Some(selected) = checked.facts.operators.expression_use(projection)
            && (selected.spelling != language_core::OperatorSpelling::Index
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
        {
            return unsupported("computed array indexing selection changed");
        }
    }
    let plans = &checked.facts.values.scalar_computations;
    let retained = plans
        .operands
        .span(*elements)
        .ok_or(LoweringError::Unsupported(
            "computed array has a stale scalar-element span",
        ))?;
    if retained.len() != source.elements.len() {
        return unsupported("computed array omitted or duplicated scalar leaves");
    }
    source
        .elements
        .into_iter()
        .zip(retained.iter().copied())
        .map(|((expression, primitive), computation)| {
            if !plans.nodes.is_valid(computation)
                || plans.nodes.get(computation).authored_root != expression
                || plans.nodes.get(computation).primitive_type != primitive
            {
                return unsupported(
                    "computed array leaf differs from its authored source or carrier",
                );
            }
            Ok((expression, computation))
        })
        .collect()
}
