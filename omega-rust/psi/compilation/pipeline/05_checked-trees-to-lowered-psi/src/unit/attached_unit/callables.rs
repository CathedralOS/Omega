//! Rejoin exact source declarations to emitted executable or boundary identities.

use super::{BoundaryParameters, LoweringError, unsupported};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_psi::TerminalCallable;

pub(super) fn callable(
    source: SymbolHandle,
    machines: &[(SymbolHandle, MachineId)],
    boundaries: &[BoundaryParameters],
) -> Result<TerminalCallable, LoweringError> {
    let mut owners = machines
        .iter()
        .filter(|(symbol, _)| *symbol == source)
        .map(|(_, identity)| TerminalCallable::Machine(*identity))
        .chain(
            boundaries
                .iter()
                .filter(|boundary| boundary.source == source)
                .map(|boundary| TerminalCallable::Boundary(boundary.id)),
        );
    let owner = owners.next().ok_or(LoweringError::Unsupported(
        "source declaration has no emitted callable",
    ))?;
    if owners.next().is_some() {
        return unsupported("source declaration has multiple emitted callables");
    }
    Ok(owner)
}
