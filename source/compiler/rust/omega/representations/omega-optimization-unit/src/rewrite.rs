use std::collections::BTreeSet;

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisSet, OptimizationCandidateIdentity, OptimizationFactReference,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
    OptimizationUnitIdentity, ScalarConstantFactIdentity,
};
use psi_core::{
    BlockId, EdgeId, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};

use crate::{FuelSettlement, PsiProvenance, ValueDefinition, ValueDefinitionSite};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeLocation {
    pub machine: MachineId,
    pub block: BlockId,
    pub node: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarSubstitution {
    pub from: ValueId,
    pub to: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceRewrite {
    pub output: NodeLocation,
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
pub enum PsiRewritePatch {
    ReplaceIntegerOperationWithConstant(IntegerConstantRewrite),
    ReplaceBooleanOperationWithConstant(BooleanConstantRewrite),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiRewriteCandidate {
    identity: OptimizationCandidateIdentity,
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    rule: OptimizationRuleIdentity,
    decision_point: NodeLocation,
    affected_blocks: Vec<BlockId>,
    required_analyses: AnalysisSet,
    invalidated_analyses: AnalysisInvalidationSet,
    safety_class: OptimizationSafetyClass,
    substitutions: Vec<ScalarSubstitution>,
    provenance: Vec<ProvenanceRewrite>,
    witness: ScalarEvaluationWitness,
    predicted_cost_delta: i64,
    patch: PsiRewritePatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsiRewriteCandidateError {
    EmptyAffectedRegion,
    NonCanonicalAffectedRegion,
    DecisionPointOutsideRegion,
    NonCanonicalSubstitutions,
    EmptyProvenanceSource,
    NonCanonicalProvenance,
    PatchDecisionPointMismatch,
}

impl std::fmt::Display for PsiRewriteCandidateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Psi rewrite candidate: {self:?}")
    }
}

impl std::error::Error for PsiRewriteCandidateError {}

impl PsiRewriteCandidate {
    #[allow(clippy::too_many_arguments)]
    pub fn new_integer_evaluation(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        witness: ScalarEvaluationWitness,
        predicted_cost_delta: i64,
        patch: IntegerConstantRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new_scalar_evaluation(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            witness,
            predicted_cost_delta,
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_boolean_evaluation(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        witness: ScalarEvaluationWitness,
        predicted_cost_delta: i64,
        patch: BooleanConstantRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new_scalar_evaluation(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            witness,
            predicted_cost_delta,
            PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_scalar_evaluation(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        witness: ScalarEvaluationWitness,
        predicted_cost_delta: i64,
        patch: PsiRewritePatch,
    ) -> Result<Self, PsiRewriteCandidateError> {
        let location = match patch {
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) => patch.location,
            PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) => patch.location,
        };
        if affected_blocks.is_empty() {
            return Err(PsiRewriteCandidateError::EmptyAffectedRegion);
        }
        if affected_blocks.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(PsiRewriteCandidateError::NonCanonicalAffectedRegion);
        }
        if !affected_blocks.contains(&location.block) {
            return Err(PsiRewriteCandidateError::DecisionPointOutsideRegion);
        }
        if substitutions.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(PsiRewriteCandidateError::NonCanonicalSubstitutions);
        }
        if provenance.iter().any(|row| row.sources.is_empty()) {
            return Err(PsiRewriteCandidateError::EmptyProvenanceSource);
        }
        if provenance
            .windows(2)
            .any(|pair| pair[0].output >= pair[1].output)
            || provenance.iter().any(|row| {
                row.sources.iter().copied().collect::<BTreeSet<_>>().len() != row.sources.len()
            })
        {
            return Err(PsiRewriteCandidateError::NonCanonicalProvenance);
        }
        if provenance.iter().any(|row| row.output != location) {
            return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
        }
        let decision_point = location;
        let canonical = encode_candidate(
            input,
            contract,
            decision_point,
            &affected_blocks,
            &substitutions,
            &provenance,
            witness,
            predicted_cost_delta,
            patch,
        );
        let identity = OptimizationCandidateIdentity::from_canonical_bytes(&canonical);
        let mut output_canonical = Vec::with_capacity(64);
        output_canonical.extend_from_slice(&input.bytes());
        output_canonical.extend_from_slice(&identity.bytes());
        let output = OptimizationUnitIdentity::from_canonical_bytes(&output_canonical);
        Ok(Self {
            identity,
            input,
            output,
            rule: contract.identity(),
            decision_point,
            affected_blocks,
            required_analyses: contract.required_analyses(),
            invalidated_analyses: contract.invalidated_analyses(),
            safety_class: contract.safety_class(),
            substitutions,
            provenance,
            witness,
            predicted_cost_delta,
            patch,
        })
    }

    pub const fn identity(&self) -> OptimizationCandidateIdentity {
        self.identity
    }

    pub const fn input(&self) -> OptimizationUnitIdentity {
        self.input
    }

    pub const fn output(&self) -> OptimizationUnitIdentity {
        self.output
    }

    pub const fn rule(&self) -> OptimizationRuleIdentity {
        self.rule
    }

    pub const fn decision_point(&self) -> NodeLocation {
        self.decision_point
    }

    pub fn affected_blocks(&self) -> &[BlockId] {
        &self.affected_blocks
    }

    pub const fn required_analyses(&self) -> AnalysisSet {
        self.required_analyses
    }

    pub const fn invalidated_analyses(&self) -> AnalysisInvalidationSet {
        self.invalidated_analyses
    }

    pub const fn safety_class(&self) -> OptimizationSafetyClass {
        self.safety_class
    }

    pub fn substitutions(&self) -> &[ScalarSubstitution] {
        &self.substitutions
    }

    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        &self.provenance
    }

    pub const fn witness(&self) -> ScalarEvaluationWitness {
        self.witness
    }

    pub fn consumed_facts(&self) -> Vec<OptimizationFactReference> {
        let mut facts = match self.witness {
            ScalarEvaluationWitness::Unary { operand_fact } => {
                vec![OptimizationFactReference::ScalarConstant(operand_fact)]
            }
            ScalarEvaluationWitness::Binary {
                left_fact,
                right_fact,
            } => vec![
                OptimizationFactReference::ScalarConstant(left_fact),
                OptimizationFactReference::ScalarConstant(right_fact),
            ],
        };
        facts.sort_unstable();
        facts.dedup();
        facts
    }

    pub const fn predicted_cost_delta(&self) -> i64 {
        self.predicted_cost_delta
    }

    pub const fn patch(&self) -> PsiRewritePatch {
        self.patch
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_candidate(
    input: OptimizationUnitIdentity,
    contract: OptimizationRuleContract,
    decision_point: NodeLocation,
    affected_blocks: &[BlockId],
    substitutions: &[ScalarSubstitution],
    provenance: &[ProvenanceRewrite],
    witness: ScalarEvaluationWitness,
    predicted_cost_delta: i64,
    patch: PsiRewritePatch,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.psi-rewrite-candidate.v3\0");
    bytes.extend_from_slice(&input.bytes());
    bytes.extend_from_slice(&contract.encode());
    encode_location(&mut bytes, decision_point);
    encode_len(&mut bytes, affected_blocks.len());
    for block in affected_blocks {
        bytes.extend_from_slice(&block.get().to_le_bytes());
    }
    encode_len(&mut bytes, substitutions.len());
    for substitution in substitutions {
        bytes.extend_from_slice(&substitution.from.get().to_le_bytes());
        bytes.extend_from_slice(&substitution.to.get().to_le_bytes());
        encode_scalar_type(&mut bytes, substitution.scalar_type);
    }
    encode_len(&mut bytes, provenance.len());
    for row in provenance {
        encode_location(&mut bytes, row.output);
        encode_len(&mut bytes, row.sources.len());
        for source in &row.sources {
            match source {
                PsiProvenance::Operation(operation) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&operation.get().to_le_bytes());
                }
                PsiProvenance::Edge(edge) => {
                    bytes.push(2);
                    bytes.extend_from_slice(&edge.get().to_le_bytes());
                }
            }
        }
        encode_len(&mut bytes, row.fuel.len());
        for settlement in &row.fuel {
            match settlement.site {
                PsiProvenance::Operation(operation) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&operation.get().to_le_bytes());
                }
                PsiProvenance::Edge(edge) => {
                    bytes.push(2);
                    bytes.extend_from_slice(&edge.get().to_le_bytes());
                }
            }
            bytes.extend_from_slice(&settlement.units.to_le_bytes());
        }
    }
    match witness {
        ScalarEvaluationWitness::Unary { operand_fact } => {
            bytes.push(1);
            bytes.extend_from_slice(&operand_fact.bytes());
        }
        ScalarEvaluationWitness::Binary {
            left_fact,
            right_fact,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&left_fact.bytes());
            bytes.extend_from_slice(&right_fact.bytes());
        }
    }
    bytes.extend_from_slice(&predicted_cost_delta.to_le_bytes());
    match patch {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) => {
            bytes.push(1);
            encode_location(&mut bytes, patch.location);
            bytes.extend_from_slice(&patch.source_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.result.get().to_le_bytes());
            encode_integer_type(&mut bytes, patch.scalar_type);
            encode_integer_value(&mut bytes, patch.constant);
        }
        PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) => {
            bytes.push(2);
            encode_location(&mut bytes, patch.location);
            bytes.extend_from_slice(&patch.source_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.result.get().to_le_bytes());
            bytes.push(u8::from(patch.constant));
        }
    }
    bytes
}

