use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    FuelSettlement, OptimizationEdge, PsiOptimizationFunction, PsiProvenance, ValueDefinition,
};
use semantic_vocabulary::ValueId;

pub(super) fn scalar_value_definition(
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

pub(super) fn scalar_operation_successors(operation: &O) -> Vec<OptimizationEdge> {
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            trivial_affine_discards,
            residual_affine_discards,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
            residual_affine_discards: residual_affine_discards.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![FuelSettlement {
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
                trivial_affine_discards: successor.trivial_affine_discards.clone(),
                residual_affine_discards: Vec::new(),
                provenance: vec![PsiProvenance::Edge(successor.psi_edge)],
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Edge(successor.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod residual_edge_tests {
    #[test]
    fn successor_projection_retains_exact_ordered_residuals() {
        use semantic_vocabulary::{BlockId, EdgeId, PlaceId, StructuralTypeId};
        use terminal_psi::{StructuralAffineDiscard, StructuralPathSegment};
        let residuals = [2, 0]
            .map(|element| StructuralAffineDiscard {
                place: PlaceId::new(81).unwrap(),
                path: vec![StructuralPathSegment::FixedIndex(element)],
                structural_type: StructuralTypeId::new(82).unwrap(),
            })
            .to_vec();
        let operation = abstract_operations::AbstractOperation::Jump {
            psi_edge: EdgeId::new(83).unwrap(),
            target: BlockId::new(84).unwrap(),
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: residuals.clone(),
        };
        let successors = super::scalar_operation_successors(&operation);
        assert_eq!(successors.len(), 1);
        assert_eq!(successors[0].residual_affine_discards, residuals);
        assert!(successors[0].trivial_affine_discards.is_empty());
        assert_eq!(successors[0].target, BlockId::new(84).unwrap());
        assert_eq!(successors[0].psi_edge, EdgeId::new(83).unwrap());
    }
}
