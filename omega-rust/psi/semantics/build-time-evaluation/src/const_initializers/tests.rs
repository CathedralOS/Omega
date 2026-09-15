use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::{ConstDefinition, Item};
use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};

fn parse(text: &str) -> (SyntaxTrees, Arc<SourceMap>) {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("main.omg"), text.to_owned())
        .source_id;
    let tokens = Lexer::new(text).tokenize().expect("initializer tokens");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("initializer syntax");
    (syntax, Arc::new(sources))
}

fn parse_files(files: &[(&str, &str)]) -> (SyntaxTrees, Arc<SourceMap>, Vec<source::SourceId>) {
    let mut sources = SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    let mut ids = Vec::new();
    for (path, text) in files {
        let source_id = sources
            .add(PathBuf::from(path), (*text).to_owned())
            .source_id;
        let tokens = Lexer::new(text).tokenize().expect("initializer tokens");
        parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
            .expect("initializer syntax");
        ids.push(source_id);
    }
    (syntax, Arc::new(sources), ids)
}

fn evaluate(text: &str) -> Result<SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    let (syntax, sources) = parse(text);
    super::evaluate(syntax, Some(sources), &[], None)
}

/// Run the complete pre-resolution evaluation and typed receipt replay so
/// retained structured leaves re-derive their canonical results through the
/// checked interpreter rather than trusting the materialized literal.
fn evaluate_fully(
    files: &[(&str, &str)],
    bindings: &[symbols::SourceScopedTopLevelBinding],
) -> typed_trees::TypedTrees {
    let (syntax, sources, _) = parse_files(files);
    let evaluated = crate::evaluate_pre_resolution(crate::BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: Some(crate::BuildTimeSourceContext {
            sources: sources.clone(),
            source_scoped_top_level_bindings: bindings,
            selection_authority: None,
            retained_base: None,
        }),
    })
    .expect("pre-resolution evaluation");
    let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
    let resolved =
        crate::syntax_probes::resolve(&syntax, Some(sources), bindings).expect("resolve");
    let mut typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    pre_check.evaluate(&mut typed).expect("receipt replay");
    typed
}

fn constant<'syntax>(syntax: &'syntax SyntaxTrees, name: &str) -> &'syntax ConstDefinition {
    syntax
        .root_items()
        .find_map(|item| match item {
            Item::Const(definition) if definition.name.as_str() == name => Some(definition),
            _ => None,
        })
        .expect("named constant")
}

fn integer_encoding(syntax: &SyntaxTrees, name: &str, value: i128) {
    let definition = constant(syntax, name);
    let receipt = definition
        .normalization
        .as_ref()
        .expect("evaluated declaration");
    assert_eq!(
        receipt.canonical_result_encoding,
        CanonicalConstIdentity::integer("u64", value).encoding
    );
    assert!(matches!(
        syntax.expressions.expression(definition.value),
        ExpressionNode::Integer(_)
    ));
}

#[test]
fn concrete_invocation_admission_preserves_demand_and_scalar_snapshots() {
    let evaluated = evaluate(
        "machine divide(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        machine forward(value: u64) -> u64 { divide(value) }
        machine identity(value: u64) -> u64 { value }
        machine gate(flag: bool) -> u64
        crashes Abort flag
        { transition { !flag -> 7 } crash Abort; }
        machine forward_gate(flag: bool) -> u64 { gate(flag) }
        machine full_width(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> value } crash Trap; }
        machine signed(value: i64) -> u64
        crashes Trap value < 0
        { transition { value >= 0 -> 9 } crash Trap; }
        const DIVIDED: u64 = forward(identity(1 + 1));
        const SELECTED: u64 = match false { true -> divide(0), false -> forward_gate(false) };
        const MAXIMUM: u64 = full_width(18446744073709551615);
        const SIGNED: u64 = signed(0);",
    )
    .expect("concrete arguments discharge guarded calls before demanded execution");
    integer_encoding(&evaluated, "DIVIDED", 5);
    integer_encoding(&evaluated, "SELECTED", 7);
    integer_encoding(&evaluated, "MAXIMUM", i128::from(u64::MAX));
    integer_encoding(&evaluated, "SIGNED", 9);
}

#[test]
fn concrete_invocation_rejects_undischarged_published_routes_before_interpretation() {
    for source in [
        "machine divide(value: u64) -> u64 crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        const UNUSED: u64 = divide(0);",
        "machine gate(flag: bool) -> u64 crashes Abort flag { 7 }
        const UNUSED: u64 = gate(true);",
        "machine gate(flag: bool) -> u64 crashes Trap flag { 7 }
        machine forward(flag: bool) -> u64 { gate(flag) }
        const UNUSED: u64 = forward(true);",
        "machine gate() -> u64 crashes Trap { 7 }
        const UNUSED: u64 = gate();",
        "machine signed(value: i64) -> u64 crashes Trap value < 0
        { transition { value >= 0 -> 9 } crash Trap; }
        const UNUSED: u64 = signed(-1);",
    ] {
        let diagnostics = evaluate(source).expect_err("published route must discharge");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("retains unhandled")),
            "must reject through admission, not an interpreter failure: {diagnostics:?}"
        );
    }
    assert!(
        evaluate(
            "machine constrained(value: u64) -> u64 requires value > 0 { value }
        const UNUSED: u64 = constrained(0);"
        )
        .is_err(),
        "ordinary precondition floor remains required"
    );
}

#[test]
fn concrete_invocation_discharges_arithmetic_guards_from_checked_evidence() {
    // The forwarding body passes an arithmetic actual. The published guard
    // retains that expression over entry values, and the checked scalar
    // channel decides it once the outer concrete call supplies the binding;
    // interpretation success alone cannot establish this safety.
    let evaluated = evaluate(
        "machine divide(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        machine forward(value: u64) -> u64 { divide(value - 1) }
        machine product(value: u64) -> u64 { divide(value * 2) }
        const DIVIDED: u64 = forward(3);
        const PRODUCT: u64 = product(1);",
    )
    .expect("concrete arguments discharge arithmetic guard evidence");
    integer_encoding(&evaluated, "DIVIDED", 5);
    integer_encoding(&evaluated, "PRODUCT", 5);
}

