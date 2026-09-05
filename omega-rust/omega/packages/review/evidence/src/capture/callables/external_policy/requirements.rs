use super::rejected;
use crate::capture::{
    api::operators::project_operator_coordinate,
    semantics::{
        declarations::{
            nominal_identity, top_level_requirement_identity, trait_requirement_identity,
        },
        types::review_signature_type_identity_with_binders,
    },
};
use crate::record::*;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::machine::{Machine, SatisfiedDeclaration, TraitConformance};

pub(super) fn project(
    compilation: &CheckedCompilation,
    machine: &Machine,
    conformance: &TraitConformance,
    binders: &[(SymbolHandle, String)],
    binding: &PackageReviewExternalBinding,
) -> Result<PackagePolicyExternalRequirement, Vec<Diagnostic>> {
    let declaration = typed_trees::machine::resolve_satisfied_declaration(
        &compilation.typed,
        machine,
        conformance,
    )
    .ok_or_else(|| rejected("external realization has no unique exact requirement"))?;
    if declaration.symbol() != conformance.requirement_symbol {
        return Err(rejected(
            "external realization changed its retained requirement",
        ));
    }
    let alias = conformance
        .alias
        .as_ref()
        .map(|alias| alias.as_str().to_owned());
    Ok(match declaration {
        SatisfiedDeclaration::Trait {
            definition,
            requirement,
        } => {
            if definition.symbol != conformance.symbol
                || conformance.trait_lifetime_arguments.len()
                    != definition.lifetime_parameters.len()
                || conformance
                    .trait_lifetime_arguments
                    .iter()
                    .any(|ordinal| *ordinal as usize >= machine.lifetime_parameters.len())
            {
                return Err(rejected(
                    "external trait application has incomplete lifetime or declaring owner coordinates",
                ));
            }
            PackagePolicyExternalRequirement::Trait(PackagePolicyCallableConformance {
                trait_identity: nominal_identity(compilation, definition.symbol)?,
                requirement_identity: trait_requirement_identity(
                    compilation,
                    definition,
                    requirement,
                )?,
                requirement_lifetime_partition:
                    typed_trees::machine::normalize_requirement_lifetime_partition(
                        &conformance.trait_lifetime_arguments,
                    ),
                trait_lifetime_arguments: conformance.trait_lifetime_arguments.clone(),
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
                alias,
            })
        }
        SatisfiedDeclaration::TopLevelRequirement(requirement) => {
            if conformance.symbol != requirement.symbol {
                return Err(rejected("external top-level requirement owner drifted"));
            }
            if !requirement.is_public
                || matches!(binding, PackageReviewExternalBinding::CompilerIntrinsic)
            {
                return Err(rejected(
                    "external top-level requirement lacks a public contract or supported closed execution",
                ));
            }
            validation::revalidate_top_level_requirement_realization(
                &compilation.typed,
                machine,
                requirement,
                conformance,
            )?;
            super::super::boundary_requirements::validate_selected_top_level_requirement_external_supply(compilation, machine, requirement, binding)?;
            PackagePolicyExternalRequirement::TopLevelRequirement {
                identity: top_level_requirement_identity(compilation, requirement)?,
                signature: super::signatures::project(compilation, requirement)?.1,
                alias,
            }
        }
        SatisfiedDeclaration::Operator(operator) => {
            if !operator.is_boundary
                || !operator.is_public
                || !compilation
                    .type_reference_table
                    .type_reference_handles(conformance.arguments)
                    .is_empty()
            {
                return Err(rejected(
                    "external operator has no exact public boundary declaration",
                ));
            }
            if !operator.lifetime_parameters.is_empty()
                || !compilation.operator_type_parameters(operator).is_empty()
                || !machine.lifetime_parameters.is_empty()
                || !machine.type_parameters.is_empty()
            {
                return Err(rejected("external operator application is not closed"));
            }
            if operator.spelling.is_some() && (!matches!(binding, PackageReviewExternalBinding::CompilerIntrinsic) || provider_planning::plans::primitive_float_binary_intrinsic_execution_identity(&compilation.typed, operator).is_none()) { return Err(rejected("fixed-token external operator has no closed intrinsic execution")); }
            super::super::boundary_operators::validate_selected_boundary_operator_external_supply(
                compilation,
                machine,
                operator,
                binding,
            )?;
            PackagePolicyExternalRequirement::Operator {
                coordinate: project_operator_coordinate(compilation, operator)?,
                alias,
            }
        }
    })
}
