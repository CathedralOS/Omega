use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbol_resolved_trees::name::DiagnosticName;
use psi_symbol_resolved_trees::signature::StateSignature;
use psi_symbol_resolved_trees::trait_definition::TraitDefinition;

/// One exact trait-requirement row selected by a path that carries no call
/// signature. Domain establishment routes and nominal static-machine binders
/// share this resolution law: neither visible satisfiers nor an expected call
/// shape may select among overloads.
pub(crate) struct ExactSignatureFreeRequirement<'program> {
    pub(crate) trait_definition: &'program TraitDefinition,
    pub(crate) requirement: &'program StateSignature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignatureFreeRequirementResolutionError {
    InvalidPath,
    TraitNotUnique,
    RequirementNotUnique,
}

pub(crate) fn resolve_signature_free_requirement<'program>(
    program: &'program SymbolResolvedTrees,
    path: &[DiagnosticName],
) -> Result<ExactSignatureFreeRequirement<'program>, SignatureFreeRequirementResolutionError> {
    let [trait_path @ .., requirement_name] = path else {
        return Err(SignatureFreeRequirementResolutionError::InvalidPath);
    };
    if trait_path.is_empty() {
        return Err(SignatureFreeRequirementResolutionError::InvalidPath);
    }

    let trait_name = trait_path
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let matching_traits = program
        .traits
        .iter()
        .filter(|definition| same_semantic_name(definition.name.as_str(), &trait_name))
        .collect::<Vec<_>>();
    let [trait_definition] = matching_traits.as_slice() else {
        return Err(SignatureFreeRequirementResolutionError::TraitNotUnique);
    };

    let matching_requirements = program
        .trait_machine_signatures(trait_definition.machines)
        .iter()
        .filter(|signature| signature.name.as_str() == requirement_name.as_str())
        .collect::<Vec<_>>();
    let [requirement] = matching_requirements.as_slice() else {
        return Err(SignatureFreeRequirementResolutionError::RequirementNotUnique);
    };

    Ok(ExactSignatureFreeRequirement {
        trait_definition,
        requirement,
    })
}

pub(crate) fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}