#[test]
fn concrete_invocation_rejects_arithmetic_guards_that_hold_or_trap() {
    for source in [
        // `1 - 1` lands on the guarded divisor: the Trap route is confirmed.
        "machine divide(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        machine forward(value: u64) -> u64 { divide(value - 1) }
        const UNUSED: u64 = forward(1);",
        // The exact subtraction itself traps before the callee is reached;
        // the caller's own recorded cause keeps the invocation refused.
        "machine divide(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        machine forward(value: u64) -> u64 { divide(value - 1) }
        const UNUSED: u64 = forward(0);",
        // An operand with no entry-value custody keeps no provable origin.
        // `hidden(0)` would interpret to a safe `divide(1)`, but a call
        // result is not checked evidence and the guarded Trap must survive.
        "machine divide(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        machine hidden(value: u64) -> u64 { 1 }
        machine forward(value: u64) -> u64 { divide(hidden(value)) }
        const UNUSED: u64 = forward(0);",
    ] {
        let diagnostics = evaluate(source)
            .expect_err("arithmetic guard evidence must not be invented from interpretation");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("retains unhandled")),
            "must reject through admission, not an interpreter failure: {diagnostics:?}"
        );
    }
}

#[test]
fn concrete_invocation_discharges_comparison_actuals_from_entry_provenance() {
    // A Boolean actual is entry provenance, not only scalar annotation:
    // `gate(value != 0)` retains the comparison over the caller's entry so the
    // concrete probe can decide it. The local `armed` spells the same origin
    // through an immutable initializer.
    let evaluated = evaluate(
        "machine gate(flag: bool) -> u64
        crashes Abort flag
        { transition { !flag -> 7 } crash Abort; }
        machine forward(value: u64) -> u64 { gate(value != 0) }
        machine inspect(value: u64) -> u64
        { let armed: bool = value != 0 && value < 9; gate(armed) }
        const DISCHARGED: u64 = forward(0);
        const INSPECTED: u64 = inspect(9);",
    )
    .expect("comparison actuals discharge guarded calls through entry provenance");
    integer_encoding(&evaluated, "DISCHARGED", 7);
    integer_encoding(&evaluated, "INSPECTED", 7);
    for source in [
        // `3 != 0` is true: the Abort route is confirmed, not merely unproven.
        "machine gate(flag: bool) -> u64
        crashes Abort flag
        { transition { !flag -> 7 } crash Abort; }
        machine forward(value: u64) -> u64 { gate(value != 0) }
        const UNUSED: u64 = forward(3);",
        // `10 != 0 && 10 > 9` is true through the local initializer: the
        // Abort route is confirmed rather than discharged.
        "machine gate(flag: bool) -> u64
        crashes Abort flag
        { transition { !flag -> 7 } crash Abort; }
        machine inspect(value: u64) -> u64
        { let armed: bool = value != 0 && value > 9; gate(armed) }
        const UNUSED: u64 = inspect(10);",
    ] {
        let diagnostics =
            evaluate(source).expect_err("a confirmed crash route must survive concrete discharge");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("retains unhandled")),
            "must reject through admission, not an interpreter failure: {diagnostics:?}"
        );
    }
}

#[test]
fn concrete_invocation_discharges_authored_requires_at_snapshot_arguments() {
    // The conservative closure fence rejects any authored `requires` because a
    // context-free evaluation cannot prove it. A concrete invocation is not
    // context-free: the snapshot probe re-runs ordinary contract checking on
    // the exact call, so `constrained(3)` proves `3 > 0` before interpretation.
    let evaluated = evaluate(
        "machine constrained(value: u64) -> u64 requires value > 0 { value }
        machine forward(value: u64) -> u64 requires value > 0 { constrained(value) }
        machine paired(left: u64, right: u64) -> u64
        requires left > 1
        requires right > 1
        { left }
        machine guarded(value: u64) -> u64
        requires value > 0
        crashes Trap value == 1
        { transition { value != 1 -> 10 / value } crash Trap; }
        const USED: u64 = constrained(3);
        const FORWARDED: u64 = forward(2);
        const PAIRED: u64 = paired(3, 4);
        const GUARDED: u64 = guarded(2);",
    )
    .expect("concrete arguments prove authored requires premises");
    integer_encoding(&evaluated, "USED", 3);
    integer_encoding(&evaluated, "FORWARDED", 2);
    integer_encoding(&evaluated, "PAIRED", 3);
    integer_encoding(&evaluated, "GUARDED", 5);
}

#[test]
fn concrete_invocation_rejects_requires_that_fail_at_snapshot_arguments() {
    for source in [
        // The premise itself is false at the concrete argument.
        "machine constrained(value: u64) -> u64 requires value > 0 { value }
        const UNUSED: u64 = constrained(0);",
        // The same premise is false through one forwarding hop: `forward`'s own
        // requires is what proves its internal `constrained(value)` call, and
        // that premise is equally false at the snapshot argument.
        "machine constrained(value: u64) -> u64 requires value > 0 { value }
        machine forward(value: u64) -> u64 requires value > 0 { constrained(value) }
        const UNUSED: u64 = forward(0);",
        // One of two premises fails.
        "machine paired(left: u64, right: u64) -> u64
        requires left > 1
        requires right > 1
        { left }
        const UNUSED: u64 = paired(3, 0);",
        // A requires premise must not be weakened just because a guarded route
        // also exists: the crash check would pass (`guarded(0)` cannot reach its
        // `value == 1` Trap) but the authored premise is still violated.
        "machine guarded(value: u64) -> u64
        requires value > 0
        crashes Trap value == 1
        { transition { value != 1 -> 10 / value } crash Trap; }
        const UNUSED: u64 = guarded(0);",
    ] {
        let diagnostics = evaluate(source)
            .expect_err("a requires premise false at the concrete arguments must reject");
        assert!(
            !diagnostics.is_empty(),
            "requires violation produced no diagnostic"
        );
    }
    assert!(
        evaluate(
            "machine guarded(value: u64) -> u64
            requires value > 0
            crashes Trap value == 1
            { transition { value != 1 -> 10 / value } crash Trap; }
            const UNUSED: u64 = guarded(1);"
        )
        .is_err(),
        "a satisfied requires premise never erases a retained crash route"
    );
}

