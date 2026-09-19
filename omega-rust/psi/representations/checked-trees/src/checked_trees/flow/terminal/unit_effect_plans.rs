//! Unit effect plans: the effect machines and every effect operation a
//! Unit-effect body performs.

use crate::checked_trees::flow::terminal::structural_return::CheckedUnitStructuralReturnPlan;
use crate::checked_trees::flow::terminal::{
    CheckedBoundaryMachinePlan, CheckedByteSequenceWritePlan, CheckedCallScalarArgument,
    CheckedComposedUnitControlMachinePlan, CheckedPrimitiveStoreDestination,
    CheckedProviderAttachmentRequirementPlan, CheckedScalarStateTerminator,
    CheckedStructuralByteSequenceFieldByteStorePlan, CheckedStructuralByteSequenceFieldStorePlan,
    CheckedStructuralCallCustodyPlan, CheckedStructuralScalarParameterPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitCallCoordinate,
    CheckedUnitClaimTransferPlan, CheckedUnitEntryClaimPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralDomainPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
    CheckedUnitStructuralTypePlan,
};
use crate::{
    CheckedScalarExpression, CheckedStructuralScalarFieldStorePlan, NominalMachineUseSite,
};
use language_semantics::{SemanticDomainId, ServiceReachPlan, ServiceReachSummary};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

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
    /// One row per checked-body machine that has neither an ordinary nor a
    /// composed plan, naming the stage that omitted it and, for closure
    /// pruning, the direct dependency that was unavailable. Rows are
    /// diagnostic evidence for the producer that later reports a missing
    /// transitive plan; they grant nothing and select no fallback body.
    pub omissions: Vec<CheckedUnitPlanOmission>,
}

/// Why one checked-body machine has no Unit plan. `machine` never appears in
/// `machines` or `composed_machines` of the same record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitPlanOmission {
    pub machine: SymbolHandle,
    pub stage: CheckedUnitPlanOmissionStage,
}

