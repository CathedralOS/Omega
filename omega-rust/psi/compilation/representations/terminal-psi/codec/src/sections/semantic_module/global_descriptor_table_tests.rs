//! Retained instructions must not acquire writable bytes from forged geometry.
use super::target_family_tests::{identity_table, unit_machine};
use crate::{decode_module, encode_module};
use semantic_vocabulary::{
    MachineId, OperationId, PlaceId, ServiceId, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::*;

fn descriptor_module(operation: GlobalDescriptorTableOperation) -> TerminalModule {
    let machine_id = MachineId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(machine_id);
    module.identities = identity_table(4);
    let service = ServiceId::new(1).unwrap();
    module.services.push(ServiceDeclaration {
        id: service,
        identity: TerminalIdentityIndex(4),
        parents: vec![],
    });
    let carrier = StructuralTypeId::new(1).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: carrier,
        identity: TerminalIdentityIndex(3),
        quotient_carrier: None,
        owns_linear_claim: false,
        copyable: false,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        scalar_restriction: None,
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity: 10,
            access: Some(StructuralAccess::MutableBorrow),
        }),
    });
    let place = PlaceId::new(1).unwrap();
    let mut machine = unit_machine(machine_id);
    machine.declaration.as_mut().unwrap().parameter_types = vec![TerminalIdentityIndex(3)];
    machine
        .parameter_order
        .push(BoundaryParameterKind::Structural);
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place,
            position: 0,
            structural_type: carrier,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::MutableBorrow,
            qualifications: vec![],
            projected_qualifications: vec![],
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter { position: 0 },
    });
    machine.declared_service_reach.push(service);
    machine.blocks[0].operations.push(Operation::unsuspended(
        OperationId::new(1).unwrap(),
        OperationResult::Unit,
        OperationKind::GlobalDescriptorTable {
            service,
            operation,
            source: StructuralArgument::whole(place, StructuralAccess::MutableBorrow),
        },
    ));
    module.machines.push(machine);
    module
}

fn refusal(module: &TerminalModule) -> String {
    let encoded = encode_module(module).expect("encode deliberately forged retained input");
    format!(
        "{:?}",
        decode_module(&encoded).expect_err("receiving validation must refuse")
    )
}

#[test]
fn descriptor_table_directions_round_trip_with_exact_borrowed_bytes() {
    for operation in [
        GlobalDescriptorTableOperation::Store,
        GlobalDescriptorTableOperation::Load,
    ] {
        let module = descriptor_module(operation);
        let bytes = encode_module(&module).unwrap();
        assert_eq!(decode_module(&bytes).unwrap(), module);
    }
}

#[test]
fn descriptor_table_receiving_rejects_wrong_extent_and_shared_backing() {
    let mut short = descriptor_module(GlobalDescriptorTableOperation::Store);
    short.structural_types[0].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity: 9,
            access: Some(StructuralAccess::MutableBorrow),
        });
    assert!(refusal(&short).contains("exact unqualified ten-byte array"));

    let mut shared = descriptor_module(GlobalDescriptorTableOperation::Store);
    shared.structural_types[0].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity: 10,
            access: Some(StructuralAccess::SharedBorrow),
        });
    assert!(refusal(&shared).contains("byte borrow exceeds its source access"));

    let mut unreadable = descriptor_module(GlobalDescriptorTableOperation::Load);
    if let OperationKind::GlobalDescriptorTable { source, .. } =
        &mut unreadable.machines[0].blocks[0].operations[0].kind
    {
        source.access = StructuralAccess::WriteOnlyBorrow;
    }
    assert!(refusal(&unreadable).contains("declared byte borrow"));
}
