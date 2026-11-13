use crate::{
    BuildTimeEvaluationRequest, BuildTimeSources, PureEvaluationRecord, evaluate_pre_resolution,
};

#[test]
fn an_empty_frontier_preserves_its_input_without_execution_receipts() {
    let original = syntax_trees::SyntaxTrees::new(source::SourceId(17));
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: original.clone(),
        source_context: BuildTimeSources::default(),
    })
    .expect("no source normalization work");
    let (returned, pending) = evaluated.into_syntax_and_pre_check();
    assert!(returned == original);
    let limits = compiler_storage::RecordLimits::default();
    let bytes = compiler_storage::encode(&pending, limits).expect("actual empty continuation");
    let mut restored: crate::PreCheckEvaluation =
        compiler_storage::decode(&bytes, limits).expect("current source-input schema");
    let mut typed = typed_trees::TypedTrees::default();
    restored
        .rejoin_source_authority(&typed, None)
        .expect("known absent authority needs no invented source map");
    let pending = restored;
    let before = typed.clone();
    let record = pending
        .evaluate_or_defer_recorded(&mut typed)
        .expect("no typed work");
    assert!(typed == before);
    assert!(record.pending.is_none());
    assert!(record.accounting.receipts().is_empty());
}

#[test]
fn nonempty_syntax_stays_exact_until_typed_requests_are_known() {
    let mut original = syntax_trees::SyntaxTrees::new(source::SourceId(17));
    original.push_root_item(syntax_trees::item::Item::Module(Default::default()));
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: original.clone(),
        source_context: BuildTimeSources::default(),
    })
    .expect("syntax is retained, not evaluated or accepted here");
    let (returned, _) = evaluated.into_syntax_and_pre_check();
    assert!(returned == original);
    assert_eq!(returned.root_item_count(), 1);
}

#[test]
fn ordinary_declarations_are_retained_without_claiming_execution() {
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax_trees::SyntaxTrees::new(source::SourceId(17)),
        source_context: BuildTimeSources::default(),
    })
    .expect("empty syntax");
    let (_, pending) = evaluated.into_syntax_and_pre_check();
    let mut typed = typed_trees::TypedTrees::default();
    typed.push_data_definition(Default::default());
    let before = typed.clone();
    let record = pending
        .evaluate_or_defer_recorded(&mut typed)
        .expect("the declaration alone requests no evaluation");
    assert!(typed == before);
    assert!(record.pending.is_none());
    assert!(record.accounting.receipts().is_empty());
}

#[test]
fn a_real_constant_request_refuses_without_erasing_its_input() {
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax_trees::SyntaxTrees::new(source::SourceId(17)),
        source_context: BuildTimeSources::default(),
    })
    .unwrap();
    let (_, pending) = evaluated.into_syntax_and_pre_check();
    let mut typed = typed_trees::TypedTrees::default();
    typed.push_const_declaration(Default::default());
    let before = typed.clone();
    let errors = pending
        .evaluate(&mut typed)
        .expect_err("no constant producer has run");
    assert!(errors[0].message.contains("source constant evaluation"));
    assert!(typed == before);
}

#[test]
fn callable_signatures_do_not_hide_pending_extent_work() {
    use symbols::{
        BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder, builtin_type_symbols,
    };
    use typed_trees::{
        TypedTrees,
        callable_values::RuntimeCallableContract,
        data::{DataDefinition, DataField, DataMember},
        expression::ExpressionNode,
        signature::{StateParameter, StateSignature},
        types::{FixedArrayLength, TypeReferenceNode},
    };

    let mut symbols = SymbolTableBuilder::default();
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = symbols.insert_children(
        root,
        builtin_type_symbols().into_iter().chain([
            (SymbolKind::Data, SymbolNameRef::Static("Holder")),
            (SymbolKind::CallableContract, SymbolNameRef::Static("Outer")),
            (SymbolKind::CallableContract, SymbolNameRef::Static("Inner")),
        ]),
    );
    let owner = declarations.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
    let outer_owner = declarations.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
    let inner_owner = declarations.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 2);
    let field = symbols
        .insert_children(
            owner,
            [(SymbolKind::Field, SymbolNameRef::Static("callback"))],
        )
        .start();
    let parameter = symbols
        .insert_children(
            outer_owner,
            [(SymbolKind::Parameter, SymbolNameRef::Static("callback"))],
        )
        .start();
    let mut typed = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let byte = typed.type_reference_table.insert(TypeReferenceNode::Named {
        symbol: declarations.handle_at(BuiltinTypeAtom::U8.ordinal() as u32),
        name: "u8".into(),
    });
    let expression = typed.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(2),
    ));
    let extent = typed
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(expression));
    let array = typed
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: byte,
            length: FixedArrayLength::Expression(extent),
        });
    let inner = typed
        .runtime_callable_contracts
        .insert(RuntimeCallableContract {
            signature: StateSignature {
                symbol: inner_owner,
                return_type: array,
                ..Default::default()
            },
            lexical_owner: owner,
            ..Default::default()
        });
    let inner_type = typed
        .type_reference_table
        .insert(TypeReferenceNode::Callable { contract: inner });
    let parameters = typed.state_parameters.insert_many([StateParameter {
        symbol: parameter,
        name: "callback".into(),
        type_reference: inner_type,
        ..Default::default()
    }]);
    let outer = typed
        .runtime_callable_contracts
        .insert(RuntimeCallableContract {
            signature: StateSignature {
                symbol: outer_owner,
                parameters,
                return_type: byte,
                ..Default::default()
            },
            lexical_owner: owner,
            ..Default::default()
        });
    let outer_type = typed
        .type_reference_table
        .insert(TypeReferenceNode::Callable { contract: outer });
    let members = typed
        .data_members
        .insert_many([DataMember::Field(DataField {
            symbol: field,
            name: "callback".into(),
            type_reference: outer_type,
            ..Default::default()
        })]);
    typed.push_data_definition(DataDefinition {
        symbol: owner,
        name: "Holder".into(),
        members,
        ..Default::default()
    });
    let (_, pending) = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax_trees::SyntaxTrees::new(source::SourceId(17)),
        source_context: BuildTimeSources::default(),
    })
    .unwrap()
    .into_syntax_and_pre_check();
    let before = typed.clone();
    let errors = pending
        .clone()
        .evaluate(&mut typed)
        .expect_err("the nested return type still needs array-length evaluation");
    assert!(
        errors.iter().any(
            |error| error.message.contains("source array-extent evaluation")
                || error.message.contains("source type-expression evaluation")
        ),
        "{errors:?}"
    );
    assert!(typed == before);

    let closed_array = typed
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: byte,
            length: FixedArrayLength::Literal(2),
        });
    typed
        .runtime_callable_contracts
        .get_mut(inner)
        .signature
        .return_type = closed_array;
    pending
        .evaluate(&mut typed)
        .expect("a nested signature with literal array length needs no evaluation");
}

