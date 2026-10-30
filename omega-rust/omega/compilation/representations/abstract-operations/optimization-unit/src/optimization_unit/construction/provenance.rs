use super::super::{AbstractOperation, PsiProvenance};

pub(super) fn operation_node_provenance(operation: &AbstractOperation) -> Vec<PsiProvenance> {
    use AbstractOperation as O;
    let site = match operation {
        abstract_operations::jump_or_conditional_pattern!() | O::StructuralCase { .. } => {
            return Vec::new();
        }
        abstract_operations::return_or_return_unit_pattern!(psi_edge, ..)
        | O::InvokeNoNormal { psi_edge, .. }
        | O::ExternalCompletion { psi_edge, .. }
        | O::Crash { psi_edge, .. } => PsiProvenance::Edge(*psi_edge),
        O::ReturnStructural {
            psi_edge,
            trivial_affine_locals,
            ..
        } => {
            // Provenance is custody order, not execution order: the terminal
            // edge remains the primary realization site, followed by the
            // compressed establishment operations in tuple order. Rewrites
            // may append inherited custody only after this exact prefix.
            return std::iter::once(PsiProvenance::Edge(*psi_edge))
                .chain(
                    trivial_affine_locals
                        .iter()
                        .map(|(operation, _, _)| PsiProvenance::Operation(*operation)),
                )
                .collect();
        }
        _ => PsiProvenance::Operation(
            operation
                .psi_operation()
                .expect("non-control operation names its authored psi operation"),
        ),
    };
    vec![site]
}
