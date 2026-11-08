/*
We define the immutable rewrite candidate taxonomy (`PsiRewriteCandidate`,
`PsiRewritePatch`, `PsiCandidateDeclaration`), scalar and control-flow rewrite
plans, SCCP machine snapshots, and loop-invariant scalar motion descriptors.
Every candidate constructor derives a canonical `PsiRewriteDecisionPoint`,
validates common and patch-specific custody invariants, and seals its
`OptimizationCandidateIdentity` before returning an admitted candidate.
*/

use optimization_core::{
    AcceptedObligationFactIdentity, AnalysisInvalidationSet, AnalysisSet,
    OptimizationCandidateIdentity, OptimizationFactReference, OptimizationRuleContract,
    OptimizationRuleIdentity, OptimizationSafetyClass, OptimizationUnitIdentity,
    OwnershipFrontierFactIdentity, ScalarConstantFactIdentity, ValueRangeFactIdentity,
};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, EdgeId, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, ValueId,
};

use crate::{
    OwnershipFrontierSite, PsiProvenance, ValueDefinition, ValueDefinitionSite, WorkSettlement,
};

pub use cfg_rewrite_plans::{
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, CaseMembershipSpecializationRewrite,
    ConstantConditionalRewrite, FieldValueResolution, FieldValueRow,
    FieldValueSpecializationRewrite, FoldedCaseMembershipRow, FoldedFieldValue,
    ForwardedFieldValue, LinearEmptyBlockRewrite, NonAdjacentBlockMergeRewrite,
    OwnershipFrontierWitness, OwnershipFrontierWitnessRow, PathQualifiedEmptyBlockRewrite,
    RedundantBlockParameterRewrite, RedundantBlockParameterWitness, SharedJumpFusionRewrite,
    SpecializedStateEdgeRow, StateArgumentSpecializationRewrite, UnreachablePrivateMachinesRewrite,
};

use contracts::PsiRewriteWitness;
pub use contracts::{
    PsiRewriteCandidate, PsiRewriteCandidateError, PsiRewriteDecisionPoint, PsiRewritePatch,
};

pub use declarations::PsiCandidateDeclaration;
pub use foundations::{
    NodeLocation, ProvenanceDisposition, ProvenanceRewrite, PsiRealizationSite, ScalarSubstitution,
    encode_location,
};

pub use loop_invariant_scalar_motion::{
    LoopInvariantNodeResult, LoopInvariantScalarMotionCandidate, LoopInvariantScalarNode,
    LoopInvariantScalarRelocation,
};

pub(crate) use scalar_evaluation::ScalarEvaluationWitness;
pub use scalar_evaluation::{
    IntegerEvaluationWitness, ScalarConstantValue, literal_scalar_constant_fact_identity,
};
pub use scalar_rewrite_plans::{
    BooleanConstantRewrite, DeadScalarNodeRewrite, DominatingScalarCommonSubexpressionRewrite,
    IntegerConstantRewrite, LocalScalarCommonSubexpressionRewrite, PhiTranslatedScalarGvnRewrite,
    PhiTranslatedScalarIncoming, ProofCertifiedScalarIdentityKind,
    ProofCertifiedScalarIdentityRewrite, TotalScalarIdentityKind, TotalScalarIdentityRewrite,
};

pub use sccp::{
    SccpBlockRow, SccpEdgeRow, SccpEdgeState, SccpMachineSnapshot, SccpValueRow, SccpValueState,
    derived_sccp_scalar_constant_fact_identity,
};

mod foundations {
    use super::{BlockId, EdgeId, MachineId, PsiProvenance, ScalarType, ValueId, WorkSettlement};
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct NodeLocation {
        pub machine: MachineId,
        pub block: BlockId,
        pub node: u32,
    }

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
        pub input: PsiRealizationSite,
        pub disposition: ProvenanceDisposition,
        pub sources: Vec<PsiProvenance>,
        pub work: Vec<WorkSettlement>,
    }

    impl ProvenanceRewrite {
        pub fn canonical_sort_key(&self) -> (PsiRealizationSite, u8, PsiRealizationSite) {
            (
                self.input,
                self.disposition.canonical_tag(),
                self.disposition.site(),
            )
        }
    }

    pub fn encode_location(canonical: &mut Vec<u8>, location: NodeLocation) {
        crate::codec_bytes::push_encoded(canonical, location.machine.get().to_le_bytes());
        crate::codec_bytes::push_encoded(canonical, location.block.get().to_le_bytes());
        crate::codec_bytes::push_encoded(canonical, location.node.to_le_bytes());
    }
}

mod canonical_encoding {
    pub(super) use crate::range::{encode_integer_type, encode_integer_value};

    use super::{ScalarType, ValueDefinitionSite};
    pub(super) fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
        match site {
            ValueDefinitionSite::FunctionParameter(position) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, position.to_le_bytes());
            }
            ValueDefinitionSite::BlockParameter { block, position } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, position.to_le_bytes());
            }
            ValueDefinitionSite::Node { block, node } => {
                bytes.push(3);
                crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, node.to_le_bytes());
            }
        }
    }

    pub(super) fn encode_len(bytes: &mut Vec<u8>, len: usize) {
        bytes.extend_from_slice(
            &u64::try_from(len)
                .expect("canonical candidate list length fits u64")
                .to_le_bytes(),
        );
    }

    pub(super) fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
        super::super::encode_scalar_type(
            bytes,
            scalar_type,
            |bytes, tag| bytes.push(tag),
            encode_integer_type,
        )
    }
}

mod scalar_evaluation {
    use super::canonical_encoding::{
        encode_definition_site, encode_integer_value, encode_scalar_type,
    };
    use super::{
        AcceptedObligationFactIdentity, IntegerValue, MachineId, OperationId,
        OptimizationUnitIdentity, ScalarConstantFactIdentity, ScalarType, ValueDefinition,
        ValueRangeFactIdentity,
    };

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
    pub enum ScalarConstantValue {
        Boolean(bool),
        Integer(IntegerValue),
    }

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
        crate::codec_bytes::push_encoded(&mut canonical, machine.get().to_le_bytes());
        crate::codec_bytes::push_encoded(&mut canonical, definition.value.get().to_le_bytes());
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
        crate::codec_bytes::push_encoded(&mut canonical, support.get().to_le_bytes());
        Some(ScalarConstantFactIdentity::from_canonical_bytes(&canonical))
    }

    pub type IntegerEvaluationWitness = ScalarEvaluationWitness;
}

