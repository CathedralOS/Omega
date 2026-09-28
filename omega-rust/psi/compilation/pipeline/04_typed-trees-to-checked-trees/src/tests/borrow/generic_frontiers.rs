use super::checks::check_program;
use crate::borrow::view_link::{DeclarationLifetimeFrontier, declaration_lifetime_frontier};
use crate::tests::front_end::typed_program;

mod static_calls;

const CARRIERS: &str = r#"
    data DecodeResult<T> { case Invalid; case Sound(value: T); }
    data Remainder { bytes: &[u8]; }
    data Relayed<T> { value: T; remainder: Remainder; }
"#;

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
fn a_non_template_incomplete_frontier_is_not_deferred() {
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
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
    let diagnostics = crate::lower_typed_trees(program, &crate::CheckingRequest::settled())
        .expect_err("ordinary unresolved shape is not a type template");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("uses unresolved fixed-array length `unknown`")),
        "{diagnostics:#?}"
    );
}
