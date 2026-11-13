//! Retain the checked program for reports, or check an explicitly named
//! inspection entry through the same publication path as an application.

use diagnostics::Diagnostic;
use provider_planning::PublishedProviderSource;
use psi_compiler::PortableTerminalProgram;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) struct ProductCodeSource<'a> {
    pub program: &'a Arc<PublishedProviderSource>,
}

impl ProductCodeSource<'_> {
    pub(crate) fn inspect(
        &self,
        name: &str,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        let typed = self.program.typed();
        // The CLI names a declaration by its qualified source spelling.
        // Resolve it once here; downstream identities come from publication.
        let mut matching = typed
            .machines()
            .iter()
            .filter(|machine| typed.symbols.display_path(machine.symbol, "::") == name);
        let entry = matching.next().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "inspection machine {name} was not found"
            ))]
        })?;
        if matching.next().is_some() {
            return Err(vec![Diagnostic::error(format!(
                "inspection machine {name} is ambiguous",
            ))]);
        }
        // Entry selection changes the module identity and its evidence seals.
        // Recheck the chosen entry, retaining the original declared obligations,
        // instead of editing the already accepted artifact's entry field.
        let inspected =
            crate::checked::checking::publish_selected_roots(self.program, &[entry.symbol], &[])?;
        ProductCodeSource {
            program: &inspected,
        }
        .portable()
        .map(Arc::new)
    }

    pub(crate) fn portable(&self) -> Result<PortableTerminalProgram, Vec<Diagnostic>> {
        let typed = self.program.typed();
        let declarations = self.program.correspondence();
        let module = self.program.module();
        let mut provenance = terminal_psi::TerminalMachineProvenance::default();
        for machine in &module.machines {
            let symbol = *declarations.machines.get(machine.id).ok_or_else(|| {
                vec![Diagnostic::error(
                    "inspection lost a checked machine's source declaration",
                )]
            })?;
            provenance.name(
                machine.id,
                typed.symbols.display_path(symbol, "::"),
                typed.symbols.symbol_source_span(symbol),
            );
        }
        if let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry {
            provenance = provenance.for_root(module, entry, None);
        }
        PortableTerminalProgram::from_checked_artifact(
            Arc::clone(self.program.artifact()),
            typed,
            &declarations.boundaries,
            provenance,
            Arc::clone(&declarations.records),
        )
    }
}
