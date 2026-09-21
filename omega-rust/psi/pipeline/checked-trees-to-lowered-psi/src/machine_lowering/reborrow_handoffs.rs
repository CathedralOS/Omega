//! Reborrow root-handoff admission over the selected source mapping.
//!
//! Reborrow custody belongs to every included source body, not only the
//! requested entry, so the lowering route must name each source's Terminal
//! machine exactly before root handoffs can be retained. The admission uses
//! the route's exact source mapping — ordinal correspondence is not evidence
//! of a callee's identity — and refuses closures whose owners are ambiguous
//! or do not resolve to one unique emitted machine.

use checked_trees::CheckedTrees;
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_psi::TerminalModule;

use crate::lowering_error::{LoweringError, unsupported};
use crate::retention::reborrow_root_handoff;

/// Admit the source-to-Terminal handoff pairs for the selected closure and
/// retain each source's reborrow root handoffs. `handoff_sources` is the
/// exact owner catalog, or the single entry pair when the route cannot name
/// callees exactly.
pub(crate) fn retain_admitted_reborrow_root_handoffs(
    checked: &CheckedTrees,
    source_machines: &[SymbolHandle],
    handoff_sources: &[(SymbolHandle, MachineId)],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    for source in source_machines {
        if checked
            .facts
            .borrow
            .reborrow_loan_resources
            .iter()
            .any(|(_, resource)| resource.machine_symbol == *source)
            && handoff_sources
                .iter()
                .filter(|(owner, _)| owner == source)
                .count()
                != 1
        {
            return unsupported("reborrow call closure has no exact source owner mapping");
        }
    }
    for (source, terminal) in handoff_sources {
        if module
            .machines
            .iter()
            .filter(|machine| machine.id == *terminal)
            .count()
            != 1
        {
            return unsupported("reborrow source owner has no unique Terminal machine");
        }
        reborrow_root_handoff::retain_selected_reborrow_root_handoffs(
            checked,
            *source,
            *terminal,
            &mut module.reborrow_root_handoffs,
        )?;
    }
    Ok(())
}
