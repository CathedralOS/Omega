/*
Target-lowered functions preserve explicit basic-block control graphs, block-parameter telescopes, and result-bearing exit terminators alongside their evaluated ABI calling plans. We define `TargetFunction`, `TargetControlGraph`, `TargetControlBlock`, and `TargetControlTerminator` in one module.

Keeping the function container and its control graph together makes clear how function-level ABI declarations (`scalar_abi`, `mixed_structural_scalar_abi`, `call_plan`) bind to entry block parameters and return/invocation terminators.
*/

use crate::{
    MixedStructuralScalarFunctionAbi, ScalarAbiValue, ScalarFunctionAbi, TargetBooleanExpression,
    TargetStructuralParameter, TargetUnitOperation, TerminalPsiProvenance,
};
use abstract_operations::ValueBinding;
use calling_conventions::CallPlan;
use semantic_vocabulary::{BlockId, EdgeId, MachineId, ScalarType, StructuralTypeId, ValueId};
use terminal_psi::TerminalAffineCleanupAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>, // Terminal attachment retained for artifact-side nominal cleanup validation.
    pub nominal_cleanup_subject: Option<StructuralTypeId>, // Owner-designated cleanup subject, independent of attachment or call sites.
    pub scalar_abi: Option<ScalarFunctionAbi>,
    pub mixed_structural_scalar_abi: Option<MixedStructuralScalarFunctionAbi>,
    pub provenance: TerminalPsiProvenance,
    pub graph: TargetControlGraph,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetControlGraph {
    pub structural_types: abstract_operations::StructuralTypeCatalog,
    pub call_plan: CallPlan,
    pub scalar_parameters: Vec<ScalarAbiValue>,
    pub parameters: Vec<TargetStructuralParameter>,
    pub entry: BlockId,
    pub blocks: Vec<TargetControlBlock>,
}

