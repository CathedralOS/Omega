//! Immutable optimization-unit aggregate and its exact carrier-family map.
//!
//! The one program root for the representation: the aggregate struct below
//! defines the current unit, and every concept area — construction,
//! identity, ledger, observation, evidence, rewrite vocabulary, and tests —
//! is subordinate to it.

use semantic_vocabulary::IntegerType;

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
    AcceptedObligationFactIndexError, ProofQuestionIndexError, attach_accepted_obligation_facts,
    attach_ownership_frontier_facts, attach_verified_module_context,
    canonical_ownership_frontier_snapshot,
};
pub use construction::{
    OptimizationUnitBuildError, operation_definition, reconstruct_psi_optimization_function_seed,
    reconstruct_psi_optimization_unit_seed,
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
    placed_view_catalog_identity_bytes, recompute_psi_optimization_unit_identity,
    scalar_refinement_identity_bytes,
};
pub use ledger::{
    InvalidPsiTransformationLedger, PrunedMachineCustody, PsiTransformationLedger,
    PsiTransformationRecord,
};
pub use manifest::{
    OptimizationManifestStage, OptimizationStructuralStatistics, PhysicalOptimizationDataStatus,
    PrePhysicalOptimizationManifest,
};
pub use observation::{
    ObservationKnowledge, PsiNodeObservation, reconstruct_psi_closed_region_observation,
    reconstruct_psi_observation_model,
};
pub use ownership::{
    OwnershipFrontierFact, OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace,
    OwnershipFrontierPartialCustody, OwnershipFrontierSite, OwnershipFrontierSnapshot,
};
pub use proof::{AcceptedObligationFact, ProofQuestionOwner, SourceCheckedScalarReceiving};
pub(crate) use proof::{ProofQuestion, ProofQuestionAdmissionKind, ProofQuestionClass};
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
    RedundantBlockParameterWitness, ScalarConstantValue, ScalarSubstitution, SccpBlockRow,
    SccpEdgeRow, SccpEdgeState, SccpMachineSnapshot, SccpValueRow, SccpValueState,
    SharedJumpFusionRewrite, SpecializedStateEdgeRow, StateArgumentSpecializationRewrite,
    TotalScalarIdentityKind, TotalScalarIdentityRewrite, UnreachablePrivateMachinesRewrite,
    derived_sccp_scalar_constant_fact_identity, encode_location,
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
    /// Identity text every `TerminalIdentityIndex` on this unit resolves
    /// against — the sealed module table, retained after the module itself
    /// is discarded.
    pub identities: terminal_psi::TerminalIdentityTable,
    pub functions: Vec<PsiOptimizationFunction>,
}

impl PsiOptimizationUnit {
    /// The optimization function for `machine`, when the unit carries it.
    pub fn function(&self, machine: MachineId) -> Option<&PsiOptimizationFunction> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

    /// The optimization function for `machine` for replay mutation, when the
    /// unit carries it.
    pub fn function_mut(&mut self, machine: MachineId) -> Option<&mut PsiOptimizationFunction> {
        self.functions
            .iter_mut()
            .find(|function| function.machine == machine)
    }

    /// The boundary machine keyed `id`, when the unit declares it.
    pub fn boundary_machine(
        &self,
        id: semantic_vocabulary::BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        self.boundary_machines
            .iter()
            .find(|declaration| declaration.id == id)
    }

    /// Every function's blocks, in function order.
    pub fn blocks(&self) -> impl Iterator<Item = &OptimizationBlock> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }

    /// Every function's nodes, in function then block order.
    pub fn block_nodes(&self) -> impl Iterator<Item = &OptimizationNode> + '_ {
        self.functions
            .iter()
            .flat_map(|function| function.block_nodes())
    }

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
    DispatchCondition, Establishment, FieldEvidence, FunctionAnalysis, Position, admit_field_node,
    argument_rewrites_place, bound_place, bound_value, declared_structural_type, dominators,
    established_case_at, field_evidence, function_analysis, integer_comparison_condition,
    observed_field_declaration, proof_basis, reachable_blocks_after_conditional_fold,
    resolved_sole_case, root_producer, scalar_value_definition, shape, sole_case_at_path,
    structural_child,
};

pub use integer_intervals::{IntervalExtraction, extract_integer_intervals, proof_range_goal};

/// Shared scalar-type tag encoding: the variant tag byte plus the caller's
/// per-kind payload encode.
pub(crate) fn encode_scalar_type<B>(
    bytes: &mut B,
    scalar_type: ScalarType,
    push_u8: impl Fn(&mut B, u8),
    encode_integer: impl FnOnce(&mut B, IntegerType),
) {
    match scalar_type {
        ScalarType::Boolean => push_u8(bytes, 1),
        ScalarType::Integer(integer) => {
            push_u8(bytes, 2);
            encode_integer(bytes, integer);
        }
        ScalarType::IeeeFloat(format) => {
            push_u8(bytes, 3);
            push_u8(
                bytes,
                match format {
                    semantic_vocabulary::IeeeFloatFormat::Binary32 => 1,
                    semantic_vocabulary::IeeeFloatFormat::Binary64 => 2,
                },
            );
        }
    }
}