#[test]
fn ordinary_machine_initializers_retain_calls_and_exact_scalar_composition() {
    let evaluated = evaluate(
        "machine size() -> u64 { 7 }
        machine identity(value: u64) -> u64 { value }
        machine enabled(value: bool) -> bool { value }
        const SIZE: u64 = identity(size()) * 2;
        const ENABLED: bool = enabled(SIZE == 14) && true;",
    )
    .expect("ordinary checked scalar calls");
    integer_encoding(&evaluated, "SIZE", 14);
    assert!(
        !constant(&evaluated, "SIZE")
            .normalization
            .as_ref()
            .unwrap()
            .call_selections
            .is_empty()
    );
    assert!(matches!(
        evaluated
            .expressions
            .expression(constant(&evaluated, "ENABLED").value),
        ExpressionNode::Boolean(true)
    ));
}

#[test]
fn machine_initializer_dependencies_are_ready_before_helper_execution() {
    let evaluated = evaluate(
        "machine size() -> u64 { BASE }
        const SIZE: u64 = size();
        const BASE: u64 = 7 / 2 * 2;",
    )
    .expect("helper dependency is evaluated before its invocation");
    integer_encoding(&evaluated, "SIZE", 7);
}

#[test]
fn machine_initializers_transport_full_width_integers_without_relanding() {
    let evaluated = evaluate(
        "machine identity(value: u64) -> u64 { value }
        const MAXIMUM: u64 = identity(18446744073709551615);",
    )
    .expect("full-width unsigned interpreter snapshot");
    integer_encoding(&evaluated, "MAXIMUM", i128::from(u64::MAX));
    for source in [
        "machine narrow() -> u8 { 7 } const VALUE: u64 = narrow();",
        "machine identity(value: u64) -> u64 { value } const VALUE: u64 = identity(7u8);",
        "machine ignore(value: u8) -> bool { true } const VALUE: bool = false && ignore(256);",
        "machine ignore(value: u8) -> bool { true } const VALUE: bool = false && ignore(7 / 2);",
    ] {
        assert!(
            evaluate(source).is_err(),
            "invalid source argument/carrier accepted: {source}"
        );
    }
}

#[test]
fn machine_initializer_calls_compose_in_nominal_and_array_leaves() {
    let evaluated = evaluate(
        "data Config [copy] { size: u64; enabled: bool; }
        machine size() -> u64 { 7 }
        machine enabled() -> bool { true }
        const CONFIG: Config = Config { size: size() * 2, enabled: enabled() };
        const SIZES: [u64; 2] = [size(), size() * 2];",
    )
    .expect("ordinary calls preserve aggregate leaf custody");
    assert!(constant(&evaluated, "CONFIG").normalization.is_some());
    assert!(constant(&evaluated, "SIZES").normalization.is_some());
}

fn array_leaves(
    syntax: &SyntaxTrees,
    expression: syntax_trees::expression::ExpressionHandle,
) -> Vec<ExpressionNode> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::ArrayLiteral(elements) => syntax
            .expressions
            .expression_handles(*elements)
            .iter()
            .flat_map(|element| array_leaves(syntax, *element))
            .collect(),
        leaf => vec![leaf.clone()],
    }
}

fn literal_encoding(text: &str, name: &str) -> String {
    let (syntax, _) = parse(text);
    syntax_trees_to_symbol_resolved_trees::pre_resolution::canonicalize_declared_const_definition(
        &syntax,
        constant(&syntax, name),
    )
    .expect("literal array canonicalization")
    .encoding
}

#[test]
fn computed_nominal_leaves_preserve_carriers_fields_and_exact_arithmetic() {
    let declarations = "data Leaf [copy] { count: u64; }
        data Config [copy] { leaf: Leaf; flags: [bool; 2]; }";
    let evaluated = evaluate(&format!("{declarations}
        const SIZE: u64 = 7 / 2 * 2;
        const CONFIG: Config = Config {{ flags: [SIZE == 7, false], leaf: Leaf {{ count: SIZE * 2 }} }};"))
        .expect("selected nominal scalar leaves");
    let config = constant(&evaluated, "CONFIG");
    let receipt = config
        .normalization
        .as_ref()
        .expect("nominal initializer receipt");
    assert_eq!(
        receipt.canonical_result_encoding,
        literal_encoding(
            &format!(
                "{declarations}
        const CONFIG: Config = Config {{ leaf: Leaf {{ count: 14 }}, flags: [true, false] }};"
            ),
            "CONFIG"
        )
    );
    assert!(matches!(
        evaluated
            .expressions
            .expression(receipt.authored_expression),
        ExpressionNode::StructLiteral(_)
    ));
    assert_ne!(receipt.authored_expression, config.value);
    assert!(
        receipt
            .selections
            .iter()
            .any(|origin| origin.declaration == constant(&evaluated, "SIZE").name.source_span())
    );
    syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&evaluated),
    )
    .expect("nominal initializer receipt independently rejoins authored selections");
}

#[test]
fn computed_nominal_leaves_reject_invalid_unused_fields_and_constructor_shapes() {
    for initializer in [
        "Config { count: 200 + 100, enabled: true }",
        "Config { count: 7 / 2, enabled: true }",
        "Config { count: 2 + 1, enabled: 2 }",
        "Config { count: 2 + 1 }",
        "Config { count: 2 + 1, count: 2, enabled: true }",
        "Other { count: 2 + 1, enabled: true }",
    ] {
        assert!(
            evaluate(&format!(
                "data Config [copy] {{ count: u8; enabled: bool; }}
            data Other [copy] {{ count: u8; enabled: bool; }}
            const UNUSED: Config = {initializer};"
            ))
            .is_err(),
            "{initializer}"
        );
    }
}

