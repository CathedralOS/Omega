//! Immutable optimization-unit aggregate and its exact carrier-family map.
//!
//! The one program root for the representation: the aggregate struct below
//! defines the current unit, and every concept area — construction,
//! identity, ledger, observation, evidence, rewrite vocabulary, and tests —
//! is subordinate to it.

use super::{
    AbstractFunction, AbstractOperation, AbstractOperationPlan, AbstractSuccessor, Arc, BTreeSet,
    BlockId, BoundaryMachineDeclaration, FuelScheduleIdentity, MachineId, ObligationId,
    OperationId, OptimizationUnitIdentity, PlaceId, ProviderCandidateConformance, ScalarType,
    ServiceDeclaration, StructuralDomainDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, TerminalPsiIdentity, TerminalRootServiceReach, ValueId,
};
mod attachment;
mod construction;
mod cycles;
mod evidence;
mod graph;
mod identity;
mod integer_intervals;
mod ledger;
mod manifest;
mod observation;
mod ownership;
mod proof;
mod range;
mod rewrite;
mod structural_places;

pub use attachment::{
    AcceptedObligationFactIndexError, OwnershipFrontierFactIndexError, ProofQuestionIndexError,
    attach_accepted_obligation_facts, attach_ownership_frontier_facts, attach_proof_questions,
    attach_verified_module_context, canonical_ownership_frontier_snapshot,
};
pub use construction::{
    OptimizationUnitBuildError, reconstruct_psi_optimization_function_seed,
    reconstruct_psi_optimization_unit_seed, restamp_psi_function_derived_metadata,
};
pub use cycles::{
    CycleComponentEdge, CycleComponentId, OptimizerCycleComponent, OptimizerCycleComponentSnapshot,
    OptimizerRankingCertificateSnapshot, OptimizerUnsignedCountdownRankingCertificate,
    OptimizerUnsignedMinusOneDescent, OptimizerUnsignedPositiveGuard,
};
pub use evidence::{
    AbstractOptimizationEvidence, OptimizationRunUsage, PsiOptimizationCommit,
    PsiValidatedCandidateDeclaration,
};
pub use graph::{
    EffectLink, FuelSettlement, OptimizationBlock, OptimizationEdge, OptimizationFact,
    OptimizationNode, OwnershipEvent, PsiOptimizationFunction, PsiProvenance, ValueDefinition,
    ValueDefinitionSite, ValueUse, encode_value_definition_site_identity,
};
pub use identity::{
    recompute_psi_optimization_unit_identity, scalar_refinement_identity_bytes,
    structural_domain_catalog_identity,
};
pub use ledger::{
    InvalidPsiTransformationLedger, PrunedMachineCustody, PsiTransformationLedger,
    PsiTransformationLedgerDecodeError, PsiTransformationRecord,
};
pub use manifest::{
    OptimizationManifestStage, OptimizationStructuralStatistics, PhysicalOptimizationDataStatus,
    PrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestDecodeError,
};
pub use observation::{
    ObservationEventClass, ObservationKnowledge, PsiClosedRegionBlockObservation,
    PsiClosedRegionObservation, PsiClosedRegionSemantics, PsiNodeObservation, PsiObservableEvent,
    PsiObservationModel, PsiRegionBoundaryEdgeObservation, PsiRegionFrontierObservation,
    reconstruct_psi_closed_region_observation, reconstruct_psi_observation_model,
};
pub use ownership::{
    OwnershipFrontierFact, OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace,
    OwnershipFrontierPartialCustody, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    ownership_frontier_fact_identity,
};
pub use proof::{
    AcceptedObligationFact, ProofQuestion, ProofQuestionAdmissionKind, ProofQuestionClass,
    ProofQuestionOwner, accepted_obligation_fact_identity, proof_question_identity,
};
pub use range::{
    ValueRangeFact, ValueRangeRegion, ValueRangeScope, ValueRangeSupport, value_range_fact_identity,
};
pub use rewrite::{
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, BooleanConstantRewrite,
    CaseMembershipSpecializationRewrite, ConstantConditionalRewrite, DeadScalarNodeRewrite,
    DominatingScalarCommonSubexpressionRewrite, FieldValueResolution, FieldValueRow,
    FieldValueSpecializationRewrite, FoldedCaseMembershipRow, FoldedFieldValue,
    ForwardedFieldValue, IntegerConstantRewrite, IntegerEvaluationWitness, LinearEmptyBlockRewrite,
    LocalScalarCommonSubexpressionRewrite, LoopInvariantNodeResult,
    LoopInvariantScalarMotionCandidate, LoopInvariantScalarNode, LoopInvariantScalarRelocation,
    NodeLocation, NonAdjacentBlockMergeRewrite, OwnershipFrontierWitness,
    OwnershipFrontierWitnessRow, PathQualifiedEmptyBlockRewrite, PhiTranslatedScalarGvnRewrite,
    PhiTranslatedScalarIncoming, ProofCertifiedScalarIdentityKind,
    ProofCertifiedScalarIdentityRewrite, ProvenanceDisposition, ProvenanceRewrite,
    PsiCandidateDeclaration, PsiRealizationSite, PsiRewriteCandidate, PsiRewriteCandidateError,
    PsiRewriteDecisionPoint, PsiRewritePatch, RedundantBlockParameterRewrite,
    RedundantBlockParameterWitness, ScalarConstantValue, ScalarEvaluationWitness,
    ScalarSubstitution, SccpBlockRow, SccpEdgeRow, SccpEdgeState, SccpMachineSnapshot,
    SccpValueRow, SccpValueState, SharedJumpFusionRewrite, SpecializedStateEdgeRow,
    StateArgumentSpecializationRewrite, TotalScalarIdentityKind, TotalScalarIdentityRewrite,
    UnreachablePrivateMachinesRewrite, derived_sccp_scalar_constant_fact_identity,
    literal_scalar_constant_fact_identity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationUnit {
    pub identity: OptimizationUnitIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub entry: MachineId,
    /// Target-neutral module declarations needed by layout, ABI, and checked
    /// provider installation after the full Terminal module is discarded.
    pub structural_types: abstract_operations::StructuralTypeCatalog,
    /// Exact verifier-owned qualification-domain catalog. Bare lowering seeds
    /// leave this empty; optimizer admission attaches it before rewrites run.
    pub structural_domains: Arc<[StructuralDomainDeclaration]>,
    /// Exact verifier-owned boundary-service hierarchy. Bare lowering seeds
    /// leave this empty; optimizer admission attaches the complete catalog so
    /// call reach and concrete service effects remain independently replayable.
    pub services: Arc<[ServiceDeclaration]>,
    /// Exact current-revision closure of services executable from `entry`.
    /// Unlike declaration custody, this derived row may narrow when a checked
    /// rewrite removes an unreachable call or concrete service effect.
    pub root_service_reach: TerminalRootServiceReach,
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub dynamic_dispatch: terminal_psi::TerminalDynamicDispatchCatalog,
    pub accepted_obligation_facts: Vec<AcceptedObligationFact>,
    /// Complete immutable verifier proof-question roster in reconstruction
    /// order. This is source-site authority, not a function-wide range index.
    pub proof_questions: Vec<ProofQuestion>,
    /// Immutable verifier projection, absent only on low-level bare seeds that
    /// are not authorized optimizer inputs.
    pub ownership_frontier_facts: Vec<OwnershipFrontierFact>,
    /// Canonical custody for source functions removed by independently proven
    /// whole-program reachability rewrites. Source ordinals bind each removed
    /// machine to the immutable verified Terminal-Psi function roster.
    pub pruned_machines: Vec<PrunedMachineCustody>,
    pub functions: Vec<PsiOptimizationFunction>,
}

impl PsiOptimizationUnit {
    /// Possible invoked machines, separate from rows retained as table data.
    /// Formation does not invoke a machine, and a dynamic call invokes only
    /// its requirement slot across the interface's complete table alternatives.
    pub fn call_machine_references<'a>(
        &'a self,
        operation: &'a AbstractOperation,
    ) -> impl Iterator<Item = MachineId> + 'a {
        let (machine, dynamic) = match operation {
            AbstractOperation::Call { callee, .. } => (Some(*callee), None),
            AbstractOperation::CallStructural { callee, .. } => match callee {
                abstract_operations::AbstractCallTarget::Machine(machine) => (Some(*machine), None),
                abstract_operations::AbstractCallTarget::Dynamic {
                    interface, slot, ..
                } => (None, Some((*interface, *slot))),
            },
            _ => (None, None),
        };
        machine.into_iter().chain(
            self.dynamic_dispatch
                .tables
                .iter()
                .filter_map(move |table| {
                    let (interface, slot) = dynamic?;
                    (table.interface == interface).then_some(())?;
                    *table.rows.get(usize::try_from(slot).ok()?)?
                }),
        )
    }

    /// Exact machine references retained by an operation's callable selection.
    /// Tables retain every executable row, including uncalled slots; a dynamic
    /// operand can name any table of its interface, independent of local flow.
    pub fn operation_machine_references<'a>(
        &'a self,
        operation: &'a AbstractOperation,
    ) -> impl Iterator<Item = MachineId> + 'a {
        let (machine, table, interface) = match operation {
            AbstractOperation::Call { callee, .. } => (Some(*callee), None, None),
            AbstractOperation::CallStructural { callee, .. } => match callee {
                abstract_operations::AbstractCallTarget::Machine(machine) => {
                    (Some(*machine), None, None)
                }
                abstract_operations::AbstractCallTarget::Dynamic { interface, .. } => {
                    (None, None, Some(*interface))
                }
            },
            AbstractOperation::SelectImplementation { table, .. } => (None, Some(*table), None),
            _ => (None, None, None),
        };
        machine.into_iter().chain(
            self.dynamic_dispatch
                .tables
                .iter()
                .filter(move |candidate| {
                    table == Some(candidate.id) || interface == Some(candidate.interface)
                })
                .flat_map(|table| table.rows.iter().copied().flatten()),
        )
    }
}
pub use structural_places::{
    DispatchCondition, Establishment, EstablishmentScalar, FieldEvidence, FunctionAnalysis,
    IntegerComparisonOrdering, ObservedFieldKind, Position, ProvenFieldValue, RootProducer,
    bound_value, canonicalize_ordinary_path, case_field_initializer, comparison_operands,
    constant_definition, declared_structural_type, descended_field, descended_field_id,
    descended_field_matching, descended_type, dominators, establishment_fields,
    establishment_producer, field_evidence, function_analysis, integer_comparison_condition,
    matches_constant_kind, matches_observed_kind, observed_field_declaration, record_initializer,
    root_producer, shape, structural_child, zeroed_fold,
};

pub use integer_intervals::{
    IntervalExtraction, PartialIntegerBounds, extract_integer_intervals, proof_range_goal,
};
