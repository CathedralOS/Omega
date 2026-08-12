use psi_symbols::SymbolHandle;
use psi_typed_trees::types::PrimitiveType;

use psi_language_core::BindingRelevance;
use psi_language_semantics::{
    CarryPolicy, Multiplicity, SemanticDomainId, ServiceReachPlan, ServiceReachSummary,
};

/// Stable machine identities and names used to select the bootstrap terminal
/// producer without reopening the typed machine table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTerminalMachineSelections {
    pub machines: Vec<CheckedTerminalMachineSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTerminalMachineSelection {
    pub machine: SymbolHandle,
    pub name: String,
    pub signature: CheckedTerminalSignatureEligibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedTerminalSignatureEligibility {
    Eligible,
    Attached,
    Unsupported,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTerminalDebugPlans {
    pub machines: Vec<CheckedTerminalMachineDebugPlan>,
}

impl CheckedTerminalDebugPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&CheckedTerminalMachineDebugPlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTerminalMachineDebugPlan {
    pub machine: SymbolHandle,
    pub machine_span: Option<psi_source::SourceSpan>,
    pub contract_span: Option<psi_source::SourceSpan>,
    pub states: Vec<CheckedTerminalStateDebugPlan>,
    pub source_files: Vec<psi_source::SourceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTerminalStateDebugPlan {
    pub state: SymbolHandle,
    pub state_span: Option<psi_source::SourceSpan>,
    pub parameter_spans: Vec<Option<psi_source::SourceSpan>>,
    pub transition_spans: Vec<psi_source::SourceSpan>,
    pub operation_spans: Vec<psi_source::SourceSpan>,
}

/// Source-handle-free control plans accepted by the bootstrap terminal-Psi
/// scalar producer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarGraphPlans {
    pub machines: Vec<CheckedScalarMachineGraph>,
}

impl CheckedScalarGraphPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&CheckedScalarMachineGraph> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarMachineGraph {
    pub machine: SymbolHandle,
    pub states: Vec<CheckedScalarStateGraph>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarStateGraph {
    pub state: SymbolHandle,
    pub parameter_types: Vec<PrimitiveType>,
    pub bindings: Vec<CheckedScalarBinding>,
    pub result_type: PrimitiveType,
    pub terminator: CheckedScalarStateTerminator,
}

/// One immutable primitive local evaluated in source order before the state
/// terminator. Its ordinal is its stable state-local identity; source symbols
/// and expression handles do not cross the checked plan boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarBinding {
    pub statement_ordinal: u32,
    pub primitive_type: PrimitiveType,
    pub value: CheckedScalarBindingValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarBindingValue {
    Expression,
    /// One closed, receiver-free call whose result initializes this binding.
    /// The call coordinate joins directly to the checked crash-call row; its
    /// arguments live in `CheckedScalarExpressionPlans` under the same binding
    /// ordinal, so terminal production does not rediscover source expressions.
    DirectCall {
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        call_ordinal: u32,
        argument_count: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarStateTerminator {
    Return {
        statement_ordinal: u32,
    },
    Crash {
        statement_ordinal: u32,
    },
    Jump(CheckedScalarSuccessor),
    Conditional {
        guard_statement_ordinal: u32,
        when_true: CheckedScalarSuccessor,
        when_false: CheckedScalarSuccessor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarSuccessor {
    pub statement_ordinal: u32,
    pub target: SymbolHandle,
    pub argument_count: u32,
}

/// Source-handle-free no-code cleanup evidence for ordinary structural
/// control edges. This is intentionally narrower than the language's complete
/// `EdgeCleanupPlan`: it names only whole, claim-free affine parameters whose
/// checked state-exit events can be realized as terminal-Psi trivial discards.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralControlCleanupPlans {
    pub states: Vec<CheckedStructuralControlStateCleanupPlan>,
}

impl CheckedStructuralControlCleanupPlans {
    pub fn for_state(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<&CheckedStructuralControlStateCleanupPlan> {
        self.states
            .iter()
            .find(|plan| plan.machine == machine && plan.state == state)
    }

    pub fn for_edge(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<&CheckedStructuralControlEdgeCleanupPlan> {
        self.for_state(machine, state)?
            .edges
            .iter()
            .find(|edge| edge.statement_ordinal == statement_ordinal)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralControlStateCleanupPlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    /// One row per supported ordinary named transition, in source statement
    /// order. Conditional arms therefore retain their exact arm coordinate.
    pub edges: Vec<CheckedStructuralControlEdgeCleanupPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralControlEdgeCleanupPlan {
    pub statement_ordinal: u32,
    pub target_state: SymbolHandle,
    /// Source-state parameter positions in reverse declaration order. A later
    /// terminal producer resolves these positions against its independently
    /// checked structural signature before assigning terminal `PlaceId`s.
    pub trivial_affine_discard_parameter_positions: Vec<u32>,
}

/// Complete checked input for the first terminal structural-control producer.
/// This deliberately supports only claim-free affine, Unit-returning attached
/// graphs whose states either return naturally or unconditionally transfer
/// whole parameters to one local successor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralUnitControlPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub machines: Vec<CheckedStructuralUnitControlMachinePlan>,
}

impl CheckedStructuralUnitControlPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedStructuralUnitControlMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralUnitControlMachinePlan {
    pub machine: SymbolHandle,
    pub attachment_type_identity: String,
    pub states: Vec<CheckedStructuralUnitControlStatePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralUnitControlStatePlan {
    pub state: SymbolHandle,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub terminator: CheckedStructuralUnitControlTerminatorPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralUnitControlTerminatorPlan {
    ReturnUnit {
        trivial_affine_discard_parameter_positions: Vec<u32>,
    },
    Jump {
        statement_ordinal: u32,
        target_state: SymbolHandle,
        transfers: Vec<CheckedStructuralControlTransferPlan>,
        trivial_affine_discard_parameter_positions: Vec<u32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedStructuralControlTransferPlan {
    pub source_parameter_index: u32,
    pub target_parameter_index: u32,
}

/// Complete checked input for the first scalar-returning structural cleanup
/// producer. The runtime value plan remains in `CheckedScalarExpressionPlans`;
/// this row binds it to the exact affine structural entry frontier.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralScalarReturnPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub machines: Vec<CheckedStructuralScalarReturnMachinePlan>,
}

impl CheckedStructuralScalarReturnPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedStructuralScalarReturnMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralScalarReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub result_type: PrimitiveType,
    pub return_statement_ordinal: u32,
    pub trivial_affine_discard_parameter_positions: Vec<u32>,
}

/// Source-handle-free plans for the first general structural/Unit terminal
/// slice. These rows are assembled only after ownership and carry checking
/// have recorded their authoritative facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedUnitEffectPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
    pub boundary_machines: Vec<CheckedUnitBoundaryMachinePlan>,
    pub machines: Vec<CheckedUnitEffectMachinePlan>,
}

impl CheckedUnitEffectPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&CheckedUnitEffectMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn boundary_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedUnitBoundaryMachinePlan> {
        self.boundary_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }
}

/// One concrete target-neutral record shape. Field order is declaration order;
/// identities are normalized declaration identities rather than field names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralTypePlan {
    pub identity: String,
    pub fields: Vec<CheckedUnitStructuralFieldPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralFieldPlan {
    pub identity: String,
    pub relevance: BindingRelevance,
    pub field_type: CheckedUnitStructuralFieldType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitStructuralFieldType {
    Scalar(PrimitiveType),
    Structural {
        type_identity: String,
    },
    /// An erased semantic field does not require an executable structural
    /// carrier. Its exact normalized type identity remains independently
    /// checkable in terminal Psi.
    Erased {
        type_identity: String,
    },
}

/// One normalized structural qualification required by a retained parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralDomainPlan {
    pub domain: SemanticDomainId,
    pub identity: String,
    pub carrier_type_identity: String,
}

/// One exact structural-domain precondition on a boundary argument. The
/// argument index is dense over `structural_parameters`; no source expression
/// or contract-fact handle survives into this plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitStructuralDomainRequirementPlan {
    pub argument_index: u32,
    pub domain: SemanticDomainId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralParameterPlan {
    /// Position in the authored state signature. Structural argument lists use
    /// their own dense order and therefore never reinterpret this coordinate.
    pub position: u32,
    pub is_self: bool,
    pub type_identity: String,
    pub multiplicity: Multiplicity,
    /// Strictly ordered normalized domain identities.
    pub qualifications: Vec<SemanticDomainId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitEntryClaimPlan {
    pub claim_identity: psi_language_semantics::PermissionClaimIdentity,
    /// Dense index into `structural_parameters`.
    pub parameter_index: u32,
    /// Stable field identities below the parameter root. The Stage-1 record
    /// shape rejects cases and dynamic indexes rather than retaining handles.
    pub field_path: Vec<String>,
    pub carry: CarryPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitCallCoordinate {
    pub statement_index: u32,
    pub call_ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralArgumentPlan {
    /// Dense index into the caller's structural parameter list.
    pub source_parameter_index: u32,
    pub type_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitClaimTransferPlan {
    pub claim_identity: psi_language_semantics::PermissionClaimIdentity,
    /// Dense index into this call's structural argument list. The callee-local
    /// claim identity is reconstructed by terminal verification.
    pub argument_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitEffectOperationPlan {
    CallUnit {
        coordinate: CheckedUnitCallCoordinate,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_fingerprint: u64,
        service_reach: ServiceReachSummary,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        claim_transfers: Vec<CheckedUnitClaimTransferPlan>,
    },
    BoundaryCallUnit {
        coordinate: CheckedUnitCallCoordinate,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_fingerprint: u64,
        service_reach: ServiceReachSummary,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        completion_receipts: Vec<CheckedUnitClaimTransferPlan>,
    },
    PortWrite {
        coordinate: CheckedUnitCallCoordinate,
        port: u16,
        value: u8,
        service_reach: ServiceReachSummary,
    },
    ReturnUnit {
        statement_index: u32,
        /// Owned affine structural parameters discarded on this return edge,
        /// in reverse declaration order.
        trivial_affine_discards: Vec<u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitEffectMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub entry_claims: Vec<CheckedUnitEntryClaimPlan>,
    /// Canonical sorted domains from `QualificationFacts`.
    pub body_qualifications: Vec<SemanticDomainId>,
    pub contract_fingerprint: u64,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
    pub operations: Vec<CheckedUnitEffectOperationPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitBoundaryMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Canonical `(argument_index, domain)` order derived from exact normalized
    /// membership facts in the boundary contract.
    pub domain_requirements: Vec<CheckedUnitStructuralDomainRequirementPlan>,
    pub contract_fingerprint: u64,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
}
