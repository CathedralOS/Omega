use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan};
use checked_trees_to_lowered_psi::{LoweringError, lower_machine};
use proof_admission::AdmissionProfile;

fn checked_siblings() -> checked_trees::CheckedTrees {
    let source = "data Main {} machine Main::run() {}
        data Value { number: u64; }
        machine forward(value: Value) -> Value { value }
        machine Main::consume_pair(left: Value, right: Value) {}
        machine Main::caller(left: Value, right: Value) {
            Main::consume_pair(forward(left), forward(right));
        }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check sibling operands")
}

#[test]
fn reordered_sibling_producers_with_repaired_bindings_reject_authored_execution_order() {
    let mut checked = checked_siblings();
    let mut lowered = lower_machine(&checked, "Main::caller").expect("authored siblings lower");
    let caller_state = lowered.source_call_occurrences[0].source_state;
    assert_eq!(
        lowered
            .source_call_occurrences
            .iter()
            .map(|call| (call.statement_index, call.call_ordinal))
            .collect::<Vec<_>>(),
        [(0, 1), (0, 2), (0, 0)]
    );
    let entry = lowered.semantic_module.entry;
    let caller = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .expect("Terminal caller");
    assert_eq!(caller.blocks[0].operations.len(), 3);
    caller.blocks[0].operations.swap(0, 1);
    // Each independent producer retains its source parameter, result place,
    // and operation identity. The consumer still receives left then right.
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("reordered siblings preserve Terminal ownership and dependencies");

    let caller = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| machine.state == caller_state)
        .expect("checked caller");
    assert_eq!(caller.operations.len(), 4);
    caller.operations.swap(0, 1);
    for (binding_ordinal, operation) in caller.operations[..2].iter_mut().enumerate() {
        let CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            result,
            structural_arguments,
            discard_result_on_return,
            ..
        } = operation
        else {
            panic!("two ordinary sibling producers");
        };
        let original_binding = 1 - binding_ordinal as u32;
        assert_eq!(result.binding_ordinal, original_binding);
        assert_eq!(coordinate.statement_index, 0);
        assert_eq!(coordinate.call_ordinal, original_binding + 1);
        assert_eq!(structural_arguments.len(), 1);
        assert_eq!(
            structural_arguments[0].source,
            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: original_binding,
            }
        );
        assert!(!*discard_result_on_return);
        result.binding_ordinal = binding_ordinal as u32;
    }
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        structural_arguments,
        ..
    } = &mut caller.operations[2]
    else {
        panic!("enclosing Unit consumer follows both siblings");
    };
    assert_eq!(coordinate.statement_index, 0);
    assert_eq!(coordinate.call_ordinal, 0);
    assert_eq!(structural_arguments.len(), 2);
    for (position, argument) in structural_arguments.iter_mut().enumerate() {
        assert_eq!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: position as u32,
            }
        );
        argument.source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 1 - position as u32,
        };
    }
    // Bindings are dense in execution order; both results precede their one
    // consuming use, which still names the exact authored argument expression.
    // Source coordinates and captured expressions deliberately remain intact.
    assert_eq!(
        lower_machine(&checked, "Main::caller").expect_err("authored sibling order must reject"),
        LoweringError::Unsupported(
            "nested structural operations disagree with authored argument execution order"
        )
    );
}
