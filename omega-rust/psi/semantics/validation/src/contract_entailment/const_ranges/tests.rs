use super::*;
use typed_trees::expression::StaticSymbolApplication;
use typed_trees::name::Identifier;
use typed_trees::statement::{StatementNode, TableCall};

fn fixture() -> (TypedTrees, SymbolHandle, TableCall) {
    let tokens = source_files_to_tokens::Lexer::new(
        "machine consume<T, const Enabled: bool, const N: u64>(value: u64[0..=N]) {}
         machine forward(value: u64[0..=2]) { consume<u8, true, 2>(value); }",
    )
    .tokenize()
    .expect("mixed static slots tokenize");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("mixed static slots parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("mixed static slots resolve");
    let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("mixed static slots type");
    let caller = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("caller");
    let state = &program.machine_states(caller)[0];
    let call = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| {
            if let StatementNode::Call(call) = statement {
                Some(call.clone())
            } else {
                None
            }
        })
        .expect("ordinary call");
    let symbol = caller.symbol;
    (program, symbol, call)
}

fn receives(program: &TypedTrees, caller: SymbolHandle, call: &TableCall) -> Vec<Diagnostic> {
    let caller = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller)
        .expect("exact caller");
    let mut diagnostics = Vec::new();
    validate_const_range_call(
        program,
        caller,
        Some(&program.machine_states(caller)[0]),
        call.target_symbol,
        &call.machine_arguments,
        program.statement_table.expression_handles(call.arguments),
        &mut diagnostics,
    );
    diagnostics
}

#[test]
fn mixed_static_slots_preserve_noninteger_const_positions() {
    let (program, caller, call) = fixture();
    assert_eq!(call.machine_arguments.len(), 3);
    assert!(receives(&program, caller, &call).is_empty());
    let mut swapped = call.clone();
    swapped.machine_arguments.swap(1, 2);
    assert!(
        !receives(&program, caller, &swapped).is_empty(),
        "numeric actual cannot occupy the Boolean const slot"
    );
}

#[test]
fn decorated_and_unknown_const_slots_cannot_be_replaced_by_inference() {
    let (program, caller, call) = fixture();
    assert!(receives(&program, caller, &call).is_empty());
    for ordinal in [1, 2] {
        let mut decorated = call.clone();
        decorated.machine_arguments[ordinal].application =
            Some(Box::new(StaticSymbolApplication {
                lifetime_arguments: Box::default(),
                arguments: Box::default(),
            }));
        assert!(
            !receives(&program, caller, &decorated).is_empty(),
            "decorated const slot {ordinal} must reject"
        );
    }
    let mut unknown = call.clone();
    unknown.machine_arguments[2].const_literal = None;
    unknown.machine_arguments[2].symbol = SymbolHandle::invalid();
    unknown.machine_arguments[2].path =
        vec![Identifier::generated_static("unknown")].into_boxed_slice();
    assert!(
        !receives(&program, caller, &unknown).is_empty(),
        "authored unknown N must not be inferred as 2"
    );
}

#[test]
fn additional_symbolic_range_cannot_hide_from_single_range_receiving() {
    let (mut program, caller, call) = fixture();
    assert!(receives(&program, caller, &call).is_empty());
    let (callee, entry) =
        crate::transitions::resolved_transition_target_state(&program, call.target_symbol)
            .expect("callee");
    let callee_symbol = callee.symbol;
    let parameter = program.state_parameters(entry)[0].clone();
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
        .clone()
    else {
        panic!("ranged parameter");
    };
    let original = program
        .type_reference_table
        .constraints(constraints)
        .to_vec();
    let mut repeated = arena::HandleSpan::empty();
    for constraint in original.iter().chain(&original) {
        program
            .type_reference_table
            .push_constraint(&mut repeated, constraint.clone());
    }
    let replacement = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type,
            constraints: repeated,
        });
    let callee = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == callee_symbol)
        .expect("callee");
    let parameters = program.machine_states(callee)[0].parameters;
    program.state_parameters.span_mut_or_empty(parameters)[0].type_reference = replacement;
    assert!(
        !receives(&program, caller, &call).is_empty(),
        "multiple const ranges need full receiving normalization, never a skipped check"
    );
}
