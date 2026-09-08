//! Independently reconstructed successor edges and edge custody.

use super::*;

pub(crate) fn successors_match_operation(
    operation: &abstract_operations::AbstractOperation,
    actual: &[OptimizationEdge],
) -> bool {
    let expected = expected_edges(operation);
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            actual.psi_edge == expected.psi_edge
                && actual.target == expected.target
                && actual.bindings == expected.bindings
                && actual.structural_bindings == expected.structural_bindings
                && actual.trivial_affine_discards == expected.trivial_affine_discards
                && actual.residual_affine_discards == expected.residual_affine_discards
                && actual.provenance.first() == Some(&PsiProvenance::Edge(actual.psi_edge))
                && actual
                    .provenance
                    .iter()
                    .all(|source| matches!(source, PsiProvenance::Edge(_)))
        })
}

pub(crate) fn expected_edges(
    operation: &abstract_operations::AbstractOperation,
) -> Vec<OptimizationEdge> {
    use abstract_operations::AbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            structural_bindings,
            trivial_affine_discards,
            residual_affine_discards,
        } => vec![OptimizationEdge {
            structural_bindings: structural_bindings.clone(),
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
            residual_affine_discards: residual_affine_discards.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![optimization_unit::FuelSettlement {
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
                structural_bindings: edge.structural_bindings.clone(),
                psi_edge: edge.psi_edge,
                target: edge.target,
                bindings: edge.bindings.clone(),
                trivial_affine_discards: edge.trivial_affine_discards.clone(),
                residual_affine_discards: Vec::new(),
                provenance: vec![PsiProvenance::Edge(edge.psi_edge)],
                fuel: vec![optimization_unit::FuelSettlement {
                    site: PsiProvenance::Edge(edge.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        O::StructuralCase { cases, .. } => cases
            .iter()
            .map(|case| OptimizationEdge {
                psi_edge: case.psi_edge,
                target: case.target,
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                trivial_affine_discards: case.trivial_affine_discards.clone(),
                residual_affine_discards: Vec::new(),
                provenance: vec![PsiProvenance::Edge(case.psi_edge)],
                fuel: vec![optimization_unit::FuelSettlement {
                    site: PsiProvenance::Edge(case.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}
