//! Normalization must preserve lexical indices before ordinary kind checking.

use syntax_trees::{SyntaxTrees, types::TypeConstraintNode};

fn normalize(text: &str) -> SyntaxTrees {
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add("domain-indices/main.omg".into(), text.to_owned())
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        },
    )
    .unwrap()
}

fn result_index(syntax: &SyntaxTrees, name: &str) -> String {
    let machine = syntax
        .root_items()
        .filter_map(|item| item.as_machine())
        .find(|machine| machine.name.as_str(syntax) == name)
        .unwrap();
    let state = syntax
        .items
        .state(*syntax.items.state_handles(machine.states).first().unwrap());
    type_index(syntax, state.return_type)
}

fn type_index(syntax: &SyntaxTrees, reference: syntax_trees::types::TypeReferenceHandle) -> String {
    let syntax_trees::types::TypeReferenceNode::Constrained { constraints, .. } =
        syntax.type_references.type_reference(reference)
    else {
        panic!("indexed result");
    };
    let domain = syntax
        .type_references
        .constraints(*constraints)
        .iter()
        .find_map(|constraint| {
            if let TypeConstraintNode::Domain(domain) = constraint {
                Some(domain)
            } else {
                None
            }
        })
        .unwrap();
    let argument = syntax
        .type_references
        .type_reference_handles(domain.arguments)[0];
    syntax
        .type_references
        .type_reference(argument)
        .as_named()
        .unwrap()
        .as_str(syntax)
        .to_owned()
}

#[test]
fn domain_indices_keep_machine_binders_while_closing_independent_globals() {
    for binder in ["Count: u64", "const Count: u64"] {
        let text = r#"
        const Count: u64 = 3;
        domain<Carrier, const Index: u64> Carrier::Tagged<Index>;
        machine selected<BINDER>() -> u64 in Tagged<Count> {
            Count as u64 in Tagged<Count>
        }
        machine independent() -> u64 in Tagged<Count> {
            3 as u64 in Tagged<Count>
        }
    "#
        .replace("BINDER", binder);
        let syntax = normalize(&text);
        assert_eq!(result_index(&syntax, "selected"), "Count");
        assert_eq!(result_index(&syntax, "independent"), "3");
    }
}

#[test]
fn domain_indices_keep_machine_contract_header_binders() {
    let syntax = normalize(
        r#"
        const Count: u64 = 3;
        domain<Carrier, const Index: u64> Carrier::Tagged<Index>;
        machine selected<Count: u64, machine Callback>()
        where machine Callback(value: u64 in Tagged<Count>) -> u64;
        { }
        machine independent<machine Callback>()
        where machine Callback(value: u64 in Tagged<Count>) -> u64;
        { }
    "#,
    );
    for (owner, expected) in [("selected", "Count"), ("independent", "3")] {
        let machine = syntax
            .root_items()
            .filter_map(|item| item.as_machine())
            .find(|machine| machine.name.as_str(&syntax) == owner)
            .unwrap();
        let signature = syntax
            .items
            .type_parameters(machine.type_parameters)
            .iter()
            .find_map(|parameter| {
                if let syntax_trees::item::TypeParameterKind::Machine {
                    contract:
                        Some(syntax_trees::item::MachineParameterContract::Structural(signature)),
                } = &parameter.kind
                {
                    Some(signature)
                } else {
                    None
                }
            })
            .unwrap();
        let parameter = syntax.items.state_parameters(signature.parameters)[0];
        assert_eq!(
            type_index(
                &syntax,
                syntax.items.state_parameter(parameter).type_reference
            ),
            expected
        );
    }
}

#[test]
fn domain_indices_do_not_turn_conformance_evidence_into_a_global_const() {
    let syntax = normalize(
        r#"
        const Evidence: u64 = 3;
        domain<Carrier, const Index: u64> Carrier::Tagged<Index>;
        trait Probe<Kind> { machine probe(value: &Kind) -> u64; }
        machine selected<Kind, Evidence satisfies Probe<Kind>>() -> u64 in Tagged<Evidence> {
            7 as u64 in Tagged<3>
        }
        machine independent() -> u64 in Tagged<Evidence> { 3 as u64 in Tagged<Evidence> }
    "#,
    );
    // Syntax normalization retains the invalid evidence use for the ordinary
    // kind checker instead of replacing it with an unrelated valid constant.
    assert_eq!(result_index(&syntax, "selected"), "Evidence");
    assert_eq!(result_index(&syntax, "independent"), "3");
}

#[test]
fn domain_indices_keep_runtime_parameter_identity() {
    let syntax = normalize(
        r#"
        const Count: u64 = 3;
        domain<Carrier, const Index: u64> Carrier::Tagged<Index>;
        machine selected(Count: u64) -> u64 in Tagged<Count> { Count as u64 in Tagged<Count> }
        machine independent() -> u64 in Tagged<Count> { 3 as u64 in Tagged<Count> }
    "#,
    );
    assert_eq!(result_index(&syntax, "selected"), "Count");
    assert_eq!(result_index(&syntax, "independent"), "3");
}

#[test]
fn domain_indices_respect_local_binding_introduction() {
    let syntax = normalize(
        r#"
        const Count: u64 = 3;
        domain<Carrier, const Index: u64> Carrier::Tagged<Index>;
        machine selected() {
            let before: u64 in Tagged<Count> = 3 as u64 in Tagged<Count>;
            let Count: u64 = 7;
            let after: u64 in Tagged<Count> = 7 as u64 in Tagged<Count>;
        }
    "#,
    );
    let machine = syntax
        .root_items()
        .find_map(|item| item.as_machine())
        .unwrap();
    let state = syntax
        .items
        .state(syntax.items.state_handles(machine.states)[0]);
    for (ordinal, expected) in [(0, "3"), (2, "Count")] {
        let statement = syntax.items.statements(state.statements)[ordinal];
        let syntax_trees::statement::StatementNode::LocalData(local) =
            syntax.statements.statement(statement)
        else {
            panic!("local declaration");
        };
        assert_eq!(type_index(&syntax, local.type_reference), expected);
    }
}
