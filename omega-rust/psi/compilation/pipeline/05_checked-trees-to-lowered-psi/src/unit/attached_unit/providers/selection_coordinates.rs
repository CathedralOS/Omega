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
