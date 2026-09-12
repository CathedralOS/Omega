use super::*;

const INTERLEAVED_FAMILIES: &str = r#"
    machine apply<T, machine First, const Count: u64, machine Second>(value: u64) -> u64
    where machine First<const Number: u64>(value: u64) -> u64;
    where machine Second<const Number: u64>(value: u64) -> u64;
    { value }
"#;

fn resolved(source: &str) -> resolved::SymbolResolvedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolution")
}

fn assert_telescope(
    source: &resolved::SymbolResolvedTrees,
    typed: &typed::TypedTrees,
    source_span: arena::HandleSpan<resolved::data::TypeParameter>,
    typed_span: arena::HandleSpan<typed::data::TypeParameter>,
    symbols: &mut Vec<symbols::SymbolHandle>,
) {
    let source_parameters = source.data_type_parameters(source_span);
    let typed_parameters = typed.data_type_parameters.span_or_empty(typed_span);
    assert_eq!(source_parameters.len(), typed_parameters.len());
    assert_eq!(typed_span.len(), typed_parameters.len());
    for (source_parameter, typed_parameter) in source_parameters.iter().zip(typed_parameters) {
        assert_eq!(source_parameter.symbol, typed_parameter.symbol);
        assert_eq!(
            source_parameter.name.as_str(),
            typed_parameter.name.as_str()
        );
        assert!(
            !symbols.contains(&typed_parameter.symbol),
            "nested and sibling binders must stay distinct"
        );
        symbols.push(typed_parameter.symbol);
        match (&source_parameter.kind, &typed_parameter.kind) {
            (
                resolved::data::TypeParameterKind::Machine {
                    contract: resolved::data::MachineParameterContract::Structural(source_signature),
                },
                typed::data::TypeParameterKind::Machine {
                    contract: typed::data::MachineParameterContract::Structural(typed_signature),
                },
            ) => {
                assert_telescope(
                    source,
                    typed,
                    source_signature.type_parameters,
                    typed_signature.type_parameters,
                    symbols,
                );
            }
            (resolved::data::TypeParameterKind::Type, typed::data::TypeParameterKind::Type)
            | (
                resolved::data::TypeParameterKind::Const { .. },
                typed::data::TypeParameterKind::Const { .. },
            ) => {}
            _ => panic!("binder kind changed"),
        }
    }
}

#[test]
fn nested_callable_telescope_siblings_keep_contiguous_spans_and_exact_symbols() {
    for source in [
        INTERLEAVED_FAMILIES,
        r#"
        machine apply<machine Schema, machine Tail>(value: u64) -> u64
        where machine Schema<machine First, machine Second>(value: u64) -> u64
        where machine First<const Number: u64>(value: u64) -> u64;
        where machine Second<const Number: u64>(value: u64) -> u64;
        where machine Tail<const Number: u64>(value: u64) -> u64;
        { value }
    "#,
    ] {
        let resolved = resolved(source);
        let typed =
            crate::lower_symbol_resolved_trees(&resolved).expect("nested sibling telescopes");
        let source_machine = resolved
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "apply")
            .unwrap();
        let typed_machine = typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == source_machine.symbol)
            .unwrap();
        let mut symbols = Vec::new();
        assert_telescope(
            &resolved,
            &typed,
            source_machine.type_parameters,
            typed_machine.type_parameters,
            &mut symbols,
        );
        assert!(symbols.len() >= 6);
    }
}

#[test]
fn nested_callable_telescope_references_exact_outer_type_before_parent_publication() {
    let source = INTERLEAVED_FAMILIES.replace("(value: u64) -> u64", "(value: T) -> T");
    let resolved = resolved(&source);
    let typed =
        crate::lower_symbol_resolved_trees(&resolved).expect("outer type in nested contracts");
    let source_machine = resolved
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .unwrap();
    let outer_type = resolved.data_type_parameters(source_machine.type_parameters)[0].symbol;
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_machine.symbol)
        .unwrap();
    let parameters = typed.machine_type_parameters(machine);
    assert_eq!(parameters[0].symbol, outer_type);
    for position in [1, 3] {
        let typed::data::TypeParameterKind::Machine {
            contract: typed::data::MachineParameterContract::Structural(signature),
        } = &parameters[position].kind
        else {
            panic!("nested callable");
        };
        let input = typed.state_signature_parameters(signature)[0].type_reference;
        for reference in [input, signature.return_type] {
            assert!(
                matches!(typed.type_reference_table.type_reference(reference),
                typed::types::TypeReferenceNode::Named { symbol, .. } if *symbol == outer_type),
                "nested signature must reference the exact outer T, not a sibling binder"
            );
        }
    }
}

#[test]
fn nested_callable_telescope_still_rejects_an_unresolved_sibling_contract() {
    let mut resolved = resolved(INTERLEAVED_FAMILIES);
    let parameters = resolved
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .unwrap()
        .type_parameters;
    resolved
        .tables
        .declarations
        .data_type_parameters
        .span_mut_or_empty(parameters)[3]
        .kind = resolved::data::TypeParameterKind::Machine {
        contract: resolved::data::MachineParameterContract::AuthoredNominal {
            requirement: Vec::new(),
        },
    };
    let diagnostics = crate::lower_symbol_resolved_trees(&resolved)
        .expect_err("unresolved contract is not an empty telescope");
    assert!(
        format!("{diagnostics:?}").contains("unresolved nominal machine-parameter requirement")
    );
}
