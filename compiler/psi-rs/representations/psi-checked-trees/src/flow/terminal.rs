use psi_symbols::SymbolHandle;
use psi_typed_trees::types::PrimitiveType;

use psi_language_core::BindingRelevance;
use psi_language_semantics::{
    CarryPolicy, Multiplicity, SemanticDomainId, ServiceReachPlan, ServiceReachSummary,
};

use crate::CheckedScalarExpression;

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
    pub conformance_application_fingerprint: u64,
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
/// scalar while consuming the complete structural claim frontier carried by
/// its arguments.
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
}

/// Source-handle-free checked plan for the first exact whole-root structural
/// result transfer. The slice admits one linear parameter, one matching entry
/// claim, and one checker-derived identity reshuffle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralReturnPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
    pub machines: Vec<CheckedStructuralReturnMachinePlan>,
}

impl CheckedStructuralReturnPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedStructuralReturnMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
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
    pub cleanup_contract_fingerprint: u64,
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
        target_contract_fingerprint: u64,
        service_reach: ServiceReachSummary,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        claim_transfers: Vec<CheckedUnitClaimTransferPlan>,
    },
    BoundaryCall {
        coordinate: CheckedUnitCallCoordinate,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_fingerprint: u64,
        service_reach: ServiceReachSummary,
        /// Checked primitive arguments in the boundary declaration's dense
        /// scalar-parameter order. Structural arguments retain their separate
        /// custody namespace below.
        scalar_arguments: Vec<CheckedScalarExpression>,
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
    /// Dense source-order declarations for the bounded empty-record affine
    /// local prefix.
    pub trivial_affine_locals: Vec<CheckedTrivialAffineStructuralLocalPlan>,
    pub entry_claims: Vec<CheckedUnitEntryClaimPlan>,
    /// Canonical sorted domains from `QualificationFacts`.
    pub body_qualifications: Vec<SemanticDomainId>,
    pub contract_fingerprint: u64,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
    pub operations: Vec<CheckedUnitEffectOperationPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryMachinePlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
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
    pub contract_fingerprint: u64,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
}
