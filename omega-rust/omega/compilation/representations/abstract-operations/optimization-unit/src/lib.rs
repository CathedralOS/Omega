#![forbid(unsafe_code)]

/*
We represent the target-neutral optimization unit (`PsiOptimizationUnit`) and the
published abstract program (`OptimizedAbstractProgram`) that abstract optimization
produces from verified Terminal Psi realization requirements.

When Terminal Psi lowers into an `AbstractOperationPlan`, basic blocks, scalar SSA
definitions and uses, control-flow edges, semantic work settlements, effect links,
and ownership events live implicitly inside a flat operation stream. We project that
plan once into `PsiOptimizationUnit` so every optimization rule and independent
validator reads explicit `OptimizationBlock`, `OptimizationNode`, `OptimizationEdge`,
and `ValueDefinition` records without re-parsing instructions.

Our unit carries two distinct kinds of data:
1. Current-revision executable graph and derived dataflow (`functions`, `root_service_reach`,
   `pruned_machines`), which rewrites transform and re-seal under a fresh
   `OptimizationUnitIdentity`.
2. Immutable verifier custody (`structural_types`, `structural_domains`, `services`,
   `accepted_obligation_facts`, `ownership_frontier_facts`,
   `runtime_evidence`), which optimizer admission attaches once from the verified
   Terminal module and preserves across every rewrite.
*/

/*
We deliberately separate this representation crate from both rewrite execution and
independent semantic validation. Earlier designs mixed graph reconstruction, pass
state, and validation queries on a mutable instruction stream, which forced every
pass to rediscover CFG edges and allowed unvalidated rewrites to masquerade as
admitted units.

By keeping `PsiOptimizationUnit` pure data and sealing every revision, candidate
patch, transformation ledger, and pre-physical manifest with canonical byte-stream
identities, we ensure that proposing passes and independent validators communicate
solely through immutable, replayable records. A bare seed reconstructed without
verifier custody leaves `runtime_evidence` and the frontier/proof tables empty, so
downstream admission rejects unauthorized seeds immediately.
*/

/*
`PsiOptimizationUnit` is seeded by `reconstruct_psi_optimization_unit_seed` in
`construction`, enriched with verified Terminal custody through `proof` and
`ownership`, and checked by the sibling `optimization-unit-semantics` crate. Once
abstract optimization completes, we bundle the final unit, its `AbstractOperationPlan`,
the `PsiTransformationLedger`, and the `PrePhysicalOptimizationManifest` into
`OptimizedAbstractProgram` for target lowering (Omega 02) and backend emission.
*/

use std::{collections::BTreeSet, sync::Arc};

use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractSuccessor, ValueBinding,
};
use optimization_core::{
    AcceptedObligationFactIdentity, OptimizationIdentityBundle, OptimizationSelections,
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    OwnershipFrontierFactIdentity, ScalarConstantFactIdentity, ValueRangeFactIdentity,
};
use semantic_vocabulary::{
    BlockId, ClaimId, EdgeId, IntegerCarrier, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, ScalarType, ServiceId, StructuralPlaceKind, StructuralTypeId, ValueId,
    WorkScheduleIdentity,
};
use terminal_psi::{
    BoundaryMachineDeclaration, ContentEntryClaim, EntryClaim, EvidenceContractLane,
    MachineContract, ProviderCandidateConformance, ServiceDeclaration, StructuralDomainDeclaration,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, TerminalAffineCleanupAction, TerminalPsiIdentity,
    TerminalRootServiceReach,
};

pub(crate) use optimization_core::codec_bytes;

mod construction;
mod dominance;
mod graph;
mod identity;
mod ledger;
mod manifest;
mod observation;
mod ownership;
mod proof;
mod range;
mod rewrite;
mod structural_places;

