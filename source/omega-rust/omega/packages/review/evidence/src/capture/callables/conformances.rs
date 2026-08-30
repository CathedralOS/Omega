use super::super::api::operators::project_operator_coordinate;
use super::super::semantics::declarations::{nominal_identity, trait_requirement_identity};
use super::super::semantics::types::review_signature_type_identity_with_binders;
use super::boundary_operators::{
    validate_selected_boundary_operator_checked_adapter,
    validate_selected_boundary_operator_external_supply,
};
use super::external_supply::{
    project_external_binding, project_external_executable_supply_with_source,
    validate_external_binding_payload,
};
use crate::capture::source::ProjectedReviewRow;
use crate::record::{
    PackageReviewCallableConformance, PackageReviewExternalBinding,
    PackageReviewExternalExecutableSupply, PackageReviewExternalRequirement,
    PackageReviewNominalIdentity, PackageReviewOperatorRealization,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;
use psi_symbols::SymbolHandle;

pub(super) fn project_callable_conformances(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    callable_identity: &PackageReviewNominalIdentity,
    binders: &[(SymbolHandle, String)],
    require_public_trait: bool,
) -> Result<
    (
        Vec<PackageReviewCallableConformance>,
        Vec<PackageReviewOperatorRealization>,
        Vec<ProjectedReviewRow<PackageReviewExternalExecutableSupply>>,
    ),
    Vec<Diagnostic>,
> {
    let expected_external = match machine.supply_mode {
        MachineSupplyMode::ExternalRealization { binding, mechanism } => {
            if machine.body_is_present {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` retains an implementation body",
                    machine.name
                ))]);
            }
            let conformances = compilation.machine_trait_conformances(machine);
            if conformances.len() != 1 {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has {} conformance applications; expected exactly one",
                    machine.name,
                    conformances.len()
                ))]);
            }
            let Some(identity) = compilation.external_bindings.identity(binding) else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has no exact binding-table identity",
                    machine.name
                ))]);
            };
            if identity.mechanism() != mechanism {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has a supply mechanism inconsistent with its exact binding identity",
                    machine.name
                ))]);
            }
            validate_external_binding_payload(compilation, machine, identity)?;
            Some((binding, project_external_binding(identity)))
        }
        MachineSupplyMode::CheckedBody
        | MachineSupplyMode::Requirement
        | MachineSupplyMode::Boundary
        | MachineSupplyMode::Accepted => None,
    };
    let mut projected = Vec::new();
    let mut operator_realizations = Vec::new();
    let mut external_executable_supply = Vec::new();
    for conformance in compilation.machine_trait_conformances(machine) {
        match (
            conformance.external_binding,
            conformance.external_binding_source_span,
        ) {
            (None, None) | (Some(_), Some(_)) => {}
            (None, Some(_)) => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` retains authored `via` custody without an external binding",
                    machine.name
                ))]);
            }
            (Some(_), None) => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has no exact authored `via` custody",
                    machine.name
                ))]);
            }
        }
        match (expected_external.as_ref(), conformance.external_binding) {
            (None, None) => {}
            (None, Some(_)) => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` retains an external conformance binding without external supply",
                    machine.name
                ))]);
            }
            (Some(_), None) => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has a conformance without its exact external binding",
                    machine.name
                ))]);
            }
            (Some((expected, _)), Some(actual)) if *expected != actual => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has a conformance binding inconsistent with its supply mode",
                    machine.name
                ))]);
            }
            (Some(_), Some(_)) => {}
        }
        let trait_definition = compilation
            .traits()
            .iter()
            .find(|definition| definition.symbol == conformance.symbol);
        let Some(trait_definition) = trait_definition else {
            let Some(requirement_name) = conformance.requirement.as_ref() else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` has an unresolved realization without an exact requirement",
                    machine.name
                ))]);
            };
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
            let external_operator =
                expected_external.is_some() || conformance.external_binding.is_some();
            let operator = if external_operator {
                psi_typed_trees::operator::resolve_satisfied_boundary_operator(
                    &compilation.typed,
                    machine,
                    conformance.name.as_str(),
                    requirement_name.as_str(),
                )
            } else {
                psi_typed_trees::operator::resolve_satisfied_checked_operator(
                    &compilation.typed,
                    machine,
                    conformance.name.as_str(),
                    requirement_name.as_str(),
                )
            };
            let Some(operator) = operator else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realization `{}::{}` resolves to neither one exact trait requirement nor one exact {}operator",
                    machine.name,
                    conformance.name,
                    requirement_name,
                    if external_operator {
                        "boundary "
                    } else {
                        "checked "
                    }
                ))]);
            };
            if operator.symbol != conformance.requirement_symbol {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` operator realization `{}::{}` drifted from its retained exact overload",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if !operator.is_public && (external_operator || require_public_trait) {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes non-public operator `{}::{}` whose complete contract is absent from package review",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if external_operator {
                let Some((_, binding)) = expected_external.as_ref() else {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed callable `{}` realizes boundary operator `{}::{}` through an external binding without exact external supply",
                        machine.name, conformance.name, requirement_name
                    ))]);
                };
                if operator.spelling.is_some()
                    && (!matches!(binding, PackageReviewExternalBinding::CompilerIntrinsic)
                        || omega_provider_planning::plans::primitive_float_binary_intrinsic_execution_identity(
                            &compilation.typed,
                            operator,
                        )
                        .is_none())
                {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed callable `{}` realizes fixed-token boundary operator `{}::{}` without one exact closed compiler-intrinsic execution",
                        machine.name, conformance.name, requirement_name
                    ))]);
                }
                if !operator.lifetime_parameters.is_empty()
                    || !compilation.operator_type_parameters(operator).is_empty()
                    || !machine.lifetime_parameters.is_empty()
                    || !machine.type_parameters.is_empty()
                {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed callable `{}` realizes generic or lifetime-parameterized boundary operator `{}::{}` through external supply not yet represented by package review",
                        machine.name, conformance.name, requirement_name
                    ))]);
                }
                if conformance.alias.is_some() {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed callable `{}` realizes boundary operator `{}::{}` through an alias not yet represented by package review",
                        machine.name, conformance.name, requirement_name
                    ))]);
                }
                validate_selected_boundary_operator_external_supply(
                    compilation,
                    machine,
                    operator,
                    binding,
                )?;
                let coordinate = project_operator_coordinate(compilation, operator)?;
                if require_public_trait {
                    operator_realizations.push(PackageReviewOperatorRealization {
                        coordinate: coordinate.clone(),
                        alias: None,
                    });
                }
                external_executable_supply.push(project_external_executable_supply_with_source(
                    machine,
                    conformance,
                    PackageReviewExternalExecutableSupply {
                        callable: callable_identity.clone(),
                        requirement: PackageReviewExternalRequirement::Operator(coordinate),
                        binding: binding.clone(),
                    },
                )?);
                continue;
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
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes fixed-token boundary operator `{}::{}` before checked-adapter token dispatch is represented",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if operator.is_boundary {
                validate_selected_boundary_operator_checked_adapter(
                    compilation,
                    machine,
                    operator,
                )?;
            }
            if !operator.lifetime_parameters.is_empty()
                || !compilation.operator_type_parameters(operator).is_empty()
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes generic or lifetime-parameterized operator `{}::{}` not yet represented by package review",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if compilation.operator_contracts(operator).iter().any(|contract| {
                matches!(
                    contract.kind,
                    psi_typed_trees::signature::SignatureContractKind::EnsuresForResultCase { .. }
                )
            }) {
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
            psi_validation::validate_checked_operator_realization_contract(
                &compilation.typed,
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
        if !trait_definition.lifetime_parameters.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` realizes lifetime-parameterized trait `{}` without retained conformance lifetime arguments",
                machine.name, trait_definition.name
            ))]);
        }
        let Some(requirement_name) = conformance.requirement.as_ref() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` has a trait realization without an exact requirement",
                machine.name
            ))]);
        };
        let Some(psi_typed_trees::machine::SatisfiedDeclaration::Trait {
            definition: resolved_trait,
            requirement,
        }) = psi_typed_trees::machine::resolve_satisfied_declaration(
            &compilation.typed,
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
        let row = PackageReviewCallableConformance {
            trait_identity: nominal_identity(compilation, trait_definition.symbol)?,
            requirement_identity: trait_requirement_identity(
                compilation,
                trait_definition,
                requirement,
            )?,
            arguments: compilation
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .iter()
                .map(|argument| {
                    review_signature_type_identity_with_binders(
                        compilation,
                        *argument,
                        binders,
                        &machine.lifetime_parameters,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            alias: conformance
                .alias
                .as_ref()
                .map(|alias| alias.as_str().to_owned()),
        };
        if let Some((_, binding)) = expected_external.as_ref() {
            external_executable_supply.push(project_external_executable_supply_with_source(
                machine,
                conformance,
                PackageReviewExternalExecutableSupply {
                    callable: callable_identity.clone(),
                    requirement: PackageReviewExternalRequirement::Trait(row.clone()),
                    binding: binding.clone(),
                },
            )?);
        }
        projected.push(row);
    }
    if expected_external.is_some() && external_executable_supply.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed external callable `{}` has no exact conformance application",
            machine.name
        ))]);
    }
    projected.sort();
    if projected.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` contains a duplicate exact trait realization",
            machine.name
        ))]);
    }
    operator_realizations.sort();
    if operator_realizations
        .windows(2)
        .any(|rows| rows[0] == rows[1])
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` contains a duplicate exact operator realization",
            machine.name
        ))]);
    }
    external_executable_supply.sort_by(|left, right| left.row.cmp(&right.row));
    if external_executable_supply
        .windows(2)
        .any(|rows| rows[0].row == rows[1].row)
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed external callable `{}` contains duplicate executable-supply identity",
            machine.name
        ))]);
    }
    Ok((projected, operator_realizations, external_executable_supply))
}