#[test]
fn computed_case_payloads_and_record_arrays_share_scalar_leaf_evaluation() {
    let declarations = "data Value [copy] { case Empty; case Number(count: u64); }
        data Row [copy] { count: u64; enabled: bool; }";
    let evaluated = evaluate(&format!(
        "{declarations}
        const VALUE: Value = Value::Number {{ count: 7 / 2 * 2 }};
        const ROWS: [Row; 2] = [Row {{ count: 1 + 1, enabled: true }},
            Row {{ count: 7 / 2 * 2, enabled: false && (1u8 / 0 == 0) }}];"
    ))
    .expect("case and record array leaves");
    for (name, value) in [
        ("VALUE", "Value::Number { count: 7 }"),
        (
            "ROWS",
            "[Row { count: 2, enabled: true }, Row { count: 7, enabled: false }]",
        ),
    ] {
        let carrier = if name == "VALUE" { "Value" } else { "[Row; 2]" };
        assert_eq!(
            constant(&evaluated, name)
                .normalization
                .as_ref()
                .unwrap()
                .canonical_result_encoding,
            literal_encoding(
                &format!("{declarations} const {name}: {carrier} = {value};"),
                name
            )
        );
    }
    syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&evaluated),
    )
    .expect("case constructor receipt replay");
}

#[test]
fn computed_record_leaves_preserve_payloadless_case_siblings() {
    let declarations = "data Mode [copy] { case On; case Off; }
        data Config [copy] { mode: Mode; count: u64; }";
    let evaluated = evaluate(&format!(
        "{declarations}
        const UNUSED: Mode = Mode::Off;
        const CONFIG: Config = Config {{ mode: Mode::On, count: 7 / 2 * 2 }};"
    ))
    .expect("case literal is not a constant dependency");
    assert_eq!(
        constant(&evaluated, "CONFIG")
            .normalization
            .as_ref()
            .unwrap()
            .canonical_result_encoding,
        literal_encoding(
            &format!(
                "{declarations} const CONFIG: Config = Config {{ mode: Mode::On, count: 7 }};"
            ),
            "CONFIG"
        )
    );
    syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&evaluated),
    )
    .expect("case identity survives evaluation and receipt replay");
}

#[test]
fn computed_nominal_leaves_reject_changed_result_and_dependency_receipts() {
    let evaluated = evaluate(
        "data Config [copy] { count: u64; }
        const SIZE: u64 = 7 / 2 * 2;
        const CONFIG: Config = Config { count: SIZE * 2 };",
    )
    .expect("nominal receipt baseline");
    for mutation in ["result", "dependency", "operator"] {
        let mut invalid = evaluated.clone();
        let item = *invalid.root_item_handles().iter().find(|item| {
            matches!(invalid.root_item(**item), Item::Const(definition) if definition.name.as_str() == "CONFIG")
        }).unwrap();
        let Item::Const(mut definition) = invalid.root_item(item).clone() else {
            panic!("constant");
        };
        let receipt = definition.normalization.as_mut().unwrap();
        match mutation {
            "result" => receipt.canonical_result_encoding.push('0'),
            "dependency" => receipt.selections.clear(),
            "operator" => receipt.builtin_operators.clear(),
            _ => unreachable!(),
        }
        invalid.items.replace_item(item, Item::Const(definition));
        assert!(
            syntax_trees_to_symbol_resolved_trees::resolve(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&invalid)
            )
            .is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn computed_array_leaves_land_independently_and_retain_the_authored_array() {
    let (syntax, sources) = parse(
        "const SIZE: u64 = 7 / 2 * 2;
         const SIZES: [u64; 2] = [SIZE * 2, 3];
         const FLAGS: [bool; 2] = [SIZE == 6, false];
         const GRID: [[u8; 2]; 1] = [[1 + 1, 2]];",
    );
    let authored_sizes = constant(&syntax, "SIZES").value;
    let authored_span = syntax.expressions.source_span(authored_sizes);
    let evaluated = super::evaluate(syntax, Some(sources), &[], None).expect("array leaves");

    assert_eq!(
        array_leaves(&evaluated, constant(&evaluated, "SIZES").value),
        vec![
            ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(14)),
            ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(3)),
        ]
    );
    assert_eq!(
        array_leaves(&evaluated, constant(&evaluated, "FLAGS").value),
        vec![
            ExpressionNode::Boolean(false),
            ExpressionNode::Boolean(false)
        ]
    );
    assert_eq!(
        array_leaves(&evaluated, constant(&evaluated, "GRID").value),
        vec![
            ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(2)),
            ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(2)),
        ]
    );
    let sizes = constant(&evaluated, "SIZES");
    let ExpressionNode::ArrayLiteral(elements) = evaluated
        .expressions
        .expression(sizes.normalization.as_ref().unwrap().authored_expression)
    else {
        panic!("authored array receipt");
    };
    assert!(matches!(
        evaluated
            .expressions
            .expression(evaluated.expressions.expression_handles(*elements)[0]),
        ExpressionNode::Binary(_)
    ));
    assert_eq!(
        evaluated.expressions.source_span(sizes.value),
        authored_span
    );
    assert!(
        sizes
            .normalization
            .as_ref()
            .unwrap()
            .selections
            .iter()
            .any(|origin| origin.declaration == constant(&evaluated, "SIZE").name.source_span())
    );
    assert!(
        constant(&evaluated, "FLAGS")
            .normalization
            .as_ref()
            .unwrap()
            .selections
            .iter()
            .any(|origin| origin.declaration == constant(&evaluated, "SIZE").name.source_span())
    );
    assert!(
        constant(&evaluated, "GRID")
            .normalization
            .as_ref()
            .unwrap()
            .selections
            .is_empty()
    );
    for (name, literal) in [
        ("SIZES", "const SIZES: [u64; 2] = [14, 3];"),
        ("FLAGS", "const FLAGS: [bool; 2] = [false, false];"),
        ("GRID", "const GRID: [[u8; 2]; 1] = [[2, 2]];"),
    ] {
        assert_eq!(
            constant(&evaluated, name)
                .normalization
                .as_ref()
                .unwrap()
                .canonical_result_encoding,
            literal_encoding(literal, name)
        );
    }
}

#[test]
fn computed_array_leaves_owe_their_declared_landing_even_when_private_and_unused() {
    let errors = evaluate("const BAD: [u8; 1] = [200 + 100]; machine run() -> u64 { 0 }")
        .expect_err("private array landing");
    assert!(
        errors
            .iter()
            .any(|error| { error.message.contains("land") || error.message.contains("u8") })
    );
}

