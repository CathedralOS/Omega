use super::*;

pub(super) fn operation_edges(operation: &AbstractOperation) -> Vec<OptimizationEdge> {
    use AbstractOperation as O;
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
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Edge(*psi_edge),
                units: 1,
            }],
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => vec![successor_edge(when_true), successor_edge(when_false)],
        O::StructuralCase { cases, .. } => cases
            .iter()
            .map(|case| OptimizationEdge {
                psi_edge: case.psi_edge,
                target: case.target,
                // Payloads are produced on this case edge, not by preceding SSA
                // operations. Their complete telescope stays on StructuralCase.
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                trivial_affine_discards: case.trivial_affine_discards.clone(),
                residual_affine_discards: Vec::new(),
                provenance: vec![PsiProvenance::Edge(case.psi_edge)],
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Edge(case.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn successor_edge(successor: &AbstractSuccessor) -> OptimizationEdge {
    OptimizationEdge {
        structural_bindings: successor.structural_bindings.clone(),
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
    }
}
