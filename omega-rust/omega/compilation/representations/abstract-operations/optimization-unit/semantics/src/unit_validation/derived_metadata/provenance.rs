//! Independently reconstructed node provenance.
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiProvenance;

pub(crate) fn expected_provenance(
    operation: &abstract_operations::AbstractOperation,
) -> Vec<PsiProvenance> {
    match operation {
        abstract_operations::jump_or_conditional_pattern!() | O::StructuralCase { .. } => {
            Vec::new()
        }
        abstract_operations::return_or_return_unit_pattern!(psi_edge, ..)
        | O::InvokeNoNormal { psi_edge, .. }
        | O::ExternalCompletion { psi_edge, .. }
        | O::Crash { psi_edge, .. } => {
            vec![PsiProvenance::Edge(*psi_edge)]
        }
        O::ReturnStructural {
            psi_edge,
            trivial_affine_locals,
            ..
        } => {
            // This is deliberately primary-site-first custody order rather
            // than execution order. The return edge anchors the node; hidden
            // establishments follow in their exact tuple order.
            std::iter::once(PsiProvenance::Edge(*psi_edge))
                .chain(
                    trivial_affine_locals
                        .iter()
                        .map(|(operation, _, _)| PsiProvenance::Operation(*operation)),
                )
                .collect()
        }
        _ => {
            vec![PsiProvenance::Operation(operation.psi_operation().expect(
                "non-control operation names its authored psi operation",
            ))]
        }
    }
}

pub(crate) fn provenance_matches_operation(
    operation: &abstract_operations::AbstractOperation,
    provenance: &[PsiProvenance],
) -> bool {
    let expected = expected_provenance(operation);
    if expected.is_empty() {
        match operation {
            abstract_operations::jump_or_conditional_pattern!() | O::StructuralCase { .. } => true,
            _ => provenance.is_empty(),
        }
    } else {
        provenance.starts_with(&expected)
    }
}
