//! Keep the provider conditions needed when an actual saved pointer is borrowed.
//! Replaying one condition can expose an upstream captured provider. Revisit
//! the same normalized use inventory until every such dependency is retained;
//! receiving validation never performs this producer-side completion.

use crate::shared::lowering_error::LoweringError;

pub(super) fn retain(module: &mut terminal_psi::TerminalModule) -> Result<(), LoweringError> {
    let maximum = module
        .machines
        .iter()
        .map(|body| body.operations().count())
        .sum::<usize>()
        .saturating_add(
            module
                .boundary_reference_forwarding
                .iter()
                .map(|row| row.sources.len())
                .sum(),
        );
    for _ in 0..=maximum {
        let required =
            terminal_semantics::descriptor_results::formation_reference_forwarding_requirements(
                module,
            )
            .map_err(LoweringError::ReferenceForwardingCompletionRefused)?;
        if required == module.boundary_reference_forwarding {
            return Ok(());
        }
        module.boundary_reference_forwarding = required;
    }
    Err(LoweringError::ReferenceForwardingCompletionLimit { maximum })
}