#[test]
fn computed_array_leaves_admit_checked_machine_calls() {
    let evaluated = evaluate(
        "const CALL: [u64; 1] = [read()];
         machine read() -> u64 { 1 }",
    )
    .expect("array leaves use ordinary invocation admission");
    let declaration = constant(&evaluated, "CALL");
    assert_eq!(
        array_leaves(&evaluated, declaration.value),
        vec![ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(1)
        )]
    );
}

#[test]
fn retained_invocation_replay_rechecks_concrete_crash_discharge() {
    // Both actuals would return 7 if interpretation were used as admission.
    // Only false discharges the published ceiling; keep the folded value fixed.
    let (syntax, sources) = parse(
        "machine gate(flag: bool) -> u64 crashes Trap flag { 7 }
        const SIZE: u64 = gate(false);",
    );
    let evaluated = super::evaluate(syntax, Some(sources.clone()), &[], None)
        .expect("concrete safe invocation");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &evaluated,
            sources: Some(sources),
            top_level_bindings: Vec::new(),
        },
    )
    .expect("retained guarded call");
    let mut typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed guarded call");
    super::validate_retained_invocations(&typed, None).expect("unchanged discharge");
    let original = typed.const_declarations()[0].authored_initializer;
    let typed_trees::expression::ExpressionNode::Call(call) =
        typed.expression_table.expression(original)
    else {
        panic!("retained call");
    };
    let argument = typed.expression_table.expression_handles(call.arguments)[0];
    *typed.expression_table.expression_mut(argument) =
        typed_trees::expression::ExpressionNode::Boolean(true);
    let diagnostics = super::validate_retained_invocations(&typed, None)
        .expect_err("same result cannot preserve changed invocation admission");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("retains unhandled")),
        "{diagnostics:?}"
    );
}

#[test]
fn retained_invocation_replay_rejects_changed_or_erased_computation_and_results() {
    let (syntax, sources) = parse(
        "machine size() -> u64 { 7 }
        machine unrelated() -> u64 { 7 }
        const SIZE: u64 = size();",
    );
    let evaluated =
        super::evaluate(syntax, Some(sources.clone()), &[], None).expect("evaluated call");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &evaluated,
            sources: Some(sources),
            top_level_bindings: Vec::new(),
        },
    )
    .expect("retained call source resolution");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("retained call typing");
    super::validate_retained_invocations(&typed, None).expect("unchanged invocation replay");
    let declaration = typed.const_declarations()[0].clone();
    for mutation in 0..6 {
        let mut changed = typed.clone();
        match mutation {
            0 => {
                *changed
                    .expression_table
                    .expression_mut(declaration.authored_initializer) =
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(7),
                    )
            }
            1 => {
                *changed
                    .expression_table
                    .expression_mut(declaration.materialized_initializer) =
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(8),
                    )
            }
            2 => {
                let other = changed
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "unrelated")
                    .unwrap();
                let target = changed.machine_states(other)[0].symbol;
                let typed_trees::expression::ExpressionNode::Call(call) = changed
                    .expression_table
                    .expression_mut(declaration.authored_initializer)
                else {
                    panic!("retained original call")
                };
                call.target_symbol = target;
            }
            3 => {
                let span = changed.roots.const_declarations;
                let retained = &mut changed.tables.const_declarations.span_mut_or_empty(span)[0];
                retained.authored_initializer =
                    typed_trees::expression::ExpressionHandle::invalid();
                retained.materialized_initializer =
                    typed_trees::expression::ExpressionHandle::invalid();
            }
            4 => {
                let machine = changed
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "size")
                    .unwrap();
                let state = &changed.machine_states(machine)[0];
                let value = match &changed.statement_table.statements(state.statement_nodes)[0] {
                    typed_trees::statement::StatementNode::Expression(value) => *value,
                    typed_trees::statement::StatementNode::Transition(transition) => {
                        let typed_trees::statement::TransitionTargetNode::Value(value) =
                            changed.statement_table.transition_target(transition.target)
                        else {
                            panic!("value return")
                        };
                        *value
                    }
                    _ => panic!("value body"),
                };
                *changed.expression_table.expression_mut(value) =
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(8),
                    );
            }
            5 => {
                let value = changed.expression_table.insert(
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(7),
                    ),
                );
                changed
                    .expression_table
                    .set_source_span(value, declaration.initializer_source_span);
                let span = changed.roots.const_declarations;
                changed.tables.const_declarations.span_mut_or_empty(span)[0]
                    .materialized_initializer = value;
            }
            _ => unreachable!(),
        }
        assert!(
            super::validate_retained_invocations(&changed, None).is_err(),
            "mutation {mutation} bypassed replay"
        );
    }
}

#[test]
fn retained_invocation_replay_rejoins_helper_constant_values() {
    let (syntax, sources) = parse(
        "const BASE: u64 = 7;
        machine size() -> u64 { BASE }
        const SIZE: u64 = size();",
    );
    let evaluated =
        super::evaluate(syntax, Some(sources.clone()), &[], None).expect("evaluated dependency");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &evaluated,
            sources: Some(sources),
            top_level_bindings: Vec::new(),
        },
    )
    .expect("retained dependency");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed dependency");
    super::validate_retained_invocations(&typed, None).expect("unchanged dependency");
    for mutate_value in [false, true] {
        let mut changed = typed.clone();
        let span = changed.roots.const_declarations;
        let dependency = &mut changed.tables.const_declarations.span_mut_or_empty(span)[0];
        dependency.canonical_value_encoding =
            Some(CanonicalConstIdentity::integer("u64", 8).encoding);
        if mutate_value {
            let root = dependency.materialized_initializer;
            *changed.expression_table.expression_mut(root) =
                typed_trees::expression::ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(8),
                );
        }
        assert!(
            super::validate_retained_invocations(&changed, None).is_err(),
            "helper constant drift bypassed receiving replay (value changed: {mutate_value})"
        );
    }
}

