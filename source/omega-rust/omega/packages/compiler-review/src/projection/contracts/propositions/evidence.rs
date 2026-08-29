use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

use crate::evidence::{
    PackageReviewEvidenceInterface, PackageReviewEvidenceRequirement, PackageReviewNominalIdentity,
    PackageReviewTypeIdentity,
};
use crate::projection::exact_identity::{
    nominal_identity, review_signature_type_identity_with_binders_and_substitutions,
    review_type_identity_with_binders_and_substitutions, trait_requirement_identity,
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
        arguments: projected_arguments,
        requirements,
    })
}

pub(crate) fn collect_evidence_requirements(
    compilation: &CheckedCompilation,
    trait_symbol: SymbolHandle,
    trait_arguments: &[psi_typed_trees::types::TypeReferenceHandle],
    proposition_binders: &[(SymbolHandle, String)],
    lifetime_binders: Option<&[psi_typed_trees::name::Identifier]>,
    inherited_substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    visited: &mut Vec<(PackageReviewNominalIdentity, Vec<PackageReviewTypeIdentity>)>,
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
    if !definition.lifetime_parameters.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed evidence interface inherits lifetime-parameterized trait `{}`",
            definition.name
        ))]);
    }
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
                review_signature_type_identity_with_binders_and_substitutions(
                    compilation,
                    *argument,
                    proposition_binders,
                    lifetime_binders,
                    inherited_substitutions,
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
        argument_identities.clone(),
    );
    if visited.contains(&visit) {
        return Ok(());
    }
    visited.push(visit);

    for requirement in compilation.trait_machine_signatures(definition) {
        requirements.push(PackageReviewEvidenceRequirement {
            declaring_trait: nominal_identity(compilation, trait_symbol)?,
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
        if !parent.lifetime_arguments.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed evidence trait `{}` has a parent with lifetime arguments not yet represented by package review",
                definition.name
            ))]);
        }
        let parent_arguments = compilation
            .type_reference_table
            .type_reference_handles(parent.arguments);
        collect_evidence_requirements(
            compilation,
            parent.symbol,
            parent_arguments,
            proposition_binders,
            lifetime_binders,
            &substitutions,
            visited,
            requirements,
        )?;
    }
    Ok(())
}