/// The planning stage at which a machine left the Unit plan roster. Closure
/// stages name the direct dependency that was unavailable; following
/// `UnavailableCallee` rows through the same record reaches the machine whose
/// own body failed local construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedUnitPlanOmissionStage {
    /// No ordinary or composed builder admitted the machine's own body.
    /// `phase` names the last phase the ordinary builder (single-state
    /// bodies) or the general state-graph builder (multi-state bodies)
    /// entered before it gave up, `state_index` the state and
    /// `statement_index` the statement it was planning, when known.
    LocalConstruction {
        phase: &'static str,
        state_index: Option<u32>,
        statement_index: Option<u32>,
    },
    /// The body was admitted, then dropped while receiver retention was
    /// propagated along the call graph (the rebuilt caller no longer
    /// retained `self`, or a receiver operand could not be rejoined).
    ReceiverReconciliation,
    /// Two builders described the same entry and neither survived the
    /// overlap resolution.
    CompetingCandidates,
    /// A composed body used an operation outside the composed vocabulary.
    ComposedVocabulary,
    /// A Unit, structural, or ordinary scalar call targets a machine that has
    /// no admitted body in the roster.
    UnavailableCallee { target: SymbolHandle },
    /// A boundary call targets a machine with no boundary plan.
    MissingBoundaryTarget { target: SymbolHandle },
    /// A scalar call has neither a registered scalar target nor an ordinary
    /// body to borrow.
    UnavailableScalarTarget { target: SymbolHandle },
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

    /// The omission row for a machine that has no plan in this record.
    pub fn omission_for_machine(&self, machine: SymbolHandle) -> Option<&CheckedUnitPlanOmission> {
        self.omissions.iter().find(|row| row.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitEffectOperationPlan {
    EstablishReference {
        result: CheckedUnitStructuralResultBindingPlan,
        source: CheckedUnitStructuralArgumentPlan,
    },
    ReleaseReference {
        statement_index: u32,
        binding_ordinal: u32,
        loan: arena::Handle<crate::BorrowLoanFact>,
    },
    /// Establish a fresh structural expression in the shared result namespace.
    /// Selected arms transfer one new owner into the expression continuation.
    EstablishStructuralValue {
        result: CheckedUnitStructuralResultBindingPlan,
        value: crate::CheckedStructuralValueHandle,
        calls: Vec<CheckedStructuralValueCall>,
        discard_result_on_return: bool,
    },
    /// Construct an unrestricted primitive fixed array in authored row-major
    /// leaf order. Empty dimensions remain in the exact structural result type.
    EstablishScalarArray {
        source: crate::CheckedArrayConstructionSource,
        result: CheckedUnitStructuralResultBindingPlan,
        elements: Vec<CheckedCallScalarArgument>,
    },
    /// Establish initialized mutable storage at its authored declaration.
    /// Later reads and borrows name the symbol, never the initializer value.
    EstablishPrimitiveLocal {
        statement_index: u32,
        symbol: SymbolHandle,
        type_identity: String,
        primitive_type: PrimitiveType,
        value: CheckedScalarExpression,
    },
    /// Normal edge after the named call completes. Cleanup belongs to this
    /// continuation, not the caller's final return or a synthetic source call.
    CallContinuationCleanup {
        coordinate: CheckedUnitCallCoordinate,
        affine_discards: Vec<CheckedUnitPartialAffineDiscardPlan>,
    },
    EstablishTrivialAffineLocal {
        statement_index: u32,
        declaration_ordinal: u32,
        type_identity: String,
    },
    /// Establish one immutable primitive local from a retained pure expression
    /// or selective computation. The result coordinate names the
    /// same dense scalar namespace used by result-bearing calls; later
    /// consumers may therefore refer to either kind without reconstructing a
    /// source expression.
    EstablishScalarLocal {
        result: CheckedUnitScalarResultBindingPlan,
        value: CheckedCallScalarArgument,
    },
    CallUnit {
        coordinate: CheckedUnitCallCoordinate,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        service_reach: ServiceReachSummary,
        scalar_arguments: Vec<CheckedCallScalarArgument>,
        /// Proof-only erased actuals in the target's erased-formal order.
        erased_scalar_arguments: Vec<CheckedCallScalarArgument>,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        claim_transfers: Vec<CheckedUnitClaimTransferPlan>,
    },
    /// Invoke one ordinary checked scalar machine from an attached Unit body
    /// and bind its primitive result into the dense local scalar namespace.
    /// Scalar graphs remain scalar-only; boundary-return bodies may also
    /// receive exact structural arguments and their existing linear claims.
    ScalarCall {
        coordinate: CheckedUnitCallCoordinate,
        result: CheckedUnitScalarResultBindingPlan,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        target_contract_commitment: crate::MachineContractCommitment,
        service_reach: ServiceReachSummary,
        scalar_arguments: Vec<CheckedCallScalarArgument>,
        /// Proof-only erased actuals in the target's erased-formal order.
        erased_scalar_arguments: Vec<CheckedCallScalarArgument>,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        claim_transfers: Vec<CheckedUnitClaimTransferPlan>,
    },
    /// Invoke an ordinary checked whole owned result producer and retain
    /// its exact result binding. This is neither a boundary invocation nor a
    /// selected operator application; the checked callee owns its semantics.
    StructuralCall {
        coordinate: CheckedUnitCallCoordinate,
        source_site: Option<NominalMachineUseSite>,
        result: CheckedUnitStructuralResultBindingPlan,
        custody: CheckedStructuralCallCustodyPlan,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        target_contract_commitment: crate::MachineContractCommitment,
        service_reach: ServiceReachSummary,
        scalar_arguments: Vec<CheckedCallScalarArgument>,
        /// Proof-only erased actuals in the target's erased-formal order.
        erased_scalar_arguments: Vec<CheckedCallScalarArgument>,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        discard_result_on_return: bool,
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
        scalar_arguments: Vec<CheckedCallScalarArgument>,
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
        scalar_arguments: Vec<CheckedCallScalarArgument>,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        completion_receipts: Vec<CheckedUnitClaimTransferPlan>,
    },
    /// Invoke one result-bearing bodyless boundary and bind its whole
    /// structural result to the exact immutable local declared by this
    /// statement. The result signature remains target-neutral; physical
    /// placement belongs to native realization.
    BoundaryStructuralCall {
        coordinate: CheckedUnitCallCoordinate,
        source_site: Option<NominalMachineUseSite>,
        result: CheckedUnitStructuralResultBindingPlan,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        service_reach: ServiceReachSummary,
        scalar_arguments: Vec<CheckedCallScalarArgument>,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        completion_receipts: Vec<CheckedUnitClaimTransferPlan>,
        /// One affine result not consumed by a later operation is explicitly
        /// discarded on the caller's final Unit return.
        discard_result_on_return: bool,
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
    /// Call the exact checked structural-scalar machine selected for one
    /// authored boundary-operator use and bind its primitive result. This
    /// Unit composition admits claim-free owned affine roots together with
    /// fixed-width scalar operands. Projected structural paths remain outside
    /// the checked vocabulary.
    SelectedOperatorStructuralScalarCall {
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
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
    },
    /// Call the exact checked mixed structural machine selected for one
    /// authored boundary-operator use and bind its whole structural result.
    /// This first family admits only a claim-free owned-affine result and
    /// whole claim-free owned-affine arguments.
    SelectedOperatorStructuralCall {
        coordinate: CheckedUnitCallCoordinate,
        result: CheckedUnitStructuralResultBindingPlan,
        requirement_operator: SymbolHandle,
        provider_plan_report_fingerprint: u64,
        provider_plan_commitment: crate::CheckedProviderPlanCommitment,
        realization_machine: SymbolHandle,
        realization_state: SymbolHandle,
        realization_contract_report_fingerprint: u64,
        realization_contract_commitment: crate::MachineContractCommitment,
        service_reach: ServiceReachSummary,
        scalar_arguments: Vec<CheckedScalarExpression>,
        structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        /// The bounded Unit carrier admits this result only as the final local
        /// initializer and settles its affine custody on the following return.
        discard_result_on_return: bool,
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
        format: semantic_vocabulary::IeeeFloatFormat,
        operands: Vec<CheckedScalarExpression>,
    },
    PortWrite {
        coordinate: CheckedUnitCallCoordinate,
        port: u16,
        value: u8,
        service_reach: ServiceReachSummary,
    },
    /// Replace one unrestricted primitive through an exclusive borrowed
    /// parameter or initialized mutable local. A nonempty declared path selects
    /// a fixed-array primitive leaf without inventing a scalar field identity.
    /// Evaluate the retained scalar expression or computation against current
    /// storage before committing the write.
    WriteOnlyPrimitiveStore {
        statement_index: u32,
        destination: CheckedPrimitiveStoreDestination,
        path: Vec<CheckedUnitStructuralPathSegment>,
        value: crate::CheckedCallScalarArgument,
    },
    /// Replace one relevant primitive field through an exact common-field
    /// path, optionally followed by one literal fixed-array index, below an
    /// exclusive structural parameter. This shares the same checked
    /// path/value custody as dynamic realization stores, but is an ordinary
    /// attached-Unit effect and therefore carries no dispatch authority. The
    /// bounded result form reads only the exact fixed-integer local produced
    /// by the immediately preceding ordinary scalar call or selected
    /// boundary-operator realization.
    StructuralScalarFieldStore(CheckedStructuralScalarFieldStorePlan),
    StructuralByteSequenceFieldStore(CheckedStructuralByteSequenceFieldStorePlan),
    StructuralByteSequenceFieldByteStore(CheckedStructuralByteSequenceFieldByteStorePlan),
    ByteSequenceWrite(CheckedByteSequenceWritePlan),
    /// Finish the body after cleanup, yielding its retained structural/scalar
    /// binding or scalar control result when present, and Unit otherwise.
    Complete {
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
pub struct CheckedStructuralValueCall {
    pub value: crate::CheckedStructuralValueHandle,
    operation: CheckedUnitEffectOperationPlan,
}

impl CheckedStructuralValueCall {
    pub fn new(
        value: crate::CheckedStructuralValueHandle,
        operation: CheckedUnitEffectOperationPlan,
    ) -> Option<Self> {
        matches!(
            operation,
            CheckedUnitEffectOperationPlan::StructuralCall { .. }
        )
        .then_some(Self { value, operation })
    }

    pub fn operation(&self) -> &CheckedUnitEffectOperationPlan {
        &self.operation
    }
}

impl CheckedUnitEffectOperationPlan {
    /// Enumerate retained operation dependencies for signature/catalog readers.
    /// This is not execution order: structural expressions evaluate their call
    /// operands in their own authored field and selected-arm order.
    pub fn with_value_calls(&self) -> impl Iterator<Item = &Self> {
        let calls = match self {
            Self::EstablishStructuralValue { calls, .. } => calls.as_slice(),
            _ => &[],
        };
        std::iter::once(self).chain(calls.iter().map(|call| &call.operation))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitScalarControlPlan {
    pub primitive_type: PrimitiveType,
    /// Exact authored conditional and return coordinates after the ordered body.
    pub terminator: CheckedScalarStateTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitEffectMachinePlan {
    /// Final ordinary scalar binding, mutually exclusive with the other completions.
    pub scalar_result: Option<CheckedUnitScalarResultBindingPlan>,
    /// Scalar control completion, mutually exclusive with either result binding.
    pub scalar_control: Option<CheckedUnitScalarControlPlan>,
    /// Whole structural value returned after ordinary sequencing, with its
    /// actual source and multiplicity retained by the completion plan.
    pub structural_result: Option<CheckedUnitStructuralReturnPlan>,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    /// Exact attached data carrier, or `None` for an ordinary free machine.
    pub attachment_type_identity: Option<String>,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Primitive parameters in authored order after removing structural
    /// parameters into their independent custody namespace.
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Proof-only erased scalar formals in authored order, retaining their
    /// authored parameter positions. They own no runtime argument lane.
    pub erased_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
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