#[test]
fn orphan_result_rows_do_not_make_a_source_frontier_empty() {
    let mut closure = typed_trees::TypedTrees::default();
    closure
        .constant_recipe_closure
        .target_bodies
        .push(Default::default());
    // The legacy convenience predicate sees only values; admission must not
    // ignore retained target selection or receipt dimensions.
    assert!(closure.constant_recipe_closure.is_empty());
    let mut placements = typed_trees::TypedTrees::default();
    placements.wire_placements.insert(Default::default());
    let mut encoding = typed_trees::TypedTrees::default();
    encoding.wire_encode_obligations.insert(Default::default());
    let mut proved = typed_trees::TypedTrees::default();
    proved.retain_proven_type_contexts(vec![typed_trees::ProvenTypeContext {
        expression: Default::default(),
        equations: Vec::new(),
        reachable: true,
        case_value: Some(true),
        admitted_cases: None,
    }]);
    for mut typed in [closure, placements, encoding, proved] {
        let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
            syntax_trees: syntax_trees::SyntaxTrees::new(source::SourceId(17)),
            source_context: BuildTimeSources::default(),
        })
        .unwrap();
        let (_, pending) = evaluated.into_syntax_and_pre_check();
        let before = typed.clone();
        assert!(pending.evaluate(&mut typed).is_err());
        assert!(typed == before);
    }
}

#[test]
fn ordinary_contract_sources_reach_typed_preparation_without_being_evaluated() {
    // These retained controls include an allowed branch computation and one
    // whose subtraction is unproved. Inline text keeps the test independent of
    // the retired fixture tree. Preparation retains both for the later checker.
    for (path, text) in [
        (
            "tests/fixtures/pass/contracts/consistent_branches_quiet_exit/main.omg",
            r#"// Both branches can run under `count >= 10`, so neither is reported and each
// subtraction is proved from its own branch's facts.
machine pick(count: u64) -> u64
requires count >= 10
{
    transition count < 50 {
        true -> (count - 10u64)
        false -> (count - 50u64)
    }
}
machine main() crashes Trap {
    transition pick(12) == 2 && pick(80) == 30 { true -> done() _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#,
        ),
        (
            "tests/fixtures/fail/contracts/unproved_branch_is_not_contradiction/main.omg",
            r#"// `count >= 10` and `count < 50` are consistent: failing to prove
// `count >= 100` on that branch is not evidence the branch is impossible.
machine pick(count: u64) -> u64
requires count >= 10
{
    transition count < 50 {
        true -> (count - 100u64)
        false -> (count - 10u64)
    }
}
machine main() {}
"#,
        ),
    ] {
        let mut sources = source::SourceMap::from_files(Vec::new());
        let source_id = sources
            .add(std::path::PathBuf::from(path), text.to_owned())
            .source_id;
        let sources = std::sync::Arc::new(sources);
        let tokens = source_files_to_tokens::Lexer::new(text)
            .tokenize()
            .expect("existing source lexes");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
            .expect("existing source parses");
        let original = syntax.clone();
        let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
            syntax_trees: syntax,
            source_context: BuildTimeSources {
                sources: Some(std::sync::Arc::clone(&sources)),
                ..BuildTimeSources::default()
            },
        })
        .expect("ordinary source is retained until requests are typed");
        let (syntax, pending) = evaluated.into_syntax_and_pre_check();
        assert!(syntax == original);
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax,
                sources: Some(sources),
                top_level_bindings: Vec::new(),
                retained_base: None,
            },
        )
        .expect("source resolves through the ordinary resolver");
        let mut typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("source reaches the ordinary typed frontier");
        assert!(typed.machines().len() >= 2);
        let before = typed.clone();
        let record = pending
            .evaluate_or_defer_recorded(&mut typed)
            .expect("these bodies request no source evaluation");
        assert!(typed == before);
        assert!(record.pending.is_none());
        assert!(record.accounting.receipts().is_empty());
    }
}

#[test]
fn retired_tree_receipts_cannot_be_decoded_as_canonical_scalar_receipts() {
    let limits = compiler_storage::RecordLimits::default();
    // The old enum reserved wire tag zero for a source/checked-tree receipt.
    // No old tree or supposed proof is constructed to test that boundary.
    let bytes = compiler_storage::encode(&0_u32, limits).expect("retired tag");
    let error = compiler_storage::decode::<PureEvaluationRecord>(&bytes, limits)
        .expect_err("retired source evaluation must not become canonical evidence");
    assert!(
        error
            .to_string()
            .contains("retired tree-evaluation receipt")
    );
}
