//! Source formation keeps an integer width's actual const binder. A declared
//! carrier contributes a question, never the fact needed to answer it.

use super::*;
use std::sync::Arc;

fn typed(text: &str) -> Result<typed_trees::TypedTrees, diagnostics::Diagnostic> {
    let mut sources = source::SourceMap::default();
    let source = sources
        .add("width_parameters/source.omg".into(), text.to_owned())
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text)
        .tokenize()
        .expect("source tokenizes");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).expect("source parses");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        },
    )
    .expect("source resolves");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
}

fn generic(text: &str) -> typed_trees_to_terminal_psi::Elaboration {
    let typed = typed(text).expect("the width is a static binder");
    let entry = super::boundary_calls::entry(&typed);
    typed_trees_to_terminal_psi::elaborate_generic(&typed, entry).expect("source elaborates")
}

fn widths(kind: &ObligationKind) -> bool {
    matches!(kind, ObligationKind::IntegerWidthPositive { .. })
}

fn positive(mut form: typed_trees_to_terminal_psi::Elaboration) {
    let discharged = discharge(&mut form).expect("the actual width roster is valid");
    let questions = verdicts(&form, &discharged, 1, widths);
    assert_eq!(questions.len(), 1);
    let (question, verdict) = questions[0];
    assert_kernel_checked(&form, 1, question, certificate(verdict));
    assert!(
        discharged
            .machines
            .get(id(1))
            .unwrap()
            .verdicts
            .iter()
            .all(|(_, verdict)| verdict.is_ok())
    );
}

fn missing_positive(text: &str) {
    match judge(Ok(generic(text))).expect("source has a valid reconstruction") {
        RouteVerdict::Rejected { unproved, .. } => {
            assert!(unproved.iter().any(|row| widths(&row.kind)), "{unproved:?}")
        }
        other => panic!("the width's positive entry question must fail: {other:?}"),
    }
}

#[test]
fn source_identity_uses_its_own_const_input_and_an_ordinary_kernel_proof() {
    let form = generic(
        "machine entry<const N: u64>(value: u<N>) -> u<N> requires N > 0u64 ensures result == value crashes none { value }",
    );
    let parameter = match form.module.machines[0].parameters[0].scalar_type {
        ScalarType::Integer(integer) => match integer.width() {
            semantic_vocabulary::IntegerWidth::Parameter(parameter) => parameter,
            other => panic!("the generic width stays rigid: {other:?}"),
        },
        other => panic!("the carrier is an integer: {other:?}"),
    };
    assert_eq!(parameter.owner, id(1));
    assert_eq!(parameter.value, ValueId::new(2).unwrap());
    assert_eq!(form.value_parameters.get(id(1)), Some(&1));
    positive(form);
}

#[test]
fn an_input_in_the_carrier_does_not_prove_its_width_positive() {
    for requirement in ["", "requires N == 0u64"] {
        missing_positive(&format!(
            "machine entry<const N: u64>(value: u<N>) -> u<N> {requirement} ensures result == value crashes none {{ value }}"
        ));
    }
}

#[test]
fn unread_borrowed_widths_have_the_same_formation_obligation() {
    positive(generic(
        "machine entry<const N: u64>(value: &u<N>) requires N > 0u64 crashes none {}",
    ));
    for requirement in ["", "requires N == 0u64"] {
        missing_positive(&format!(
            "machine entry<const N: u64>(value: &u<N>) {requirement} crashes none {{}}"
        ));
    }
}

#[test]
fn an_empty_array_still_requires_a_well_formed_element_carrier() {
    for (parameter, body) in [("value: [u<N>; 0]", ""), ("", "let value: [u<N>; 0];")] {
        positive(generic(&format!(
            "machine entry<const N: u64>({parameter}) requires N > 0u64 crashes none {{{body}}}"
        )));
        for requirement in ["", "requires N == 0u64"] {
            missing_positive(&format!(
                "machine entry<const N: u64>({parameter}) {requirement} crashes none {{{body}}}"
            ));
        }
    }
}

#[test]
fn borrowed_erased_and_affine_predecessors_do_not_renumber_the_width() {
    let form = generic(
        "data Token {} machine entry<const N: u64>(marker: &u32, prior [erased]: u32, token: Token, value: u<N>) -> u<N> requires N > 0u64 ensures result == value crashes none { value }",
    );
    assert_eq!(
        form.module.machines[0].erased_scalar_formals[0].id,
        ValueId::new(4).unwrap()
    );
    let subjects =
        terminal_check_tables::obligations::integer_width_parameters(&form.module).unwrap();
    assert_eq!(subjects, vec![(id(1), vec![ValueId::new(2).unwrap()])]);
    positive(form);
}

#[test]
fn a_runtime_binder_does_not_become_static_from_a_positive_requirement() {
    let refusal = typed("machine entry<N: u64>(value: u<N>) -> u<N> requires N > 0u64 { value }")
        .unwrap_err();
    assert!(refusal.message.contains("runtime value"), "{refusal:?}");
}

#[test]
fn a_positive_width_does_not_authorize_the_wrong_body_guarantee() {
    let mut form = generic(
        "machine entry<const N: u64>(value: u<N>) -> u<N> requires N > 0u64 ensures result != value crashes none { value }",
    );
    let discharged = discharge(&mut form).unwrap();
    assert!(
        verdicts(&form, &discharged, 1, widths)
            .iter()
            .all(|(_, verdict)| verdict.is_ok())
    );
    assert!(
        verdicts(&form, &discharged, 1, guarantee)
            .iter()
            .any(|(_, verdict)| verdict.is_err())
    );
}

#[test]
fn an_ordinary_const_value_is_not_an_integer_width_use() {
    let mut form = generic("machine entry<const N: u64>(value: u32) -> u32 crashes none { value }");
    assert!(
        form.tables
            .machines
            .get(id(1))
            .unwrap()
            .obligations
            .iter()
            .all(|(_, row)| !widths(&row.kind))
    );
    let discharged = discharge(&mut form).unwrap();
    assert!(
        discharged
            .machines
            .get(id(1))
            .unwrap()
            .verdicts
            .iter()
            .all(|(_, verdict)| verdict.is_ok())
    );
}
