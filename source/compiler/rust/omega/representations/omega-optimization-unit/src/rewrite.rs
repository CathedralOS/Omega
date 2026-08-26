use std::collections::BTreeSet;

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisSet, OptimizationCandidateIdentity, OptimizationRuleContract,
    OptimizationRuleIdentity, OptimizationSafetyClass, OptimizationUnitIdentity,
};
use psi_core::{
    BlockId, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};

use crate::{FuelSettlement, PsiProvenance};

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
pub struct IntegerEvaluationWitness {
    pub left_support: OperationId,
    pub right_support: OperationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntegerConstantRewrite {
    pub location: NodeLocation,
    pub source_operation: OperationId,
    pub result: ValueId,
    pub scalar_type: IntegerType,
    pub constant: IntegerValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsiRewritePatch {
    ReplaceIntegerOperationWithConstant(IntegerConstantRewrite),
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
    witness: IntegerEvaluationWitness,
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
        witness: IntegerEvaluationWitness,
        predicted_cost_delta: i64,
        patch: IntegerConstantRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        if affected_blocks.is_empty() {
            return Err(PsiRewriteCandidateError::EmptyAffectedRegion);
        }
        if affected_blocks.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(PsiRewriteCandidateError::NonCanonicalAffectedRegion);
        }
        if !affected_blocks.contains(&patch.location.block) {
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
        if provenance.iter().any(|row| row.output != patch.location) {
            return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
        }
        let decision_point = patch.location;
        let patch = PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch);
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

    pub const fn witness(&self) -> IntegerEvaluationWitness {
        self.witness
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
    witness: IntegerEvaluationWitness,
    predicted_cost_delta: i64,
    patch: PsiRewritePatch,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.psi-rewrite-candidate.v1\0");
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
    bytes.extend_from_slice(&witness.left_support.get().to_le_bytes());
    bytes.extend_from_slice(&witness.right_support.get().to_le_bytes());
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
    }
    bytes
}

fn encode_location(bytes: &mut Vec<u8>, location: NodeLocation) {
    bytes.extend_from_slice(&location.machine.get().to_le_bytes());
    bytes.extend_from_slice(&location.block.get().to_le_bytes());
    bytes.extend_from_slice(&location.node.to_le_bytes());
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
