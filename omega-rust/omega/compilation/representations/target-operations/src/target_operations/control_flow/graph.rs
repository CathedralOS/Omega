//! Ordinary control retains block identity and exact result-bearing exits.
use crate::{
    ScalarAbiValue, TargetBooleanExpression, TargetStructuralParameter, TargetUnitOperation,
};
use abstract_operations::ValueBinding;
use calling_conventions::CallPlan;
use semantic_vocabulary::{BlockId, EdgeId, MachineId, ScalarType, ValueId};
use terminal_psi::TerminalAffineCleanupAction;

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
    /// Every block's nonterminal operations, in block order.
    pub fn operations(&self) -> impl Iterator<Item = &TargetUnitOperation> + '_ {
        self.blocks.iter().flat_map(|block| &block.operations)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetControlBlock {
    pub block: BlockId,
    pub parameters: Vec<TargetScalarBlockParameter>,
    pub structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    /// Nonterminal operations only; control is owned by the terminator.
    pub operations: Vec<TargetUnitOperation>,
    pub terminator: TargetControlTerminator,
}

impl TargetControlBlock {
    /// The structural parameter arriving at `place`, when declared.
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
    /// An ordinary declared-ABI call whose receiving control has no normal edge.
    /// Physical result placement remains in the call plan, without a value home.
    InvokeNoNormal {
        psi_edge: EdgeId,
        callee: MachineId,
        call_plan: calling_conventions::CallPlan,
        scalar_arguments: Vec<crate::TargetUnitScalarCallArgument>,
        arguments: Vec<crate::TargetStructuralArgument>,
        placed_bindings: Vec<crate::TargetPlacedCallBinding>,
        claim_transfers: Vec<terminal_psi::ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<terminal_psi::CrashRouteBucket>,
        declared_result: abstract_operations::AbstractFunctionResult,
    },
    /// An exact admitted boundary invocation with no normal continuation.
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
    /// No successor or cleanup. Retain semantic evidence independently of the
    /// target's chosen terminating instruction.
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

/// Owned returns retain custody of their home or ingress. Shared descriptors
/// return the two-word value at a place, not ownership of its backing storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetStructuralReturnSource {
    SharedDescriptor { place: semantic_vocabulary::PlaceId },
    Home(crate::TargetStructuralHomeRequirement),
    Parameter(TargetStructuralParameter),
}

/// The inspected sum: an activation-local home, or the function's own owned
/// incoming parameter. A parameter root keeps the exact prepared parameter row
/// and the sum layout its value copy carries; it never borrows a home origin.
/// A borrowed parameter root keeps the same prepared row and the referent's
/// sum layout — the dispatch reads the tag and payload leaves through the
/// borrow pointer rather than an activation value copy.
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
    /// An edge-carried join keeps the referent's address in the destination
    /// block's retained pointer: the dispatch reads tag and payload leaves
    /// through it, under the same readable-borrow contract as a borrowed
    /// function parameter. The declaration is the block's own structural
    /// parameter row — there is no entry ABI placement to retain.
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

/// Ordered sum alternative and the exact destination telescope it produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetControlCaseSuccessor {
    pub psi_edge: EdgeId,
    pub case: semantic_vocabulary::StructuralCaseId,
    pub case_tag: i32,
    pub target: BlockId,
    pub payloads: Vec<TargetControlCasePayload>,
    pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
}

/// A payload is defined by its destination parameter, not the sum producer.
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
    /// The structural binding keyed `parameter`, when the successor declares
    /// it.
    pub fn structural_binding(
        &self,
        parameter: semantic_vocabulary::PlaceId,
    ) -> Option<&abstract_operations::AbstractStructuralBinding> {
        self.structural_bindings
            .iter()
            .find(|binding| binding.parameter == parameter)
    }
}
