//! Retain a candidate's original selected row beside its executable coordinates.
//!
//! Omega plans select authored callable rows, while the emitted catalog names
//! their closed applications. Keeping the exact association here lets a receiver
//! enumerate every eligible instance, including uncalled tuples, without trusting
//! a proposal's supplied subset. Immediate specialization templates retain any
//! outer application already present; walking to a lexical ancestor would erase
//! that application. Checking owns tuple admission and provider conformance.

use super::{CheckedTrees, CheckedUnitProviderCandidate, LoweringError};
use terminal_psi::{TerminalIdentityIndex, TerminalIdentityTableBuilder};

/// Publish the already checked receiving association, never recompute provider
/// eligibility from an erased signature. The original requirement application
/// supplies the portable key; the selecting declaration owns region-slot order.
pub(super) fn intern_receiving_application(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    candidate: &CheckedUnitProviderCandidate,
) -> Result<(TerminalIdentityIndex, Vec<TerminalIdentityIndex>), LoweringError> {
    let typed = &checked.typed;
    let Some(application) = typed.closed_requirement_application(candidate.boundary) else {
        return Ok((TerminalIdentityIndex::default(), Vec::new()));
    };
    let selecting_declaration = if typed.conformance_by_symbol(candidate.provider).is_some() {
        candidate.provider
    } else {
        // Direct machine satisfaction is exported under its attached data
        // owner, but its checked satisfaction row belongs to the exact body.
        candidate.candidate
    };
    let mut rows = checked
        .facts
        .provider_closed_requirements
        .iter()
        .filter(|row| {
            row.provider == candidate.candidate
                && row.selecting_declaration == selecting_declaration
                && row.template == application.template
                && row.closed_requirement == candidate.boundary
                && row.receiving_application == application.trait_application_identity
        });
    let row = rows.next().ok_or(LoweringError::Unsupported(
        "provider candidate lost its checked receiving application",
    ))?;
    if rows.next().is_some() {
        return Err(LoweringError::Unsupported(
            "provider candidate has multiple checked receiving associations",
        ));
    }
    let publication = &checked.facts.provider_instance_publication;
    let receiving = publication
        .receiving(row)
        .map_err(|reason| LoweringError::InvalidBoundaryApplication(reason.to_owned()))?;
    intern_receiving_description(identities, publication, receiving)
}

/// Both an emitted candidate and the independent obligation roster consume
/// the checker's original receiving association, never an erased signature.
pub(super) fn intern_prepared_receiving_application(
    identities: &mut TerminalIdentityTableBuilder,
    publication: &checked_trees::CheckedProviderInstancePublication,
    receiving: arena::Handle<checked_trees::CheckedProviderReceivingApplication>,
) -> Result<(TerminalIdentityIndex, Vec<TerminalIdentityIndex>), LoweringError> {
    if !receiving.is_valid() {
        return Ok((TerminalIdentityIndex::default(), Vec::new()));
    }
    let receiving = publication.receivers.get(receiving);
    let receiving = receiving
        .description
        .as_ref()
        .map_err(|reason| LoweringError::InvalidBoundaryApplication(reason.clone()))?;
    intern_receiving_description(identities, publication, receiving)
}

fn intern_receiving_description(
    identities: &mut TerminalIdentityTableBuilder,
    publication: &checked_trees::CheckedProviderInstancePublication,
    receiving: &checked_trees::CheckedProviderReceivingDescription,
) -> Result<(TerminalIdentityIndex, Vec<TerminalIdentityIndex>), LoweringError> {
    let regions = publication.regions.span(receiving.regions).ok_or_else(|| {
        LoweringError::InvalidBoundaryApplication(
            "provider lost its checked receiving region publication".into(),
        )
    })?;
    Ok((
        identities.intern(&receiving.application),
        regions
            .iter()
            .map(|region| identities.intern(region))
            .collect(),
    ))
}

pub(super) fn intern(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    candidate: &CheckedUnitProviderCandidate,
) -> Result<
    (
        TerminalIdentityIndex,
        TerminalIdentityIndex,
        Option<semantic_vocabulary::PackageKeyIdentity>,
    ),
    LoweringError,
> {
    let typed = &checked.typed;
    let requirement_template = immediate_template(checked, candidate.boundary)?;
    let requirement = if let Some(application) =
        typed.closed_requirement_application(candidate.boundary)
    {
        let (owner, signature) = typed.trait_machine_signature(application.template).ok_or(
            LoweringError::Unsupported("closed provider requirement lost its original signature"),
        )?;
        typed.trait_requirement_callable_identity_text(owner, signature)
    } else if requirement_template.is_valid() {
        let machine = typed
            .machine(requirement_template)
            .ok_or(LoweringError::Unsupported(
                "closed provider requirement lost its specialization template",
            ))?;
        typed
            .machine_callable_identity_text(machine)
            .ok_or(LoweringError::Unsupported(
                "provider requirement template has no callable identity",
            ))?
    } else {
        typed.callable_identity_text(candidate.requirement_identity)
    };
    let realization_template = immediate_template(checked, candidate.candidate)?;
    let realization_symbol = if realization_template.is_valid() {
        realization_template
    } else {
        candidate.candidate
    };
    let machine = typed
        .machine(realization_symbol)
        .ok_or(LoweringError::Unsupported(
            "closed provider realization lost its specialization template",
        ))?;
    let realization =
        typed
            .machine_callable_identity_text(machine)
            .ok_or(LoweringError::Unsupported(
                "provider realization template has no callable identity",
            ))?;
    if requirement.is_empty() || realization.is_empty() {
        return Err(LoweringError::Unsupported(
            "provider selection coordinate has an empty original callable identity",
        ));
    }
    Ok((
        identities.intern(&requirement),
        identities.intern(&realization),
        typed.symbols.symbol_package_identity(realization_symbol),
    ))
}

fn immediate_template(
    checked: &CheckedTrees,
    instance: symbols::SymbolHandle,
) -> Result<symbols::SymbolHandle, LoweringError> {
    let mut receipts = checked
        .typed
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.instance == instance);
    let template = receipts
        .next()
        .map_or(symbols::SymbolHandle::default(), |receipt| receipt.template);
    if receipts.next().is_some() {
        return Err(LoweringError::Unsupported(
            "provider callable has more than one specialization receipt",
        ));
    }
    Ok(template)
}
