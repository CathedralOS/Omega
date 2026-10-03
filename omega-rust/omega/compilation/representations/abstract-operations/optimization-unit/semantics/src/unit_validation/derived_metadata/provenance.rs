//! Independently reconstructed node provenance.
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiProvenance;

pub(crate) fn expected_provenance(
    operation: &abstract_operations::AbstractOperation,
) -> Vec<PsiProvenance> {
    use abstract_operations::AbstractOperation as O;
    match operation {
        O::DynamicDescriptorParameter { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::StructuralCase { .. } => Vec::new(),
        O::Return { psi_edge, .. } | O::ReturnUnit { psi_edge, .. } | O::Crash { psi_edge, .. } => {
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
        O::EstablishPrimitiveLocal { psi_operation, .. }
        | O::PrimitiveLocalStore { psi_operation, .. }
        | O::PrimitiveScalarRead { psi_operation, .. }
        | O::StructuralCaseMembership { psi_operation, .. }
        | O::WriteOnlyPrimitiveStore { psi_operation, .. }
        | O::StructuralScalarFieldStore { psi_operation, .. }
        | O::AtomicEvent { psi_operation, .. }
        | O::StoreDynamicDescriptor { psi_operation, .. }
        | O::EstablishScalarArray { psi_operation, .. }
        | O::EstablishCase { psi_operation, .. }
        | O::EstablishByteSequenceConcatenation { psi_operation, .. }
        | O::EstablishByteSequenceLiteral { psi_operation, .. }
        | O::EstablishTrivialAffineLocal { psi_operation, .. }
        | O::EstablishZeroedStructural { psi_operation, .. }
        | O::EstablishStructuralLocal { psi_operation, .. }
        | O::EstablishReference { psi_operation, .. }
        | O::ReleaseReference { psi_operation, .. }
        | O::MoveStructuralField { psi_operation, .. }
        | O::StoreStructuralField { psi_operation, .. }
        | O::StructuralLeafCopy { psi_operation, .. }
        | O::EstablishRecord { psi_operation, .. }
        | O::CallStructural { psi_operation, .. }
        | O::CallDynamic { psi_operation, .. }
        | O::BoundaryCall { psi_operation, .. }
        | O::PortWrite { psi_operation, .. }
        | O::Call { psi_operation, .. }
        | O::IntegerConstant { psi_operation, .. }
        | O::IeeeFloatConstant { psi_operation, .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. }
        | O::IeeeFloatCompare { psi_operation, .. }
        | O::FloatResultPolicy { psi_operation, .. }
        | O::BooleanConstant { psi_operation, .. }
        | O::ByteSequenceRead { psi_operation, .. }
        | O::ByteSequencePredicate { psi_operation, .. }
        | O::ByteSequenceWrite { psi_operation, .. }
        | O::ByteSequenceStore { psi_operation, .. }
        | O::StructuralByteSequenceFieldStore { psi_operation, .. }
        | O::StructuralByteViewFieldStore { psi_operation, .. }
        | O::StructuralByteSequenceFieldByteStore { psi_operation, .. }
        | O::ByteSequenceSubslice { psi_operation, .. }
        | O::ByteSequenceLength { psi_operation, .. }
        | O::EstablishElementView { psi_operation, .. }
        | O::ElementViewLength { psi_operation, .. }
        | O::ElementViewRead { psi_operation, .. }
        | O::ElementViewWrite { psi_operation, .. }
        | O::ElementViewSubslice { psi_operation, .. }
        | O::StructuralByteSequenceFieldLength { psi_operation, .. }
        | O::StructuralByteSequenceFieldRead { psi_operation, .. }
        | O::StructuralScalarFieldRead { psi_operation, .. }
        | O::BooleanNot { psi_operation, .. }
        | O::BooleanEqual { psi_operation, .. }
        | O::IntegerEqual { psi_operation, .. }
        | O::IntegerLessThan { psi_operation, .. }
        | O::IntegerLessOrEqual { psi_operation, .. }
        | O::IntegerBitwiseNot { psi_operation, .. }
        | O::IntegerWiden { psi_operation, .. }
        | O::IntegerExactCast { psi_operation, .. }
        | O::ScalarRepresentationView { psi_operation, .. }
        | O::IntegerBitwiseAnd { psi_operation, .. }
        | O::IntegerBitwiseOr { psi_operation, .. }
        | O::IntegerBitwiseXor { psi_operation, .. }
        | O::IntegerShift { psi_operation, .. }
        | O::IntegerArithmetic { psi_operation, .. }
        | O::TrappingInteger { psi_operation, .. } => {
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
            O::DynamicDescriptorParameter { .. } => provenance.is_empty(),
            O::Jump { .. } | O::Conditional { .. } | O::StructuralCase { .. } => true,
            _ => provenance.is_empty(),
        }
    } else {
        provenance.starts_with(&expected)
    }
}
