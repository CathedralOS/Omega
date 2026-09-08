use super::*;

#[test]
fn literal_identity_binds_payload_declaration_and_producer_reference() {
    let mut plan = call_aware_plan();
    let call = structural_call_mut(&mut plan);
    let (_, argument) = structural_argument_mut(&mut call.arguments[0]);
    argument.source = target_operations::TargetStructuralArgumentSource::ByteSequenceLiteral {
        psi_operation: id(900),
    };
    let destination = StructuralPlaceDeclaration {
        id: argument.place,
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: argument.structural_type,
        },
    };
    let structural_type = StructuralTypeDeclaration {
        id: argument.structural_type,
        identity: "bytes".into(),
        shape: StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView),
    };
    let mut establishment = plan.scalar_functions[0].blocks[0].instructions[0].clone();
    establishment.operation = id(900);
    establishment.result = None;
    establishment.kind = LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
        destination,
        structural_type,
        bytes: vec![0, 128, 255],
    };
    plan.scalar_functions[0].blocks[0]
        .instructions
        .push(establishment);
    let identity = legalized_operation_plan_identity(&plan);
    for mutation in 0..8 {
        let mut changed = plan.clone();
        if mutation < 2 {
            let call = structural_call_mut(&mut changed);
            let (_, argument) = structural_argument_mut(&mut call.arguments[0]);
            argument.source = if mutation == 0 {
                target_operations::TargetStructuralArgumentSource::ByteSequenceLiteral {
                    psi_operation: id(901),
                }
            } else {
                argument.destination.clone().into()
            };
        } else {
            let row = changed.scalar_functions[0].blocks[0]
                .instructions
                .last_mut()
                .unwrap();
            let LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                destination,
                structural_type,
                bytes,
            } = &mut row.kind
            else {
                panic!("literal fixture");
            };
            match mutation {
                2 => row.operation = id(901),
                3 => destination.id = id(901),
                4 => {
                    destination.kind = StructuralPlaceKind::ByteSequenceLiteral {
                        declaration_ordinal: 1,
                        structural_type: structural_type.id,
                    }
                }
                5 => structural_type.id = id(901),
                6 => bytes[1] = 129,
                _ => bytes.push(0),
            }
        }
        assert_identity_drift(identity, &changed);
    }
}
