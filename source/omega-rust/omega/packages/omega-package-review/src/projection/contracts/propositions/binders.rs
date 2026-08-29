use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

use crate::evidence::{
    PackageReviewContractExpression, PackageReviewContractKind,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionBinderValue,
    PackageReviewTypeIdentity,
};
use crate::projection::checked_semantics::declarations::{
    nominal_identity, trait_requirement_identity_from_symbols,
};
use crate::projection::checked_semantics::types::missing_exact_toolchain_type_owner;
use crate::projection::contracts::checked::facts::ContractProjectionContext;
use crate::projection::contracts::expressions::names::portable_parameter_position;
use crate::projection::contracts::propositions::evidence::project_evidence_interface;

pub(crate) fn project_proposition_binder_argument(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    callable_binders: &[(SymbolHandle, String)],
    argument: &psi_typed_trees::proposition::PropositionBinderArgument,
    substitutions: &[(SymbolHandle, PackageReviewPropositionBinderArgument)],
) -> Result<PackageReviewPropositionBinderArgument, Vec<Diagnostic>> {
    if let Some((_, substitution)) = substitutions
        .iter()
        .rev()
        .find(|(symbol, _)| *symbol == argument.symbol)
    {
        if substitution.kind != argument.kind {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` proposition binder substitution changes kind",
                context.subject_kind, context.subject_name
            ))]);
        }
        return Ok(substitution.clone());
    }
    let value = if let Some(projection) = argument.evidence_projection.as_ref() {
        project_proposition_evidence_projection(compilation, context, projection)?
    } else if let Some(literal) = &argument.const_literal {
        PackageReviewPropositionBinderValue::Integer(literal.text().to_owned())
    } else if let Some(position) = callable_binders
        .iter()
        .position(|(symbol, _)| *symbol == argument.symbol)
    {
        PackageReviewPropositionBinderValue::GenericBinder(portable_parameter_position(position)?)
    } else if argument.symbol.is_valid() {
        match argument.kind {
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Type => {
                let identity = compilation
                    .package_qualified_nominal_type_identity_with_toolchain_sources(
                        argument.symbol,
                        compilation.exact_toolchain_sources(),
                    )
                    .ok_or_else(missing_exact_toolchain_type_owner)?;
                PackageReviewPropositionBinderValue::Type(PackageReviewTypeIdentity {
                    canonical: identity.into_string(),
                })
            }
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine => {
                PackageReviewPropositionBinderValue::Machine(nominal_identity(
                    compilation,
                    argument.symbol,
                )?)
            }
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Const => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` proposition contains a non-literal const binder argument without an exact caller binder",
                    context.subject_kind, context.subject_name
                ))]);
            }
        }
    } else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition contains an unresolved binder argument",
            context.subject_kind, context.subject_name
        ))]);
    };
    Ok(PackageReviewPropositionBinderArgument {
        kind: argument.kind,
        value,
    })
}

pub(crate) fn project_proposition_evidence_projection(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    projection: &psi_typed_trees::expression::EvidenceProjection,
) -> Result<PackageReviewPropositionBinderValue, Vec<Diagnostic>> {
    let matching_terms = compilation
        .facts
        .proof
        .evidence_terms
        .iter()
        .filter_map(|(handle, term)| {
            (term.owner == context.owner
                && term.kind == psi_checked_trees::ContractProofFactKind::Requires
                && term.name == projection.term.as_str())
            .then_some((handle, term))
        })
        .collect::<Vec<_>>();
    let [(term_handle, term)] = matching_terms.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` resolves to {} checked source terms; expected one",
            context.subject_kind,
            context.subject_name,
            projection.term,
            projection.member,
            matching_terms.len()
        ))]);
    };
    let Some(checked_interface) = term.evidence_interface.as_ref() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` has no exact checked source interface",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    };
    let matching_requirements = checked_interface
        .requirements
        .iter()
        .filter(|requirement| {
            compilation.symbols.name(requirement.requirement) == projection.member.as_str()
        })
        .collect::<Vec<_>>();
    let [checked_requirement] = matching_requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` resolves to {} checked requirement rows; expected one",
            context.subject_kind,
            context.subject_name,
            projection.term,
            projection.member,
            matching_requirements.len()
        ))]);
    };
    if !compilation
        .facts
        .proof
        .proposition_vocabulary
        .applications
        .iter()
        .flat_map(|application| &application.binder_arguments)
        .filter_map(|argument| argument.evidence_projection.as_ref())
        .any(|retained| {
            retained.term == *term_handle
                && retained.declaring_trait == checked_requirement.declaring_trait
                && retained.requirement == checked_requirement.requirement
        })
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` has no retained checked projection row",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    }

    let declaration = compilation
        .propositions()
        .iter()
        .find(|candidate| candidate.symbol == term.proposition.declaration)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed {} `{}` evidence projection `{}.{}` has an unresolved source proposition endpoint",
                context.subject_kind, context.subject_name, projection.term, projection.member
            ))]
        })?;
    let psi_typed_trees::proposition::PropositionBody::Witness { evidence } = &declaration.body
    else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` does not originate from witness evidence",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    };
    let proposition_binders = compilation
        .proposition_binders(declaration)
        .iter()
        .enumerate()
        .map(|(position, binder)| (binder.symbol, format!("proposition-binder:{position}")))
        .collect::<Vec<_>>();
    let interface = project_evidence_interface(compilation, *evidence, &proposition_binders)?;
    if nominal_identity(compilation, checked_interface.trait_symbol)? != interface.trait_identity {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` changed source interface during checked lowering",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    }
    let declaring_trait = nominal_identity(compilation, checked_requirement.declaring_trait)?;
    let requirement = trait_requirement_identity_from_symbols(
        compilation,
        checked_requirement.declaring_trait,
        checked_requirement.requirement,
        "checked evidence projection",
    )?;
    let matching_projected = interface
        .requirements
        .iter()
        .filter(|candidate| {
            candidate.declaring_trait == declaring_trait && candidate.requirement == requirement
        })
        .collect::<Vec<_>>();
    let [projected_requirement] = matching_projected.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` resolves to {} structural interface rows; expected one",
            context.subject_kind,
            context.subject_name,
            projection.term,
            projection.member,
            matching_projected.len()
        ))]);
    };
    Ok(PackageReviewPropositionBinderValue::EvidenceProjection {
        source_kind: PackageReviewContractKind::Requires,
        source_lane_position: portable_parameter_position(term.lane_position)?,
        declaring_trait,
        declaring_trait_arguments: projected_requirement.declaring_trait_arguments.clone(),
        requirement,
    })
}

pub(crate) fn proposition_binder_value_expression(
    argument: &PackageReviewPropositionBinderArgument,
) -> Option<PackageReviewContractExpression> {
    match &argument.value {
        PackageReviewPropositionBinderValue::Machine(identity) => {
            Some(PackageReviewContractExpression::Nominal(identity.clone()))
        }
        PackageReviewPropositionBinderValue::Type(_) => None,
        PackageReviewPropositionBinderValue::GenericBinder(position) => {
            Some(PackageReviewContractExpression::GenericBinder(*position))
        }
        PackageReviewPropositionBinderValue::Integer(value) => {
            Some(PackageReviewContractExpression::Integer(value.clone()))
        }
        PackageReviewPropositionBinderValue::EvidenceProjection { .. } => None,
    }
}
