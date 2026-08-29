//! Immutable rewrite plans, witnesses, and candidate model.

use super::codec::{encode_definition_site, encode_integer_value, encode_len, encode_scalar_type};
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeLocation {
    pub machine: MachineId,
    pub block: BlockId,
    pub node: u32,
}

/// An exact occurrence of source semantic work in an optimization revision.
/// Successor edges are separate from their owner node because only the taken
/// arm executes. This distinction is required for path-dependent rewrites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsiRealizationSite {
    Node(NodeLocation),
    Edge { machine: MachineId, edge: EdgeId },
}

impl PsiRealizationSite {
    pub const fn machine(self) -> MachineId {
        match self {
            Self::Node(location) => location.machine,
            Self::Edge { machine, .. } => machine,
        }
    }

    pub const fn node(self) -> Option<NodeLocation> {
        match self {
            Self::Node(location) => Some(location),
            Self::Edge { .. } => None,
        }
    }
}

/// The exact disposition of source semantic work after one accepted rewrite.
///
/// A realized row names a node in the output revision. A proven-unreachable
/// row instead names the node in the input revision that owned a removed source
/// site; the node itself may survive, as with one rejected conditional edge.
/// Removal is legal only because the validating rewrite proved that no
/// execution can reach that source site. Fuel rows retain the source schedule
/// amount in both cases; only a realized disposition is a logical charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProvenanceDisposition {
    RealizedAt(PsiRealizationSite),
    ProvenUnreachableAt(PsiRealizationSite),
}

