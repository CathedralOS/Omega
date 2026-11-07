//! Retained task instructions must keep the storage and scalar types checked by source.
use super::target_family_tests::{identity_table, unit_machine};
use crate::{decode_module, encode_module};
use semantic_vocabulary::{
    IntegerSign, IntegerType, MachineId, OperationId, PlaceId, ScalarType, ServiceId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::*;

fn unsigned(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
}

fn task_module() -> TerminalModule {
    let machine_id = MachineId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(machine_id);
    module.identities = identity_table(8);
    let service = ServiceId::new(1).unwrap();
    module.services.push(ServiceDeclaration {
        id: service,
        identity: TerminalIdentityIndex(8),
        parents: vec![],
    });
    let shapes = [
        StructuralTypeShape::FixedArray {
            element: StructuralTypeId::new(4).unwrap(),
            length: ArrayExtent::Literal(5),
        },
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity: 104,
            access: Some(StructuralAccess::SharedBorrow),
        }),
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity: 10,
            access: Some(StructuralAccess::MutableBorrow),
        }),
        StructuralTypeShape::PrimitiveScalar(unsigned(64)),
    ];
    for (index, shape) in shapes.into_iter().enumerate() {
        module.structural_types.push(StructuralTypeDeclaration {
            id: StructuralTypeId::new(index as u64 + 1).unwrap(),
            identity: TerminalIdentityIndex(index as u32 + 3),
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: false,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            scalar_restriction: None,
            shape,
        });
    }
    let mut machine = unit_machine(machine_id);
    machine.declaration.as_mut().unwrap().parameter_types =
        (3..7).map(TerminalIdentityIndex).collect();
    for (position, access) in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ]
    .into_iter()
    .enumerate()
    {
        let place = PlaceId::new(position as u64 + 1).unwrap();
        machine
            .parameter_order
            .push(BoundaryParameterKind::Structural);
        machine
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position: position as u32,
                structural_type: StructuralTypeId::new(position as u64 + 1).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                access,
                qualifications: vec![],
                projected_qualifications: vec![],
                additional_input_qualifications: Vec::new(),
            });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position: position as u32,
            },
        });
    }
    let selector = ValueId::new(1).unwrap();
    machine.parameters.push(ValueDeclaration {
        id: selector,
        scalar_type: unsigned(16),
        qualifications: Default::default(),
    });
    machine.parameter_order.push(BoundaryParameterKind::Scalar);
    machine.declared_service_reach.push(service);
    machine.blocks[0].operations.push(Operation::unsuspended(
        OperationId::new(1).unwrap(),
        OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(2).unwrap(),
            scalar_type: unsigned(8),
            qualifications: Default::default(),
        }),
        OperationKind::ActivateTaskRegister {
            service,
            entries: 5,
            selector,
            gdt: StructuralArgument::whole(
                PlaceId::new(1).unwrap(),
                StructuralAccess::MutableBorrow,
            ),
            task_state: StructuralArgument::whole(
                PlaceId::new(2).unwrap(),
                StructuralAccess::SharedBorrow,
            ),
            scratch: StructuralArgument::whole(
                PlaceId::new(3).unwrap(),
                StructuralAccess::MutableBorrow,
            ),
        },
    ));
    machine.blocks[0].operations.push(Operation::unsuspended(
        OperationId::new(2).unwrap(),
        OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(3).unwrap(),
            scalar_type: unsigned(16),
            qualifications: Default::default(),
        }),
        OperationKind::ReadTaskRegister { service },
    ));
    module.machines.push(machine);
    module
}

fn refusal(module: &TerminalModule) -> String {
    let bytes = encode_module(module).expect("encode deliberately malformed retained input");
    format!(
        "{:?}",
        decode_module(&bytes).expect_err("receiving must refuse the changed obligation")
    )
}

#[test]
fn task_register_operations_round_trip_with_exact_loans() {
    let module = task_module();
    let bytes = encode_module(&module).unwrap();
    assert_eq!(decode_module(&bytes).unwrap(), module);
}

#[test]
fn task_register_receiving_rejects_forged_operands_and_result() {
    let mut shared = task_module();
    if let OperationKind::ActivateTaskRegister { gdt, .. } =
        &mut shared.machines[0].blocks[0].operations[0].kind
    {
        gdt.access = StructuralAccess::SharedBorrow;
    }
    assert!(refusal(&shared).contains("mutable GDT"));
    let mut short = task_module();
    short.structural_types[1].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity: 103,
            access: Some(StructuralAccess::SharedBorrow),
        });
    assert!(refusal(&short).contains("exact unqualified"));
    let mut extent = task_module();
    if let OperationKind::ActivateTaskRegister { entries, .. } =
        &mut extent.machines[0].blocks[0].operations[0].kind
    {
        *entries = 6;
    }
    assert!(refusal(&extent).contains("exact unqualified"));
    let mut narrow = task_module();
    narrow.structural_types[3].shape = StructuralTypeShape::PrimitiveScalar(unsigned(32));
    assert!(refusal(&narrow).contains("declared element width"));
    let mut result = task_module();
    if let OperationResult::Scalar(result) = &mut result.machines[0].blocks[0].operations[1].result
    {
        result.scalar_type = unsigned(8);
    }
    assert!(refusal(&result).contains("exact unsigned result"));
    let mut selector = task_module();
    selector.machines[0].parameters[0].scalar_type = unsigned(32);
    assert!(refusal(&selector).contains("u16 selector"));
}
