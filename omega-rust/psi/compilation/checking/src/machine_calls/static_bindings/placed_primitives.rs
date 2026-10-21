//! Admit the source occurrence, not merely an executable specialization. An
//! unused static-machine operand and an uninstantiated generic body still owe
//! accessor identity, permission and exposure. Run at the ordinary immutable
//! static-binding boundary after inference reaches its fixed point.

use diagnostics::Diagnostic;
use typed_trees::static_applications::{
    StaticMachineApplication, StaticMachineBinding, StaticMachineBindingValue,
};
use typed_trees::typed_trees::PlacedAccessPrimitive;

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    application: &StaticMachineApplication,
    bindings: &[StaticMachineBinding],
    selected_targets: impl Iterator<Item = symbols::SymbolHandle>,
) -> Result<(), Diagnostic> {
    let Some(operation) = PlacedAccessPrimitive::from_declaration(
        program.typed_program(),
        application.binding_declaration,
    )?
    else {
        return Ok(());
    };
    let machine = program
        .machine(application.binding_declaration)
        .ok_or_else(|| Diagnostic::error("placed primitive application lost its declaration"))?;
    let parameters = program.machine_type_parameters(machine);
    let selected_type = |parameter| {
        bindings.iter().find_map(|binding| match &binding.value {
            StaticMachineBindingValue::Type(reference) if binding.parameter == parameter => {
                Some(*reference)
            }
            _ => None,
        })
    };
    let (Some(accessor), Some(value)) = (
        selected_type(parameters[0].symbol),
        selected_type(parameters[1].symbol),
    ) else {
        return Err(Diagnostic::error(format!(
            "placed {} application requires an exact accessor/value tuple; generic callers must declare an explicit operation requirement",
            operation.operation(),
        )));
    };
    let target = operation.select_target(program.typed_program(), accessor, value)?;
    for selected in selected_targets {
        if selected != target.state_symbol {
            return Err(Diagnostic::error(
                "placed primitive application does not select its exact plan-issued field operation",
            ));
        }
    }
    let Some((view, field)) = program.placed_view_plans.iter().find_map(|view| {
        view.fields
            .iter()
            .find(|field| {
                field
                    .accessor_targets
                    .iter()
                    .any(|candidate| candidate.state_symbol == target.state_symbol)
            })
            .map(|field| (view, field))
    }) else {
        return Err(Diagnostic::error(
            "placed primitive selection lost its field plan",
        ));
    };
    if matches!(
        &field.access,
        access_plans::FieldAccess::Stable {
            exposure: access_plans::AccessExposure::BindingPrivate,
            ..
        } | access_plans::FieldAccess::StableRegion {
            exposure: access_plans::AccessExposure::BindingPrivate,
            ..
        } | access_plans::FieldAccess::External {
            exposure: access_plans::AccessExposure::BindingPrivate,
            ..
        }
    ) && !program
        .symbols
        .same_symbol_source_package(application.lexical_owner, view.policy_symbol)
    {
        return Err(Diagnostic::error(format!(
            "binding-private placed accessor `{}` belongs to placement policy `{}`'s package",
            field.field_name, view.policy_name,
        )));
    }
    Ok(())
}
