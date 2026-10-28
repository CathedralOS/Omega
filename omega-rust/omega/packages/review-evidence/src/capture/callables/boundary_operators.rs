use crate::capture::PackageReviewInput;
use diagnostics::Diagnostic;

/// The singleton members a boundary-operator plan must carry: exactly one
/// schema method, one realization row, and one requirement and one
/// realization declaration in the retained provenance.
fn boundary_operator_plan_singletons<'plan>(
    plan: &'plan effects::provider_plan::ProviderPlan,
    retained: &'plan provider_planning::SelectedProviderReviewProvenance,
) -> Result<
    (
        &'plan effects::provider_plan::ServiceMethod,
        &'plan effects::provider_plan::ProviderPlanRow,
        &'plan symbols::SymbolHandle,
        &'plan provider_planning::ProviderRowOrigin,
    ),
    Vec<Diagnostic>,
> {
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
    Ok((method, row, requirement_symbol, realization_symbol))
}

pub(super) fn validate_fixed_token_checked_adapter_dispatch_shape(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Result<(), Vec<Diagnostic>> {
    let Some(spelling) = operator.spelling else {
        return Ok(());
    };
    let operand_count = compilation.operator_parameters(operator).len();
    let supported = operand_count == 2
        && matches!(
            spelling,
            language_core::OperatorSpelling::Add
                | language_core::OperatorSpelling::Subtract
                | language_core::OperatorSpelling::Multiply
                | language_core::OperatorSpelling::Divide
                | language_core::OperatorSpelling::Modulo
                | language_core::OperatorSpelling::Equal
                | language_core::OperatorSpelling::NotEqual
                | language_core::OperatorSpelling::Less
                | language_core::OperatorSpelling::LessEqual
                | language_core::OperatorSpelling::Greater
                | language_core::OperatorSpelling::GreaterEqual
                | language_core::OperatorSpelling::Index
        );
    if !supported {
        return Err(vec![Diagnostic::error(format!(
            "reviewed checked adapter `{}` realizes fixed-token boundary operator `{}` with unsupported dispatch shape `{}` and {operand_count} normalized operands",
            machine.name,
            typed_trees::operator::boundary_operator_requirement_identity(
                compilation.checking_input,
                operator,
            ),
            spelling.symbol(),
        ))]);
    }
    Ok(())
}

pub(super) fn validate_selected_boundary_operator_checked_adapter(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Result<(), Vec<Diagnostic>> {
    let selected_execution = compilation.custody.execution();
    let plans = selected_execution.selected_provider_plan_facts.plans();
    let provenance = selected_execution.selected_provider_provenance.as_slice();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected boundary-operator provider plans are not aligned with retained declaration provenance",
        )]);
    }
    let slot = typed_trees::operator::boundary_operator_requirement_identity(
        compilation.checking_input,
        operator,
    );
    let matches = plans
        .iter()
        .zip(provenance)
        .filter(|(plan, retained)| {
            plan.schema.identity_text(plan.schema.trait_name) == slot
                && retained.provider.schema
                    == provider_planning::ProviderSchemaDeclaration::BoundaryOperator(
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
    let (method, row, requirement_symbol, realization_symbol) =
        boundary_operator_plan_singletons(plan, retained)?;
    let expected_machine_identity = compilation
        .machine_callable_identity_text(machine)
        .unwrap_or_default();
    let expected_package = compilation
        .typed
        .symbols
        .symbol_package_identity(machine.symbol);
    let selected_identity_matches = match &row.binding {
        effects::provider_plan::ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } if *machine_package_identity == expected_package => {
            plan.identity_text(*machine_identity) == expected_machine_identity
                || compilation
                    .typed
                    .machine_specializations
                    .iter()
                    .filter(|specialization| specialization.instance == machine.symbol)
                    .any(|specialization| {
                        typed_trees::machine_specialization_matches_template_identity(
                            &compilation.typed,
                            specialization,
                            plan.identity_text(*machine_identity),
                            *machine_package_identity,
                        )
                    })
        }
        _ => false,
    };
    if retained.plan != **plan
        || *requirement_symbol != operator.symbol
        || realization_symbol.machine() != Some(machine.symbol)
        || method.requirement_owner != slot
        || plan.schema.identity_text(method.requirement_identity) != slot
        || plan.identities.text(row.requirement_identity) != slot
        || !selected_identity_matches
    {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` does not join exact operator `{slot}` to checked adapter `{}`",
            plan.name, machine.name,
        ))]);
    }
    Ok(())
}
