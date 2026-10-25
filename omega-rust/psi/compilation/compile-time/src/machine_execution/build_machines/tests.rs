use super::PreparedBuildMachineProgram;
use semantic_vocabulary::IntegerValue;
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("lex");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let mut sources = source::SourceMap::default();
    sources.add_with_metadata(
        "constant.omg".into(),
        source.into(),
        ".".into(),
        semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
        source::SourceOrigin::User,
    );
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

fn checked(source: &str) -> PreparedBuildMachineProgram {
    let (typed, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
        typed(source),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .expect("prepare");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        preparation,
        &typed_trees_to_checked_trees::CheckingRequest::new(),
    )
    .expect("check");
    PreparedBuildMachineProgram::from_checked(checked)
}

fn on_compilation_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .expect("start compilation test")
        .join()
        .expect("compilation test");
}

#[test]
fn checked_constant_runs_transitive_terminal_calls() {
    on_compilation_stack(|| {
        let program = checked(
            "machine leaf() -> u64 { 16 }
             machine middle() -> u64 { leaf() }
             machine answer() -> u64 { middle() }",
        );
        let symbol = program
            .typed()
            .realized_machine_named("answer")
            .expect("answer")
            .symbol;
        let entry = program.entry(symbol).expect("entry");
        let accounting = crate::evaluation_accounting::EvaluationAccounting::recording();
        let result = program
            .evaluate_checked_constant(&entry, &[], &accounting)
            .expect("Terminal execution");
        let completed = accounting.finish().expect("complete Terminal accounting");
        assert!(completed.has_complete_usage());
        let [crate::PureEvaluationRecord::Terminal(receipt)] = completed.receipts() else {
            panic!("one root Terminal execution must retain one typed receipt");
        };
        assert_eq!(receipt.machine(), symbol);
        let input = program.checked_program().expect("exact checked input");
        assert!(matches!(
            receipt.source_program(),
            crate::PureEvaluationProgram::CheckedClosure {
                execution,
                product_declarations,
            } if execution.as_ref() == input.checked.as_ref()
                && product_declarations.as_ref() == input.product_declarations.as_ref()
        ));
        assert!(receipt.operators().is_empty());
        assert!(receipt.program().is_some());
        assert!(receipt.usage().total_units() > 0);
        assert!(matches!(
            receipt.outcome(),
            crate::PureTerminalEvaluationOutcome::Returned { value } if value == &result
        ));
        completed
            .validate_charges()
            .expect("retained Terminal usage");
        assert!(
            matches!(
                result,
                TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(16),
                    ..
                })
            ),
            "{result:?}"
        );
    });
}

#[test]
fn checked_constant_rejects_another_preparations_entry() {
    on_compilation_stack(|| {
        let first = checked("machine answer() -> u64 { 16 }");
        let second = checked("machine answer() -> u64 { 17 }");
        let symbol = first
            .typed()
            .realized_machine_named("answer")
            .expect("answer")
            .symbol;
        let entry = first.entry(symbol).expect("entry");
        let error = second
            .evaluate_checked_constant(
                &entry,
                &[],
                &crate::evaluation_accounting::EvaluationAccounting::default(),
            )
            .expect_err("foreign entry");
        assert!(error.contains("different prepared program"), "{error}");
    });
}

#[test]
fn checked_constant_retains_unsigned_result_meaning() {
    on_compilation_stack(|| {
        let program = checked("machine answer() -> u64 { 18446744073709551615u64 }");
        let symbol = program
            .typed()
            .realized_machine_named("answer")
            .expect("answer")
            .symbol;
        let entry = program.entry(symbol).expect("entry");
        let result = program
            .evaluate_checked_constant(
                &entry,
                &[],
                &crate::evaluation_accounting::EvaluationAccounting::default(),
            )
            .expect("Terminal execution");
        assert!(
            matches!(
                result,
                TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(18446744073709551615),
                    ..
                })
            ),
            "{result:?}"
        );
    });
}

#[test]
fn checked_constant_does_not_install_boundary_imports() {
    on_compilation_stack(|| {
        let program = checked(
            "boundary trait Counter { machine read() -> u64; }
             machine answer() -> u64 reaches Counter { Counter::read() }",
        );
        let symbol = program
            .typed()
            .realized_machine_named("answer")
            .expect("answer")
            .symbol;
        let entry = program.entry(symbol).expect("entry");
        let error = program
            .evaluate_checked_constant(
                &entry,
                &[],
                &crate::evaluation_accounting::EvaluationAccounting::default(),
            )
            .expect_err("no host authority");
        assert!(error.contains("UnboundImports"), "{error}");
    });
}
