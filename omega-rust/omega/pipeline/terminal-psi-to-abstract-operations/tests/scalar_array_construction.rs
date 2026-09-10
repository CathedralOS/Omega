//! Verified scalar-array construction retains exact payloads through current IR.

use abstract_operations::AbstractOperation;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_psi::OperationKind;
use terminal_psi_to_abstract_operations::{
    build_verified_psi_optimization_unit, lower_artifact_sections,
    lower_artifact_sections_for_native_realization, lower_artifact_sections_for_optimization,
};

#[test]
fn verified_scalar_array_constructors_retain_empty_and_material_payloads() {
    for (carrier, initializer) in [
        ("[u8; 2]", "[7, 9]"),
        ("[u8; 0]", "[]"),
        ("[[bool; 2]; 2]", "[[true, false], [false, true]]"),
        ("[[u8; 3]; 0]", "[]"),
        ("[[u8; 0]; 2]", "[[], []]"),
    ] {
        let source = format!(
            "data Sizes {{}} const Sizes::ROWS: [{carrier}; 1] = [{initializer}];
             machine selected() -> {carrier} {{ Sizes::ROWS[0] }}"
        );
        assert_array_retention(&source, "selected");
    }
}

#[test]
fn array_construction_composes_with_owned_calls_and_ordered_scalar_helpers() {
    let source = r"
        machine keep_row(row: [u8; 2]) -> [u8; 2] { row }
        machine answer_row(row: [u8; 2], value: u8) -> u8 { value }
        machine ordered_answer_row(value: u8) -> u8 {
            let row: [u8; 2] = [7u8, 9u8];
            answer_row(row, value)
        }
        machine forward_answer_row(value: u8) -> u8 { ordered_answer_row(value) }
        machine transitive_computation_row(value: u8) -> [u8; 2] {
            keep_row([forward_answer_row(value), 9u8])
        }
        machine selected(value: u8) -> [u8; 2] {
            let row: [u8; 2] = transitive_computation_row(value);
            keep_row(row)
        }
    ";
    assert_array_retention(source, "selected");
}

#[test]
fn array_result_calls_retain_borrowed_primitive_inputs_and_ordered_writes() {
    let source = r"
        machine row(value: &mut u8) -> [u8; 2] {
            value = 7u8;
            [value, 9u8]
        }
        machine selected() -> [u8; 2] {
            let mut value: u8 = 0u8;
            row(&mut value)
        }
    ";
    assert_array_retention(source, "selected");
}

fn assert_array_retention(source: &str, entry: &str) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize array source");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse array source");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve exact array selection");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type array source");
    let checked =
        typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check array source");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
        .expect("selected array reaches Terminal");
    let semantic = encode_module(&lowered.semantic_module).expect("encode array semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode array proof");
    let module = decode_module(&semantic).expect("decode array semantics");
    let decoded_proof = decode_proof_bundle(&proof).expect("decode array proof");
    let profile = AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &decoded_proof, &profile)
        .expect("array construction verifies independently before Omega lowering");
    let plan = lower_artifact_sections(&semantic, &proof, &profile)
        .expect("ordinary abstract lowering retains the verified array payload");
    let optimizer_input = lower_artifact_sections_for_optimization(&semantic, &proof, &profile)
        .expect("optimizer admission retains arrays");
    let native_input = lower_artifact_sections_for_native_realization(&semantic, &proof, &profile)
        .expect("native artifact admission retains arrays");
    assert_eq!(&plan, optimizer_input.plan());
    assert_eq!(&plan, native_input.plan());
    let unit = build_verified_psi_optimization_unit(
        optimizer_input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("reconstruct optimizer unit from canonical input");
    optimization_unit_semantics::validate_psi_optimization_unit(unit.unit())
        .expect("independent current-IR array validation");
    for (function_position, function) in unit.unit().functions.iter().enumerate() {
        for (block_position, block) in function.blocks.iter().enumerate() {
            for (node_position, node) in block.nodes.iter().enumerate() {
                let AbstractOperation::CallStructural {
                    structural_arguments,
                    ..
                } = &node.operation
                else {
                    continue;
                };
                if structural_arguments.is_empty() {
                    continue;
                }
                let mut changed = unit.unit().clone();
                let AbstractOperation::CallStructural {
                    structural_arguments,
                    ..
                } = &mut changed.functions[function_position].blocks[block_position].nodes
                    [node_position]
                    .operation
                else {
                    unreachable!()
                };
                structural_arguments[0].access = match structural_arguments[0].access {
                    terminal_psi::StructuralAccess::Owned => {
                        terminal_psi::StructuralAccess::MutableBorrow
                    }
                    _ => terminal_psi::StructuralAccess::Owned,
                };
                changed.identity =
                    optimization_unit::recompute_psi_optimization_unit_identity(&changed);
                assert!(
                    optimization_unit_semantics::validate_psi_optimization_unit(&changed).is_err(),
                    "array result cannot excuse an incorrect argument access"
                );
            }
        }
    }
    let constructors: Vec<_> = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::EstablishScalarArray { .. }))
        .collect();
    assert!(!constructors.is_empty());
    let retained: Vec<_> = plan
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .filter(|operation| matches!(operation, AbstractOperation::EstablishScalarArray { .. }))
        .collect();
    assert_eq!(retained.len(), constructors.len());
    for constructor in &constructors {
        let OperationKind::EstablishScalarArray { elements } = &constructor.kind else {
            unreachable!()
        };
        let expected = AbstractOperation::EstablishScalarArray {
            psi_operation: constructor.id,
            result: constructor
                .result
                .structural()
                .cloned()
                .expect("complete array result"),
            elements: elements.clone(),
        };
        assert!(retained.contains(&&expected));
        let node = unit
            .unit()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.nodes)
            .find(|node| node.operation == expected)
            .expect("same array constructor in optimizer");
        assert_eq!(
            node.uses
                .iter()
                .map(|operand| operand.value)
                .collect::<Vec<_>>(),
            *elements
        );
    }
    // Native payload transport and remaining ABI limits are exercised through
    // image publication in native-differential/tests/scalar_array_results.rs.
}
