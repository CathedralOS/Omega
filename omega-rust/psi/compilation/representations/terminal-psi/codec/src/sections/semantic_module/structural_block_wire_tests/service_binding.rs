//! A portable service declaration is not authority to construct its value.

use super::{
    Operation, OperationKind, OperationResult, StructuralMultiplicity, StructuralOperationResult,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalModule, decode_module, encode_module, id, unit_module,
};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralParameterDeclaration, Terminator,
};

fn service_module() -> TerminalModule {
    let mut module = unit_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: id(1),
        identity: "Binding<Host>".into(),
        shape: StructuralTypeShape::FusedServiceBinding,
    });
    module
}

#[test]
fn fused_service_binding_module_roundtrip_preserves_nonconstructible_shape() {
    let module = service_module();
    let bytes = encode_module(&module).expect("a declaration does not establish a binding");
    assert_eq!(decode_module(&bytes), Ok(module));
}

#[test]
fn fused_service_binding_rejects_record_and_zero_construction_on_both_wire_entrances() {
    for kind in [
        OperationKind::EstablishRecord { fields: Vec::new() },
        OperationKind::EstablishZeroedStructural {},
    ] {
        let mut module = service_module();
        let machine = &mut module.machines[0];
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: id(1),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(1),
                structural_type: id(1),
            },
        });
        machine.blocks[0].operations.push(Operation {
            id: id(1),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Structural(StructuralOperationResult {
                place: id(1),
                structural_type: id(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            kind,
        });
        // The identical construction is valid for an ordinary empty record;
        // rejection must come from service validity, not an ill-formed fixture.
        let mut ordinary = module.clone();
        ordinary.structural_types[0].shape = StructuralTypeShape::Record { fields: Vec::new() };
        let bytes = encode_module(&ordinary).expect("ordinary empty record construction");
        assert_eq!(decode_module(&bytes), Ok(ordinary));
        // Use the binding's valid owned multiplicity so this control tests
        // construction, independently of unrestricted-owner rejection.
        let OperationResult::Structural(result) =
            &mut module.machines[0].blocks[0].operations[0].result
        else {
            panic!("the fixture declares a structural result");
        };
        result.multiplicity = StructuralMultiplicity::Affine;
        assert!(encode_module(&module).is_err());
        let forged =
            super::super::encode_raw(&module).expect("untrusted bytes can spell the forgery");
        assert!(decode_module(&forged).is_err());
    }
}

fn parameter_module(nested: bool, access: StructuralAccess) -> TerminalModule {
    let mut module = service_module();
    let structural_type = if nested {
        module.structural_types.push(StructuralTypeDeclaration {
            id: id(2),
            identity: "ServiceOwner".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id(1),
                    identity: "service".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(id(1)),
                }],
            },
        });
        id(2)
    } else {
        id(1)
    };
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(1),
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(1),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    module
}

#[test]
fn fused_service_binding_rejects_unrestricted_owned_parameters_on_both_wire_entrances() {
    for nested in [false, true] {
        let module = parameter_module(nested, StructuralAccess::Owned);
        assert!(encode_module(&module).is_err(), "nested={nested}");
        let forged = super::super::encode_raw(&module).expect("raw owned signature");
        assert!(decode_module(&forged).is_err(), "nested={nested}");
        // Borrowing the very same carrier leaves ownership with the caller.
        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ] {
            let borrowed = parameter_module(nested, access);
            let bytes = encode_module(&borrowed).expect("service borrow is unrestricted");
            assert_eq!(decode_module(&bytes), Ok(borrowed));
        }
    }
}

fn block_loan_module(nested: bool, access: StructuralAccess) -> TerminalModule {
    let mut module = parameter_module(nested, access);
    let machine = &mut module.machines[0];
    let mut destination = machine.structural_parameters[0].clone();
    destination.place = id(2);
    let mut successor = machine.blocks[0].clone();
    successor.id = id(2);
    successor.structural_parameters = vec![destination];
    successor.terminator = Terminator::ReturnUnit {
        edge: id(2),
        trivial_affine_discards: Vec::new(),
    };
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(2),
        kind: StructuralPlaceKind::BlockParameter {
            block: id(2),
            position: 0,
        },
    });
    machine.blocks[0].terminator = Terminator::Jump {
        edge: id(1),
        target: id(2),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: id(1),
            path: Vec::new(),
            access,
        }],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(successor);
    module
}

#[test]
fn fused_service_binding_block_loans_preserve_exact_custody() {
    for nested in [false, true] {
        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ] {
            let module = block_loan_module(nested, access);
            let bytes = encode_module(&module).expect("existing binding loan crosses a state edge");
            assert_eq!(decode_module(&bytes), Ok(module.clone()));

            let mut substituted = module.clone();
            let mut wrong_carrier = substituted.structural_types.last().unwrap().clone();
            wrong_carrier.id = id(3);
            wrong_carrier.identity = "AnotherServiceCarrier".into();
            substituted.structural_types.push(wrong_carrier);
            substituted.machines[0].blocks[1].structural_parameters[0].structural_type = id(3);
            assert!(
                encode_module(&substituted).is_err(),
                "carrier identity cannot change"
            );

            let mut amplified = module;
            amplified.machines[0].blocks[1].structural_parameters[0].access =
                StructuralAccess::Owned;
            amplified.machines[0].blocks[1].structural_parameters[0].multiplicity =
                StructuralMultiplicity::Affine;
            assert!(
                encode_module(&amplified).is_err(),
                "loan cannot become ownership"
            );
        }
    }
}

#[test]
fn fused_service_binding_mutable_state_transfer_rejects_stale_source_name() {
    for nested in [false, true] {
        let mut module = block_loan_module(nested, StructuralAccess::MutableBorrow);
        let machine = &mut module.machines[0];
        let mut final_block = machine.blocks[1].clone();
        final_block.id = id(3);
        final_block.structural_parameters[0].place = id(3);
        final_block.terminator = Terminator::ReturnUnit {
            edge: id(3),
            trivial_affine_discards: Vec::new(),
        };
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: id(3),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(3),
                position: 0,
            },
        });
        let mut forwarding = machine.blocks[0].terminator.clone();
        let Terminator::Jump {
            edge,
            target,
            structural_arguments,
            ..
        } = &mut forwarding
        else {
            panic!("fixture entry jumps");
        };
        *edge = id(2);
        *target = id(3);
        structural_arguments[0].place = id(2);
        machine.blocks[1].terminator = forwarding;
        machine.blocks.push(final_block);
        encode_module(&module).expect("current exclusive name forwards once");
        let Terminator::Jump {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[1].terminator
        else {
            panic!("fixture continuation jumps");
        };
        structural_arguments[0].place = id(1);
        assert!(
            encode_module(&module).is_err(),
            "transferred name cannot lend again"
        );
    }
}
