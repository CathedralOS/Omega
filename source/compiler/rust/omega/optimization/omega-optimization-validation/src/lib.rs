#![forbid(unsafe_code)]

//! Independent structural validation for [`PsiOptimizationUnit`].
//!
//! Pass implementations do not participate in this validator. Publication
//! must call it after applying a candidate and before committing the candidate
//! to the durable transformation ledger.

use std::collections::{BTreeMap, BTreeSet};

use omega_optimization_core::{
    AnalysisKind, OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationSafetyClass,
    OptimizationUnitIdentity, OptimizationValidatorIdentity,
};
use omega_optimization_unit::{
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, BooleanConstantRewrite,
    ConstantConditionalRewrite, DeadScalarNodeRewrite, IntegerConstantRewrite,
    IntegerEvaluationWitness, NodeLocation, OptimizationEdge, OptimizationFact, OwnershipEvent,
    OwnershipFrontierFact, OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace,
    OwnershipFrontierPartialCustody, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    ProvenanceDisposition, PsiNodeObservation, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiProvenance, PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch,
    RedundantBlockParameterRewrite, ScalarConstantValue, ScalarSubstitution, SccpBlockRow,
    SccpEdgeRow, SccpEdgeState, SccpMachineSnapshot, SccpValueRow, SccpValueState,
    SharedTerminalJumpFusionRewrite, ValueDefinition, ValueDefinitionSite, ValueUse,
    canonical_ownership_frontier_snapshot, derived_sccp_scalar_constant_fact_identity,
    literal_scalar_constant_fact_identity, recompute_psi_optimization_unit_identity,
    reconstruct_psi_closed_region_observation, reconstruct_psi_observation_model,
};
use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, IntegerCarrier, IntegerType, MachineId, PlaceId,
    ScalarType, ValueId,
};
use psi_terminal_fuel::TerminalFuelSchedule;

mod prephysical_manifest;
mod projection;

pub use prephysical_manifest::{
    OptimizationManifestStage, OptimizationStructuralStatistics, PhysicalOptimizationDataStatus,
    PrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestDecodeError,
    PrePhysicalOptimizationManifestError, ValidatedPrePhysicalOptimizationManifest,
    project_pre_physical_optimization_manifest, validate_pre_physical_optimization_manifest,
};
pub use projection::{
    OptimizedAbstractPlanProjectionError, ValidatedOptimizedAbstractPlanProjection,
    validate_optimized_abstract_plan_projection,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationUnitValidationError {
    ContentIdentityMismatch {
        stored: OptimizationUnitIdentity,
        recomputed: OptimizationUnitIdentity,
    },
    WrongFuelSchedule,
    EntryClaimIndexMismatch(MachineId),
    FunctionResultMismatch(MachineId),
    MissingEntryMachine(MachineId),
    DuplicateMachine(MachineId),
    NonCanonicalPrunedMachineRoster,
    ActivePrunedMachineOverlap(MachineId),
    PrunedEntryMachine(MachineId),
    PrunedProviderMachine(MachineId),
    DuplicateBoundaryMachine(BoundaryMachineId),
    ScalarOperationContractMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    MissingEntryBlock {
        machine: MachineId,
        block: BlockId,
    },
    EntryBlockHasParameters {
        machine: MachineId,
        block: BlockId,
    },
    DuplicateBlock {
        machine: MachineId,
        block: BlockId,
    },
    EmptyBlock {
        machine: MachineId,
        block: BlockId,
    },
    TerminatorNotLast {
        machine: MachineId,
        block: BlockId,
    },
    MissingTerminator {
        machine: MachineId,
        block: BlockId,
    },
    UnknownSuccessor {
        machine: MachineId,
        block: BlockId,
        target: BlockId,
    },
    UnreachableBlock {
        machine: MachineId,
        block: BlockId,
    },
    ControlCycle {
        machine: MachineId,
        block: BlockId,
    },
    ParameterMetadataMismatch {
        machine: MachineId,
        block: Option<BlockId>,
    },
    DuplicateEdge(EdgeId),
    DuplicateProvenance(PsiProvenance),
    CoExecutableProvenanceOccurrences(PsiProvenance),
    IncompleteProvenance {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    FuelDoesNotMatchProvenance {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    DuplicateFuelSettlement(PsiProvenance),
    OperationMetadataMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    FactIndexMismatch(MachineId),
    BrokenEffectChain {
        machine: MachineId,
        expected: u64,
        actual: u64,
    },
    DuplicateValue(ValueId),
    UndefinedValue {
        machine: MachineId,
        block: BlockId,
        value: ValueId,
    },
    NondominatingValue {
        machine: MachineId,
        block: BlockId,
        value: ValueId,
    },
    UseBeforeDefinition {
        machine: MachineId,
        block: BlockId,
        value: ValueId,
    },
    BindingArityMismatch {
        machine: MachineId,
        edge: EdgeId,
    },
    BindingTypeMismatch {
        machine: MachineId,
        edge: EdgeId,
        value: ValueId,
    },
    UnknownPlace {
        machine: MachineId,
        place: PlaceId,
    },
    UnknownClaim {
        machine: MachineId,
        claim: ClaimId,
    },
    TerminalIdentityMismatch,
    ProofFingerprintMismatch,
    AcceptedObligationMismatch(psi_core::ObligationId),
    OperationObligationOwnerMismatch {
        machine: MachineId,
        operation: psi_core::OperationId,
        obligation: psi_core::ObligationId,
    },
    AcceptedObligationFactIndexMismatch,
    OwnershipFrontierFactIndexMismatch,
    CandidateAcceptedObligationFactMismatch,
    MissingStructuralFrontierMachine(MachineId),
    MissingStructuralOperationFrontier {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    MissingStructuralEdgeFrontier {
        machine: MachineId,
        edge: EdgeId,
    },
    ContextIdentity(psi_terminal_codec::CodecError),
    ContextProofFingerprint(psi_terminal_codec::ProofCodecError),
    VerifiedOptimizationUnitProjectionMismatch,
    CandidateInputMismatch,
    CandidateAnalysisContractMismatch,
    CandidateSafetyClassMismatch,
    CandidateLocationMissing,
    CandidatePatchMismatch,
    CandidateProvenanceMismatch,
    CandidateFuelMismatch,
    CandidateOperandFactMismatch,
    CandidateEvaluationMismatch,
    CandidateObservationMismatch,
    CandidateLiveBoundaryMismatch,
    CandidateRegionObservationUnavailable,
    CandidateRegionObservationMismatch,
    CandidateReachabilityMismatch,
    CandidateOutsideRegionMismatch,
    CandidateBlockParameterMismatch,
    CandidateIncomingBindingMismatch,
    CandidateSubstitutionMismatch,
}

impl std::fmt::Display for OptimizationUnitValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Psi optimization unit: {self:?}")
    }
}

impl std::error::Error for OptimizationUnitValidationError {}

/// Independently reconstructed scalar interface of one closed node region.
/// Canonical ordering is by `ValueId`; block-parameter bindings remain uses of
/// the predecessor terminator and therefore participate naturally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedScalarObservationBoundary {
    pub location: NodeLocation,
    pub live_in: Vec<ValueId>,
    pub live_out: Vec<ValueId>,
}

pub fn reconstruct_closed_scalar_node_boundary(
    unit: &PsiOptimizationUnit,
    location: NodeLocation,
) -> Option<ClosedScalarObservationBoundary> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == location.machine)?;
    let mut live_entry = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut live_exit = live_entry.clone();
    loop {
        let mut changed = false;
        for block in function.blocks.iter().rev() {
            let next_exit = block
                .nodes
                .last()
                .into_iter()
                .flat_map(|node| &node.successors)
                .filter_map(|edge| live_entry.get(&edge.target))
                .flat_map(|values| values.iter().copied())
                .collect::<BTreeSet<_>>();
            let mut next_entry = next_exit.clone();
            for node in block.nodes.iter().rev() {
                for definition in &node.definitions {
                    next_entry.remove(&definition.value);
                }
                next_entry.extend(node.uses.iter().map(|use_site| use_site.value));
            }
            for parameter in &block.parameters {
                next_entry.remove(&parameter.value);
            }
            if live_exit[&block.id] != next_exit {
                live_exit.insert(block.id, next_exit);
                changed = true;
            }
            if live_entry[&block.id] != next_entry {
                live_entry.insert(block.id, next_entry);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let block = function
        .blocks
        .iter()
        .find(|block| block.id == location.block)?;
    let target = usize::try_from(location.node).ok()?;
    if target >= block.nodes.len() {
        return None;
    }
    let mut live = live_exit[&block.id].clone();
    for (node_index, node) in block.nodes.iter().enumerate().rev() {
        let live_out = live.clone();
        for definition in &node.definitions {
            live.remove(&definition.value);
        }
        live.extend(node.uses.iter().map(|use_site| use_site.value));
        if node_index == target {
            return Some(ClosedScalarObservationBoundary {
                location,
                live_in: live.iter().copied().collect(),
                live_out: live_out.iter().copied().collect(),
            });
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPsiRewrite {
    unit: PsiOptimizationUnit,
    candidate: OptimizationCandidateIdentity,
    validator: OptimizationValidatorIdentity,
    provenance: Vec<omega_optimization_unit::ProvenanceRewrite>,
}

impl ValidatedPsiRewrite {
    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub const fn candidate(&self) -> OptimizationCandidateIdentity {
        self.candidate
    }

    pub const fn validator(&self) -> OptimizationValidatorIdentity {
        self.validator
    }

    /// Validator-accepted source disposition and fuel accounting. Consumers
    /// must ledger this value rather than re-reading the proposal.
    pub fn provenance(&self) -> &[omega_optimization_unit::ProvenanceRewrite] {
        &self.provenance
    }

    pub fn into_unit(self) -> PsiOptimizationUnit {
        self.unit
    }
}

pub fn validate_psi_optimization_unit(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    let recomputed = recompute_psi_optimization_unit_identity(unit);
    if unit.identity != recomputed {
        return Err(OptimizationUnitValidationError::ContentIdentityMismatch {
            stored: unit.identity,
            recomputed,
        });
    }
    if unit.fuel_schedule != TerminalFuelSchedule::CURRENT.identity() {
        return Err(OptimizationUnitValidationError::WrongFuelSchedule);
    }
    if unit
        .accepted_obligation_facts
        .iter()
        .any(|fact| fact.terminal_psi != unit.terminal_psi || !fact.has_canonical_identity())
        || unit.accepted_obligation_facts.windows(2).any(|pair| {
            (pair[0].machine, pair[0].operation, pair[0].obligation)
                >= (pair[1].machine, pair[1].operation, pair[1].obligation)
        })
    {
        return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
    }
    if unit.ownership_frontier_facts.iter().any(|fact| {
        fact.terminal_psi != unit.terminal_psi
            || !fact.has_canonical_identity()
            || !canonical_ownership_frontier_snapshot(&fact.snapshot)
    }) || unit
        .ownership_frontier_facts
        .windows(2)
        .any(|pair| (pair[0].machine, pair[0].site) >= (pair[1].machine, pair[1].site))
    {
        return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
    }
    let mut machines = BTreeMap::new();
    for function in &unit.functions {
        if machines.insert(function.machine, function).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateMachine(
                function.machine,
            ));
        }
    }
    if unit
        .pruned_machines
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(OptimizationUnitValidationError::NonCanonicalPrunedMachineRoster);
    }
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|custody| custody.machine)
        .collect::<BTreeSet<_>>();
    if pruned.len() != unit.pruned_machines.len() {
        return Err(OptimizationUnitValidationError::NonCanonicalPrunedMachineRoster);
    }
    if let Some(machine) = machines
        .keys()
        .find(|machine| pruned.contains(machine))
        .copied()
    {
        return Err(OptimizationUnitValidationError::ActivePrunedMachineOverlap(
            machine,
        ));
    }
    if pruned.contains(&unit.entry) {
        return Err(OptimizationUnitValidationError::PrunedEntryMachine(
            unit.entry,
        ));
    }
    if let Some(machine) = unit
        .provider_candidates
        .iter()
        .map(|candidate| candidate.candidate)
        .find(|machine| pruned.contains(machine))
    {
        return Err(OptimizationUnitValidationError::PrunedProviderMachine(
            machine,
        ));
    }
    if unit
        .accepted_obligation_facts
        .iter()
        .any(|fact| !machines.contains_key(&fact.machine) && !pruned.contains(&fact.machine))
    {
        return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
    }
    let mut boundary_machines = BTreeMap::new();
    for boundary in &unit.boundary_machines {
        if boundary_machines.insert(boundary.id, boundary).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateBoundaryMachine(
                boundary.id,
            ));
        }
    }
    for function in &unit.functions {
        validate_function(function, &machines, &boundary_machines)?;
    }
    for fact in &unit.ownership_frontier_facts {
        if unit
            .functions
            .iter()
            .find(|function| function.machine == fact.machine)
            .is_none()
            && !pruned.contains(&fact.machine)
        {
            return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
        }
    }
    if !machines.contains_key(&unit.entry) {
        return Err(OptimizationUnitValidationError::MissingEntryMachine(
            unit.entry,
        ));
    }
    Ok(())
}

/// Independently check and construct one integer-evaluation rewrite.
/// The proposing rule never receives a mutable unit and cannot construct the
/// accepted output itself.
pub fn validate_integer_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ScalarConstants)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(usize::try_from(patch.location.node).expect("u32 fits usize"))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let input_observation = observation_at(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [provenance] = candidate.provenance() else {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    };
    let site = PsiRealizationSite::Node(patch.location);
    if provenance.input != site
        || provenance.disposition != ProvenanceDisposition::RealizedAt(site)
        || provenance.sources != node.provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if provenance.fuel != node.fuel {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }
    let accepted_provenance = vec![omega_optimization_unit::ProvenanceRewrite {
        input: site,
        disposition: ProvenanceDisposition::RealizedAt(site),
        sources: node.provenance.clone(),
        fuel: node.fuel.clone(),
    }];

    let (source_operation, result, scalar_type, evaluated, safety_class) =
        evaluate_integer_operation(function, node, candidate)?;
    if candidate.safety_class() != safety_class {
        return Err(OptimizationUnitValidationError::CandidateSafetyClassMismatch);
    }
    match (
        safety_class,
        candidate
            .scalar_evaluation_witness()
            .and_then(IntegerEvaluationWitness::obligation_fact),
    ) {
        (OptimizationSafetyClass::ProofCertified, Some(identity)) => {
            let fact = input
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.identity == identity
                        && fact.machine == function.machine
                        && fact.operation == source_operation
                })
                .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if !function.facts.iter().any(|reference| {
                matches!(
                    reference,
                    OptimizationFact::OperationObligationReference { obligation, support }
                        if *support == source_operation && *obligation == fact.obligation
                )
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
        }
        (OptimizationSafetyClass::ProofCertified, None) | (_, Some(_)) => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
        (_, None) => {}
    }
    if patch
        != (IntegerConstantRewrite {
            location: patch.location,
            source_operation,
            result,
            scalar_type,
            constant: evaluated,
        })
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }
    let mut output = input.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate source function exists");
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate source block exists");
    let node = &mut block.nodes[usize::try_from(patch.location.node).expect("u32 fits usize")];
    node.operation =
        omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerConstant {
            psi_operation: patch.source_operation,
            result: patch.result,
            scalar_type: ScalarType::Integer(patch.scalar_type),
            value: patch.constant,
        };
    node.definitions = vec![ValueDefinition {
        value: patch.result,
        scalar_type: ScalarType::Integer(patch.scalar_type),
        site: ValueDefinitionSite::Node {
            block: patch.location.block,
            node: patch.location.node,
        },
    }];
    node.uses.clear();
    node.successors.clear();
    node.ownership.clear();
    function.facts = reconstruct_fact_index(function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_observation = observation_at(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !same_closed_scalar_observation(&input_observation, &output_observation) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let output_live = reconstruct_closed_scalar_node_boundary(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if input_live.live_out != output_live.live_out
        || output_live
            .live_in
            .iter()
            .any(|value| !input_live.live_in.contains(value))
    {
        return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.exact-integer-evaluation.v2",
        ),
        provenance: accepted_provenance,
    })
}

/// Dispatch one typed scalar-constant candidate to its independent validator.
pub fn validate_scalar_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    match candidate.patch() {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(_) => {
            validate_integer_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
            validate_boolean_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::FoldConstantConditional(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::MergeAdjacentBlock(_)
        | PsiRewritePatch::FuseSharedTerminalJump(_)
        | PsiRewritePatch::RemoveDeadScalarNode(_)
        | PsiRewritePatch::PruneUnreachablePrivateMachines(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
    }
}

/// Dispatch one closed Psi rewrite candidate to a patch-specific independent
/// validator. Rules cannot construct accepted outputs themselves.
pub fn validate_psi_rewrite_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    match candidate.patch() {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(_)
        | PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
            validate_scalar_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(_) => {
            validate_redundant_block_parameter_candidate(input, candidate)
        }
        PsiRewritePatch::FoldConstantConditional(_) => {
            validate_constant_conditional_candidate(input, candidate)
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(_) => {
            validate_linear_empty_block_candidate(input, candidate)
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(_) => {
            validate_path_qualified_empty_block_candidate(input, candidate)
        }
        PsiRewritePatch::MergeAdjacentBlock(_) => {
            validate_adjacent_block_merge_candidate(input, candidate)
        }
        PsiRewritePatch::FuseSharedTerminalJump(_) => {
            validate_shared_terminal_jump_fusion_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveDeadScalarNode(_) => {
            validate_dead_scalar_node_candidate(input, candidate)
        }
        PsiRewritePatch::PruneUnreachablePrivateMachines(_) => {
            validate_unreachable_private_machines_candidate(input, candidate)
        }
    }
}

/// Independently reconstruct the complete executable-machine root closure and
/// remove its exact active complement. Proof/frontier catalogs remain immutable
/// historical custody; only executable function bodies leave the active roster.
pub fn validate_unreachable_private_machines_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::CallGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::CallGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
        || !candidate.affected_blocks().is_empty()
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let omega_optimization_unit::PsiRewriteDecisionPoint::MachineSet(decision_machines) =
        candidate.decision_point()
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let PsiRewritePatch::PruneUnreachablePrivateMachines(patch) = candidate.patch_ref() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let expected_machines = unreachable_private_machine_complement(input);
    let patch_machines = patch
        .machines
        .iter()
        .map(|row| row.machine)
        .collect::<Vec<_>>();
    if expected_machines.is_empty()
        || *decision_machines != expected_machines
        || patch_machines != expected_machines
        || candidate.affected_machines() != expected_machines
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let source_ordinals = validator_active_source_ordinals(input);
    let expected_custody = expected_machines
        .iter()
        .map(|machine| omega_optimization_unit::PrunedMachineCustody {
            machine: *machine,
            source_ordinal: source_ordinals[machine],
        })
        .collect::<Vec<_>>();
    if patch.machines != expected_custody {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let expected_provenance = pruned_machine_provenance(input, &expected_machines)
        .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.provenance() != expected_provenance {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let removed = expected_machines.iter().copied().collect::<BTreeSet<_>>();
    let mut output = input.clone();
    output
        .functions
        .retain(|function| !removed.contains(&function.machine));
    output.pruned_machines.extend(expected_custody);
    output.pruned_machines.sort_unstable();
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.unreachable-private-machine-pruning.v1",
        ),
        provenance: expected_provenance,
    })
}

fn validator_active_source_ordinals(unit: &PsiOptimizationUnit) -> BTreeMap<MachineId, u32> {
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|row| (row.source_ordinal, row.machine))
        .collect::<BTreeMap<_, _>>();
    let mut active = unit.functions.iter();
    let mut result = BTreeMap::new();
    for ordinal in 0..(unit.functions.len() + unit.pruned_machines.len()) {
        let ordinal = u32::try_from(ordinal).expect("function ordinal fits u32");
        if !pruned.contains_key(&ordinal) {
            if let Some(function) = active.next() {
                result.insert(function.machine, ordinal);
            }
        }
    }
    result
}

fn unreachable_private_machine_complement(unit: &PsiOptimizationUnit) -> Vec<MachineId> {
    let active = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let mut reachable = BTreeSet::from([unit.entry]);
    reachable.extend(
        unit.provider_candidates
            .iter()
            .map(|candidate| candidate.candidate),
    );
    reachable.extend(
        unit.functions
            .iter()
            .filter(|function| function.attachment.is_some())
            .map(|function| function.machine),
    );
    let references = unit
        .functions
        .iter()
        .map(|function| (function.machine, validator_machine_references(function)))
        .collect::<BTreeMap<_, _>>();
    let mut work = reachable.iter().copied().collect::<Vec<_>>();
    while let Some(machine) = work.pop() {
        for callee in references.get(&machine).into_iter().flatten().copied() {
            if active.contains(&callee) && reachable.insert(callee) {
                work.push(callee);
            }
        }
    }
    active.difference(&reachable).copied().collect()
}

fn validator_machine_references(function: &PsiOptimizationFunction) -> BTreeSet<MachineId> {
    let mut references = BTreeSet::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        match operation {
            O::CallUnit { callee, .. }
            | O::CallStructuralScalar { callee, .. }
            | O::CallStructural { callee, .. }
            | O::Call { callee, .. } => {
                references.insert(*callee);
            }
            O::Return {
                cleanup_actions, ..
            }
            | O::ReturnUnit {
                cleanup_actions, ..
            } => {
                references.extend(cleanup_actions.iter().filter_map(|action| match action {
                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        Some(cleanup.cleanup_machine)
                    }
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(_)
                    | psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => None,
                }));
            }
            _ => {}
        }
    }
    references
}

