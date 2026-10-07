//! A local invocation selects its implementation independently of its operands.
//!
//! A direct machine supplies its own declared signature. A dynamic target names
//! the interface's requirement slot; no table row is a representative callee.
//! The selector is a scalar use of the invocation, never a data argument.

use semantic_vocabulary::{MachineId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractCallTarget {
    Machine(MachineId),
    Dynamic {
        selector: ValueId,
        interface: u32,
        slot: u32,
    },
}

impl AbstractCallTarget {
    pub const fn machine(self) -> Option<MachineId> {
        match self {
            Self::Machine(machine) => Some(machine),
            Self::Dynamic { .. } => None,
        }
    }

    pub const fn selector(self) -> Option<ValueId> {
        match self {
            Self::Machine(_) => None,
            Self::Dynamic { selector, .. } => Some(selector),
        }
    }
}

impl From<MachineId> for AbstractCallTarget {
    fn from(machine: MachineId) -> Self {
        Self::Machine(machine)
    }
}
