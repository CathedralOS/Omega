use crate::signature_free_requirements::{
    SignatureFreeRequirementResolutionError, resolve_signature_free_requirement,
};
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbol_resolved_trees::data::{MachineParameterContract, TypeParameterKind};

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