impl ProvenanceDisposition {
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::RealizedAt(_) => 1,
            Self::ProvenUnreachableAt(_) => 2,
        }
    }

    pub const fn site(self) -> PsiRealizationSite {
        match self {
            Self::RealizedAt(site) | Self::ProvenUnreachableAt(site) => site,
        }
    }

    pub const fn is_realized(self) -> bool {
        matches!(self, Self::RealizedAt(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarSubstitution {
    pub from: ValueId,
    pub to: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceRewrite {
    /// Exact occurrence in the input revision whose custody is transformed.
    pub input: PsiRealizationSite,
    pub disposition: ProvenanceDisposition,
    pub sources: Vec<PsiProvenance>,
    pub fuel: Vec<FuelSettlement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarEvaluationWitness {
    Unary {
        operand_fact: ScalarConstantFactIdentity,
    },
    Binary {
        left_fact: ScalarConstantFactIdentity,
        right_fact: ScalarConstantFactIdentity,
    },
    ProofCertifiedUnary {
        operand_fact: ScalarConstantFactIdentity,
        obligation_fact: AcceptedObligationFactIdentity,
    },
    ProofCertifiedBinary {
        left_fact: ScalarConstantFactIdentity,
        right_fact: ScalarConstantFactIdentity,
        obligation_fact: AcceptedObligationFactIdentity,
    },
    RangeAgainstConstant {
        range_fact: ValueRangeFactIdentity,
        constant_fact: ScalarConstantFactIdentity,
    },
    RangeAgainstRange {
        left_range_fact: ValueRangeFactIdentity,
        right_range_fact: ValueRangeFactIdentity,
    },
}

impl ScalarEvaluationWitness {
    pub const fn unary_operand(self) -> Option<ScalarConstantFactIdentity> {
        match self {
            Self::Unary { operand_fact } | Self::ProofCertifiedUnary { operand_fact, .. } => {
                Some(operand_fact)
            }
            Self::Binary { .. }
            | Self::ProofCertifiedBinary { .. }
            | Self::RangeAgainstConstant { .. }
            | Self::RangeAgainstRange { .. } => None,
        }
    }

    pub const fn binary_operands(
        self,
    ) -> Option<(ScalarConstantFactIdentity, ScalarConstantFactIdentity)> {
        match self {
            Self::Binary {
                left_fact,
                right_fact,
            }
            | Self::ProofCertifiedBinary {
                left_fact,
                right_fact,
                ..
            } => Some((left_fact, right_fact)),
            Self::Unary { .. }
            | Self::ProofCertifiedUnary { .. }
            | Self::RangeAgainstConstant { .. }
            | Self::RangeAgainstRange { .. } => None,
        }
    }

    pub const fn obligation_fact(self) -> Option<AcceptedObligationFactIdentity> {
        match self {
            Self::ProofCertifiedUnary {
                obligation_fact, ..
            }
            | Self::ProofCertifiedBinary {
                obligation_fact, ..
            } => Some(obligation_fact),
            Self::Unary { .. }
            | Self::Binary { .. }
            | Self::RangeAgainstConstant { .. }
            | Self::RangeAgainstRange { .. } => None,
        }
    }

    pub const fn range_against_constant(
        self,
    ) -> Option<(ValueRangeFactIdentity, ScalarConstantFactIdentity)> {
        match self {
            Self::RangeAgainstConstant {
                range_fact,
                constant_fact,
            } => Some((range_fact, constant_fact)),
            Self::Unary { .. }
            | Self::Binary { .. }
            | Self::ProofCertifiedUnary { .. }
            | Self::ProofCertifiedBinary { .. }
            | Self::RangeAgainstRange { .. } => None,
        }
    }

    pub const fn range_against_range(
        self,
    ) -> Option<(ValueRangeFactIdentity, ValueRangeFactIdentity)> {
        match self {
            Self::RangeAgainstRange {
                left_range_fact,
                right_range_fact,
            } => Some((left_range_fact, right_range_fact)),
            Self::Unary { .. }
            | Self::Binary { .. }
            | Self::ProofCertifiedUnary { .. }
            | Self::ProofCertifiedBinary { .. }
            | Self::RangeAgainstConstant { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockParameterIncomingBinding {
    pub source: BlockId,
    pub edge: EdgeId,
    pub argument: ValueId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RedundantBlockParameterWitness {
    pub incoming: Vec<BlockParameterIncomingBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarConstantValue {
    Boolean(bool),
    Integer(IntegerValue),
}

/// Bind one literal scalar fact to the exact immutable input and definition it
/// describes. The optimizer and independent validator may share this encoding,
/// but each must reconstruct its inputs independently.
pub fn literal_scalar_constant_fact_identity(
    input: OptimizationUnitIdentity,
    machine: MachineId,
    definition: ValueDefinition,
    constant: ScalarConstantValue,
    support: OperationId,
) -> Option<ScalarConstantFactIdentity> {
    match (definition.scalar_type, constant) {
        (ScalarType::Boolean, ScalarConstantValue::Boolean(_))
        | (ScalarType::Integer(_), ScalarConstantValue::Integer(_)) => {}
        _ => return None,
    }
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-literal-scalar-constant-fact.v1\0");
    canonical.extend_from_slice(&input.bytes());
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&definition.value.get().to_le_bytes());
    encode_scalar_type(&mut canonical, definition.scalar_type);
    encode_definition_site(&mut canonical, definition.site);
    match constant {
        ScalarConstantValue::Boolean(value) => {
            canonical.push(1);
            canonical.push(u8::from(value));
        }
        ScalarConstantValue::Integer(value) => {
            canonical.push(2);
            encode_integer_value(&mut canonical, value);
        }
    }
    canonical.extend_from_slice(&support.get().to_le_bytes());
    Some(ScalarConstantFactIdentity::from_canonical_bytes(&canonical))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SccpValueState {
    Unknown,
    Boolean(bool),
    Integer(IntegerValue),
    Overdefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SccpValueRow {
    pub definition: ValueDefinition,
    pub state: SccpValueState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SccpBlockRow {
    pub block: BlockId,
    pub executable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SccpEdgeState {
    Executable,
    Inexecutable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SccpEdgeRow {
    pub source: BlockId,
    pub edge: EdgeId,
    pub target: BlockId,
    pub state: SccpEdgeState,
}

/// Canonical result vocabulary for the coupled SCCP fixed point. It contains
/// every block, exact edge, and scalar definition in one machine, so a derived
/// fact identity cannot omit a competing incoming edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SccpMachineSnapshot {
    pub blocks: Vec<SccpBlockRow>,
    pub edges: Vec<SccpEdgeRow>,
    pub values: Vec<SccpValueRow>,
}

pub fn derived_sccp_scalar_constant_fact_identity(
    input: OptimizationUnitIdentity,
    machine: MachineId,
    definition: ValueDefinition,
    constant: ScalarConstantValue,
    snapshot: &SccpMachineSnapshot,
) -> Option<ScalarConstantFactIdentity> {
    if snapshot
        .blocks
        .windows(2)
        .any(|pair| pair[0].block >= pair[1].block)
        || snapshot
            .edges
            .windows(2)
            .any(|pair| (pair[0].source, pair[0].edge) >= (pair[1].source, pair[1].edge))
        || snapshot
            .values
            .windows(2)
            .any(|pair| pair[0].definition.value >= pair[1].definition.value)
    {
        return None;
    }
    let expected_state = match (definition.scalar_type, constant) {
        (ScalarType::Boolean, ScalarConstantValue::Boolean(value)) => {
            SccpValueState::Boolean(value)
        }
        (ScalarType::Integer(_), ScalarConstantValue::Integer(value)) => {
            SccpValueState::Integer(value)
        }
        _ => return None,
    };
    if !snapshot
        .values
        .iter()
        .any(|row| row.definition == definition && row.state == expected_state)
    {
        return None;
    }

    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-derived-sccp-scalar-constant-fact.v1\0");
    canonical.extend_from_slice(&input.bytes());
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&definition.value.get().to_le_bytes());
    encode_scalar_type(&mut canonical, definition.scalar_type);
    encode_definition_site(&mut canonical, definition.site);
    encode_scalar_constant_value(&mut canonical, constant);
    encode_len(&mut canonical, snapshot.blocks.len());
    for row in &snapshot.blocks {
        canonical.extend_from_slice(&row.block.get().to_le_bytes());
        canonical.push(u8::from(row.executable));
    }
    encode_len(&mut canonical, snapshot.edges.len());
    for row in &snapshot.edges {
        canonical.extend_from_slice(&row.source.get().to_le_bytes());
        canonical.extend_from_slice(&row.edge.get().to_le_bytes());
        canonical.extend_from_slice(&row.target.get().to_le_bytes());
        canonical.push(match row.state {
            SccpEdgeState::Executable => 1,
            SccpEdgeState::Inexecutable => 2,
            SccpEdgeState::Unknown => 3,
        });
    }
    encode_len(&mut canonical, snapshot.values.len());
    for row in &snapshot.values {
        canonical.extend_from_slice(&row.definition.value.get().to_le_bytes());
        encode_scalar_type(&mut canonical, row.definition.scalar_type);
        encode_definition_site(&mut canonical, row.definition.site);
        match row.state {
            SccpValueState::Unknown => canonical.push(1),
            SccpValueState::Boolean(value) => {
                canonical.push(2);
                canonical.push(u8::from(value));
            }
            SccpValueState::Integer(value) => {
                canonical.push(3);
                encode_integer_value(&mut canonical, value);
            }
            SccpValueState::Overdefined => canonical.push(4),
        }
    }
    Some(ScalarConstantFactIdentity::from_canonical_bytes(&canonical))
}

fn encode_scalar_constant_value(bytes: &mut Vec<u8>, constant: ScalarConstantValue) {
    match constant {
        ScalarConstantValue::Boolean(value) => {
            bytes.push(1);
            bytes.push(u8::from(value));
        }
        ScalarConstantValue::Integer(value) => {
            bytes.push(2);
            encode_integer_value(bytes, value);
        }
    }
}

/// Compatibility name retained while integer-only rules migrate to the shared
/// scalar candidate vocabulary.
pub type IntegerEvaluationWitness = ScalarEvaluationWitness;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntegerConstantRewrite {
    pub location: NodeLocation,
    pub source_operation: OperationId,
    pub result: ValueId,
    pub scalar_type: IntegerType,
    pub constant: IntegerValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BooleanConstantRewrite {
    pub location: NodeLocation,
    pub source_operation: OperationId,
    pub result: ValueId,
    pub constant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RedundantBlockParameterRewrite {
    pub machine: MachineId,
    pub block: BlockId,
    pub position: u32,
    pub parameter: ValueId,
    pub replacement: ValueId,
    pub scalar_type: ScalarType,
}

/// Replace one Boolean-proven conditional with its exact selected edge. Both
/// edge identities are bound so replay cannot silently swap or discard a
/// different successor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstantConditionalRewrite {
    pub location: NodeLocation,
    pub condition: ValueId,
    pub constant: bool,
    pub selected_edge: EdgeId,
    pub rejected_edge: EdgeId,
}

/// Thread one non-entry, single-incoming block whose only node is an
/// unconditional jump. The predecessor and removed jump are necessarily
/// co-executed, so both source edges remain realized at `predecessor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinearEmptyBlockRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub empty: NodeLocation,
    pub outgoing_edge: EdgeId,
    pub target: BlockId,
}

/// Thread one non-entry empty jump block through every exact incoming edge.
/// The outgoing source occurrence fans out to those mutually exclusive edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathQualifiedEmptyBlockRewrite {
    pub empty: NodeLocation,
    pub outgoing_edge: EdgeId,
    pub target: BlockId,
}

/// Merge the immediately adjacent, single-predecessor target block into an
/// unconditional predecessor. The target's block parameters are replaced by
/// the exact incoming bindings. The removed edge is realized at the first
/// moved operation or, for a conditional-only target, on both mutually
/// exclusive successor edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdjacentBlockMergeRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub target: BlockId,
}

/// One exact verifier-owned ownership fact consumed by an adjacent block
/// merge. Rows are canonical in source-site order; the rule-specific
/// validator reconstructs both the required site set and each fact identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierWitnessRow {
    pub site: OwnershipFrontierSite,
    pub fact: OwnershipFrontierFactIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierWitness {
    pub rows: Vec<OwnershipFrontierWitnessRow>,
}

/// Merge a non-adjacent, single-predecessor target block into its
/// unconditional predecessor. Unlike the adjacent form, this patch explicitly
/// authorizes movement across intervening source-roster blocks; execution
/// legality is still established from CFG dominance rather than roster order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonAdjacentBlockMergeRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub target: BlockId,
}

/// Fuse one unconditional jump into a shared, terminal-only target without
/// removing that target. The terminal occurrence is cloned onto the selected
/// incoming path and remains at the target for every other incoming path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SharedJumpFusionRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub target: BlockId,
}

/// Remove one unused, independently total scalar-producing node. Its source
/// occurrence and fuel are fused into the immediately following direct node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeadScalarNodeRewrite {
    pub location: NodeLocation,
    pub source_operation: OperationId,
    pub result: ValueId,
    pub scalar_type: ScalarType,
}

/// The closed integer identities whose verifier-accepted obligation permits
/// the operation to disappear while its live result is replaced by an
/// existing operand. Exact, wrapping, and saturating operation identities are
/// never reclassified across policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofCertifiedScalarIdentityKind {
    ExactIntegerAddZeroLeft,
    ExactIntegerAddZeroRight,
    ExactIntegerSubtractZeroRight,
    ExactIntegerMultiplyOneLeft,
    ExactIntegerMultiplyOneRight,
    ExactIntegerShiftLeftZeroCount,
    ExactIntegerShiftRightZeroCount,
    ExactIntegerDivideOneRight,
    WrappingIntegerDivideOneRight,
    SaturatingIntegerDivideOneRight,
    ExactIntegerMultiplyZeroLeft,
    ExactIntegerMultiplyZeroRight,
    ExactIntegerDivideZeroLeft,
    WrappingIntegerDivideZeroLeft,
    SaturatingIntegerDivideZeroLeft,
    ExactIntegerRemainderZeroLeft,
    WrappingIntegerRemainderZeroLeft,
    SaturatingIntegerRemainderZeroLeft,
    ExactIntegerShiftLeftZeroValue,
    ExactIntegerShiftRightZeroValue,
    ExactIntegerShiftRightNegativeOneValue,
}

/// Remove one proof-certified integer identity and replace every use of its
/// live result with the equivalent existing operand. The removed occurrence
/// and fuel remain realized at the next co-executed node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofCertifiedScalarIdentityRewrite {
    pub location: NodeLocation,
    pub source_operation: OperationId,
    pub result: ValueId,
    pub replacement: ValueId,
    pub scalar_type: IntegerType,
    pub identity: ProofCertifiedScalarIdentityKind,
}

/// Replace every use of a later same-block scalar result with the result of an
/// earlier, independently identical total scalar expression, then remove the
/// redundant node. The later source occurrence remains realized at its next
/// co-executed node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalScalarCommonSubexpressionRewrite {
    pub leader: NodeLocation,
    pub redundant: NodeLocation,
    pub leader_operation: OperationId,
    pub redundant_operation: OperationId,
    pub leader_result: ValueId,
    pub redundant_result: ValueId,
    pub scalar_type: ScalarType,
}

/// Replace every use of a scalar result with an equivalent result defined in
/// a different block that independently dominates the redundant definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DominatingScalarCommonSubexpressionRewrite {
    pub leader: NodeLocation,
    pub redundant: NodeLocation,
    pub leader_operation: OperationId,
    pub redundant_operation: OperationId,
    pub leader_result: ValueId,
    pub redundant_result: ValueId,
    pub scalar_type: ScalarType,
}

/// One incoming control-flow arm supplying a value already computed for a
/// phi-translated total scalar expression at the target block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhiTranslatedScalarIncoming {
    pub source: BlockId,
    pub edge: EdgeId,
    pub leader: NodeLocation,
    pub leader_operation: OperationId,
    pub leader_result: ValueId,
}

/// Preserve the redundant result identity as a new target-block parameter,
/// bind every incoming edge to its available translated leader, and remove the
/// now-redundant target-block computation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhiTranslatedScalarGvnRewrite {
    pub redundant: NodeLocation,
    pub redundant_operation: OperationId,
    pub redundant_result: ValueId,
    pub scalar_type: ScalarType,
    pub parameter_position: u32,
    pub incoming: Vec<PhiTranslatedScalarIncoming>,
}

/// Remove the exact canonical complement of the independently reconstructed
/// executable-machine root closure.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnreachablePrivateMachinesRewrite {
    pub machines: Vec<crate::PrunedMachineCustody>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsiRewriteDecisionPoint {
    Node(NodeLocation),
    MachineSet(Vec<MachineId>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsiRewritePatch {
    ReplaceIntegerOperationWithConstant(IntegerConstantRewrite),
    ReplaceBooleanOperationWithConstant(BooleanConstantRewrite),
    RemoveRedundantBlockParameter(RedundantBlockParameterRewrite),
    FoldConstantConditional(ConstantConditionalRewrite),
    ThreadLinearEmptyBlock(LinearEmptyBlockRewrite),
    ThreadPathQualifiedEmptyBlock(PathQualifiedEmptyBlockRewrite),
    MergeAdjacentBlock(AdjacentBlockMergeRewrite),
    MergeNonAdjacentBlock(NonAdjacentBlockMergeRewrite),
    FuseSharedTerminalJump(SharedJumpFusionRewrite),
    RemoveDeadScalarNode(DeadScalarNodeRewrite),
    EliminateLocalScalarCommonSubexpression(LocalScalarCommonSubexpressionRewrite),
    EliminateDominatedScalarCommonSubexpression(DominatingScalarCommonSubexpressionRewrite),
    EliminatePhiTranslatedScalarCommonSubexpression(PhiTranslatedScalarGvnRewrite),
    EliminateProofCertifiedScalarIdentity(ProofCertifiedScalarIdentityRewrite),
    PruneUnreachablePrivateMachines(UnreachablePrivateMachinesRewrite),
}

impl PsiRewritePatch {
    pub fn pruned_machine_custody(&self) -> &[crate::PrunedMachineCustody] {
        match self {
            Self::PruneUnreachablePrivateMachines(patch) => &patch.machines,
            _ => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum PsiRewriteWitness {
    ScalarEvaluation(ScalarEvaluationWitness),
    RedundantBlockParameter(RedundantBlockParameterWitness),
    AcceptedObligation(AcceptedObligationFactIdentity),
    ProofCertifiedScalarIdentity {
        constant_fact: ScalarConstantFactIdentity,
        obligation_fact: AcceptedObligationFactIdentity,
    },
    OwnershipFrontiers(OwnershipFrontierWitness),
    StructuralIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiRewriteCandidate {
    pub(super) identity: OptimizationCandidateIdentity,
    pub(super) input: OptimizationUnitIdentity,
    pub(super) rule: OptimizationRuleIdentity,
    pub(super) decision_point: PsiRewriteDecisionPoint,
    pub(super) affected_blocks: Vec<BlockId>,
    pub(super) required_analyses: AnalysisSet,
    pub(super) invalidated_analyses: AnalysisInvalidationSet,
    pub(super) safety_class: OptimizationSafetyClass,
    pub(super) substitutions: Vec<ScalarSubstitution>,
    pub(super) provenance: Vec<ProvenanceRewrite>,
    pub(super) witness: PsiRewriteWitness,
    pub(super) predicted_cost_delta: i64,
    pub(super) patch: PsiRewritePatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsiRewriteCandidateError {
    EmptyAffectedRegion,
    NonCanonicalAffectedRegion,
    DecisionPointOutsideRegion,
    NonCanonicalSubstitutions,
    EmptyProvenanceSource,
    NonCanonicalProvenance,
    FuelProvenanceMismatch,
    PatchDecisionPointMismatch,
    EmptyIncomingBindings,
    NonCanonicalIncomingBindings,
    NonCanonicalOwnershipFrontierWitness,
    BlockParameterSubstitutionMismatch,
    ProofWitnessSafetyMismatch,
}

impl std::fmt::Display for PsiRewriteCandidateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Psi rewrite candidate: {self:?}")
    }
}

impl std::error::Error for PsiRewriteCandidateError {}
