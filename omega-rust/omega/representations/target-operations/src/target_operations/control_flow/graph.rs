//! Ordinary control retains block identity and exact result-bearing exits.
use crate::{
    ScalarAbiValue, TargetBooleanExpression, TargetDynamicDescriptorParameterAbi,
    TargetStructuralParameter, TargetUnitOperation,
};
use abstract_operations::ValueBinding;
use calling_conventions::CallPlan;
use semantic_vocabulary::{BlockId, EdgeId, ScalarType, ValueId};
use terminal_psi::TerminalAffineCleanupAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetControlGraph {
    pub structural_types: abstract_operations::StructuralTypeCatalog,
    pub call_plan: CallPlan,
    pub scalar_parameters: Vec<ScalarAbiValue>,
    pub parameters: Vec<TargetStructuralParameter>,
    /// The function's borrowed existential descriptor parameters in declared
    /// order. Each row binds one complete `TerminalDynamicDescriptorParameter`
    /// to its two trailing call-plan placements; the roster is empty unless
    /// the authored signature ends in `&dyn`/`&mut dyn` parameters.
    pub dynamic_parameters: Vec<TargetDynamicDescriptorParameterAbi>,
    pub entry: BlockId,
    pub blocks: Vec<TargetControlBlock>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetScalarBlockParameter {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetControlTerminator {
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

/// A structural return reads either a produced home or the original incoming ABI value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetStructuralReturnSource {
    Home(crate::TargetStructuralHomeRequirement),
    Parameter(TargetStructuralParameter),
}

/// The inspected sum: an activation-local home, or the function's own owned
/// incoming parameter. A parameter root keeps the exact prepared parameter row
/// and the sum layout its value copy carries; it never borrows a home origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetStructuralCaseSource {
    Home(crate::TargetStructuralHomeRequirement),
    Parameter {
        parameter: TargetStructuralParameter,
        layout: crate::TargetStructuralHomeLayout,
    },
}

impl TargetStructuralCaseSource {
    pub const fn place(&self) -> semantic_vocabulary::PlaceId {
        match self {
            Self::Home(home) => home.place(),
            Self::Parameter { parameter, .. } => parameter.place,
        }
    }

    pub const fn structural_type(&self) -> semantic_vocabulary::StructuralTypeId {
        match self {
            Self::Home(home) => home.structural_type(),
            Self::Parameter { parameter, .. } => parameter.structural_type,
        }
    }

    pub const fn layout(&self) -> &crate::TargetStructuralHomeLayout {
        match self {
            Self::Home(home) => &home.layout,
            Self::Parameter { layout, .. } => layout,
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
    pub trivial_affine_discards: Vec<semantic_vocabulary::PlaceId>,
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
