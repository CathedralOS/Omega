use super::{
    evaluate_const_arguments, evaluate_constant_initializers_with_source_consent,
    validate_constant_evaluations,
};
use crate::evaluation::{PreCheckEvaluation, SourceCompletionAccounting};
use crate::source_state::TypedTrees;
use crate::source_state::data::DataMember;
use crate::source_state::types::{FixedArrayLength, TypeReferenceNode};
use crate::stages::frontend_fixture;
use source_completion_accounting::{PureEvaluationRecord, PureTerminalEvaluationOutcome};
use source_provenance::DependencyScope;
use std::sync::Arc;

fn typed_source(text: &str) -> TypedTrees {
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x52; 32]).unwrap();
    let mut sources = crate::source_inputs::SourceMap::default();
    let source = sources
        .add_checked_instance(
            "constants/main.omg".into(),
            text,
            "constants".into(),
            Some(package),
            source_admission::SourceOrigin::User,
            source_admission::SourceResolutionStratum::Base,
            DependencyScope::Product,
        )
        .source_id;
    let tokens = frontend_fixture::Lexer::new(text).tokenize().unwrap();
    let syntax = frontend_fixture::parse_syntax_trees_with_id(source, &tokens).unwrap();
    let mut request = crate::resolution::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    let resolved = crate::resolution::resolve(request).unwrap();
    let mut typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
    PreCheckEvaluation::defer_declared_requests(&mut typed).unwrap();
    typed
}

fn request(entries: &[symbols::SymbolHandle]) -> crate::production::TerminalSourceCompilation<'_> {
    crate::production::TerminalSourceCompilation {
        entries,
        subjects: &[],
        purpose: DependencyScope::Product,
        declaration_metadata: &[],
        selected: &[],
        bindings: &[],
        calling: None,
        providers: None,
        supplies: &[],
    }
}

#[test]
fn a_computed_constant_uses_its_source_receipt_only_when_reached() {
    let mut typed = typed_source(
        "const VALUE: u32 = calc(); const UNREAD: u32 = spare();
        machine spare() -> u32 crashes none { 99 } machine calc() -> u32 crashes none { 17 }
        machine main() -> u32 crashes none { VALUE }",
    );
    let entry = typed.realized_machine_named("main").unwrap().symbol;
    let mut accounting = SourceCompletionAccounting::default();
    let demanded = evaluate_constant_initializers_with_source_consent(
        &mut typed,
        request(&[entry]),
        &[],
        None,
        None,
        &mut accounting,
    )
    .unwrap();
    assert_eq!(demanded.len(), 1);
    assert!(
        typed
            .const_declaration_by_symbol(demanded[0])
            .unwrap()
            .evaluated_value
            .is_some()
    );
    validate_constant_evaluations(&typed, &accounting).unwrap();
    let [PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
        panic!("one actual invocation")
    };
    assert!(matches!(
        receipt.outcome(),
        PureTerminalEvaluationOutcome::Materialized {
            value: build_time_values::BuildTimeValue::Int(17),
            ..
        }
    ));
    assert_eq!(
        validate_constant_evaluations(&typed, &Default::default()).unwrap_err(),
        "constant result has no accepted invocation receipt"
    );
}

#[test]
fn only_a_reader_of_an_array_requests_its_extent() {
    let base = typed_source(
        "machine extent() -> u64 { 4u64 } data Packet { bytes: [u8; extent()]; }
        machine reader(packet: @Packet) -> u64 { 0u64 } machine unrelated() -> u64 { 0u64 }",
    );
    for (entry, computes_extent) in [("reader", true), ("unrelated", false)] {
        let mut typed = base.clone();
        let entry = typed.realized_machine_named(entry).unwrap().symbol;
        let mut accounting = SourceCompletionAccounting::default();
        evaluate_const_arguments(&mut typed, request(&[entry]), None, None, &mut accounting)
            .unwrap();
        assert_eq!(accounting.receipts().len(), usize::from(computes_extent));
        accounting.validate_charges().unwrap();
        let packet = typed.data_definition_by_name_str("Packet").unwrap();
        let Some([DataMember::Field(bytes)]) = typed.data_members(packet).as_array() else {
            panic!("the original array field")
        };
        let array = typed
            .type_reference_table
            .type_reference(bytes.type_reference);
        assert_eq!(
            matches!(
                array,
                TypeReferenceNode::FixedArray {
                    length: FixedArrayLength::Literal(4),
                    ..
                }
            ),
            computes_extent
        );
    }
}
