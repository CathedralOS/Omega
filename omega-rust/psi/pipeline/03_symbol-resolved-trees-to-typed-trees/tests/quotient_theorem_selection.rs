#[test]
fn quotient_request_retains_exact_resultless_theorem_machine_selection() {
    let typed = crate::front_end::typed_program_result(
        r#"
        data Representative { value: i32; }
        machine representative(value: Representative) -> Representative { value }
        machine representative_respects(left: Representative, right: Representative) {}
        machine wrapper(value: Representative) -> Representative {
            Quotient::lift<representative, representative_respects>(value)
        }
        "#,
    )
    .expect("an exact theorem-machine application should type");
    let request = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call) => call.quotient_operation.as_ref(),
            _ => None,
        })
        .expect("sealed quotient request");

    assert_eq!(
        typed
            .symbols
            .get(request.theorem_evidence[0].application.symbol)
            .kind,
        symbols::SymbolKind::State
    );
    assert_eq!(
        typed.symbols.name(
            typed
                .symbols
                .get(request.theorem_evidence[0].application.symbol)
                .parent,
        ),
        "representative_respects"
    );
}

#[test]
fn three_argument_lift_retains_canonical_congruence_then_transport_roles() {
    let typed = crate::front_end::typed_program_result(
        r#"
        data Representative { value: i32; }
        machine representative(value: Representative) -> Representative { value }
        machine representative_respects(left: Representative, right: Representative) {}
        machine representative_transports(value: Representative) {}
        machine wrapper(value: Representative) -> Representative {
            Quotient::lift<
                representative,
                representative_respects,
                representative_transports
            >(value)
        }
        "#,
    )
    .expect("the explicit transport form should type in canonical role order");
    let request = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call) => call.quotient_operation.as_ref(),
            _ => None,
        })
        .expect("sealed quotient request");

    assert_eq!(request.theorem_evidence.len(), 2);
    assert_eq!(
        request.theorem_evidence[0].role,
        typed_trees::expression::QuotientTheoremRole::Congruence
    );
    assert_eq!(
        request.theorem_evidence[1].role,
        typed_trees::expression::QuotientTheoremRole::ForwardPreconditionTransport
    );
}

#[test]
fn define_rejects_surplus_transport_selection() {
    let diagnostic = crate::front_end::typed_program_result(
        r#"
        data Representative { value: i32; }
        machine representative(value: Representative) -> Representative { value }
        machine representative_respects(left: Representative, right: Representative) {}
        machine representative_transports(value: Representative) {}
        machine wrapper(value: Representative) -> Representative {
            Quotient::define<
                representative,
                representative_respects,
                representative_transports
            >(value)
        }
        "#,
    )
    .expect_err("define has no transport theorem role");

    assert!(
        diagnostic.message.contains("exactly `F, Congruence`"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn quotient_request_rejects_a_conformance_as_theorem_selection() {
    let diagnostic = crate::front_end::typed_program_result(
        r#"
        data Representative { value: i32; }
        trait Respects {}
        RepresentativeRespect: Representative satisfies Respects {}
        machine representative(value: Representative) -> Representative { value }
        machine wrapper(value: Representative) -> Representative {
            Quotient::define<representative, RepresentativeRespect>(value)
        }
        "#,
    )
    .expect_err("conformance-shaped proof discovery must reject");

    assert!(
        diagnostic.message.contains("resultless theorem machine"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}
