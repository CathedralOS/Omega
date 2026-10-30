//! Measure definitions and their optional parameter.

use crate::lowering::expression::expression_table;
use crate::lowering::expression::lower_expression_into_table;
use crate::lowering::name::lower_operator_path_name;
use crate::lowering::state::lower_state_parameter;
use crate::lowering::type_reference::lower_type_reference_handle;
use crate::resolution::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_resolved_trees::measure::MeasureDefinition;
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_measure_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    measure: &syntax::item::MeasureDefinition,
) -> Result<MeasureDefinition, Diagnostic> {
    let name = lower_operator_path_name(lowerer, syntax_trees, measure.name);

    let parameter = measure
        .parameter
        .is_valid()
        .then(|| lower_state_parameter(lowerer, syntax_trees, measure.parameter))
        .transpose()?;

    let return_type = measure
        .return_type
        .is_valid()
        .then(|| lower_type_reference_handle(lowerer, syntax_trees, measure.return_type))
        .transpose()?;

    let mut lowered_components = Vec::new();
    for expression in syntax_trees.expressions.expression_handles(measure.body) {
        let lowered = lower_expression_into_table(lowerer, syntax_trees, *expression)?;
        lowered_components.push(lowered);
    }
    let body = expression_table(lowerer).insert_expression_handles(lowered_components);

    Ok(MeasureDefinition {
        symbol: SymbolHandle::invalid(),
        name,
        parameter,
        return_type,
        lexicographic: measure.lexicographic,
        body,
    })
}
