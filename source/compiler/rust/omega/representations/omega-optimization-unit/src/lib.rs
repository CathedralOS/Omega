#![forbid(unsafe_code)]

//! Reconstructible, target-neutral optimization input derived from verified
//! Terminal Psi realization requirements.
//!
//! This crate deliberately performs no optimization. It makes the implicit
//! structure in [`TerminalAbstractOperationPlan`] explicit so independent
//! validators and later passes do not have to rediscover CFG, SSA, semantic
//! fuel, effects, or provenance from a mutable instruction stream.

use std::collections::BTreeSet;

use omega_optimization_core::{AcceptedObligationFactIdentity, OptimizationUnitIdentity};
use omega_terminal_abstract_operations::{
    TerminalAbstractFunction, TerminalAbstractFunctionResult, TerminalAbstractOperation,
    TerminalAbstractOperationPlan, TerminalAbstractSuccessor, TerminalValueBinding,
};
use psi_core::{
    BlockId, ClaimId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, ScalarType, ServiceId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    BoundaryMachineDeclaration, EntryClaim, ProviderCandidateConformance,
    StructuralParameterDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
    TerminalPsiIdentity,
};

mod identity;
mod ledger;
mod observation;
mod rewrite;

pub use identity::recompute_psi_optimization_unit_identity;

pub use ledger::{
    InvalidPsiTransformationLedger, PsiTransformationLedger, PsiTransformationRecord,
};
pub use observation::{
    ObservationEventClass, ObservationKnowledge, PsiNodeObservation, PsiObservableEvent,
    PsiObservationModel, reconstruct_psi_observation_model,
};

pub use rewrite::{
    BlockParameterIncomingBinding, BooleanConstantRewrite, IntegerConstantRewrite,
    IntegerEvaluationWitness, NodeLocation, ProvenanceRewrite, PsiRewriteCandidate,
    PsiRewriteCandidateError, PsiRewritePatch, RedundantBlockParameterRewrite,
    RedundantBlockParameterWitness, ScalarConstantValue, ScalarEvaluationWitness,
    ScalarSubstitution, SccpBlockRow, SccpEdgeRow, SccpEdgeState, SccpMachineSnapshot,
    SccpValueRow, SccpValueState, derived_sccp_scalar_constant_fact_identity,
    literal_scalar_constant_fact_identity,
};

/// The exact immutable Terminal Psi semantic site realized by one unit node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsiProvenance {
    Operation(OperationId),
    Edge(EdgeId),
}

/// One source logical-fuel settlement. Native lowering must retain this even
/// when several source nodes become one physical instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FuelSettlement {
    pub site: PsiProvenance,
    pub units: u64,
}