fn pruned_machine_provenance(
    unit: &PsiOptimizationUnit,
    machines: &[MachineId],
) -> Option<Vec<omega_optimization_unit::ProvenanceRewrite>> {
    let machines = machines.iter().copied().collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for function in unit
        .functions
        .iter()
        .filter(|function| machines.contains(&function.machine))
    {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let input = PsiRealizationSite::Node(NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                });
                if !node.provenance.is_empty() {
                    rows.push(omega_optimization_unit::ProvenanceRewrite {
                        input,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(input),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
                for edge in &node.successors {
                    let input = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    if !edge.provenance.is_empty() {
                        rows.push(omega_optimization_unit::ProvenanceRewrite {
                            input,
                            disposition: ProvenanceDisposition::ProvenUnreachableAt(input),
                            sources: edge.provenance.clone(),
                            fuel: edge.fuel.clone(),
                        });
                    }
                }
            }
        }
    }
    rows.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some(rows)
}

/// Independently replay one Boolean-proven conditional fold and atomically
/// remove the exact block complement made unreachable by the rejected edge.
pub fn validate_constant_conditional_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ScalarConstants)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::CallGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::ExactOperationSemantics
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::FoldConstantConditional(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(usize::try_from(patch.location.node).expect("u32 fits usize"))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let O::Conditional {
        condition,
        when_true,
        when_false,
    } = &node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let condition_fact = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::unary_operand)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let constant = literal_boolean_fact(function, input.identity, *condition, condition_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let (selected, rejected) = if constant {
        (when_true, when_false)
    } else {
        (when_false, when_true)
    };
    if patch
        != (ConstantConditionalRewrite {
            location: patch.location,
            condition: *condition,
            constant,
            selected_edge: selected.psi_edge,
            rejected_edge: rejected.psi_edge,
        })
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }
    let reachable =
        reachable_blocks_after_conditional_fold(function, patch.location.block, selected.psi_edge)
            .ok_or(OptimizationUnitValidationError::CandidateReachabilityMismatch)?;
    let (expected_blocks, accepted_provenance) = reconstruct_conditional_fold_accounting(
        function,
        patch.location,
        selected.psi_edge,
        rejected.psi_edge,
        &reachable,
    )
    .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if candidate.provenance().len() != accepted_provenance.len()
        || candidate
            .provenance()
            .iter()
            .zip(&accepted_provenance)
            .any(|(actual, expected)| {
                actual.input != expected.input
                    || actual.disposition != expected.disposition
                    || actual.sources != expected.sources
            })
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if candidate
        .provenance()
        .iter()
        .zip(&accepted_provenance)
        .any(|(actual, expected)| actual.fuel != expected.fuel)
    {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }
    let selected_site = PsiRealizationSite::Edge {
        machine: patch.location.machine,
        edge: selected.psi_edge,
    };
    let selected_fuel = accepted_provenance
        .iter()
        .find(|row| row.disposition == ProvenanceDisposition::RealizedAt(selected_site))
        .expect("independent accounting includes the selected edge")
        .fuel
        .clone();

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate block exists");
    let output_node =
        &mut output_block.nodes[usize::try_from(patch.location.node).expect("u32 fits usize")];
    output_node.operation = O::Jump {
        psi_edge: selected.psi_edge,
        target: selected.target,
        bindings: selected.bindings.clone(),
    };
    output_node.definitions.clear();
    output_node.uses = selected
        .bindings
        .iter()
        .map(|binding| ValueUse {
            value: binding.argument,
            block: patch.location.block,
            node: patch.location.node,
        })
        .collect();
    output_node.successors = vec![OptimizationEdge {
        psi_edge: selected.psi_edge,
        target: selected.target,
        bindings: selected.bindings.clone(),
        provenance: vec![PsiProvenance::Edge(selected.psi_edge)],
        fuel: selected_fuel,
    }];
    output_node.ownership.clear();
    output_node.provenance.clear();
    output_node.fuel.clear();
    output_function
        .blocks
        .retain(|block| reachable.contains(&block.id));
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .expect("validated function effect count fits u64"),
            };
            effect = effect
                .checked_add(1)
                .expect("validated function effect count fits u64");
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;

    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .expect("output function exists");
    for input_block in function
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
    {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.constant-conditional-fold.v4",
        ),
        provenance: accepted_provenance,
    })
}

fn reachable_blocks_after_conditional_fold(
    function: &PsiOptimizationFunction,
    source: BlockId,
    selected_edge: EdgeId,
) -> Option<BTreeSet<BlockId>> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block_id) = pending.pop() {
        if !reachable.insert(block_id) {
            continue;
        }
        let Some(block) = function.blocks.iter().find(|block| block.id == block_id) else {
            return None;
        };
        for edge in block.nodes.iter().flat_map(|node| &node.successors) {
            if block_id != source || edge.psi_edge == selected_edge {
                pending.push(edge.target);
            }
        }
    }
    Some(reachable)
}

fn reconstruct_conditional_fold_accounting(
    function: &PsiOptimizationFunction,
    decision: NodeLocation,
    selected_edge: EdgeId,
    rejected_edge: EdgeId,
    reachable: &BTreeSet<BlockId>,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let decision_node = function
        .blocks
        .iter()
        .find(|block| block.id == decision.block)?
        .nodes
        .get(usize::try_from(decision.node).ok()?)?;
    let selected = decision_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == selected_edge)?;
    let rejected = decision_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == rejected_edge)?;
    let selected_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: selected_edge,
    };
    let rejected_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: rejected_edge,
    };
    let removed = function
        .blocks
        .iter()
        .map(|block| block.id)
        .filter(|block| !reachable.contains(block))
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([decision.block]);
    affected.extend(removed.iter().copied());
    let mut realized = vec![omega_optimization_unit::ProvenanceRewrite {
        input: selected_site,
        disposition: ProvenanceDisposition::RealizedAt(selected_site),
        sources: selected.provenance.clone(),
        fuel: selected.fuel.clone(),
    }];
    let mut unreachable = vec![omega_optimization_unit::ProvenanceRewrite {
        input: rejected_site,
        disposition: ProvenanceDisposition::ProvenUnreachableAt(rejected_site),
        sources: rejected.provenance.clone(),
        fuel: rejected.fuel.clone(),
    }];
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if removed.contains(&block.id) {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                };
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    unreachable.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
                for edge in &node.successors {
                    let site = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    unreachable.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(site),
                        sources: edge.provenance.clone(),
                        fuel: edge.fuel.clone(),
                    });
                }
            }
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes && location != decision {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    realized.extend(unreachable);
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

/// Independently replay one linear empty-jump thread. This deliberately
/// excludes conditional or multiple predecessors: only two edge occurrences
/// that always execute together may be fused into one output edge occurrence.
pub fn validate_linear_empty_block_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::OwnershipFrontiers)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ThreadLinearEmptyBlock(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.predecessor)
        || patch.empty.node != 0
        || patch.empty.machine != patch.predecessor.machine
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == patch.empty.block {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let empty_block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.empty.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [empty_node] = empty_block.nodes.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let O::Jump {
        psi_edge: outgoing_edge,
        target,
        bindings: outgoing_bindings,
    } = &empty_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if *outgoing_edge != patch.outgoing_edge || *target != patch.target {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if empty_block.parameters.iter().any(|parameter| {
        function.blocks.iter().any(|block| {
            block.nodes.iter().any(|node| {
                node.uses.iter().any(|use_site| {
                    use_site.value == parameter.value
                        && (use_site.block != empty_block.id || use_site.node != 0)
                })
            })
        })
    }) {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }

    let incoming = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .filter_map(move |(node_index, node)| {
                    node.successors
                        .iter()
                        .any(|edge| edge.target == patch.empty.block)
                        .then_some((block, node_index, node))
                })
        })
        .collect::<Vec<_>>();
    let [(predecessor_block, predecessor_index, predecessor_node)] = incoming.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    };
    let predecessor_location = NodeLocation {
        machine: function.machine,
        block: predecessor_block.id,
        node: u32::try_from(*predecessor_index)
            .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
    };
    let O::Jump {
        psi_edge: incoming_edge,
        target: predecessor_target,
        bindings: incoming_bindings,
    } = &predecessor_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if predecessor_location != patch.predecessor
        || *incoming_edge != patch.incoming_edge
        || *predecessor_target != patch.empty.block
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let composed_bindings = reconstruct_linear_thread_bindings(
        &empty_block.parameters,
        incoming_bindings,
        outgoing_bindings,
    )
    .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
    if !reconstruct_linear_thread_ownership_is_identity(
        input,
        function,
        patch.incoming_edge,
        patch.empty.block,
        patch.outgoing_edge,
        patch.target,
    ) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_linear_thread_accounting(function, patch.predecessor, patch.empty)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if candidate.provenance().len() != accepted_provenance.len()
        || candidate
            .provenance()
            .iter()
            .zip(&accepted_provenance)
            .any(|(actual, expected)| {
                actual.input != expected.input
                    || actual.disposition != expected.disposition
                    || actual.sources != expected.sources
            })
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if candidate
        .provenance()
        .iter()
        .zip(&accepted_provenance)
        .any(|(actual, expected)| actual.fuel != expected.fuel)
    {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("candidate function exists");
    let output_predecessor = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.predecessor.block)
        .and_then(|block| {
            block
                .nodes
                .get_mut(usize::try_from(patch.predecessor.node).ok()?)
        })
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor_edge = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let empty_edge = empty_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.outgoing_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let mut combined_sources = predecessor_edge.provenance.clone();
    combined_sources.extend_from_slice(&empty_edge.provenance);
    let mut combined_fuel = predecessor_edge.fuel.clone();
    combined_fuel.extend_from_slice(&empty_edge.fuel);
    output_predecessor.operation = O::Jump {
        psi_edge: patch.incoming_edge,
        target: patch.target,
        bindings: composed_bindings,
    };
    output_predecessor.definitions = expected_definitions(
        &output_predecessor.operation,
        patch.predecessor.block,
        patch.predecessor.node,
    );
    output_predecessor.uses = expected_uses(
        &output_predecessor.operation,
        patch.predecessor.block,
        patch.predecessor.node,
    );
    output_predecessor.successors = expected_edges(&output_predecessor.operation);
    output_predecessor.successors[0].provenance = combined_sources;
    output_predecessor.successors[0].fuel = combined_fuel;
    output_predecessor.ownership = expected_ownership(&output_predecessor.operation);
    output_predecessor.provenance.clear();
    output_predecessor.fuel.clear();
    output_function
        .blocks
        .retain(|block| block.id != patch.empty.block);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .expect("validated function effect count fits u64"),
            };
            effect = effect
                .checked_add(1)
                .expect("validated function effect count fits u64");
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;

    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if input_block.id != patch.empty.block
            && !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.linear-empty-block-thread.v2",
        ),
        provenance: accepted_provenance,
    })
}

/// Independently replay an all-predecessor empty-block bypass. Every incoming
/// edge remains its own output occurrence; the removed outgoing occurrence is
/// copied only onto that mutually exclusive edge antichain.
pub fn validate_path_qualified_empty_block_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::OwnershipFrontiers)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.empty) || patch.empty.node != 0 {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.empty.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == patch.empty.block {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let empty_block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.empty.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [empty_node] = empty_block.nodes.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let O::Jump {
        psi_edge: outgoing_edge,
        target,
        bindings: outgoing_bindings,
    } = &empty_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if *outgoing_edge != patch.outgoing_edge || *target != patch.target {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if empty_block.parameters.iter().any(|parameter| {
        function.blocks.iter().any(|block| {
            block.nodes.iter().any(|node| {
                node.uses.iter().any(|use_site| {
                    use_site.value == parameter.value
                        && (use_site.block != empty_block.id || use_site.node != 0)
                })
            })
        })
    }) {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let mut incoming = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            for edge in node
                .successors
                .iter()
                .filter(|edge| edge.target == patch.empty.block)
            {
                let composed = reconstruct_linear_thread_bindings(
                    &empty_block.parameters,
                    &edge.bindings,
                    outgoing_bindings,
                )
                .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
                if !reconstruct_linear_thread_ownership_is_identity(
                    input,
                    function,
                    edge.psi_edge,
                    patch.empty.block,
                    patch.outgoing_edge,
                    patch.target,
                ) {
                    return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
                }
                incoming.push((
                    NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).map_err(|_| {
                            OptimizationUnitValidationError::CandidateLocationMissing
                        })?,
                    },
                    edge.psi_edge,
                    composed,
                ));
            }
        }
    }
    if incoming.is_empty()
        || (incoming.len() == 1
            && matches!(
                function
                    .blocks
                    .iter()
                    .find(|block| block.id == incoming[0].0.block)
                    .and_then(|block| block.nodes.get(usize::try_from(incoming[0].0.node).ok()?))
                    .map(|node| &node.operation),
                Some(O::Jump { .. })
            ))
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let incoming_edges = incoming
        .iter()
        .map(|(_, edge, _)| *edge)
        .collect::<Vec<_>>();
    let (expected_blocks, accepted_provenance) =
        reconstruct_path_thread_accounting(function, patch.empty, &incoming_edges)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let outgoing_edge = empty_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.outgoing_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.empty.machine)
        .expect("candidate function exists");
    for (location, incoming_edge, composed) in &incoming {
        let node = output_function
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .and_then(|block| block.nodes.get_mut(usize::try_from(location.node).ok()?))
            .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        if !rewrite_successor_operation(&mut node.operation, *incoming_edge, patch.target, composed)
        {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        let edge = node
            .successors
            .iter_mut()
            .find(|edge| edge.psi_edge == *incoming_edge)
            .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
        edge.target = patch.target;
        edge.bindings = composed.clone();
        edge.provenance.extend_from_slice(&outgoing_edge.provenance);
        edge.fuel.extend_from_slice(&outgoing_edge.fuel);
        node.definitions = expected_definitions(&node.operation, location.block, location.node);
        node.uses = expected_uses(&node.operation, location.block, location.node);
        node.ownership = expected_ownership(&node.operation);
    }
    output_function
        .blocks
        .retain(|block| block.id != patch.empty.block);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .expect("validated function effect count fits u64"),
            };
            effect = effect
                .checked_add(1)
                .expect("validated function effect count fits u64");
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;

    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.empty.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if input_block.id != patch.empty.block
            && !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.path-qualified-empty-block-thread.v1",
        ),
        provenance: accepted_provenance,
    })
}

/// Independently replay one adjacent single-predecessor block merge. The
/// validator rederives adjacency, unique incoming custody, typed parameter
/// substitutions, ownership-frontier identity, and every moved occurrence.
pub fn validate_adjacent_block_merge_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::OwnershipFrontiers)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::MergeAdjacentBlock(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.predecessor) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if target_position != predecessor_position + 1 || function.entry == patch.target {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let predecessor = &function.blocks[predecessor_position];
    let target = &function.blocks[target_position];
    let predecessor_index = usize::try_from(patch.predecessor.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let eligible_first = target.nodes.first().is_some_and(|node| {
        (node.successors.is_empty()
            && (matches!(node.provenance.first(), Some(PsiProvenance::Operation(_)))
                || (matches!(node.provenance.first(), Some(PsiProvenance::Edge(_)))
                    && matches!(
                        node.operation,
                        O::Return { .. }
                            | O::ReturnUnit { .. }
                            | O::ReturnStructural { .. }
                            | O::Crash { .. }
                    ))))
            || (matches!(node.operation, O::Conditional { .. })
                && node.successors.len() == 2
                && node.provenance.is_empty())
    });
    if predecessor_index + 1 != predecessor.nodes.len() || !eligible_first {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let predecessor_node = &predecessor.nodes[predecessor_index];
    let O::Jump {
        psi_edge,
        target: jump_target,
        bindings,
    } = &predecessor_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if *psi_edge != patch.incoming_edge || *jump_target != patch.target {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let incoming = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == patch.target)
        .collect::<Vec<_>>();
    if incoming.len() != 1 || incoming[0].psi_edge != patch.incoming_edge {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if target.parameters.len() != bindings.len() {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let mut substitutions = target
        .parameters
        .iter()
        .zip(bindings)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some(ScalarSubstitution {
                    from: parameter.value,
                    to: binding.argument,
                    scalar_type: parameter.scalar_type,
                })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
    substitutions.sort();
    if candidate.substitutions() != substitutions {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }
    if !reconstruct_adjacent_merge_ownership_is_identity(
        input,
        function,
        patch.incoming_edge,
        patch.target,
    ) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_adjacent_merge_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("candidate function exists");
    let mut moved = output_function.blocks.remove(target_position);
    let output_predecessor = &mut output_function.blocks[predecessor_position];
    let removed = output_predecessor
        .nodes
        .pop()
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let removed_edge = removed
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    for node in &mut moved.nodes {
        rewrite_scalar_substitutions(
            &mut node.operation,
            &substitutions,
            patch.predecessor.machine,
            patch.target,
        );
    }
    let first = moved
        .nodes
        .first_mut()
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if first.successors.is_empty() {
        first.provenance.extend_from_slice(&removed_edge.provenance);
        first.fuel.extend_from_slice(&removed_edge.fuel);
    } else {
        for successor in &mut first.successors {
            successor
                .provenance
                .extend_from_slice(&removed_edge.provenance);
            successor.fuel.extend_from_slice(&removed_edge.fuel);
        }
    }
    output_predecessor.nodes.append(&mut moved.nodes);
    for (node_index, node) in output_predecessor.nodes.iter_mut().enumerate() {
        let node_index = u32::try_from(node_index)
            .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
        node.definitions = expected_definitions(&node.operation, output_predecessor.id, node_index);
        node.uses = expected_uses(&node.operation, output_predecessor.id, node_index);
        node.successors = preserve_edge_custody(node);
        node.ownership = expected_ownership(&node.operation);
    }
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .expect("validated function effect count fits u64"),
            };
            effect = effect
                .checked_add(1)
                .expect("validated function effect count fits u64");
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.adjacent-single-predecessor-block-merge.v3",
        ),
        provenance: accepted_provenance,
    })
}

/// Independently replay one selected incoming path into a shared terminal.
/// The target remains intact; only the chosen jump is replaced by a typed
/// terminal clone, with exact fanout and fused incoming-edge custody.
pub fn validate_shared_terminal_jump_fusion_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::OwnershipFrontiers)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::FuseSharedTerminalJump(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.predecessor) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor = function
        .blocks
        .iter()
        .find(|block| block.id == patch.predecessor.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor_index = usize::try_from(patch.predecessor.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    if predecessor_index + 1 != predecessor.nodes.len() {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let predecessor_node = predecessor
        .nodes
        .get(predecessor_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let O::Jump {
        psi_edge,
        target: jump_target,
        bindings,
    } = &predecessor_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if *psi_edge != patch.incoming_edge || *jump_target != patch.target {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let target = function
        .blocks
        .iter()
        .find(|block| block.id == patch.target)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [terminal] = target.nodes.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if target.id == function.entry
        || predecessor.id == target.id
        || !terminal.successors.is_empty()
        || !matches!(terminal.provenance.first(), Some(PsiProvenance::Edge(_)))
        || !matches!(
            terminal.operation,
            O::Return { .. } | O::ReturnUnit { .. } | O::ReturnStructural { .. } | O::Crash { .. }
        )
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let incoming = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == patch.target)
        .collect::<Vec<_>>();
    if incoming.len() < 2
        || incoming
            .iter()
            .filter(|edge| edge.psi_edge == patch.incoming_edge)
            .count()
            != 1
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if target.parameters.len() != bindings.len() {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let mut substitutions = target
        .parameters
        .iter()
        .zip(bindings)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some(ScalarSubstitution {
                    from: parameter.value,
                    to: binding.argument,
                    scalar_type: parameter.scalar_type,
                })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
    substitutions.sort();
    if candidate.substitutions() != substitutions {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }
    if !reconstruct_adjacent_merge_ownership_is_identity(
        input,
        function,
        patch.incoming_edge,
        patch.target,
    ) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_shared_terminal_fusion_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let incoming_edge = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?
        .clone();
    let removed_effect = predecessor_node.effect;
    let mut clone = terminal.clone();
    rewrite_scalar_substitutions(
        &mut clone.operation,
        &substitutions,
        patch.predecessor.machine,
        patch.target,
    );
    clone
        .provenance
        .extend_from_slice(&incoming_edge.provenance);
    clone.fuel.extend_from_slice(&incoming_edge.fuel);
    clone.effect = removed_effect;
    clone.definitions = expected_definitions(
        &clone.operation,
        patch.predecessor.block,
        patch.predecessor.node,
    );
    clone.uses = expected_uses(
        &clone.operation,
        patch.predecessor.block,
        patch.predecessor.node,
    );
    clone.successors = expected_edges(&clone.operation);
    clone.ownership = expected_ownership(&clone.operation);

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("candidate function exists");
    let output_predecessor = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.predecessor.block)
        .expect("candidate predecessor exists");
    output_predecessor.nodes[predecessor_index] = clone;
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.shared-terminal-jump-fusion.v1",
        ),
        provenance: accepted_provenance,
    })
}

