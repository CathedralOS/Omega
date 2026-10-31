use super::rejected;
use crate::capture::PackageReviewInput;
use crate::capture::{
    api::operators::project_operator_coordinate,
    semantics::declarations::top_level_requirement_identity,
};
use crate::record::{PackagePolicyExternalRequirement, PackageReviewExternalBinding};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::machine::{Machine, SatisfiedDeclaration, TraitConformance};

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    machine: &Machine,
    conformance: &TraitConformance,
    binders: &[(SymbolHandle, String)],
    lifetime_parameters: &[typed_trees::name::Identifier],
    binding: &PackageReviewExternalBinding,
) -> Result<PackagePolicyExternalRequirement, Vec<Diagnostic>> {
    let declaration = typed_trees::machine::resolve_satisfied_declaration(
        compilation.checking_input,
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
            PackagePolicyExternalRequirement::Trait(
                crate::capture::callables::conformances::callable_conformance(
                    compilation,
                    definition,
                    requirement,
                    conformance,
                    binders,
                    lifetime_parameters,
                    alias,
                )?,
            )
        }
        SatisfiedDeclaration::TopLevelRequirement(requirement) => {
            if conformance.symbol != requirement.symbol {
                return Err(rejected("external top-level requirement owner drifted"));
            }
            if matches!(binding, PackageReviewExternalBinding::CompilerIntrinsic) {
                return Err(rejected(
                    "external top-level requirement lacks supported closed execution",
                ));
            }
            validation::revalidate_top_level_requirement_realization(
                compilation.checking_input,
                machine,
                requirement,
                conformance,
            )?;
            super::super::boundary_requirements::validate_selected_top_level_requirement_external_supply(compilation, machine, requirement, binding)?;
            PackagePolicyExternalRequirement::TopLevelRequirement {
                identity: top_level_requirement_identity(compilation, requirement)?,
                signature: super::signatures::project(compilation, requirement)?.2,
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
                || !lifetime_parameters.is_empty()
                || !binders.is_empty()
            {
                return Err(rejected("external operator application is not closed"));
            }
            // Review asks only the float lane: a fixed-token integer
            // comparison that selected dispatch closes is still rejected
            // here. See `IntrinsicRequirement::closed_float_execution`.
            if operator.spelling.is_some()
                && (!matches!(binding, PackageReviewExternalBinding::CompilerIntrinsic)
                    || provider_planning::IntrinsicRequirement::from_operator(
                        compilation.checking_input,
                        operator,
                    )
                    .and_then(|requirement| {
                        requirement.closed_float_execution(compilation.checking_input)
                    })
                    .is_none())
            {
                return Err(rejected(
                    "fixed-token external operator has no closed intrinsic execution",
                ));
            }
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