/// A conservative semantic sequencing token. Initially every node is chained;
/// analyses may later prove selected scalar nodes independent without erasing
/// the source order represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectLink {
    pub input: u64,
    pub output: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueDefinitionSite {
    FunctionParameter(u32),
    BlockParameter { block: BlockId, position: u32 },
    Node { block: BlockId, node: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueDefinition {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub site: ValueDefinitionSite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueUse {
    pub value: ValueId,
    pub block: BlockId,
    pub node: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationEdge {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<TerminalValueBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipEvent {
    ClaimTransfer(Vec<ClaimId>),
    ClaimCompletion(Vec<ClaimId>),
    Cleanup(Vec<TerminalAffineCleanupAction>),
    StructuralReturn(Vec<ClaimId>),
    CrashFrontier(Vec<ClaimId>),
}

/// A proof/range fact is always indexed by its exact source support. The first
/// builder only emits facts reconstructed directly from literal operations;
/// proof-derived facts remain absent (and therefore unavailable to rules)
/// until their verified evidence is retained across the lowering boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationFact {
    /// A proof-bearing operation's obligation lookup key. This reference is
    /// not itself accepted evidence: publication must resolve it against the
    /// verifier-owned context for the immutable Terminal Psi artifact.
    OperationObligationReference {
        obligation: ObligationId,
        support: OperationId,
    },
    BooleanConstant {
        value: ValueId,
        constant: bool,
        support: OperationId,
    },
    IntegerConstant {
        value: ValueId,
        constant: IntegerValue,
        support: OperationId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationNode {
    pub operation: TerminalAbstractOperation,
    pub provenance: Vec<PsiProvenance>,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub definitions: Vec<ValueDefinition>,
    pub uses: Vec<ValueUse>,
    pub successors: Vec<OptimizationEdge>,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationBlock {
    pub id: BlockId,
    pub parameters: Vec<ValueDefinition>,
    pub nodes: Vec<OptimizationNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationFunction {
    pub machine: MachineId,
    /// Exact nominal receiver attachment from the verified Terminal-Psi
    /// signature. Optimization may inspect but never rewrite this identity.
    pub attachment: Option<StructuralTypeId>,
    pub entry: BlockId,
    pub parameters: Vec<ValueDefinition>,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    /// Exact normal result signature retained independently of executable
    /// return nodes, including Unit and structural-result distinctions.
    pub result: TerminalAbstractFunctionResult,
    pub declared_places: BTreeSet<PlaceId>,
    /// Full ordered caller/root claim signature. `entry_claims` below is the
    /// independently checked membership index used by ownership validation.
    pub entry_claim_declarations: Vec<EntryClaim>,
    pub entry_claims: BTreeSet<ClaimId>,
    /// Exact verifier-normalized service ceiling in canonical Terminal-Psi
    /// order. It is semantic custody, not an optimizer-selected reach set.
    pub published_service_ceiling: Vec<ServiceId>,
    pub facts: Vec<OptimizationFact>,
    pub blocks: Vec<OptimizationBlock>,
}

/// An admitted proof fact projected from the immutable verifier context.
///
/// The row binds both semantic artifact identities and the exact operation
/// owner. It remains attached after a rewrite removes that operation so the
/// transformation ledger and manifest can retain proof custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedObligationFact {
    pub identity: AcceptedObligationFactIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub proof_bundle_fingerprint: [u8; 32],
    pub machine: MachineId,
    pub operation: OperationId,
    pub obligation: ObligationId,
    pub proposition: Vec<u8>,
}

impl AcceptedObligationFact {
    pub fn new(
        terminal_psi: TerminalPsiIdentity,
        proof_bundle_fingerprint: [u8; 32],
        machine: MachineId,
        operation: OperationId,
        obligation: ObligationId,
        proposition: Vec<u8>,
    ) -> Self {
        let identity = accepted_obligation_fact_identity(
            terminal_psi,
            proof_bundle_fingerprint,
            machine,
            operation,
            obligation,
            &proposition,
        );
        Self {
            identity,
            terminal_psi,
            proof_bundle_fingerprint,
            machine,
            operation,
            obligation,
            proposition,
        }
    }

    pub fn has_canonical_identity(&self) -> bool {
        self.identity
            == accepted_obligation_fact_identity(
                self.terminal_psi,
                self.proof_bundle_fingerprint,
                self.machine,
                self.operation,
                self.obligation,
                &self.proposition,
            )
    }
}

pub fn accepted_obligation_fact_identity(
    terminal_psi: TerminalPsiIdentity,
    proof_bundle_fingerprint: [u8; 32],
    machine: MachineId,
    operation: OperationId,
    obligation: ObligationId,
    proposition: &[u8],
) -> AcceptedObligationFactIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-accepted-obligation-fact.v1\0");
    canonical.extend_from_slice(terminal_psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&terminal_psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(&proof_bundle_fingerprint);
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&operation.get().to_le_bytes());
    canonical.extend_from_slice(&obligation.get().to_le_bytes());
    canonical.extend_from_slice(
        &u64::try_from(proposition.len())
            .expect("canonical proposition length fits u64")
            .to_le_bytes(),
    );
    canonical.extend_from_slice(proposition);
    AcceptedObligationFactIdentity::from_canonical_bytes(&canonical)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationUnit {
    pub identity: OptimizationUnitIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub entry: MachineId,
    /// Target-neutral module declarations needed by layout, ABI, and checked
    /// provider installation after the full Terminal module is discarded.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub accepted_obligation_facts: Vec<AcceptedObligationFact>,
    pub functions: Vec<PsiOptimizationFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptedObligationFactIndexError {
    AlreadyAttached,
    TerminalIdentityMismatch,
    InvalidFactIdentity,
    DuplicateOwner,
}

impl std::fmt::Display for AcceptedObligationFactIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid accepted obligation fact index: {self:?}"
        )
    }
}

impl std::error::Error for AcceptedObligationFactIndexError {}

/// Attach the canonical verifier projection exactly once and bind it into the
/// optimization-unit identity. Bare units intentionally retain an empty index.
pub fn attach_accepted_obligation_facts(
    mut unit: PsiOptimizationUnit,
    mut facts: Vec<AcceptedObligationFact>,
) -> Result<PsiOptimizationUnit, AcceptedObligationFactIndexError> {
    if !unit.accepted_obligation_facts.is_empty() {
        return Err(AcceptedObligationFactIndexError::AlreadyAttached);
    }
    if facts
        .iter()
        .any(|fact| fact.terminal_psi != unit.terminal_psi)
    {
        return Err(AcceptedObligationFactIndexError::TerminalIdentityMismatch);
    }
    if facts.iter().any(|fact| !fact.has_canonical_identity()) {
        return Err(AcceptedObligationFactIndexError::InvalidFactIdentity);
    }
    facts.sort_by_key(|fact| (fact.machine, fact.operation, fact.obligation));
    if facts.windows(2).any(|pair| {
        (pair[0].machine, pair[0].operation, pair[0].obligation)
            == (pair[1].machine, pair[1].operation, pair[1].obligation)
    }) {
        return Err(AcceptedObligationFactIndexError::DuplicateOwner);
    }
    unit.accepted_obligation_facts = facts;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationUnitBuildError {
    MissingBlocks(MachineId),
    FirstBlockDoesNotStartAtZero(MachineId),
    InvalidBlockOffset { machine: MachineId, offset: usize },
    DuplicateBlock(MachineId, BlockId),
    NodeIndexOverflow(MachineId),
    ParameterIndexOverflow(MachineId),
}

impl std::fmt::Display for OptimizationUnitBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot construct canonical Psi optimization unit: {self:?}"
        )
    }
}

impl std::error::Error for OptimizationUnitBuildError {}

/// Low-level deterministic projection from the clean lowering seed.
///
/// This is not an optimizer admission boundary: consumers that may transform
/// the unit must use the verified constructor owned by the Terminal-Psi
/// artifact boundary so the plan cannot detach from its verifier context.
pub fn reconstruct_psi_optimization_unit_seed(
    plan: &TerminalAbstractOperationPlan,
    fuel_schedule: FuelScheduleIdentity,
) -> Result<PsiOptimizationUnit, OptimizationUnitBuildError> {
    let functions = plan
        .functions
        .iter()
        .map(build_function)
        .collect::<Result<Vec<_>, _>>()?;
    let mut unit = PsiOptimizationUnit {
        identity: OptimizationUnitIdentity::from_canonical_bytes(b"pending canonical content"),
        terminal_psi: plan.terminal_psi,
        fuel_schedule,
        entry: plan.entry,
        structural_types: plan.structural_types.clone(),
        boundary_machines: plan.boundary_machines.clone(),
        provider_candidates: plan.provider_candidates.clone(),
        accepted_obligation_facts: Vec::new(),
        functions,
    };
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}

fn build_function(
    function: &TerminalAbstractFunction,
) -> Result<PsiOptimizationFunction, OptimizationUnitBuildError> {
    if function.block_entries.is_empty() {
        return Err(OptimizationUnitBuildError::MissingBlocks(function.machine));
    }
    if function.block_entries[0].operation_offset != 0 {
        return Err(OptimizationUnitBuildError::FirstBlockDoesNotStartAtZero(
            function.machine,
        ));
    }
    let mut block_ids = BTreeSet::new();
    for entry in &function.block_entries {
        if entry.operation_offset > function.operations.len() {
            return Err(OptimizationUnitBuildError::InvalidBlockOffset {
                machine: function.machine,
                offset: entry.operation_offset,
            });
        }
        if !block_ids.insert(entry.block) {
            return Err(OptimizationUnitBuildError::DuplicateBlock(
                function.machine,
                entry.block,
            ));
        }
    }

    let parameters = function
        .parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            Ok(ValueDefinition {
                value: parameter.value,
                scalar_type: parameter.scalar_type,
                site: ValueDefinitionSite::FunctionParameter(u32::try_from(position).map_err(
                    |_| OptimizationUnitBuildError::ParameterIndexOverflow(function.machine),
                )?),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut facts = Vec::new();
    let mut declared_places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .chain(function.entry_claims.iter().map(|claim| claim.input))
        .collect::<BTreeSet<_>>();
    let mut effect_token = 0u64;
    let mut blocks = Vec::with_capacity(function.block_entries.len());
    for (block_index, entry) in function.block_entries.iter().enumerate() {
        let end = function
            .block_entries
            .get(block_index + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        if end < entry.operation_offset {
            return Err(OptimizationUnitBuildError::InvalidBlockOffset {
                machine: function.machine,
                offset: end,
            });
        }
        let block_parameter_rows = entry
            .parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                Ok(ValueDefinition {
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                    site: ValueDefinitionSite::BlockParameter {
                        block: entry.block,
                        position: u32::try_from(position).map_err(|_| {
                            OptimizationUnitBuildError::ParameterIndexOverflow(function.machine)
                        })?,
                    },
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut nodes = Vec::with_capacity(end - entry.operation_offset);
        for (local_index, operation) in function.operations[entry.operation_offset..end]
            .iter()
            .enumerate()
        {
            let node = u32::try_from(local_index)
                .map_err(|_| OptimizationUnitBuildError::NodeIndexOverflow(function.machine))?;
            let provenance = operation_provenance(operation);
            let fuel = provenance
                .iter()
                .copied()
                .map(|site| FuelSettlement { site, units: 1 })
                .collect();
            let definitions = operation_definition(operation)
                .into_iter()
                .map(|(value, scalar_type)| ValueDefinition {
                    value,
                    scalar_type,
                    site: ValueDefinitionSite::Node {
                        block: entry.block,
                        node,
                    },
                })
                .collect();
            let uses = operation_uses(operation)
                .into_iter()
                .map(|value| ValueUse {
                    value,
                    block: entry.block,
                    node,
                })
                .collect();
            collect_places(operation, &mut declared_places);
            collect_fact(operation, &mut facts);
            let ownership = operation_ownership(operation);
            let successors = operation_edges(operation);
            nodes.push(OptimizationNode {
                operation: operation.clone(),
                provenance,
                fuel,
                effect: EffectLink {
                    input: effect_token,
                    output: effect_token + 1,
                },
                definitions,
                uses,
                successors,
                ownership,
            });
            effect_token += 1;
        }
        blocks.push(OptimizationBlock {
            id: entry.block,
            parameters: block_parameter_rows,
            nodes,
        });
    }

    Ok(PsiOptimizationFunction {
        machine: function.machine,
        attachment: function.attachment,
        entry: function.entry,
        parameters,
        structural_parameters: function.structural_parameters.clone(),
        result: function.result.clone(),
        declared_places,
        entry_claim_declarations: function.entry_claims.clone(),
        entry_claims: function
            .entry_claims
            .iter()
            .map(|claim| claim.claim)
            .collect(),
        published_service_ceiling: function.published_service_ceiling.clone(),
        facts,
        blocks,
    })
}

fn operation_provenance(operation: &TerminalAbstractOperation) -> Vec<PsiProvenance> {
    use TerminalAbstractOperation as O;
    let site = match operation {
        O::Jump { psi_edge, .. }
        | O::Return { psi_edge, .. }
        | O::ReturnUnit { psi_edge, .. }
        | O::ReturnStructural { psi_edge, .. }
        | O::Crash { psi_edge, .. } => PsiProvenance::Edge(*psi_edge),
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            return vec![
                PsiProvenance::Edge(when_true.psi_edge),
                PsiProvenance::Edge(when_false.psi_edge),
            ];
        }
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
            PsiProvenance::Operation(*psi_operation)
        }
    };
    vec![site]
}

fn operation_definition(operation: &TerminalAbstractOperation) -> Option<(ValueId, ScalarType)> {
    use TerminalAbstractOperation as O;
    match operation {
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
    }
}

fn operation_uses(operation: &TerminalAbstractOperation) -> Vec<ValueId> {
    use TerminalAbstractOperation as O;
    match operation {
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
    }
}

fn operation_edges(operation: &TerminalAbstractOperation) -> Vec<OptimizationEdge> {
    use TerminalAbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => vec![successor_edge(when_true), successor_edge(when_false)],
        _ => Vec::new(),
    }
}

fn successor_edge(successor: &TerminalAbstractSuccessor) -> OptimizationEdge {
    OptimizationEdge {
        psi_edge: successor.psi_edge,
        target: successor.target,
        bindings: successor.bindings.clone(),
    }
}

fn collect_places(operation: &TerminalAbstractOperation, places: &mut BTreeSet<PlaceId>) {
    use TerminalAbstractOperation as O;
    match operation {
        O::EstablishByteSequenceLiteral { place, .. }
        | O::EstablishTrivialAffineLocal { place, .. } => {
            places.insert(place.id);
        }
        O::CallStructural { result, .. } => {
            places.insert(result.place);
        }
        O::BooleanStructuralField { source, .. } | O::ReturnStructural { source, .. } => {
            places.insert(*source);
        }
        _ => {}
    }
}

fn collect_fact(operation: &TerminalAbstractOperation, facts: &mut Vec<OptimizationFact>) {
    if let Some((obligation, support)) = operation_obligation(operation) {
        facts.push(OptimizationFact::OperationObligationReference {
            obligation,
            support,
        });
    }
    match operation {
        TerminalAbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        } => facts.push(OptimizationFact::BooleanConstant {
            value: *result,
            constant: *value,
            support: *psi_operation,
        }),
        TerminalAbstractOperation::IntegerConstant {
            psi_operation,
            result,
            value,
            ..
        } => facts.push(OptimizationFact::IntegerConstant {
            value: *result,
            constant: *value,
            support: *psi_operation,
        }),
        _ => {}
    }
}

fn operation_obligation(
    operation: &TerminalAbstractOperation,
) -> Option<(ObligationId, OperationId)> {
    use TerminalAbstractOperation as O;
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
        } => Some((*obligation, *psi_operation)),
        _ => None,
    }
}

fn operation_ownership(operation: &TerminalAbstractOperation) -> Vec<OwnershipEvent> {
    use TerminalAbstractOperation as O;
    match operation {
        O::CallUnit {
            claim_transfers, ..
        }
        | O::CallStructuralScalar {
            claim_transfers, ..
        } => {
            vec![OwnershipEvent::ClaimTransfer(
                claim_transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect(),
            )]
        }
        O::CallStructural {
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
        } => {
            vec![OwnershipEvent::Cleanup(cleanup_actions.clone())]
        }
        O::ReturnStructural {
            returned_claims, ..
        } => {
            vec![OwnershipEvent::StructuralReturn(returned_claims.clone())]
        }
        O::Crash {
            frontier_lower_bound,
            ..
        } => {
            vec![OwnershipEvent::CrashFrontier(frontier_lower_bound.clone())]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_terminal_abstract_operations::{
        TerminalAbstractBlockEntry, TerminalAbstractFunctionResult, TerminalAbstractParameter,
        TerminalAbstractResult, TerminalValueBinding,
    };
    use psi_core::{
        BoundaryMachineId, IntegerSign, IntegerType, IntegerValue, ServiceId, StructuralTypeId,
    };
    use psi_terminal::{
        BoundaryMachineDeclaration, ByteSequenceCarrier, ProviderCandidateConformance,
        ProviderUnitRefinement, ProviderUnitSignature, SemanticFingerprint,
        StructuralTypeDeclaration, StructuralTypeShape, VocabularyMarker,
    };

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    fn plan() -> TerminalAbstractOperationPlan {
        let machine = id(1, MachineId::new);
        let block = id(2, BlockId::new);
        let value = id(3, ValueId::new);
        let result = id(4, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("valid width");
        TerminalAbstractOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![TerminalAbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![TerminalAbstractParameter {
                    value,
                    scalar_type: ScalarType::Integer(integer),
                }],
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(
                    omega_terminal_abstract_operations::TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                    },
                ),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![TerminalAbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    TerminalAbstractOperation::IntegerConstant {
                        psi_operation: id(5, OperationId::new),
                        result,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(9),
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: id(6, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        }
    }

    #[test]
    fn rebuild_is_deterministic_and_keeps_distinct_fuel_sites() {
        let schedule = FuelScheduleIdentity::new(1).expect("nonzero schedule");
        let first = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
        let second = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.functions[0].blocks[0].nodes.len(), 2);
        assert_ne!(
            first.functions[0].blocks[0].nodes[0].fuel[0].site,
            first.functions[0].blocks[0].nodes[1].fuel[0].site
        );
        let source = plan();
        assert_eq!(first.structural_types, source.structural_types);
        assert_eq!(first.boundary_machines, source.boundary_machines);
        assert_eq!(first.provider_candidates, source.provider_candidates);
        assert_eq!(
            first.functions[0].attachment,
            source.functions[0].attachment
        );
        assert_eq!(first.functions[0].result, source.functions[0].result);
        assert_eq!(
            first.functions[0].entry_claim_declarations,
            source.functions[0].entry_claims
        );
        assert_eq!(
            first.functions[0].published_service_ceiling,
            source.functions[0].published_service_ceiling
        );
    }

    #[test]
    fn canonical_identity_is_content_recomputable_and_history_independent() {
        let schedule = FuelScheduleIdentity::new(1).expect("nonzero schedule");
        let first = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
        let second = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
        assert_eq!(
            recompute_psi_optimization_unit_identity(&first),
            recompute_psi_optimization_unit_identity(&second)
        );

        let mut different_stored_history = first.clone();
        different_stored_history.identity =
            OptimizationUnitIdentity::from_canonical_bytes(b"unrelated stored history");
        assert_eq!(
            recompute_psi_optimization_unit_identity(&first),
            recompute_psi_optimization_unit_identity(&different_stored_history)
        );
    }

    #[test]
    fn canonical_identity_binds_every_retained_field_class() {
        let baseline = reconstruct_psi_optimization_unit_seed(
            &plan(),
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .unwrap();
        let baseline_identity = recompute_psi_optimization_unit_identity(&baseline);
        let machine = baseline.functions[0].machine;
        let block = baseline.functions[0].blocks[0].id;
        let scalar_type = baseline.functions[0].parameters[0].scalar_type;
        let mut mutations = Vec::new();

        let mut unit = baseline.clone();
        unit.terminal_psi.program_fingerprint = SemanticFingerprint::from_bytes([8; 32]);
        mutations.push(("terminal identity", unit));
        let mut unit = baseline.clone();
        unit.fuel_schedule = FuelScheduleIdentity::new(2).unwrap();
        mutations.push(("fuel schedule", unit));
        let mut unit = baseline.clone();
        unit.entry = id(90, MachineId::new);
        mutations.push(("entry machine", unit));
        let structural_type = id(105, StructuralTypeId::new);
        let boundary = id(106, BoundaryMachineId::new);
        let mut unit = baseline.clone();
        unit.structural_types.push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "identity-test-structural-type".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        });
        mutations.push(("module structural type", unit));
        let mut unit = baseline.clone();
        unit.boundary_machines.push(BoundaryMachineDeclaration {
            id: boundary,
            identity: "identity-test-boundary".into(),
            attachment: Some(structural_type),
            scalar_parameters: vec![ScalarType::Boolean],
            structural_parameters: Vec::new(),
            result: Some(ScalarType::Boolean),
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: vec![id(107, ServiceId::new)],
        });
        mutations.push(("module boundary declaration", unit));
        let mut unit = baseline.clone();
        unit.provider_candidates.push(ProviderCandidateConformance {
            boundary,
            requirement_identity: "identity-test-requirement".into(),
            provider_identity: "identity-test-provider".into(),
            candidate_identity: "identity-test-candidate".into(),
            candidate: machine,
            signature: ProviderUnitSignature {
                parameters: Vec::new(),
            },
            refinement: ProviderUnitRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: vec![id(108, ServiceId::new)],
            },
        });
        mutations.push(("module provider candidate", unit));
        let mut unit = baseline.clone();
        unit.accepted_obligation_facts
            .push(AcceptedObligationFact::new(
                unit.terminal_psi,
                [4; 32],
                machine,
                id(5, OperationId::new),
                id(91, ObligationId::new),
                vec![1, 2, 3],
            ));
        mutations.push(("accepted fact", unit));
        let mut unit = baseline.clone();
        unit.functions[0].machine = id(92, MachineId::new);
        mutations.push(("function identity", unit));
        let mut unit = baseline.clone();
        unit.functions[0].attachment = Some(structural_type);
        mutations.push(("function attachment", unit));
        let mut unit = baseline.clone();
        unit.functions[0].parameters[0].value = id(93, ValueId::new);
        mutations.push(("scalar parameter", unit));
        let mut unit = baseline.clone();
        unit.functions[0].structural_parameters.push(
            psi_terminal::StructuralParameterDeclaration {
                place: id(94, PlaceId::new),
                position: 0,
                is_self: false,
                structural_type: id(95, psi_core::StructuralTypeId::new),
                multiplicity: psi_terminal::StructuralMultiplicity::Affine,
                access: psi_terminal::StructuralAccess::Owned,
                qualifications: Vec::new(),
            },
        );
        mutations.push(("structural parameter", unit));
        let mut unit = baseline.clone();
        unit.functions[0].result = TerminalAbstractFunctionResult::Unit;
        mutations.push(("function result signature", unit));
        let mut unit = baseline.clone();
        unit.functions[0]
            .declared_places
            .insert(id(96, PlaceId::new));
        mutations.push(("declared place", unit));
        let mut unit = baseline.clone();
        unit.functions[0].entry_claim_declarations.push(EntryClaim {
            claim: id(109, ClaimId::new),
            input: id(110, PlaceId::new),
            path: Vec::new(),
        });
        mutations.push(("entry claim declaration", unit));
        let mut unit = baseline.clone();
        unit.functions[0].entry_claims.insert(id(97, ClaimId::new));
        mutations.push(("entry claim", unit));
        let mut unit = baseline.clone();
        unit.functions[0]
            .published_service_ceiling
            .push(id(111, ServiceId::new));
        mutations.push(("function service ceiling", unit));
        let mut unit = baseline.clone();
        unit.functions[0].facts.clear();
        mutations.push(("optimization fact", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].id = id(98, BlockId::new);
        mutations.push(("block", unit));
        let mut unit = baseline.clone();
        let TerminalAbstractOperation::IntegerConstant { value, .. } =
            &mut unit.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(10);
        mutations.push(("operation payload", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0].provenance[0] =
            PsiProvenance::Operation(id(99, OperationId::new));
        mutations.push(("provenance", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0].fuel[0].units = 2;
        mutations.push(("fuel settlement", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0].effect.output = 77;
        mutations.push(("effect", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0].definitions[0].scalar_type = ScalarType::Boolean;
        mutations.push(("definition", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[1].uses[0].value = id(100, ValueId::new);
        mutations.push(("use", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0]
            .successors
            .push(OptimizationEdge {
                psi_edge: id(101, EdgeId::new),
                target: block,
                bindings: vec![TerminalValueBinding {
                    parameter: id(102, ValueId::new),
                    argument: id(103, ValueId::new),
                    scalar_type,
                }],
            });
        mutations.push(("successor", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0]
            .ownership
            .push(OwnershipEvent::ClaimTransfer(vec![id(104, ClaimId::new)]));
        mutations.push(("ownership", unit));

        for (field_class, unit) in mutations {
            assert_ne!(
                recompute_psi_optimization_unit_identity(&unit),
                baseline_identity,
                "{field_class} must contribute to canonical content identity"
            );
        }
    }

    #[test]
    fn observation_projection_keeps_external_events_and_semantic_accounting() {
        let unit = reconstruct_psi_optimization_unit_seed(
            &plan(),
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .unwrap();
        let observations = reconstruct_psi_observation_model(&unit);

        assert_eq!(observations.revision, unit.identity);
        assert_eq!(observations.nodes.len(), 2);
        assert!(observations.nodes[0].events.is_empty());
        assert_eq!(observations.nodes[0].crash, ObservationKnowledge::No);
        assert_eq!(observations.nodes[0].provenance.len(), 1);
        assert_eq!(observations.nodes[0].fuel.len(), 1);
        assert_eq!(observations.nodes[1].events.len(), 1);
        assert_eq!(
            observations.nodes[1].events[0].class,
            ObservationEventClass::NormalExit
        );
        assert!(matches!(
            observations.nodes[1].events[0].operation,
            TerminalAbstractOperation::Return { .. }
        ));
    }

    #[test]
    fn block_parameters_keep_terminal_declaration_order() {
        let mut plan = plan();
        let function = &mut plan.functions[0];
        let entry = function.entry;
        let target = id(20, BlockId::new);
        // Deliberately descending identities prove this is declaration order,
        // not the previous BTreeMap order.
        let first_parameter = id(90, ValueId::new);
        let second_parameter = id(80, ValueId::new);
        let first_argument = function.parameters[0].value;
        let second_argument = id(70, ValueId::new);
        let scalar_type = function.parameters[0].scalar_type;
        function.parameters.push(TerminalAbstractParameter {
            value: second_argument,
            scalar_type,
        });
        function.result = TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
            value: first_parameter,
            scalar_type,
        });
        function.block_entries = vec![
            TerminalAbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            },
            TerminalAbstractBlockEntry {
                block: target,
                parameters: vec![
                    TerminalAbstractParameter {
                        value: first_parameter,
                        scalar_type,
                    },
                    TerminalAbstractParameter {
                        value: second_parameter,
                        scalar_type,
                    },
                ],
                operation_offset: 1,
            },
        ];
        function.operations = vec![
            TerminalAbstractOperation::Jump {
                psi_edge: id(60, EdgeId::new),
                target,
                bindings: vec![
                    TerminalValueBinding {
                        parameter: first_parameter,
                        argument: first_argument,
                        scalar_type,
                    },
                    TerminalValueBinding {
                        parameter: second_parameter,
                        argument: second_argument,
                        scalar_type,
                    },
                ],
            },
            TerminalAbstractOperation::Return {
                psi_edge: id(61, EdgeId::new),
                result: first_parameter,
                value: first_parameter,
                scalar_type,
                cleanup_actions: Vec::new(),
            },
        ];

        let unit = reconstruct_psi_optimization_unit_seed(
            &plan,
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .expect("ordered block parameters");
        assert_eq!(
            unit.functions[0].blocks[1]
                .parameters
                .iter()
                .map(|parameter| parameter.value)
                .collect::<Vec<_>>(),
            vec![first_parameter, second_parameter]
        );
    }
}
