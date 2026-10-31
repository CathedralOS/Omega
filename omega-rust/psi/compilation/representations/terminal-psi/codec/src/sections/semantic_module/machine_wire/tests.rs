use super::super::module_wire::encode_raw;
use super::super::target_family_tests::{identity_table, unit_machine};
use crate::{decode_module, encode_module, semantic_fingerprint};
use semantic_vocabulary::{
    IntegerSign, IntegerType, MachineId, PlaceId, ScalarType, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    BoundaryParameterKind::{Scalar, Structural},
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralRecordStorage, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalIdentityIndex, TerminalModule, ValueDeclaration,
};

fn module() -> TerminalModule {
    let identity = MachineId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(identity);
    module.identities = identity_table(4);
    let mut machine = unit_machine(identity);
    machine.declaration.as_mut().unwrap().parameter_types = vec![TerminalIdentityIndex(3); 4];
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    machine.parameters = [1, 2]
        .map(|value| ValueDeclaration {
            id: ValueId::new(value).unwrap(),
            scalar_type,
            qualifications: Default::default(),
        })
        .to_vec();
    let place = PlaceId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place,
            position: 0,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter { position: 0 },
    });
    module.structural_types.push(StructuralTypeDeclaration {
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        copyable: true,
        scalar_restriction: None,
        id: structural_type,
        identity: TerminalIdentityIndex(3),
        shape: StructuralTypeShape::Record {
            fields: Vec::new(),
            storage: StructuralRecordStorage::Packed,
        },
    });
    machine.parameter_order = vec![Scalar, Structural, Scalar];
    module.machines.push(machine);
    module
}

#[test]
fn machine_parameter_order_round_trip_and_bind_identity() {
    let original = module();
    let bytes = encode_module(&original).unwrap();
    let decoded = decode_module(&bytes).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(
        decoded.machines[0].runtime_parameter_formals().unwrap(),
        [0, 2, 1]
    );
    let mut different = original.clone();
    different.machines[0].parameter_order = vec![Structural, Scalar, Scalar];
    assert_eq!(
        different.machines[0].runtime_parameter_formals().unwrap(),
        [1, 2, 0]
    );
    assert_ne!(
        semantic_fingerprint(&original).unwrap(),
        semantic_fingerprint(&different).unwrap()
    );
    assert_eq!(
        decode_module(&encode_module(&different).unwrap()).unwrap(),
        different
    );
}

#[test]
fn machine_parameter_order_rejects_missing_extra_and_wrong_lane() {
    for order in [
        vec![],
        vec![Scalar, Structural],
        vec![Scalar; 3],
        vec![Scalar, Structural, Structural],
        vec![Scalar, Structural, Scalar, Scalar],
    ] {
        let mut invalid = module();
        invalid.machines[0].parameter_order = order;
        assert!(invalid.machines[0].validate_parameter_order().is_err());
        assert!(decode_module(&encode_raw(&invalid).unwrap()).is_err());
    }
}

#[test]
fn machine_parameter_order_has_no_erased_formal_lane() {
    let original = module();
    // The checked declaration has four source formals but only three runtime
    // parameters. No ABI position is reserved for the erased formal.
    assert_eq!(
        original.machines[0]
            .declaration
            .as_ref()
            .unwrap()
            .parameter_types
            .len(),
        4
    );
    assert_eq!(original.machines[0].parameter_order.len(), 3);
    assert_eq!(
        original.machines[0].runtime_parameter_formals().unwrap(),
        [0, 2, 1]
    );
}
