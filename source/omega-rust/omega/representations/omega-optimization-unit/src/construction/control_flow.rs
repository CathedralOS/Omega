use super::*;

pub(super) fn operation_edges(operation: &AbstractOperation) -> Vec<OptimizationEdge> {
    use AbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            trivial_affine_discards,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
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
        _ => Vec::new(),
    }
}

fn successor_edge(successor: &AbstractSuccessor) -> OptimizationEdge {
    OptimizationEdge {
        psi_edge: successor.psi_edge,
        target: successor.target,
        bindings: successor.bindings.clone(),
        trivial_affine_discards: successor.trivial_affine_discards.clone(),
        provenance: vec![PsiProvenance::Edge(successor.psi_edge)],
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Edge(successor.psi_edge),
            units: 1,
        }],
    }
}
