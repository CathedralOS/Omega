use super::{rejected, types};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::trait_definition::TraitDefinition;
use psi_typed_trees::types::TypeReferenceHandle;

#[derive(Clone)]
pub(super) struct Application {
    pub owner: TraitDefinition,
    pub arguments: Vec<TypeReferenceHandle>,
    pub lifetime_arguments: Vec<Identifier>,
    pub inherited_substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
}

pub(super) fn collect(
    compilation: &mut CheckedCompilation,
    application: Application,
    requirement: SymbolHandle,
    active: &mut Vec<SymbolHandle>,
    found: &mut Vec<Application>,
) -> Result<(), Vec<Diagnostic>> {
    if active.len() >= 64 || active.contains(&application.owner.symbol) {
        return Err(rejected(
            "calling requirement inheritance is cyclic or over-deep",
        ));
    }
    let parameters = compilation
        .trait_type_parameters(&application.owner)
        .to_vec();
    if parameters.len() != application.arguments.len()
        || application.owner.lifetime_parameters.len() != application.lifetime_arguments.len()
    {
        return Err(rejected(
            "calling requirement application has stale telescope arity",
        ));
    }
    let local = compilation
        .trait_machine_signatures(&application.owner)
        .iter()
        .filter(|signature| signature.symbol == requirement)
        .count();
    if local > 1 {
        return Err(rejected(
            "calling requirement repeats its declaration symbol",
        ));
    }
    if local == 1 {
        found.push(application.clone());
    }
    let mut substitutions = application.inherited_substitutions.clone();
    substitutions.extend(
        parameters
            .iter()
            .zip(&application.arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument))
            .collect::<Vec<_>>(),
    );
    let lifetimes = application
        .owner
        .lifetime_parameters
        .iter()
        .cloned()
        .zip(application.lifetime_arguments)
        .collect::<Vec<_>>();
    active.push(application.owner.symbol);
    for edge in compilation.trait_requirements(&application.owner).to_vec() {
        let parents = compilation
            .traits()
            .iter()
            .filter(|owner| owner.symbol == edge.symbol && owner.is_boundary)
            .cloned()
            .collect::<Vec<_>>();
        if parents.is_empty() {
            continue;
        }
        let [parent] = parents.as_slice() else {
            return Err(rejected("calling parent trait identity is ambiguous"));
        };
        let mut arguments = compilation
            .type_reference_table
            .type_reference_handles(edge.arguments)
            .to_vec();
        for argument in &mut arguments {
            *argument = types::instantiate(compilation, *argument, &substitutions, &lifetimes, 0)?;
        }
        let lifetime_arguments = edge
            .lifetime_arguments
            .iter()
            .map(|name| {
                lifetimes
                    .iter()
                    .find(|(source, _)| source == name)
                    .map(|(_, actual)| actual.clone())
                    .ok_or_else(|| {
                        rejected("calling parent lifetime is outside its declaring telescope")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        collect(
            compilation,
            Application {
                owner: parent.clone(),
                arguments,
                lifetime_arguments,
                inherited_substitutions: substitutions.clone(),
            },
            requirement,
            active,
            found,
        )?;
    }
    active.pop();
    Ok(())
}
