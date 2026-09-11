//! Graph projection preserves cycles; this is not ranking or native admission.
use super::*;

fn cyclic() -> AbstractOperationPlan {
    let mut plan = transfers::transferred();
    let caller = &mut plan.functions[1];
    caller.operations[4] = AbstractOperation::Jump {
        psi_edge: edge(5),
        target: block(4),
        bindings: caller.block_entries[1]
            .parameters
            .iter()
            .map(|parameter| abstract_operations::ValueBinding {
                argument: parameter.value,
                parameter: parameter.value,
                scalar_type: parameter.scalar_type,
            })
            .collect(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    plan
}

#[test]
fn cyclic_unit_graph_preserves_destination_values_and_backedge_without_rank_authority() {
    let plan = cyclic();
    let lowered = lower(&plan).unwrap();
    let TargetOperation::ControlGraph(graph) = &lowered.functions[1].operation else {
        panic!("graph");
    };
    assert_eq!(
        graph
            .blocks
            .iter()
            .map(|block| block.block)
            .collect::<Vec<_>>(),
        vec![block(1), block(4), block(5), block(6)]
    );
    let target_operations::TargetControlTerminator::Jump { successor } =
        &graph.blocks[2].terminator
    else {
        panic!("backedge");
    };
    assert_eq!(successor.target, block(4));
    assert_eq!(successor.bindings.len(), 3);
    assert_eq!(successor.bindings[0].argument, value(50));
    assert!(matches!(
        graph.blocks[3].terminator,
        target_operations::TargetControlTerminator::Return { .. }
    ));
}

#[test]
fn cyclic_unit_graph_rejects_future_and_sibling_definitions() {
    for argument in [value(10), value(99)] {
        let mut plan = cyclic();
        let AbstractOperation::Conditional { when_true, .. } = &mut plan.functions[1].operations[0]
        else {
            panic!("entry");
        };
        when_true.bindings[0].argument = argument;
        assert!(lower(&plan).is_err());
    }
    let mut plan = fixture();
    let AbstractOperation::Jump { target, .. } = &mut plan.functions[1].operations[3] else {
        panic!("jump");
    };
    *target = block(2);
    let AbstractOperation::CallUnit { arguments, .. } = &mut plan.functions[1].operations[5] else {
        panic!("call");
    };
    arguments[0] = value(10);
    assert!(lower(&plan).is_err());
}

fn descriptor_cycle() -> AbstractOperationPlan {
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    let mut plan = cyclic();
    let structural_type = StructuralTypeId::new(60).unwrap();
    let root = PlaceId::new(60).unwrap();
    let current = PlaceId::new(61).unwrap();
    let suffix = PlaceId::new(62).unwrap();
    plan.structural_types
        .make_mut()
        .push(terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "cycle::Bytes".into(),
            shape: terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        });
    let parameter = terminal_psi::StructuralParameterDeclaration {
        place: root,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        access: terminal_psi::StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let binding = |place| abstract_operations::AbstractStructuralBinding {
        parameter: current,
        argument: terminal_psi::StructuralArgument {
            place,
            access: terminal_psi::StructuralAccess::SharedBorrow,
            path: Vec::new(),
        },
    };
    let caller = &mut plan.functions[1];
    caller.structural_parameters.push(parameter.clone());
    caller.block_entries[1].structural_parameters.push(
        terminal_psi::StructuralParameterDeclaration {
            place: current,
            ..parameter
        },
    );
    let AbstractOperation::Conditional {
        when_true,
        when_false,
        ..
    } = &mut caller.operations[0]
    else {
        panic!("entry");
    };
    when_true.structural_bindings = vec![binding(root)];
    when_false.structural_bindings = vec![binding(root)];
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut caller.operations[4]
    else {
        panic!("backedge");
    };
    *structural_bindings = vec![binding(suffix)];
    caller.operations.splice(
        4..4,
        [
            AbstractOperation::ByteSequenceLength {
                psi_operation: operation(20),
                source: current,
                result: abstract_operations::AbstractResult {
                    value: value(20),
                    scalar_type: caller.parameters[2].scalar_type,
                },
            },
            AbstractOperation::ByteSequenceSubslice {
                psi_operation: operation(21),
                source: current,
                start: value(52),
                end: value(20),
                length: value(20),
                obligation: semantic_vocabulary::ObligationId::new(21).unwrap(),
                result: terminal_psi::StructuralOperationResult {
                    place: suffix,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                },
            },
        ],
    );
    caller.block_entries[3].operation_offset += 2;
    plan
}

#[test]
fn cyclic_unit_graph_retains_fresh_descriptor_observation_and_exact_backedge() {
    let plan = descriptor_cycle();
    let lowered = lower(&plan).unwrap();
    let TargetOperation::ControlGraph(graph) = &lowered.functions[1].operation else {
        panic!("graph");
    };
    assert_eq!(
        graph.blocks[1].structural_parameters[0].place,
        semantic_vocabulary::PlaceId::new(61).unwrap()
    );
    assert_eq!(graph.blocks[2].operations.len(), 2);
    let target_operations::TargetControlTerminator::Jump { successor } =
        &graph.blocks[2].terminator
    else {
        panic!("backedge");
    };
    assert_eq!(
        successor.structural_bindings[0].argument.place,
        semantic_vocabulary::PlaceId::new(62).unwrap()
    );
    for wrong_source in [60, 62] {
        let mut wrong = plan.clone();
        let AbstractOperation::ByteSequenceLength { source, .. } =
            &mut wrong.functions[1].operations[4]
        else {
            panic!("length");
        };
        *source = semantic_vocabulary::PlaceId::new(wrong_source).unwrap();
        assert!(
            lower(&wrong).is_err(),
            "wrong/future descriptor {wrong_source}"
        );
    }
}