impl TargetControlGraph {
    pub fn operations(&self) -> impl Iterator<Item = &TargetUnitOperation> + '_ {
        self.blocks.iter().flat_map(|block| &block.operations)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetControlBlock {
    pub block: BlockId,
    pub parameters: Vec<TargetScalarBlockParameter>,
    pub structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    pub operations: Vec<TargetUnitOperation>, // Nonterminal operations only; control is owned by `terminator`.
    pub terminator: TargetControlTerminator,
}

impl TargetControlBlock {
    pub fn structural_parameter(
        &self,
        place: semantic_vocabulary::PlaceId,
    ) -> Option<&terminal_psi::StructuralParameterDeclaration> {
        self.structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetScalarBlockParameter {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetControlTerminator {
    InvokeNoNormal {
        psi_edge: EdgeId,
        callee: MachineId,
        call_plan: calling_conventions::CallPlan,
        scalar_arguments: Vec<crate::TargetUnitScalarCallArgument>,
        arguments: Vec<crate::TargetStructuralArgument>,
        access_bindings: Vec<crate::TargetAccessCallBinding>,
        claim_transfers: Vec<terminal_psi::ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<terminal_psi::CrashRouteBucket>,
        declared_result: abstract_operations::AbstractFunctionResult,
    },
    ExternalCompletion {
        psi_edge: EdgeId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        kind: semantic_vocabulary::ExternalCompletionKind,
        binding: crate::ExternalCompletionBinding,
        arguments: Vec<crate::TargetUnitScalarCallArgument>,
        structural_arguments: Vec<terminal_psi::StructuralArgument>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        frontier_lower_bound: Vec<semantic_vocabulary::ClaimId>,
    },
    Crash {
        psi_edge: EdgeId,
        cause: terminal_psi::CrashCause,
        site_guard: Vec<terminal_psi::CrashPredicateTerm>,
        frontier_lower_bound: Vec<semantic_vocabulary::ClaimId>,
    },
    ReturnStructural {
        psi_edge: EdgeId,
        source: TargetStructuralReturnSource,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    ReturnScalar {
        psi_edge: EdgeId,
        source_value: ValueId,
        expression: crate::TargetScalarExpression,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    StructuralCase {
        source: TargetStructuralCaseSource,
        path: Vec<terminal_psi::StructuralPathSegment>,
        byte_offset: u32,
        layout: calling_conventions::ConventionalSumLayout,
        /// Pointer words crossed to reach the sum, in chase order.
        ///
        /// Stage 02 records this chase while projecting the sum. Selection
        /// borrows the list. `region_byte_size` is the region those words
        /// address: the sum's own size when the chase is empty and the sum
        /// fills its region, and the referent's size after a chase. A sum
        /// that is only a field of that referent is smaller than the region.
        pointer_slots: Vec<TargetCasePointerSlot>,
        region_byte_size: u32,
        cases: Vec<TargetControlCaseSuccessor>,
    },
    Return {
        psi_edge: EdgeId,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    Jump {
        successor: TargetControlSuccessor,
    },
    Conditional {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetControlSuccessor,
        when_false: TargetControlSuccessor,
    },
}

/// One pointer word between a sum and the storage selection has to load.
///
/// `index_start..index_end` selects this word's runtime indices in the
/// operation's flattened index list. The list is prepared once, on the
/// target operation that projected the sum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetCasePointerSlot {
    pub byte_offset: u32,
    pub index_start: u32,
    pub index_end: u32,
    pub scalar_descriptor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetStructuralReturnSource {
    SharedDescriptor { place: semantic_vocabulary::PlaceId },
    Home(crate::TargetStructuralHomeRequirement),
    Parameter(TargetStructuralParameter),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetStructuralCaseSource {
    Home(crate::TargetStructuralHomeRequirement),
    Parameter {
        parameter: TargetStructuralParameter,
        layout: crate::TargetStructuralHomeLayout,
    },
    BorrowedParameter {
        parameter: TargetStructuralParameter,
        layout: crate::TargetStructuralHomeLayout,
    },
    BorrowedBlockParameter {
        parameter: terminal_psi::StructuralParameterDeclaration,
        layout: crate::TargetStructuralHomeLayout,
    },
}

impl TargetStructuralCaseSource {
    pub const fn place(&self) -> semantic_vocabulary::PlaceId {
        match self {
            Self::Home(home) => home.place(),
            Self::Parameter { parameter, .. } | Self::BorrowedParameter { parameter, .. } => {
                parameter.place
            }
            Self::BorrowedBlockParameter { parameter, .. } => parameter.place,
        }
    }

    pub const fn structural_type(&self) -> semantic_vocabulary::StructuralTypeId {
        match self {
            Self::Home(home) => home.structural_type(),
            Self::Parameter { parameter, .. } | Self::BorrowedParameter { parameter, .. } => {
                parameter.structural_type
            }
            Self::BorrowedBlockParameter { parameter, .. } => parameter.structural_type,
        }
    }

    pub const fn layout(&self) -> &crate::TargetStructuralHomeLayout {
        match self {
            Self::Home(home) => &home.layout,
            Self::Parameter { layout, .. }
            | Self::BorrowedParameter { layout, .. }
            | Self::BorrowedBlockParameter { layout, .. } => layout,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetControlCaseSuccessor {
    pub psi_edge: EdgeId,
    pub case: semantic_vocabulary::StructuralCaseId,
    pub case_tag: i32,
    pub target: BlockId,
    pub payloads: Vec<TargetControlCasePayload>,
    pub structural_bindings: Vec<abstract_operations::AbstractStructuralBinding>,
    pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetControlCasePayload {
    pub field: semantic_vocabulary::StructuralFieldId,
    pub field_byte_offset: u32,
    pub parameter: crate::TargetScalarBlockValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetControlSuccessor {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub structural_bindings: Vec<abstract_operations::AbstractStructuralBinding>,
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}

impl TargetControlSuccessor {
    pub fn structural_binding(
        &self,
        parameter: semantic_vocabulary::PlaceId,
    ) -> Option<&abstract_operations::AbstractStructuralBinding> {
        self.structural_bindings
            .iter()
            .find(|binding| binding.parameter == parameter)
    }
}
