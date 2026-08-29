use crate::evidence::PackageReviewExternalBinding;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(super) fn validate_selected_boundary_operator_checked_adapter(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    operator: &psi_typed_trees::operator::OperatorDefinition,
) -> Result<(), Vec<Diagnostic>> {
    let plans = compilation.selected_provider_plans().plans();
    let provenance = compilation.selected_provider_provenance();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected boundary-operator provider plans are not aligned with retained declaration provenance",
        )]);
    }
    let slot = psi_typed_trees::operator::boundary_operator_requirement_identity(
        &compilation.typed,
        operator,
    );
    let matches = plans
        .iter()
        .zip(provenance)
        .filter(|(plan, retained)| {
            plan.schema.trait_name == slot
                && retained.provider.schema
                    == omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryOperator(
                        operator.symbol,
                    )
        })
        .collect::<Vec<_>>();
    let [(plan, retained)] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed checked adapter `{}` realizes boundary operator `{slot}`, but package review found {} exact selected provider plans for that operator",
            machine.name,
            matches.len(),
        ))]);
    };
    let [method] = plan.schema.methods.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one schema method",
            plan.name,
        ))]);
    };
    let [row] = plan.rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        ))]);
    };
    let [requirement_symbol] = retained.provider.row_requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must retain exactly one requirement declaration",
            plan.name,
        ))]);
    };
    let [realization_symbol] = retained.provider.row_realizations.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must retain exactly one realization declaration",
            plan.name,
        ))]);
    };
    let expected_machine_identity = compilation
        .normalized_machine_overload_identity(machine)
        .map(|identity| identity.identity())
        .unwrap_or_default();
    let expected_package = compilation
        .typed
        .symbols
        .symbol_package_identity(machine.symbol);
    if retained.plan != **plan
        || *requirement_symbol != operator.symbol
        || *realization_symbol != machine.symbol
        || method.requirement_owner != slot
        || method.requirement_identity != slot
        || row.requirement_identity != slot
        || !matches!(
            &row.binding,
            omega_effects::provider_plan::ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } if machine_identity == &expected_machine_identity
                && *machine_package_identity == expected_package
        )
    {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` does not join exact operator `{slot}` to checked adapter `{}`",
            plan.name, machine.name,
        ))]);
    }
    Ok(())
}

pub(super) fn validate_selected_boundary_operator_external_supply(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    binding: &PackageReviewExternalBinding,
) -> Result<(), Vec<Diagnostic>> {
    let plans = compilation.selected_provider_plans().plans();
    let provenance = compilation.selected_provider_provenance();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected boundary-operator provider plans are not aligned with retained declaration provenance",
        )]);
    }
    let slot = psi_typed_trees::operator::boundary_operator_requirement_identity(
        &compilation.typed,
        operator,
    );
    let matches = plans
        .iter()
        .zip(provenance)
        .filter(|(plan, retained)| {
            plan.schema.trait_name == slot
                && retained.provider.schema
                    == omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryOperator(
                        operator.symbol,
                    )
                && retained.provider.row_realizations.contains(&machine.symbol)
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Ok(());
    }
    let [(plan, retained)] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed external leaf `{}` realizes boundary operator `{slot}`, but package review found {} selected provider plans for that exact candidate",
            machine.name,
            matches.len(),
        ))]);
    };
    let [method] = plan.schema.methods.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one schema method",
            plan.name,
        ))]);
    };
    let [row] = plan.rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        ))]);
    };
    let [requirement_symbol] = retained.provider.row_requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must retain exactly one requirement declaration",
            plan.name,
        ))]);
    };
    let [realization_symbol] = retained.provider.row_realizations.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must retain exactly one realization declaration",
            plan.name,
        ))]);
    };
    let expected_machine_identity = compilation
        .normalized_machine_overload_identity(machine)
        .map(|identity| identity.identity())
        .unwrap_or_default();
    let expected_package = compilation
        .typed
        .symbols
        .symbol_package_identity(machine.symbol);
    let expected_table = machine
        .attached_data
        .as_ref()
        .map(|name| name.as_str())
        .unwrap_or_default();
    let binding_matches = match (binding, &row.binding) {
        (
            PackageReviewExternalBinding::Import { library, symbol },
            omega_effects::provider_plan::ProviderBinding::StringBackedImportBootstrap {
                library: selected_library,
                symbol: selected_symbol,
            },
        ) => library == selected_library && symbol == selected_symbol,
        (
            PackageReviewExternalBinding::Syscall { number },
            omega_effects::provider_plan::ProviderBinding::Syscall {
                number: selected_number,
            },
        ) => number == selected_number,
        (
            PackageReviewExternalBinding::CompilerIntrinsic,
            omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                machine: selected_machine,
            },
        ) => selected_machine == &expected_machine_identity,
        (
            PackageReviewExternalBinding::VtableSlot { index },
            omega_effects::provider_plan::ProviderBinding::VtableSlot {
                index: selected_index,
            },
        ) => index == selected_index,
        (
            PackageReviewExternalBinding::VtableField { field },
            omega_effects::provider_plan::ProviderBinding::VtableField {
                table,
                field: selected_field,
            },
        ) => table == expected_table && field == selected_field,
        (
            PackageReviewExternalBinding::TableFunction { field },
            omega_effects::provider_plan::ProviderBinding::TableFunction {
                table,
                field: selected_field,
            },
        ) => table == expected_table && field == selected_field,
        _ => false,
    };
    if retained.plan != **plan
        || *requirement_symbol != operator.symbol
        || *realization_symbol != machine.symbol
        || plan.origin_package_identity != expected_package
        || method.requirement_owner != slot
        || method.requirement_identity != slot
        || row.requirement_identity != slot
        || !binding_matches
    {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` does not join exact operator `{slot}` to external leaf `{}` and its binding",
            plan.name, machine.name,
        ))]);
    }
    Ok(())
}
