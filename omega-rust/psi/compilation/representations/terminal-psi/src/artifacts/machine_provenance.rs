//! Source-completion diagnostic attribution, never semantic or admission evidence.
//!
//! Psi projects source names and spans while it owns the checked source. A
//! policy can attribute a rejected Terminal site without retaining that source
//! or depending on a lowering implementation. Source-completion storage retains
//! this companion; it is not part of canonical Terminal semantics.

use semantic_vocabulary::MachineId;
use source::SourceSpan;
use std::collections::BTreeMap;

#[derive(Debug, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalMachineProvenance {
    machines: BTreeMap<MachineId, (String, Option<SourceSpan>)>,
    callers: BTreeMap<MachineId, MachineId>,
}

impl TerminalMachineProvenance {
    pub fn name(&mut self, machine: MachineId, name: String, span: Option<SourceSpan>) {
        self.machines.entry(machine).or_insert((name, span));
    }

    pub fn record_caller(&mut self, machine: MachineId, caller: MachineId) {
        self.callers.entry(machine).or_insert(caller);
    }

    fn named_ancestor(
        &self,
        machine: MachineId,
    ) -> Option<(MachineId, &(String, Option<SourceSpan>))> {
        let mut current = machine;
        for _ in 0..=self.callers.len() {
            if let Some(named) = self.machines.get(&current) {
                return Some((current, named));
            }
            current = *self.callers.get(&current)?;
        }
        None
    }

    pub fn label(&self, machine: MachineId) -> String {
        match self.named_ancestor(machine) {
            Some((named, (name, _))) if named == machine => {
                format!("machine `{name}` (Terminal machine {machine})")
            }
            Some((named, (name, _))) => format!(
                "Terminal machine {machine}, reached through machine `{name}` (Terminal machine {named})"
            ),
            None => format!("Terminal machine {machine}"),
        }
    }

    pub fn source_span(&self, machine: MachineId) -> Option<SourceSpan> {
        let (_, (_, span)) = self.named_ancestor(machine)?;
        *span
    }
}
