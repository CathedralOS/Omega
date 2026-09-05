use crate::capture::semantics::declarations::{
    nominal_identity, nominal_owner, provider_requirement_schema,
};
use crate::record::PackageReviewNominalIdentity;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn trait_requirement_identity(
    compilation: &CheckedCompilation,
    owner: &psi_typed_trees::trait_definition::TraitDefinition,
    requirement: &psi_typed_trees::signature::StateSignature,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owner_identity = nominal_identity(compilation, owner.symbol)?;
    let requirement_owner = nominal_owner(compilation, requirement.symbol)?;
    if owner_identity.owner != requirement_owner {
        return Err(vec![Diagnostic::error(format!(
            "package review trait `{}` and requirement `{}` have mismatched exact ownership",
            owner.name, requirement.name
        ))]);
    }
    Ok(PackageReviewNominalIdentity {
        owner: requirement_owner,
        path: compilation
            .normalized_trait_requirement_overload_identity(owner, requirement)
            .identity(),
    })
}

pub(crate) fn trait_requirement_identity_from_symbols(
    compilation: &CheckedCompilation,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    context: &str,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owners = compilation
        .traits()
        .iter()
        .filter(|candidate| candidate.symbol == trait_symbol)
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{context} resolves its declaring trait to {} declarations; expected exactly one",
            owners.len()
        ))]);
    };
    let requirements = compilation
        .trait_machine_signatures(owner)
        .iter()
        .filter(|candidate| candidate.symbol == requirement_symbol)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{context} resolves its requirement to {} overload declarations under its exact trait; expected exactly one",
            requirements.len()
        ))]);
    };
    trait_requirement_identity(compilation, owner, requirement)
}

pub(crate) fn provider_requirement_identity(
    compilation: &CheckedCompilation,
    schema: omega_provider_planning::plans::ProviderSchemaDeclaration,
    requirement_symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    match provider_requirement_schema(compilation, schema, requirement_symbol)? {
        omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryTrait(trait_symbol) => {
            trait_requirement_identity_from_symbols(
                compilation,
                trait_symbol,
                requirement_symbol,
                "selected provider row",
            )
        }
        omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryRequirement(
            schema_symbol,
        ) => {
            let matches = compilation
                .machines()
                .iter()
                .filter(|candidate| {
                    candidate.symbol == schema_symbol
                        && candidate.symbol == requirement_symbol
                        && candidate.supply_mode
                            == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
                })
                .collect::<Vec<_>>();
            let [requirement] = matches.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider row resolves its top-level boundary requirement to {} declarations; expected exactly one",
                    matches.len(),
                ))]);
            };
            top_level_requirement_identity(compilation, requirement)
        }
        omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryOperator(_) => {
            let operators = compilation.operators().iter().chain(
                compilation
                    .domain_definitions()
                    .iter()
                    .flat_map(|domain| compilation.domain_operators(domain)),
            );
            let matches = operators
                .filter(|candidate| candidate.symbol == requirement_symbol && candidate.is_boundary)
                .collect::<Vec<_>>();
            let [operator] = matches.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider row resolves its boundary operator requirement to {} declarations; expected exactly one",
                    matches.len()
                ))]);
            };
            let nominal = nominal_identity(compilation, requirement_symbol)?;
            Ok(PackageReviewNominalIdentity {
                owner: nominal.owner,
                path: psi_typed_trees::operator::boundary_operator_requirement_identity(
                    &compilation.typed,
                    operator,
                ),
            })
        }
    }
}

pub(crate) fn top_level_requirement_identity(
    compilation: &CheckedCompilation,
    requirement: &psi_typed_trees::machine::Machine,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    if requirement.supply_mode != psi_language_semantics::MachineSupplyMode::TopLevelRequirement {
        return Err(vec![Diagnostic::error(format!(
            "package review declaration `{}` is not a top-level boundary requirement",
            requirement.name
        ))]);
    }
    let path = compilation
        .normalized_machine_overload_identity(requirement)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "top-level requirement `{}` has no normalized overload identity",
                requirement.name
            ))]
        })?
        .identity();
    Ok(PackageReviewNominalIdentity {
        owner: nominal_owner(compilation, requirement.symbol)?,
        path,
    })
}
