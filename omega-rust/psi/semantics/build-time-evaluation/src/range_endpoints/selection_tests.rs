use super::*;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::{path::PathBuf, sync::Arc};

struct Selection {
    retained_requester: PackageKeyIdentity,
    deny_retained_requester: bool,
}

impl crate::BuildTimeSelectionAuthority for Selection {
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        _: PackageKeyIdentity,
    ) -> bool {
        !self.deny_retained_requester || requester != self.retained_requester
    }

    fn package_label(&self, package: PackageKeyIdentity) -> String {
        if package == self.retained_requester {
            "retained-requester".to_owned()
        } else {
            "callee-owner".to_owned()
        }
    }
}

fn program_with_retained_requester(text: &str) -> (TypedTrees, source::SourceSpan, Selection) {
    let owner = PackageKeyIdentity::from_digest([0x71; 32]).unwrap();
    let requester = PackageKeyIdentity::from_digest([0x72; 32]).unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("owner/main.omg"),
            text.to_owned(),
            PathBuf::from("owner"),
            Some(owner),
            source::SourceOrigin::User,
        )
        .source_id;
    let retained_source = sources
        .add_with_metadata(
            PathBuf::from("requester/retained.omg"),
            "Limits".to_owned(),
            PathBuf::from("requester"),
            Some(requester),
            source::SourceOrigin::User,
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        Arc::new(sources),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    (
        program,
        source::SourceSpan::new(retained_source, source::Span::new(0, 6)),
        Selection {
            retained_requester: requester,
            deny_retained_requester: true,
        },
    )
}

fn retain_qualifier_selection(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    source_span: source::SourceSpan,
) {
    let limits = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Limits::capacity")
        .unwrap()
        .attached_data_symbol;
    // Model an occurrence retained through an earlier expression copy. Its
    // requester remains distinct from the current invocation and callee owner.
    let occurrence = program
        .record_resolved_authored_declaration_selection_once(
            source_span,
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::StaticPathSegment,
            limits,
        )
        .unwrap();
    program
        .expression_table
        .attach_authored_selection_occurrences(expression, [occurrence]);
}

#[test]
fn own_call_and_qualifier_selections_reject_before_body_evaluation() {
    for body in ["256", "1u64 / 0"] {
        for attach_to_receiver in [false, true] {
            let (mut program, retained_source, authority) =
                program_with_retained_requester(&format!(
                    "data Limits {{}} machine Limits::capacity() -> u64 {{ {body} }}
                     machine bounded(value: u64[0..=Limits::capacity() + 1]) {{}}"
                ));
            let call = pending_endpoints(&program).unwrap().remove(0);
            let selected = if attach_to_receiver {
                let ExpressionNode::Call(node) =
                    program.expression_table.expression(call.expression)
                else {
                    panic!("authored call");
                };
                node.receiver
            } else {
                call.expression
            };
            assert!(selected.is_valid());
            retain_qualifier_selection(&mut program, selected, retained_source);
            let errors = evaluate_const_range_endpoints_with_authority(
                &mut program,
                Some(Arc::new(authority)),
            )
            .expect_err("callee authority cannot authorize a retained qualifier occurrence");
            assert_eq!(errors.len(), 1, "{errors:?}");
            assert!(
                errors[0].message.contains("retained-requester")
                    && errors[0]
                        .message
                        .contains("without direct dependency authority"),
                "{body}: {errors:?}"
            );
            assert!(matches!(
                program.expression_table.expression(call.expression),
                ExpressionNode::Call(_)
            ));
        }
    }
}

#[test]
fn normalized_arguments_check_the_original_nested_occurrence_requester() {
    let (mut original, retained_source, mut authority) = program_with_retained_requester(
        "data Limits {} machine Limits::capacity() -> u64 {256}
         machine endpoint(value: u64) -> u64 {value}
         machine bounded(value: u64[0..=endpoint(Limits::capacity())]) {}",
    );
    let calls = pending_endpoints(&original).unwrap();
    assert_eq!(calls.len(), 2);
    let inner = &calls[0];
    let outer = &calls[1];
    retain_qualifier_selection(&mut original, inner.expression, retained_source);
    let mut evaluated = original.clone();
    *evaluated.expression_table.expression_mut(inner.expression) = ExpressionNode::Integer(
        IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "256")
            .unwrap()
            .with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U64,
                domain: ArithmeticDomain::Exact,
            }),
    );
    let error = arguments::evaluate(
        &evaluated,
        &original,
        outer.expression,
        outer.machine,
        Some(&authority),
    )
    .expect_err("an allowed outer call cannot authorize the original nested occurrence");
    assert!(error.contains("retained-requester"), "{error}");
    assert!(
        error.contains("without direct dependency authority"),
        "{error}"
    );

    authority.deny_retained_requester = false;
    let (values, _) = arguments::evaluate(
        &evaluated,
        &original,
        outer.expression,
        outer.machine,
        Some(&authority),
    )
    .expect("the original call still resolves when its retained requester is authorized");
    assert_eq!(values, vec![crate::BuildTimeValue::Int(256)]);
}