#[test]
fn retained_invocation_replay_checks_payloadless_constructor_siblings() {
    let (syntax, sources) = parse(
        "data Mode [copy] { case On; case Off; }
        data Config [copy] { mode: Mode; count: u64; }
        machine size() -> u64 { 7 }
        const CONFIG: Config = Config { mode: Mode::On, count: size() };",
    );
    let evaluated =
        super::evaluate(syntax, Some(sources.clone()), &[], None).expect("evaluated record");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &evaluated,
            sources: Some(sources),
            top_level_bindings: Vec::new(),
        },
    )
    .expect("retained record");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed record");
    let declaration = &typed.const_declarations()[0];
    assert!(typed.expression_table.authored_selection_occurrences(declaration.materialized_initializer)
        .filter_map(|occurrence| typed.authored_declaration_selections().get(occurrence))
        .any(|selection| selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call), "materialized constructor retains its complete call roster");
    super::validate_retained_invocations(&typed, None).expect("payloadless sibling replay");
}

#[test]
fn retained_composed_invocation_rejects_erased_roots() {
    let (syntax, sources) = parse("machine size() -> u64 { 7 } const SIZE: u64 = size() * 2;");
    let evaluated =
        super::evaluate(syntax, Some(sources.clone()), &[], None).expect("evaluated composition");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &evaluated,
            sources: Some(sources),
            top_level_bindings: Vec::new(),
        },
    )
    .expect("retained composition");
    let mut typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed composition");
    super::validate_retained_invocations(&typed, None).expect("unchanged composition");
    let span = typed.roots.const_declarations;
    let declaration = &mut typed.tables.const_declarations.span_mut_or_empty(span)[0];
    declaration.authored_initializer = typed_trees::expression::ExpressionHandle::invalid();
    declaration.materialized_initializer = typed_trees::expression::ExpressionHandle::invalid();
    assert!(super::validate_retained_invocations(&typed, None).is_err());
}

#[test]
fn anonymous_fractional_intermediate_lands_once_and_retains_authored_expression() {
    let (syntax, sources) = parse("const SIZE: u64 = 7 / 2 * 2;");
    let original = constant(&syntax, "SIZE").value;
    let original_expression = syntax.expressions.expression(original).clone();
    let original_span = syntax.expressions.source_span(original);
    let evaluated =
        super::evaluate(syntax, Some(sources), &[], None).expect("integral final landing");
    integer_encoding(&evaluated, "SIZE", 7);
    let definition = constant(&evaluated, "SIZE");
    let receipt = definition
        .normalization
        .as_ref()
        .expect("initializer receipt");
    assert_eq!(receipt.authored_expression, original);
    assert_ne!(definition.value, original);
    assert_eq!(
        evaluated.expressions.expression(original),
        &original_expression
    );
    assert_eq!(
        evaluated.expressions.source_span(definition.value),
        original_span
    );
    assert!(receipt.selections.is_empty());
    assert_eq!(receipt.builtin_operators.len(), 2);
}

#[test]
fn forward_boolean_dependency_retains_exact_declaration_and_initializer_custody() {
    let (syntax, sources) = parse("const ENABLED: bool = SIZE == 7; const SIZE: u64 = 7 / 2 * 2;");
    let size = constant(&syntax, "SIZE");
    let declaration = size.name.source_span();
    let initializer = syntax.expressions.source_span(size.value);
    let ExpressionNode::Binary(comparison) = syntax
        .expressions
        .expression(constant(&syntax, "ENABLED").value)
    else {
        panic!("authored comparison");
    };
    let ExpressionNode::Name(path) = syntax.expressions.expression(comparison.left) else {
        panic!("named dependency");
    };
    let reference = syntax.expressions.identifier_path_members(*path)[0].source_span();
    let evaluated = super::evaluate(syntax, Some(sources), &[], None).expect("forward dependency");
    integer_encoding(&evaluated, "SIZE", 7);
    let enabled = constant(&evaluated, "ENABLED");
    assert_eq!(
        evaluated.expressions.expression(enabled.value),
        &ExpressionNode::Boolean(true)
    );
    let receipt = enabled.normalization.as_ref().expect("comparison receipt");
    assert_eq!(
        receipt.canonical_result_encoding,
        CanonicalConstValue::boolean(true).encoding
    );
    assert_eq!(
        receipt.selections,
        vec![syntax_trees::types::ConstArgumentOrigin {
            reference,
            declaration,
            initializer,
            canonical_value_encoding: CanonicalConstIdentity::integer("u64", 7).encoding,
        }]
    );
}

#[test]
fn dependency_layers_preserve_declared_integer_division_and_transitive_origins() {
    let evaluated = evaluate(
        "const RESTORED: u64 = HALF * 2;
         const HALF: u64 = SIZE / 2;
         const SIZE: u64 = 7 / 2 * 2;",
    )
    .expect("typed dependency layers");
    integer_encoding(&evaluated, "SIZE", 7);
    integer_encoding(&evaluated, "HALF", 3);
    integer_encoding(&evaluated, "RESTORED", 6);
    let receipt = constant(&evaluated, "RESTORED")
        .normalization
        .as_ref()
        .expect("layered receipt");
    assert_eq!(receipt.selections.len(), 2);
    for (name, value) in [("HALF", 3), ("SIZE", 7)] {
        assert!(
            receipt.selections.iter().any(|origin| origin.declaration
                == constant(&evaluated, name).name.source_span()
                && origin.canonical_value_encoding
                    == CanonicalConstIdentity::integer("u64", value).encoding),
            "missing {name} provenance: {receipt:?}"
        );
    }
}

#[test]
fn unused_invalid_declarations_cannot_escape_evaluation() {
    for declaration in [
        "const UNUSED: u64 = 7 / 2;",
        "const UNUSED: u8 = 255 + 1;",
        "const UNUSED: u8 = 255u8 + 1u8 - 1u8;",
        "const UNUSED: u64 = 1 / 0;",
    ] {
        let text = format!("{declaration} machine run() -> u64 {{ 0 }}");
        let errors = evaluate(&text).expect_err("unused invalid initializer must reject");
        assert!(!errors.is_empty(), "{declaration}");
    }
}