pub use construction::{
    OptimizationUnitBuildError, operation_definition, operation_ownership, operation_uses,
    reconstruct_psi_optimization_function_seed, reconstruct_psi_optimization_unit_seed,
};
pub use dominance::{DominatorSet, DominatorSets};
pub use graph::{
    CycleComponentEdge, CycleComponentId, EffectLink, OptimizationBlock, OptimizationEdge,
    OptimizationFact, OptimizationNode, OptimizerCycleComponent, OptimizerCycleComponentSnapshot,
    OptimizerRankingCertificateSnapshot, OptimizerUnsignedCountdownRankingCertificate,
    OptimizerUnsignedMinusOneDescent, OptimizerUnsignedPositiveGuard, OwnershipEvent,
    PsiOptimizationFunction, PsiProvenance, ValueDefinition, ValueDefinitionSite, ValueUse,
    WorkSettlement, encode_value_definition_site_identity,
};
pub use identity::{
    access_view_catalog_identity_bytes, recompute_psi_optimization_unit_identity,
    scalar_refinement_identity_bytes,
};
pub use ledger::{
    AbstractOptimizationEvidence, InvalidPsiTransformationLedger, OptimizationRunUsage,
    PrunedMachineCustody, PsiOptimizationCommit, PsiTransformationLedger, PsiTransformationRecord,
    PsiValidatedCandidateDeclaration,
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
    OwnershipFrontierFact, OwnershipFrontierFactIndexError, OwnershipFrontierLiveClaim,
    OwnershipFrontierOwnedPlace, OwnershipFrontierPartialCustody, OwnershipFrontierSite,
    OwnershipFrontierSnapshot, attach_ownership_frontier_facts,
    canonical_ownership_frontier_snapshot,
};
pub use proof::{
    AcceptedObligationFact, AcceptedObligationFactIndexError, RuntimeEvidenceCatalog,
    SourceCheckedScalarReceiving, attach_accepted_obligation_facts, attach_verified_module_context,
    validate_runtime_evidence_context,
};
pub use range::{ValueRangeFact, ValueRangeRegion, ValueRangeSupport, value_range_fact_identity};
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
pub use structural_places::{
    DispatchCondition, Establishment, FieldEvidence, FunctionAnalysis, Position,
    argument_rewrites_place, bound_place, bound_value, declared_structural_type, dominators,
    established_case_at, field_evidence, field_value_accounting, field_value_reads,
    function_analysis, integer_comparison_condition, observed_field_declaration,
    operation_result_placement, proof_basis, reachable_blocks_after_conditional_fold,
    receiver_custody_distinct, resolved_sole_case, retirable_scalar_node, root_producer,
    scalar_value_definition, shape, sole_case_at_path, structural_child,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationUnit {
    pub identity: OptimizationUnitIdentity,
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub entry: MachineId,
    pub structural_types: abstract_operations::StructuralTypeCatalog, // Retained after the full Terminal module is discarded.
    pub structural_domains: Arc<[StructuralDomainDeclaration]>, // Empty on bare lowering seeds; attached before rewrites run.
    pub services: Arc<[ServiceDeclaration]>, // Complete boundary-service hierarchy for call reach and effect replay.
    pub root_service_reach: TerminalRootServiceReach, // Current-revision closure of services executable from `entry`.
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub dynamic_dispatch: terminal_psi::TerminalDynamicDispatchCatalog,
    pub callables: terminal_psi::TerminalCallableCatalog,
    pub accepted_obligation_facts: Vec<AcceptedObligationFact>,
    pub ownership_frontier_facts: Vec<OwnershipFrontierFact>, // Absent only on low-level bare seeds.
    pub pruned_machines: Vec<PrunedMachineCustody>, // Source ordinals bind removed machines to the verified Terminal roster.
    pub identities: terminal_psi::TerminalIdentityTable, // Sealed module table that every `TerminalIdentityIndex` resolves against.
    pub runtime_evidence: Option<Arc<RuntimeEvidenceCatalog>>, // Present on all verified units; `None` only on bare seeds.
    pub functions: Vec<PsiOptimizationFunction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallMachineReference {
    Declaration(MachineId),
    Callable {
        callee: semantic_vocabulary::ValueId,
        contract: semantic_vocabulary::CallableContractId,
    },
}

impl PsiOptimizationUnit {
    pub fn function(&self, machine: MachineId) -> Option<&PsiOptimizationFunction> {
        self.function_index(machine)
            .map(|index| &self.functions[index])
    }

    pub fn function_mut(&mut self, machine: MachineId) -> Option<&mut PsiOptimizationFunction> {
        self.function_index(machine)
            .map(|index| &mut self.functions[index])
    }

    pub fn function_index(&self, machine: MachineId) -> Option<usize> {
        self.functions
            .binary_search_by_key(&machine, |function| function.machine)
            .ok()
    }

    pub fn boundary_machine(
        &self,
        id: semantic_vocabulary::BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        self.boundary_machines
            .iter()
            .find(|declaration| declaration.id == id)
    }

    pub fn blocks(&self) -> impl Iterator<Item = &OptimizationBlock> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }

    pub fn block_nodes(&self) -> impl Iterator<Item = &OptimizationNode> + '_ {
        self.functions
            .iter()
            .flat_map(|function| function.block_nodes())
    }

    pub fn call_machine_references<'a>(
        &'a self,
        operation: &'a AbstractOperation,
    ) -> impl Iterator<Item = CallMachineReference> + 'a {
        let (machine, dynamic, callable) = match operation {
            AbstractOperation::Call { callee, .. }
            | AbstractOperation::InvokeNoNormal { callee, .. } => (Some(*callee), None, None),
            AbstractOperation::CallStructural { callee, .. } => match callee {
                abstract_operations::AbstractCallTarget::Machine(machine) => {
                    (Some(*machine), None, None)
                }
                abstract_operations::AbstractCallTarget::Dynamic {
                    interface, slot, ..
                } => (None, Some((*interface, *slot)), None),
                abstract_operations::AbstractCallTarget::Callable { callee, contract } => (
                    None,
                    None,
                    Some(CallMachineReference::Callable {
                        callee: *callee,
                        contract: *contract,
                    }),
                ),
            },
            _ => (None, None, None),
        };
        machine
            .into_iter()
            .chain(
                self.dynamic_dispatch
                    .tables
                    .iter()
                    .filter_map(move |table| {
                        let (interface, slot) = dynamic?;
                        (table.interface == interface).then_some(())?;
                        *table.rows.get(usize::try_from(slot).ok()?)?
                    }),
            )
            .map(CallMachineReference::Declaration)
            .chain(callable)
    }

    pub fn operation_machine_references<'a>(
        &'a self,
        operation: &'a AbstractOperation,
    ) -> impl Iterator<Item = MachineId> + 'a {
        let (machine, table, interface) = match operation {
            AbstractOperation::EstablishNativeCodeLayout { machine, .. } => {
                (Some(*machine), None, None)
            }
            AbstractOperation::Call { callee, .. } => (Some(*callee), None, None),
            AbstractOperation::CallStructural { callee, .. } => match callee {
                abstract_operations::AbstractCallTarget::Machine(machine) => {
                    (Some(*machine), None, None)
                }
                abstract_operations::AbstractCallTarget::Dynamic { interface, .. } => {
                    (None, None, Some(*interface))
                }
                // Runtime code has no nominal reference. Its construction
                // retains the selected body through SelectMachine below.
                abstract_operations::AbstractCallTarget::Callable { .. } => (None, None, None),
            },
            AbstractOperation::SelectMachine { descriptor, .. } => (
                self.callables
                    .descriptor(*descriptor)
                    .map(|row| row.machine),
                None,
                None,
            ),
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

#[derive(Debug)]
pub struct OptimizedAbstractProgram {
    pub plan: Arc<AbstractOperationPlan>,
    pub unit: PsiOptimizationUnit,
    pub selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
    pub identity_bundle: OptimizationIdentityBundle,
    pub projection: OptimizedAbstractPlanProjectionIdentity, // Validated projection identity binding the Terminal program to `final_unit`.
    pub projection_psi: TerminalPsiIdentity,
    pub final_unit: OptimizationUnitIdentity,
    pub pre_physical_manifest: PrePhysicalOptimizationManifest,
}

impl OptimizedAbstractProgram {
    pub fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }

    pub fn shared_program(&self) -> Arc<AbstractOperationPlan> {
        Arc::clone(&self.plan)
    }

    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }

    pub const fn identity_bundle(&self) -> OptimizationIdentityBundle {
        self.identity_bundle
    }

    pub const fn projection(&self) -> OptimizedAbstractPlanProjectionIdentity {
        self.projection
    }

    pub const fn projection_psi(&self) -> TerminalPsiIdentity {
        self.projection_psi
    }

    pub const fn final_unit(&self) -> OptimizationUnitIdentity {
        self.final_unit
    }

    pub const fn pre_physical_manifest(&self) -> &PrePhysicalOptimizationManifest {
        &self.pre_physical_manifest
    }
}

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
        ScalarType::Callable(contract) => {
            push_u8(bytes, 4);
            for byte in contract.get().to_le_bytes() {
                push_u8(bytes, byte);
            }
        }
    }
}

pub(crate) use optimization_core::cursor::TakeError;

pub(crate) struct Cursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> Cursor<'encoded> {
    pub(crate) const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    pub(crate) fn take(&mut self, length: usize) -> Result<&'encoded [u8], TakeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(TakeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(TakeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], TakeError> {
        self.take(N)?.try_into().map_err(|_| TakeError::Truncated)
    }

    pub(crate) fn byte(&mut self) -> Result<u8, TakeError> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, TakeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, TakeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, TakeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(crate) fn length(&mut self) -> Result<usize, TakeError> {
        usize::try_from(self.u64()?).map_err(|_| TakeError::LengthOverflow)
    }

    pub(crate) fn length_prefixed(&mut self) -> Result<&'encoded [u8], TakeError> {
        let length = self.length()?;
        self.take(length)
    }

    pub(crate) fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
