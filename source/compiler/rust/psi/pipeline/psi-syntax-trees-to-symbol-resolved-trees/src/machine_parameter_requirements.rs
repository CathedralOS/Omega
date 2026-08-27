use crate::signature_free_requirements::{
    SignatureFreeRequirementResolutionError, resolve_signature_free_requirement,
};
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbol_resolved_trees::data::{MachineParameterContract, TypeParameterKind};
use psi_symbol_resolved_trees::types::TypeReference;

/// Resolve every authored nominal machine-parameter requirement only after
/// top-level symbols and direct trait requirements exist. Resolution is
/// staged before mutation so one bad binder cannot leave a partially
/// normalized program behind.
pub(crate) fn normalize_nominal_machine_parameter_requirements(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    let mut replacements = Vec::new();

    for (handle, parameter) in program.tables.declarations.data_type_parameters.iter() {
        let TypeParameterKind::Machine {
            contract: MachineParameterContract::AuthoredNominal { requirement },
        } = &parameter.kind
        else {
            continue;
        };

        let rendered = requirement
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let exact = resolve_signature_free_requirement(program, requirement).map_err(|error| {
            Diagnostic::error(match error {
                SignatureFreeRequirementResolutionError::InvalidPath => format!(
                    "nominal machine parameter `{}` requirement `{rendered}` must name an exact `Trait::requirement`",
                    parameter.name
                ),
                SignatureFreeRequirementResolutionError::TraitNotUnique => format!(
                    "nominal machine parameter `{}` requirement `{rendered}` does not resolve to one exact trait",
                    parameter.name
                ),
                SignatureFreeRequirementResolutionError::RequirementNotUnique => format!(
                    "nominal machine parameter `{}` requirement `{rendered}` does not resolve to one exact trait requirement; signature-free references reject overloads",
                    parameter.name
                ),
            })
        })?;
        replacements.push((
            handle,
            exact.trait_definition.symbol,
            exact.requirement.symbol,
        ));
    }

    for (handle, trait_definition, requirement) in replacements {
        let parameter = program
            .tables
            .declarations
            .data_type_parameters
            .get_mut(handle);
        parameter.kind = TypeParameterKind::Machine {
            contract: MachineParameterContract::Nominal {
                trait_definition,
                requirement,
            },
        };
    }

    Ok(())
}

/// Bind trait-level requirement-identity arguments after the complete trait
/// catalog exists. The generic argument carrier is shared with type arguments,
/// but this slot accepts only one exact signature-free `Trait::requirement`
/// declaration; concrete machines and ordinary types fail closed.
pub(crate) fn normalize_trait_machine_requirement_arguments(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    let mut replacements = Vec::new();

    for conformance in &program.conformances {
        let Some(trait_definition) = program
            .traits
            .iter()
            .find(|candidate| candidate.symbol == conformance.trait_symbol)
        else {
            continue;
        };
        let parameters = program.trait_type_parameters(trait_definition);
        let arguments = program.child_type_references(conformance.arguments);
        for (index, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
            if !matches!(
                parameter.kind,
                TypeParameterKind::Machine {
                    contract: MachineParameterContract::RequirementIdentity
                }
            ) {
                continue;
            }
            let TypeReference::Named { symbol, name } = argument else {
                continue;
            };
            let rendered = name.as_str();
            let exact = if symbol.is_valid()
                && program.symbols.get(*symbol).kind == psi_symbols::SymbolKind::State
                && !program.traits.iter().any(|definition| {
                    program
                        .trait_machine_signatures(definition.machines)
                        .iter()
                        .any(|requirement| requirement.symbol == *symbol)
                }) {
                *symbol
            } else {
                resolve_rendered_requirement(program, rendered).map_err(|reason| {
                    Diagnostic::error(format!(
                        "trait machine requirement argument `{rendered}` {reason}"
                    ))
                })?
            };
            let handle = psi_arena::Handle::from_parts(
                conformance
                    .arguments
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(index).expect("trait argument index fits u32"))
                    .expect("trait argument handle overflow"),
                conformance.arguments.start().generation(),
            );
            replacements.push((handle, exact));
        }
    }

    for (handle, requirement) in replacements {
        let TypeReference::Named { symbol, .. } = program
            .tables
            .declarations
            .child_type_references
            .get_mut(handle)
        else {
            unreachable!("staged requirement-identity argument remains a name")
        };
        *symbol = requirement;
    }
    Ok(())
}

fn resolve_rendered_requirement(
    program: &SymbolResolvedTrees,
    rendered: &str,
) -> Result<psi_symbols::SymbolHandle, &'static str> {
    let members = rendered.split("::").collect::<Vec<_>>();
    let [trait_path @ .., requirement_name] = members.as_slice() else {
        return Err("expected one exact `Trait::requirement` path");
    };
    if trait_path.is_empty() {
        return Err("expected one exact `Trait::requirement` path");
    }
    let trait_name = trait_path.join("::");
    let matching_traits = program
        .traits
        .iter()
        .filter(|definition| {
            crate::signature_free_requirements::same_semantic_name(
                definition.name.as_str(),
                &trait_name,
            )
        })
        .collect::<Vec<_>>();
    let [trait_definition] = matching_traits.as_slice() else {
        return Err("path does not resolve to one exact trait");
    };
    let matching_requirements = program
        .trait_machine_signatures(trait_definition.machines)
        .iter()
        .filter(|signature| signature.name.as_str() == *requirement_name)
        .collect::<Vec<_>>();
    let [requirement] = matching_requirements.as_slice() else {
        return Err(
            "path does not resolve to one exact trait requirement; signature-free references reject overloads",
        );
    };
    Ok(requirement.symbol)
}