#[test]
fn initializer_cycles_and_unresolved_operands_reject() {
    for text in [
        "const FIRST: u64 = SECOND + 1; const SECOND: u64 = FIRST + 1;",
        "const SELF: u64 = SELF + 1;",
        "const VALUE: u64 = MISSING + 1;",
    ] {
        let errors = evaluate(text).expect_err("initializer cannot invent a dependency value");
        assert!(!errors.is_empty(), "{text}");
    }
}

#[test]
fn selective_expressions_retain_unselected_dependency_origins() {
    for (expression, expected) in [
        ("false && FLAG", false),
        ("true || FLAG", true),
        ("match true { true -> true, false -> FLAG }", true),
    ] {
        let text = format!("const RESULT: bool = {expression}; const FLAG: bool = 1 == 1;");
        let evaluated = evaluate(&text).expect("valid skipped dependency retains custody");
        let result = constant(&evaluated, "RESULT");
        assert_eq!(
            evaluated.expressions.expression(result.value),
            &ExpressionNode::Boolean(expected)
        );
        let receipt = result.normalization.as_ref().expect("selective receipt");
        assert_eq!(
            receipt.canonical_result_encoding,
            CanonicalConstValue::boolean(expected).encoding
        );
        assert_eq!(receipt.selections.len(), 1);
        assert_eq!(
            receipt.selections[0].declaration,
            constant(&evaluated, "FLAG").name.source_span()
        );
        assert_eq!(
            receipt.selections[0].canonical_value_encoding,
            CanonicalConstValue::boolean(true).encoding
        );
    }
}

#[test]
fn selective_expressions_cannot_hide_invalid_or_unresolved_dependencies() {
    for expression in [
        "false && (BAD == 0)",
        "true || (BAD == 0)",
        "match true { true -> true, false -> BAD == 0 }",
    ] {
        for declaration in ["const BAD: u64 = 7 / 2;", ""] {
            let text = format!("const RESULT: bool = {expression}; {declaration}");
            let errors =
                evaluate(&text).expect_err("unselected dependencies still require admission");
            assert!(!errors.is_empty(), "{text}");
        }
    }
}

#[test]
fn anonymous_decimal_leaves_retain_exact_integer_and_boolean_meaning() {
    let integer = evaluate("const COUNT: u64 = 3.0;").expect("integral anonymous decimal");
    integer_encoding(&integer, "COUNT", 3);
    let evaluated = evaluate("const COUNT: u64 = 1.5 * 2; const FLAG: bool = 0.5 < 1;")
        .expect("anonymous decimals are exact rationals, not landed floating operands");
    integer_encoding(&evaluated, "COUNT", 3);
    let flag = constant(&evaluated, "FLAG");
    assert_eq!(
        evaluated.expressions.expression(flag.value),
        &ExpressionNode::Boolean(true)
    );
    assert_eq!(
        flag.normalization
            .as_ref()
            .expect("decimal comparison receipt")
            .canonical_result_encoding,
        CanonicalConstValue::boolean(true).encoding
    );
}

#[test]
fn bare_aliases_preserve_selected_declared_values() {
    let evaluated = evaluate("const COPY: u64 = VALUE; const VALUE: u64 = 3 + 4;")
        .expect("bare constant initializer alias");
    integer_encoding(&evaluated, "COPY", 7);
    let receipt = constant(&evaluated, "COPY").normalization.as_ref().unwrap();
    assert_eq!(receipt.selections.len(), 1);
    assert_eq!(
        receipt.selections[0].declaration,
        constant(&evaluated, "VALUE").name.source_span()
    );
}

#[test]
fn landed_float_leaves_cannot_be_reinterpreted_as_anonymous_scalar_operands() {
    for text in [
        "const COUNT: u64 = 1.5f32 * 2;",
        "const FLAG: bool = 0.5f32 < 1;",
    ] {
        let errors =
            evaluate(text).expect_err("landed float operands remain outside the scalar evaluator");
        assert!(!errors.is_empty(), "{text}");
    }
}

#[test]
fn aggregate_constant_names_materialize_the_selected_value() {
    let declarations = "data Pair [copy] { first: u64; second: u64; }";
    let evaluated = evaluate(&format!(
        "{declarations}
        const A: [u64; 2] = [4, 9];
        const P: Pair = Pair {{ first: 7, second: 8 }};
        const B: [u64; 2] = A;
        const Q: Pair = P;",
    ))
    .expect("aggregate constant names evaluate");
    for (name, carrier, literal) in [
        ("B", "[u64; 2]", "[4, 9]"),
        ("Q", "Pair", "Pair { first: 7, second: 8 }"),
    ] {
        let definition = constant(&evaluated, name);
        let receipt = definition
            .normalization
            .as_ref()
            .expect("evaluated receipt");
        assert_eq!(
            receipt.canonical_result_encoding,
            literal_encoding(
                &format!("{declarations} const {name}: {carrier} = {literal};"),
                name,
            ),
            "{name}"
        );
        assert!(
            matches!(
                evaluated
                    .expressions
                    .expression(receipt.authored_expression),
                ExpressionNode::Name(_)
            ),
            "{name} retains its authored constant use"
        );
        assert_ne!(receipt.authored_expression, definition.value);
    }
    for (name, dependency) in [("B", "A"), ("Q", "P")] {
        let receipt = constant(&evaluated, name).normalization.as_ref().unwrap();
        assert!(
            receipt.selections.iter().any(|origin| {
                origin.declaration == constant(&evaluated, dependency).name.source_span()
            }),
            "{name} retains the exact selected declaration"
        );
    }
    assert!(matches!(
        evaluated
            .expressions
            .expression(constant(&evaluated, "B").value),
        ExpressionNode::ArrayLiteral(_)
    ));
    assert!(matches!(
        evaluated
            .expressions
            .expression(constant(&evaluated, "Q").value),
        ExpressionNode::StructLiteral(_)
    ));
    crate::syntax_probes::resolve(&evaluated, None, &[])
        .expect("aggregate constant-name receipts rejoin");
    evaluate_fully(
        &[(
            "main.omg",
            &format!(
                "{declarations}
        const A: [u64; 2] = [4, 9];
        const P: Pair = Pair {{ first: 7, second: 8 }};
        const B: [u64; 2] = A;
        const Q: Pair = P;",
            ),
        )],
        &[],
    );
}

