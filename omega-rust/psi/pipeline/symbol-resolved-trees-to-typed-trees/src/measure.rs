use crate::expression::lower_expression_handle;
use crate::lowerer::Lowerer;
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

    let return_type = measure
        .return_type
        .as_ref()
        .map(|type_reference| lower_type_reference_into_table(lowerer, type_reference))
        .transpose()?
        .unwrap_or_else(typed_trees::types::TypeReferenceHandle::invalid);

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
            .push_measure_path_member(&mut typed_measure, crate::name::lower_name(member));
    }

    let mut lowered_components = Vec::new();
    for expression in lowerer
        .source_trees
        .tables
        .bodies
        .expressions
        .expression_handles(measure.body)
    {
        lowered_components.push(lower_expression_handle(lowerer, *expression)?);
    }
    typed_measure.body = lowerer
        .typed_trees
        .expression_table
        .insert_expression_handles(lowered_components);

    Ok(typed_measure)
}

fn lower_measure_parameter(
    lowerer: &mut Lowerer,
    parameter: &symbol_resolved_trees::signature::StateParameter,
) -> Result<typed_trees::signature::StateParameter, Diagnostic> {
    let type_reference = lower_type_reference_into_table(lowerer, &parameter.type_reference)?;
    Ok(typed_trees::signature::StateParameter {
        symbol: parameter.symbol,
        name: crate::name::lower_name(&parameter.name),
        type_reference,
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    })
}
