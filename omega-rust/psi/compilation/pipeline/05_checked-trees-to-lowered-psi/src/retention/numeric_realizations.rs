//! Preserve checked external satisfaction independently of executable call
//! shapes. Comparisons and folded calls still carry exact provider demands.

use crate::LoweringError;
use crate::machine_lowering::declarations;
use checked_trees::CheckedTrees;
use terminal_psi::{
    NumericRealization, NumericRealizationOwner, TerminalIdentityTableBuilder, TerminalModule,
};
use typed_trees::machine::{SatisfiedDeclaration, resolve_satisfied_declaration};

pub(crate) fn retain(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    module.numeric_realizations = project(
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
) -> Result<Vec<NumericRealization>, LoweringError> {
    let typed = &checked.typed;
    let mut candidates = Vec::new();
    for machine in typed.machines() {
        for conformance in typed.machine_trait_conformances(machine) {
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
                let Some(catalogue) =
                    typed.sealed_numeric_realization_descriptor(machine.symbol, requirement)
                else {
                    continue;
                };

                let numeric_intrinsic = catalogue.execution;
                let callable = crate::unit::attached_unit::checked_unit_boundary_identity(
                    checked,
                    requirement,
                )?
                .render(checked)?;
                let realization_callable =
                    crate::unit::attached_unit::checked_unit_boundary_identity(
                        checked,
                        machine.symbol,
                    )?
                    .render(checked)?;
                if !include(identities, &callable) && !include(identities, &realization_callable) {
                    continue;
                }
                let owner = typed.machine_code_owner_symbol(machine);
                let provider = if let Some(owner) = owner {
                    Some(NumericRealizationOwner {
                        declaration: declarations::nominal(identities, checked, sources, owner)
                            .map_err(LoweringError::Unsupported)?,
                        identity: identities.intern(
                            &typed.machine_code_owner_path(machine).ok_or(
                                LoweringError::Unsupported(
                                    "numeric realization has no exact code owner",
                                ),
                            )?,
                        ),
                        package: checked.symbols.symbol_package_identity(owner),
                    })
                } else {
                    None
                };
                if let language_semantics::numeric_intrinsics::NumericIntrinsic::Builtin {
                    function,
                    ..
                } = numeric_intrinsic
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
                let requirement_schema = typed_trees::typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement::by_symbol(typed, requirement)
                    .map(|requirement| terminal_psi::NumericRequirementSchema {
                        identity: identities.intern(&requirement.schema_identity()),
                        method: identities.intern(requirement.method_name()),
                        owner: identities.intern(requirement.method_owner()),
                    });
                let candidate = NumericRealization {
                    requirement: declarations::nominal(identities, checked, sources, requirement)
                        .map_err(LoweringError::Unsupported)?,
                    requirement_identity: identities.intern(&callable),
                    requirement_package: checked.symbols.symbol_package_identity(requirement),
                    numeric_intrinsic,
                    instruction_set: catalogue.instruction_set,
                    requirement_schema,
                    realization: declarations::identity(
                        identities,
                        checked,
                        sources,
                        machine.symbol,
                    )
                    .map_err(LoweringError::InvalidDeclarationIdentity)?,
                    realization_package: checked.symbols.symbol_package_identity(machine.symbol),
                    provider,
                };
                retain_declaration(requirement, candidate.requirement.clone());
                retain_declaration(machine.symbol, candidate.realization.machine.clone());
                if let Some(provider) = &candidate.provider {
                    retain_declaration(
                        owner.ok_or(LoweringError::Unsupported("numeric owner disappeared"))?,
                        provider.declaration.clone(),
                    );
                }
                candidates.push(candidate);
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}
