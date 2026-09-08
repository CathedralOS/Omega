use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn input(program: &TypedTrees) -> TypeReferenceHandle {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    program.state_parameters(&program.machine_states(machine)[0])[0].type_reference
}

#[test]
fn primitive_ranges_inside_owned_records_do_not_introduce_cleanup() {
    let source =
        "data Limits { limit: u64; divisor: u64 [3..=5]; } machine inspect(limits: Limits) {}";
    let program = typed(source);
    assert!(!has_plain_owned_contents(&program, input(&program)));
    assert!(has_plain_owned_contents_with_numeric_constraints(
        &program,
        input(&program)
    ));
    let nested = typed(
        "data Inner { divisor: u64 [3..=5]; } data Outer { inner: Inner; } machine inspect(outer: Outer) {}",
    );
    assert!(!has_plain_owned_contents(&nested, input(&nested)));
    assert!(has_plain_owned_contents_with_numeric_constraints(
        &nested,
        input(&nested)
    ));
    for source in [
        format!("{source} machine Limits::drop(&mut self) {{}}"),
        "data Inner { divisor: u64 [3..=5]; } data Outer { inner: Inner; } machine inspect(outer: Outer) {} machine Inner::drop(&mut self) {}".to_owned(),
    ] {
        let program = typed(&source);
        assert!(!has_plain_owned_contents(&program, input(&program)));
        assert!(!has_plain_owned_contents_with_numeric_constraints(&program, input(&program)));
    }
}

#[test]
fn primitive_range_admission_does_not_erase_other_constraints() {
    let mut program = typed("machine inspect(value: u64 [3..=5]) {}");
    let reference = input(&program);
    assert!(!has_plain_owned_contents(&program, reference));
    assert!(has_plain_owned_contents_with_numeric_constraints(
        &program, reference
    ));
    let TypeReferenceNode::Constrained { base_type, .. } =
        *program.type_reference_table.type_reference(reference)
    else {
        panic!("range constraint");
    };
    let constraints = program.type_reference_table.insert_constraints([
        typed_trees::types::TypeConstraintNode::Named("atomic".into()),
    ]);
    let qualified = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type,
            constraints,
        });
    assert!(!has_plain_owned_contents(&program, qualified));
    assert!(!has_plain_owned_contents_with_numeric_constraints(
        &program, qualified
    ));
    let empty = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type,
            constraints: arena::HandleSpan::empty(),
        });
    assert!(!has_plain_owned_contents(&program, empty));
    assert!(!has_plain_owned_contents_with_numeric_constraints(
        &program, empty
    ));
}
