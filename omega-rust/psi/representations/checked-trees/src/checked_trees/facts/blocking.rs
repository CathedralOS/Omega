//! EFX: normalized worker-blocking contracts keyed by exact machine identity.
//!
//! Blocking stays independent from service reach and suspension. The interface
//! preserves public omission as an explicit negative guarantee; checked
//! inference remains implementation evidence.

use language_semantics::BlockingPlan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockingFacts {
    /// One exact-keyed entry per checked machine, in machine order.
    pub machines: Vec<MachineBlockingFact>,
}

impl BlockingFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<BlockingPlan> {
        self.machines
            .iter()
            .find(|fact| fact.machine == machine)
            .map(|fact| fact.plan)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineBlockingFact {
    pub machine: SymbolHandle,
    pub plan: BlockingPlan,
}
