use super::*;
use symbol_resolved_trees::domain::ProofFact;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};

#[test]
fn operator_contracts_resolve_each_overloads_own_formal_parameters() {
    let source = r#"
        boundary operator [] Slice::read<Element>(items: &[Element], position: u64) -> Element
        requires position < items.len;
        boundary operator [] Bytes::read(items: &[u8], position: u64) -> u8
        requires position < items.len;
        data Buffer {}
        domain Buffer::Indexed;
        boundary operator [] Buffer::Indexed::read(items: &Buffer, position: u64) -> u8
        requires position >= 0;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve");
    let operators = program
        .operators
        .iter()
        .chain(program.domain_definitions.iter().flat_map(|domain| {
            program
                .tables
                .declarations
                .operator_definitions
                .span_or_empty(domain.operators)
        }))
        .collect::<Vec<_>>();
    assert_eq!(operators.len(), 3);
    let mut selected_parameters = Vec::new();
    for operator in operators {
        let parameters = program.state_parameters(operator.parameters);
        let mut names = Vec::new();
        for contract in program.signature_contracts(operator.contracts) {
            for fact in program.proof_facts(contract.facts) {
                let ProofFact::Expression(expression) = fact else {
                    panic!("expression contract");
                };
                collect_names(&program, *expression, &mut names);
            }
        }
        assert!(!names.is_empty());
        for expression in names {
            let ExpressionNode::Name(path) =
                program.tables.bodies.expressions.expression(expression)
            else {
                unreachable!();
            };
            let [name] = program
                .tables
                .bodies
                .expressions
                .name_path_members(path.members)
            else {
                panic!("one lexical formal name");
            };
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.name == *name)
                .expect("own formal");
            assert!(parameter.symbol.is_valid());
            assert_eq!(path.symbol, parameter.symbol);
            assert_eq!(path.head_symbol, parameter.symbol);
            assert!(
                !selected_parameters.contains(&parameter.symbol),
                "different operators cannot share a lexical formal"
            );
            selected_parameters.push(parameter.symbol);
        }
    }
    assert_eq!(selected_parameters.len(), 5);
}

fn collect_names(
    program: &symbol_resolved_trees::SymbolResolvedTrees,
    expression: ExpressionHandle,
    names: &mut Vec<ExpressionHandle>,
) {
    match program.tables.bodies.expressions.expression(expression) {
        ExpressionNode::Name(_) => names.push(expression),
        ExpressionNode::Binary(binary) => {
            collect_names(program, binary.left, names);
            collect_names(program, binary.right, names);
        }
        ExpressionNode::Member(member) => collect_names(program, member.receiver, names),
        ExpressionNode::Integer(_) => {}
        unexpected => panic!("unexpected contract operand: {unexpected:?}"),
    }
}
