use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) fn lower_state_parameter(
    lowerer: &mut Lowerer,
    parameter: &resolved::signature::StateParameter,
) -> Result<typed::signature::StateParameter, Diagnostic> {
    let type_reference = lower_type_reference_into_table(lowerer, &parameter.type_reference)?;
    crate::type_reference::domain_constraints::normalize_domain_constraints_for_type(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
    )?;
    Ok(state_parameter(parameter, type_reference))
}

/// The typed shape of an authored state parameter once its written type is interned.
pub(crate) fn state_parameter(
    parameter: &resolved::signature::StateParameter,
    type_reference: typed::types::TypeReferenceHandle,
) -> typed::signature::StateParameter {
    typed::signature::StateParameter {
        symbol: parameter.symbol,
        name: crate::lowerer::name::lower_name(&parameter.name),
        type_reference,
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        relevance: parameter.relevance,
    }
}
