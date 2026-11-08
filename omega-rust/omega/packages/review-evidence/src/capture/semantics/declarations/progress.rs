/*
We resolve the closed progress-profile declaration a termination premise names
and the subject coordinates that premise certifies.

A termination guarantee is conditional on a closed progress profile, so every
premise resolves through this owner: an unknown, duplicated, or differently
classified domain rejects before any producer records a row. A premise subject
is either an entry parameter position or the nominal identity of the public
declaration its root names; private roots carry no review coordinate and are
rejected rather than exporting compiler-local handles.
*/

use super::nominal_identity;
use crate::capture::EvidenceOption;
use crate::capture::SourceDeclarationInput;
use crate::capture::contracts::expressions::names::portable_ordinal;
use crate::record::{PackageReviewNominalIdentity, PackageReviewProgressSubject};
use diagnostics::Diagnostic;
use language_semantics::{DomainClassification, ProgressPremise, SemanticDomainId};
use symbols::SymbolHandle;
use typed_trees::domain::DomainDefinition;
use typed_trees::signature::StateParameter;

pub(crate) fn progress_profile<'a>(
    compilation: &'a SourceDeclarationInput<'_>,
    profile: SemanticDomainId,
    subject: &str,
) -> Result<&'a DomainDefinition, Vec<Diagnostic>> {
    let mut matches = compilation
        .domain_definitions()
        .iter()
        .filter(|domain| domain.semantic_id == profile);
    let profile = matches
        .next()
        .or_evidence(format!("{subject} has an unknown termination profile"))?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "{subject} has a duplicated termination profile"
        ))]);
    }
    if profile.classification != Some(DomainClassification::ProgressProfile) {
        return Err(vec![Diagnostic::error(format!(
            "{subject} does not name a closed progress-profile domain"
        ))]);
    }
    Ok(profile)
}

pub(crate) fn premise_subject(
    compilation: &SourceDeclarationInput<'_>,
    parameters: &[StateParameter],
    root: SymbolHandle,
    subject: &str,
) -> Result<PackageReviewProgressSubject, Vec<Diagnostic>> {
    if let Some(position) = typed_trees::signature::state_parameter_position(parameters, root) {
        return portable_ordinal(position).map(PackageReviewProgressSubject::Parameter);
    }
    let is_public_declaration = compilation
        .data_definitions()
        .iter()
        .any(|data| data.symbol == root && data.is_public)
        || compilation
            .domain_definitions()
            .iter()
            .any(|domain| domain.symbol == root && domain.is_public)
        || compilation
            .machines()
            .iter()
            .any(|machine| machine.symbol == root && machine.is_public);
    if !is_public_declaration {
        return Err(vec![Diagnostic::error(format!(
            "{subject} has a termination premise rooted at neither a parameter nor a public declaration"
        ))]);
    }
    nominal_identity(compilation, root).map(PackageReviewProgressSubject::Declaration)
}

pub(crate) fn premise_projections(
    compilation: &SourceDeclarationInput<'_>,
    premise: &ProgressPremise,
) -> Result<Vec<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    premise
        .subject
        .projections
        .iter()
        .map(|symbol| nominal_identity(compilation, *symbol))
        .collect()
}
