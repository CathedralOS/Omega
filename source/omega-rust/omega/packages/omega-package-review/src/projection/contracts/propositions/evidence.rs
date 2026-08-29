use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

use crate::evidence::{
    PackageReviewEvidenceInterface, PackageReviewEvidenceRequirement, PackageReviewNominalIdentity,
    PackageReviewTypeIdentity,
};
use crate::projection::checked_semantics::declarations::{
    nominal_identity, trait_requirement_identity,
};
use crate::projection::checked_semantics::types::lifetimes::lifetime_binder_ordinal;
use crate::projection::checked_semantics::types::{
    review_signature_type_identity_with_binders_and_substitutions_and_lifetimes,
    review_type_identity_with_binders_and_substitutions,
};

pub(crate) fn project_evidence_interface(
    compilation: &CheckedCompilation,
    evidence: psi_typed_trees::types::TypeReferenceHandle,
    proposition_binders: &[(SymbolHandle, String)],
) -> Result<PackageReviewEvidenceInterface, Vec<Diagnostic>> {
    use psi_typed_trees::types::TypeReferenceNode;

    let (trait_symbol, arguments) = match compilation.type_reference_table.type_reference(evidence)
    {
        TypeReferenceNode::Named { symbol, .. } => (*symbol, Vec::new()),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => (
            *base_symbol,
            compilation
                .type_reference_table
                .type_reference_handles(*arguments)
                .to_vec(),
        ),
        _ => {
            return Err(vec![Diagnostic::error(
                "reviewed witness proposition uses a non-nominal evidence interface",
            )]);
        }
    };
    let definition = compilation
        .traits()
        .iter()
        .find(|candidate| candidate.symbol == trait_symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "reviewed witness proposition has an unresolved evidence trait",
            )]
        })?;
    if !definition.lifetime_parameters.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed witness proposition uses lifetime-parameterized evidence trait `{}` without retained lifetime arguments",
            definition.name
        ))]);
    }
    let projected_arguments = arguments
        .iter()
        .map(|argument| {
            review_type_identity_with_binders_and_substitutions(
                compilation,
                *argument,
                proposition_binders,
                &[],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut requirements = Vec::new();
    collect_evidence_requirements(
        compilation,
        trait_symbol,
        &arguments,
        &[],
        proposition_binders,
        None,
        &[],
        &mut Vec::new(),
        &mut requirements,
    )?;
    requirements.sort();
    requirements.dedup();
    Ok(PackageReviewEvidenceInterface {
        trait_identity: nominal_identity(compilation, trait_symbol)?,
        lifetime_arguments: Vec::new(),
        arguments: projected_arguments,
        requirements,
    })
}

pub(crate) fn collect_evidence_requirements(
    compilation: &CheckedCompilation,
    trait_symbol: SymbolHandle,
    trait_arguments: &[psi_typed_trees::types::TypeReferenceHandle],
    trait_lifetime_arguments: &[psi_typed_trees::name::Identifier],
    proposition_binders: &[(SymbolHandle, String)],
    lifetime_binders: Option<&[psi_typed_trees::name::Identifier]>,
    inherited_substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    visited: &mut Vec<(
        PackageReviewNominalIdentity,
        Vec<u32>,
        Vec<PackageReviewTypeIdentity>,
    )>,
    requirements: &mut Vec<PackageReviewEvidenceRequirement>,
) -> Result<(), Vec<Diagnostic>> {
    let definition = compilation
        .traits()
        .iter()
        .find(|candidate| candidate.symbol == trait_symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "reviewed evidence interface inherits an unresolved trait",
            )]
        })?;
    if definition.lifetime_parameters.len() != trait_lifetime_arguments.len() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed evidence interface trait `{}` has {} lifetime parameter(s), but its checked application retains {}",
            definition.name,
            definition.lifetime_parameters.len(),
            trait_lifetime_arguments.len(),
        ))]);
    }
    let lifetime_argument_ordinals = match lifetime_binders {
        Some(lifetime_binders) => trait_lifetime_arguments
            .iter()
            .map(|argument| {
                lifetime_binder_ordinal(argument, lifetime_binders, "evidence trait application")
            })
            .collect::<Result<Vec<_>, _>>()?,
        None if trait_lifetime_arguments.is_empty() => Vec::new(),
        None => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed evidence interface trait `{}` has lifetime arguments outside a retained lifetime telescope",
                definition.name
            ))]);
        }
    };
    let type_parameters = compilation.trait_type_parameters(definition);
    if type_parameters.len() != trait_arguments.len() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed evidence trait `{}` has inconsistent instantiated arity",
            definition.name
        ))]);
    }
    if type_parameters.iter().any(|parameter| {
        !matches!(
            &parameter.kind,
            psi_typed_trees::data::TypeParameterKind::Type
                | psi_typed_trees::data::TypeParameterKind::Const { .. }
                | psi_typed_trees::data::TypeParameterKind::Machine {
                    contract: psi_typed_trees::data::MachineParameterContract::RequirementIdentity
                }
        )
    }) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed evidence trait `{}` uses a structural/nominal machine or proposition parameter not represented by package review",
            definition.name
        ))]);
    }
    let argument_identities = trait_arguments
        .iter()
        .map(|argument| match lifetime_binders {
            Some(lifetime_binders) => {
                review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
                    compilation,
                    *argument,
                    proposition_binders,
                    lifetime_binders,
                    inherited_substitutions,
                    &definition
                        .lifetime_parameters
                        .iter()
                        .cloned()
                        .zip(trait_lifetime_arguments.iter().cloned())
                        .collect::<Vec<_>>(),
                )
            }
            None => review_type_identity_with_binders_and_substitutions(
                compilation,
                *argument,
                proposition_binders,
                inherited_substitutions,
            ),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let visit = (
        nominal_identity(compilation, trait_symbol)?,
        lifetime_argument_ordinals.clone(),
        argument_identities.clone(),
    );
    if visited.contains(&visit) {
        return Ok(());
    }
    visited.push(visit);

    for requirement in compilation.trait_machine_signatures(definition) {
        requirements.push(PackageReviewEvidenceRequirement {
            declaring_trait: nominal_identity(compilation, trait_symbol)?,
            declaring_trait_lifetime_arguments: lifetime_argument_ordinals.clone(),
            declaring_trait_arguments: argument_identities.clone(),
            requirement: trait_requirement_identity(compilation, definition, requirement)?,
        });
    }

    let mut substitutions = inherited_substitutions.to_vec();
    substitutions.extend(
        type_parameters
            .iter()
            .zip(trait_arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument)),
    );
    for parent in compilation.trait_requirements(definition) {
        let parent_lifetime_arguments = parent
            .lifetime_arguments
            .iter()
            .map(|argument| {
                let Some(ordinal) = definition
                    .lifetime_parameters
                    .iter()
                    .position(|parameter| parameter == argument)
                else {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed evidence trait `{}` parent refers to undeclared lifetime `'{}'",
                        definition.name,
                        argument.as_str(),
                    ))]);
                };
                trait_lifetime_arguments.get(ordinal).cloned().ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed evidence trait `{}` parent lifetime substitution is incomplete",
                        definition.name
                    ))]
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let parent_arguments = compilation
            .type_reference_table
            .type_reference_handles(parent.arguments);
        collect_evidence_requirements(
            compilation,
            parent.symbol,
            parent_arguments,
            &parent_lifetime_arguments,
            proposition_binders,
            lifetime_binders,
            &substitutions,
            visited,
            requirements,
        )?;
    }
    Ok(())
}
