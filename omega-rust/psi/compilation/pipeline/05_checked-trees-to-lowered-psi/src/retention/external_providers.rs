//! Preserve checked external satisfaction independently of executable call
//! shapes. Comparisons and folded calls still carry exact provider demands.

use crate::LoweringError;
use crate::machine_lowering::declarations;
use checked_trees::CheckedTrees;
use terminal_psi::{
    ExternalProviderConformance, ExternalProviderOwner, TerminalIdentityTableBuilder,
    TerminalModule,
};
use typed_trees::machine::{SatisfiedDeclaration, resolve_satisfied_declaration};

pub(crate) fn retain(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    module.external_providers = project(
        identities,
        checked,
        sources,
        |identities, callable| {
            module
                .boundary_machines
                .iter()
                .any(|boundary| identities.text(boundary.identity) == Some(callable))
                || module
                    .boundary_application_demands
                    .iter()
                    .any(|demand| demand.requirement().overload() == callable)
        },
        |_, _| {},
    )?;
    Ok(())
}

pub(crate) fn project(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    include: impl Fn(&TerminalIdentityTableBuilder, &str) -> bool,
    mut retain_declaration: impl FnMut(symbols::SymbolHandle, terminal_psi::NominalDeclarationIdentity),
) -> Result<Vec<ExternalProviderConformance>, LoweringError> {
    let typed = &checked.typed;
    let mut candidates = Vec::new();
    for machine in typed.machines() {
        for conformance in typed.machine_trait_conformances(machine) {
            let Some(binding) = conformance
                .external_binding
                .and_then(|binding| typed.external_bindings.identity(binding))
            else {
                continue;
            };
            let Some(satisfied) = resolve_satisfied_declaration(typed, machine, conformance) else {
                continue;
            };
            if matches!(satisfied, SatisfiedDeclaration::Trait { definition, .. } if !definition.is_boundary)
            {
                continue;
            }
            let requirements = match satisfied {
                SatisfiedDeclaration::TopLevelRequirement(requirement)
                    if !typed.machine_type_parameters(requirement).is_empty() =>
                {
                    checked
                        .facts
                        .provider_closed_requirements
                        .iter()
                        .filter(|row| {
                            row.provider == machine.symbol
                                && row.selecting_declaration == machine.symbol
                                && row.template == requirement.symbol
                        })
                        .map(|row| row.closed_requirement)
                        .collect::<Vec<_>>()
                }
                _ => vec![satisfied.symbol()],
            };
            for requirement in requirements {
                // Declaration projection also visits open trait templates.
                // They have an exact authored callable but no executable
                // Terminal selection until an application closes them. Do not
                // ask the executable call-closure lookup to name a template.
                let callable = match satisfied {
                    SatisfiedDeclaration::Trait {
                        definition,
                        requirement,
                    } => {
                        let identity =
                            typed.trait_requirement_callable_identity(definition, requirement);
                        if !identity.is_valid() {
                            return Err(LoweringError::Unsupported(
                                "external trait declaration has no exact callable identity",
                            ));
                        }
                        typed.callable_identity_text(identity)
                    }
                    _ => crate::unit::attached_unit::checked_unit_boundary_identity(
                        checked,
                        requirement,
                    )?
                    .render(checked)?,
                };
                if !include(identities, &callable) {
                    continue;
                }
                let provider = if machine.attached_data_symbol.is_valid() {
                    Some(ExternalProviderOwner {
                        declaration: declarations::nominal(
                            identities,
                            checked,
                            sources,
                            machine.attached_data_symbol,
                        )
                        .map_err(LoweringError::Unsupported)?,
                        identity: identities.intern(&typed.attached_data_path(machine).ok_or(
                            LoweringError::Unsupported(
                                "external provider has no attached owner identity",
                            ),
                        )?),
                        package: checked
                            .symbols
                            .symbol_package_identity(machine.attached_data_symbol),
                    })
                } else {
                    None
                };
                let numeric_intrinsic = typed.numeric_intrinsic_execution(requirement);
                let requirement_schema = typed_trees::typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement::by_symbol(typed, requirement)
                    .map(|requirement| terminal_psi::ExternalRequirementSchema {
                        identity: identities.intern(&requirement.schema_identity()),
                        method: identities.intern(requirement.method_name()),
                        owner: identities.intern(requirement.method_owner()),
                    });
                if let Some(language_semantics::numeric_intrinsics::NumericIntrinsic::Builtin {
                    function,
                    ..
                }) = numeric_intrinsic
                {
                    let symbol = checked.symbols.builtin_function_symbol(function).ok_or(
                        LoweringError::Unsupported(
                            "numeric builtin is absent from its exact root slot",
                        ),
                    )?;
                    if checked.symbols.builtin_function_for_symbol(symbol) != Some(function) {
                        return Err(LoweringError::Unsupported(
                            "numeric builtin does not round-trip through its exact root slot",
                        ));
                    }
                }
                let candidate = ExternalProviderConformance {
                    requirement: declarations::nominal(identities, checked, sources, requirement)
                        .map_err(LoweringError::Unsupported)?,
                    requirement_identity: identities.intern(&callable),
                    requirement_package: checked.symbols.symbol_package_identity(requirement),
                    numeric_intrinsic,
                    requirement_schema,
                    target: typed.machine_authored_target(machine).map(str::to_owned),
                    realization: declarations::identity(
                        identities,
                        checked,
                        sources,
                        machine.symbol,
                    )
                    .map_err(LoweringError::InvalidDeclarationIdentity)?,
                    realization_package: checked.symbols.symbol_package_identity(machine.symbol),
                    provider,
                    binding: binding.clone(),
                };
                retain_declaration(requirement, candidate.requirement.clone());
                retain_declaration(machine.symbol, candidate.realization.machine.clone());
                if let Some(provider) = &candidate.provider {
                    retain_declaration(machine.attached_data_symbol, provider.declaration.clone());
                }
                candidates.push(candidate);
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}
