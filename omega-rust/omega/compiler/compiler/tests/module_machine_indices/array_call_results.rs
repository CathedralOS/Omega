//! A separately decoded source product must retain its array payload when used
//! as an ordinary callee. This is consumer coverage, not source-call admission.

use super::{Sources, compile, root_inputs};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerValue, MachineId, OperationId, PlaceId,
    StructuralPlaceKind, ValueId,
};
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};
use terminal_psi::{
    Block, Operation, OperationKind, OperationResult, StructuralOperationResult,
    StructuralPlaceDeclaration, TerminalMachineResult, Terminator, ValueDeclaration,
};

#[test]
fn decoded_source_array_constructor_returns_through_an_ordinary_call() {
    let tree = Sources::new();
    let root = tree.package("root");
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/omega/pass/modules/module_array_constant_indices");
    for filename in ["main.omg", "settings.omg"] {
        Sources::write(
            root.join(filename),
            &std::fs::read_to_string(fixture.join(filename)).unwrap(),
        );
    }
    let checked = compile(&root, root_inputs(&root));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "computed_row").unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    drop(lowered);
    drop(checked);
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    let callee = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(module.machines.len(), 1);
    assert_eq!(callee.parameters.len(), 1);
    assert!(callee.structural_parameters.is_empty());
    assert!(callee.contract.requires.is_empty());
    assert!(callee.contract.crash_routes.is_empty());
    let callee_id = callee.id;
    let scalar_type = callee.parameters[0].scalar_type;
    let mut caller = callee.clone();
    // The source fixture's identities remain untouched, including arithmetic
    // obligations. The consumer adds only a fresh caller with a total literal.
    caller.id = MachineId::new(10001).unwrap();
    caller.parameters.clear();
    caller.contract.id = ContractId::new(10001).unwrap();
    caller.contract.ensures.clear();
    caller.contract.outcome_specific_ensures.clear();
    let TerminalMachineResult::Structural(result) = &mut caller.result else {
        panic!("source array result");
    };
    result.place = PlaceId::new(10002).unwrap();
    let structural_type = result.structural_type;
    let value = ValueId::new(10001).unwrap();
    let call = OperationId::new(10002).unwrap();
    let returned = PlaceId::new(10001).unwrap();
    caller.structural_places = vec![
        StructuralPlaceDeclaration {
            id: returned,
            kind: StructuralPlaceKind::OperationResult {
                producer: call,
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: result.place,
            kind: StructuralPlaceKind::Result,
        },
    ];
    caller.entry = BlockId::new(10001).unwrap();
    caller.blocks = vec![Block {
        id: caller.entry,
        parameters: vec![],
        structural_parameters: vec![],
        operations: vec![
            Operation {
                id: OperationId::new(10001).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: value,
                    scalar_type,
                }),
                kind: OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(42),
                },
            },
            Operation {
                id: call,
                result: OperationResult::Structural(StructuralOperationResult {
                    place: returned,
                    structural_type,
                    multiplicity: result.multiplicity,
                    qualifications: vec![],
                    projected_qualifications: vec![],
                    claims: vec![],
                }),
                kind: OperationKind::CallStructuralWithScalarArguments {
                    callee: callee_id,
                    arguments: vec![value],
                    structural_arguments: vec![],
                    claim_transfers: vec![],
                    returned_claim_transfers: vec![],
                    requirement_obligations: vec![],
                    crash_continuations: vec![],
                },
            },
        ],
        terminator: Terminator::ReturnStructural {
            edge: EdgeId::new(10001).unwrap(),
            source: returned,
            returned_claims: vec![],
            trivial_affine_discards: vec![],
        },
    }];
    module.entry = caller.id;
    module.machines.push(caller);
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let execution = terminal_interpreter::interpret_terminal_artifact(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    let TerminalExecutionResult::ScalarArray(result) = execution else {
        panic!("call must return the payload, not an opaque place");
    };
    assert_eq!(result.value.structural_type, structural_type);
    assert_eq!(
        result.value.elements,
        [42, 9].map(|value| TerminalScalarValue::Integer {
            scalar_type: match scalar_type {
                semantic_vocabulary::ScalarType::Integer(integer) => integer,
                _ => panic!("byte"),
            },
            value: IntegerValue::Unsigned(value),
        })
    );
}
