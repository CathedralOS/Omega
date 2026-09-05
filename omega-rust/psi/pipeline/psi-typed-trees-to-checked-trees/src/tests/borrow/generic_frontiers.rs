use super::checks::check_program;
use crate::borrow::view_link::{DeclarationLifetimeFrontier, declaration_lifetime_frontier};

const CARRIERS: &str = r#"
    data DecodeResult<T> { case Invalid; case Sound(value: T); }
    data Remainder { bytes: &[u8]; }
    data Relayed<T> { value: T; remainder: Remainder; }
"#;

fn typed_program(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn declarations_retain_template_dependent_frontiers_for_unknown_and_nested_carriers() {
    for result in [
        "Value",
        "DecodeResult<Value>",
        "DecodeResult<Relayed<Value>>",
    ] {
        let source = format!(
            "{CARRIERS} trait Decode<Value> {{ machine decode(bytes: &[u8]) -> {result}; }}"
        );
        check_program(&source).unwrap_or_else(|diagnostics| panic!("{result}: {diagnostics:#?}"));
        let program = typed_program(&source);
        let declaration = &program.traits()[0];
        let signature = &program.trait_machine_signatures(declaration)[0];
        let parameter = program.trait_type_parameters(declaration)[0].symbol;
        assert_eq!(
            declaration_lifetime_frontier(&program, signature.return_type, &[parameter]),
            DeclarationLifetimeFrontier::TemplateDependent,
            "{result}"
        );
        assert_eq!(
            declaration_lifetime_frontier(&program, signature.return_type, &[]),
            DeclarationLifetimeFrontier::Incomplete,
            "a template never establishes a concrete frontier: {result}"
        );
    }
}

#[test]
fn template_parameters_are_exact_declaration_symbols_not_same_spelled_names() {
    let program = typed_program(&format!(
        r#"{CARRIERS}
        trait Decode<Value> {{ machine decode(bytes: &[u8]) -> DecodeResult<Relayed<Value>>; }}
        trait Other<Value> {{}}
    "#
    ));
    let signature = &program.trait_machine_signatures(&program.traits()[0])[0];
    let foreign = program.trait_type_parameters(&program.traits()[1])[0].symbol;
    assert_eq!(
        declaration_lifetime_frontier(&program, signature.return_type, &[foreign]),
        DeclarationLifetimeFrontier::Incomplete
    );
}

#[test]
fn concrete_decode_carrier_contract_keeps_access_and_ambiguity_checks() {
    let accepted = format!(
        r#"{CARRIERS}
        trait Decode<Value> {{ machine decode(bytes: &[u8]) -> DecodeResult<Relayed<Value>>; }}
        machine decode(bytes: &[u8]) -> DecodeResult<Relayed<i32>> {{ DecodeResult::Invalid }}
    "#
    );
    check_program(&accepted).expect("concrete read-only result has one source");
    let hostile = format!(
        r#"{CARRIERS}
        data Mutable {{ bytes: &mut [u8]; }}
        machine decode(bytes: &[u8]) -> DecodeResult<Relayed<Mutable>> {{ DecodeResult::Invalid }}
    "#
    );
    let diagnostics =
        check_program(&hostile).expect_err("shared input cannot supply mutable result");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("access cannot be supplied")),
        "{diagnostics:#?}"
    );
    let ambiguous = format!(
        r#"{CARRIERS}
        machine decode(left: &[u8], right: &[u8]) -> DecodeResult<Relayed<i32>> {{ DecodeResult::Invalid }}
    "#
    );
    let diagnostics = check_program(&ambiguous).expect_err("two inputs remain ambiguous");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("candidate ref inputs")),
        "{diagnostics:#?}"
    );
}

#[test]
fn unresolved_generic_receiver_calls_cannot_produce_or_discard_carriers() {
    for result in ["DecodeResult<Value>", "DecodeResult<Relayed<Value>>"] {
        for body in [
            "decoder.decode(bytes);",
            "let held: DecodeResult<i32> = decoder.decode(bytes);",
            "transition { _ -> decoder.decode(bytes) }",
        ] {
            let source = format!(
                r#"{CARRIERS}
                trait Decode<Value> {{ machine decode(&self, bytes: &[u8]) -> {result}; }}
                machine exercise<Decoder>(decoder: &Decoder, bytes: &[u8])
                where Decoder satisfies Decode<i32>
                {{ {body} }}
            "#
            );
            let diagnostics =
                check_program(&source).expect_err("raw requirement needs closed lifetime contract");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("template-dependent returned-carrier lifetime frontier")),
                "{result}, {body}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn concrete_decode_realization_and_call_are_checked_by_the_complete_pipeline() {
    let source = format!(
        r#"{CARRIERS}
        trait Decode<Value> {{ machine decode(bytes: &[u8]) -> DecodeResult<Relayed<Value>>; }}
        machine decode(bytes: &[u8]) -> DecodeResult<Relayed<i32>>
        satisfies Decode<i32>::decode
        {{ DecodeResult::Invalid }}
        machine consume(value: DecodeResult<Relayed<i32>>) {{}}
        machine exercise(bytes: &[u8]) {{
            let result: DecodeResult<Relayed<i32>> = decode(bytes);
            consume(result);
        }}
    "#
    );
    crate::lower_typed_trees(typed_program(&source))
        .unwrap_or_else(|diagnostics| panic!("closed implementation and call: {diagnostics:#?}"));
}

#[test]
fn a_non_template_incomplete_frontier_is_not_deferred() {
    use psi_typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    let mut program = typed_program(
        r#"
        data View { body: &[u8]; }
        trait Fixed { machine make(bytes: &[u8]) -> [View; 1]; }
    "#,
    );
    let signature = &program.trait_machine_signatures(&program.traits()[0])[0];
    let result = signature.return_type;
    let TypeReferenceNode::FixedArray { element_type, .. } =
        program.type_reference_table.type_reference(result).clone()
    else {
        panic!("array result");
    };
    program.type_reference_table.substitute_node(
        result,
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::ConstParameter {
                symbol: Default::default(),
                name: "unknown".into(),
            },
        },
    );
    assert_eq!(
        declaration_lifetime_frontier(&program, result, &[]),
        DeclarationLifetimeFrontier::Incomplete
    );
    let diagnostics = crate::lower_typed_trees(program)
        .expect_err("ordinary unresolved shape is not a type template");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("uses unresolved fixed-array length `unknown`")),
        "{diagnostics:#?}"
    );
}
