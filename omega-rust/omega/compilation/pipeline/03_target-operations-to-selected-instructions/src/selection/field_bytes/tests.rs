use super::FieldBytes;
use crate::tests::legalization::owned_block_store::{arrival, id, legalized_arrival};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralTypeShape,
};

#[test]
fn measured_and_indexed_fields_share_backing_geometry_and_read_permission() {
    for (carrier, expected) in [
        (
            ByteSequenceCarrier::BoundedOwned { capacity: 8 },
            FieldBytes::Inline(8),
        ),
        (
            ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            },
            FieldBytes::Descriptor(0),
        ),
    ] {
        let mut function = legalized_arrival();
        let declaration = function
            .structural
            .as_mut()
            .unwrap()
            .structural_types
            .make_mut()
            .iter_mut()
            .find(|declaration| declaration.id == arrival().structural_type)
            .unwrap();
        let StructuralTypeShape::Record { fields } = &mut declaration.shape else {
            panic!("record fixture");
        };
        fields[0].field_type = StructuralFieldType::ByteSequence(carrier);
        let argument = StructuralArgument {
            place: arrival().place,
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        };
        let operand = terminal_psi::control_flow::ByteSequenceOperand::Field {
            source: argument.clone(),
            field: id(1),
            length: id(99),
        };
        for access in [
            StructuralAccess::Owned,
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let parameter = function
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.structural_parameters)
                .find(|parameter| parameter.place == argument.place)
                .unwrap();
            parameter.access = access;
            let readable = access != StructuralAccess::WriteOnlyBorrow;
            assert_eq!(
                FieldBytes::resolve(&function, &argument, id(1), &[]),
                readable.then_some(expected),
            );
            assert_eq!(
                crate::selection::byte_concatenation_input::field_offset(&function, &operand),
                readable.then_some(expected.offset()),
            );
        }
        function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.structural_parameters)
            .find(|parameter| parameter.place == argument.place)
            .unwrap()
            .access = StructuralAccess::SharedBorrow;
        assert_eq!(FieldBytes::resolve(&function, &argument, id(2), &[]), None);
        assert_eq!(
            FieldBytes::resolve(&function, &argument, id(999), &[]),
            None
        );
        let exclusive = StructuralArgument {
            access: StructuralAccess::MutableBorrow,
            ..argument
        };
        assert_eq!(super::root_type(&function, &exclusive), None);
    }
}
