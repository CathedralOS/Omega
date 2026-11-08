//! State used by generated entry code and its body, independently of the ABI
//! promise. Reads need permission; only writes can endanger interrupted state.

use super::state_footprints::machine_state_for_registers;
use super::{MachineStateSet, RegisterSet, StateFootprintEvidence, StatePlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateEffectsEvidence {
    reads: RegisterSet,
    footprint: StateFootprintEvidence,
    clobbered_state: MachineStateSet,
}

impl StateEffectsEvidence {
    pub fn empty() -> Self {
        Self::new(
            RegisterSet::default(),
            RegisterSet::default(),
            MachineStateSet::empty(),
            MachineStateSet::empty(),
        )
    }
    pub fn new(
        reads: RegisterSet,
        writes: RegisterSet,
        additional_use: MachineStateSet,
        additional_clobbers: MachineStateSet,
    ) -> Self {
        let clobbered_state = machine_state_for_registers(&writes).union(additional_clobbers);
        Self {
            footprint: StateFootprintEvidence::new(
                writes,
                additional_use
                    .union(clobbered_state)
                    .union(machine_state_for_registers(&reads)),
            ),
            reads,
            clobbered_state,
        }
    }

    /// A provider that publishes only a ceiling may overwrite any class in it.
    /// Missing write evidence must never turn into a read-only claim.
    pub fn conservative(footprint: StateFootprintEvidence) -> Self {
        Self {
            reads: RegisterSet::default(),
            clobbered_state: footprint.machine_state(),
            footprint,
        }
    }

    pub fn footprint(&self) -> &StateFootprintEvidence {
        &self.footprint
    }
    pub fn registers_read(&self) -> &RegisterSet {
        &self.reads
    }
    pub fn registers(&self) -> &RegisterSet {
        self.footprint.registers()
    }
    pub fn machine_state(&self) -> MachineStateSet {
        self.footprint.machine_state()
    }
    pub fn clobbered_state(&self) -> MachineStateSet {
        self.clobbered_state
    }

    pub fn compose(&self, other: &Self) -> Self {
        Self::new(
            RegisterSet::new(
                self.reads
                    .as_slice()
                    .iter()
                    .copied()
                    .chain(other.reads.as_slice().iter().copied()),
            ),
            RegisterSet::new(
                self.registers()
                    .as_slice()
                    .iter()
                    .copied()
                    .chain(other.registers().as_slice().iter().copied()),
            ),
            self.machine_state().union(other.machine_state()),
            self.clobbered_state.union(other.clobbered_state),
        )
    }

    pub fn validate(&self, plan: &StatePlan) -> Result<(), &'static str> {
        if !plan
            .permitted_transitive_use
            .contains_all(self.machine_state())
        {
            return Err("complete entry code uses machine state outside its declared ceiling");
        }
        if !plan
            .saved_state
            .contains_all(self.clobbered_state.intersection(plan.interrupted_state))
        {
            return Err(
                "complete entry code overwrites interrupted machine state it does not save",
            );
        }
        Ok(())
    }
}

impl From<StateFootprintEvidence> for StateEffectsEvidence {
    fn from(footprint: StateFootprintEvidence) -> Self {
        Self::conservative(footprint)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratedEntryControl {
    FallThrough,
    Branch {
        target_byte_offset: i64,
        conditional: bool,
    },
    Call {
        field_byte_offset: u32,
    },
    LocalCall {
        target_byte_offset: i64,
    },
    Return,
    InterruptReturn,
    Stop,
}

/// One closed target-decoded form. A form may contain a target-specified fixed
/// sequence (such as privileged stop); its whole span has one replay authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedEntryInstruction {
    pub byte_offset: u32,
    pub byte_count: u32,
    pub effects: StateEffectsEvidence,
    pub control: GeneratedEntryControl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedEntryEvidence {
    pub instructions: Vec<GeneratedEntryInstruction>,
    pub effects: StateEffectsEvidence,
}

impl GeneratedEntryEvidence {
    /// Structural closure follows target decoding. Artifact admission must
    /// independently decode the bytes again and compare the retained rows.
    pub fn from_target_instructions(
        byte_count: usize,
        instructions: Vec<GeneratedEntryInstruction>,
        body_call_field: Option<u32>,
    ) -> Result<Self, &'static str> {
        let mut cursor = 0usize;
        let mut calls = Vec::new();
        let mut effects = StateEffectsEvidence::new(
            RegisterSet::default(),
            RegisterSet::default(),
            MachineStateSet::empty(),
            MachineStateSet::empty(),
        );
        for row in &instructions {
            if body_call_field.is_none()
                && matches!(row.control, GeneratedEntryControl::LocalCall { .. })
            {
                return Err("generated frame protocol contains an unaccounted call");
            }
            if row.byte_count == 0 || row.byte_offset as usize != cursor {
                return Err("generated entry instruction coverage has a gap, overlap or empty row");
            }
            cursor = cursor
                .checked_add(row.byte_count as usize)
                .ok_or("generated entry byte coverage overflows")?;
            if cursor > byte_count {
                return Err("generated entry instruction exceeds its byte image");
            }
            if let GeneratedEntryControl::Call { field_byte_offset } = row.control {
                calls.push(field_byte_offset);
            }
            effects = effects.compose(&row.effects);
        }
        if cursor != byte_count {
            return Err("generated entry instructions do not cover every byte");
        }
        if calls.as_slice() != body_call_field.as_slice() {
            return Err("generated entry body call differs from its exact relocation");
        }
        for row in &instructions {
            if let GeneratedEntryControl::Branch {
                target_byte_offset, ..
            }
            | GeneratedEntryControl::LocalCall { target_byte_offset } = row.control
                && !instructions
                    .iter()
                    .any(|target| i64::from(target.byte_offset) == target_byte_offset)
            {
                return Err("generated entry branch leaves its instruction boundaries");
            }
        }
        Ok(Self {
            instructions,
            effects,
        })
    }
}
