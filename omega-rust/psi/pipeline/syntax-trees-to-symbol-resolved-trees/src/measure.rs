use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use crate::state::lower_state_parameter;
use crate::type_reference::lower_type_reference_handle;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees::measure::MeasureDefinition;
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_measure_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    measure: &syntax::item::MeasureDefinition,
) -> Result<MeasureDefinition, Diagnostic> {
    let name = lower_measure_name(lowerer, syntax_trees, measure.name);

    let parameter = if measure.parameter.is_valid() {
        Some(lower_state_parameter(
            lowerer,
            syntax_trees,
            measure.parameter,
        )?)
    } else {
        None
    };

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
    let body = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert_expression_handles(lowered_components);

    Ok(MeasureDefinition {
        symbol: SymbolHandle::invalid(),
        name,
        parameter,
        return_type,
        lexicographic: measure.lexicographic,
        body,
    })
}

fn lower_measure_name(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<symbol_resolved_trees::name::DiagnosticName> {
    let mut span = HandleSpan::empty();

    for member in syntax_trees.items.identifier_path_members(name) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .operator_path_members
            .append_to_span(&mut span, crate::name::lower_name(member));
    }

    span
}