mod sccp {
    use super::canonical_encoding::{
        encode_definition_site, encode_integer_value, encode_len, encode_scalar_type,
    };
    use super::scalar_evaluation::ScalarConstantValue;
    use super::{
        BlockId, EdgeId, IntegerValue, MachineId, OptimizationUnitIdentity,
        ScalarConstantFactIdentity, ScalarType, ValueDefinition,
    };

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
        crate::codec_bytes::push_encoded(&mut canonical, machine.get().to_le_bytes());
        crate::codec_bytes::push_encoded(&mut canonical, definition.value.get().to_le_bytes());
        encode_scalar_type(&mut canonical, definition.scalar_type);
        encode_definition_site(&mut canonical, definition.site);
        encode_scalar_constant_value(&mut canonical, constant);
        encode_len(&mut canonical, snapshot.blocks.len());
        for row in &snapshot.blocks {
            crate::codec_bytes::push_encoded(&mut canonical, row.block.get().to_le_bytes());
            canonical.push(u8::from(row.executable));
        }
        encode_len(&mut canonical, snapshot.edges.len());
        for row in &snapshot.edges {
            crate::codec_bytes::push_encoded(&mut canonical, row.source.get().to_le_bytes());
            crate::codec_bytes::push_encoded(&mut canonical, row.edge.get().to_le_bytes());
            crate::codec_bytes::push_encoded(&mut canonical, row.target.get().to_le_bytes());
            canonical.push(match row.state {
                SccpEdgeState::Executable => 1,
                SccpEdgeState::Inexecutable => 2,
                SccpEdgeState::Unknown => 3,
            });
        }
        encode_len(&mut canonical, snapshot.values.len());
        for row in &snapshot.values {
            crate::codec_bytes::push_encoded(
                &mut canonical,
                row.definition.value.get().to_le_bytes(),
            );
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
}

mod cfg_rewrite_plans {
    use super::foundations::NodeLocation;
    use super::{
        BlockId, CanonicalStructuralPathSegment, EdgeId, IntegerValue, MachineId, OperationId,
        OwnershipFrontierFactIdentity, OwnershipFrontierSite, PlaceId, ScalarType,
        StructuralCaseId, StructuralFieldId, ValueId,
    };

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
    pub struct RedundantBlockParameterRewrite {
        pub machine: MachineId,
        pub block: BlockId,
        pub position: u32,
        pub parameter: ValueId,
        pub replacement: ValueId,
        pub scalar_type: ScalarType,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ConstantConditionalRewrite {
        pub location: NodeLocation,
        pub condition: ValueId,
        pub constant: bool,
        pub selected_edge: EdgeId,
        pub rejected_edge: EdgeId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct LinearEmptyBlockRewrite {
        pub predecessor: NodeLocation,
        pub incoming_edge: EdgeId,
        pub empty: NodeLocation,
        pub outgoing_edge: EdgeId,
        pub target: BlockId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PathQualifiedEmptyBlockRewrite {
        pub empty: NodeLocation,
        pub outgoing_edge: EdgeId,
        pub target: BlockId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct AdjacentBlockMergeRewrite {
        pub predecessor: NodeLocation,
        pub incoming_edge: EdgeId,
        pub target: BlockId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct OwnershipFrontierWitnessRow {
        pub site: OwnershipFrontierSite,
        pub fact: OwnershipFrontierFactIdentity,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct OwnershipFrontierWitness {
        pub rows: Vec<OwnershipFrontierWitnessRow>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct NonAdjacentBlockMergeRewrite {
        pub predecessor: NodeLocation,
        pub incoming_edge: EdgeId,
        pub target: BlockId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct SharedJumpFusionRewrite {
        pub predecessor: NodeLocation,
        pub incoming_edge: EdgeId,
        pub target: BlockId,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct UnreachablePrivateMachinesRewrite {
        pub machines: Vec<crate::PrunedMachineCustody>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct SpecializedStateEdgeRow {
        pub incoming_edge: EdgeId,
        pub predecessor: NodeLocation,
        pub parameter: ValueId,
        pub argument: ValueId,
        pub constant: bool,
        pub taken_edge: EdgeId,
        pub rejected_edge: EdgeId,
        pub resolved_target: BlockId,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StateArgumentSpecializationRewrite {
        pub machine: MachineId,
        pub dispatch: BlockId,
        pub edges: Vec<SpecializedStateEdgeRow>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FoldedCaseMembershipRow {
        pub site: NodeLocation,
        pub psi_operation: OperationId,
        pub result: ValueId,
        pub source: PlaceId,
        pub producer: Option<OperationId>,
        pub observed_case: StructuralCaseId,
        pub proven_case: StructuralCaseId,
        pub outcome: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CaseMembershipSpecializationRewrite {
        pub machine: MachineId,
        pub place: PlaceId,
        pub producer: Option<OperationId>,
        pub memberships: Vec<FoldedCaseMembershipRow>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum FoldedFieldValue {
        Boolean(bool),
        Integer(IntegerValue),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ForwardedFieldValue {
        pub initializer: ValueId,
        pub scalar_type: ScalarType,
        pub uses: Vec<NodeLocation>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum FieldValueResolution {
        Constant(FoldedFieldValue),
        Forward(ForwardedFieldValue),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FieldValueRow {
        pub site: NodeLocation,
        pub psi_operation: OperationId,
        pub result: ValueId,
        pub source: PlaceId,
        pub path: Vec<CanonicalStructuralPathSegment>,
        pub field: StructuralFieldId,
        pub producer: Option<OperationId>,
        pub resolution: FieldValueResolution,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FieldValueSpecializationRewrite {
        pub machine: MachineId,
        pub place: PlaceId,
        pub producer: Option<OperationId>,
        pub reads: Vec<FieldValueRow>,
    }
}

mod scalar_rewrite_plans {
    use super::foundations::NodeLocation;
    use super::{BlockId, EdgeId, IntegerType, IntegerValue, OperationId, ScalarType, ValueId};

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
    pub struct DeadScalarNodeRewrite {
        pub location: NodeLocation,
        pub source_operation: OperationId,
        pub result: ValueId,
        pub scalar_type: ScalarType,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TotalScalarIdentityKind {
        WrappingIntegerAddZeroLeft,
        WrappingIntegerAddZeroRight,
        WrappingIntegerSubtractZeroRight,
        WrappingIntegerMultiplyOneLeft,
        WrappingIntegerMultiplyOneRight,
        WrappingIntegerShiftLeftZeroCount,
        WrappingIntegerShiftRightZeroCount,
        WrappingIntegerMultiplyZeroLeft,
        WrappingIntegerMultiplyZeroRight,
        SaturatingIntegerAddZeroLeft,
        SaturatingIntegerAddZeroRight,
        SaturatingIntegerSubtractZeroRight,
        SaturatingIntegerMultiplyOneLeft,
        SaturatingIntegerMultiplyOneRight,
        SaturatingIntegerMultiplyZeroLeft,
        SaturatingIntegerMultiplyZeroRight,
        IntegerBitwiseAndAllOnesLeft,
        IntegerBitwiseAndAllOnesRight,
        IntegerBitwiseOrZeroLeft,
        IntegerBitwiseOrZeroRight,
        IntegerBitwiseXorZeroLeft,
        IntegerBitwiseXorZeroRight,
        IntegerBitwiseAndZeroLeft,
        IntegerBitwiseAndZeroRight,
        IntegerBitwiseOrAllOnesLeft,
        IntegerBitwiseOrAllOnesRight,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TotalScalarIdentityRewrite {
        pub location: NodeLocation,
        pub source_operation: OperationId,
        pub result: ValueId,
        pub replacement: ValueId,
        pub scalar_type: IntegerType,
        pub identity: TotalScalarIdentityKind,
    }

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

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProofCertifiedScalarIdentityRewrite {
        pub location: NodeLocation,
        pub source_operation: OperationId,
        pub result: ValueId,
        pub replacement: ValueId,
        pub scalar_type: IntegerType,
        pub identity: ProofCertifiedScalarIdentityKind,
    }

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

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PhiTranslatedScalarIncoming {
        pub source: BlockId,
        pub edge: EdgeId,
        pub leader: NodeLocation,
        pub leader_operation: OperationId,
        pub leader_result: ValueId,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PhiTranslatedScalarGvnRewrite {
        pub redundant: NodeLocation,
        pub redundant_operation: OperationId,
        pub redundant_result: ValueId,
        pub scalar_type: ScalarType,
        pub parameter_position: u32,
        pub incoming: Vec<PhiTranslatedScalarIncoming>,
    }
}

mod loop_invariant_scalar_motion {
    use semantic_vocabulary::{OperationId, PlaceId, ScalarType, ValueId};

    use super::foundations::{NodeLocation, encode_location};
    use crate::CycleComponentId;
    use crate::{PsiProvenance, WorkSettlement, codec_bytes::push_encoded};
    use optimization_core::{
        AnalysisInvalidationSet, AnalysisSet, OptimizationCandidateIdentity,
        OptimizationFactReference, OptimizationRuleContract, OptimizationRuleIdentity,
        OptimizationSafetyClass, OptimizationUnitIdentity,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum LoopInvariantNodeResult {
        Scalar {
            value: ValueId,
            scalar_type: ScalarType,
        },
        Structural(terminal_psi::StructuralOperationResult),
        LiteralPlace(terminal_psi::StructuralPlaceDeclaration),
        TrivialAffineLocal(terminal_psi::StructuralPlaceDeclaration),
        Unit,
    }

    impl LoopInvariantNodeResult {
        pub const fn scalar_value(&self) -> Option<ValueId> {
            match self {
                Self::Scalar { value, .. } => Some(*value),
                Self::Structural(_)
                | Self::LiteralPlace(_)
                | Self::TrivialAffineLocal(_)
                | Self::Unit => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LoopInvariantScalarNode {
        psi_operation: OperationId,
        result: LoopInvariantNodeResult,
        location: NodeLocation,
        operand_rewrites: Vec<(ValueId, ValueId)>,
        root_rewrite: Option<(PlaceId, PlaceId)>,
        argument_rewrites: Vec<(PlaceId, PlaceId)>,
        provenance: Vec<PsiProvenance>,
        work: Vec<WorkSettlement>,
    }

    impl LoopInvariantScalarNode {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            psi_operation: OperationId,
            result: LoopInvariantNodeResult,
            location: NodeLocation,
            mut operand_rewrites: Vec<(ValueId, ValueId)>,
            root_rewrite: Option<(PlaceId, PlaceId)>,
            argument_rewrites: Vec<(PlaceId, PlaceId)>,
            provenance: Vec<PsiProvenance>,
            work: Vec<WorkSettlement>,
        ) -> Self {
            operand_rewrites.sort_unstable();
            Self {
                psi_operation,
                result,
                location,
                operand_rewrites,
                root_rewrite,
                argument_rewrites,
                provenance,
                work,
            }
        }

        pub const fn psi_operation(&self) -> OperationId {
            self.psi_operation
        }

        pub const fn result(&self) -> &LoopInvariantNodeResult {
            &self.result
        }

        pub const fn location(&self) -> NodeLocation {
            self.location
        }

        pub fn operand_rewrites(&self) -> &[(ValueId, ValueId)] {
            &self.operand_rewrites
        }

        pub const fn root_rewrite(&self) -> Option<(PlaceId, PlaceId)> {
            self.root_rewrite
        }

        pub fn argument_rewrites(&self) -> &[(PlaceId, PlaceId)] {
            &self.argument_rewrites
        }

        pub fn provenance(&self) -> &[PsiProvenance] {
            &self.provenance
        }

        pub fn work(&self) -> &[WorkSettlement] {
            &self.work
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LoopInvariantScalarRelocation {
        node: LoopInvariantScalarNode,
        destination: NodeLocation,
    }

    impl LoopInvariantScalarRelocation {
        pub const fn new(node: LoopInvariantScalarNode, destination: NodeLocation) -> Self {
            Self { node, destination }
        }

        pub const fn node(&self) -> &LoopInvariantScalarNode {
            &self.node
        }

        pub const fn destination(&self) -> NodeLocation {
            self.destination
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LoopInvariantScalarMotionCandidate {
        identity: OptimizationCandidateIdentity,
        input: OptimizationUnitIdentity,
        output: OptimizationUnitIdentity,
        rule: OptimizationRuleIdentity,
        required_analyses: AnalysisSet,
        invalidated_analyses: AnalysisInvalidationSet,
        safety_class: OptimizationSafetyClass,
        component: CycleComponentId,
        relocations: Vec<LoopInvariantScalarRelocation>,
    }

    impl LoopInvariantScalarMotionCandidate {
        pub fn new(
            contract: OptimizationRuleContract,
            input: OptimizationUnitIdentity,
            output: OptimizationUnitIdentity,
            component: CycleComponentId,
            relocations: Vec<LoopInvariantScalarRelocation>,
        ) -> Self {
            let identity = candidate_identity(contract, input, output, &component, &relocations);
            Self {
                identity,
                input,
                output,
                rule: contract.identity(),
                required_analyses: contract.required_analyses(),
                invalidated_analyses: contract.invalidated_analyses(),
                safety_class: contract.safety_class(),
                component,
                relocations,
            }
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

        pub const fn required_analyses(&self) -> AnalysisSet {
            self.required_analyses
        }

        pub const fn invalidated_analyses(&self) -> AnalysisInvalidationSet {
            self.invalidated_analyses
        }

        pub const fn safety_class(&self) -> OptimizationSafetyClass {
            self.safety_class
        }

        pub const fn component(&self) -> &CycleComponentId {
            &self.component
        }

        pub fn relocations(&self) -> &[LoopInvariantScalarRelocation] {
            &self.relocations
        }

        pub fn consumed_facts(&self) -> Vec<OptimizationFactReference> {
            Vec::new()
        }

        pub fn predicted_cost_delta(&self) -> i64 {
            -i64::try_from(self.relocations.len()).expect("relocation count fits i64")
        }
    }

    fn candidate_identity(
        contract: OptimizationRuleContract,
        input: OptimizationUnitIdentity,
        output: OptimizationUnitIdentity,
        component: &CycleComponentId,
        relocations: &[LoopInvariantScalarRelocation],
    ) -> OptimizationCandidateIdentity {
        let mut canonical = b"omega.psi.loop-invariant-scalar-motion-candidate.v2".to_vec();
        canonical.extend_from_slice(&contract.encode());
        canonical.extend_from_slice(&input.bytes());
        canonical.extend_from_slice(&output.bytes());
        push_encoded(&mut canonical, component.machine.get().to_le_bytes());
        canonical.extend_from_slice(
            &u64::try_from(component.internal_edges.len())
                .expect("component edge count fits u64")
                .to_le_bytes(),
        );
        for edge in &component.internal_edges {
            push_encoded(&mut canonical, edge.edge.get().to_le_bytes());
            push_encoded(&mut canonical, edge.source.get().to_le_bytes());
            push_encoded(&mut canonical, edge.target.get().to_le_bytes());
        }
        canonical.extend_from_slice(
            &u64::try_from(relocations.len())
                .expect("relocation count fits u64")
                .to_le_bytes(),
        );
        for relocation in relocations {
            push_encoded(
                &mut canonical,
                relocation.node.psi_operation.get().to_le_bytes(),
            );
            match &relocation.node.result {
                LoopInvariantNodeResult::Scalar { value, .. } => {
                    canonical.push(0);
                    push_encoded(&mut canonical, value.get().to_le_bytes());
                }
                LoopInvariantNodeResult::Structural(result) => {
                    canonical.push(1);
                    push_encoded(&mut canonical, result.place.get().to_le_bytes());
                    push_encoded(&mut canonical, result.structural_type.get().to_le_bytes());
                    canonical.push(match result.multiplicity {
                        terminal_psi::StructuralMultiplicity::Unrestricted => 0,
                        terminal_psi::StructuralMultiplicity::Affine => 1,
                        terminal_psi::StructuralMultiplicity::Linear => 2,
                    });
                    for count in [
                        result.qualifications.len(),
                        result.projected_qualifications.len(),
                        result.claims.len(),
                    ] {
                        canonical.extend_from_slice(
                            &u64::try_from(count)
                                .expect("result roster length fits u64")
                                .to_le_bytes(),
                        );
                    }
                }
                LoopInvariantNodeResult::LiteralPlace(place) => {
                    canonical.push(2);
                    push_encoded(&mut canonical, place.id.get().to_le_bytes());
                    match place.kind {
                        semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                            declaration_ordinal,
                            structural_type,
                        } => {
                            canonical.push(0);
                            push_encoded(&mut canonical, declaration_ordinal.to_le_bytes());
                            push_encoded(&mut canonical, structural_type.get().to_le_bytes());
                        }
                        _ => canonical.push(1),
                    }
                }
                LoopInvariantNodeResult::TrivialAffineLocal(place) => {
                    canonical.push(4);
                    push_encoded(&mut canonical, place.id.get().to_le_bytes());
                    match place.kind {
                        semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                            declaration_ordinal,
                            structural_type,
                            construction,
                        } => {
                            canonical.push(0);
                            push_encoded(&mut canonical, declaration_ordinal.to_le_bytes());
                            push_encoded(&mut canonical, structural_type.get().to_le_bytes());
                            match construction {
                                Some(element) => {
                                    canonical.push(1);
                                    canonical.extend_from_slice(
                                        &element.root_structural_type.get().to_le_bytes(),
                                    );
                                    push_encoded(&mut canonical, element.index.to_le_bytes());
                                }
                                None => canonical.push(0),
                            }
                        }
                        _ => canonical.push(1),
                    }
                }
                LoopInvariantNodeResult::Unit => {
                    canonical.push(3);
                }
            }
            encode_location(&mut canonical, relocation.node.location);
            encode_location(&mut canonical, relocation.destination);
            canonical.extend_from_slice(
                &u64::try_from(relocation.node.operand_rewrites.len())
                    .expect("operand rewrite count fits u64")
                    .to_le_bytes(),
            );
            for (parameter, representative) in &relocation.node.operand_rewrites {
                push_encoded(&mut canonical, parameter.get().to_le_bytes());
                push_encoded(&mut canonical, representative.get().to_le_bytes());
            }
            match relocation.node.root_rewrite {
                Some((parameter, representative)) => {
                    canonical.push(1);
                    push_encoded(&mut canonical, parameter.get().to_le_bytes());
                    push_encoded(&mut canonical, representative.get().to_le_bytes());
                }
                None => canonical.push(0),
            }
            canonical.extend_from_slice(
                &u64::try_from(relocation.node.argument_rewrites.len())
                    .expect("argument rewrite count fits u64")
                    .to_le_bytes(),
            );
            for (parameter, root) in &relocation.node.argument_rewrites {
                push_encoded(&mut canonical, parameter.get().to_le_bytes());
                push_encoded(&mut canonical, root.get().to_le_bytes());
            }
        }
        OptimizationCandidateIdentity::from_canonical_bytes(&canonical)
    }
}

mod contracts {
    use super::cfg_rewrite_plans::{
        AdjacentBlockMergeRewrite, CaseMembershipSpecializationRewrite, ConstantConditionalRewrite,
        FieldValueSpecializationRewrite, LinearEmptyBlockRewrite, NonAdjacentBlockMergeRewrite,
        OwnershipFrontierWitness, PathQualifiedEmptyBlockRewrite, RedundantBlockParameterRewrite,
        RedundantBlockParameterWitness, SharedJumpFusionRewrite,
        StateArgumentSpecializationRewrite, UnreachablePrivateMachinesRewrite,
    };
    use super::foundations::{NodeLocation, ProvenanceRewrite, ScalarSubstitution};
    use super::scalar_evaluation::ScalarEvaluationWitness;
    use super::scalar_rewrite_plans::{
        BooleanConstantRewrite, DeadScalarNodeRewrite, DominatingScalarCommonSubexpressionRewrite,
        IntegerConstantRewrite, LocalScalarCommonSubexpressionRewrite,
        PhiTranslatedScalarGvnRewrite, ProofCertifiedScalarIdentityRewrite,
        TotalScalarIdentityRewrite,
    };
    use super::{
        AcceptedObligationFactIdentity, AnalysisInvalidationSet, AnalysisSet, BlockId, MachineId,
        OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationSafetyClass,
        OptimizationUnitIdentity, ScalarConstantFactIdentity,
    };

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
        EliminateTotalScalarIdentity(TotalScalarIdentityRewrite),
        SpecializeStateArgument(StateArgumentSpecializationRewrite),
        SpecializeCaseMembership(CaseMembershipSpecializationRewrite),
        SpecializeFieldValue(FieldValueSpecializationRewrite),
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
    pub(in crate::rewrite) enum PsiRewriteWitness {
        ScalarEvaluation(ScalarEvaluationWitness),
        RedundantBlockParameter(RedundantBlockParameterWitness),
        AcceptedObligation(AcceptedObligationFactIdentity),
        ProofCertifiedScalarIdentity {
            constant_fact: ScalarConstantFactIdentity,
            obligation_fact: AcceptedObligationFactIdentity,
        },
        TotalScalarIdentity {
            constant_fact: ScalarConstantFactIdentity,
        },
        OwnershipFrontiers(OwnershipFrontierWitness),
        StructuralIdentity,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PsiRewriteCandidate {
        pub(in crate::rewrite) identity: OptimizationCandidateIdentity,
        pub(in crate::rewrite) input: OptimizationUnitIdentity,
        pub(in crate::rewrite) rule: OptimizationRuleIdentity,
        pub(in crate::rewrite) decision_point: PsiRewriteDecisionPoint,
        pub(in crate::rewrite) affected_blocks: Vec<BlockId>,
        pub(in crate::rewrite) required_analyses: AnalysisSet,
        pub(in crate::rewrite) invalidated_analyses: AnalysisInvalidationSet,
        pub(in crate::rewrite) safety_class: OptimizationSafetyClass,
        pub(in crate::rewrite) substitutions: Vec<ScalarSubstitution>,
        pub(in crate::rewrite) provenance: Vec<ProvenanceRewrite>,
        pub(in crate::rewrite) witness: PsiRewriteWitness,
        pub(in crate::rewrite) predicted_cost_delta: i64,
        pub(in crate::rewrite) patch: PsiRewritePatch,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PsiRewriteCandidateError {
        EmptyAffectedRegion,
        NonCanonicalAffectedRegion,
        DecisionPointOutsideRegion,
        NonCanonicalSubstitutions,
        EmptyProvenanceSource,
        NonCanonicalProvenance,
        WorkProvenanceMismatch,
        PatchDecisionPointMismatch,
        EmptyIncomingBindings,
        NonCanonicalIncomingBindings,
        NonCanonicalOwnershipFrontierWitness,
        BlockParameterSubstitutionMismatch,
        ProofWitnessSafetyMismatch,
    }

    optimization_core::invalid_error_display!(PsiRewriteCandidateError, "Psi rewrite candidate");
}

mod declarations {
    use super::loop_invariant_scalar_motion::LoopInvariantScalarMotionCandidate;
    use super::{
        OptimizationCandidateIdentity, OptimizationFactReference, OptimizationRuleIdentity,
        OptimizationSafetyClass, OptimizationUnitIdentity, ProvenanceRewrite, PsiRewriteCandidate,
    };
    use crate::PrunedMachineCustody;
    use optimization_core::{AnalysisInvalidationSet, AnalysisSet};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum PsiCandidateDeclaration {
        Rewrite(PsiRewriteCandidate),
        LoopInvariantScalarMotion(LoopInvariantScalarMotionCandidate),
    }

    impl PsiCandidateDeclaration {
        pub const fn identity(&self) -> OptimizationCandidateIdentity {
            match self {
                Self::Rewrite(candidate) => candidate.identity(),
                Self::LoopInvariantScalarMotion(candidate) => candidate.identity(),
            }
        }

        pub const fn input(&self) -> OptimizationUnitIdentity {
            match self {
                Self::Rewrite(candidate) => candidate.input(),
                Self::LoopInvariantScalarMotion(candidate) => candidate.input(),
            }
        }

        pub const fn rule(&self) -> OptimizationRuleIdentity {
            match self {
                Self::Rewrite(candidate) => candidate.rule(),
                Self::LoopInvariantScalarMotion(candidate) => candidate.rule(),
            }
        }

        pub const fn required_analyses(&self) -> AnalysisSet {
            match self {
                Self::Rewrite(candidate) => candidate.required_analyses(),
                Self::LoopInvariantScalarMotion(candidate) => candidate.required_analyses(),
            }
        }

        pub const fn invalidated_analyses(&self) -> AnalysisInvalidationSet {
            match self {
                Self::Rewrite(candidate) => candidate.invalidated_analyses(),
                Self::LoopInvariantScalarMotion(candidate) => candidate.invalidated_analyses(),
            }
        }

        pub const fn safety_class(&self) -> OptimizationSafetyClass {
            match self {
                Self::Rewrite(candidate) => candidate.safety_class(),
                Self::LoopInvariantScalarMotion(candidate) => candidate.safety_class(),
            }
        }

        pub fn consumed_facts(&self) -> Vec<OptimizationFactReference> {
            match self {
                Self::Rewrite(candidate) => candidate.consumed_facts(),
                Self::LoopInvariantScalarMotion(candidate) => candidate.consumed_facts(),
            }
        }

        pub fn predicted_cost_delta(&self) -> i64 {
            match self {
                Self::Rewrite(candidate) => candidate.predicted_cost_delta(),
                Self::LoopInvariantScalarMotion(candidate) => candidate.predicted_cost_delta(),
            }
        }

        pub fn provenance(&self) -> &[ProvenanceRewrite] {
            match self {
                Self::Rewrite(candidate) => candidate.provenance(),
                Self::LoopInvariantScalarMotion(_) => &[],
            }
        }

        pub fn pruned_machine_custody(&self) -> &[PrunedMachineCustody] {
            match self {
                Self::Rewrite(candidate) => candidate.patch_ref().pruned_machine_custody(),
                Self::LoopInvariantScalarMotion(_) => &[],
            }
        }

        pub const fn rewrite(&self) -> Option<&PsiRewriteCandidate> {
            match self {
                Self::Rewrite(candidate) => Some(candidate),
                Self::LoopInvariantScalarMotion(_) => None,
            }
        }

        pub const fn loop_invariant_scalar_motion(
            &self,
        ) -> Option<&LoopInvariantScalarMotionCandidate> {
            match self {
                Self::Rewrite(_) => None,
                Self::LoopInvariantScalarMotion(candidate) => Some(candidate),
            }
        }
    }
}

mod codec {
    use super::canonical_encoding::{
        encode_integer_type, encode_integer_value, encode_len, encode_scalar_type,
    };
    use super::{
        BlockId, CanonicalStructuralPathSegment, OptimizationRuleContract,
        OptimizationUnitIdentity, PsiProvenance,
    };
    use super::{
        FieldValueResolution, FoldedFieldValue, NodeLocation, ProofCertifiedScalarIdentityKind,
        ProvenanceRewrite, PsiRealizationSite, PsiRewriteDecisionPoint, PsiRewritePatch,
        PsiRewriteWitness, ScalarEvaluationWitness, ScalarSubstitution, TotalScalarIdentityKind,
    };
    use crate::ownership::encode_ownership_frontier_site;

    macro_rules! push_bytes {
        ($bytes:expr, $value:expr) => {
            crate::codec_bytes::push_encoded($bytes, $value)
        };
    }

    macro_rules! encode_scalar_common_subexpression {
        ($bytes:expr, $patch:expr) => {{
            encode_location($bytes, $patch.leader);
            encode_location($bytes, $patch.redundant);
            crate::codec_bytes::push_encoded($bytes, $patch.leader_operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(
                $bytes,
                $patch.redundant_operation.get().to_le_bytes(),
            );
            crate::codec_bytes::push_encoded($bytes, $patch.leader_result.get().to_le_bytes());
            crate::codec_bytes::push_encoded($bytes, $patch.redundant_result.get().to_le_bytes());
            encode_scalar_type($bytes, $patch.scalar_type);
        }};
    }

    macro_rules! encode_specialization_site {
        ($bytes:expr, $patch:expr) => {{
            crate::codec_bytes::push_encoded($bytes, $patch.machine.get().to_le_bytes());
            crate::codec_bytes::push_encoded($bytes, $patch.place.get().to_le_bytes());
            let producer = $patch.producer.map(|producer| producer.get()).unwrap_or(0);
            $bytes.extend_from_slice(&producer.to_le_bytes());
        }};
    }

    macro_rules! encode_rewrite_site {
        ($bytes:expr, $patch:expr) => {{
            encode_location($bytes, $patch.location);
            crate::codec_bytes::push_encoded($bytes, $patch.source_operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded($bytes, $patch.result.get().to_le_bytes());
        }};
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode_candidate(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        decision_point: &PsiRewriteDecisionPoint,
        affected_blocks: &[BlockId],
        substitutions: &[ScalarSubstitution],
        provenance: &[ProvenanceRewrite],
        witness: &PsiRewriteWitness,
        predicted_cost_delta: i64,
        patch: &PsiRewritePatch,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"omega.psi-rewrite-candidate.v24\0");
        bytes.extend_from_slice(&input.bytes());
        bytes.extend_from_slice(&contract.encode());
        match decision_point {
            PsiRewriteDecisionPoint::Node(location) => {
                bytes.push(1);
                encode_location(&mut bytes, *location);
            }
            PsiRewriteDecisionPoint::MachineSet(machines) => {
                bytes.push(2);
                encode_len(&mut bytes, machines.len());
                for machine in machines {
                    crate::codec_bytes::push_encoded(&mut bytes, machine.get().to_le_bytes());
                }
            }
        }
        encode_len(&mut bytes, affected_blocks.len());
        for block in affected_blocks {
            crate::codec_bytes::push_encoded(&mut bytes, block.get().to_le_bytes());
        }
        encode_len(&mut bytes, substitutions.len());
        for substitution in substitutions {
            crate::codec_bytes::push_encoded(&mut bytes, substitution.from.get().to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, substitution.to.get().to_le_bytes());
            encode_scalar_type(&mut bytes, substitution.scalar_type);
        }
        encode_len(&mut bytes, provenance.len());
        for row in provenance {
            encode_realization_site(&mut bytes, row.input);
            bytes.push(row.disposition.canonical_tag());
            encode_realization_site(&mut bytes, row.disposition.site());
            encode_len(&mut bytes, row.sources.len());
            for source in &row.sources {
                match source {
                    PsiProvenance::Operation(operation) => {
                        bytes.push(1);
                        crate::codec_bytes::push_encoded(&mut bytes, operation.get().to_le_bytes());
                    }
                    PsiProvenance::Edge(edge) => {
                        bytes.push(2);
                        crate::codec_bytes::push_encoded(&mut bytes, edge.get().to_le_bytes());
                    }
                }
            }
            encode_len(&mut bytes, row.work.len());
            for settlement in &row.work {
                match settlement.site {
                    PsiProvenance::Operation(operation) => {
                        bytes.push(1);
                        crate::codec_bytes::push_encoded(&mut bytes, operation.get().to_le_bytes());
                    }
                    PsiProvenance::Edge(edge) => {
                        bytes.push(2);
                        crate::codec_bytes::push_encoded(&mut bytes, edge.get().to_le_bytes());
                    }
                }
                crate::codec_bytes::push_encoded(&mut bytes, settlement.units.to_le_bytes());
            }
        }
        match witness {
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Unary {
                operand_fact,
            }) => {
                bytes.extend_from_slice(&[1, 1]);
                bytes.extend_from_slice(&operand_fact.bytes());
            }
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Binary {
                left_fact,
                right_fact,
            }) => {
                bytes.extend_from_slice(&[1, 2]);
                bytes.extend_from_slice(&left_fact.bytes());
                bytes.extend_from_slice(&right_fact.bytes());
            }
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::ProofCertifiedUnary {
                operand_fact,
                obligation_fact,
            }) => {
                bytes.extend_from_slice(&[1, 3]);
                bytes.extend_from_slice(&operand_fact.bytes());
                bytes.extend_from_slice(&obligation_fact.bytes());
            }
            PsiRewriteWitness::ScalarEvaluation(
                ScalarEvaluationWitness::ProofCertifiedBinary {
                    left_fact,
                    right_fact,
                    obligation_fact,
                },
            ) => {
                bytes.extend_from_slice(&[1, 4]);
                bytes.extend_from_slice(&left_fact.bytes());
                bytes.extend_from_slice(&right_fact.bytes());
                bytes.extend_from_slice(&obligation_fact.bytes());
            }
            PsiRewriteWitness::ScalarEvaluation(
                ScalarEvaluationWitness::RangeAgainstConstant {
                    range_fact,
                    constant_fact,
                },
            ) => {
                bytes.extend_from_slice(&[1, 5]);
                bytes.extend_from_slice(&range_fact.bytes());
                bytes.extend_from_slice(&constant_fact.bytes());
            }
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::RangeAgainstRange {
                left_range_fact,
                right_range_fact,
            }) => {
                bytes.extend_from_slice(&[1, 6]);
                bytes.extend_from_slice(&left_range_fact.bytes());
                bytes.extend_from_slice(&right_range_fact.bytes());
            }
            PsiRewriteWitness::RedundantBlockParameter(witness) => {
                bytes.push(2);
                encode_len(&mut bytes, witness.incoming.len());
                for incoming in &witness.incoming {
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        incoming.source.get().to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(&mut bytes, incoming.edge.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        incoming.argument.get().to_le_bytes(),
                    );
                }
            }
            PsiRewriteWitness::StructuralIdentity => bytes.push(3),
            PsiRewriteWitness::AcceptedObligation(identity) => {
                bytes.push(4);
                bytes.extend_from_slice(&identity.bytes());
            }
            PsiRewriteWitness::ProofCertifiedScalarIdentity {
                constant_fact,
                obligation_fact,
            } => {
                bytes.push(5);
                bytes.extend_from_slice(&constant_fact.bytes());
                bytes.extend_from_slice(&obligation_fact.bytes());
            }
            PsiRewriteWitness::OwnershipFrontiers(witness) => {
                bytes.push(6);
                encode_len(&mut bytes, witness.rows.len());
                for row in &witness.rows {
                    encode_ownership_frontier_site(&mut bytes, row.site);
                    bytes.extend_from_slice(&row.fact.bytes());
                }
            }
            PsiRewriteWitness::TotalScalarIdentity { constant_fact } => {
                bytes.push(7);
                bytes.extend_from_slice(&constant_fact.bytes());
            }
        }
        crate::codec_bytes::push_encoded(&mut bytes, predicted_cost_delta.to_le_bytes());
        match patch {
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) => {
                bytes.push(1);
                encode_rewrite_site!(&mut bytes, patch);
                encode_integer_type(&mut bytes, patch.scalar_type);
                encode_integer_value(&mut bytes, patch.constant);
            }
            PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) => {
                bytes.push(2);
                encode_rewrite_site!(&mut bytes, patch);
                bytes.push(u8::from(patch.constant));
            }
            PsiRewritePatch::RemoveRedundantBlockParameter(patch) => {
                bytes.push(3);
                crate::codec_bytes::push_encoded(&mut bytes, patch.machine.get().to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, patch.block.get().to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, patch.position.to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, patch.parameter.get().to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, patch.replacement.get().to_le_bytes());
                encode_scalar_type(&mut bytes, patch.scalar_type);
            }
            PsiRewritePatch::FoldConstantConditional(patch) => {
                bytes.push(4);
                encode_location(&mut bytes, patch.location);
                crate::codec_bytes::push_encoded(&mut bytes, patch.condition.get().to_le_bytes());
                bytes.push(u8::from(patch.constant));
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    patch.selected_edge.get().to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    patch.rejected_edge.get().to_le_bytes(),
                );
            }
            PsiRewritePatch::ThreadLinearEmptyBlock(patch) => {
                bytes.push(5);
                encode_location(&mut bytes, patch.predecessor);
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    patch.incoming_edge.get().to_le_bytes(),
                );
                encode_location(&mut bytes, patch.empty);
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    patch.outgoing_edge.get().to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(&mut bytes, patch.target.get().to_le_bytes());
            }
            PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) => {
                bytes.push(6);
                encode_location(&mut bytes, patch.empty);
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    patch.outgoing_edge.get().to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(&mut bytes, patch.target.get().to_le_bytes());
            }
            PsiRewritePatch::MergeAdjacentBlock(patch) => {
                bytes.push(7);
                encode_location(&mut bytes, patch.predecessor);
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    patch.incoming_edge.get().to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(&mut bytes, patch.target.get().to_le_bytes());
            }
            PsiRewritePatch::PruneUnreachablePrivateMachines(patch) => {
                bytes.push(8);
                encode_len(&mut bytes, patch.machines.len());
                for custody in &patch.machines {
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        custody.machine.get().to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        custody.source_ordinal.to_le_bytes(),
                    );
                }
            }
            PsiRewritePatch::FuseSharedTerminalJump(patch) => {
                bytes.push(9);
                encode_location(&mut bytes, patch.predecessor);
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    patch.incoming_edge.get().to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(&mut bytes, patch.target.get().to_le_bytes());
            }
            PsiRewritePatch::RemoveDeadScalarNode(patch) => {
                bytes.push(10);
                encode_rewrite_site!(&mut bytes, patch);
                encode_scalar_type(&mut bytes, patch.scalar_type);
            }
            PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) => {
                bytes.push(11);
                encode_scalar_common_subexpression!(&mut bytes, patch);
            }
            PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) => {
                bytes.push(12);
                encode_scalar_common_subexpression!(&mut bytes, patch);
            }
            PsiRewritePatch::MergeNonAdjacentBlock(patch) => {
                bytes.push(13);
                encode_location(&mut bytes, patch.predecessor);
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    patch.incoming_edge.get().to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(&mut bytes, patch.target.get().to_le_bytes());
            }
            PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) => {
                bytes.push(14);
                encode_location(&mut bytes, patch.redundant);
                push_bytes!(&mut bytes, patch.redundant_operation.get().to_le_bytes());
                push_bytes!(&mut bytes, patch.redundant_result.get().to_le_bytes());
                encode_scalar_type(&mut bytes, patch.scalar_type);
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    patch.parameter_position.to_le_bytes(),
                );
                encode_len(&mut bytes, patch.incoming.len());
                for incoming in &patch.incoming {
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        incoming.source.get().to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(&mut bytes, incoming.edge.get().to_le_bytes());
                    encode_location(&mut bytes, incoming.leader);
                    push_bytes!(&mut bytes, incoming.leader_operation.get().to_le_bytes());
                    push_bytes!(&mut bytes, incoming.leader_result.get().to_le_bytes());
                }
            }
            PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) => {
                bytes.push(15);
                encode_rewrite_site!(&mut bytes, patch);
                crate::codec_bytes::push_encoded(&mut bytes, patch.replacement.get().to_le_bytes());
                encode_integer_type(&mut bytes, patch.scalar_type);
                bytes.push(match patch.identity {
                    ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft => 1,
                    ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight => 2,
                    ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight => 3,
                    ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft => 4,
                    ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight => 5,
                    ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount => 6,
                    ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount => 7,
                    ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight => 8,
                    ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight => 9,
                    ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight => 10,
                    ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft => 11,
                    ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight => 12,
                    ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft => 13,
                    ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft => 14,
                    ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft => 15,
                    ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft => 16,
                    ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft => 17,
                    ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft => 18,
                    ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue => 19,
                    ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue => 20,
                    ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue => 21,
                });
            }
            PsiRewritePatch::EliminateTotalScalarIdentity(patch) => {
                bytes.push(16);
                encode_rewrite_site!(&mut bytes, patch);
                crate::codec_bytes::push_encoded(&mut bytes, patch.replacement.get().to_le_bytes());
                encode_integer_type(&mut bytes, patch.scalar_type);
                bytes.push(match patch.identity {
                    TotalScalarIdentityKind::WrappingIntegerAddZeroLeft => 1,
                    TotalScalarIdentityKind::WrappingIntegerAddZeroRight => 2,
                    TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight => 3,
                    TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft => 4,
                    TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight => 5,
                    TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount => 6,
                    TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount => 7,
                    TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft => 8,
                    TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight => 9,
                    TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft => 10,
                    TotalScalarIdentityKind::SaturatingIntegerAddZeroRight => 11,
                    TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight => 12,
                    TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft => 13,
                    TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight => 14,
                    TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft => 15,
                    TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight => 16,
                    TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft => 17,
                    TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight => 18,
                    TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft => 19,
                    TotalScalarIdentityKind::IntegerBitwiseOrZeroRight => 20,
                    TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft => 21,
                    TotalScalarIdentityKind::IntegerBitwiseXorZeroRight => 22,
                    TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft => 23,
                    TotalScalarIdentityKind::IntegerBitwiseAndZeroRight => 24,
                    TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft => 25,
                    TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight => 26,
                });
            }
            PsiRewritePatch::SpecializeStateArgument(patch) => {
                bytes.push(17);
                crate::codec_bytes::push_encoded(&mut bytes, patch.machine.get().to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, patch.dispatch.get().to_le_bytes());
                encode_len(&mut bytes, patch.edges.len());
                for row in &patch.edges {
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        row.incoming_edge.get().to_le_bytes(),
                    );
                    encode_location(&mut bytes, row.predecessor);
                    crate::codec_bytes::push_encoded(&mut bytes, row.parameter.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, row.argument.get().to_le_bytes());
                    bytes.push(u8::from(row.constant));
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        row.taken_edge.get().to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        row.rejected_edge.get().to_le_bytes(),
                    );
                    push_bytes!(&mut bytes, row.resolved_target.get().to_le_bytes());
                }
            }
            PsiRewritePatch::SpecializeCaseMembership(patch) => {
                bytes.push(18);
                encode_specialization_site!(&mut bytes, patch);
                encode_len(&mut bytes, patch.memberships.len());
                for row in &patch.memberships {
                    encode_location(&mut bytes, row.site);
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        row.psi_operation.get().to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(&mut bytes, row.result.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, row.source.get().to_le_bytes());
                    bytes.extend_from_slice(
                        &row.producer
                            .map(|producer| producer.get())
                            .unwrap_or(0)
                            .to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        row.observed_case.get().to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        row.proven_case.get().to_le_bytes(),
                    );
                    bytes.push(u8::from(row.outcome));
                }
            }
            PsiRewritePatch::SpecializeFieldValue(patch) => {
                bytes.push(19);
                encode_specialization_site!(&mut bytes, patch);
                encode_len(&mut bytes, patch.reads.len());
                for row in &patch.reads {
                    encode_location(&mut bytes, row.site);
                    crate::codec_bytes::push_encoded(
                        &mut bytes,
                        row.psi_operation.get().to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(&mut bytes, row.result.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, row.source.get().to_le_bytes());
                    encode_len(&mut bytes, row.path.len());
                    for segment in &row.path {
                        match segment {
                            CanonicalStructuralPathSegment::Referent => bytes.push(4),
                            CanonicalStructuralPathSegment::Field(identity) => {
                                bytes.push(1);
                                push_bytes!(&mut bytes, identity.get().to_le_bytes());
                            }
                            CanonicalStructuralPathSegment::FixedIndex(index) => {
                                bytes.push(2);
                                crate::codec_bytes::push_encoded(&mut bytes, index.to_le_bytes());
                            }
                            CanonicalStructuralPathSegment::Case(identity) => {
                                bytes.push(3);
                                push_bytes!(&mut bytes, identity.get().to_le_bytes());
                            }
                        }
                    }
                    crate::codec_bytes::push_encoded(&mut bytes, row.field.get().to_le_bytes());
                    bytes.extend_from_slice(
                        &row.producer
                            .map(|producer| producer.get())
                            .unwrap_or(0)
                            .to_le_bytes(),
                    );
                    match &row.resolution {
                        FieldValueResolution::Constant(FoldedFieldValue::Boolean(constant)) => {
                            bytes.push(1);
                            bytes.push(u8::from(*constant));
                        }
                        FieldValueResolution::Constant(FoldedFieldValue::Integer(constant)) => {
                            bytes.push(2);
                            encode_integer_value(&mut bytes, *constant);
                        }
                        FieldValueResolution::Forward(forwarded) => {
                            bytes.push(3);
                            push_bytes!(&mut bytes, forwarded.initializer.get().to_le_bytes());
                            encode_scalar_type(&mut bytes, forwarded.scalar_type);
                            encode_len(&mut bytes, forwarded.uses.len());
                            for site in &forwarded.uses {
                                encode_location(&mut bytes, *site);
                            }
                        }
                    }
                }
            }
        }
        bytes
    }

    pub(super) fn encode_location(bytes: &mut Vec<u8>, location: NodeLocation) {
        crate::codec_bytes::push_encoded(bytes, location.machine.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, location.block.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, location.node.to_le_bytes());
    }

    pub(super) fn encode_realization_site(bytes: &mut Vec<u8>, site: PsiRealizationSite) {
        match site {
            PsiRealizationSite::Node(location) => {
                bytes.push(1);
                encode_location(bytes, location);
            }
            PsiRealizationSite::Edge { machine, edge } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            }
        }
    }
}

mod candidate {
    #[rustfmt::skip]
macro_rules! structural_identity_constructors {
    ( $( ($name:ident, $patch:ty, $variant:ident) ),+ $(,)? ) => { $(
        pub fn $name(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, provenance: Vec<ProvenanceRewrite>, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, Vec::new(), provenance, PsiRewriteWitness::StructuralIdentity, predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}

    #[rustfmt::skip]
macro_rules! substituted_structural_identity_constructors {
    ( $( ($name:ident, $patch:ty, $variant:ident) ),+ $(,)? ) => { $(
        #[allow(clippy::too_many_arguments)]
        pub fn $name(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, substitutions: Vec<ScalarSubstitution>, provenance: Vec<ProvenanceRewrite>, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, substitutions, provenance, PsiRewriteWitness::StructuralIdentity, predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}

    #[rustfmt::skip]
macro_rules! scalar_evaluation_constructors {
    ( $( ($name:ident, $patch:ty, $variant:ident) ),+ $(,)? ) => { $(
        #[allow(clippy::too_many_arguments)]
        pub fn $name(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, substitutions: Vec<ScalarSubstitution>, provenance: Vec<ProvenanceRewrite>, witness: ScalarEvaluationWitness, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, substitutions, provenance, PsiRewriteWitness::ScalarEvaluation(witness), predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}

    #[rustfmt::skip]
macro_rules! accepted_obligation_constructors {
    ( $( ($name:ident, $patch:ty, $variant:ident) ),+ $(,)? ) => { $(
        #[allow(clippy::too_many_arguments)]
        pub fn $name(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, provenance: Vec<ProvenanceRewrite>, obligation_fact: AcceptedObligationFactIdentity, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, Vec::new(), provenance, PsiRewriteWitness::AcceptedObligation(obligation_fact), predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}

    #[rustfmt::skip]
macro_rules! condition_fact_constructors {
    ( $( ($name:ident, $patch:ty, $variant:ident) ),+ $(,)? ) => { $(
        #[allow(clippy::too_many_arguments)]
        pub fn $name(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, provenance: Vec<ProvenanceRewrite>, condition_fact: ScalarConstantFactIdentity, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, Vec::new(), provenance, PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Unary { operand_fact: condition_fact }), predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}

    #[rustfmt::skip]
macro_rules! redundant_result_pair_constructors {
    ( $( ($plain:ident, $certified:ident, $patch:ty, $variant:ident, $from:ident, $to:ident, $scalar_type:ident) ),+ $(,)? ) => { $(
        pub fn $plain(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, provenance: Vec<ProvenanceRewrite>, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, vec![ScalarSubstitution { from: patch.$from, to: patch.$to, scalar_type: patch.$scalar_type }], provenance, PsiRewriteWitness::StructuralIdentity, predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }

        #[allow(clippy::too_many_arguments)]
        pub fn $certified(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, provenance: Vec<ProvenanceRewrite>, obligation_fact: AcceptedObligationFactIdentity, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, vec![ScalarSubstitution { from: patch.$from, to: patch.$to, scalar_type: patch.$scalar_type }], provenance, PsiRewriteWitness::AcceptedObligation(obligation_fact), predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}

    use super::{
        BlockId, OptimizationCandidateIdentity, OptimizationRuleContract, OptimizationUnitIdentity,
        ProvenanceRewrite, PsiRewriteCandidate, PsiRewriteCandidateError, PsiRewritePatch,
        PsiRewriteWitness, ScalarSubstitution,
    };

    use super::codec::encode_candidate;

    impl PsiRewriteCandidate {
        #[allow(clippy::too_many_arguments)]
        fn new(
            input: OptimizationUnitIdentity,
            contract: OptimizationRuleContract,
            affected_blocks: Vec<BlockId>,
            substitutions: Vec<ScalarSubstitution>,
            provenance: Vec<ProvenanceRewrite>,
            witness: PsiRewriteWitness,
            predicted_cost_delta: i64,
            patch: PsiRewritePatch,
        ) -> Result<Self, PsiRewriteCandidateError> {
            let decision_point = decision_point::derive(&patch)?;
            let location = common_invariants::validate(
                &contract,
                &decision_point,
                &affected_blocks,
                &substitutions,
                &provenance,
                &witness,
            )?;
            patch_invariants::validate(
                location,
                &affected_blocks,
                &substitutions,
                &provenance,
                &witness,
                &patch,
            )?;
            let canonical = encode_candidate(
                input,
                contract,
                &decision_point,
                &affected_blocks,
                &substitutions,
                &provenance,
                &witness,
                predicted_cost_delta,
                &patch,
            );
            let identity = OptimizationCandidateIdentity::from_canonical_bytes(&canonical);
            Ok(Self {
                identity,
                input,
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
    }

    mod access {
        use super::super::{
            AcceptedObligationFactIdentity, AnalysisInvalidationSet, AnalysisSet, BlockId,
            MachineId, NodeLocation, OptimizationCandidateIdentity, OptimizationFactReference,
            OptimizationRuleIdentity, OptimizationSafetyClass, OptimizationUnitIdentity,
            OwnershipFrontierWitness, ProvenanceRewrite, PsiRewriteCandidate,
            PsiRewriteDecisionPoint, PsiRewritePatch, PsiRewriteWitness,
            RedundantBlockParameterWitness, ScalarConstantFactIdentity, ScalarEvaluationWitness,
            ScalarSubstitution,
        };
        impl PsiRewriteCandidate {
            pub const fn identity(&self) -> OptimizationCandidateIdentity {
                self.identity
            }

            pub const fn input(&self) -> OptimizationUnitIdentity {
                self.input
            }

            pub const fn rule(&self) -> OptimizationRuleIdentity {
                self.rule
            }

            pub const fn decision_point(&self) -> &PsiRewriteDecisionPoint {
                &self.decision_point
            }

            pub const fn node_decision_point(&self) -> Option<NodeLocation> {
                match &self.decision_point {
                    PsiRewriteDecisionPoint::Node(location) => Some(*location),
                    PsiRewriteDecisionPoint::MachineSet(_) => None,
                }
            }

            pub fn affected_machines(&self) -> &[MachineId] {
                match &self.decision_point {
                    PsiRewriteDecisionPoint::Node(_) => &[],
                    PsiRewriteDecisionPoint::MachineSet(machines) => machines,
                }
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

            pub const fn scalar_evaluation_witness(&self) -> Option<ScalarEvaluationWitness> {
                match &self.witness {
                    PsiRewriteWitness::ScalarEvaluation(witness) => Some(*witness),
                    PsiRewriteWitness::RedundantBlockParameter(_)
                    | PsiRewriteWitness::AcceptedObligation(_)
                    | PsiRewriteWitness::ProofCertifiedScalarIdentity { .. }
                    | PsiRewriteWitness::TotalScalarIdentity { .. }
                    | PsiRewriteWitness::OwnershipFrontiers(_)
                    | PsiRewriteWitness::StructuralIdentity => None,
                }
            }

            pub fn redundant_block_parameter_witness(
                &self,
            ) -> Option<&RedundantBlockParameterWitness> {
                match &self.witness {
                    PsiRewriteWitness::ScalarEvaluation(_) => None,
                    PsiRewriteWitness::RedundantBlockParameter(witness) => Some(witness),
                    PsiRewriteWitness::AcceptedObligation(_) => None,
                    PsiRewriteWitness::ProofCertifiedScalarIdentity { .. } => None,
                    PsiRewriteWitness::TotalScalarIdentity { .. } => None,
                    PsiRewriteWitness::OwnershipFrontiers(_) => None,
                    PsiRewriteWitness::StructuralIdentity => None,
                }
            }

            pub const fn accepted_obligation_witness(
                &self,
            ) -> Option<AcceptedObligationFactIdentity> {
                match &self.witness {
                    PsiRewriteWitness::AcceptedObligation(identity) => Some(*identity),
                    PsiRewriteWitness::ProofCertifiedScalarIdentity {
                        obligation_fact, ..
                    } => Some(*obligation_fact),
                    PsiRewriteWitness::ScalarEvaluation(_)
                    | PsiRewriteWitness::RedundantBlockParameter(_)
                    | PsiRewriteWitness::TotalScalarIdentity { .. }
                    | PsiRewriteWitness::OwnershipFrontiers(_)
                    | PsiRewriteWitness::StructuralIdentity => None,
                }
            }

            pub const fn proof_certified_scalar_identity_witness(
                &self,
            ) -> Option<(ScalarConstantFactIdentity, AcceptedObligationFactIdentity)> {
                match &self.witness {
                    PsiRewriteWitness::ProofCertifiedScalarIdentity {
                        constant_fact,
                        obligation_fact,
                    } => Some((*constant_fact, *obligation_fact)),
                    PsiRewriteWitness::ScalarEvaluation(_)
                    | PsiRewriteWitness::RedundantBlockParameter(_)
                    | PsiRewriteWitness::AcceptedObligation(_)
                    | PsiRewriteWitness::TotalScalarIdentity { .. }
                    | PsiRewriteWitness::OwnershipFrontiers(_)
                    | PsiRewriteWitness::StructuralIdentity => None,
                }
            }

            pub const fn total_scalar_identity_witness(
                &self,
            ) -> Option<ScalarConstantFactIdentity> {
                match &self.witness {
                    PsiRewriteWitness::TotalScalarIdentity { constant_fact } => {
                        Some(*constant_fact)
                    }
                    _ => None,
                }
            }

            pub fn ownership_frontier_witness(&self) -> Option<&OwnershipFrontierWitness> {
                match &self.witness {
                    PsiRewriteWitness::OwnershipFrontiers(witness) => Some(witness),
                    _ => None,
                }
            }

            pub fn consumed_facts(&self) -> Vec<OptimizationFactReference> {
                let mut facts = match &self.witness {
                    PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Unary {
                        operand_fact,
                    }) => {
                        vec![OptimizationFactReference::ScalarConstant(*operand_fact)]
                    }
                    PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Binary {
                        left_fact,
                        right_fact,
                    }) => vec![
                        OptimizationFactReference::ScalarConstant(*left_fact),
                        OptimizationFactReference::ScalarConstant(*right_fact),
                    ],
                    PsiRewriteWitness::ScalarEvaluation(
                        ScalarEvaluationWitness::ProofCertifiedUnary {
                            operand_fact,
                            obligation_fact,
                        },
                    ) => vec![
                        OptimizationFactReference::ScalarConstant(*operand_fact),
                        OptimizationFactReference::AcceptedObligation(*obligation_fact),
                    ],
                    PsiRewriteWitness::ScalarEvaluation(
                        ScalarEvaluationWitness::ProofCertifiedBinary {
                            left_fact,
                            right_fact,
                            obligation_fact,
                        },
                    ) => vec![
                        OptimizationFactReference::ScalarConstant(*left_fact),
                        OptimizationFactReference::ScalarConstant(*right_fact),
                        OptimizationFactReference::AcceptedObligation(*obligation_fact),
                    ],
                    PsiRewriteWitness::ScalarEvaluation(
                        ScalarEvaluationWitness::RangeAgainstConstant {
                            range_fact,
                            constant_fact,
                        },
                    ) => vec![
                        OptimizationFactReference::ValueRange(*range_fact),
                        OptimizationFactReference::ScalarConstant(*constant_fact),
                    ],
                    PsiRewriteWitness::ScalarEvaluation(
                        ScalarEvaluationWitness::RangeAgainstRange {
                            left_range_fact,
                            right_range_fact,
                        },
                    ) => vec![
                        OptimizationFactReference::ValueRange(*left_range_fact),
                        OptimizationFactReference::ValueRange(*right_range_fact),
                    ],
                    PsiRewriteWitness::AcceptedObligation(identity) => {
                        vec![OptimizationFactReference::AcceptedObligation(*identity)]
                    }
                    PsiRewriteWitness::ProofCertifiedScalarIdentity {
                        constant_fact,
                        obligation_fact,
                    } => vec![
                        OptimizationFactReference::ScalarConstant(*constant_fact),
                        OptimizationFactReference::AcceptedObligation(*obligation_fact),
                    ],
                    PsiRewriteWitness::TotalScalarIdentity { constant_fact } => {
                        vec![OptimizationFactReference::ScalarConstant(*constant_fact)]
                    }
                    PsiRewriteWitness::OwnershipFrontiers(witness) => witness
                        .rows
                        .iter()
                        .map(|row| OptimizationFactReference::OwnershipFrontier(row.fact))
                        .collect(),
                    PsiRewriteWitness::RedundantBlockParameter(_)
                    | PsiRewriteWitness::StructuralIdentity => Vec::new(),
                };
                facts.sort_unstable();
                facts.dedup();
                facts
            }

            pub const fn predicted_cost_delta(&self) -> i64 {
                self.predicted_cost_delta
            }

            pub fn patch(&self) -> PsiRewritePatch {
                self.patch.clone()
            }

            pub const fn patch_ref(&self) -> &PsiRewritePatch {
                &self.patch
            }
        }
    }

    mod common_invariants {
        use super::super::{
            BlockId, NodeLocation, OptimizationRuleContract, OptimizationSafetyClass,
            ProvenanceRewrite, PsiRewriteCandidateError, PsiRewriteDecisionPoint,
            PsiRewriteWitness, ScalarEvaluationWitness, ScalarSubstitution,
        };
        use std::collections::BTreeSet;

        #[allow(clippy::too_many_arguments)]
        pub(super) fn validate(
            contract: &OptimizationRuleContract,
            decision_point: &PsiRewriteDecisionPoint,
            affected_blocks: &[BlockId],
            substitutions: &[ScalarSubstitution],
            provenance: &[ProvenanceRewrite],
            witness: &PsiRewriteWitness,
        ) -> Result<Option<NodeLocation>, PsiRewriteCandidateError> {
            let location = match &decision_point {
                PsiRewriteDecisionPoint::Node(location) => Some(*location),
                PsiRewriteDecisionPoint::MachineSet(_) => None,
            };
            if affected_blocks.is_empty() && location.is_some() {
                return Err(PsiRewriteCandidateError::EmptyAffectedRegion);
            }
            if affected_blocks.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(PsiRewriteCandidateError::NonCanonicalAffectedRegion);
            }
            if location.is_some_and(|location| !affected_blocks.contains(&location.block)) {
                return Err(PsiRewriteCandidateError::DecisionPointOutsideRegion);
            }
            if substitutions.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(PsiRewriteCandidateError::NonCanonicalSubstitutions);
            }
            if provenance.is_empty() || provenance.iter().any(|row| row.sources.is_empty()) {
                return Err(PsiRewriteCandidateError::EmptyProvenanceSource);
            }
            if provenance
                .windows(2)
                .any(|pair| pair[0].canonical_sort_key() >= pair[1].canonical_sort_key())
                || provenance.iter().any(|row| {
                    row.sources.iter().copied().collect::<BTreeSet<_>>().len() != row.sources.len()
                })
            {
                return Err(PsiRewriteCandidateError::NonCanonicalProvenance);
            }
            for group in provenance.chunk_by(|left, right| left.input == right.input) {
                if group.len() > 1
                    && (group.iter().any(|row| !row.disposition.is_realized())
                        || group.iter().skip(1).any(|row| {
                            row.sources != group[0].sources || row.work != group[0].work
                        }))
                {
                    return Err(PsiRewriteCandidateError::NonCanonicalProvenance);
                }
            }
            for row in provenance {
                let sources = row.sources.iter().copied().collect::<BTreeSet<_>>();
                if row.input.machine() != row.disposition.site().machine() {
                    return Err(PsiRewriteCandidateError::NonCanonicalProvenance);
                }
                let work = row
                    .work
                    .iter()
                    .map(|settlement| settlement.site)
                    .collect::<BTreeSet<_>>();
                if work.len() != row.work.len()
                    || work != sources
                    || row.work.iter().any(|settlement| settlement.units == 0)
                {
                    return Err(PsiRewriteCandidateError::WorkProvenanceMismatch);
                }
            }
            if matches!(
                contract.safety_class(),
                OptimizationSafetyClass::ProofCertified
            ) != matches!(
                witness,
                PsiRewriteWitness::ScalarEvaluation(
                    ScalarEvaluationWitness::ProofCertifiedUnary { .. }
                        | ScalarEvaluationWitness::ProofCertifiedBinary { .. }
                        | ScalarEvaluationWitness::RangeAgainstConstant { .. }
                        | ScalarEvaluationWitness::RangeAgainstRange { .. }
                ) | PsiRewriteWitness::AcceptedObligation(_)
                    | PsiRewriteWitness::ProofCertifiedScalarIdentity { .. }
            ) {
                return Err(PsiRewriteCandidateError::ProofWitnessSafetyMismatch);
            }
            Ok(location)
        }
    }

    mod control_flow {
        use super::super::{
            AdjacentBlockMergeRewrite, BlockId, CaseMembershipSpecializationRewrite,
            ConstantConditionalRewrite, FieldValueSpecializationRewrite, LinearEmptyBlockRewrite,
            NonAdjacentBlockMergeRewrite, OptimizationRuleContract, OptimizationUnitIdentity,
            OwnershipFrontierWitness, PathQualifiedEmptyBlockRewrite, ProvenanceRewrite,
            PsiRewriteCandidate, PsiRewriteCandidateError, PsiRewritePatch, PsiRewriteWitness,
            RedundantBlockParameterRewrite, RedundantBlockParameterWitness,
            ScalarConstantFactIdentity, ScalarEvaluationWitness, ScalarSubstitution,
            SharedJumpFusionRewrite, StateArgumentSpecializationRewrite,
            UnreachablePrivateMachinesRewrite,
        };
        impl PsiRewriteCandidate {
            #[rustfmt::skip]
            structural_identity_constructors![
        ( new_linear_empty_block, LinearEmptyBlockRewrite, ThreadLinearEmptyBlock ),
        ( new_path_qualified_empty_block, PathQualifiedEmptyBlockRewrite, ThreadPathQualifiedEmptyBlock ),
        ( new_state_argument_specialization, StateArgumentSpecializationRewrite, SpecializeStateArgument ),
        ( new_case_membership_specialization, CaseMembershipSpecializationRewrite, SpecializeCaseMembership ),
        ( new_unreachable_private_machines, UnreachablePrivateMachinesRewrite, PruneUnreachablePrivateMachines ),
    ];

    #[rustfmt::skip]
            substituted_structural_identity_constructors![
        ( new_non_adjacent_block_merge, NonAdjacentBlockMergeRewrite, MergeNonAdjacentBlock ),
        ( new_shared_jump_fusion, SharedJumpFusionRewrite, FuseSharedTerminalJump ),
        ( new_field_value_specialization, FieldValueSpecializationRewrite, SpecializeFieldValue ),
    ];

    #[rustfmt::skip]
            condition_fact_constructors![( new_constant_conditional, ConstantConditionalRewrite, FoldConstantConditional ),];

            #[allow(clippy::too_many_arguments)]
            pub fn new_redundant_block_parameter(
                input: OptimizationUnitIdentity,
                contract: OptimizationRuleContract,
                affected_blocks: Vec<BlockId>,
                provenance: Vec<ProvenanceRewrite>,
                witness: RedundantBlockParameterWitness,
                predicted_cost_delta: i64,
                patch: RedundantBlockParameterRewrite,
            ) -> Result<Self, PsiRewriteCandidateError> {
                if witness.incoming.is_empty() {
                    return Err(PsiRewriteCandidateError::EmptyIncomingBindings);
                }
                if witness
                    .incoming
                    .windows(2)
                    .any(|pair| (pair[0].edge, pair[0].source) >= (pair[1].edge, pair[1].source))
                {
                    return Err(PsiRewriteCandidateError::NonCanonicalIncomingBindings);
                }
                let substitutions = vec![ScalarSubstitution {
                    from: patch.parameter,
                    to: patch.replacement,
                    scalar_type: patch.scalar_type,
                }];
                Self::new(
                    input,
                    contract,
                    affected_blocks,
                    substitutions,
                    provenance,
                    PsiRewriteWitness::RedundantBlockParameter(witness),
                    predicted_cost_delta,
                    PsiRewritePatch::RemoveRedundantBlockParameter(patch),
                )
            }

            #[allow(clippy::too_many_arguments)]
            pub fn new_adjacent_block_merge(
                input: OptimizationUnitIdentity,
                contract: OptimizationRuleContract,
                affected_blocks: Vec<BlockId>,
                substitutions: Vec<ScalarSubstitution>,
                provenance: Vec<ProvenanceRewrite>,
                ownership_witness: OwnershipFrontierWitness,
                predicted_cost_delta: i64,
                patch: AdjacentBlockMergeRewrite,
            ) -> Result<Self, PsiRewriteCandidateError> {
                if ownership_witness
                    .rows
                    .windows(2)
                    .any(|pair| pair[0].site >= pair[1].site)
                {
                    return Err(PsiRewriteCandidateError::NonCanonicalOwnershipFrontierWitness);
                }
                Self::new(
                    input,
                    contract,
                    affected_blocks,
                    substitutions,
                    provenance,
                    PsiRewriteWitness::OwnershipFrontiers(ownership_witness),
                    predicted_cost_delta,
                    PsiRewritePatch::MergeAdjacentBlock(patch),
                )
            }
        }
    }

    mod decision_point {
        use super::super::{
            NodeLocation, PsiRewriteCandidateError, PsiRewriteDecisionPoint, PsiRewritePatch,
        };
        pub(super) fn derive(
            patch: &PsiRewritePatch,
        ) -> Result<PsiRewriteDecisionPoint, PsiRewriteCandidateError> {
            let decision_point = match &patch {
                PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.location)
                }
                PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.location)
                }
                PsiRewritePatch::RemoveRedundantBlockParameter(patch) => {
                    PsiRewriteDecisionPoint::Node(NodeLocation {
                        machine: patch.machine,
                        block: patch.block,
                        node: 0,
                    })
                }
                PsiRewritePatch::FoldConstantConditional(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.location)
                }
                PsiRewritePatch::ThreadLinearEmptyBlock(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.predecessor)
                }
                PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.empty)
                }
                PsiRewritePatch::MergeAdjacentBlock(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.predecessor)
                }
                PsiRewritePatch::MergeNonAdjacentBlock(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.predecessor)
                }
                PsiRewritePatch::FuseSharedTerminalJump(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.predecessor)
                }
                PsiRewritePatch::RemoveDeadScalarNode(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.location)
                }
                PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.redundant)
                }
                PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.redundant)
                }
                PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.redundant)
                }
                PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.location)
                }
                PsiRewritePatch::EliminateTotalScalarIdentity(patch) => {
                    PsiRewriteDecisionPoint::Node(patch.location)
                }
                PsiRewritePatch::SpecializeStateArgument(patch) => {
                    PsiRewriteDecisionPoint::Node(NodeLocation {
                        machine: patch.machine,
                        block: patch.dispatch,
                        node: 0,
                    })
                }
                PsiRewritePatch::SpecializeCaseMembership(patch) => {
                    let Some(first) = patch.memberships.first() else {
                        return Err(PsiRewriteCandidateError::EmptyAffectedRegion);
                    };
                    if first.site.machine != patch.machine {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                    PsiRewriteDecisionPoint::Node(first.site)
                }
                PsiRewritePatch::SpecializeFieldValue(patch) => {
                    let Some(first) = patch.reads.first() else {
                        return Err(PsiRewriteCandidateError::EmptyAffectedRegion);
                    };
                    if first.site.machine != patch.machine {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                    PsiRewriteDecisionPoint::Node(first.site)
                }
                PsiRewritePatch::PruneUnreachablePrivateMachines(patch) => {
                    if patch.machines.is_empty()
                        || patch.machines.windows(2).any(|pair| pair[0] >= pair[1])
                    {
                        return Err(PsiRewriteCandidateError::NonCanonicalAffectedRegion);
                    }
                    let machines = patch
                        .machines
                        .iter()
                        .map(|row| row.machine)
                        .collect::<Vec<_>>();
                    if machines.windows(2).any(|pair| pair[0] >= pair[1]) {
                        return Err(PsiRewriteCandidateError::NonCanonicalAffectedRegion);
                    }
                    PsiRewriteDecisionPoint::MachineSet(machines)
                }
            };
            Ok(decision_point)
        }
    }

    mod patch_invariants {
        use super::super::{
            BlockId, FieldValueResolution, NodeLocation, ProvenanceDisposition, ProvenanceRewrite,
            PsiRealizationSite, PsiRewriteCandidateError, PsiRewritePatch, PsiRewriteWitness,
            ScalarSubstitution, ScalarType,
        };
        use semantic_vocabulary::EdgeId;
        use std::collections::BTreeSet;

        pub(super) fn validate(
            location: Option<NodeLocation>,
            affected_blocks: &[BlockId],
            substitutions: &[ScalarSubstitution],
            provenance: &[ProvenanceRewrite],
            witness: &PsiRewriteWitness,
            patch: &PsiRewritePatch,
        ) -> Result<(), PsiRewriteCandidateError> {
            match &patch {
                PsiRewritePatch::ReplaceIntegerOperationWithConstant(_)
                | PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
                    if provenance.iter().any(|row| {
                        row.disposition
                            != ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                                location.unwrap(),
                            ))
                            || row.input != PsiRealizationSite::Node(location.unwrap())
                    }) {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::RemoveRedundantBlockParameter(patch) => {
                    if provenance.is_empty()
                        || !provenance_realized_within(patch.machine, affected_blocks, provenance)
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                    if substitutions
                        != [ScalarSubstitution {
                            from: patch.parameter,
                            to: patch.replacement,
                            scalar_type: patch.scalar_type,
                        }]
                    {
                        return Err(PsiRewriteCandidateError::BlockParameterSubstitutionMismatch);
                    }
                }
                PsiRewritePatch::FoldConstantConditional(patch) => {
                    let selected = PsiRealizationSite::Edge {
                        machine: location.unwrap().machine,
                        edge: patch.selected_edge,
                    };
                    let rejected = PsiRealizationSite::Edge {
                        machine: location.unwrap().machine,
                        edge: patch.rejected_edge,
                    };
                    if provenance
                        .iter()
                        .filter(|row| {
                            row.input == selected
                                && row.disposition == ProvenanceDisposition::RealizedAt(selected)
                        })
                        .count()
                        != 1
                        || provenance
                            .iter()
                            .filter(|row| {
                                row.input == rejected
                                    && row.disposition
                                        == ProvenanceDisposition::ProvenUnreachableAt(rejected)
                            })
                            .count()
                            != 1
                        || !provenance.iter().any(|row| {
                            matches!(
                                row.disposition,
                                ProvenanceDisposition::ProvenUnreachableAt(_)
                            )
                        })
                        || !substitutions.is_empty()
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::ThreadLinearEmptyBlock(patch) => {
                    let incoming = PsiRealizationSite::Edge {
                        machine: patch.predecessor.machine,
                        edge: patch.incoming_edge,
                    };
                    let outgoing = PsiRealizationSite::Edge {
                        machine: patch.predecessor.machine,
                        edge: patch.outgoing_edge,
                    };
                    if patch.empty.node != 0
                        || patch.empty.machine != patch.predecessor.machine
                        || !affected_blocks.contains(&patch.empty.block)
                        || !provenance_realized_within(
                            patch.predecessor.machine,
                            affected_blocks,
                            provenance,
                        )
                        || !provenance.iter().any(|row| {
                            row.input == incoming
                                && row.disposition == ProvenanceDisposition::RealizedAt(incoming)
                        })
                        || !provenance.iter().any(|row| {
                            row.input == outgoing
                                && row.disposition == ProvenanceDisposition::RealizedAt(incoming)
                        })
                        || !substitutions.is_empty()
                        || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) => {
                    let outgoing = PsiRealizationSite::Edge {
                        machine: patch.empty.machine,
                        edge: patch.outgoing_edge,
                    };
                    let fanout = provenance
                        .iter()
                        .filter(|row| row.input == outgoing && row.disposition.is_realized())
                        .count();
                    if patch.empty.node != 0
                        || !affected_blocks.contains(&patch.empty.block)
                        || fanout == 0
                        || !provenance_realized_within(
                            patch.empty.machine,
                            affected_blocks,
                            provenance,
                        )
                        || !substitutions.is_empty()
                        || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::MergeAdjacentBlock(patch) => {
                    if !incoming_realization_consistent(
                        patch.predecessor,
                        patch.incoming_edge,
                        patch.target,
                        affected_blocks,
                        provenance,
                    ) || !matches!(witness, PsiRewriteWitness::OwnershipFrontiers(_))
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::MergeNonAdjacentBlock(patch) => {
                    if !incoming_realization_consistent(
                        patch.predecessor,
                        patch.incoming_edge,
                        patch.target,
                        affected_blocks,
                        provenance,
                    ) || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::FuseSharedTerminalJump(patch) => {
                    if !incoming_realization_consistent(
                        patch.predecessor,
                        patch.incoming_edge,
                        patch.target,
                        affected_blocks,
                        provenance,
                    ) || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::RemoveDeadScalarNode(patch) => {
                    if !substitutions.is_empty()
                        || !elimination_consistent(
                            patch.location,
                            affected_blocks,
                            provenance,
                            witness,
                        )
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) => {
                    if patch.leader.machine != patch.redundant.machine
                        || patch.leader.block != patch.redundant.block
                        || patch.leader.node >= patch.redundant.node
                        || patch.leader_operation == patch.redundant_operation
                        || patch.leader_result == patch.redundant_result
                        || substitutions
                            != [ScalarSubstitution {
                                from: patch.redundant_result,
                                to: patch.leader_result,
                                scalar_type: patch.scalar_type,
                            }]
                        || !elimination_consistent(
                            patch.redundant,
                            affected_blocks,
                            provenance,
                            witness,
                        )
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) => {
                    if patch.leader.machine != patch.redundant.machine
                        || patch.leader.block == patch.redundant.block
                        || patch.leader_operation == patch.redundant_operation
                        || patch.leader_result == patch.redundant_result
                        || substitutions
                            != [ScalarSubstitution {
                                from: patch.redundant_result,
                                to: patch.leader_result,
                                scalar_type: patch.scalar_type,
                            }]
                        || !elimination_consistent(
                            patch.redundant,
                            affected_blocks,
                            provenance,
                            witness,
                        )
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) => {
                    if !substitutions.is_empty()
                        || patch.incoming.len() < 2
                        || patch.incoming.windows(2).any(|pair| {
                            (pair[0].edge, pair[0].source) >= (pair[1].edge, pair[1].source)
                        })
                        || patch.incoming.iter().any(|incoming| {
                            incoming.leader.machine != patch.redundant.machine
                                || incoming.leader_operation == patch.redundant_operation
                                || incoming.leader_result == patch.redundant_result
                                || !affected_blocks.contains(&incoming.source)
                        })
                        || !elimination_consistent(
                            patch.redundant,
                            affected_blocks,
                            provenance,
                            witness,
                        )
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) => {
                    let input = PsiRealizationSite::Node(patch.location);
                    if substitutions
                        != [ScalarSubstitution {
                            from: patch.result,
                            to: patch.replacement,
                            scalar_type: ScalarType::Integer(patch.scalar_type),
                        }]
                        || patch.result == patch.replacement
                        || !provenance.iter().any(|row| row.input == input)
                        || !provenance_realized_within(
                            patch.location.machine,
                            affected_blocks,
                            provenance,
                        )
                        || !matches!(
                            witness,
                            PsiRewriteWitness::ProofCertifiedScalarIdentity { .. }
                        )
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::EliminateTotalScalarIdentity(patch) => {
                    let input = PsiRealizationSite::Node(patch.location);
                    if substitutions
                        != [ScalarSubstitution {
                            from: patch.result,
                            to: patch.replacement,
                            scalar_type: ScalarType::Integer(patch.scalar_type),
                        }]
                        || patch.result == patch.replacement
                        || !provenance.iter().any(|row| row.input == input)
                        || !provenance_realized_within(
                            patch.location.machine,
                            affected_blocks,
                            provenance,
                        )
                        || !matches!(witness, PsiRewriteWitness::TotalScalarIdentity { .. })
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::SpecializeStateArgument(patch) => {
                    if patch.edges.is_empty()
                        || patch
                            .edges
                            .windows(2)
                            .any(|pair| pair[0].incoming_edge >= pair[1].incoming_edge)
                        || patch.edges.iter().any(|row| {
                            row.predecessor.machine != patch.machine
                                || !affected_blocks.contains(&row.predecessor.block)
                        })
                        || !affected_blocks.contains(&patch.dispatch)
                        || provenance.is_empty()
                        || !provenance_realized_within(patch.machine, affected_blocks, provenance)
                        || !substitutions.is_empty()
                        || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::SpecializeCaseMembership(patch) => {
                    if patch.memberships.is_empty()
                        || patch
                            .memberships
                            .windows(2)
                            .any(|pair| pair[0].site >= pair[1].site)
                        || patch.memberships.iter().any(|row| {
                            row.site.machine != patch.machine
                                || row.source != patch.place
                                || row.outcome != (row.proven_case == row.observed_case)
                                || !affected_blocks.contains(&row.site.block)
                        })
                        || provenance.is_empty()
                        || !provenance_realized_within(patch.machine, affected_blocks, provenance)
                        || !substitutions.is_empty()
                        || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::SpecializeFieldValue(patch) => {
                    let mut expected_substitutions = patch
                        .reads
                        .iter()
                        .filter_map(|row| match &row.resolution {
                            FieldValueResolution::Forward(forwarded) => Some(ScalarSubstitution {
                                from: row.result,
                                to: forwarded.initializer,
                                scalar_type: forwarded.scalar_type,
                            }),
                            FieldValueResolution::Constant(_) => None,
                        })
                        .collect::<Vec<_>>();
                    expected_substitutions.sort();
                    if patch.reads.is_empty()
                        || patch
                            .reads
                            .windows(2)
                            .any(|pair| pair[0].site >= pair[1].site)
                        || patch.reads.iter().any(|row| {
                            row.site.machine != patch.machine
                                || row.source != patch.place
                                || !affected_blocks.contains(&row.site.block)
                                || match &row.resolution {
                                    FieldValueResolution::Constant(_) => false,
                                    FieldValueResolution::Forward(forwarded) => {
                                        forwarded.initializer == row.result
                                            || forwarded
                                                .uses
                                                .windows(2)
                                                .any(|pair| pair[0] >= pair[1])
                                            || forwarded.uses.iter().any(|site| {
                                                site.machine != patch.machine
                                                    || !affected_blocks.contains(&site.block)
                                            })
                                    }
                                }
                        })
                        || provenance.is_empty()
                        || !provenance_realized_within(patch.machine, affected_blocks, provenance)
                        || substitutions != expected_substitutions
                        || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                    {
                        return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                    }
                }
                PsiRewritePatch::PruneUnreachablePrivateMachines(patch) => {
                    let pruned = patch
                        .machines
                        .iter()
                        .map(|row| row.machine)
                        .collect::<BTreeSet<_>>();
                    if !affected_blocks.is_empty()
                    || !substitutions.is_empty()
                    || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                    || provenance.iter().any(|row| {
                        !pruned.contains(&row.input.machine())
                            || !matches!(row.disposition, ProvenanceDisposition::ProvenUnreachableAt(site) if site == row.input)
                    })
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
                }
            }
            Ok(())
        }

        fn incoming_realization_consistent(
            predecessor: NodeLocation,
            incoming_edge: EdgeId,
            target: BlockId,
            affected_blocks: &[BlockId],
            provenance: &[ProvenanceRewrite],
        ) -> bool {
            let incoming = PsiRealizationSite::Edge {
                machine: predecessor.machine,
                edge: incoming_edge,
            };
            affected_blocks.contains(&target)
                && provenance.iter().any(|row| row.input == incoming)
                && provenance_realized_within(predecessor.machine, affected_blocks, provenance)
        }

        fn elimination_consistent(
            location: NodeLocation,
            affected_blocks: &[BlockId],
            provenance: &[ProvenanceRewrite],
            witness: &PsiRewriteWitness,
        ) -> bool {
            provenance
                .iter()
                .any(|row| row.input == PsiRealizationSite::Node(location))
                && provenance_realized_within(location.machine, affected_blocks, provenance)
                && matches!(
                    witness,
                    PsiRewriteWitness::StructuralIdentity
                        | PsiRewriteWitness::AcceptedObligation(_)
                )
        }

        fn provenance_realized_within(
            machine: semantic_vocabulary::MachineId,
            affected_blocks: &[BlockId],
            provenance: &[ProvenanceRewrite],
        ) -> bool {
            provenance.iter().all(|row| {
                let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                    return false;
                };
                site.machine() == machine
                    && site
                        .node()
                        .is_none_or(|location| affected_blocks.contains(&location.block))
            })
        }
    }

    mod scalar {
        use super::super::{
            AcceptedObligationFactIdentity, BlockId, BooleanConstantRewrite, DeadScalarNodeRewrite,
            DominatingScalarCommonSubexpressionRewrite, IntegerConstantRewrite,
            LocalScalarCommonSubexpressionRewrite, OptimizationRuleContract,
            OptimizationUnitIdentity, PhiTranslatedScalarGvnRewrite,
            ProofCertifiedScalarIdentityRewrite, ProvenanceRewrite, PsiRewriteCandidate,
            PsiRewriteCandidateError, PsiRewritePatch, PsiRewriteWitness,
            ScalarConstantFactIdentity, ScalarEvaluationWitness, ScalarSubstitution, ScalarType,
            TotalScalarIdentityRewrite,
        };
        impl PsiRewriteCandidate {
            #[rustfmt::skip]
            structural_identity_constructors![
        ( new_dead_scalar_node, DeadScalarNodeRewrite, RemoveDeadScalarNode ),
        ( new_phi_translated_scalar_common_subexpression, PhiTranslatedScalarGvnRewrite, EliminatePhiTranslatedScalarCommonSubexpression ),
    ];

    #[rustfmt::skip]
            scalar_evaluation_constructors![
        ( new_integer_evaluation, IntegerConstantRewrite, ReplaceIntegerOperationWithConstant ),
        ( new_boolean_evaluation, BooleanConstantRewrite, ReplaceBooleanOperationWithConstant ),
    ];

    #[rustfmt::skip]
            accepted_obligation_constructors![
        ( new_proof_certified_dead_scalar_node, DeadScalarNodeRewrite, RemoveDeadScalarNode ),
        ( new_proof_certified_phi_translated_scalar_common_subexpression, PhiTranslatedScalarGvnRewrite, EliminatePhiTranslatedScalarCommonSubexpression ),
    ];

    #[rustfmt::skip]
            redundant_result_pair_constructors![
        (new_local_scalar_common_subexpression, new_proof_certified_local_scalar_common_subexpression, LocalScalarCommonSubexpressionRewrite, EliminateLocalScalarCommonSubexpression, redundant_result, leader_result, scalar_type),
        (new_dominating_scalar_common_subexpression, new_proof_certified_dominating_scalar_common_subexpression, DominatingScalarCommonSubexpressionRewrite, EliminateDominatedScalarCommonSubexpression, redundant_result, leader_result, scalar_type),
    ];

            #[allow(clippy::too_many_arguments)]
            pub fn new_proof_certified_integer_constant_replacement(
                input: OptimizationUnitIdentity,
                contract: OptimizationRuleContract,
                affected_blocks: Vec<BlockId>,
                provenance: Vec<ProvenanceRewrite>,
                obligation_fact: AcceptedObligationFactIdentity,
                predicted_cost_delta: i64,
                patch: IntegerConstantRewrite,
            ) -> Result<Self, PsiRewriteCandidateError> {
                Self::new(
                    input,
                    contract,
                    affected_blocks,
                    Vec::new(),
                    provenance,
                    PsiRewriteWitness::AcceptedObligation(obligation_fact),
                    predicted_cost_delta,
                    PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch),
                )
            }

            #[allow(clippy::too_many_arguments)]
            pub fn new_literal_proof_certified_integer_constant_replacement(
                input: OptimizationUnitIdentity,
                contract: OptimizationRuleContract,
                affected_blocks: Vec<BlockId>,
                provenance: Vec<ProvenanceRewrite>,
                constant_fact: ScalarConstantFactIdentity,
                obligation_fact: AcceptedObligationFactIdentity,
                predicted_cost_delta: i64,
                patch: IntegerConstantRewrite,
            ) -> Result<Self, PsiRewriteCandidateError> {
                Self::new(
                    input,
                    contract,
                    affected_blocks,
                    Vec::new(),
                    provenance,
                    PsiRewriteWitness::ProofCertifiedScalarIdentity {
                        constant_fact,
                        obligation_fact,
                    },
                    predicted_cost_delta,
                    PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch),
                )
            }

            #[allow(clippy::too_many_arguments)]
            pub fn new_proof_certified_scalar_identity(
                input: OptimizationUnitIdentity,
                contract: OptimizationRuleContract,
                affected_blocks: Vec<BlockId>,
                provenance: Vec<ProvenanceRewrite>,
                constant_fact: ScalarConstantFactIdentity,
                obligation_fact: AcceptedObligationFactIdentity,
                predicted_cost_delta: i64,
                patch: ProofCertifiedScalarIdentityRewrite,
            ) -> Result<Self, PsiRewriteCandidateError> {
                Self::new(
                    input,
                    contract,
                    affected_blocks,
                    vec![ScalarSubstitution {
                        from: patch.result,
                        to: patch.replacement,
                        scalar_type: ScalarType::Integer(patch.scalar_type),
                    }],
                    provenance,
                    PsiRewriteWitness::ProofCertifiedScalarIdentity {
                        constant_fact,
                        obligation_fact,
                    },
                    predicted_cost_delta,
                    PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch),
                )
            }

            #[allow(clippy::too_many_arguments)]
            pub fn new_total_scalar_identity(
                input: OptimizationUnitIdentity,
                contract: OptimizationRuleContract,
                affected_blocks: Vec<BlockId>,
                provenance: Vec<ProvenanceRewrite>,
                constant_fact: ScalarConstantFactIdentity,
                predicted_cost_delta: i64,
                patch: TotalScalarIdentityRewrite,
            ) -> Result<Self, PsiRewriteCandidateError> {
                Self::new(
                    input,
                    contract,
                    affected_blocks,
                    vec![ScalarSubstitution {
                        from: patch.result,
                        to: patch.replacement,
                        scalar_type: ScalarType::Integer(patch.scalar_type),
                    }],
                    provenance,
                    PsiRewriteWitness::TotalScalarIdentity { constant_fact },
                    predicted_cost_delta,
                    PsiRewritePatch::EliminateTotalScalarIdentity(patch),
                )
            }
        }
    }
}