#[test]
fn aggregate_producing_calls_replay_both_roots_against_the_canonical_result() {
    let declarations = "data Pair [copy] { first: u64; second: u64; }
        machine make() -> Pair { Pair { first: 1, second: 2 } }
        machine elements() -> [u64; 2] { [3, 4] }";
    let text = format!(
        "{declarations}
        const P: Pair = make();
        const A: [u64; 2] = elements();",
    );
    // Call-custody receipts re-derive machine declaration spans; rejoining
    // them therefore requires the sourced lowering entrypoint.
    let (syntax, sources) = parse(&text);
    let evaluated = super::evaluate(syntax, Some(sources.clone()), &[], None)
        .expect("aggregate-producing calls evaluate");
    for (name, carrier, literal) in [
        ("P", "Pair", "Pair { first: 1, second: 2 }"),
        ("A", "[u64; 2]", "[3, 4]"),
    ] {
        assert_eq!(
            constant(&evaluated, name)
                .normalization
                .as_ref()
                .unwrap()
                .canonical_result_encoding,
            literal_encoding(
                &format!("{declarations} const {name}: {carrier} = {literal};"),
                name,
            ),
            "{name}"
        );
    }
    let receipt = constant(&evaluated, "P").normalization.as_ref().unwrap();
    assert!(
        !receipt.call_selections.is_empty(),
        "aggregate call leaf retains invocation custody"
    );
    assert!(matches!(
        evaluated
            .expressions
            .expression(receipt.authored_expression),
        ExpressionNode::Call(_)
    ));
    assert!(matches!(
        evaluated
            .expressions
            .expression(constant(&evaluated, "P").value),
        ExpressionNode::StructLiteral(_)
    ));
    crate::syntax_probes::resolve(&evaluated, Some(sources), &[])
        .expect("aggregate call receipts rejoin");
    evaluate_fully(&[("main.omg", &text)], &[]);
}

#[test]
fn aggregate_producers_compose_inside_literal_fields_and_match_arms() {
    let declarations = "data Holder [copy] { values: [u64; 2]; tag: u64; }
        data Pair [copy] { first: u64; second: u64; }
        machine elements() -> [u64; 2] { [5, 6] }";
    let text = format!(
        "{declarations}
        const H: Holder = Holder {{ values: elements(), tag: 3 }};
        const M: Pair = match true {{ true -> Pair {{ first: 1, second: 2 }}, false -> Pair {{ first: 9, second: 9 }} }};",
    );
    let evaluated = evaluate(&text).expect("nested aggregate producers evaluate");
    assert_eq!(
        constant(&evaluated, "H")
            .normalization
            .as_ref()
            .unwrap()
            .canonical_result_encoding,
        literal_encoding(
            &format!("{declarations} const H: Holder = Holder {{ values: [5, 6], tag: 3 }};"),
            "H",
        ),
    );
    assert_eq!(
        constant(&evaluated, "M")
            .normalization
            .as_ref()
            .unwrap()
            .canonical_result_encoding,
        literal_encoding(
            &format!("{declarations} const M: Pair = Pair {{ first: 1, second: 2 }};"),
            "M",
        ),
    );
    evaluate_fully(&[("main.omg", &text)], &[]);
}

#[test]
fn module_owned_aggregate_producers_reselect_their_exact_carrier() {
    let main_text =
        "use selected::settings::Pair; use selected::settings::make; const B: Pair = make();";
    let declaration_text = "module settings;
        pub data Pair [copy] { first: u64; second: u64; }
        pub machine make() -> Pair { Pair { first: 7, second: 8 } }";
    let (syntax, sources, ids) = parse_files(&[
        ("root/main.omg", main_text),
        ("dependency/settings.omg", declaration_text),
    ]);
    let [main, declaration] = ids.as_slice() else {
        panic!("module probe sources");
    };
    let bindings = [
        symbols::SourceScopedTopLevelBinding::module_import(
            *main,
            *declaration,
            "selected::settings::Pair",
            1,
        ),
        symbols::SourceScopedTopLevelBinding::module_import(
            *main,
            *declaration,
            "selected::settings::make",
            1,
        ),
    ];
    let evaluated = super::evaluate(syntax, Some(sources), &bindings, None)
        .expect("module-owned aggregate call evaluates");
    let definition = constant(&evaluated, "B");
    let ExpressionNode::StructLiteral(literal) = evaluated.expressions.expression(definition.value)
    else {
        panic!("module-owned aggregate materializes its constructor literal");
    };
    assert_eq!(literal.constructor_name.as_str(), "settings::Pair");
    evaluate_fully(
        &[
            ("root/main.omg", main_text),
            ("dependency/settings.omg", declaration_text),
        ],
        &bindings,
    );
}

#[test]
fn aggregate_receipts_reject_changed_results_and_dropped_call_custody() {
    let text = "data Pair [copy] { first: u64; second: u64; }
        machine make() -> Pair { Pair { first: 1, second: 2 } }
        const P: Pair = make();";
    // The receipt comparison re-derives machine declaration custody; only the
    // sourced lowering can supply those spans, so the mutations below — not
    // the missing source context — must be the cause of rejection.
    let (syntax, sources) = parse(text);
    let evaluated = super::evaluate(syntax, Some(sources.clone()), &[], None)
        .expect("aggregate receipt baseline");
    for mutation in ["result", "calls"] {
        let mut invalid = evaluated.clone();
        let item = *invalid
            .root_item_handles()
            .iter()
            .find(|item| {
                matches!(invalid.root_item(**item), Item::Const(definition) if definition.name.as_str() == "P")
            })
            .unwrap();
        let Item::Const(mut definition) = invalid.root_item(item).clone() else {
            panic!("constant");
        };
        let receipt = definition.normalization.as_mut().unwrap();
        match mutation {
            "result" => receipt.canonical_result_encoding.push('0'),
            "calls" => receipt.call_selections.clear(),
            _ => unreachable!(),
        }
        invalid.items.replace_item(item, Item::Const(definition));
        assert!(
            crate::syntax_probes::resolve(&invalid, Some(sources.clone()), &[]).is_err(),
            "{mutation}"
        );
    }
}
