use omega_register_model::{
    RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineSemanticKind,
    SelectedInstructionId, VirtualRegisterId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalPairPatternId {
    Aarch64CompareI64ZeroBranchNonZeroV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TerminalPairPattern {
    pub(crate) id: TerminalPairPatternId,
    first: InstructionPattern,
    second: InstructionPattern,
    live_through: UnitSetPattern,
    dead_after: UnitSetPattern,
}

impl TerminalPairPattern {
    pub(crate) const fn new(
        id: TerminalPairPatternId,
        first: InstructionPattern,
        second: InstructionPattern,
        live_through: UnitSetPattern,
        dead_after: UnitSetPattern,
    ) -> Self {
        Self {
            id,
            first,
            second,
            live_through,
            dead_after,
        }
    }

    pub(crate) const fn first(&self) -> &InstructionPattern {
        &self.first
    }

    pub(crate) const fn second(&self) -> &InstructionPattern {
        &self.second
    }

    pub(crate) const fn live_through(&self) -> UnitSetPattern {
        self.live_through
    }

    pub(crate) const fn dead_after(&self) -> UnitSetPattern {
        self.dead_after
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InstructionPattern {
    pub semantic: MachineSemanticKind,
    pub selected_operand_count: usize,
    pub family: MachineAlternativeFamily,
    pub variant: u32,
    pub external_reads: &'static [u16],
    pub external_writes: &'static [u16],
    pub implicit_uses: UnitSetPattern,
    pub implicit_defs: UnitSetPattern,
    pub implicit_clobbers: UnitSetPattern,
    pub memory: MachineEncodedMemoryEffect,
    pub stack: MachineEncodedStackEffect,
    pub trap: MachineEncodedTrapBehavior,
    pub control: MachineEncodedControlEffect,
    pub operands: &'static [OperandPattern],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperandPattern {
    pub operand: u16,
    pub access: RegisterOperandAccess,
    pub read_equals_storage: bool,
    pub writes_empty: bool,
    pub no_write_semantics: bool,
    pub view: ViewPattern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewPattern {
    IndexedAllocatable {
        prefix: char,
        maximum_index: u8,
        bits: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnitSetPattern(pub &'static [&'static str]);

impl UnitSetPattern {
    pub const EMPTY: Self = Self(&[]);

    pub const fn named(names: &'static [&'static str]) -> Self {
        Self(names)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedNamedUnitSet {
    pub name: &'static str,
    pub units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchedPhysicalRead {
    pub source_instruction: SelectedInstructionId,
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
    pub units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalPairMatch {
    first_reads: Vec<MatchedPhysicalRead>,
    dead_sets_live_out: bool,
}

impl TerminalPairMatch {
    pub(crate) const fn new(
        first_reads: Vec<MatchedPhysicalRead>,
        dead_sets_live_out: bool,
    ) -> Self {
        Self {
            first_reads,
            dead_sets_live_out,
        }
    }

    pub(crate) fn first_read(&self, operand: u16) -> Option<&MatchedPhysicalRead> {
        self.first_reads.iter().find(|read| read.operand == operand)
    }

    pub(crate) const fn dead_sets_live_out(&self) -> bool {
        self.dead_sets_live_out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalPairMatchError {
    MissingArchitecturalView(&'static str),
    FirstRoster(SelectedInstructionId),
    SecondRoster(SelectedInstructionId),
    FirstFootprint(SelectedInstructionId),
    SecondFootprint(SelectedInstructionId),
    FirstPhysicalSource(SelectedInstructionId),
    SecondPhysicalSource(SelectedInstructionId),
    Liveness(SelectedInstructionId),
}
