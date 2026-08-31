use super::super::shared::*;

pub(super) type DerivedValue<'a> = (
    &'a omega_optimization_unit::OptimizationNode,
    SourceLeafValue,
);

pub(super) struct LeafContext<'a> {
    pub(super) function: usize,
    pub(super) arm_edge: EdgeId,
    pub(super) source_value: psi_core::ValueId,
    pub(super) nodes: &'a [omega_optimization_unit::OptimizationNode],
    pub(super) abstracted: &'a omega_abstract_operations::AbstractFunction,
    pub(super) optimized: &'a omega_optimization_unit::PsiOptimizationFunction,
    pub(super) accepted_obligation_facts: &'a [AcceptedObligationFact],
    pub(super) temporaries: [LegalizedTemporaryId; 2],
    pub(super) u64_integer_type: psi_core::IntegerType,
    pub(super) u64_type: ScalarType,
}
