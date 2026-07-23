//! STR4 slice 2 (decision 22): the KINDED effect-row facts -- the published
//! CEILING (the authored `effects` clause, normalized at lowering) beside
//! the checker-INFERRED direct/transitive summaries, all as normalized row
//! identities into one table. The bit->member hop goes through the CANONICAL
//! NAME (never the bit value): the legacy bits only enumerate which standard
//! members are present, and the cross-crate consistency pin holds the
//! correspondence.

use omega_core::semantics::{EffectRowId, EffectRowTable};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectRowFacts {
    /// The typed trees' interner EXTENDED with the inferred rows (extension
    /// preserves the typed ids -- prefix-stable).
    pub rows: EffectRowTable,
    /// One entry per machine in the effect plan, in plan order.
    pub machines: Vec<MachineEffectRows>,
    /// EFX: symbol-resolved boundary-service rows. The surrounding legacy
    /// effect facts are retained only as a migration projection.
    pub service_reaches: super::ServiceReachFacts,
}

impl EffectRowFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineEffectRows> {
        self.machines.iter().find(|fact| fact.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineEffectRows {
    pub machine: SymbolHandle,
    /// The authored `effects` clause (the machine record's normalized row):
    /// the PUBLISHED ceiling callers may rely on.
    pub published_ceiling: EffectRowId,
    /// What the checker inferred from THIS body's own statements.
    pub inferred_direct: EffectRowId,
    /// The transitive closure over calls (the decision-12 surface).
    pub inferred_transitive: EffectRowId,
}
