use super::super::shared::*;

pub(super) type DerivedValue<'a> = (&'a optimization_unit::OptimizationNode, SourceLeafValue);

pub(super) struct LeafContext<'a> {
    pub(super) function: usize,
    pub(super) arm_edge: EdgeId,
    pub(super) source_value: semantic_vocabulary::ValueId,
    pub(super) nodes: &'a [optimization_unit::OptimizationNode],
    pub(super) abstracted: &'a abstract_operations::AbstractFunction,
    pub(super) optimized: &'a optimization_unit::PsiOptimizationFunction,
    pub(super) accepted_obligation_facts: &'a [AcceptedObligationFact],
    pub(super) temporaries: [LegalizedTemporaryId; 2],
    pub(super) u64_integer_type: semantic_vocabulary::IntegerType,
    pub(super) u64_type: ScalarType,
}
