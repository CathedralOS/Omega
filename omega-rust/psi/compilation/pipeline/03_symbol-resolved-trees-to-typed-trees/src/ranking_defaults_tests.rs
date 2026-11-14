/* Default views belong to typed subject selection, not source-name guessing. */
use language_semantics::{RankingViewId, RankingWitness};
use typed_trees::TypedTrees;

fn resolve_and_type(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("ranking source lexes");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("ranking source parses");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .expect("ranking subjects resolve");
    for machine in resolved.machines.iter() {
        if let Some(witness) = &machine.termination_plan.implementation_witness
            && witness.view_path.is_empty()
        {
            assert_eq!(witness.ranking_view, RankingViewId::NULL);
        }
    }
    crate::lower_symbol_resolved_trees(&resolved).expect("ranking subjects type")
}

fn witness<'program>(program: &'program TypedTrees, name: &str) -> &'program RankingWitness {
    program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("named machine exists")
        .termination_plan
        .implementation_witness
        .as_ref()
        .expect("machine retains its ranking witness")
}

#[test]
fn ranking_defaults_follow_typed_carriers_and_selected_length() {
    let program = resolve_and_type(
        r#"
        data Counter { len: i32; }
        machine natural(n: u64) terminates by n {}
        machine signed(n: i32) terminates by n {}
        machine slice(items: &[u8]) terminates by items {}
        machine length(items: &[u8]) terminates by items.len {}
        machine field(counter: &Counter) terminates by counter.len {}
        machine explicit(n: i32) terminates by n -> Nat::Descending {}
        machine distance(index: u64, limit: u64) terminates by (index, limit) {}
        "#,
    );
    for (name, expected) in [
        ("natural", RankingViewId::NAT_DESCENDING),
        ("slice", RankingViewId::SLICE_LENGTH),
        ("length", RankingViewId::NAT_DESCENDING),
        ("explicit", RankingViewId::NAT_DESCENDING),
        ("distance", RankingViewId::NAT_BOUNDED_DISTANCE),
        ("signed", RankingViewId::NULL),
        ("field", RankingViewId::NULL),
    ] {
        let witness = witness(&program, name);
        assert_eq!(witness.ranking_view, expected, "{name}");
        assert_eq!(
            witness.view_path,
            expected.canonical_path().unwrap_or_default(),
            "{name}"
        );
    }
}

#[test]
fn ranking_defaults_preserve_exact_binders_and_bound_shell_traversal() {
    use typed_trees::types::TypeReferenceNode;

    let mut program = resolve_and_type("machine natural(n: u64) terminates by n {}");
    let mut machine = program.machines()[0].clone();
    let entry = program.machine_states(&machine)[0].clone();
    let parameter = program.state_parameters(&entry)[0].clone();
    let mut reference = parameter.type_reference;
    for _ in 0..140 {
        reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: reference,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            });
    }
    program.state_parameters.span_mut_or_empty(entry.parameters)[0].type_reference = reference;
    let settle = |program: &TypedTrees, machine: &mut typed_trees::machine::Machine| {
        let witness = machine
            .termination_plan
            .implementation_witness
            .as_mut()
            .unwrap();
        witness.ranking_view = RankingViewId::NULL;
        witness.view_path.clear();
        crate::lowerer::progress::settle_machine_ranking_default(
            program,
            &program.symbols,
            machine,
        );
        machine
            .termination_plan
            .implementation_witness
            .as_ref()
            .unwrap()
            .ranking_view
    };
    assert_eq!(
        settle(&program, &mut machine),
        RankingViewId::NAT_DESCENDING
    );

    let subject = program
        .ranking_expression_custody_for(machine.symbol)
        .unwrap()
        .subjects[0];
    let typed_trees::expression::ExpressionNode::Name(path) =
        program.expression_table.expression_mut(subject)
    else {
        panic!("the authored subject is a name");
    };
    path.symbol = machine.symbol;
    path.head_symbol = machine.symbol;
    assert_eq!(
        settle(&program, &mut machine),
        RankingViewId::NULL,
        "the same spelling cannot replace the parameter identity"
    );

    let typed_trees::expression::ExpressionNode::Name(path) =
        program.expression_table.expression_mut(subject)
    else {
        panic!("the authored subject is still a name");
    };
    path.symbol = parameter.symbol;
    path.head_symbol = parameter.symbol;
    let missing = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: Default::default(),
            access: language_core::ReferenceAccess::Shared,
            lifetime: None,
        });
    program.state_parameters.span_mut_or_empty(entry.parameters)[0].type_reference = missing;
    assert_eq!(
        settle(&program, &mut machine),
        RankingViewId::NULL,
        "a missing type supplies no canonical carrier"
    );
}