/// Independently remove one unused, unconditionally total scalar operation.
/// Execution custody remains realized at the immediately following,
/// necessarily co-executed node; it is never represented as unreachable work.
pub fn validate_dead_scalar_node_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let proof_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1",
    );
    let expected_safety = if candidate.rule() == proof_rule {
        OptimizationSafetyClass::ProofCertified
    } else {
        OptimizationSafetyClass::ExactOperationSemantics
    };
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ValueLiveness)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != expected_safety
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::RemoveDeadScalarNode(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location)
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node_index = usize::try_from(patch.location.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let (source_operation, result, scalar_type, obligation) =
        independently_validated_dead_scalar_shape(candidate.rule(), &node.operation)
            .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if source_operation != patch.source_operation
        || result != patch.result
        || scalar_type != patch.scalar_type
        || node.definitions
            != [ValueDefinition {
                value: result,
                scalar_type,
                site: ValueDefinitionSite::Node {
                    block: block.id,
                    node: patch.location.node,
                },
            }]
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
        || block.nodes.get(node_index + 1).is_none()
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if live.live_out.contains(&result)
        || function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.uses)
            .any(|use_site| use_site.value == result)
    {
        return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
    }
    match (obligation, candidate.accepted_obligation_witness()) {
        (Some(obligation), Some(identity)) => {
            if !function.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference {
                        obligation: reference,
                        support,
                    } if *support == source_operation && *reference == obligation
                )
            }) || !input.accepted_obligation_facts.iter().any(|fact| {
                fact.identity == identity
                    && fact.machine == function.machine
                    && fact.operation == source_operation
                    && fact.obligation == obligation
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
        }
        (None, None) => {
            if function.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference { support, .. }
                        if *support == source_operation
                )
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
        }
        _ => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
    }
    let receiver = &block.nodes[node_index + 1];
    if receiver
        .provenance
        .iter()
        .any(|source| node.provenance.contains(source))
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_dead_scalar_node_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate block exists");
    let removed = output_block.nodes.remove(node_index);
    let receiver = output_block
        .nodes
        .get_mut(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&removed.provenance);
    receiver.fuel.extend_from_slice(&removed.fuel);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = effect
                .checked_add(1)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(if obligation.is_some() {
            b"omega.validator.dead-unused-proof-certified-scalar-node.v1"
        } else {
            b"omega.validator.dead-unused-total-scalar-node.v1"
        }),
        provenance: accepted_provenance,
    })
}

fn independently_validated_dead_scalar_shape(
    rule: OptimizationRuleIdentity,
    operation: &O,
) -> Option<(
    psi_core::OperationId,
    ValueId,
    ScalarType,
    Option<psi_core::ObligationId>,
)> {
    let literal_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1",
    );
    let total_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1",
    );
    let proof_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1",
    );
    match (rule, operation) {
        (
            rule,
            O::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) if rule == literal_rule => Some((*psi_operation, *result, *scalar_type, None)),
        (
            rule,
            O::BooleanConstant {
                psi_operation,
                result,
                ..
            },
        ) if rule == literal_rule => Some((*psi_operation, *result, ScalarType::Boolean, None)),
        (
            rule,
            O::BooleanNot {
                psi_operation,
                result,
                ..
            }
            | O::BooleanEqual {
                psi_operation,
                result,
                ..
            }
            | O::IntegerEqual {
                psi_operation,
                result,
                ..
            }
            | O::IntegerLessThan {
                psi_operation,
                result,
                ..
            }
            | O::IntegerLessOrEqual {
                psi_operation,
                result,
                ..
            },
        ) if rule == total_rule => Some((*psi_operation, *result, ScalarType::Boolean, None)),
        (
            rule,
            O::IntegerBitwiseNot {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) if rule == total_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            None,
        )),
        (
            rule,
            O::IntegerWiden {
                psi_operation,
                result,
                target_type,
                ..
            },
        ) if rule == total_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
            None,
        )),
        (
            rule,
            O::WrappingIntegerShiftLeft {
                psi_operation,
                result,
                value_type,
                ..
            }
            | O::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                ..
            },
        ) if rule == total_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            None,
        )),
        (
            rule,
            O::IntegerExactCast {
                psi_operation,
                obligation,
                result,
                target_type,
                ..
            },
        ) if rule == proof_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
            Some(*obligation),
        )),
        (
            rule,
            O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type,
                ..
            }
            | O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                ..
            },
        ) if rule == proof_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            Some(*obligation),
        )),
        (
            rule,
            O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            },
        ) if rule == proof_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            Some(*obligation),
        )),
        _ => None,
    }
}

fn preserve_edge_custody(
    node: &omega_optimization_unit::OptimizationNode,
) -> Vec<OptimizationEdge> {
    let expected = expected_edges(&node.operation);
    expected
        .into_iter()
        .map(|mut edge| {
            if let Some(existing) = node
                .successors
                .iter()
                .find(|existing| existing.psi_edge == edge.psi_edge)
            {
                edge.provenance = existing.provenance.clone();
                edge.fuel = existing.fuel.clone();
            }
            edge
        })
        .collect()
}

fn rewrite_scalar_substitutions(
    operation: &mut O,
    substitutions: &[ScalarSubstitution],
    machine: MachineId,
    removed_block: BlockId,
) {
    for substitution in substitutions {
        rewrite_block_parameter_operation(
            operation,
            RedundantBlockParameterRewrite {
                machine,
                block: removed_block,
                position: 0,
                parameter: substitution.from,
                replacement: substitution.to,
                scalar_type: substitution.scalar_type,
            },
        );
    }
}

fn reconstruct_adjacent_merge_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    incoming: EdgeId,
    target: BlockId,
) -> bool {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| {
        unit.ownership_frontier_facts
            .iter()
            .find(|fact| fact.machine == function.machine && fact.site == site)
    });
    if facts.iter().all(Option::is_none) {
        return function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty();
    }
    facts.iter().all(Option::is_some)
        && facts
            .windows(2)
            .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
}

fn reconstruct_adjacent_merge_accounting(
    function: &PsiOptimizationFunction,
    patch: AdjacentBlockMergeRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)?;
    if target_position != predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor = &function.blocks[predecessor_position];
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)?;
    let target = &function.blocks[target_position];
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let mut affected = BTreeSet::from([predecessor.id, target.id]);
    let first = target.nodes.first()?;
    let mut realized = if first.successors.is_empty() {
        vec![omega_optimization_unit::ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                NodeLocation {
                    machine: function.machine,
                    block: predecessor.id,
                    node: patch.predecessor.node,
                },
            )),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        }]
    } else {
        first
            .successors
            .iter()
            .map(|successor| omega_optimization_unit::ProvenanceRewrite {
                input: incoming_site,
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: successor.psi_edge,
                }),
                sources: incoming.provenance.clone(),
                fuel: incoming.fuel.clone(),
            })
            .collect()
    };
    for (node_index, node) in target.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target.id,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.id,
            node: patch
                .predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(omega_optimization_unit::ProvenanceRewrite {
            input,
            disposition: ProvenanceDisposition::RealizedAt(output),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    for block in function.blocks.iter().skip(target_position + 1) {
        affected.insert(block.id);
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn reconstruct_shared_terminal_fusion_accounting(
    function: &PsiOptimizationFunction,
    patch: SharedTerminalJumpFusionRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor = function
        .blocks
        .iter()
        .find(|block| block.id == patch.predecessor.block)?;
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)?;
    let target = function
        .blocks
        .iter()
        .find(|block| block.id == patch.target)?;
    let [terminal] = target.nodes.as_slice() else {
        return None;
    };
    let input_edge = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let input_terminal = PsiRealizationSite::Node(NodeLocation {
        machine: function.machine,
        block: patch.target,
        node: 0,
    });
    let output_clone = PsiRealizationSite::Node(patch.predecessor);
    let mut provenance = vec![
        omega_optimization_unit::ProvenanceRewrite {
            input: input_edge,
            disposition: ProvenanceDisposition::RealizedAt(output_clone),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        },
        omega_optimization_unit::ProvenanceRewrite {
            input: input_terminal,
            disposition: ProvenanceDisposition::RealizedAt(output_clone),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        },
        omega_optimization_unit::ProvenanceRewrite {
            input: input_terminal,
            disposition: ProvenanceDisposition::RealizedAt(input_terminal),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        },
    ];
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    let mut blocks = vec![patch.predecessor.block, patch.target];
    blocks.sort();
    blocks.dedup();
    Some((blocks, provenance))
}

fn reconstruct_dead_scalar_node_accounting(
    function: &PsiOptimizationFunction,
    patch: DeadScalarNodeRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let block_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.location.block)?;
    let node_position = usize::try_from(patch.location.node).ok()?;
    let block = &function.blocks[block_position];
    let removed = block.nodes.get(node_position)?;
    block.nodes.get(node_position.checked_add(1)?)?;
    let mut provenance = vec![omega_optimization_unit::ProvenanceRewrite {
        input: PsiRealizationSite::Node(patch.location),
        disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(patch.location)),
        sources: removed.provenance.clone(),
        fuel: removed.fuel.clone(),
    }];
    for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
        if node.provenance.is_empty() {
            continue;
        }
        let old = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        };
        let new = NodeLocation {
            node: old.node.checked_sub(1)?,
            ..old
        };
        provenance.push(omega_optimization_unit::ProvenanceRewrite {
            input: PsiRealizationSite::Node(old),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(new)),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    let mut blocks = vec![block.id];
    for later in function.blocks.iter().skip(block_position + 1) {
        blocks.push(later.id);
        for (index, node) in later.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: later.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    blocks.sort();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((blocks, provenance))
}

fn rewrite_successor_operation(
    operation: &mut O,
    edge: EdgeId,
    target: BlockId,
    bindings: &[omega_terminal_abstract_operations::TerminalValueBinding],
) -> bool {
    match operation {
        O::Jump {
            psi_edge,
            target: operation_target,
            bindings: operation_bindings,
        } if *psi_edge == edge => {
            *operation_target = target;
            *operation_bindings = bindings.to_vec();
            true
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            let successor = if when_true.psi_edge == edge {
                when_true
            } else if when_false.psi_edge == edge {
                when_false
            } else {
                return false;
            };
            successor.target = target;
            successor.bindings = bindings.to_vec();
            true
        }
        _ => false,
    }
}

fn reconstruct_linear_thread_bindings(
    parameters: &[ValueDefinition],
    incoming: &[omega_terminal_abstract_operations::TerminalValueBinding],
    outgoing: &[omega_terminal_abstract_operations::TerminalValueBinding],
) -> Option<Vec<omega_terminal_abstract_operations::TerminalValueBinding>> {
    if parameters.len() != incoming.len() {
        return None;
    }
    let replacements = parameters
        .iter()
        .zip(incoming)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some((parameter.value, (binding.argument, binding.scalar_type)))
        })
        .collect::<Option<BTreeMap<_, _>>>()?;
    Some(
        outgoing
            .iter()
            .map(|binding| {
                replacements
                    .get(&binding.argument)
                    .map_or(*binding, |(argument, scalar_type)| {
                        omega_terminal_abstract_operations::TerminalValueBinding {
                            parameter: binding.parameter,
                            argument: *argument,
                            scalar_type: *scalar_type,
                        }
                    })
            })
            .collect(),
    )
}

fn reconstruct_linear_thread_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    incoming: EdgeId,
    empty: BlockId,
    outgoing: EdgeId,
    target: BlockId,
) -> bool {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(empty),
        OwnershipFrontierSite::EdgeEntry(outgoing),
        OwnershipFrontierSite::EdgeExit(outgoing),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| {
        unit.ownership_frontier_facts
            .iter()
            .find(|fact| fact.machine == function.machine && fact.site == site)
    });
    if facts.iter().all(Option::is_none) {
        return function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty();
    }
    facts.iter().all(Option::is_some)
        && facts
            .windows(2)
            .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
}

fn reconstruct_linear_thread_accounting(
    function: &PsiOptimizationFunction,
    predecessor: NodeLocation,
    empty: NodeLocation,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor_node = function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)?
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let empty_node = function
        .blocks
        .iter()
        .find(|block| block.id == empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let predecessor_edge = predecessor_node.successors.first()?;
    let empty_edge = empty_node.successors.first()?;
    let output_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: predecessor_edge.psi_edge,
    };
    let predecessor_site = output_site;
    let empty_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: empty_edge.psi_edge,
    };
    let mut affected = BTreeSet::from([predecessor.block, empty.block]);
    let mut realized = vec![
        omega_optimization_unit::ProvenanceRewrite {
            input: predecessor_site,
            disposition: ProvenanceDisposition::RealizedAt(output_site),
            sources: predecessor_edge.provenance.clone(),
            fuel: predecessor_edge.fuel.clone(),
        },
        omega_optimization_unit::ProvenanceRewrite {
            input: empty_site,
            disposition: ProvenanceDisposition::RealizedAt(output_site),
            sources: empty_edge.provenance.clone(),
            fuel: empty_edge.fuel.clone(),
        },
    ];
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes && location != predecessor {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn reconstruct_path_thread_accounting(
    function: &PsiOptimizationFunction,
    empty: NodeLocation,
    incoming_edges: &[EdgeId],
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let empty_node = function
        .blocks
        .iter()
        .find(|block| block.id == empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let outgoing = empty_node.successors.first()?;
    let outgoing_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: outgoing.psi_edge,
    };
    let incoming_set = incoming_edges.iter().copied().collect::<BTreeSet<_>>();
    if incoming_set.len() != incoming_edges.len() || incoming_set.is_empty() {
        return None;
    }
    let mut affected = BTreeSet::from([empty.block]);
    let mut realized = Vec::new();
    for block in &function.blocks {
        for node in &block.nodes {
            for edge in &node.successors {
                if !incoming_set.contains(&edge.psi_edge) || edge.target != empty.block {
                    continue;
                }
                affected.insert(block.id);
                let site = PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: edge.psi_edge,
                };
                realized.push(omega_optimization_unit::ProvenanceRewrite {
                    input: site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: edge.provenance.clone(),
                    fuel: edge.fuel.clone(),
                });
                realized.push(omega_optimization_unit::ProvenanceRewrite {
                    input: outgoing_site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: outgoing.provenance.clone(),
                    fuel: outgoing.fuel.clone(),
                });
            }
        }
    }
    if realized.len() != incoming_edges.len().checked_mul(2)? {
        return None;
    }
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

/// Independently replay one redundant block-parameter elimination. The rule's
/// incoming-edge witness is treated only as a claim: this validator enumerates
/// every exact incoming edge again before applying the substitution.
pub fn validate_redundant_block_parameter_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::Dominators)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::RemoveRedundantBlockParameter(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let witness = candidate
        .redundant_block_parameter_witness()
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == patch.block || patch.parameter == patch.replacement {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let position = usize::try_from(patch.position).expect("u32 fits usize");
    let Some(parameter) = block.parameters.get(position) else {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    };
    if parameter.value != patch.parameter
        || parameter.scalar_type != patch.scalar_type
        || parameter.site
            != (ValueDefinitionSite::BlockParameter {
                block: patch.block,
                position: patch.position,
            })
    {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    }
    let replacement_type = function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
        .find(|definition| definition.value == patch.replacement)
        .map(|definition| definition.scalar_type);
    if replacement_type != Some(patch.scalar_type) {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }

    let mut incoming = Vec::new();
    let mut expected_provenance = Vec::new();
    let mut affected_blocks = BTreeSet::from([patch.block]);
    for source in &function.blocks {
        for (node_index, node) in source.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: patch.machine,
                block: source.id,
                node: u32::try_from(node_index).expect("unit node index fits u32"),
            };
            let changes_use = node
                .uses
                .iter()
                .any(|use_site| use_site.value == patch.parameter);
            for edge in &node.successors {
                if edge.target != patch.block {
                    continue;
                }
                let Some(binding) = edge.bindings.get(position) else {
                    return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
                };
                incoming.push(BlockParameterIncomingBinding {
                    source: source.id,
                    edge: edge.psi_edge,
                    argument: binding.argument,
                });
                let site = PsiRealizationSite::Edge {
                    machine: patch.machine,
                    edge: edge.psi_edge,
                };
                expected_provenance.push(omega_optimization_unit::ProvenanceRewrite {
                    input: site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: edge.provenance.clone(),
                    fuel: edge.fuel.clone(),
                });
            }
            if changes_use {
                affected_blocks.insert(source.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    expected_provenance.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            if node
                .successors
                .iter()
                .any(|edge| edge.target == patch.block)
            {
                affected_blocks.insert(source.id);
            }
        }
    }
    incoming.sort_by_key(|row| (row.edge, row.source));
    expected_provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    if incoming != witness.incoming
        || incoming
            .iter()
            .any(|binding| binding.argument != patch.replacement)
    {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    if candidate.substitutions()
        != [omega_optimization_unit::ScalarSubstitution {
            from: patch.parameter,
            to: patch.replacement,
            scalar_type: patch.scalar_type,
        }]
    {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }
    if candidate.affected_blocks() != affected_blocks.into_iter().collect::<Vec<_>>()
        || candidate.provenance() != expected_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let normalized_input =
        normalize_redundant_parameter_observation_input(input, patch, candidate.affected_blocks())?;
    let input_region = reconstruct_psi_closed_region_observation(
        &normalized_input,
        patch.machine,
        candidate.affected_blocks(),
    )
    .ok_or(OptimizationUnitValidationError::CandidateRegionObservationUnavailable)?;

    let mut output = input.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.machine)
        .expect("candidate function exists");
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.block)
        .expect("candidate block exists");
    block.parameters.remove(position);
    for (new_position, parameter) in block.parameters.iter_mut().enumerate().skip(position) {
        parameter.site = ValueDefinitionSite::BlockParameter {
            block: patch.block,
            position: u32::try_from(new_position).expect("parameter index fits u32"),
        };
    }
    for block in &mut function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            rewrite_block_parameter_operation(&mut node.operation, patch);
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = expected_edges(&node.operation);
            node.ownership = expected_ownership(&node.operation);
        }
    }
    function.facts = reconstruct_fact_index(function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    if !unchanged_outside_redundant_parameter_region(
        input,
        &output,
        patch.machine,
        candidate.affected_blocks(),
    ) {
        return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
    }
    let output_region = reconstruct_psi_closed_region_observation(
        &output,
        patch.machine,
        candidate.affected_blocks(),
    )
    .ok_or(OptimizationUnitValidationError::CandidateRegionObservationUnavailable)?;
    if input_region.semantics != output_region.semantics {
        return Err(OptimizationUnitValidationError::CandidateRegionObservationMismatch);
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.redundant-block-parameter.v2",
        ),
        provenance: expected_provenance,
    })
}