fn encode_location(bytes: &mut Vec<u8>, location: NodeLocation) {
    bytes.extend_from_slice(&location.machine.get().to_le_bytes());
    bytes.extend_from_slice(&location.block.get().to_le_bytes());
    bytes.extend_from_slice(&location.node.to_le_bytes());
}

fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(1);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(3);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(
        &u64::try_from(len)
            .expect("canonical candidate list length fits u64")
            .to_le_bytes(),
    );
}

fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.push(1),
        ScalarType::Integer(integer) => {
            bytes.push(2);
            encode_integer_type(bytes, integer);
        }
    }
}

fn encode_integer_type(bytes: &mut Vec<u8>, integer: IntegerType) {
    bytes.push(match integer.carrier() {
        IntegerCarrier::Fixed => 1,
        IntegerCarrier::Address => 2,
    });
    bytes.push(match integer.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
    });
    bytes.extend_from_slice(&integer.bits().to_le_bytes());
}

fn encode_integer_value(bytes: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            bytes.push(2);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_fact_identity_binds_revision_definition_value_type_constant_and_support() {
        let revision = OptimizationUnitIdentity::from_canonical_bytes(b"revision-a");
        let machine = MachineId::new(1).unwrap();
        let definition = ValueDefinition {
            value: ValueId::new(2).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
            site: ValueDefinitionSite::Node {
                block: BlockId::new(3).unwrap(),
                node: 4,
            },
        };
        let identity = literal_scalar_constant_fact_identity(
            revision,
            machine,
            definition,
            ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
            OperationId::new(5).unwrap(),
        )
        .unwrap();
        assert_eq!(
            identity,
            literal_scalar_constant_fact_identity(
                revision,
                machine,
                definition,
                ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
                OperationId::new(5).unwrap(),
            )
            .unwrap()
        );
        assert_ne!(
            identity,
            literal_scalar_constant_fact_identity(
                OptimizationUnitIdentity::from_canonical_bytes(b"revision-b"),
                machine,
                definition,
                ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
                OperationId::new(5).unwrap(),
            )
            .unwrap()
        );
        assert_ne!(
            identity,
            literal_scalar_constant_fact_identity(
                revision,
                machine,
                ValueDefinition {
                    site: ValueDefinitionSite::Node {
                        block: BlockId::new(3).unwrap(),
                        node: 6,
                    },
                    ..definition
                },
                ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
                OperationId::new(5).unwrap(),
            )
            .unwrap()
        );
        assert_ne!(
            identity,
            literal_scalar_constant_fact_identity(
                revision,
                machine,
                definition,
                ScalarConstantValue::Integer(IntegerValue::Unsigned(8)),
                OperationId::new(5).unwrap(),
            )
            .unwrap()
        );
        assert_ne!(
            identity,
            literal_scalar_constant_fact_identity(
                revision,
                machine,
                definition,
                ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
                OperationId::new(6).unwrap(),
            )
            .unwrap()
        );
        assert!(
            literal_scalar_constant_fact_identity(
                revision,
                machine,
                definition,
                ScalarConstantValue::Boolean(true),
                OperationId::new(5).unwrap(),
            )
            .is_none()
        );
    }

    #[test]
    fn derived_sccp_identity_binds_every_exact_edge_verdict() {
        let revision = OptimizationUnitIdentity::from_canonical_bytes(b"sccp-revision");
        let machine = MachineId::new(10).unwrap();
        let entry = BlockId::new(11).unwrap();
        let merge = BlockId::new(12).unwrap();
        let definition = ValueDefinition {
            value: ValueId::new(13).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
            site: ValueDefinitionSite::BlockParameter {
                block: merge,
                position: 0,
            },
        };
        let snapshot = SccpMachineSnapshot {
            blocks: vec![
                SccpBlockRow {
                    block: entry,
                    executable: true,
                },
                SccpBlockRow {
                    block: merge,
                    executable: true,
                },
            ],
            edges: vec![
                SccpEdgeRow {
                    source: entry,
                    edge: EdgeId::new(14).unwrap(),
                    target: merge,
                    state: SccpEdgeState::Executable,
                },
                SccpEdgeRow {
                    source: entry,
                    edge: EdgeId::new(15).unwrap(),
                    target: merge,
                    state: SccpEdgeState::Inexecutable,
                },
            ],
            values: vec![SccpValueRow {
                definition,
                state: SccpValueState::Integer(IntegerValue::Unsigned(7)),
            }],
        };
        let identity = derived_sccp_scalar_constant_fact_identity(
            revision,
            machine,
            definition,
            ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
            &snapshot,
        )
        .unwrap();
        let mut changed_verdict = snapshot.clone();
        changed_verdict.edges[1].state = SccpEdgeState::Unknown;
        assert_ne!(
            identity,
            derived_sccp_scalar_constant_fact_identity(
                revision,
                machine,
                definition,
                ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
                &changed_verdict,
            )
            .unwrap()
        );
        let mut omitted_edge = snapshot.clone();
        omitted_edge.edges.pop();
        assert_ne!(
            identity,
            derived_sccp_scalar_constant_fact_identity(
                revision,
                machine,
                definition,
                ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
                &omitted_edge,
            )
            .unwrap()
        );
        let mut noncanonical = snapshot;
        noncanonical.edges.reverse();
        assert!(
            derived_sccp_scalar_constant_fact_identity(
                revision,
                machine,
                definition,
                ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
                &noncanonical,
            )
            .is_none()
        );
    }
}
