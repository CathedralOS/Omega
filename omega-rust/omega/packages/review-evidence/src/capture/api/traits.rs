use super::super::behavior::{
    project_crash_routes, project_service_row, project_synchronous_invocations,
    project_trait_requirement_termination,
};
use super::super::contracts::facts::{
    ContractProjectionContext, project_trait_requirement_contracts,
};
use super::super::semantics::conformances::project_conformance_bounds;
use super::super::semantics::declarations::{
    is_product_scope_instance, nominal_identity, reviewed_package_owns, trait_requirement_identity,
};
use super::super::semantics::signatures::parameters::{
    project_type_parameters, project_type_parameters_after,
};
use super::super::semantics::types::lifetimes::lifetime_binder_ordinal;
use super::super::semantics::types::review_signature_type_identity_with_binders;
use crate::capture::PackageReviewInput;
use crate::capture::source::ProjectedReviewRow;
use crate::record::{
    PackageReviewSignatureParameter, PackageReviewTraitCompositionKind, PackageReviewTraitParent,
    PackageReviewTraitRequirement, PackageReviewTraitShape,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;
use typed_trees::trait_definition::TraitDefinition;

pub(crate) fn project_public_traits<'a>(
    compilation: &'a PackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<'a, PackageReviewTraitShape, TraitDefinition>>, Vec<Diagnostic>>
{
    let mut rows = Vec::new();
    for definition in compilation.traits().iter().filter(|row| {
        row.is_public && is_product_scope_instance(&compilation.typed.symbols, row.symbol)
    }) {
        let identity = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let parameters = compilation.trait_type_parameters(definition);
        let (mut trait_binders, type_parameters) = project_type_parameters(
            compilation,
            parameters,
            "trait",
            &identity.path,
            &definition.lifetime_parameters,
        )?;
        trait_binders.insert(0, (definition.symbol, "trait-self".to_owned()));
        let conformance_bounds = project_conformance_bounds(
            compilation,
            &definition.conformance_bounds,
            &trait_binders,
            &definition.lifetime_parameters,
            "public trait",
            &identity.path,
        )?;
        let parents = compilation
            .trait_requirements(definition)
            .iter()
            .map(|parent| {
                project_trait_parent(
                    compilation,
                    parent,
                    &trait_binders,
                    &definition.lifetime_parameters,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let requirements = compilation
            .trait_machine_signatures(definition)
            .iter()
            .map(|requirement| {
                project_trait_requirement(
                    compilation,
                    definition.symbol,
                    requirement,
                    &trait_binders,
                    parameters.len(),
                    &definition.lifetime_parameters,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        rows.push(ProjectedReviewRow {
            row: PackageReviewTraitShape {
                identity,
                is_boundary: definition.is_boundary,
                lifetime_parameter_count: definition.lifetime_parameters.len(),
                type_parameters,
                conformance_bounds,
                parents,
                requirements,
            },
            declaration: definition,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

pub(crate) fn project_trait_parent(
    compilation: &PackageReviewInput<'_>,
    parent: &typed_trees::trait_definition::TraitRequirement,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[typed_trees::name::Identifier],
) -> Result<PackageReviewTraitParent, Vec<Diagnostic>> {
    let matches = compilation
        .traits()
        .iter()
        .filter(|candidate| candidate.symbol == parent.symbol)
        .collect::<Vec<_>>();
    let [definition] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "package review trait parent resolves to {} declarations; expected exactly one",
            matches.len()
        ))]);
    };
    if parent.lifetime_arguments.len() != definition.lifetime_parameters.len() {
        return Err(vec![Diagnostic::error(format!(
            "package review trait parent `{}` has {} resolved lifetime arguments; expected {}",
            parent.name,
            parent.lifetime_arguments.len(),
            definition.lifetime_parameters.len(),
        ))]);
    }
    Ok(PackageReviewTraitParent {
        kind: if definition.is_boundary {
            PackageReviewTraitCompositionKind::ServiceReach
        } else {
            PackageReviewTraitCompositionKind::Policy
        },
        identity: nominal_identity(compilation, definition.symbol)?,
        lifetime_arguments: parent
            .lifetime_arguments
            .iter()
            .map(|argument| lifetime_binder_ordinal(argument, lifetime_binders, "trait parent"))
            .collect::<Result<Vec<_>, _>>()?,
        arguments: project_parent_arguments(
            compilation,
            definition,
            parent,
            binders,
            lifetime_binders,
        )?,
    })
}

pub(crate) fn project_trait_requirement(
    compilation: &PackageReviewInput<'_>,
    trait_symbol: SymbolHandle,
    requirement: &typed_trees::signature::StateSignature,
    trait_binders: &[(SymbolHandle, String)],
    trait_parameter_count: usize,
    trait_lifetime_parameters: &[typed_trees::name::Identifier],
) -> Result<PackageReviewTraitRequirement, Vec<Diagnostic>> {
    let owner = compilation
        .trait_definition_by_symbol(trait_symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "package review trait requirement has no exact declaring trait",
            )]
        })?;
    let identity = trait_requirement_identity(compilation, owner, requirement)?;
    let parameters = compilation.state_signature_type_parameters(requirement);
    let mut lifetime_binders = trait_lifetime_parameters.to_vec();
    lifetime_binders.extend(requirement.lifetime_parameters.iter().cloned());
    let (binders, type_parameters) = project_type_parameters_after(
        compilation,
        parameters,
        "trait requirement",
        &identity.path,
        trait_binders,
        trait_parameter_count,
        &lifetime_binders,
        0,
    )?;
    // Preserve the more specific public-progress validation before the same
    // membership facts enter the general contract lane.
    let termination = project_trait_requirement_termination(compilation, requirement)?;
    let contract_parameters = compilation.state_signature_parameters(requirement);
    let contract_context = ContractProjectionContext {
        subject_kind: "public trait requirement",
        subject_name: &identity.path,
        owner: checked_trees::ContractProofFactOwner::StateSignature {
            owner_symbol: trait_symbol,
            state_symbol: requirement.symbol,
        },
        point: facts::ProgramPoint::State {
            machine_symbol: trait_symbol,
            state_symbol: requirement.symbol,
        },
        parameters: contract_parameters,
        domain_symbol: None,
        data_symbol: None,
        lifetime_binders: &lifetime_binders,
        lifetime_substitutions: &[],
        selection_exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
    };
    let contracts =
        project_trait_requirement_contracts(compilation, requirement, &contract_context, &binders)?;
    let mut crash_capsules = compilation
        .facts
        .contract_plans
        .crash_capsules
        .iter()
        .filter(|capsule| {
            capsule.target_machine() == trait_symbol && capsule.target_state() == requirement.symbol
        });
    let crash_capsule = crash_capsules.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "public trait requirement `{}` has no exact checked crash capsule",
            identity.path
        ))]
    })?;
    if crash_capsules.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "public trait requirement `{}` has duplicate checked crash capsules",
            identity.path
        ))]);
    }
    let published_crash = project_crash_routes(crash_capsule.published_buckets());
    Ok(PackageReviewTraitRequirement {
        identity,
        spelling: requirement.spelling,
        has_default_realization: requirement.is_default,
        lifetime_parameter_count: requirement.lifetime_parameters.len(),
        type_parameters,
        parameters: compilation
            .state_signature_parameters(requirement)
            .iter()
            .map(|parameter| {
                Ok(PackageReviewSignatureParameter {
                    name: parameter.name.as_str().to_owned(),
                    type_identity: review_signature_type_identity_with_binders(
                        compilation,
                        parameter.type_reference,
                        &binders,
                        &lifetime_binders,
                    )?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
        return_type: review_signature_type_identity_with_binders(
            compilation,
            requirement.return_type,
            &binders,
            &lifetime_binders,
        )?,
        contracts,
        published_crash,
        service_reach: project_service_row(compilation, requirement.service_reach_row)?,
        service_reach_is_installation_bound: requirement.service_reach_is_installation_bound,
        synchronous_invocations: project_synchronous_invocations(
            compilation,
            &validation::declared_signature_invocations(compilation.checking_input, requirement),
        )?,
        suspends: requirement.suspends,
        blocks: requirement.blocks,
        termination,
    })
}

/// A named implementation is code with its own telescope, not a data type.
/// Reclose the source selection before projecting its exact checked arguments.
fn project_parent_arguments(
    compilation: &PackageReviewInput<'_>,
    definition: &typed_trees::trait_definition::TraitDefinition,
    parent: &typed_trees::trait_definition::TraitRequirement,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[typed_trees::name::Identifier],
) -> Result<Vec<crate::record::PackageReviewTypeIdentity>, Vec<Diagnostic>> {
    use typed_trees::data::TypeParameterKind;
    use typed_trees::types::TypeReferenceNode;
    let arguments = compilation
        .type_reference_table
        .type_reference_handles(parent.arguments);
    let parameters = compilation.trait_type_parameters(definition);
    if arguments.len() != parameters.len() {
        return Err(vec![Diagnostic::error(
            "reviewed trait parent has stale static telescope arity",
        )]);
    }
    arguments.iter().zip(parameters).map(|(reference, parameter)| {
        if !matches!(parameter.kind, TypeParameterKind::Implementation) {
            return review_signature_type_identity_with_binders(compilation, *reference, binders, lifetime_binders);
        }
        let operand = compilation.static_operand_from_type_reference(*reference)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        // Caller-owned implementation binders retain their ordinal identity.
        if operand.symbol.is_valid() && compilation.symbols.get(operand.symbol).kind == symbols::SymbolKind::ConformanceParameter {
            return review_signature_type_identity_with_binders(compilation, *reference, binders, lifetime_binders);
        }
        let closed = validation::conformance_applications::close_conformance_application(compilation.checking_input, &operand)
            .map_err(|diagnostic| vec![diagnostic])?;
        let declaration = compilation.conformance_by_symbol(closed.declaration).ok_or_else(|| vec![Diagnostic::error("reviewed trait parent has no exact implementation declaration")])?;
        let selected_parameters = compilation.conformance_type_parameters(declaration);
        if selected_parameters.len() != closed.arguments.len() {
            return Err(vec![Diagnostic::error("reviewed trait parent implementation has stale telescope arity")]);
        }
        let mut projection = validation::CheckingProjection::new(compilation.checking_input);
        let children = selected_parameters.iter().zip(&closed.arguments).map(|(parameter, argument)| {
            crate::capture::semantics::conformances::argument_type_reference(projection.program_mut(), argument, &parameter.kind, 0)
        }).collect::<Result<Vec<_>, _>>()?;
        let arguments = projection.program_mut().type_reference_table.insert_type_reference_handles(children);
        let reference = projection.program_mut().type_reference_table.insert(TypeReferenceNode::Generic {
            base_symbol: declaration.symbol,
            base_name: declaration.alias.clone().ok_or_else(|| vec![Diagnostic::error("reviewed trait parent has an unnamed implementation")])?,
            lifetime_arguments: closed.lifetime_arguments.iter().map(typed_trees::name::Identifier::generated).collect(),
            arguments,
        });
        crate::capture::semantics::types::signature_type_identity(
            compilation, &projection, compilation.custody.sources().exact_toolchain_sources(), reference, binders, lifetime_binders, &[], &[], false,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
        )
    }).collect()
}
