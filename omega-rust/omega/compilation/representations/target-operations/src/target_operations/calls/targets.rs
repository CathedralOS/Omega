//! Internal call control is independent of the requirement's data ABI.
//!
//! Dynamic selection retains the canonical interface and row. The selector is
//! one opaque word in ordinary value flow, never an added data parameter; the
//! requirement owns the call plan for every realization in its table.

use semantic_vocabulary::{MachineId, ValueId};

/// Target carrier only; source selector identity stays in its checked operation.
pub fn selector_carrier_type() -> semantic_vocabulary::ScalarType {
    semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .expect("u64 carrier"),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeCallTarget {
    Machine(MachineId),
    Dynamic {
        selector: ValueId,
        interface: u32,
        slot: u32,
    },
}

impl NativeCallTarget {
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

    pub fn canonical_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            Self::Machine(machine) => {
                bytes.push(0);
                bytes.extend_from_slice(&machine.get().to_le_bytes());
            }
            Self::Dynamic {
                selector,
                interface,
                slot,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&selector.get().to_le_bytes());
                bytes.extend_from_slice(&interface.to_le_bytes());
                bytes.extend_from_slice(&slot.to_le_bytes());
            }
        }
        bytes
    }
}
