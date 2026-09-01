use psi_symbols::SymbolHandle;
use psi_typed_trees::types::PrimitiveType;

use psi_language_core::BindingRelevance;
use psi_language_semantics::{
    CarryPolicy, Multiplicity, SemanticDomainId, ServiceReachPlan, ServiceReachSummary,
};

use crate::{CheckedScalarExpression, NominalMachineUseSite};

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
/// `EdgeCleanupPlan`: executable rows name only whole, claim-free affine
/// parameters whose checked state-exit events can be realized as terminal-Psi
/// trivial discards. The separate projected row remains checked-only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralControlCleanupPlans {
    pub states: Vec<CheckedStructuralControlStateCleanupPlan>,
    /// Checked-only cleanup rows for the first direct-record projected jump
    /// cohort. These are deliberately kept out of `states`: every existing
    /// Terminal consumer uses `for_edge`, so a path-sensitive row cannot be
    /// mistaken for the older whole-root executable vocabulary.
    pub projected_edges: Vec<CheckedStructuralControlProjectedEdgeCleanupPlan>,
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
        if self
            .for_projected_edge(machine, state, statement_ordinal)
            .is_some()
        {
            return None;
        }
        self.for_state(machine, state)?
            .edges
            .iter()
            .find(|edge| edge.statement_ordinal == statement_ordinal)
    }

    pub fn for_projected_edge(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<&CheckedStructuralControlProjectedEdgeCleanupPlan> {
        self.projected_edges.iter().find(|edge| {
            edge.machine == machine
                && edge.state == state
                && edge.statement_ordinal == statement_ordinal
        })
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

/// One checked-only path-sensitive cleanup row for an ordinary state jump.
/// The first cohort has exactly one source root, one whole direct-field move,
/// and one maximal sibling residual; Terminal control has no corresponding
/// path vocabulary yet and must continue to reject it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralControlProjectedEdgeCleanupPlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub target_state: SymbolHandle,
    pub transfer: CheckedStructuralControlProjectedTransferPlan,
    pub residual_affine_discards: Vec<CheckedUnitPartialAffineDiscardPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralControlProjectedTransferPlan {
    /// Authored source-state parameter position. The bounded first cohort
    /// admits exactly position zero, but retains the coordinate explicitly.
    pub source_parameter_position: u32,
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: String,
    pub target_parameter_position: u32,
}

/// Complete checked input for the first terminal structural-control producer.
/// This deliberately supports only claim-free affine, Unit-returning attached
/// graphs whose states return naturally, unconditionally transfer whole
/// parameters, or have at most two states select independent whole-parameter
/// successors from one retained Boolean scalar input. One two-predecessor join
/// may reconverge identical structural frontiers. Ordinary successor edges may
/// also forward direct primitive scalar inputs.
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
    /// The first retained cyclic-control proof. `None` preserves the acyclic
    /// structural-Unit slice; a cyclic plan is published only when the
    /// termination checker supplied this exact source-handle-free component.
    pub ranked_scc: Option<CheckedStructuralRankedSccPlan>,
}

/// One canonical Nat-descending component admitted by the first cyclic
/// structural-Unit slice. Bounds are the exact unsigned carrier bounds, not
/// authored text, and every retained edge names the checked transition
/// coordinate whose positive guard and decrement the ranking checker proved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralRankedSccPlan {
    pub header_state: SymbolHandle,
    pub rank_scalar_parameter_index: u32,
    pub rank_primitive_type: PrimitiveType,
    pub rank_lower_bound: u128,
    pub rank_upper_bound: u128,
    pub covered_cyclic_edges: Vec<CheckedStructuralRankedSccEdgePlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedStructuralRankedSccEdgePlan {
    pub source_state: SymbolHandle,
    pub target_state: SymbolHandle,
    pub statement_ordinal: u32,
    pub guard: CheckedStructuralRankedGuardPlan,
    pub successor_argument: CheckedStructuralRankedArgumentPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralRankedGuardPlan {
    UnsignedParameterPositive {
        scalar_parameter_index: u32,
        primitive_type: PrimitiveType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralRankedArgumentPlan {
    UnsignedParameterMinusOne {
        argument_ordinal: u32,
        source_scalar_parameter_index: u32,
        target_scalar_parameter_index: u32,
        primitive_type: PrimitiveType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralUnitControlStatePlan {
    pub state: SymbolHandle,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
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
        scalar_arguments: Vec<CheckedStructuralScalarArgumentPlan>,
        trivial_affine_discard_parameter_positions: Vec<u32>,
    },
    Conditional {
        guard_scalar_parameter_index: u32,
        when_true: CheckedStructuralControlSuccessorPlan,
        when_false: CheckedStructuralControlSuccessorPlan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralControlSuccessorPlan {
    pub statement_ordinal: u32,
    pub target_state: SymbolHandle,
    pub transfers: Vec<CheckedStructuralControlTransferPlan>,
    pub scalar_arguments: Vec<CheckedStructuralScalarArgumentPlan>,
    pub trivial_affine_discard_parameter_positions: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedStructuralScalarArgumentPlan {
    /// Authored target-argument position retained as the checked expression
    /// coordinate.
    pub argument_ordinal: u32,
    pub source_scalar_parameter_index: u32,
    pub target_scalar_parameter_index: u32,
    pub primitive_type: PrimitiveType,
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
    /// Boundary-operator applications whose checked realization consumes an
    /// exact structural frontier and returns one scalar. Selection and the
    /// authored application stay explicit; this is not rediscovered from the
    /// rewritten ordinary call expression.
    pub selected_operator_machines: Vec<CheckedSelectedOperatorStructuralScalarReturnMachinePlan>,
    /// Direct trait-backed fixed-token returns. These stay separate from the
    /// builtin scalar-expression lane because the selected realization is an
    /// executable structural call, not a primitive comparison.
    pub trait_operator_machines: Vec<CheckedTraitOperatorScalarReturnMachinePlan>,
}

impl CheckedStructuralScalarReturnPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedStructuralScalarReturnMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn trait_operator_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedTraitOperatorScalarReturnMachinePlan> {
        self.trait_operator_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }

    pub fn selected_operator_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedSelectedOperatorStructuralScalarReturnMachinePlan> {
        self.selected_operator_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }
}

/// One exact selected boundary-operator return over whole structural
/// parameters. The provider plan and concrete checked realization cross the
/// checked boundary explicitly so Terminal can retain the authored D29 use
/// while emitting an ordinary structural scalar call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedSelectedOperatorStructuralScalarReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub result_type: PrimitiveType,
    pub return_statement_ordinal: u32,
    pub requirement_operator: SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: crate::CheckedProviderPlanCommitment,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_contract_report_fingerprint: u64,
    pub realization_contract_commitment: crate::MachineContractCommitment,
    pub service_reach: ServiceReachSummary,
    /// Authored source-parameter positions in fixed-token operand order.
    pub argument_source_positions: Vec<u32>,
}

/// One exact trait-backed fixed-token return over whole structural parameters.
/// The selected closed application and realization row cross the checked
/// boundary explicitly; Terminal lowering must not rediscover either from
/// names or visible conformances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTraitOperatorScalarReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: Option<String>,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub result_type: PrimitiveType,
    pub return_statement_ordinal: u32,
    pub conformance: SymbolHandle,
    /// Compact report coordinate; authority uses the adjacent commitment.
    pub conformance_application_report_fingerprint: u64,
    pub conformance_application_commitment:
        psi_typed_trees::typed_trees::ClosedConformanceApplicationCommitment,
    pub requirement: SymbolHandle,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    /// Source-independent checked body of the exact selected realization.
    /// This is retained here because an unselected conformance member is not
    /// otherwise part of the ordinary terminal scalar-expression roots.
    pub realization_return_expression: CheckedScalarExpression,
    /// Authored source-parameter positions in fixed-token operand order.
    pub argument_source_positions: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralScalarReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Dense scalar input order with each entry's original source position.
    /// Together with `structural_parameters`, this must exactly partition the
    /// authored state-parameter positions.
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Immutable primitive bindings evaluated in source order. Initializer
    /// expressions remain in `CheckedScalarExpressionPlans` at the binding's
    /// exact statement coordinate.
    pub bindings: Vec<CheckedScalarBinding>,
    pub result_type: PrimitiveType,
    pub return_statement_ordinal: u32,
    /// One bounded actual CFG convergence: a single finite `!`/`&&`/`||`
    /// binding over a finite nonempty set of runtime Boolean inputs has typed
    /// value leaves entering one shared direct return/cleanup block. Boolean
    /// equality with a constant is normalized to identity/negation. One direct
    /// relevant Boolean field identity on one nominal-cleanup root is also
    /// admitted. Integer-comparison leaves separately accept scalar parameters
    /// and landed constants beneath up to two total binary, bitwise-not, or
    /// integer-widening shells, or one proof-bearing exact-cast, exact-add,
    /// exact-subtract, exact-multiply, exact shift, exact-divide, or
    /// exact-remainder computation shell. Proof-bearing parameter bounds remain
    /// explicit.
    /// Nested or multiple field identities, member/comparison mixtures, wider
    /// integer computations, and richer leaves retain the source-distributed
    /// fallback and publish `None`.
    pub shared_boolean_convergence: Option<CheckedStructuralBooleanConvergencePlan>,
    /// Complete canonical direct-Boolean caller facts preserved at the closed
    /// scalar return edge. Nominal cleanup actions select root-local subsets
    /// by `source_parameter_index`; no-code actions consume no premise.
    pub caller_requirements: Vec<CheckedUnitNominalAffineCallerRequirementPlan>,
    /// Bounded scalar premises retained from the authored contract. This slice
    /// admits direct fixed-width integer parameter bounds and pairwise
    /// parameter relations so proof-bearing exact arithmetic can be
    /// reconstructed terminally.
    pub scalar_requirements: Vec<CheckedStructuralScalarIntegerBoundRequirementPlan>,
    /// Complete post-result cleanup stream in reverse authored parameter
    /// order. Keeping trivial and nominal actions in one list prevents either
    /// representation or a later producer from losing their relative order.
    pub cleanup_actions: Vec<CheckedStructuralScalarReturnCleanupAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralScalarIntegerBoundRequirementPlan {
    /// Dense position in this plan's scalar parameter namespace.
    pub parameter_position: u32,
    pub primitive_type: PrimitiveType,
    pub kind: CheckedStructuralScalarIntegerBoundKind,
    pub bound: CheckedStructuralScalarIntegerBoundPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralScalarIntegerBoundPlan {
    Literal(psi_numerics::literals::IntegerLiteral),
    /// Dense position in the same scalar parameter namespace.
    Parameter(u32),
    /// The maximum of this fixed-width carrier minus the named dense parameter.
    MaximumMinusParameter(u32),
    /// The minimum of this signed carrier minus the named dense parameter.
    SignedMinimumMinusParameter(u32),
    /// The minimum of this signed carrier plus the named dense parameter.
    SignedMinimumPlusParameter(u32),
    /// The maximum of this signed carrier plus the named dense parameter.
    SignedMaximumPlusParameter(u32),
    /// The maximum of this fixed-width carrier divided by the named dense parameter.
    MaximumDivideParameter(u32),
    /// The minimum of this signed carrier divided by the named dense parameter.
    SignedMinimumDivideParameter(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralScalarIntegerBoundKind {
    Lower,
    Upper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedStructuralBooleanConvergencePlan {
    pub binding_ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralScalarReturnCleanupAction {
    DiscardRoot(u32),
    InvokeNominal(CheckedUnitNominalAffineCleanupPlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedStructuralScalarParameterPlan {
    pub source_position: u32,
    pub primitive_type: PrimitiveType,
}

/// Source-handle-free checked plans for the first result-bearing bodyless
/// boundary slice. One successful boundary invocation returns a primitive
/// scalar. When structural custody is present, the invocation also consumes
/// the complete claim frontier carried by its arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedBoundaryScalarReturnPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
    pub boundary_machines: Vec<CheckedBoundaryMachinePlan>,
    pub machines: Vec<CheckedBoundaryScalarReturnMachinePlan>,
}

impl CheckedBoundaryScalarReturnPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedBoundaryScalarReturnMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryScalarReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub entry_claims: Vec<CheckedUnitEntryClaimPlan>,
    pub boundary_call: CheckedUnitEffectOperationPlan,
    pub result_type: PrimitiveType,
    pub return_statement_ordinal: u32,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
}

/// Source-handle-free plans for the first general structural/Unit terminal
/// slice. These rows are assembled only after ownership and carry checking
/// have recorded their authoritative facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedUnitEffectPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
    pub boundary_machines: Vec<CheckedBoundaryMachinePlan>,
    pub machines: Vec<CheckedUnitEffectMachinePlan>,
    /// Dynamic dispatch custody is grouped separately so direct and rebound
    /// descriptor/table lanes can evolve without repeatedly widening this
    /// general Unit-plan record.
    pub dynamic_dispatch: crate::CheckedDynamicDispatchPlans,
    /// Multi-state Unit machines whose exact control and effect rows were
    /// admitted as one atomic executable plan.
    pub composed_machines: Vec<CheckedComposedUnitControlMachinePlan>,
}

impl CheckedUnitEffectPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&CheckedUnitEffectMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn boundary_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedBoundaryMachinePlan> {
        self.boundary_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }

    pub fn composed_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedComposedUnitControlMachinePlan> {
        self.composed_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedComposedUnitControlMachinePlan {
    pub machine: SymbolHandle,
    pub attachment_type_identity: String,
    pub provider_attachment_requirements: Vec<CheckedProviderAttachmentRequirementPlan>,
    pub body_qualifications: Vec<SemanticDomainId>,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: crate::MachineContractCommitment,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
    pub states: Vec<CheckedComposedUnitControlStatePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedComposedUnitControlStatePlan {
    pub state: SymbolHandle,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub entry_claims: Vec<CheckedUnitEntryClaimPlan>,
    /// Ordered effect operations only. Control exits remain in `terminator`.
    pub operations: Vec<CheckedUnitEffectOperationPlan>,
    pub terminator: CheckedComposedUnitControlTerminatorPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedComposedUnitControlTerminatorPlan {
    ReturnUnit,
    Jump {
        successor: CheckedStructuralControlSuccessorPlan,
    },
    Conditional {
        /// Exact checked scalar expression selected by the authored guard.
        /// The current family admits either one Boolean state parameter or a
        /// closed expression with no local or structural dependencies.
        guard: CheckedScalarExpression,
        when_true: CheckedStructuralControlSuccessorPlan,
        when_false: CheckedStructuralControlSuccessorPlan,
    },
}

/// Source-handle-free checked plans for the bounded structural-result lanes.
/// Claim-bearing machines admit an exact whole-root linear transfer; the
/// separate payload-less-case lane admits exact zero-input unrestricted sum
/// construction without manufacturing claim custody.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralReturnPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
    pub machines: Vec<CheckedStructuralReturnMachinePlan>,
    /// Exact zero-input constructors for one payload-less case of a closed
    /// unrestricted sum. These remain separate from claim-bearing whole-root
    /// transfers so downstream consumers cannot confuse construction with a
    /// parameter claim reshuffle.
    pub payloadless_case_machines: Vec<CheckedPayloadlessCaseReturnMachinePlan>,
}

impl CheckedStructuralReturnPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedStructuralReturnMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn payloadless_case_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedPayloadlessCaseReturnMachinePlan> {
        self.payloadless_case_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }
}

/// Source-handle-free checked plan for the first exact nominal sum-case
/// result constructor. The selected case owns no payload, and the result is a
/// closed, qualification-free unrestricted sum, so no runtime input or
/// ownership claim participates in construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPayloadlessCaseReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    pub result: CheckedStructuralResultPlan,
    /// Normalized identity from the selected case declaration, not its source
    /// spelling or arena handle.
    pub returned_case_identity: String,
}

/// Source-handle-free checked plan for the first internal structural-result
/// call. This deliberately admits only a final direct call whose one
/// whole-root linear result is immediately returned by the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralCallReturnPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
    pub machines: Vec<CheckedStructuralCallReturnMachinePlan>,
    /// Exact unrestricted payloadless calls whose exhaustive case arms all
    /// return the saved call result unchanged. Proof selectors remain erased
    /// and may bind at most one guarded caller-local evidence term.
    pub payloadless_guarded_machines: Vec<CheckedPayloadlessGuardedCallReturnMachinePlan>,
}

impl CheckedStructuralCallReturnPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedStructuralCallReturnMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn payloadless_guarded_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedPayloadlessGuardedCallReturnMachinePlan> {
        self.payloadless_guarded_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPayloadlessGuardedCallReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    pub result: CheckedStructuralResultPlan,
    pub call: CheckedUnitCallCoordinate,
    pub target_machine: SymbolHandle,
    pub target_state: SymbolHandle,
    /// Canonically ordered explicitly selected named rows. An empty vector
    /// retains the callee's guarded implications without minting caller terms.
    pub selected_evidence: Vec<CheckedPayloadlessGuardedCallEvidencePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPayloadlessGuardedCallEvidencePlan {
    pub arm_statement_index: u32,
    pub guarantee: psi_arena::Handle<crate::OutcomeSpecificGuaranteeFact>,
    pub selected_term: psi_arena::Handle<crate::CheckedEvidenceTerm>,
    /// True exactly for the bounded whole-result proposition substitution.
    pub substitutes_result: bool,
    /// The one bounded proof-only use of this selected term. Up to two distinct
    /// selected rows may each occupy one dense tail-requirement lane. Runtime
    /// lowering continues to return the saved structural result unchanged.
    pub tail_use: Option<CheckedPayloadlessGuardedCallEvidenceUsePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPayloadlessGuardedCallEvidenceUsePlan {
    pub target_state: SymbolHandle,
    pub input_position: u32,
    pub parameter: psi_arena::Handle<crate::CheckedEvidenceTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralCallReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub result: CheckedStructuralResultPlan,
    pub entry_claim: CheckedUnitEntryClaimPlan,
    pub call: CheckedStructuralCallPlan,
    /// Caller-local identity re-established beneath the operation-result root
    /// and transferred unchanged to the machine result.
    pub returned_claim: psi_language_semantics::PermissionClaimIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralCallPlan {
    pub coordinate: CheckedUnitCallCoordinate,
    pub target_machine: SymbolHandle,
    pub target_state: SymbolHandle,
    pub target_contract_report_fingerprint: u64,
    pub service_reach: ServiceReachSummary,
    pub structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
    pub claim_transfers: Vec<CheckedUnitClaimTransferPlan>,
    /// Exact callee-local result claim mapped back to the caller-local claim
    /// namespace after successful completion.
    pub callee_returned_claim: psi_language_semantics::PermissionClaimIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralReturnMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: String,
    /// Exact structural signature in dense terminal order: one returned linear
    /// root followed by a finite claim-free affine cleanup tail.
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Dense parameter index of the whole root transferred to the result.
    pub returned_parameter_index: u32,
    pub result: CheckedStructuralResultPlan,
    /// Source-handle-free declarations for the exact trivial affine locals
    /// established before the whole-root return, in dense declaration order.
    pub trivial_affine_locals: Vec<CheckedTrivialAffineStructuralLocalPlan>,
    /// Exact local declaration coordinates cleaned before parameter cleanup,
    /// in reverse declaration order.
    pub trivial_affine_local_discard_ordinals: Vec<u32>,
    pub entry_claim: CheckedUnitEntryClaimPlan,
    /// Exact reverse-declaration affine parameter cleanup positions committed
    /// after result materialization.
    pub trivial_affine_discards: Vec<u32>,
    /// Exact identity shared by the entry claim, normalized claim outcome, and
    /// identity-reshuffle fact.
    pub transferred_claim: psi_language_semantics::PermissionClaimIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTrivialAffineStructuralLocalPlan {
    pub declaration_ordinal: u32,
    pub type_identity: String,
    /// Present only for a statically established fixed-array construction
    /// element. The root stays semantic metadata and never becomes an input.
    pub construction: Option<CheckedAffineConstructionElementPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedAffineConstructionElementPlan {
    pub root_type_identity: String,
    pub index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralResultPlan {
    pub type_identity: String,
    pub multiplicity: Multiplicity,
    pub qualifications: Vec<SemanticDomainId>,
}

/// One concrete target-neutral structural shape. Identities are normalized
/// semantic type identities rather than source-tree handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralTypePlan {
    pub identity: String,
    pub shape: CheckedUnitStructuralTypeShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitStructuralTypeShape {
    /// One whole primitive referent carried through structural custody. This
    /// is distinct from a by-value scalar parameter: the place remains the
    /// identity of an existing live value across an exclusive borrow.
    PrimitiveScalar(psi_typed_trees::types::PrimitiveType),
    /// Immutable view over exact literal octets. This is semantic custody,
    /// not an assertion about a target pointer/length layout.
    ByteSequence(CheckedByteSequenceCarrier),
    /// Field order is declaration order; field identities are normalized
    /// declaration identities rather than source spellings alone.
    Record {
        fields: Vec<CheckedUnitStructuralFieldPlan>,
    },
    /// The first indexed aggregate carrier admits only literal lengths and a
    /// structural element type. Runtime lengths and scalar elements remain
    /// outside this checked terminal slice.
    FixedArray {
        element_type_identity: String,
        length: u64,
    },
    /// A closed pure sum. Each case owns its exact payload-field roster; an
    /// empty roster is the payload-less case form.
    Sum {
        cases: Vec<CheckedUnitStructuralCasePlan>,
    },
    /// A closed sum with common fields. Common-field and case declaration
    /// order are both semantic; payload fields remain owned by their case.
    Mixed {
        fields: Vec<CheckedUnitStructuralFieldPlan>,
        cases: Vec<CheckedUnitStructuralCasePlan>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralCasePlan {
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
    ByteSequence(CheckedByteSequenceCarrier),
    Structural {
        type_identity: String,
    },
    /// An authored dynamic boundary-trait field whose runtime carrier is
    /// eliminated only by an exact provider-installation specialization.
    /// The enclosing machine plan must retain one requirement row for every
    /// boundary call routed through this field.
    ProviderBacked {
        provider_type_identity: String,
    },
    /// An erased semantic field does not require an executable structural
    /// carrier. Its exact normalized type identity remains independently
    /// checkable in terminal Psi.
    Erased {
        type_identity: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedByteSequenceCarrier {
    BorrowedView,
    BoundedOwned { capacity: u64 },
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

/// Semantic authority retained by one structural carrier. Borrowed modes have
/// the same physical pointer ABI, but are deliberately distinct in checked and
/// Terminal identity so lowering cannot widen a non-observing loan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralAccess {
    Owned,
    SharedBorrow,
    MutableBorrow,
    WriteOnlyBorrow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralParameterPlan {
    /// Position in the authored state signature. Structural argument lists use
    /// their own dense order and therefore never reinterpret this coordinate.
    pub position: u32,
    pub is_self: bool,
    pub type_identity: String,
    pub multiplicity: Multiplicity,
    pub access: CheckedStructuralAccess,
    /// Strictly ordered normalized domain identities.
    pub qualifications: Vec<SemanticDomainId>,
}

/// Source-handle-free structural path retained by checked terminal plans.
/// Cases and runtime indexes deliberately have no variant in this vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedUnitStructuralPathSegment {
    Field(String),
    FixedIndex(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitEntryClaimPlan {
    pub claim_identity: psi_language_semantics::PermissionClaimIdentity,
    /// Dense index into `structural_parameters`.
    pub parameter_index: u32,
    /// Stable structural path below the parameter root. Cases and dynamic
    /// indexes reject rather than retaining source handles.
    pub path: Vec<CheckedUnitStructuralPathSegment>,
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
    /// Empty names the complete source parameter. Accepted projected slices
    /// retain exactly one literal fixed-array index or a finite nonempty record
    /// field path; their specialized checked plans constrain the destination.
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: String,
    /// Explicit access presented at this call site. This is independently
    /// checked against both the source carrier and target parameter.
    pub access: CheckedStructuralAccess,
    /// Present only for an exact byte-sequence literal passed directly to a
    /// bodyless boundary. The parameter index is then deliberately invalid and
    /// must never be interpreted as caller storage.
    pub byte_sequence_literal: Option<Vec<u8>>,
}

/// One claim-free affine structural leaf that remains live after a projected
/// transfer and is disposed on the enclosing Unit-return edge. The root is a
/// dense structural-parameter coordinate; the path is canonical semantic
/// identity rather than a retained source handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitPartialAffineDiscardPlan {
    pub source_parameter_index: u32,
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: String,
}

/// Checked-only carrier for direct-record-field partial cleanup. It remains
/// separate from `CheckedUnitEffectPlans` because its return cleanup is
/// path-sensitive rather than a whole-root discard.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedPartialAffineUnitCleanupPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub machines: Vec<CheckedPartialAffineUnitCleanupMachinePlan>,
}

impl CheckedPartialAffineUnitCleanupPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedPartialAffineUnitCleanupMachinePlan> {
        self.machines
            .iter()
            .find(|plan| plan.machine.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPartialAffineUnitCleanupMachinePlan {
    /// Complete ordinary Unit signature/call/return plan. This machine is not
    /// published through `CheckedUnitEffectPlans` while its return cleanup is
    /// still path-sensitive.
    pub machine: CheckedUnitEffectMachinePlan,
    /// Exact maximal residual subtrees after every source-ordered projected
    /// call commits. Pairwise prefix-disjoint paths are grouped recursively at
    /// selected ancestors while retaining reverse declaration order at every
    /// record level.
    pub residual_affine_discards: Vec<CheckedUnitPartialAffineDiscardPlan>,
}

/// Checked-only carrier for the first whole-root nominal-cleanup slice. It is
/// intentionally separate from `CheckedUnitEffectPlans`: a nominal cleanup is
/// executable edge work and must never be reinterpreted as a trivial affine
/// discard by an older terminal producer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedNominalAffineUnitCleanupPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub machines: Vec<CheckedNominalAffineUnitCleanupMachinePlan>,
}

impl CheckedNominalAffineUnitCleanupPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedNominalAffineUnitCleanupMachinePlan> {
        self.machines
            .iter()
            .find(|plan| plan.machine.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedNominalAffineUnitCleanupMachinePlan {
    /// Exact ordinary Unit signature and return edge. Both trivial-discard
    /// lists are empty; `cleanups` is the complete reverse-parameter-order
    /// disposal list committed by the edge.
    pub machine: CheckedUnitEffectMachinePlan,
    /// Complete canonical direct-Boolean caller requirement set at the sole
    /// return edge. Cleanup-local requirements select subsets of this set by
    /// `source_parameter_index`.
    pub caller_requirements: Vec<CheckedUnitNominalAffineCallerRequirementPlan>,
    pub cleanups: Vec<CheckedUnitNominalAffineCleanupPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitNominalAffineCallerRequirementPlan {
    pub source_parameter_index: u32,
    pub field_identity: String,
    pub expected: bool,
}

/// One whole affine parameter disposed by its exact checked empty nominal
/// cleanup machine. The parameter index is dense in the checked structural
/// signature and the type identity joins the root to the cleanup attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitNominalAffineCleanupPlan {
    pub source_parameter_index: u32,
    pub type_identity: String,
    pub cleanup_machine: SymbolHandle,
    pub cleanup_state: SymbolHandle,
    pub cleanup_contract_report_fingerprint: u64,
    /// Source-independent preconditions proved at this exact implicit cleanup
    /// edge. The contextual slice admits a finite canonical set of direct
    /// relevant Boolean fields of the cleanup receiver.
    pub requirements: Vec<CheckedUnitNominalAffineCleanupRequirementPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitNominalAffineCleanupRequirementPlan {
    /// Normalized declaration identity, using the same `#identity`/name
    /// convention as [`CheckedUnitStructuralFieldPlan::identity`].
    pub field_identity: String,
    pub expected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitClaimTransferPlan {
    pub claim_identity: psi_language_semantics::PermissionClaimIdentity,
    /// Dense index into this call's structural argument list. The callee-local
    /// claim identity is reconstructed by terminal verification.
    pub argument_index: u32,
}

/// Exact state-local coordinate receiving one primitive boundary result in a
/// Unit-effect body. The local has no structural place or cleanup action; its
/// dense binding ordinal is the scalar value namespace used by later checked
/// call arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitScalarResultBindingPlan {
    pub statement_index: u32,
    pub binding_ordinal: u32,
    pub primitive_type: PrimitiveType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitEffectOperationPlan {
    EstablishTrivialAffineLocal {
        statement_index: u32,
        declaration_ordinal: u32,
        type_identity: String,
    },
    CallUnit {
        coordinate: CheckedUnitCallCoordinate,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        service_reach: ServiceReachSummary,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        claim_transfers: Vec<CheckedUnitClaimTransferPlan>,
    },
    BoundaryCall {
        coordinate: CheckedUnitCallCoordinate,
        /// Exact authored call site retained for target-owned occurrence joins.
        /// Transition-target calls have no statement/expression arena site.
        source_site: Option<NominalMachineUseSite>,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        service_reach: ServiceReachSummary,
        /// Checked primitive arguments in the boundary declaration's dense
        /// scalar-parameter order. Structural arguments retain their separate
        /// custody namespace below.
        scalar_arguments: Vec<CheckedScalarExpression>,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        completion_receipts: Vec<CheckedUnitClaimTransferPlan>,
    },
    /// Invoke one result-bearing bodyless boundary and bind its primitive
    /// result to the exact immutable local declared by this statement. This is
    /// deliberately distinct from `BoundaryCall`: downstream lowering must
    /// publish a scalar result and make it available to later operations.
    BoundaryScalarCall {
        coordinate: CheckedUnitCallCoordinate,
        source_site: Option<NominalMachineUseSite>,
        result: CheckedUnitScalarResultBindingPlan,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        service_reach: ServiceReachSummary,
        scalar_arguments: Vec<CheckedScalarExpression>,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        completion_receipts: Vec<CheckedUnitClaimTransferPlan>,
    },
    /// Call the exact checked scalar machine selected for one authored
    /// boundary-operator use and bind its result. The requirement and
    /// realization identities are joined only by compiler-owned ProviderPlan
    /// settlement; Terminal retains the realization as an ordinary call.
    SelectedOperatorScalarCall {
        coordinate: CheckedUnitCallCoordinate,
        result: CheckedUnitScalarResultBindingPlan,
        requirement_operator: SymbolHandle,
        provider_plan_report_fingerprint: u64,
        provider_plan_commitment: crate::CheckedProviderPlanCommitment,
        realization_machine: SymbolHandle,
        realization_state: SymbolHandle,
        realization_contract_report_fingerprint: u64,
        realization_contract_commitment: crate::MachineContractCommitment,
        service_reach: ServiceReachSummary,
        scalar_arguments: Vec<CheckedScalarExpression>,
    },
    /// Execute one exact nearest-even IEEE fused multiply-add selected from a
    /// compiler-intrinsic ProviderPlan. Unlike a checked-body adapter this has
    /// no synthetic callee: Terminal lowering publishes the target-neutral
    /// scalar operation directly while retaining the exact selected-plan join.
    SelectedIeeeFloatFusedMultiplyAdd {
        coordinate: CheckedUnitCallCoordinate,
        result: CheckedUnitScalarResultBindingPlan,
        requirement_operator: SymbolHandle,
        provider_plan_report_fingerprint: u64,
        provider_plan_commitment: crate::CheckedProviderPlanCommitment,
        format: psi_core::IeeeFloatFormat,
        operands: Vec<CheckedScalarExpression>,
    },
    PortWrite {
        coordinate: CheckedUnitCallCoordinate,
        port: u16,
        value: u8,
        service_reach: ServiceReachSummary,
    },
    /// Replace one whole unrestricted primitive through an exact write-only
    /// structural parameter. The checked scalar expression is retained so
    /// later lowering can emit its ordinary scalar producer before the store;
    /// the first admitted producer rung restricts this to a landed integer or
    /// Boolean literal.
    WriteOnlyPrimitiveStore {
        statement_index: u32,
        destination_parameter_index: u32,
        value: CheckedScalarExpression,
    },
    ReturnUnit {
        statement_index: u32,
        /// Exact local declaration coordinates cleaned before parameters, in
        /// reverse declaration order.
        trivial_affine_local_discard_ordinals: Vec<u32>,
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
    /// Exact boundary requirements replacing one authored provider-backed
    /// attachment field. Empty means no attachment specialization occurred.
    pub provider_attachment_requirements: Vec<CheckedProviderAttachmentRequirementPlan>,
    /// Dense source-order declarations for the bounded empty-record affine
    /// local prefix.
    pub trivial_affine_locals: Vec<CheckedTrivialAffineStructuralLocalPlan>,
    pub entry_claims: Vec<CheckedUnitEntryClaimPlan>,
    /// Canonical sorted domains from `QualificationFacts`.
    pub body_qualifications: Vec<SemanticDomainId>,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: crate::MachineContractCommitment,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
    pub operations: Vec<CheckedUnitEffectOperationPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProviderAttachmentRequirementPlan {
    pub field_identity: String,
    pub provider_type_identity: String,
    pub boundary: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    /// Exact owner of the canonical contract carrier. For an attached
    /// boundary declaration this is `machine`; for a boundary-trait
    /// requirement it is the declaring trait that owns the crash capsule.
    pub contract_owner: SymbolHandle,
    /// Present for a bodyless attached boundary declaration; absent for a
    /// static boundary-trait requirement, which has no runtime provider value.
    pub attachment_type_identity: Option<String>,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Primitive parameters in authored order after removing structural
    /// parameters into their independent custody namespace.
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub result_type: Option<PrimitiveType>,
    /// Canonical `(argument_index, domain)` order derived from exact normalized
    /// membership facts in the boundary contract.
    pub domain_requirements: Vec<CheckedUnitStructuralDomainRequirementPlan>,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: crate::MachineContractCommitment,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
}
