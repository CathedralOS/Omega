use omega_register_model::{
    RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineSemanticKind,
    SelectedInstructionId, VirtualRegisterId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstructionPairPatternId {
    Aarch64CompareI64ZeroBranchNonZeroV1,
    Aarch64SameViewCopyI64BeforeReturnV1,
    Aarch64SameViewCopyI64BeforeCompareZeroV1,
    Aarch64SameViewCopyI64BeforeCompareI64LeftOperandV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstructionPairTopology {
    BodyTailAndTerminatorV1,
    AdjacentBodyInstructionsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InstructionPairPattern {
    pub(crate) id: InstructionPairPatternId,
    topology: InstructionPairTopology,
    first: InstructionPattern,
    second: InstructionPattern,
    live_through: UnitSetPattern,
    dead_after: UnitSetPattern,
    relations: &'static [OperandRelation],
    live_through_operands: &'static [OperandCoordinate],
}

impl InstructionPairPattern {
    pub(crate) const fn new(
        id: InstructionPairPatternId,
        topology: InstructionPairTopology,
        first: InstructionPattern,
        second: InstructionPattern,
        live_through: UnitSetPattern,
        dead_after: UnitSetPattern,
        relations: &'static [OperandRelation],
        live_through_operands: &'static [OperandCoordinate],
    ) -> Self {
        Self {
            id,
            topology,
            first,
            second,
            live_through,
            dead_after,
            relations,
            live_through_operands,
        }
    }

    pub(crate) const fn topology(&self) -> InstructionPairTopology {
        self.topology
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

    pub(crate) const fn relations(&self) -> &'static [OperandRelation] {
        self.relations
    }

    pub(crate) const fn live_through_operands(&self) -> &'static [OperandCoordinate] {
        self.live_through_operands
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
    pub control: ControlPattern,
    pub operands: &'static [OperandPattern],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlPattern {
    Exact(MachineEncodedControlEffect),
    ReturnIndirectNamed(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperandPattern {
    pub operand: u16,
    pub access: RegisterOperandAccess,
    pub read: OperandReadPattern,
    pub write: OperandWritePattern,
    pub view: ViewPattern,
    pub fixed_view: FixedViewPattern,
    pub tied_to: Option<u16>,
    pub early_clobber: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixedViewPattern {
    None,
    Named(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperandReadPattern {
    Empty,
    StorageUnits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperandWritePattern {
    Empty,
    ViewWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewPattern {
    IndexedAllocatable {
        prefix: char,
        maximum_index: u8,
        bits: u16,
    },
    Named {
        name: &'static str,
        bits: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PairInstruction {
    First,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperandCoordinate {
    pub instruction: PairInstruction,
    pub operand: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperandRelation {
    SameVirtualRegister(OperandCoordinate, OperandCoordinate),
    SamePhysicalViewAndStorageUnits(OperandCoordinate, OperandCoordinate),
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
    pub storage_units: Vec<RegisterUnitId>,
    pub units: Vec<RegisterUnitId>,
    pub write_units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstructionPairMatch {
    first_reads: Vec<MatchedPhysicalRead>,
    second_reads: Vec<MatchedPhysicalRead>,
    failed_relations: Vec<OperandRelation>,
    dead_sets_live_out: bool,
}

impl InstructionPairMatch {
    pub(crate) const fn new(
        first_reads: Vec<MatchedPhysicalRead>,
        second_reads: Vec<MatchedPhysicalRead>,
        failed_relations: Vec<OperandRelation>,
        dead_sets_live_out: bool,
    ) -> Self {
        Self {
            first_reads,
            second_reads,
            failed_relations,
            dead_sets_live_out,
        }
    }

    pub(crate) fn first_read(&self, operand: u16) -> Option<&MatchedPhysicalRead> {
        self.first_reads.iter().find(|read| read.operand == operand)
    }

    pub(crate) fn second_read(&self, operand: u16) -> Option<&MatchedPhysicalRead> {
        self.second_reads
            .iter()
            .find(|read| read.operand == operand)
    }

    pub(crate) fn failed_relations(&self) -> &[OperandRelation] {
        &self.failed_relations
    }

    pub(crate) const fn dead_sets_live_out(&self) -> bool {
        self.dead_sets_live_out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstructionPairMatchError {
    MissingArchitecturalView(&'static str),
    FirstRoster(SelectedInstructionId),
    SecondRoster(SelectedInstructionId),
    FirstFootprint(SelectedInstructionId),
    SecondFootprint(SelectedInstructionId),
    FirstPhysicalSource(SelectedInstructionId),
    SecondPhysicalSource(SelectedInstructionId),
    Liveness(SelectedInstructionId),
    Topology,
}
