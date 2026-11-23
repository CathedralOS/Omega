/* Measure definitions and their optional parameter. */

use crate::resolution::lowering::expression::expression_table;
use crate::resolution::lowering::expression::lower_expression_into_table;
use crate::resolution::lowering::lowerer::Lowerer;
use crate::resolution::lowering::name::lower_operator_path_name;
use crate::resolution::lowering::state::lower_state_parameter;
use crate::resolution::lowering::type_reference::lower_type_reference_handle;
use crate::source_resolved::measure::MeasureDefinition;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_measure_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    measure: &syntax::item::MeasureDefinition,
) -> Result<MeasureDefinition, Diagnostic> {
    if !measure.lexicographic && (!measure.parameter.is_valid() || !measure.return_type.is_valid())
    {
        let mut diagnostic = Diagnostic::error(
            "a computed measure requires exactly one subject parameter and a result type",
        );
        if let Some(name) = syntax_trees
            .items
            .identifier_path_members(measure.name)
            .last()
        {
            diagnostic = diagnostic.with_source_span(name.source_span());
        }
        return Err(diagnostic);
    }
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
