//! One module's machine checks each replay the same callee bodies. A
//! completed summary is shared here so a callee is replayed once per module
//! rather than once per checked caller.
//!
//! A summary is published only when its replay succeeded. A callee whose
//! summary is unavailable fails its caller's replay, so a published summary
//! never saw a recursive call in progress or the replay depth bound: it is
//! the same for every caller and every order in which workers reach it. Each
//! entry keeps the callees its replay asked for and the deepest nested
//! replay below it, so a caller adopts the entry only where its own replay
//! would have reached the same answer (`Replay::adopted_summary`).

use super::engine::Summary;
use crate::descriptors::descriptor_results::{DescriptorMachine, DescriptorSlotKind};
use semantic_vocabulary::MachineId;
use std::sync::OnceLock;
use terminal_psi::generic_calls::NominalCallArgument;
use terminal_psi::{OperationKind, StructuralTypeDeclaration, TerminalModule};

/// Completed borrowed-storage summaries of one module's machines, shared by
/// the analyses that check its machines, including those on other workers.
///
/// An analysis may use this table only if its rows are this module's
/// borrowed-storage closure built with no selected calls and this table's
/// nominal calls (`terminal_borrowed_storage_graph_with_nominal_calls`),
/// unedited. `DescriptorAnalysis::share_summaries` checks what the rows and
/// arguments show of that.
pub struct ModuleSummaries<'m> {
    types: &'m [StructuralTypeDeclaration],
    nominal_calls: &'m [NominalCallArgument],
    /// Sorted, unique: an entry's index is its machine's rank here.
    machines: Vec<MachineId>,
    /// Sorted: machines whose bodies make a dynamic call.
    dynamic: Vec<MachineId>,
    published: Vec<OnceLock<PublishedSummary>>,
}

pub(crate) struct PublishedSummary {
    /// As first replayed, before any storage frame was attached.
    pub(crate) summary: Summary,
    /// The summaries its state replay asked for, sorted and unique. Empty
    /// when the summary needed no state replay.
    pub(crate) callees: Box<[MachineId]>,
    /// How many nested calls below this one replay states, or `None` when
    /// no state replay happens at or below it.
    pub(crate) height: Option<usize>,
}

impl<'m> ModuleSummaries<'m> {
    pub fn borrowed_storage(
        module: &'m TerminalModule,
        nominal_calls: &'m [NominalCallArgument],
    ) -> Self {
        let mut dynamic = module
            .machines
            .iter()
            .filter(|machine| {
                machine
                    .operations()
                    .any(|operation| matches!(operation.kind, OperationKind::CallDynamic { .. }))
            })
            .map(|machine| machine.id)
            .collect::<Vec<_>>();
        dynamic.sort_unstable();
        Self::over(
            &module.types.structural_types,
            nominal_calls,
            module.machines.iter().map(|machine| machine.id),
            dynamic,
        )
    }

    pub(crate) fn over(
        types: &'m [StructuralTypeDeclaration],
        nominal_calls: &'m [NominalCallArgument],
        machines: impl IntoIterator<Item = MachineId>,
        dynamic: Vec<MachineId>,
    ) -> Self {
        let mut machines = machines.into_iter().collect::<Vec<_>>();
        machines.sort_unstable();
        machines.dedup();
        let published = machines.iter().map(|_| OnceLock::new()).collect();
        Self {
            types,
            nominal_calls,
            machines,
            dynamic,
            published,
        }
    }

    /// Whether an analysis over these rows replays exactly the bodies this
    /// table's entries were replayed from. A closure with a dynamic call
    /// adapts every body against that closure's own rows, so its rows can
    /// differ from another closure's rows for the same machine.
    pub(crate) fn serves(
        &self,
        types: &[StructuralTypeDeclaration],
        slot_kind: DescriptorSlotKind,
        rows: &[DescriptorMachine],
        nominal_calls: &[NominalCallArgument],
    ) -> bool {
        slot_kind == DescriptorSlotKind::BorrowedStorage
            && std::ptr::eq(types, self.types)
            && std::ptr::eq(nominal_calls, self.nominal_calls)
            && rows.iter().all(|row| {
                self.rank(row.id).is_some() && self.dynamic.binary_search(&row.id).is_err()
            })
    }

    fn rank(&self, machine: MachineId) -> Option<usize> {
        self.machines.binary_search(&machine).ok()
    }

    pub(crate) fn get(&self, machine: MachineId) -> Option<&PublishedSummary> {
        self.published.get(self.rank(machine)?)?.get()
    }

    /// Every replay that publishes a machine computes the same entry, so
    /// the first one kept is the only one.
    pub(crate) fn publish(&self, machine: MachineId, entry: PublishedSummary) {
        if let Some(slot) = self.rank(machine).and_then(|rank| self.published.get(rank)) {
            let _ = slot.set(entry);
        }
    }
}

#[cfg(test)]
mod tests;
