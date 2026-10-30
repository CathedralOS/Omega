use super::super::api::operators::project_operator_coordinate;
use super::super::semantics::declarations::{nominal_identity, trait_requirement_identity};
use super::super::semantics::types::review_signature_type_identity_with_binders;
use super::boundary_operators::{
    validate_fixed_token_checked_adapter_dispatch_shape,
    validate_selected_boundary_operator_checked_adapter,
};
use super::boundary_requirements::project_checked_requirement_realization;
use crate::capture::PackageReviewInput;
use crate::record::{PackagePolicyCallableConformance, PackageReviewOperatorRealization};
use diagnostics::Diagnostic;
use language_semantics::MachineSupplyMode;
use symbols::SymbolHandle;

/// Project the trait, operator and top-level requirement realizations of one
/// non-external callable. External callables are projected by
/// `project_checked_external_supply_policy`.

pub(super) fn callable_conformance(
    compilation: &PackageReviewInput<'_>,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
    requirement: &typed_trees::signature::StateSignature,
    conformance: &typed_trees::machine::TraitConformance,
    binders: &[(SymbolHandle, String)],
    lifetime_parameters: &[typed_trees::name::Identifier],
    alias: Option<String>,
) -> Result<PackagePolicyCallableConformance, Vec<Diagnostic>> {
    Ok(PackagePolicyCallableConformance {
        trait_identity: nominal_identity(compilation, trait_definition.symbol)?,
        requirement_identity: trait_requirement_identity(
            compilation,
            trait_definition,
            requirement,
        )?,
        requirement_lifetime_partition:
            language_semantics::type_identity::normalize_requirement_lifetime_partition(
                &conformance.trait_lifetime_arguments,
            ),
        trait_lifetime_arguments: conformance.trait_lifetime_arguments.clone(),
        arguments: compilation
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .iter()
            .map(|argument| {
                review_signature_type_identity_with_binders(
                    compilation,
                    *argument,
                    binders,
                    lifetime_parameters,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        alias,
    })
}

pub(super) fn project_callable_conformances(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
    binders: &[(SymbolHandle, String)],
    lifetime_parameters: &[typed_trees::name::Identifier],
    require_public_trait: bool,
) -> Result<
    (
        Vec<PackagePolicyCallableConformance>,
        Vec<PackageReviewOperatorRealization>,
        Vec<crate::record::PackageReviewRequirementRealization>,
    ),
    Vec<Diagnostic>,
> {
    let mut policy = Vec::new();
    let mut operator_realizations = Vec::new();
    let mut requirement_realizations = Vec::new();
    for conformance in compilation.machine_trait_conformances(machine) {
        let trait_definition = compilation.trait_definition_by_symbol(conformance.symbol);
        let Some(trait_definition) = trait_definition else {
            let Some(requirement_name) = conformance.requirement.as_ref() else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` has an unresolved realization without an exact requirement",
                    machine.name
                ))]);
            };
            if let Some(typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                requirement,
            )) = typed_trees::machine::resolve_satisfied_declaration(
                compilation.checking_input,
                machine,
                conformance,
            ) {
                requirement_realizations.push(project_checked_requirement_realization(
                    compilation,
                    machine,
                    conformance,
                    requirement,
                )?);
                continue;
            }
            if !compilation
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .is_empty()
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` supplies type arguments to operator realization `{}::{}`",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            let operator =
                typed_trees::operator::resolve_satisfied_checked_operator_for_conformance(
                    compilation.checking_input,
                    machine,
                    conformance,
                )
                .or_else(|| {
                    typed_trees::operator::resolve_specialized_checked_operator_application(
                        compilation.checking_input,
                        machine,
                        conformance.name.as_str(),
                        requirement_name.as_str(),
                    )
                    .map(|(operator, _)| operator)
                });
            let Some(operator) = operator else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realization `{}::{}` resolves to neither one exact trait requirement nor one exact checked operator",
                    machine.name, conformance.name, requirement_name
                ))]);
            };
            if operator.symbol != conformance.requirement_symbol {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` operator realization `{}::{}` drifted from its retained exact overload",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if !operator.is_public && require_public_trait {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes non-public operator `{}::{}` whose complete contract is absent from package review",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if !matches!(machine.supply_mode, MachineSupplyMode::CheckedBody)
                || !machine.body_is_present
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes operator `{}::{}` without one checked implementation body",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if operator.is_boundary && operator.spelling.is_some() {
                if conformance.alias.is_some() {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed callable `{}` realizes fixed-token boundary operator `{}::{}` through an alias not represented by checked-adapter token dispatch",
                        machine.name, conformance.name, requirement_name
                    ))]);
                }
                if !operator.lifetime_parameters.is_empty() || !lifetime_parameters.is_empty() {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed callable `{}` realizes lifetime-parameterized fixed-token boundary operator `{}::{}` not represented by checked-adapter token dispatch",
                        machine.name, conformance.name, requirement_name
                    ))]);
                }
                validate_fixed_token_checked_adapter_dispatch_shape(
                    compilation,
                    machine,
                    operator,
                )?;
            }
            if operator.is_boundary {
                validate_selected_boundary_operator_checked_adapter(
                    compilation,
                    machine,
                    operator,
                )?;
            }
            if !operator.lifetime_parameters.is_empty() || !lifetime_parameters.is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes lifetime-parameterized operator `{}::{}` not yet represented by package review",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if compilation
                .operator_contracts(operator)
                .iter()
                .any(|contract| {
                    matches!(
                        contract.kind,
                        typed_trees::signature::SignatureContractKind::EnsuresForResultCase { .. }
                    )
                })
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes operator `{}::{}` with outcome-specific contracts outside checked operator refinement",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            let Some(_provider_envelope) = compilation
                .facts
                .contract_plans
                .realized_envelope(machine.symbol)
            else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed checked operator provider `{}` has no retained realized contract envelope",
                    machine.name
                ))]);
            };
            validation::validate_checked_operator_realization_contract(
                compilation.checking_input,
                machine,
                operator,
            )?;
            operator_realizations.push(PackageReviewOperatorRealization {
                coordinate: project_operator_coordinate(compilation, operator)?,
                alias: conformance
                    .alias
                    .as_ref()
                    .map(|alias| alias.as_str().to_owned()),
            });
            continue;
        };
        if require_public_trait && !trait_definition.is_public {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` realizes non-public trait `{}` whose complete contract is absent from package review",
                machine.name, trait_definition.name
            ))]);
        }
        if conformance.trait_lifetime_arguments.len() != trait_definition.lifetime_parameters.len()
            || conformance.trait_lifetime_arguments.iter().any(|ordinal| {
                usize::try_from(*ordinal)
                    .map_or(true, |ordinal| ordinal >= machine.lifetime_parameters.len())
            })
        {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` has an incomplete target-trait lifetime application for `{}`",
                machine.name, trait_definition.name
            ))]);
        }
        let Some(requirement_name) = conformance.requirement.as_ref() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` has a trait realization without an exact requirement",
                machine.name
            ))]);
        };
        let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
            definition: resolved_trait,
            requirement,
        }) = typed_trees::machine::resolve_satisfied_declaration(
            compilation.checking_input,
            machine,
            conformance,
        )
        else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` trait realization `{}::{}` does not resolve to one exact retained requirement overload",
                machine.name, trait_definition.name, requirement_name,
            ))]);
        };
        if resolved_trait.symbol != trait_definition.symbol
            || requirement.symbol != conformance.requirement_symbol
        {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` trait realization `{}::{}` drifted from its retained exact requirement",
                machine.name, trait_definition.name, requirement_name
            ))]);
        }
        policy.push(callable_conformance(
            compilation,
            trait_definition,
            requirement,
            conformance,
            binders,
            lifetime_parameters,
            conformance
                .alias
                .as_ref()
                .map(|alias| alias.as_str().to_owned()),
        )?);
    }
    super::conformance_order::finish(
        machine.name.as_str(),
        &mut policy,
        &mut operator_realizations,
        &mut requirement_realizations,
    )?;
    Ok((policy, operator_realizations, requirement_realizations))
}
