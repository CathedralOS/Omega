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
                let callable = crate::unit::attached_unit::checked_unit_boundary_identity(
                    checked,
                    requirement,
                )?
                .render(checked)?;
                let demanded =
                    module.boundary_machines.iter().any(|boundary| {
                        identities.text(boundary.identity) == Some(callable.as_str())
                    }) || module
                        .boundary_application_demands
                        .iter()
                        .any(|demand| demand.requirement().overload() == callable);
                if !demanded {
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
                candidates.push(ExternalProviderConformance {
                    requirement: declarations::nominal(identities, checked, sources, requirement)
                        .map_err(LoweringError::Unsupported)?,
                    requirement_identity: identities.intern(&callable),
                    requirement_package: checked.symbols.symbol_package_identity(requirement),
                    numeric_intrinsic: typed.numeric_intrinsic_execution(requirement),
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
                });
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    module.external_providers = candidates;
    Ok(())
}
