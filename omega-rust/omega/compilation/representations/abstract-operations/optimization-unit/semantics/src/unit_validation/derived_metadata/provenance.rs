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
        O::SelectImplementation { psi_operation, .. }
        | O::EstablishPrimitiveLocal { psi_operation, .. }
        | O::PrimitiveLocalStore { psi_operation, .. }
        | abstract_operations::scalar_read_or_membership_pattern!(psi_operation, ..)
        | O::WriteOnlyPrimitiveStore { psi_operation, .. }
        | O::StructuralScalarFieldStore { psi_operation, .. }
        | O::AtomicEvent { psi_operation, .. }
        | O::EstablishScalarArray { psi_operation, .. }
        | abstract_operations::structural_array_or_case_pattern!(psi_operation, ..)
        | abstract_operations::establish_byte_literal_triple_pattern!(psi_operation, ..)
        | O::EstablishZeroedStructural { psi_operation, .. }
        | O::EstablishStructuralLocal { psi_operation, .. }
        | O::EstablishReference { psi_operation, .. }
        | O::EstablishPlacedReference { psi_operation, .. }
        | O::ReleaseReference { psi_operation, .. }
        | O::MoveStructuralField { psi_operation, .. }
        | O::StoreStructuralField { psi_operation, .. }
        | O::StructuralLeafCopy { psi_operation, .. }
        | O::EstablishRecord { psi_operation, .. }
        | abstract_operations::boundary_or_structural_call_pair_pattern!(psi_operation, ..)
        | O::PortWrite { psi_operation, .. }
        | O::RegisterAccess { psi_operation, .. }
        | O::Call { psi_operation, .. }
        | O::IntegerConstant { psi_operation, .. }
        | O::IeeeFloatConstant { psi_operation, .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. }
        | O::IeeeFloatCompare { psi_operation, .. }
        | O::FloatResultPolicy { psi_operation, .. }
        | O::BooleanConstant { psi_operation, .. }
        | abstract_operations::byte_read_predicate_equal_pattern!(psi_operation, ..)
        | O::ByteSequenceWrite { psi_operation, .. }
        | abstract_operations::byte_sequence_store_pair_pattern!(psi_operation, ..)
        | abstract_operations::byte_view_triple_pattern!(psi_operation, ..)
        | abstract_operations::byte_view_length_triple_pattern!(psi_operation, ..)
        | abstract_operations::element_view_read_write_pair_pattern!(psi_operation, ..)
        | O::ElementViewSubslice { psi_operation, .. }
        | abstract_operations::byte_len_or_field_read_pattern!(psi_operation, ..)
        | O::StructuralScalarFieldRead { psi_operation, .. }
        | abstract_operations::boolean_not_or_equal_pair_pattern!(psi_operation, ..)
        | abstract_operations::integer_equal_less_pair_pattern!(psi_operation, ..)
        | abstract_operations::bitwise_not_or_less_pair_pattern!(psi_operation, ..)
        | abstract_operations::integer_widen_or_exact_cast_pattern!(psi_operation, ..)
        | abstract_operations::bitwise_and_or_view_pair_pattern!(psi_operation, ..)
        | abstract_operations::bitwise_or_xor_pair_pattern!(psi_operation, ..)
        | abstract_operations::integer_dispatch_triple_pattern!(psi_operation, ..) => {
            vec![PsiProvenance::Operation(*psi_operation)]
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