/// Construct the validator's normalized pre-rewrite question independently of
/// the output constructor below. Only the exact scalar substitution and the
/// one proved incoming binding slot may change.
fn normalize_redundant_parameter_observation_input(
    input: &PsiOptimizationUnit,
    patch: RedundantBlockParameterRewrite,
    affected_blocks: &[BlockId],
) -> Result<PsiOptimizationUnit, OptimizationUnitValidationError> {
    let affected = affected_blocks.iter().copied().collect::<BTreeSet<_>>();
    let mut normalized = input.clone();
    let function = normalized
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let target = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let position = usize::try_from(patch.position).expect("u32 fits usize");
    let removed = target
        .parameters
        .get(position)
        .copied()
        .ok_or(OptimizationUnitValidationError::CandidateBlockParameterMismatch)?;
    if removed.value != patch.parameter
        || removed.scalar_type != patch.scalar_type
        || removed.site
            != (ValueDefinitionSite::BlockParameter {
                block: patch.block,
                position: patch.position,
            })
    {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    }
    target.parameters.remove(position);
    for (new_position, parameter) in target.parameters.iter_mut().enumerate().skip(position) {
        parameter.site = ValueDefinitionSite::BlockParameter {
            block: patch.block,
            position: u32::try_from(new_position).expect("parameter index fits u32"),
        };
    }

    for block in function
        .blocks
        .iter_mut()
        .filter(|block| affected.contains(&block.id))
    {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            node.operation =
                normalize_redundant_parameter_observation_operation(&node.operation, patch)?;
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = expected_edges(&node.operation);
            node.ownership = expected_ownership(&node.operation);
        }
    }
    normalized.identity = recompute_psi_optimization_unit_identity(&normalized);
    Ok(normalized)
}

fn normalize_redundant_parameter_observation_operation(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
    patch: RedundantBlockParameterRewrite,
) -> Result<
    omega_terminal_abstract_operations::TerminalAbstractOperation,
    OptimizationUnitValidationError,
> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;

    let mut normalized = operation.clone();
    let replace = |value: &mut ValueId| {
        if *value == patch.parameter {
            *value = patch.replacement;
        }
    };
    let normalize_bindings =
        |target: BlockId,
         bindings: &mut Vec<omega_terminal_abstract_operations::TerminalValueBinding>|
         -> Result<(), OptimizationUnitValidationError> {
            for binding in bindings.iter_mut() {
                replace(&mut binding.argument);
            }
            if target == patch.block {
                let position = usize::try_from(patch.position).expect("u32 fits usize");
                let binding = bindings
                    .get(position)
                    .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
                if binding.parameter != patch.parameter
                    || binding.argument != patch.replacement
                    || binding.scalar_type != patch.scalar_type
                {
                    return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
                }
                bindings.remove(position);
            }
            Ok(())
        };

    match &mut normalized {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => replace(operand),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            replace(value);
            replace(count);
        }
        O::Jump {
            target, bindings, ..
        } => normalize_bindings(*target, bindings)?,
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            normalize_bindings(when_true.target, &mut when_true.bindings)?;
            normalize_bindings(when_false.target, &mut when_false.bindings)?;
        }
        O::Return { value, .. } => replace(value),
        O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
    }
    Ok(normalized)
}

fn unchanged_outside_redundant_parameter_region(
    input: &PsiOptimizationUnit,
    output: &PsiOptimizationUnit,
    machine: MachineId,
    affected_blocks: &[BlockId],
) -> bool {
    let mut expected = input.clone();
    let Some(expected_function) = expected
        .functions
        .iter_mut()
        .find(|function| function.machine == machine)
    else {
        return false;
    };
    let Some(output_function) = output
        .functions
        .iter()
        .find(|function| function.machine == machine)
    else {
        return false;
    };
    for block_id in affected_blocks {
        let Some(expected_block) = expected_function
            .blocks
            .iter_mut()
            .find(|block| block.id == *block_id)
        else {
            return false;
        };
        let Some(output_block) = output_function
            .blocks
            .iter()
            .find(|block| block.id == *block_id)
        else {
            return false;
        };
        *expected_block = output_block.clone();
    }
    expected.identity = output.identity;
    expected == *output
}

fn rewrite_block_parameter_operation(
    operation: &mut omega_terminal_abstract_operations::TerminalAbstractOperation,
    patch: RedundantBlockParameterRewrite,
) {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;

    let replace = |value: &mut ValueId| {
        if *value == patch.parameter {
            *value = patch.replacement;
        }
    };
    let rewrite_bindings =
        |bindings: &mut Vec<omega_terminal_abstract_operations::TerminalValueBinding>| {
            for binding in bindings.iter_mut() {
                if binding.argument == patch.parameter {
                    binding.argument = patch.replacement;
                }
            }
        };
    match operation {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => replace(operand),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            replace(value);
            replace(count);
        }
        O::Jump {
            target, bindings, ..
        } => {
            rewrite_bindings(bindings);
            if *target == patch.block {
                bindings.remove(usize::try_from(patch.position).expect("u32 fits usize"));
            }
        }
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            for successor in [when_true, when_false] {
                rewrite_bindings(&mut successor.bindings);
                if successor.target == patch.block {
                    successor
                        .bindings
                        .remove(usize::try_from(patch.position).expect("u32 fits usize"));
                }
            }
        }
        O::Return { value, .. } => replace(value),
        O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
    }
}

/// Independently check and construct one Boolean-evaluation rewrite.
pub fn validate_boolean_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ScalarConstants)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(usize::try_from(patch.location.node).expect("u32 fits usize"))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let input_observation = observation_at(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [provenance] = candidate.provenance() else {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    };
    let site = PsiRealizationSite::Node(patch.location);
    if provenance.input != site
        || provenance.disposition != ProvenanceDisposition::RealizedAt(site)
        || provenance.sources != node.provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if provenance.fuel != node.fuel {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }
    let accepted_provenance = vec![omega_optimization_unit::ProvenanceRewrite {
        input: site,
        disposition: ProvenanceDisposition::RealizedAt(site),
        sources: node.provenance.clone(),
        fuel: node.fuel.clone(),
    }];
    let (source_operation, result, evaluated) =
        evaluate_boolean_operation(function, node, candidate)?;
    if candidate.safety_class() != OptimizationSafetyClass::ExactOperationSemantics {
        return Err(OptimizationUnitValidationError::CandidateSafetyClassMismatch);
    }
    if patch
        != (BooleanConstantRewrite {
            location: patch.location,
            source_operation,
            result,
            constant: evaluated,
        })
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }
    let mut output = input.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate source function exists");
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate source block exists");
    let node = &mut block.nodes[usize::try_from(patch.location.node).expect("u32 fits usize")];
    node.operation =
        omega_terminal_abstract_operations::TerminalAbstractOperation::BooleanConstant {
            psi_operation: patch.source_operation,
            result: patch.result,
            value: patch.constant,
        };
    node.definitions = vec![ValueDefinition {
        value: patch.result,
        scalar_type: ScalarType::Boolean,
        site: ValueDefinitionSite::Node {
            block: patch.location.block,
            node: patch.location.node,
        },
    }];
    node.uses.clear();
    node.successors.clear();
    node.ownership.clear();
    function.facts = reconstruct_fact_index(function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_observation = observation_at(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !same_closed_scalar_observation(&input_observation, &output_observation) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let output_live = reconstruct_closed_scalar_node_boundary(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if input_live.live_out != output_live.live_out
        || output_live
            .live_in
            .iter()
            .any(|value| !input_live.live_in.contains(value))
    {
        return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.boolean-evaluation.v1",
        ),
        provenance: accepted_provenance,
    })
}

fn evaluate_boolean_operation(
    function: &PsiOptimizationFunction,
    node: &omega_optimization_unit::OptimizationNode,
    candidate: &PsiRewriteCandidate,
) -> Result<(psi_core::OperationId, ValueId, bool), OptimizationUnitValidationError> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    match node.operation {
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => {
            let Some(operand_fact) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::unary_operand)
            else {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            };
            let operand = literal_boolean_fact(function, candidate.input(), operand, operand_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            Ok((psi_operation, result, !operand))
        }
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let Some((left_fact, right_fact)) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::binary_operands)
            else {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            };
            let left = literal_boolean_fact(function, candidate.input(), left, left_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let right = literal_boolean_fact(function, candidate.input(), right, right_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            Ok((psi_operation, result, left == right))
        }
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        }
        | O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        }
        | O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let Some((left_fact, right_fact)) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::binary_operands)
            else {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            };
            let left_value = literal_integer_fact(function, candidate.input(), left, left_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let right_value = literal_integer_fact(function, candidate.input(), right, right_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let left_type = validator_integer_value_type(function, left)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            if validator_integer_value_type(function, right) != Some(left_type) {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            }
            let ordering = left_type
                .compare(left_value, right_value)
                .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
            let constant = match node.operation {
                O::IntegerEqual { .. } => ordering.is_eq(),
                O::IntegerLessThan { .. } => ordering.is_lt(),
                O::IntegerLessOrEqual { .. } => !ordering.is_gt(),
                _ => unreachable!(),
            };
            Ok((psi_operation, result, constant))
        }
        _ => Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    }
}

fn observation_at(
    unit: &PsiOptimizationUnit,
    location: omega_optimization_unit::NodeLocation,
) -> Option<PsiNodeObservation> {
    reconstruct_psi_observation_model(unit)
        .nodes
        .into_iter()
        .find(|row| {
            row.machine == location.machine
                && row.block == location.block
                && row.node == location.node
        })
}

fn same_closed_scalar_observation(input: &PsiNodeObservation, output: &PsiNodeObservation) -> bool {
    input.machine == output.machine
        && input.block == output.block
        && input.node == output.node
        && input.definitions == output.definitions
        && input.effect == output.effect
        && input.ownership == output.ownership
        && input.provenance == output.provenance
        && input.fuel == output.fuel
        && input.crash == output.crash
        && input.suspension == output.suspension
        && input.events == output.events
}

fn evaluate_integer_operation(
    function: &PsiOptimizationFunction,
    node: &omega_optimization_unit::OptimizationNode,
    candidate: &PsiRewriteCandidate,
) -> Result<
    (
        psi_core::OperationId,
        ValueId,
        psi_core::IntegerType,
        psi_core::IntegerValue,
        OptimizationSafetyClass,
    ),
    OptimizationUnitValidationError,
> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    if let O::IntegerExactCast {
        psi_operation,
        result,
        source_type,
        target_type,
        operand,
        ..
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = source_type
            .exact_cast_value_to(target_type, operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            target_type,
            evaluated,
            OptimizationSafetyClass::ProofCertified,
        ));
    }
    if let O::IntegerWiden {
        psi_operation,
        result,
        source_type,
        target_type,
        operand,
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = source_type
            .widen_value_to(target_type, operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            target_type,
            evaluated,
            OptimizationSafetyClass::ExactOperationSemantics,
        ));
    }
    if let O::IntegerBitwiseNot {
        psi_operation,
        result,
        scalar_type,
        operand,
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = scalar_type
            .bitwise_not(operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            scalar_type,
            evaluated,
            OptimizationSafetyClass::ExactOperationSemantics,
        ));
    }
    enum IntegerOperation {
        ExactAdd,
        ExactSubtract,
        ExactMultiply,
        WrappingAdd,
        WrappingSubtract,
        WrappingMultiply,
        SaturatingAdd,
        SaturatingSubtract,
        SaturatingMultiply,
        ExactDivide,
        ExactRemainder,
        WrappingDivide,
        WrappingRemainder,
        SaturatingDivide,
        SaturatingRemainder,
        ExactShiftLeft(psi_core::IntegerType),
        ExactShiftRight(psi_core::IntegerType),
        WrappingShiftLeft(psi_core::IntegerType),
        WrappingShiftRight(psi_core::IntegerType),
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
    }
    let (kind, source, result, scalar_type, left, right) = match &node.operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::WrappingDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::WrappingRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::SaturatingDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::SaturatingRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            IntegerOperation::ExactShiftLeft(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            IntegerOperation::ExactShiftRight(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IntegerOperation::WrappingShiftLeft(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IntegerOperation::WrappingShiftRight(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseAnd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseOr,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseXor,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    let Some((left_fact, right_fact)) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::binary_operands)
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let left_value = literal_integer_fact(function, candidate.input(), left, left_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let right_value = literal_integer_fact(function, candidate.input(), right, right_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let (evaluated, safety_class) = match kind {
        IntegerOperation::ExactAdd => (
            scalar_type.exact_add(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactSubtract => (
            scalar_type.exact_sub(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactMultiply => (
            scalar_type.exact_mul(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingAdd => (
            scalar_type.wrapping_add(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingSubtract => (
            scalar_type.wrapping_sub(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingMultiply => (
            scalar_type.wrapping_mul(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingAdd => (
            scalar_type.saturating_add(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingSubtract => (
            scalar_type.saturating_sub(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingMultiply => (
            scalar_type.saturating_mul(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::ExactDivide => (
            scalar_type.exact_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactRemainder => (
            scalar_type.exact_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingDivide => (
            scalar_type.wrapping_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingRemainder => (
            scalar_type.wrapping_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::SaturatingDivide => (
            scalar_type.saturating_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::SaturatingRemainder => (
            scalar_type.saturating_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactShiftLeft(count_type) => (
            scalar_type.exact_shift_left(left_value, count_type, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactShiftRight(count_type) => (
            scalar_type.exact_shift_right(left_value, count_type, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingShiftLeft(count_type) => (
            scalar_type.wrapping_shift_left(left_value, count_type, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingShiftRight(count_type) => (
            scalar_type.wrapping_shift_right(left_value, count_type, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseAnd => (
            scalar_type.bitwise_and(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseOr => (
            scalar_type.bitwise_or(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseXor => (
            scalar_type.bitwise_xor(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
    };
    let evaluated =
        evaluated.ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
    Ok((source, result, scalar_type, evaluated, safety_class))
}

fn unary_integer_operand(
    function: &PsiOptimizationFunction,
    candidate: &PsiRewriteCandidate,
    operand: ValueId,
) -> Result<psi_core::IntegerValue, OptimizationUnitValidationError> {
    let Some(operand_fact) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::unary_operand)
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    literal_integer_fact(function, candidate.input(), operand, operand_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)
}

fn literal_integer_fact(
    function: &PsiOptimizationFunction,
    input: omega_optimization_core::OptimizationUnitIdentity,
    value: ValueId,
    identity: omega_optimization_core::ScalarConstantFactIdentity,
) -> Option<psi_core::IntegerValue> {
    validator_scalar_constant_facts(input, function)
        .into_iter()
        .find_map(|(fact_value, constant, fact_identity)| {
            (fact_value == value && fact_identity == identity)
                .then_some(constant)
                .and_then(|constant| match constant {
                    ScalarConstantValue::Integer(value) => Some(value),
                    ScalarConstantValue::Boolean(_) => None,
                })
        })
}

fn literal_boolean_fact(
    function: &PsiOptimizationFunction,
    input: omega_optimization_core::OptimizationUnitIdentity,
    value: ValueId,
    identity: omega_optimization_core::ScalarConstantFactIdentity,
) -> Option<bool> {
    validator_scalar_constant_facts(input, function)
        .into_iter()
        .find_map(|(fact_value, constant, fact_identity)| {
            (fact_value == value && fact_identity == identity)
                .then_some(constant)
                .and_then(|constant| match constant {
                    ScalarConstantValue::Boolean(value) => Some(value),
                    ScalarConstantValue::Integer(_) => None,
                })
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidatorSccpValue {
    Unknown,
    Constant(ScalarConstantValue),
    Overdefined,
}

fn validator_scalar_constant_facts(
    input: omega_optimization_core::OptimizationUnitIdentity,
    function: &PsiOptimizationFunction,
) -> Vec<(
    ValueId,
    ScalarConstantValue,
    omega_optimization_core::ScalarConstantFactIdentity,
)> {
    fn merge(target: &mut ValidatorSccpValue, incoming: ValidatorSccpValue) -> bool {
        let next = match (*target, incoming) {
            (ValidatorSccpValue::Unknown, incoming) => incoming,
            (_, ValidatorSccpValue::Unknown) | (ValidatorSccpValue::Overdefined, _) => {
                return false;
            }
            (_, ValidatorSccpValue::Overdefined) => ValidatorSccpValue::Overdefined,
            (ValidatorSccpValue::Constant(current), ValidatorSccpValue::Constant(incoming))
                if current == incoming =>
            {
                return false;
            }
            (ValidatorSccpValue::Constant(_), ValidatorSccpValue::Constant(_)) => {
                ValidatorSccpValue::Overdefined
            }
        };
        if *target == next {
            false
        } else {
            *target = next;
            true
        }
    }

    let mut values = BTreeMap::<ValueId, ValidatorSccpValue>::new();
    for parameter in &function.parameters {
        values.insert(parameter.value, ValidatorSccpValue::Overdefined);
    }
    for block in &function.blocks {
        for parameter in &block.parameters {
            values.insert(parameter.value, ValidatorSccpValue::Unknown);
        }
        for definition in block.nodes.iter().flat_map(|node| &node.definitions) {
            values.insert(definition.value, ValidatorSccpValue::Overdefined);
        }
    }
    let support_blocks = function
        .blocks
        .iter()
        .flat_map(|block| {
            block.nodes.iter().flat_map(move |node| {
                node.provenance
                    .iter()
                    .filter_map(move |source| match source {
                        PsiProvenance::Operation(operation) => Some((*operation, block.id)),
                        PsiProvenance::Edge(_) => None,
                    })
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut literal_rows = Vec::new();
    let mut literal_support = BTreeMap::new();
    for fact in &function.facts {
        let (value, constant, support) = match fact {
            OptimizationFact::BooleanConstant {
                value,
                constant,
                support,
            } => (*value, ScalarConstantValue::Boolean(*constant), *support),
            OptimizationFact::IntegerConstant {
                value,
                constant,
                support,
            } => (*value, ScalarConstantValue::Integer(*constant), *support),
            OptimizationFact::OperationObligationReference { .. } => continue,
        };
        let block = support_blocks.get(&support).copied();
        literal_rows.push((value, constant, block));
        literal_support.insert(value, support);
        values.insert(
            value,
            if block.is_some() {
                ValidatorSccpValue::Unknown
            } else {
                ValidatorSccpValue::Constant(constant)
            },
        );
    }

    let mut reachable = BTreeSet::from([function.entry]);
    let mut feasible_edges = BTreeSet::<EdgeId>::new();
    loop {
        let mut changed = false;
        for block in &function.blocks {
            if !reachable.contains(&block.id) {
                continue;
            }
            for (value, constant, site) in &literal_rows {
                if *site == Some(block.id)
                    && matches!(values.get(value), Some(ValidatorSccpValue::Unknown))
                {
                    values.insert(*value, ValidatorSccpValue::Constant(*constant));
                    changed = true;
                }
            }
            let Some(node) = block.nodes.last() else {
                continue;
            };
            let operation_successors = validator_scalar_operation_successors(&node.operation);
            let successors = match &node.operation {
                omega_terminal_abstract_operations::TerminalAbstractOperation::Jump { .. } => {
                    operation_successors.iter().collect::<Vec<_>>()
                }
                omega_terminal_abstract_operations::TerminalAbstractOperation::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => match values.get(condition) {
                    Some(ValidatorSccpValue::Constant(ScalarConstantValue::Boolean(value))) => {
                        let selected = if *value {
                            when_true.psi_edge
                        } else {
                            when_false.psi_edge
                        };
                        operation_successors
                            .iter()
                            .filter(|successor| successor.psi_edge == selected)
                            .collect()
                    }
                    Some(ValidatorSccpValue::Overdefined) => {
                        operation_successors.iter().collect::<Vec<_>>()
                    }
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            for successor in successors {
                changed |= feasible_edges.insert(successor.psi_edge);
                changed |= reachable.insert(successor.target);
                for binding in &successor.bindings {
                    let incoming = values
                        .get(&binding.argument)
                        .copied()
                        .unwrap_or(ValidatorSccpValue::Overdefined);
                    let target = values
                        .entry(binding.parameter)
                        .or_insert(ValidatorSccpValue::Unknown);
                    changed |= merge(target, incoming);
                }
            }
        }
        if !changed {
            break;
        }
    }

    let snapshot = validator_sccp_snapshot(function, &values, &reachable, &feasible_edges);
    values
        .into_iter()
        .filter_map(|(value, state)| {
            let ValidatorSccpValue::Constant(constant) = state else {
                return None;
            };
            let definition = scalar_value_definition(function, value)?;
            let identity = literal_support
                .get(&value)
                .and_then(|support| {
                    literal_scalar_constant_fact_identity(
                        input,
                        function.machine,
                        definition,
                        constant,
                        *support,
                    )
                })
                .or_else(|| {
                    derived_sccp_scalar_constant_fact_identity(
                        input,
                        function.machine,
                        definition,
                        constant,
                        &snapshot,
                    )
                })?;
            Some((value, constant, identity))
        })
        .collect()
}

fn validator_scalar_operation_successors(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
) -> Vec<OptimizationEdge> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![omega_optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(*psi_edge),
                units: 1,
            }],
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|successor| OptimizationEdge {
                psi_edge: successor.psi_edge,
                target: successor.target,
                bindings: successor.bindings.clone(),
                provenance: vec![PsiProvenance::Edge(successor.psi_edge)],
                fuel: vec![omega_optimization_unit::FuelSettlement {
                    site: PsiProvenance::Edge(successor.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn validator_sccp_snapshot(
    function: &PsiOptimizationFunction,
    values: &BTreeMap<ValueId, ValidatorSccpValue>,
    reachable: &BTreeSet<BlockId>,
    feasible_edges: &BTreeSet<EdgeId>,
) -> SccpMachineSnapshot {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    let mut blocks = function
        .blocks
        .iter()
        .map(|block| SccpBlockRow {
            block: block.id,
            executable: reachable.contains(&block.id),
        })
        .collect::<Vec<_>>();
    blocks.sort_by_key(|row| row.block);
    let mut edges = function
        .blocks
        .iter()
        .flat_map(|block| {
            let reachable_source = reachable.contains(&block.id);
            block.nodes.last().into_iter().flat_map(move |node| {
                validator_scalar_operation_successors(&node.operation)
                    .into_iter()
                    .map(move |successor| {
                        let state = if feasible_edges.contains(&successor.psi_edge) {
                            SccpEdgeState::Executable
                        } else if !reachable_source {
                            SccpEdgeState::Inexecutable
                        } else if let O::Conditional { condition, .. } = &node.operation {
                            match values.get(condition) {
                                Some(ValidatorSccpValue::Constant(
                                    ScalarConstantValue::Boolean(_),
                                )) => SccpEdgeState::Inexecutable,
                                _ => SccpEdgeState::Unknown,
                            }
                        } else {
                            SccpEdgeState::Inexecutable
                        };
                        SccpEdgeRow {
                            source: block.id,
                            edge: successor.psi_edge,
                            target: successor.target,
                            state,
                        }
                    })
            })
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|row| (row.source, row.edge));
    let mut snapshot_values = values
        .iter()
        .filter_map(|(value, state)| {
            let definition = scalar_value_definition(function, *value)?;
            Some(SccpValueRow {
                definition,
                state: match state {
                    ValidatorSccpValue::Unknown => SccpValueState::Unknown,
                    ValidatorSccpValue::Constant(ScalarConstantValue::Boolean(value)) => {
                        SccpValueState::Boolean(*value)
                    }
                    ValidatorSccpValue::Constant(ScalarConstantValue::Integer(value)) => {
                        SccpValueState::Integer(*value)
                    }
                    ValidatorSccpValue::Overdefined => SccpValueState::Overdefined,
                },
            })
        })
        .collect::<Vec<_>>();
    snapshot_values.sort_by_key(|row| row.definition.value);
    SccpMachineSnapshot {
        blocks,
        edges,
        values: snapshot_values,
    }
}

fn scalar_value_definition(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<ValueDefinition> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.definitions),
        )
        .copied()
        .find(|definition| definition.value == value)
}

fn validator_integer_value_type(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<psi_core::IntegerType> {
    scalar_value_definition(function, value).and_then(|definition| match definition.scalar_type {
        ScalarType::Integer(integer) => Some(integer),
        ScalarType::Boolean => None,
    })
}

/// Independently validate both the reconstructible unit and the required
/// verifier context retained by the optimizer-facing constructor.
pub fn validate_verified_psi_optimization_unit(
    verified: &omega_terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_context(verified.input(), verified.unit(), true)
}

/// Validate a committed optimization revision while retaining the immutable
/// verifier context that authorized its proof and ownership facts.
///
/// Unlike [`validate_verified_psi_optimization_unit`], this permits the unit's
/// revision identity and executable shape to differ from the initial verified
/// seed. The admitted-fact projection and every surviving provenance frontier
/// must still match the original artifact exactly.
pub fn validate_transformed_psi_optimization_unit(
    input: &omega_terminal_psi_to_abstract_operations::VerifiedTerminalOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_context(input, unit, false)
}

fn validate_psi_optimization_unit_with_context(
    input: &omega_terminal_psi_to_abstract_operations::VerifiedTerminalOptimizationInput,
    unit: &PsiOptimizationUnit,
    require_initial_revision: bool,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(unit)?;
    let context = input.context();
    let terminal_identity = psi_terminal_codec::terminal_psi_identity(context.terminal_module())
        .map_err(OptimizationUnitValidationError::ContextIdentity)?;
    if input.plan().terminal_psi != terminal_identity || unit.terminal_psi != terminal_identity {
        return Err(OptimizationUnitValidationError::TerminalIdentityMismatch);
    }
    let proof_fingerprint = psi_terminal_codec::proof_bundle_fingerprint(context.proof_bundle())
        .map_err(OptimizationUnitValidationError::ContextProofFingerprint)?;
    if proof_fingerprint != context.proof_bundle_fingerprint() {
        return Err(OptimizationUnitValidationError::ProofFingerprintMismatch);
    }
    let ownership_frontiers = independently_project_ownership_frontiers(input)
        .ok_or(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)?;
    if ownership_frontiers != unit.ownership_frontier_facts {
        return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
    }

    let reconstructed = context
        .reconstructed_obligations()
        .obligations()
        .iter()
        .map(|row| (row.obligation.id, row))
        .collect::<BTreeMap<_, _>>();
    let accepted = context
        .accepted_facts()
        .iter()
        .map(|fact| (fact.obligation, fact))
        .collect::<BTreeMap<_, _>>();
    if reconstructed.len() != accepted.len() {
        let obligation = reconstructed
            .keys()
            .find(|id| !accepted.contains_key(id))
            .or_else(|| accepted.keys().find(|id| !reconstructed.contains_key(id)))
            .copied()
            .expect("different finite obligation maps have a differing key");
        return Err(OptimizationUnitValidationError::AcceptedObligationMismatch(
            obligation,
        ));
    }
    for (obligation, row) in &reconstructed {
        if accepted
            .get(obligation)
            .is_none_or(|fact| fact.proposition != row.obligation.proposition)
        {
            return Err(OptimizationUnitValidationError::AcceptedObligationMismatch(
                *obligation,
            ));
        }
    }

    let seed = omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
        input.plan(),
        unit.fuel_schedule,
    )
    .map_err(|_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
    if !same_immutable_signature_custody(&seed, unit) {
        return Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch);
    }
    let mut projected_facts = Vec::new();
    for function in &seed.functions {
        for reference in &function.facts {
            let OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = reference
            else {
                continue;
            };
            let row = reconstructed.get(obligation).filter(|row| {
                row.owner
                    == psi_terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                        machine: function.machine,
                        operation: *support,
                    }
            });
            let fact = accepted.get(obligation);
            let (Some(row), Some(fact)) = (row, fact) else {
                return Err(
                    OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch,
                );
            };
            if row.obligation.proposition != fact.proposition {
                return Err(
                    OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch,
                );
            }
            let proposition =
                psi_terminal_codec::canonical_proposition_order_key(&fact.proposition)
                    .map_err(OptimizationUnitValidationError::ContextIdentity)?;
            projected_facts.push(omega_optimization_unit::AcceptedObligationFact::new(
                seed.terminal_psi,
                *proof_fingerprint.as_bytes(),
                function.machine,
                *support,
                *obligation,
                proposition,
            ));
        }
    }
    let projected =
        omega_optimization_unit::attach_accepted_obligation_facts(seed, projected_facts).map_err(
            |_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch,
        )?;
    let projected =
        omega_optimization_unit::attach_ownership_frontier_facts(projected, ownership_frontiers)
            .map_err(|_| {
                OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch
            })?;
    if (require_initial_revision && projected.identity != unit.identity)
        || projected.accepted_obligation_facts != unit.accepted_obligation_facts
    {
        return Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch);
    }

    for function in &unit.functions {
        let Some(frontiers) = context.structural_frontiers().machine(function.machine) else {
            return Err(
                OptimizationUnitValidationError::MissingStructuralFrontierMachine(function.machine),
            );
        };
        for fact in &function.facts {
            let OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = fact
            else {
                continue;
            };
            let owner_matches = reconstructed.get(obligation).is_some_and(|row| {
                row.owner
                    == psi_terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                        machine: function.machine,
                        operation: *support,
                    }
            });
            if !owner_matches || !accepted.contains_key(obligation) {
                return Err(
                    OptimizationUnitValidationError::OperationObligationOwnerMismatch {
                        machine: function.machine,
                        operation: *support,
                        obligation: *obligation,
                    },
                );
            }
        }
        for site in function.blocks.iter().flat_map(|block| {
            block
                .nodes
                .iter()
                .flat_map(|node| node.provenance.iter().copied())
        }) {
            match site {
                PsiProvenance::Operation(operation)
                    if frontiers.operation_entry(operation).is_none()
                        || frontiers.operation_exit(operation).is_none() =>
                {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralOperationFrontier {
                            machine: function.machine,
                            operation,
                        },
                    );
                }
                PsiProvenance::Edge(edge) if frontiers.edge_entry(edge).is_none() => {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                            machine: function.machine,
                            edge,
                        },
                    );
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn independently_project_ownership_frontiers(
    input: &omega_terminal_psi_to_abstract_operations::VerifiedTerminalOptimizationInput,
) -> Option<Vec<OwnershipFrontierFact>> {
    let context = input.context();
    let mut facts = Vec::new();
    for machine in &context.terminal_module().machines {
        let frontiers = context.structural_frontiers().machine(machine.id)?;
        for block in &machine.blocks {
            push_independent_ownership_frontier(
                &mut facts,
                input.plan().terminal_psi,
                machine.id,
                OwnershipFrontierSite::BlockEntry(block.id),
                frontiers.block_entry(block.id)?,
            );
            for operation in &block.operations {
                push_independent_ownership_frontier(
                    &mut facts,
                    input.plan().terminal_psi,
                    machine.id,
                    OwnershipFrontierSite::OperationEntry(operation.id),
                    frontiers.operation_entry(operation.id)?,
                );
                push_independent_ownership_frontier(
                    &mut facts,
                    input.plan().terminal_psi,
                    machine.id,
                    OwnershipFrontierSite::OperationExit(operation.id),
                    frontiers.operation_exit(operation.id)?,
                );
            }
            for edge in block.terminator.edges() {
                push_independent_ownership_frontier(
                    &mut facts,
                    input.plan().terminal_psi,
                    machine.id,
                    OwnershipFrontierSite::EdgeEntry(edge),
                    frontiers.edge_entry(edge)?,
                );
                if let Some(snapshot) = frontiers.edge_exit(edge) {
                    push_independent_ownership_frontier(
                        &mut facts,
                        input.plan().terminal_psi,
                        machine.id,
                        OwnershipFrontierSite::EdgeExit(edge),
                        snapshot,
                    );
                }
            }
        }
    }
    facts.sort_by_key(|fact| (fact.machine, fact.site));
    Some(facts)
}

fn push_independent_ownership_frontier(
    facts: &mut Vec<OwnershipFrontierFact>,
    terminal_psi: psi_terminal::TerminalPsiIdentity,
    machine: MachineId,
    site: OwnershipFrontierSite,
    snapshot: &psi_terminal_verifier::VerifiedStructuralOwnershipFrontier,
) {
    facts.push(OwnershipFrontierFact::new(
        terminal_psi,
        machine,
        site,
        OwnershipFrontierSnapshot {
            claims: snapshot
                .claims()
                .iter()
                .map(|claim| OwnershipFrontierLiveClaim {
                    claim: claim.claim,
                    input: claim.input,
                    path: claim.path.clone(),
                    multiplicity: claim.multiplicity,
                })
                .collect(),
            owned_places: snapshot
                .owned_places()
                .iter()
                .map(|place| OwnershipFrontierOwnedPlace {
                    place: place.place,
                    multiplicity: place.multiplicity,
                })
                .collect(),
            partial_custody: snapshot
                .partial_custody()
                .iter()
                .map(|partial| OwnershipFrontierPartialCustody {
                    place: partial.place,
                    moved_paths: partial.moved_paths.clone(),
                })
                .collect(),
        },
    ));
}

fn same_immutable_signature_custody(
    seed: &PsiOptimizationUnit,
    unit: &PsiOptimizationUnit,
) -> bool {
    seed.terminal_psi == unit.terminal_psi
        && seed.entry == unit.entry
        && seed.structural_types == unit.structural_types
        && seed.boundary_machines == unit.boundary_machines
        && seed.provider_candidates == unit.provider_candidates
        && source_roster_partition_is_exact(seed, unit)
        && unit.functions.iter().all(|unit| {
            seed.functions
                .iter()
                .find(|seed| seed.machine == unit.machine)
                .is_some_and(|seed| {
                    seed.machine == unit.machine
                        && seed.attachment == unit.attachment
                        && seed.parameters == unit.parameters
                        && seed.structural_parameters == unit.structural_parameters
                        && seed.result == unit.result
                        && seed.entry_claim_declarations == unit.entry_claim_declarations
                        && seed.entry_claims == unit.entry_claims
                        && seed.published_service_ceiling == unit.published_service_ceiling
                })
        })
}

fn source_roster_partition_is_exact(
    seed: &PsiOptimizationUnit,
    unit: &PsiOptimizationUnit,
) -> bool {
    if unit
        .pruned_machines
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return false;
    }
    let active = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|row| (row.source_ordinal, row.machine))
        .collect::<BTreeMap<_, _>>();
    if active.len() != unit.functions.len() || active.len() + pruned.len() != seed.functions.len() {
        return false;
    }
    let mut active_order = unit.functions.iter().map(|function| function.machine);
    for (ordinal, source) in seed.functions.iter().enumerate() {
        let ordinal = u32::try_from(ordinal).ok();
        if active.contains(&source.machine) {
            if active_order.next() != Some(source.machine) {
                return false;
            }
        } else if ordinal.and_then(|ordinal| pruned.get(&ordinal).copied()) != Some(source.machine)
        {
            return false;
        }
    }
    active_order.next().is_none()
}

fn validate_function(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let indexed_entry_claims = function
        .entry_claim_declarations
        .iter()
        .map(|claim| claim.claim)
        .collect::<BTreeSet<_>>();
    if indexed_entry_claims.len() != function.entry_claim_declarations.len()
        || indexed_entry_claims != function.entry_claims
    {
        return Err(OptimizationUnitValidationError::EntryClaimIndexMismatch(
            function.machine,
        ));
    }
    let mut blocks = BTreeMap::new();
    for block in &function.blocks {
        if blocks.insert(block.id, block).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateBlock {
                machine: function.machine,
                block: block.id,
            });
        }
    }
    if !blocks.contains_key(&function.entry) {
        return Err(OptimizationUnitValidationError::MissingEntryBlock {
            machine: function.machine,
            block: function.entry,
        });
    }
    if !blocks[&function.entry].parameters.is_empty() {
        return Err(OptimizationUnitValidationError::EntryBlockHasParameters {
            machine: function.machine,
            block: function.entry,
        });
    }
    validate_parameter_metadata(function)?;

    let mut edge_ids = BTreeSet::new();
    let mut predecessor = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut successors = function
        .blocks
        .iter()
        .map(|block| (block.id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &function.blocks {
        if block.nodes.is_empty() {
            return Err(OptimizationUnitValidationError::EmptyBlock {
                machine: function.machine,
                block: block.id,
            });
        }
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index).expect("unit node index was built as u32");
            if !provenance_matches_operation(&node.operation, &node.provenance)
                || node.definitions != expected_definitions(&node.operation, block.id, node_index)
                || node.uses != expected_uses(&node.operation, block.id, node_index)
                || !successors_match_operation(&node.operation, &node.successors)
                || node.ownership != expected_ownership(&node.operation)
            {
                return Err(OptimizationUnitValidationError::OperationMetadataMismatch {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                });
            }
            let terminal = is_terminator(&node.operation);
            if terminal && index + 1 != block.nodes.len() {
                return Err(OptimizationUnitValidationError::TerminatorNotLast {
                    machine: function.machine,
                    block: block.id,
                });
            }
            for edge in &node.successors {
                if !blocks.contains_key(&edge.target) {
                    return Err(OptimizationUnitValidationError::UnknownSuccessor {
                        machine: function.machine,
                        block: block.id,
                        target: edge.target,
                    });
                }
                if !edge_ids.insert(edge.psi_edge) {
                    return Err(OptimizationUnitValidationError::DuplicateEdge(
                        edge.psi_edge,
                    ));
                }
                predecessor
                    .get_mut(&edge.target)
                    .expect("known target")
                    .insert(block.id);
                successors
                    .get_mut(&block.id)
                    .expect("every block has a successor row")
                    .push(edge.target);
            }
        }
        if !is_terminator(&block.nodes.last().expect("nonempty").operation) {
            return Err(OptimizationUnitValidationError::MissingTerminator {
                machine: function.machine,
                block: block.id,
            });
        }
    }

    validate_total_cfg(function, &blocks, &successors)?;

    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        let matches = match (operation, &function.result) {
            (
                omega_terminal_abstract_operations::TerminalAbstractOperation::Return {
                    result,
                    scalar_type,
                    ..
                },
                omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Scalar(
                    signature,
                ),
            ) => *result == signature.value && *scalar_type == signature.scalar_type,
            (
                omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
                    ..
                },
                omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Unit,
            )
            | (
                omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnStructural {
                    ..
                },
                omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Structural(_),
            ) => true,
            (
                omega_terminal_abstract_operations::TerminalAbstractOperation::Return { .. }
                | omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
                    ..
                }
                | omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnStructural {
                    ..
                },
                _,
            ) => false,
            _ => continue,
        };
        if !matches {
            return Err(OptimizationUnitValidationError::FunctionResultMismatch(
                function.machine,
            ));
        }
    }

    validate_provenance_fuel_effects(function)?;
    validate_fact_index(function)?;
    validate_values_and_bindings(
        function,
        &blocks,
        &predecessor,
        functions,
        boundary_machines,
    )?;
    validate_places_and_claims(function)?;
    Ok(())
}

fn validate_parameter_metadata(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    for (position, parameter) in function.parameters.iter().enumerate() {
        let Ok(position) = u32::try_from(position) else {
            return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                machine: function.machine,
                block: None,
            });
        };
        if parameter.site != ValueDefinitionSite::FunctionParameter(position) {
            return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                machine: function.machine,
                block: None,
            });
        }
    }
    for block in &function.blocks {
        for (position, parameter) in block.parameters.iter().enumerate() {
            let Ok(position) = u32::try_from(position) else {
                return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                    machine: function.machine,
                    block: Some(block.id),
                });
            };
            if parameter.site
                != (ValueDefinitionSite::BlockParameter {
                    block: block.id,
                    position,
                })
            {
                return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                    machine: function.machine,
                    block: Some(block.id),
                });
            }
        }
    }
    Ok(())
}

fn validate_total_cfg(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(successors[&block].iter().copied());
        }
    }
    if reachable.len() != blocks.len() {
        let block = blocks
            .keys()
            .find(|block| !reachable.contains(block))
            .copied()
            .expect("different block counts have an unreachable block");
        return Err(OptimizationUnitValidationError::UnreachableBlock {
            machine: function.machine,
            block,
        });
    }

    let mut indegree = blocks
        .keys()
        .copied()
        .map(|block| (block, 0usize))
        .collect::<BTreeMap<_, _>>();
    for target in successors.values().flatten() {
        *indegree.get_mut(target).expect("successor was validated") += 1;
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut visited = 0usize;
    while let Some(block) = ready.pop_first() {
        visited += 1;
        for target in &successors[&block] {
            let count = indegree.get_mut(target).expect("successor was validated");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }
    if visited != blocks.len() {
        let block = indegree
            .iter()
            .find_map(|(block, count)| (*count != 0).then_some(*block))
            .expect("a cyclic graph leaves positive indegree");
        return Err(OptimizationUnitValidationError::ControlCycle {
            machine: function.machine,
            block,
        });
    }
    Ok(())
}

fn validate_fact_index(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let expected = reconstruct_fact_index(function);
    if expected != function.facts {
        return Err(OptimizationUnitValidationError::FactIndexMismatch(
            function.machine,
        ));
    }
    Ok(())
}

fn reconstruct_fact_index(function: &PsiOptimizationFunction) -> Vec<OptimizationFact> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;

    let mut expected = Vec::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        match operation {
            O::IntegerExactCast {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerAdd {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                ..
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                ..
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                ..
            } => expected.push(OptimizationFact::OperationObligationReference {
                obligation: *obligation,
                support: *psi_operation,
            }),
            _ => {}
        }
        match operation {
            O::BooleanConstant {
                psi_operation,
                result,
                value,
            } => expected.push(OptimizationFact::BooleanConstant {
                value: *result,
                constant: *value,
                support: *psi_operation,
            }),
            O::IntegerConstant {
                psi_operation,
                result,
                value,
                ..
            } => expected.push(OptimizationFact::IntegerConstant {
                value: *result,
                constant: *value,
                support: *psi_operation,
            }),
            _ => {}
        }
    }
    expected
}

fn validate_provenance_fuel_effects(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let mut node_provenance = BTreeMap::<PsiProvenance, Vec<(BlockId, bool)>>::new();
    let mut edge_provenance = BTreeMap::<PsiProvenance, BTreeSet<EdgeId>>::new();
    let mut edge_shapes = BTreeMap::<EdgeId, (BlockId, BlockId)>::new();
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let index = u32::try_from(index).expect("unit node index was built as u32");
            if node.provenance.is_empty() && node.successors.is_empty() {
                return Err(OptimizationUnitValidationError::IncompleteProvenance {
                    machine: function.machine,
                    block: block.id,
                    node: index,
                });
            }
            let unique_node_sources = node.provenance.iter().copied().collect::<BTreeSet<_>>();
            if unique_node_sources.len() != node.provenance.len() {
                return Err(OptimizationUnitValidationError::DuplicateProvenance(
                    *node
                        .provenance
                        .first()
                        .expect("duplicated provenance is nonempty"),
                ));
            }
            let is_exact_terminal = node.successors.is_empty()
                && matches!(
                    node.operation,
                    O::Return { .. }
                        | O::ReturnUnit { .. }
                        | O::ReturnStructural { .. }
                        | O::Crash { .. }
                );
            for site in &node.provenance {
                if edge_provenance.contains_key(site) {
                    return Err(OptimizationUnitValidationError::DuplicateProvenance(*site));
                }
                node_provenance
                    .entry(*site)
                    .or_default()
                    .push((block.id, is_exact_terminal));
            }
            let source_sites = node.provenance.iter().copied().collect::<BTreeSet<_>>();
            let settled_sites = node
                .fuel
                .iter()
                .map(|settlement| settlement.site)
                .collect::<BTreeSet<_>>();
            if source_sites != settled_sites
                || node.fuel.len() != node.provenance.len()
                || node
                    .fuel
                    .iter()
                    .zip(&node.provenance)
                    .any(|(settlement, source)| settlement.site != *source || settlement.units != 1)
            {
                return Err(
                    OptimizationUnitValidationError::FuelDoesNotMatchProvenance {
                        machine: function.machine,
                        block: block.id,
                        node: index,
                    },
                );
            }
            for settlement in &node.fuel {
                let _ = settlement;
            }
            for edge in &node.successors {
                edge_shapes.insert(edge.psi_edge, (block.id, edge.target));
                if edge.provenance.is_empty()
                    || edge.provenance.first() != Some(&PsiProvenance::Edge(edge.psi_edge))
                    || edge
                        .provenance
                        .iter()
                        .any(|site| !matches!(site, PsiProvenance::Edge(_)))
                {
                    return Err(OptimizationUnitValidationError::IncompleteProvenance {
                        machine: function.machine,
                        block: block.id,
                        node: index,
                    });
                }
                let source_sites = edge.provenance.iter().copied().collect::<BTreeSet<_>>();
                if source_sites.len() != edge.provenance.len()
                    || node_provenance
                        .keys()
                        .any(|site| source_sites.contains(site))
                {
                    return Err(OptimizationUnitValidationError::DuplicateProvenance(
                        *edge
                            .provenance
                            .first()
                            .expect("edge provenance is nonempty"),
                    ));
                }
                if edge.fuel.len() != edge.provenance.len()
                    || edge
                        .fuel
                        .iter()
                        .zip(&edge.provenance)
                        .any(|(settlement, source)| {
                            settlement.site != *source || settlement.units != 1
                        })
                {
                    return Err(
                        OptimizationUnitValidationError::FuelDoesNotMatchProvenance {
                            machine: function.machine,
                            block: block.id,
                            node: index,
                        },
                    );
                }
                for source in &edge.provenance {
                    edge_provenance
                        .entry(*source)
                        .or_default()
                        .insert(edge.psi_edge);
                }
            }
            if node.effect.input != expected_effect || node.effect.output != expected_effect + 1 {
                return Err(OptimizationUnitValidationError::BrokenEffectChain {
                    machine: function.machine,
                    expected: expected_effect,
                    actual: node.effect.input,
                });
            }
            expected_effect += 1;
        }
    }
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .iter()
                    .flat_map(|node| node.successors.iter().map(|edge| edge.target))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (source, occurrences) in node_provenance {
        if occurrences.len() < 2 {
            continue;
        }
        if !matches!(source, PsiProvenance::Edge(_))
            || occurrences.iter().any(|(_, terminal)| !terminal)
        {
            return Err(OptimizationUnitValidationError::DuplicateProvenance(source));
        }
        for (index, (left, _)) in occurrences.iter().enumerate() {
            for (right, _) in &occurrences[index + 1..] {
                if left == right
                    || block_reaches(&successors, *left, *right)
                    || block_reaches(&successors, *right, *left)
                {
                    return Err(
                        OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source),
                    );
                }
            }
        }
    }
    for (source, occurrences) in edge_provenance {
        let occurrences = occurrences.into_iter().collect::<Vec<_>>();
        for (index, left) in occurrences.iter().enumerate() {
            let (_, left_target) = edge_shapes[left];
            for right in &occurrences[index + 1..] {
                let (right_owner, right_target) = edge_shapes[right];
                let (left_owner, _) = edge_shapes[left];
                if block_reaches(&successors, left_target, right_owner)
                    || block_reaches(&successors, right_target, left_owner)
                {
                    return Err(
                        OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source),
                    );
                }
            }
        }
    }
    Ok(())
}

fn block_reaches(
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
    start: BlockId,
    target: BlockId,
) -> bool {
    let mut visited = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(block) = pending.pop() {
        if block == target {
            return true;
        }
        if visited.insert(block) {
            pending.extend(successors.get(&block).into_iter().flatten().copied());
        }
    }
    false
}

fn validate_values_and_bindings(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut definitions = BTreeMap::new();
    for definition in function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
    {
        if definitions.insert(definition.value, *definition).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateValue(
                definition.value,
            ));
        }
    }

    let dominators = dominators(function.entry, blocks.keys().copied(), predecessors);
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            for use_site in &node.uses {
                let Some(definition) = definitions.get(&use_site.value) else {
                    return Err(OptimizationUnitValidationError::UndefinedValue {
                        machine: function.machine,
                        block: block.id,
                        value: use_site.value,
                    });
                };
                match definition.site {
                    ValueDefinitionSite::FunctionParameter(_) => {}
                    ValueDefinitionSite::BlockParameter {
                        block: defining, ..
                    } => {
                        if !dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(&defining))
                        {
                            return Err(OptimizationUnitValidationError::NondominatingValue {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                    ValueDefinitionSite::Node {
                        block: defining,
                        node,
                    } if defining == block.id => {
                        if usize::try_from(node).expect("u32 fits usize") >= node_index {
                            return Err(OptimizationUnitValidationError::UseBeforeDefinition {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                    ValueDefinitionSite::Node {
                        block: defining, ..
                    } => {
                        if !dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(&defining))
                        {
                            return Err(OptimizationUnitValidationError::NondominatingValue {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                }
            }
            if !operation_scalar_types_match(
                function,
                &node.operation,
                &definitions,
                functions,
                boundary_machines,
            ) {
                return Err(
                    OptimizationUnitValidationError::ScalarOperationContractMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("unit node index fits u32"),
                    },
                );
            }
            for edge in &node.successors {
                let target = blocks.get(&edge.target).expect("successor validated");
                if edge.bindings.len() != target.parameters.len() {
                    return Err(OptimizationUnitValidationError::BindingArityMismatch {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    });
                }
                for (binding, parameter) in edge.bindings.iter().zip(&target.parameters) {
                    let source_type = definitions
                        .get(&binding.argument)
                        .map(|row| row.scalar_type);
                    if binding.parameter != parameter.value
                        || binding.scalar_type != parameter.scalar_type
                        || source_type != Some(parameter.scalar_type)
                    {
                        return Err(OptimizationUnitValidationError::BindingTypeMismatch {
                            machine: function.machine,
                            edge: edge.psi_edge,
                            value: binding.argument,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn operation_scalar_types_match(
    function: &PsiOptimizationFunction,
    operation: &O,
    definitions: &BTreeMap<ValueId, ValueDefinition>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
) -> bool {
    let scalar = |value: ValueId| definitions.get(&value).map(|row| row.scalar_type);
    let integer = |value: ValueId, expected: IntegerType| {
        scalar(value) == Some(ScalarType::Integer(expected))
    };
    let fixed = |integer: IntegerType| integer.carrier() == IntegerCarrier::Fixed;
    let binary = |left: ValueId, right: ValueId, expected: IntegerType| {
        integer(left, expected) && integer(right, expected)
    };
    match operation {
        O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::PortWrite { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => true,
        O::IntegerConstant {
            scalar_type, value, ..
        } => match scalar_type {
            ScalarType::Integer(integer) => integer.admits(*value),
            ScalarType::Boolean => false,
        },
        O::BooleanConstant { .. } => true,
        O::BooleanNot { operand, .. } => scalar(*operand) == Some(ScalarType::Boolean),
        O::BooleanEqual { left, right, .. } => {
            scalar(*left) == Some(ScalarType::Boolean)
                && scalar(*right) == Some(ScalarType::Boolean)
        }
        O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. } => {
            matches!(scalar(*left), Some(ScalarType::Integer(_)))
                && scalar(*left) == scalar(*right)
        }
        O::IntegerBitwiseNot {
            scalar_type,
            operand,
            ..
        } => integer(*operand, *scalar_type),
        O::IntegerWiden {
            source_type,
            target_type,
            operand,
            ..
        } => integer(*operand, *source_type) && source_type.can_widen_to(*target_type),
        O::IntegerExactCast {
            source_type,
            target_type,
            operand,
            ..
        } => {
            integer(*operand, *source_type)
                && source_type.can_exact_cast_to(*target_type)
                && !source_type.can_widen_to(*target_type)
                && source_type != target_type
        }
        O::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
            ..
        }
        | O::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        } => binary(*left, *right, *scalar_type),
        O::ExactIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        } => fixed(*scalar_type) && binary(*left, *right, *scalar_type),
        O::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | O::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
            ..
        } => integer(*value, *value_type) && integer(*count, *count_type),
        O::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | O::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
            ..
        } => {
            fixed(*value_type)
                && fixed(*count_type)
                && integer(*value, *value_type)
                && integer(*count, *count_type)
        }
        O::Jump { .. } => true,
        O::Conditional { condition, .. } => scalar(*condition) == Some(ScalarType::Boolean),
        O::Return {
            result,
            value,
            scalar_type,
            ..
        } => {
            scalar(*value) == Some(*scalar_type)
                && matches!(
                    function.result,
                    omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Scalar(signature)
                        if signature.value == *result && signature.scalar_type == *scalar_type
                )
        }
        O::Call {
            result: _,
            scalar_type,
            callee,
            arguments,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            callee.structural_parameters.is_empty()
                && callee.declared_places.is_empty()
                && callee.entry_claim_declarations.is_empty()
                && matches!(
                    callee.result,
                    omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Scalar(signature)
                        if signature.scalar_type == *scalar_type
                )
                && arguments.len() == callee.parameters.len()
                && arguments.iter().zip(&callee.parameters).all(|(argument, parameter)| {
                    scalar(*argument) == Some(parameter.scalar_type)
                })
        }),
        O::CallUnit { callee, .. } => functions.get(callee).is_some_and(|callee| {
            callee.parameters.is_empty()
                && matches!(
                    callee.result,
                    omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Unit
                )
        }),
        O::CallStructuralScalar { result, callee, .. } => {
            functions.get(callee).is_some_and(|callee| {
                callee.parameters.is_empty()
                    && matches!(
                        callee.result,
                        omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Scalar(signature)
                            if signature.scalar_type == result.scalar_type
                    )
            })
        }
        O::CallStructural { callee, .. } => functions.get(callee).is_some_and(|callee| {
            callee.parameters.is_empty()
                && matches!(
                    callee.result,
                    omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Structural(_)
                )
        }),
        O::BoundaryCall {
            result,
            boundary,
            arguments,
            ..
        } => boundary_machines.get(boundary).is_some_and(|boundary| {
            result.as_ref().map(|result| result.scalar_type) == boundary.result
                && arguments.len() == boundary.scalar_parameters.len()
                && arguments
                    .iter()
                    .zip(&boundary.scalar_parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(*parameter))
        }),
    }
}

fn dominators(
    entry: BlockId,
    block_ids: impl Iterator<Item = BlockId>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let all = block_ids.collect::<BTreeSet<_>>();
    let mut result = all
        .iter()
        .copied()
        .map(|block| {
            let initial = if block == entry {
                [entry].into_iter().collect()
            } else {
                all.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in all.iter().copied().filter(|block| *block != entry) {
            let incoming = predecessors.get(&block).expect("all blocks indexed");
            let mut next = if let Some(first) = incoming.first() {
                result[first].clone()
            } else {
                BTreeSet::new()
            };
            for predecessor in incoming.iter().skip(1) {
                next = next.intersection(&result[predecessor]).copied().collect();
            }
            next.insert(block);
            if result[&block] != next {
                result.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

fn validate_places_and_claims(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let known_places = reconstruct_declared_places(function)?;
    for parameter in &function.structural_parameters {
        if !function.declared_places.contains(&parameter.place) {
            return Err(OptimizationUnitValidationError::UnknownPlace {
                machine: function.machine,
                place: parameter.place,
            });
        }
    }
    for block in &function.blocks {
        for node in &block.nodes {
            for event in &node.ownership {
                let claims: &[ClaimId] = match event {
                    omega_optimization_unit::OwnershipEvent::ClaimTransfer(claims)
                    | omega_optimization_unit::OwnershipEvent::ClaimCompletion(claims)
                    | omega_optimization_unit::OwnershipEvent::StructuralReturn(claims)
                    | omega_optimization_unit::OwnershipEvent::CrashFrontier(claims) => claims,
                    omega_optimization_unit::OwnershipEvent::Cleanup(_) => continue,
                };
                for claim in claims {
                    if !function.entry_claims.contains(claim) {
                        return Err(OptimizationUnitValidationError::UnknownClaim {
                            machine: function.machine,
                            claim: *claim,
                        });
                    }
                }
            }
        }
    }
    if known_places != function.declared_places {
        let place = known_places
            .symmetric_difference(&function.declared_places)
            .next()
            .copied()
            .expect("different place sets have a difference");
        return Err(OptimizationUnitValidationError::UnknownPlace {
            machine: function.machine,
            place,
        });
    }
    Ok(())
}

fn reconstruct_declared_places(
    function: &PsiOptimizationFunction,
) -> Result<BTreeSet<PlaceId>, OptimizationUnitValidationError> {
    let mut known_places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .chain(
            function
                .entry_claim_declarations
                .iter()
                .map(|claim| claim.input),
        )
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        for node in &block.nodes {
            validate_operation_places(function.machine, &node.operation, &mut known_places)?;
        }
    }
    Ok(known_places)
}

fn validate_operation_places(
    machine: MachineId,
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
    known: &mut BTreeSet<PlaceId>,
) -> Result<(), OptimizationUnitValidationError> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    let require = |place: PlaceId, known: &BTreeSet<PlaceId>| {
        if known.contains(&place) {
            Ok(())
        } else {
            Err(OptimizationUnitValidationError::UnknownPlace { machine, place })
        }
    };
    match operation {
        O::EstablishByteSequenceLiteral { place, .. }
        | O::EstablishTrivialAffineLocal { place, .. } => {
            known.insert(place.id);
        }
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments {
                require(argument.place, known)?;
            }
            if let O::CallStructural { result, .. } = operation {
                known.insert(result.place);
            }
        }
        O::BooleanStructuralField { source, .. } | O::ReturnStructural { source, .. } => {
            require(*source, known)?;
        }
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
            for cleanup in cleanup_actions {
                let place = match cleanup {
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => *place,
                    psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        discard.place
                    }
                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        cleanup.place
                    }
                };
                require(place, known)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn expected_definitions(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueDefinition> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    let definition = match operation {
        O::Call {
            result,
            scalar_type,
            ..
        }
        | O::IntegerConstant {
            result,
            scalar_type,
            ..
        } => Some((*result, *scalar_type)),
        O::CallStructuralScalar { result, .. } => Some((result.value, result.scalar_type)),
        O::BoundaryCall {
            result: Some(result),
            ..
        } => Some((result.value, result.scalar_type)),
        O::BooleanConstant { result, .. }
        | O::BooleanStructuralField { result, .. }
        | O::BooleanNot { result, .. }
        | O::BooleanEqual { result, .. }
        | O::IntegerEqual { result, .. }
        | O::IntegerLessThan { result, .. }
        | O::IntegerLessOrEqual { result, .. } => Some((*result, ScalarType::Boolean)),
        O::IntegerBitwiseNot {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseAnd {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseOr {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseXor {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerMultiply {
            result,
            scalar_type,
            ..
        } => Some((*result, ScalarType::Integer(*scalar_type))),
        O::IntegerWiden {
            result,
            target_type,
            ..
        }
        | O::IntegerExactCast {
            result,
            target_type,
            ..
        } => Some((*result, ScalarType::Integer(*target_type))),
        O::WrappingIntegerShiftLeft {
            result, value_type, ..
        }
        | O::WrappingIntegerShiftRight {
            result, value_type, ..
        }
        | O::ExactIntegerShiftLeft {
            result, value_type, ..
        }
        | O::ExactIntegerShiftRight {
            result, value_type, ..
        } => Some((*result, ScalarType::Integer(*value_type))),
        _ => None,
    };
    definition
        .into_iter()
        .map(|(value, scalar_type)| ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::Node { block, node },
        })
        .collect()
}

fn expected_uses(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueUse> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    let values = match operation {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => arguments.clone(),
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => vec![*operand],
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. } => vec![*left, *right],
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => vec![*value, *count],
        O::Jump { bindings, .. } => bindings.iter().map(|binding| binding.argument).collect(),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => std::iter::once(*condition)
            .chain(when_true.bindings.iter().map(|binding| binding.argument))
            .chain(when_false.bindings.iter().map(|binding| binding.argument))
            .collect(),
        O::Return { value, .. } => vec![*value],
        _ => Vec::new(),
    };
    values
        .into_iter()
        .map(|value| ValueUse { value, block, node })
        .collect()
}

fn expected_provenance(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
) -> Vec<PsiProvenance> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    match operation {
        O::Jump { .. } | O::Conditional { .. } => Vec::new(),
        O::Return { psi_edge, .. }
        | O::ReturnUnit { psi_edge, .. }
        | O::ReturnStructural { psi_edge, .. }
        | O::Crash { psi_edge, .. } => vec![PsiProvenance::Edge(*psi_edge)],
        O::EstablishByteSequenceLiteral { psi_operation, .. }
        | O::EstablishTrivialAffineLocal { psi_operation, .. }
        | O::CallUnit { psi_operation, .. }
        | O::CallStructuralScalar { psi_operation, .. }
        | O::CallStructural { psi_operation, .. }
        | O::BoundaryCall { psi_operation, .. }
        | O::PortWrite { psi_operation, .. }
        | O::Call { psi_operation, .. }
        | O::IntegerConstant { psi_operation, .. }
        | O::BooleanConstant { psi_operation, .. }
        | O::BooleanStructuralField { psi_operation, .. }
        | O::BooleanNot { psi_operation, .. }
        | O::BooleanEqual { psi_operation, .. }
        | O::IntegerEqual { psi_operation, .. }
        | O::IntegerLessThan { psi_operation, .. }
        | O::IntegerLessOrEqual { psi_operation, .. }
        | O::IntegerBitwiseNot { psi_operation, .. }
        | O::IntegerWiden { psi_operation, .. }
        | O::IntegerExactCast { psi_operation, .. }
        | O::IntegerBitwiseAnd { psi_operation, .. }
        | O::IntegerBitwiseOr { psi_operation, .. }
        | O::IntegerBitwiseXor { psi_operation, .. }
        | O::WrappingIntegerShiftLeft { psi_operation, .. }
        | O::WrappingIntegerShiftRight { psi_operation, .. }
        | O::ExactIntegerShiftLeft { psi_operation, .. }
        | O::ExactIntegerShiftRight { psi_operation, .. }
        | O::WrappingIntegerAdd { psi_operation, .. }
        | O::ExactIntegerAdd { psi_operation, .. }
        | O::SaturatingIntegerAdd { psi_operation, .. }
        | O::WrappingIntegerSubtract { psi_operation, .. }
        | O::ExactIntegerSubtract { psi_operation, .. }
        | O::SaturatingIntegerSubtract { psi_operation, .. }
        | O::WrappingIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerDivide { psi_operation, .. }
        | O::ExactIntegerRemainder { psi_operation, .. }
        | O::WrappingIntegerDivide { psi_operation, .. }
        | O::WrappingIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerDivide { psi_operation, .. }
        | O::SaturatingIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerMultiply { psi_operation, .. } => {
            vec![PsiProvenance::Operation(*psi_operation)]
        }
    }
}

fn provenance_matches_operation(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
    provenance: &[PsiProvenance],
) -> bool {
    let expected = expected_provenance(operation);
    if expected.is_empty() {
        matches!(operation, O::Jump { .. } | O::Conditional { .. }) || provenance.is_empty()
    } else {
        provenance.starts_with(&expected)
    }
}

fn successors_match_operation(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
    actual: &[OptimizationEdge],
) -> bool {
    let expected = expected_edges(operation);
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            actual.psi_edge == expected.psi_edge
                && actual.target == expected.target
                && actual.bindings == expected.bindings
                && actual.provenance.first() == Some(&PsiProvenance::Edge(actual.psi_edge))
                && actual
                    .provenance
                    .iter()
                    .all(|source| matches!(source, PsiProvenance::Edge(_)))
        })
}

fn expected_edges(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
) -> Vec<OptimizationEdge> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![omega_optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(*psi_edge),
                units: 1,
            }],
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|edge| OptimizationEdge {
                psi_edge: edge.psi_edge,
                target: edge.target,
                bindings: edge.bindings.clone(),
                provenance: vec![PsiProvenance::Edge(edge.psi_edge)],
                fuel: vec![omega_optimization_unit::FuelSettlement {
                    site: PsiProvenance::Edge(edge.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn expected_ownership(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
) -> Vec<OwnershipEvent> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    match operation {
        O::CallUnit {
            claim_transfers, ..
        }
        | O::CallStructuralScalar {
            claim_transfers, ..
        }
        | O::CallStructural {
            claim_transfers, ..
        } => vec![OwnershipEvent::ClaimTransfer(
            claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect(),
        )],
        O::BoundaryCall {
            completion_receipts,
            ..
        } => vec![OwnershipEvent::ClaimCompletion(
            completion_receipts
                .iter()
                .map(|receipt| receipt.claim)
                .collect(),
        )],
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => vec![OwnershipEvent::Cleanup(cleanup_actions.clone())],
        O::ReturnStructural {
            returned_claims, ..
        } => vec![OwnershipEvent::StructuralReturn(returned_claims.clone())],
        O::Crash {
            frontier_lower_bound,
            ..
        } => vec![OwnershipEvent::CrashFrontier(frontier_lower_bound.clone())],
        _ => Vec::new(),
    }
}

fn is_terminator(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
) -> bool {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    matches!(
        operation,
        O::Jump { .. }
            | O::Conditional { .. }
            | O::Return { .. }
            | O::ReturnUnit { .. }
            | O::ReturnStructural { .. }
            | O::Crash { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_optimization_core::{
        AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
        OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
    };
    use omega_optimization_unit::{
        IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceRewrite,
        PsiRewriteCandidate, ValueUse, reconstruct_psi_optimization_unit_seed,
    };
    use omega_terminal_abstract_operations::{
        TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractFunctionResult,
        TerminalAbstractOperation, TerminalAbstractOperationPlan, TerminalAbstractParameter,
        TerminalAbstractResult,
    };
    use psi_core::{
        FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType,
        ValueId,
    };
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn refresh_identity(unit: &mut PsiOptimizationUnit) {
        unit.identity = recompute_psi_optimization_unit_identity(unit);
    }

    fn refresh_node_derivatives(
        unit: &mut PsiOptimizationUnit,
        function_index: usize,
        block_index: usize,
        node_index: usize,
    ) {
        let block = unit.functions[function_index].blocks[block_index].id;
        let node_index = u32::try_from(node_index).expect("test node index fits u32");
        let operation = unit.functions[function_index].blocks[block_index].nodes
            [node_index as usize]
            .operation
            .clone();
        let node =
            &mut unit.functions[function_index].blocks[block_index].nodes[node_index as usize];
        node.definitions = expected_definitions(&operation, block, node_index);
        node.uses = expected_uses(&operation, block, node_index);
        node.provenance = expected_provenance(&operation);
        node.successors = expected_edges(&operation);
        node.ownership = expected_ownership(&operation);
        unit.functions[function_index].facts =
            reconstruct_fact_index(&unit.functions[function_index]);
        refresh_identity(unit);
    }

    fn verified_unit() -> omega_terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit {
        use psi_terminal::{
            Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule,
            Terminator,
        };

        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: id(101, MachineId::new),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: id(101, MachineId::new),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: id(102, BlockId::new),
                blocks: vec![Block {
                    id: id(102, BlockId::new),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: id(103, EdgeId::new),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: id(104, psi_core::ContractId::new),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = psi_terminal_verifier::ProofBundle::default();
        let semantic = psi_terminal_codec::encode_module(&module).expect("encode unit module");
        let proof = psi_terminal_codec::encode_proof_bundle(&proof).expect("encode empty proof");
        let input =
            omega_terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
                &semantic,
                &proof,
                &psi_proof_admission::AdmissionProfile::default(),
            )
            .expect("verified optimizer input");
        omega_terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            TerminalFuelSchedule::CURRENT.identity(),
        )
        .expect("verified optimizer unit")
    }

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    fn unit() -> PsiOptimizationUnit {
        let machine = id(1, MachineId::new);
        let block = id(2, BlockId::new);
        let result = id(3, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("valid width");
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([11; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![TerminalAbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: result,
                    scalar_type: ScalarType::Integer(integer),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![TerminalAbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    TerminalAbstractOperation::IntegerConstant {
                        psi_operation: id(4, OperationId::new),
                        result,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(7),
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: id(5, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        };
        reconstruct_psi_optimization_unit_seed(
            &plan,
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .expect("valid unit")
    }

    fn exact_add_unit() -> PsiOptimizationUnit {
        let machine = id(201, MachineId::new);
        let block = id(202, BlockId::new);
        let left = id(203, ValueId::new);
        let right = id(204, ValueId::new);
        let result = id(205, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([12; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![TerminalAbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: result,
                    scalar_type: ScalarType::Integer(integer),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![TerminalAbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    TerminalAbstractOperation::IntegerConstant {
                        psi_operation: id(206, OperationId::new),
                        result: left,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(7),
                    },
                    TerminalAbstractOperation::IntegerConstant {
                        psi_operation: id(207, OperationId::new),
                        result: right,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(8),
                    },
                    TerminalAbstractOperation::ExactIntegerAdd {
                        psi_operation: id(208, OperationId::new),
                        obligation: id(209, psi_core::ObligationId::new),
                        result,
                        scalar_type: integer,
                        left,
                        right,
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: id(210, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        };
        let unit =
            reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
                .unwrap();
        omega_optimization_unit::attach_accepted_obligation_facts(
            unit.clone(),
            vec![omega_optimization_unit::AcceptedObligationFact::new(
                unit.terminal_psi,
                [23; 32],
                machine,
                id(208, OperationId::new),
                id(209, psi_core::ObligationId::new),
                b"validation-test-obligation".to_vec(),
            )],
        )
        .unwrap()
    }

    fn scalar_call_unit() -> PsiOptimizationUnit {
        let caller = id(301, MachineId::new);
        let callee = id(302, MachineId::new);
        let caller_block = id(303, BlockId::new);
        let callee_block = id(304, BlockId::new);
        let argument = id(305, ValueId::new);
        let caller_result = id(306, ValueId::new);
        let parameter = id(307, ValueId::new);
        let callee_result = id(308, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([13; 32]),
            },
            entry: caller,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                TerminalAbstractFunction {
                    machine: caller,
                    attachment: None,
                    entry: caller_block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: caller_result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block: caller_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(309, OperationId::new),
                            result: argument,
                            scalar_type,
                            value: IntegerValue::Unsigned(7),
                        },
                        TerminalAbstractOperation::Call {
                            psi_operation: id(310, OperationId::new),
                            result: caller_result,
                            scalar_type,
                            callee,
                            arguments: vec![argument],
                        },
                        TerminalAbstractOperation::Return {
                            psi_edge: id(311, EdgeId::new),
                            result: caller_result,
                            value: caller_result,
                            scalar_type,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                },
                TerminalAbstractFunction {
                    machine: callee,
                    attachment: None,
                    entry: callee_block,
                    parameters: vec![TerminalAbstractParameter {
                        value: parameter,
                        scalar_type,
                    }],
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: callee_result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block: callee_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![TerminalAbstractOperation::Return {
                        psi_edge: id(312, EdgeId::new),
                        result: callee_result,
                        value: parameter,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    }],
                },
            ],
        };
        reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap()
    }

    fn scalar_boundary_call_unit() -> PsiOptimizationUnit {
        let machine = id(321, MachineId::new);
        let boundary = id(322, BoundaryMachineId::new);
        let block = id(323, BlockId::new);
        let argument = id(324, ValueId::new);
        let result = id(325, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([14; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
                id: boundary,
                identity: "validation::scalar-boundary".into(),
                attachment: None,
                scalar_parameters: vec![scalar_type],
                structural_parameters: Vec::new(),
                result: Some(scalar_type),
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            }],
            provider_candidates: Vec::new(),
            functions: vec![TerminalAbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![TerminalAbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    TerminalAbstractOperation::IntegerConstant {
                        psi_operation: id(326, OperationId::new),
                        result: argument,
                        scalar_type,
                        value: IntegerValue::Unsigned(7),
                    },
                    TerminalAbstractOperation::BoundaryCall {
                        psi_operation: id(327, OperationId::new),
                        result: Some(TerminalAbstractResult {
                            value: result,
                            scalar_type,
                        }),
                        boundary,
                        arguments: vec![argument],
                        structural_arguments: Vec::new(),
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: id(328, EdgeId::new),
                        result,
                        value: result,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        };
        reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap()
    }

    fn redundant_parameter_region_fixture() -> (
        PsiOptimizationUnit,
        PsiOptimizationUnit,
        RedundantBlockParameterRewrite,
        Vec<BlockId>,
    ) {
        use omega_terminal_abstract_operations::{TerminalAbstractSuccessor, TerminalValueBinding};

        let machine = id(701, MachineId::new);
        let entry = id(702, BlockId::new);
        let merge = id(703, BlockId::new);
        let condition = id(704, ValueId::new);
        let shared = id(705, ValueId::new);
        let alternate = id(706, ValueId::new);
        let parameter = id(707, ValueId::new);
        let result = id(708, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let binding = || TerminalValueBinding {
            parameter,
            argument: shared,
            scalar_type,
        };
        let input = reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([22; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![
                        TerminalAbstractParameter {
                            value: condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        TerminalAbstractParameter {
                            value: shared,
                            scalar_type,
                        },
                        TerminalAbstractParameter {
                            value: alternate,
                            scalar_type,
                        },
                    ],
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        TerminalAbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        TerminalAbstractBlockEntry {
                            block: merge,
                            parameters: vec![TerminalAbstractParameter {
                                value: parameter,
                                scalar_type,
                            }],
                            operation_offset: 1,
                        },
                    ],
                    operations: vec![
                        TerminalAbstractOperation::Conditional {
                            condition,
                            when_true: TerminalAbstractSuccessor {
                                psi_edge: id(709, EdgeId::new),
                                target: merge,
                                bindings: vec![binding()],
                            },
                            when_false: TerminalAbstractSuccessor {
                                psi_edge: id(710, EdgeId::new),
                                target: merge,
                                bindings: vec![binding()],
                            },
                        },
                        TerminalAbstractOperation::ExactIntegerAdd {
                            psi_operation: id(711, OperationId::new),
                            obligation: id(713, psi_core::ObligationId::new),
                            result,
                            scalar_type: integer,
                            left: parameter,
                            right: alternate,
                        },
                        TerminalAbstractOperation::Return {
                            psi_edge: id(712, EdgeId::new),
                            result,
                            value: result,
                            scalar_type,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let patch = RedundantBlockParameterRewrite {
            machine,
            block: merge,
            position: 0,
            parameter,
            replacement: shared,
            scalar_type,
        };
        let affected = vec![entry, merge];
        let output = normalize_redundant_parameter_observation_input(&input, patch, &affected)
            .expect("exact structural normalization");
        (input, output, patch, affected)
    }

    fn integer_candidate(
        unit: &PsiOptimizationUnit,
        constant: IntegerValue,
    ) -> PsiRewriteCandidate {
        integer_candidate_with_facts(unit, constant, None, None)
    }

    fn integer_candidate_with_facts(
        unit: &PsiOptimizationUnit,
        constant: IntegerValue,
        supplied_left_fact: Option<omega_optimization_core::ScalarConstantFactIdentity>,
        supplied_obligation_fact: Option<omega_optimization_core::AcceptedObligationFactIdentity>,
    ) -> PsiRewriteCandidate {
        integer_candidate_with_facts_and_cost(
            unit,
            constant,
            supplied_left_fact,
            supplied_obligation_fact,
            -1,
        )
    }

    fn integer_candidate_with_facts_and_cost(
        unit: &PsiOptimizationUnit,
        constant: IntegerValue,
        supplied_left_fact: Option<omega_optimization_core::ScalarConstantFactIdentity>,
        supplied_obligation_fact: Option<omega_optimization_core::AcceptedObligationFactIdentity>,
        predicted_cost_delta: i64,
    ) -> PsiRewriteCandidate {
        let function = &unit.functions[0];
        let block = &function.blocks[0];
        let node = &block.nodes[2];
        let TerminalAbstractOperation::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } = node.operation
        else {
            panic!("fixture contains exact add")
        };
        let location = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: 2,
        };
        let contract = OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(b"fold-exact-add"),
            OptimizationPassIdentity::from_canonical_bytes(b"constant-evaluation"),
            1,
            AnalysisSet::new([AnalysisKind::ScalarConstants]),
            AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
            OptimizationSafetyClass::ProofCertified,
        )
        .unwrap();
        PsiRewriteCandidate::new_integer_evaluation(
            unit.identity,
            contract,
            vec![block.id],
            Vec::new(),
            vec![ProvenanceRewrite {
                input: PsiRealizationSite::Node(location),
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(location)),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            }],
            IntegerEvaluationWitness::ProofCertifiedBinary {
                left_fact: supplied_left_fact.unwrap_or_else(|| {
                    literal_scalar_constant_fact_identity(
                        unit.identity,
                        function.machine,
                        scalar_value_definition(function, left).unwrap(),
                        ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
                        id(206, OperationId::new),
                    )
                    .unwrap()
                }),
                right_fact: literal_scalar_constant_fact_identity(
                    unit.identity,
                    function.machine,
                    scalar_value_definition(function, right).unwrap(),
                    ScalarConstantValue::Integer(IntegerValue::Unsigned(8)),
                    id(207, OperationId::new),
                )
                .unwrap(),
                obligation_fact: supplied_obligation_fact
                    .unwrap_or(unit.accepted_obligation_facts[0].identity),
            },
            predicted_cost_delta,
            IntegerConstantRewrite {
                location,
                source_operation: psi_operation,
                result,
                scalar_type,
                constant,
            },
        )
        .unwrap()
    }

    #[test]
    fn independently_accepts_builder_output() {
        validate_psi_optimization_unit(&unit()).unwrap();
        validate_psi_optimization_unit(&scalar_call_unit()).unwrap();
        validate_psi_optimization_unit(&scalar_boundary_call_unit()).unwrap();
    }

    #[test]
    fn rejects_self_consistent_scalar_operation_contract_corruption() {
        let mut arithmetic = exact_add_unit();
        let (psi_operation, result) = match &arithmetic.functions[0].blocks[0].nodes[1].operation {
            TerminalAbstractOperation::IntegerConstant {
                psi_operation,
                result,
                ..
            } => (*psi_operation, *result),
            _ => panic!("fixture right operand is an integer constant"),
        };
        arithmetic.functions[0].blocks[0].nodes[1].operation =
            TerminalAbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value: true,
            };
        refresh_node_derivatives(&mut arithmetic, 0, 0, 1);
        assert_eq!(
            validate_psi_optimization_unit(&arithmetic),
            Err(
                OptimizationUnitValidationError::ScalarOperationContractMismatch {
                    machine: id(201, MachineId::new),
                    block: id(202, BlockId::new),
                    node: 2,
                }
            )
        );

        let mut out_of_range = unit();
        let TerminalAbstractOperation::IntegerConstant { value, .. } =
            &mut out_of_range.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with an integer constant")
        };
        *value = IntegerValue::Unsigned(256);
        refresh_node_derivatives(&mut out_of_range, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&out_of_range),
            Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 0, .. })
        ));
    }

    #[test]
    fn rejects_self_consistent_control_and_return_type_corruption() {
        let mut conditional = redundant_parameter_region_fixture().0;
        conditional.functions[0].parameters[0].scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("valid integer"));
        refresh_identity(&mut conditional);
        assert!(matches!(
            validate_psi_optimization_unit(&conditional),
            Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 0, .. })
        ));

        let mut scalar_return = unit();
        let (psi_operation, result) = match &scalar_return.functions[0].blocks[0].nodes[0].operation
        {
            TerminalAbstractOperation::IntegerConstant {
                psi_operation,
                result,
                ..
            } => (*psi_operation, *result),
            _ => panic!("fixture begins with an integer constant"),
        };
        scalar_return.functions[0].blocks[0].nodes[0].operation =
            TerminalAbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value: true,
            };
        refresh_node_derivatives(&mut scalar_return, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&scalar_return),
            Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
        ));
    }

    #[test]
    fn rejects_self_consistent_call_signature_corruption() {
        let mut call = scalar_call_unit();
        let (psi_operation, result) = match &call.functions[0].blocks[0].nodes[0].operation {
            TerminalAbstractOperation::IntegerConstant {
                psi_operation,
                result,
                ..
            } => (*psi_operation, *result),
            _ => panic!("caller begins with an integer constant"),
        };
        call.functions[0].blocks[0].nodes[0].operation =
            TerminalAbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value: true,
            };
        refresh_node_derivatives(&mut call, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&call),
            Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
        ));

        let mut boundary = scalar_boundary_call_unit();
        let (psi_operation, result) = match &boundary.functions[0].blocks[0].nodes[0].operation {
            TerminalAbstractOperation::IntegerConstant {
                psi_operation,
                result,
                ..
            } => (*psi_operation, *result),
            _ => panic!("boundary caller begins with an integer constant"),
        };
        boundary.functions[0].blocks[0].nodes[0].operation =
            TerminalAbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value: true,
            };
        refresh_node_derivatives(&mut boundary, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&boundary),
            Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
        ));

        let mut duplicate_boundary = scalar_boundary_call_unit();
        duplicate_boundary
            .boundary_machines
            .push(duplicate_boundary.boundary_machines[0].clone());
        refresh_identity(&mut duplicate_boundary);
        assert!(matches!(
            validate_psi_optimization_unit(&duplicate_boundary),
            Err(OptimizationUnitValidationError::DuplicateBoundaryMachine(_))
        ));
    }

    #[test]
    fn stale_stored_content_identity_is_rejected_before_structural_validation() {
        let mut stale = unit();
        stale.functions[0].blocks[0].nodes[0].effect.output += 1;
        let recomputed = recompute_psi_optimization_unit_identity(&stale);
        assert!(matches!(
            validate_psi_optimization_unit(&stale),
            Err(OptimizationUnitValidationError::ContentIdentityMismatch {
                stored,
                recomputed: actual,
            }) if stored == stale.identity && actual == recomputed
        ));
    }

    #[test]
    fn recomputed_immutable_signature_forgery_is_rejected_by_verified_context() {
        let verified = verified_unit();
        let structural_type = id(120, psi_core::StructuralTypeId::new);
        let boundary = id(121, psi_core::BoundaryMachineId::new);
        let service = id(122, psi_core::ServiceId::new);
        let mut forged = Vec::new();

        let mut unit = verified.unit().clone();
        unit.structural_types
            .push(psi_terminal::StructuralTypeDeclaration {
                id: structural_type,
                identity: "forged-structural-type".into(),
                shape: psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            });
        forged.push(unit);

        let mut unit = verified.unit().clone();
        unit.boundary_machines
            .push(psi_terminal::BoundaryMachineDeclaration {
                id: boundary,
                identity: "forged-boundary".into(),
                attachment: None,
                scalar_parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: None,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            });
        forged.push(unit);

        let mut unit = verified.unit().clone();
        unit.provider_candidates
            .push(psi_terminal::ProviderCandidateConformance {
                boundary,
                requirement_identity: "forged-requirement".into(),
                provider_identity: "forged-provider".into(),
                candidate_identity: "forged-candidate".into(),
                candidate: unit.functions[0].machine,
                signature: psi_terminal::ProviderUnitSignature {
                    parameters: Vec::new(),
                },
                refinement: psi_terminal::ProviderUnitRefinement {
                    positional_parameters: Vec::new(),
                    required_domains: Vec::new(),
                    realized_service_ceiling: Vec::new(),
                },
            });
        forged.push(unit);

        let mut unit = verified.unit().clone();
        unit.functions[0].attachment = Some(structural_type);
        forged.push(unit);

        let mut unit = verified.unit().clone();
        let result_value = id(126, ValueId::new);
        unit.functions[0].result = TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
            value: result_value,
            scalar_type: ScalarType::Boolean,
        });
        unit.functions[0].parameters.push(ValueDefinition {
            value: result_value,
            scalar_type: ScalarType::Boolean,
            site: ValueDefinitionSite::FunctionParameter(0),
        });
        let block = unit.functions[0].blocks[0].id;
        let node = &mut unit.functions[0].blocks[0].nodes[0];
        let psi_edge = match &node.operation {
            TerminalAbstractOperation::ReturnUnit { psi_edge, .. } => *psi_edge,
            _ => panic!("verified fixture must return Unit"),
        };
        node.operation = TerminalAbstractOperation::Return {
            psi_edge,
            result: result_value,
            value: result_value,
            scalar_type: ScalarType::Boolean,
            cleanup_actions: Vec::new(),
        };
        node.uses = vec![ValueUse {
            value: result_value,
            block,
            node: 0,
        }];
        forged.push(unit);

        let mut unit = verified.unit().clone();
        unit.functions[0].published_service_ceiling.push(service);
        forged.push(unit);

        let mut unit = verified.unit().clone();
        let claim = id(123, ClaimId::new);
        let place = id(124, PlaceId::new);
        unit.functions[0]
            .entry_claim_declarations
            .push(psi_terminal::EntryClaim {
                claim,
                input: place,
                path: Vec::new(),
            });
        unit.functions[0].entry_claims.insert(claim);
        unit.functions[0].declared_places.insert(place);
        forged.push(unit);

        for (index, mut unit) in forged.into_iter().enumerate() {
            refresh_identity(&mut unit);
            let result = validate_transformed_psi_optimization_unit(verified.input(), &unit);
            assert!(
                matches!(
                    result,
                    Err(
                        OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch
                    )
                ),
                "forgery class {index} returned {result:?}"
            );
        }
    }

    #[test]
    fn ownership_frontier_catalog_rejects_reordering_duplication_and_context_forgery() {
        let verified = verified_unit();
        let original = verified.unit();
        assert!(original.ownership_frontier_facts.len() >= 2);

        let mut reordered = original.clone();
        reordered.ownership_frontier_facts.swap(0, 1);
        refresh_identity(&mut reordered);
        assert_eq!(
            validate_psi_optimization_unit(&reordered),
            Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
        );

        let mut duplicated = original.clone();
        duplicated
            .ownership_frontier_facts
            .insert(1, duplicated.ownership_frontier_facts[0].clone());
        refresh_identity(&mut duplicated);
        assert_eq!(
            validate_psi_optimization_unit(&duplicated),
            Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
        );

        let mut missing = original.clone();
        missing.ownership_frontier_facts.pop();
        refresh_identity(&mut missing);
        assert_eq!(
            validate_transformed_psi_optimization_unit(verified.input(), &missing),
            Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
        );

        let mut forged = original.clone();
        let prior = forged.ownership_frontier_facts[0].clone();
        let mut snapshot = prior.snapshot;
        snapshot.owned_places.push(OwnershipFrontierOwnedPlace {
            place: id(130, PlaceId::new),
            multiplicity: psi_terminal::StructuralMultiplicity::Affine,
        });
        snapshot.owned_places.sort_by_key(|place| place.place);
        forged.ownership_frontier_facts[0] =
            OwnershipFrontierFact::new(prior.terminal_psi, prior.machine, prior.site, snapshot);
        refresh_identity(&mut forged);
        assert_eq!(
            validate_transformed_psi_optimization_unit(verified.input(), &forged),
            Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
        );
    }

    #[test]
    fn bare_unit_result_signature_must_match_normal_exits() {
        let mut forged = verified_unit().unit().clone();
        forged.functions[0].result =
            TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                value: id(125, ValueId::new),
                scalar_type: ScalarType::Boolean,
            });
        refresh_identity(&mut forged);
        assert!(matches!(
            validate_psi_optimization_unit(&forged),
            Err(OptimizationUnitValidationError::FunctionResultMismatch(_))
        ));
    }

    #[test]
    fn independently_accepts_verified_context_and_frontier_coverage() {
        validate_verified_psi_optimization_unit(&verified_unit()).unwrap();
    }

    #[test]
    fn redundant_parameter_region_observation_is_canonical_and_axis_complete() {
        let (input, output, patch, affected) = redundant_parameter_region_fixture();
        let normalized = normalize_redundant_parameter_observation_input(&input, patch, &affected)
            .expect("independent input normalization");
        let expected = reconstruct_psi_closed_region_observation(
            &normalized,
            patch.machine,
            &[affected[1], affected[0], affected[1]],
        )
        .expect("canonical normalized region");
        let baseline = reconstruct_psi_closed_region_observation(&output, patch.machine, &affected)
            .expect("canonical output region");
        assert_eq!(expected.semantics, baseline.semantics);
        assert_ne!(input.identity, output.identity);
        assert_eq!(baseline.semantics.blocks.len(), 2);
        assert!(baseline.semantics.incoming_edges.is_empty());
        assert!(baseline.semantics.outgoing_edges.is_empty());
        assert_eq!(baseline.semantics.scalar_live_ins.len(), 3);
        assert!(baseline.semantics.scalar_live_outs.is_empty());
        let merge_only =
            reconstruct_psi_closed_region_observation(&output, patch.machine, &[patch.block])
                .expect("single-block graph cut");
        assert_eq!(merge_only.semantics.incoming_edges.len(), 2);
        assert!(merge_only.semantics.outgoing_edges.is_empty());
        assert_eq!(merge_only.semantics.scalar_live_ins.len(), 2);
        assert!(unchanged_outside_redundant_parameter_region(
            &input,
            &output,
            patch.machine,
            &affected,
        ));
        let mut outside_region = output.clone();
        outside_region.fuel_schedule = FuelScheduleIdentity::new(2).unwrap();
        assert!(!unchanged_outside_redundant_parameter_region(
            &input,
            &outside_region,
            patch.machine,
            &affected,
        ));

        let mut corruptions = Vec::new();

        let mut arithmetic_policy = output.clone();
        let node = &mut arithmetic_policy.functions[0].blocks[1].nodes[0];
        let TerminalAbstractOperation::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } = node.operation.clone()
        else {
            unreachable!()
        };
        node.operation = TerminalAbstractOperation::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        };
        corruptions.push(("arithmetic policy", arithmetic_policy));

        let mut edge = output.clone();
        let TerminalAbstractOperation::Conditional { when_true, .. } =
            &mut edge.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        when_true.psi_edge = id(799, EdgeId::new);
        corruptions.push(("control edge", edge));

        let mut successor = output.clone();
        successor.functions[0].blocks[0].nodes[0].successors[0].psi_edge = id(796, EdgeId::new);
        corruptions.push(("successor row", successor));

        let mut normal_exit = output.clone();
        let TerminalAbstractOperation::Return { psi_edge, .. } =
            &mut normal_exit.functions[0].blocks[1].nodes[1].operation
        else {
            unreachable!()
        };
        *psi_edge = id(798, EdgeId::new);
        corruptions.push(("normal exit", normal_exit));

        let mut effect = output.clone();
        effect.functions[0].blocks[1].nodes[0].effect.output += 1;
        corruptions.push(("effect", effect));

        let mut ownership = output.clone();
        ownership.functions[0].blocks[1].nodes[0]
            .ownership
            .push(OwnershipEvent::ClaimCompletion(Vec::new()));
        corruptions.push(("ownership/cleanup", ownership));

        let mut provenance = output.clone();
        provenance.functions[0].blocks[1].nodes[0]
            .provenance
            .push(PsiProvenance::Edge(id(797, EdgeId::new)));
        corruptions.push(("provenance", provenance));

        let mut fuel = output.clone();
        fuel.functions[0].blocks[1].nodes[0].fuel[0].units += 1;
        corruptions.push(("fuel", fuel));

        let mut call_and_suspension = output.clone();
        call_and_suspension.functions[0].blocks[1].nodes[0].operation =
            TerminalAbstractOperation::Call {
                psi_operation: id(711, OperationId::new),
                result: id(708, ValueId::new),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                ),
                callee: patch.machine,
                arguments: vec![patch.replacement],
            };
        corruptions.push(("call/crash/suspension", call_and_suspension));

        let mut live_boundary = output.clone();
        live_boundary.functions[0].blocks[1].nodes[0].uses[0].value = id(704, ValueId::new);
        corruptions.push(("typed scalar boundary", live_boundary));

        let mut frontier = output.clone();
        frontier
            .ownership_frontier_facts
            .push(OwnershipFrontierFact::new(
                frontier.terminal_psi,
                patch.machine,
                OwnershipFrontierSite::BlockEntry(affected[0]),
                OwnershipFrontierSnapshot {
                    claims: Vec::new(),
                    owned_places: Vec::new(),
                    partial_custody: Vec::new(),
                },
            ));
        corruptions.push(("verifier frontier", frontier));

        for (axis, corrupted) in corruptions {
            let observed =
                reconstruct_psi_closed_region_observation(&corrupted, patch.machine, &affected)
                    .expect("corrupted region remains observable");
            assert_ne!(baseline.semantics, observed.semantics, "{axis}");
        }
    }

    #[test]
    fn independent_integer_rewrite_constructor_accepts_only_declared_evaluation() {
        let input = exact_add_unit();
        let candidate = integer_candidate(&input, IntegerValue::Unsigned(15));
        let replay = integer_candidate(&input, IntegerValue::Unsigned(15));
        assert_eq!(candidate.identity(), replay.identity());
        let input_boundary = reconstruct_closed_scalar_node_boundary(
            &input,
            NodeLocation {
                machine: id(201, MachineId::new),
                block: id(202, BlockId::new),
                node: 2,
            },
        )
        .unwrap();
        let accepted = validate_integer_evaluation_candidate(&input, &candidate).unwrap();
        let output_boundary =
            reconstruct_closed_scalar_node_boundary(accepted.unit(), input_boundary.location)
                .unwrap();
        assert_eq!(input_boundary.live_in.len(), 2);
        assert!(output_boundary.live_in.is_empty());
        assert_eq!(input_boundary.live_out, output_boundary.live_out);
        assert_eq!(accepted.candidate(), candidate.identity());
        assert_ne!(accepted.unit().identity, input.identity);
        assert_eq!(
            accepted.unit().identity,
            recompute_psi_optimization_unit_identity(accepted.unit())
        );
        assert_eq!(
            accepted.unit().functions[0].blocks[0].nodes[2].provenance,
            input.functions[0].blocks[0].nodes[2].provenance
        );
        assert_eq!(
            accepted.unit().functions[0].blocks[0].nodes[2].fuel,
            input.functions[0].blocks[0].nodes[2].fuel
        );
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(15),
                ..
            }
        ));
        assert!(matches!(
            accepted.unit().functions[0].facts[2],
            OptimizationFact::IntegerConstant {
                constant: IntegerValue::Unsigned(15),
                ..
            }
        ));
        assert!(matches!(
            input.functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::ExactIntegerAdd { .. }
        ));

        let wrong = integer_candidate(&input, IntegerValue::Unsigned(14));
        assert!(matches!(
            validate_integer_evaluation_candidate(&input, &wrong),
            Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
        ));

        let foreign_fact = integer_candidate_with_facts(
            &input,
            IntegerValue::Unsigned(15),
            Some(
                omega_optimization_core::ScalarConstantFactIdentity::from_canonical_bytes(
                    b"fact from another revision",
                ),
            ),
            None,
        );
        assert!(matches!(
            validate_integer_evaluation_candidate(&input, &foreign_fact),
            Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
        ));

        let foreign_obligation = integer_candidate_with_facts(
            &input,
            IntegerValue::Unsigned(15),
            None,
            Some(
                omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                    b"fact admitted for another operation",
                ),
            ),
        );
        assert!(matches!(
            validate_integer_evaluation_candidate(&input, &foreign_obligation),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        ));
    }

    #[test]
    fn candidate_history_does_not_declare_the_accepted_content_identity() {
        let input = exact_add_unit();
        let first = integer_candidate_with_facts_and_cost(
            &input,
            IntegerValue::Unsigned(15),
            None,
            None,
            -1,
        );
        let second = integer_candidate_with_facts_and_cost(
            &input,
            IntegerValue::Unsigned(15),
            None,
            None,
            -2,
        );
        assert_ne!(first.identity(), second.identity());

        let first_output = validate_integer_evaluation_candidate(&input, &first).unwrap();
        let second_output = validate_integer_evaluation_candidate(&input, &second).unwrap();
        assert_eq!(first_output.unit(), second_output.unit());
        assert_eq!(
            first_output.unit().identity,
            recompute_psi_optimization_unit_identity(first_output.unit())
        );
    }

    #[test]
    fn corruption_classes_fail_independently() {
        let mut accepted_fact = exact_add_unit();
        accepted_fact.accepted_obligation_facts[0].proof_bundle_fingerprint[0] ^= 1;
        refresh_identity(&mut accepted_fact);
        assert!(matches!(
            validate_psi_optimization_unit(&accepted_fact),
            Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)
        ));

        let mut provenance = unit();
        provenance.functions[0].blocks[0].nodes[0]
            .provenance
            .clear();
        refresh_identity(&mut provenance);
        assert!(matches!(
            validate_psi_optimization_unit(&provenance),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut fuel = unit();
        fuel.functions[0].blocks[0].nodes[0].fuel.clear();
        refresh_identity(&mut fuel);
        assert!(matches!(
            validate_psi_optimization_unit(&fuel),
            Err(OptimizationUnitValidationError::FuelDoesNotMatchProvenance { .. })
        ));

        let mut effects = unit();
        effects.functions[0].blocks[0].nodes[1].effect.input = 99;
        refresh_identity(&mut effects);
        assert!(matches!(
            validate_psi_optimization_unit(&effects),
            Err(OptimizationUnitValidationError::BrokenEffectChain { .. })
        ));

        let mut facts = unit();
        facts.functions[0].facts.clear();
        refresh_identity(&mut facts);
        assert!(matches!(
            validate_psi_optimization_unit(&facts),
            Err(OptimizationUnitValidationError::FactIndexMismatch(_))
        ));

        let mut forged_uses = unit();
        let block = forged_uses.functions[0].blocks[0].id;
        forged_uses.functions[0].blocks[0].nodes[1]
            .uses
            .push(ValueUse {
                value: id(99, ValueId::new),
                block,
                node: 1,
            });
        refresh_identity(&mut forged_uses);
        assert!(matches!(
            validate_psi_optimization_unit(&forged_uses),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut forged_definitions = unit();
        forged_definitions.functions[0].blocks[0].nodes[0]
            .definitions
            .clear();
        refresh_identity(&mut forged_definitions);
        assert!(matches!(
            validate_psi_optimization_unit(&forged_definitions),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut undefined = unit();
        let unknown = id(99, ValueId::new);
        let TerminalAbstractOperation::Return { value, .. } =
            &mut undefined.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("unit ends in return")
        };
        *value = unknown;
        undefined.functions[0].blocks[0].nodes[1].uses = vec![ValueUse {
            value: unknown,
            block,
            node: 1,
        }];
        refresh_identity(&mut undefined);
        assert!(matches!(
            validate_psi_optimization_unit(&undefined),
            Err(OptimizationUnitValidationError::UndefinedValue { .. })
        ));

        let mut place = unit();
        place.functions[0]
            .declared_places
            .insert(id(88, PlaceId::new));
        refresh_identity(&mut place);
        assert!(matches!(
            validate_psi_optimization_unit(&place),
            Err(OptimizationUnitValidationError::UnknownPlace { .. })
        ));

        let mut cleanup = unit();
        cleanup.functions[0].blocks[0].nodes[1].ownership.clear();
        refresh_identity(&mut cleanup);
        assert!(matches!(
            validate_psi_optimization_unit(&cleanup),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut cfg = unit();
        cfg.functions[0].blocks[0].nodes[1].operation = TerminalAbstractOperation::Jump {
            psi_edge: id(5, EdgeId::new),
            target: id(77, BlockId::new),
            bindings: Vec::new(),
        };
        cfg.functions[0].blocks[0].nodes[1].successors =
            expected_edges(&cfg.functions[0].blocks[0].nodes[1].operation);
        cfg.functions[0].blocks[0].nodes[1].uses.clear();
        cfg.functions[0].blocks[0].nodes[1].ownership.clear();
        cfg.functions[0].blocks[0].nodes[1].provenance.clear();
        cfg.functions[0].blocks[0].nodes[1].fuel.clear();
        refresh_identity(&mut cfg);
        assert!(matches!(
            validate_psi_optimization_unit(&cfg),
            Err(OptimizationUnitValidationError::UnknownSuccessor { .. })
        ));

        let mut entry_parameters = unit();
        let block = entry_parameters.functions[0].entry;
        entry_parameters.functions[0].blocks[0]
            .parameters
            .push(ValueDefinition {
                value: id(76, ValueId::new),
                scalar_type: ScalarType::Boolean,
                site: ValueDefinitionSite::BlockParameter { block, position: 0 },
            });
        refresh_identity(&mut entry_parameters);
        assert!(matches!(
            validate_psi_optimization_unit(&entry_parameters),
            Err(OptimizationUnitValidationError::EntryBlockHasParameters { .. })
        ));

        let mut unreachable = unit();
        let block = id(75, BlockId::new);
        let mut detached = unreachable.functions[0].blocks[0].clone();
        detached.id = block;
        for (node_index, node) in detached.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index).unwrap();
            node.definitions = expected_definitions(&node.operation, block, node_index);
            node.uses = expected_uses(&node.operation, block, node_index);
        }
        unreachable.functions[0].blocks.push(detached);
        refresh_identity(&mut unreachable);
        assert!(matches!(
            validate_psi_optimization_unit(&unreachable),
            Err(OptimizationUnitValidationError::UnreachableBlock { .. })
        ));

        let mut cycle = unit();
        let block = cycle.functions[0].entry;
        let operation = TerminalAbstractOperation::Jump {
            psi_edge: id(5, EdgeId::new),
            target: block,
            bindings: Vec::new(),
        };
        let node = &mut cycle.functions[0].blocks[0].nodes[1];
        node.operation = operation;
        node.provenance = expected_provenance(&node.operation);
        node.uses = expected_uses(&node.operation, block, 1);
        node.successors = expected_edges(&node.operation);
        node.ownership = expected_ownership(&node.operation);
        refresh_identity(&mut cycle);
        assert!(matches!(
            validate_psi_optimization_unit(&cycle),
            Err(OptimizationUnitValidationError::ControlCycle { .. })
        ));
    }

    #[test]
    fn unknown_claim_frontier_is_rejected() {
        let mut unit = unit();
        let claim = id(71, ClaimId::new);
        let edge = id(5, EdgeId::new);
        let operation = TerminalAbstractOperation::Crash {
            psi_edge: edge,
            cause: psi_terminal::CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: vec![claim],
        };
        let node = &mut unit.functions[0].blocks[0].nodes[1];
        node.operation = operation;
        node.provenance = expected_provenance(&node.operation);
        node.fuel[0].site = PsiProvenance::Edge(edge);
        node.uses.clear();
        node.successors.clear();
        node.ownership = expected_ownership(&node.operation);
        refresh_identity(&mut unit);
        assert!(matches!(
            validate_psi_optimization_unit(&unit),
            Err(OptimizationUnitValidationError::UnknownClaim { .. })
        ));
    }
}
