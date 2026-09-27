//! Maximal subtree and empty-complement acceptance, independent of producer fixtures.

use super::fixtures::{array, field, index, record, unit};
use crate::validate_psi_optimization_unit;

#[test]
fn general_partial_affine_scalar_record_fields_need_no_cleanup() {
    use semantic_vocabulary::{IeeeFloatFormat, ScalarType, StructuralFieldId};
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape,
    };

    let mut carrier = record(3, &[("row", 2)]);
    let StructuralTypeShape::Record { fields } = &mut carrier.shape else {
        unreachable!()
    };
    for (position, field_type) in [
        StructuralFieldType::Scalar(ScalarType::Boolean),
        StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64),
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
            capacity: 8,
        }),
    ]
    .into_iter()
    .enumerate()
    {
        fields.push(StructuralFieldDeclaration {
            id: crate::tests::support::id(position as u64 + 2, StructuralFieldId::new),
            identity: format!("metadata{position}"),
            relevance: BindingRelevance::Relevant,
            field_type,
        });
    }
    validate_psi_optimization_unit(&unit(
        vec![record(1, &[]), array(2, 1, 2), carrier.clone()],
        3,
        &[(vec![field("row"), index(1)], 1)],
        &[(vec![field("row"), index(0)], 1)],
    ))
    .expect("scalar, floating, and bounded byte fields add no residual cleanup");
    validate_psi_optimization_unit(&unit(
        vec![record(1, &[]), array(2, 1, 2), carrier],
        3,
        &[
            (vec![field("row"), index(1)], 1),
            (vec![field("row"), index(0)], 1),
        ],
        &[],
    ))
    .expect("scalar metadata does not prevent an empty structural complement");
}
