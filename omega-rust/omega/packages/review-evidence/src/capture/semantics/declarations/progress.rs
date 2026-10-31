//! The progress-profile declaration a termination premise names, and the
//! subject coordinates that premise certifies.

use super::nominal_identity;
use crate::capture::EvidenceOption;
use crate::capture::PackageReviewInput;
use crate::capture::contracts::expressions::names::portable_ordinal;
use crate::record::{PackageReviewNominalIdentity, PackageReviewProgressSubject};
use diagnostics::Diagnostic;
use language_semantics::{DomainClassification, ProgressPremise, SemanticDomainId};
use symbols::SymbolHandle;
use typed_trees::domain::DomainDefinition;
use typed_trees::signature::StateParameter;

/// The exact closed progress-profile declaration a termination premise names.
///
/// A termination guarantee is conditional on a closed progress profile, so the
/// premise resolves through this owner everywhere it is projected: an unknown,
/// duplicated, or differently classified domain rejects before any producer
/// records a row. `subject` names the declaring signature in the diagnostic.
pub(crate) fn progress_profile<'a>(
    compilation: &'a PackageReviewInput<'_>,
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

/// A premise subject: an entry parameter position, else the identity of the
/// public declaration its root names. A private root carries no review
/// coordinate and rejects rather than exporting a compiler-local handle.
pub(crate) fn premise_subject(
    compilation: &PackageReviewInput<'_>,
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

/// The declaration identities a premise subject projects through.
pub(crate) fn premise_projections(
    compilation: &PackageReviewInput<'_>,
    premise: &ProgressPremise,
) -> Result<Vec<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    premise
        .subject
        .projections
        .iter()
        .map(|symbol| nominal_identity(compilation, *symbol))
        .collect()
}
