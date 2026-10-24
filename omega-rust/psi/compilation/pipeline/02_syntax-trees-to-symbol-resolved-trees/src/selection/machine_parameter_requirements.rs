//! Nominal machine-parameter requirements and trait requirement-identity
//! arguments.
//!
//! Both resolve once top-level symbols and direct trait requirements exist,
//! staged before mutation so one bad binder cannot leave a partially
//! normalized program behind.

use crate::selection::signature_free_requirements::{
    SignatureFreeRequirementResolutionError, resolve_signature_free_requirement,
};
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::{MachineParameterContract, TypeParameterKind};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::types::TypeReference;

/// Resolve every authored nominal machine-parameter requirement only after
/// top-level symbols and direct trait requirements exist. Resolution is
/// staged before mutation so one bad binder cannot leave a partially
/// normalized program behind.
pub(crate) fn normalize_nominal_machine_parameter_requirements(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    let mut replacements = Vec::new();

    for (handle, parameter) in program.tables.declarations.data_type_parameters.iter() {
        let Some((requirement, trait_arguments, rendered)) =
            super::authored_nominal_requirement(parameter)
        else {
            continue;
        };
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
            requirement.clone(),
            *trait_arguments,
        ));
    }

    for (handle, trait_definition, requirement, authored_path, trait_arguments) in replacements {
        let parameter = program
            .tables
            .declarations
            .data_type_parameters
            .get_mut(handle);
        parameter.kind = TypeParameterKind::Machine {
            contract: MachineParameterContract::Nominal {
                trait_definition,
                requirement,
                authored_path,
                trait_arguments,
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
        let Some(trait_definition) = program.trait_definition(conformance.trait_symbol) else {
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
            let TypeReference::Named { symbol, name, .. } = argument else {
                continue;
            };
            let rendered = name.as_str();
            let exact = if symbol.is_valid()
                && program.symbols.get(*symbol).kind == symbols::SymbolKind::State
                && program
                    .symbols
                    .source_reference_can_see_symbol(name.source_span(), *symbol)
                && !program.traits.iter().any(|definition| {
                    program
                        .trait_machine_signatures(definition.machines)
                        .iter()
                        .any(|requirement| requirement.symbol == *symbol)
                }) {
                *symbol
            } else {
                resolve_signature_free_requirement(program, &requirement_argument_path(name))
                    .map(|exact| exact.requirement.symbol)
                    .map_err(|error| {
                        Diagnostic::error(format!(
                            "trait machine requirement argument `{rendered}` {}",
                            requirement_resolution_reason(error)
                        ))
                    })?
            };
            let handle = arena::Handle::from_parts(
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

/// A `TypeReference::Named` argument retains one rendered `Trait::requirement`
/// spelling; splitting it back into exact name segments (each carrying the
/// whole spelling's span) lets the shared signature-free law resolve it
/// instead of a second copy of that law.
fn requirement_argument_path(name: &DiagnosticName) -> Vec<DiagnosticName> {
    name.as_str()
        .split("::")
        .map(|segment| DiagnosticName::from_str(segment, name.source_span()))
        .collect()
}

/// The diagnostic sentences for the shared resolution law's error variants.
fn requirement_resolution_reason(error: SignatureFreeRequirementResolutionError) -> &'static str {
    match error {
        SignatureFreeRequirementResolutionError::InvalidPath => {
            "expected one exact `Trait::requirement` path"
        }
        SignatureFreeRequirementResolutionError::TraitNotUnique => {
            "path does not resolve to one exact trait"
        }
        SignatureFreeRequirementResolutionError::RequirementNotUnique => {
            "path does not resolve to one exact trait requirement; signature-free references reject overloads"
        }
    }
}
