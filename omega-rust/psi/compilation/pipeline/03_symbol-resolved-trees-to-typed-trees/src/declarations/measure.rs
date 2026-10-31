use crate::expressions::expression::lower_expression_handle;
use crate::lowerer::Lowerer;
use crate::signatures::parameters::state_parameter;
use crate::tables::expression_table_mut;
use crate::tables::resolved_expressions;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;

pub(crate) fn lower_measure_definition(
    lowerer: &mut Lowerer,
    measure: &symbol_resolved_trees::measure::MeasureDefinition,
) -> Result<typed_trees::measure::MeasureDefinition, Diagnostic> {
    let parameter = measure
        .parameter
        .as_ref()
        .map(|parameter| lower_measure_parameter(lowerer, parameter))
        .transpose()?;

    let return_type = crate::type_reference::lower_return_type(lowerer, &measure.return_type)?;

    let mut typed_measure = typed_trees::measure::MeasureDefinition {
        symbol: measure.symbol,
        name: Default::default(),
        parameter,
        return_type,
        lexicographic: measure.lexicographic,
        body: Default::default(),
    };

    for member in lowerer.source_trees.measure_path_members(measure.name) {
        lowerer
            .typed_trees
            .push_measure_path_member(&mut typed_measure, crate::lowerer::name::lower_name(member));
    }

    let mut lowered_components = Vec::new();
    for expression in resolved_expressions(lowerer.source_trees).expression_handles(measure.body) {
        lowered_components.push(lower_expression_handle(lowerer, *expression)?);
    }
    typed_measure.body = expression_table_mut(&mut lowerer.typed_trees)
        .insert_expression_handles(lowered_components);

    Ok(typed_measure)
}

fn lower_measure_parameter(
    lowerer: &mut Lowerer,
    parameter: &symbol_resolved_trees::signature::StateParameter,
) -> Result<typed_trees::signature::StateParameter, Diagnostic> {
    let type_reference = lower_type_reference_into_table(lowerer, &parameter.type_reference)?;
    Ok(state_parameter(parameter, type_reference))
}
