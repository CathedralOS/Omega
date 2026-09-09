//! Array result payloads survive both source call lowering and independently
//! authored Terminal callers. Decode drops all producer-owned execution state.

use super::array_construction::{assert_decoded_array, integer};
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

fn execute_source(
    source: &str,
    dimensions: &[u64],
    expected: &[TerminalScalarValue],
    leaf: semantic_vocabulary::ScalarType,
) {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(root.join("main.omg"), source);
    let checked = compile(&root, root_inputs(&root));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "read")
        .unwrap_or_else(|error| panic!("{source}: {error:?}"));
    let semantics = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    drop(checked);
    drop(lowered);
    assert_decoded_array(&semantics, &proof, &[], dimensions, expected, leaf);
}

fn byte_type() -> semantic_vocabulary::ScalarType {
    semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .unwrap(),
    )
}

#[test]
fn empty_array_results_can_be_ignored_without_affine_cleanup() {
    execute_source(
        "machine read() -> [u8; 1] { touch(); [9] }
         machine touch() { let row: [u8; 0] = empty(); _ = empty(); }
         machine empty() -> [u8; 0] { [] }",
        &[1],
        &[integer(9, 8)],
        byte_type(),
    );
}

#[test]
fn array_call_scalar_operands_execute_once_in_authored_order() {
    execute_source(
        "machine read() -> [u8; 4] {
             let mut slot: u8 = 1;
             make(slot, stamp(&mut slot, 7), slot, stamp(&mut slot, 9))
         }
         machine make(first: u8, second: u8, third: u8, fourth: u8) -> [u8; 4] {
             [first, second, third, fourth]
         }
         machine stamp(slot: &mut u8, value: u8) -> u8 { slot = value; value }",
        &[4],
        &[1, 7, 7, 9].map(|value| integer(value, 8)),
        byte_type(),
    );
}

#[test]
fn array_call_result_keeps_callee_numeric_requirements() {
    execute_source(
        "machine read() -> [u8; 1] { bounded(7u8 + 2u8) }
         machine bounded(value: u8 [0..=9]) -> [u8; 1] { [value + 1u8] }",
        &[1],
        &[integer(10, 8)],
        byte_type(),
    );
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine read() -> [u8; 1] { bounded(10) }
         machine bounded(value: u8 [0..=9]) -> [u8; 1] { [value + 1u8] }",
    );
    assert!(
        super::compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .is_err(),
        "callee requirement is not proved by the result shape"
    );
}

#[test]
fn source_array_calls_compose_with_locals_calls_and_discarded_values() {
    for body in [
        "make(7u8 + 2u8)",
        "let row: [u8; 2] = make(7u8 + 2u8); row",
        "touch(); let row: [u8; 2] = make(9); touch(); row",
        "let row: [u8; 2] = make(9); let ignored: [u8; 2] = make(3); row",
        "let ignored: [u8; 2] = [1, 2]; let row: [u8; 2] = make(9); row",
        "let row: [u8; 2] = make(9); let ignored: [u8; 2] = [1, 2]; row",
        "let mut value: u8 = 2; value = 9; let row: [u8; 2] = make(value); value = 3; row",
        "_ = make(3); make(9)",
    ] {
        // The caller precedes its callee: closure availability cannot depend on
        // declaration order, and same-typed later results cannot replace row.
        execute_source(
            &format!(
                "machine read() -> [u8; 2] {{ {body} }} machine make(value: u8) -> [u8; 2] {{ [value, 7] }} machine touch() {{}}"
            ),
            &[2],
            &[integer(9, 8), integer(7, 8)],
            byte_type(),
        );
    }
}

#[test]
fn source_array_calls_keep_nested_scalar_argument_evaluation() {
    execute_source(
        "machine read() -> [u8; 2] { make(identity(7u8 + 2u8), identity(3)) }
         machine identity(value: u8) -> u8 { value }
         machine make(left: u8, right: u8) -> [u8; 2] { [left, right] }",
        &[2],
        &[integer(9, 8), integer(3, 8)],
        byte_type(),
    );
}

#[test]
fn source_array_calls_retain_payload_across_mutating_calls() {
    execute_source(
        "machine read() -> [u8; 2] {
            let mut slot: u8 = 1;
            let row: [u8; 2] = make(&mut slot, 9);
            stamp(&mut slot, 3);
            row
         }
         machine make(slot: &mut u8, value: u8) -> [u8; 2] { slot = value; [slot, 7] }
         machine stamp(slot: &mut u8, value: u8) { slot = value; }",
        &[2],
        &[integer(9, 8), integer(7, 8)],
        byte_type(),
    );
}

#[test]
fn source_array_calls_preserve_nested_boolean_and_empty_shapes() {
    for (result_type, literal, dimensions, values, leaf) in [
        (
            "[[u8; 2]; 2]",
            "[[1u8, 2u8], [3u8, 4u8]]",
            vec![2, 2],
            (1..=4).map(|value| integer(value, 8)).collect::<Vec<_>>(),
            byte_type(),
        ),
        (
            "[[bool; 0]; 2]",
            "[[], []]",
            vec![2, 0],
            vec![],
            semantic_vocabulary::ScalarType::Boolean,
        ),
        ("[[u8; 3]; 0]", "[]", vec![0, 3], vec![], byte_type()),
        (
            "[bool; 2]",
            "[true, false]",
            vec![2],
            vec![
                TerminalScalarValue::Boolean(true),
                TerminalScalarValue::Boolean(false),
            ],
            semantic_vocabulary::ScalarType::Boolean,
        ),
    ] {
        execute_source(
            &format!(
                "machine read() -> {result_type} {{ let row: {result_type} = middle(); row }} machine middle() -> {result_type} {{ make() }} machine make() -> {result_type} {{ {literal} }}"
            ),
            &dimensions,
            &values,
            leaf,
        );
    }
}

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
