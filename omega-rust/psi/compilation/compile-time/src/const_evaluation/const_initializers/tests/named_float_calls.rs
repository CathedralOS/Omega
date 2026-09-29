//! Named floating boundary calls (`F32::square_root`) materialize through the
//! ordinary invocation route. The call keeps its exact operator declaration
//! identity, the checked interpreter's named-requirement route supplies the
//! value through `FloatSemantics`, and the one canonical float landing
//! publishes exact format and bits — a bodyless requirement owns no machine
//! body to traverse, so dependency collection records the call's custody
//! without queueing it.

use super::{evaluate, evaluate_fully};
use language_semantics::const_value::CanonicalConstValue;
use numerics::literals::{FloatFormat, FloatLiteral};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

fn declared_encoding(syntax: &syntax_trees::SyntaxTrees, name: &str) -> String {
    super::constant(syntax, name)
        .normalization
        .as_ref()
        .expect("evaluated declaration")
        .canonical_result_encoding
        .clone()
}

fn typed_encoding(typed: &typed_trees::TypedTrees, name: &str) -> Option<String> {
    typed
        .const_declarations()
        .iter()
        .find(|declaration| typed.symbols.name(declaration.symbol) == name)
        .and_then(|declaration| declaration.canonical_value_encoding.clone())
}

fn float_leaf(program: &typed_trees::TypedTrees, root: ExpressionHandle) -> ExpressionHandle {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression(expression) {
            ExpressionNode::Float(_) => return expression,
            ExpressionNode::ArrayLiteral(elements) => pending.extend(
                program
                    .expression_table
                    .expression_handles(*elements)
                    .iter()
                    .copied(),
            ),
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            _ => {}
        }
    }
    panic!("materialized initializer contains a floating leaf");
}

#[test]
fn repro_named_float_boundary_call() {
    let text = "pub data F32 { }
        pub boundary operator F32::square_root(value: f32) -> f32;
        pub const ROOT: f32 = F32::square_root(4.0f32);";
    let typed = evaluate_fully(&[("main.omg", text)], &[]);
    assert_eq!(
        typed.const_declarations()[0]
            .canonical_value_encoding
            .as_deref(),
        Some("float:f32:40000000")
    );
}

/// Every named requirement family the checked interpreter's boundary route
/// answers — arithmetic helpers, directed rounding, fused forms, minimum and
/// maximum, and the Boolean classification predicates — must enter through
/// the same admitted call path and land at its declared carrier.
#[test]
fn named_float_boundary_families_reuse_the_one_checked_route() {
    let text = "pub data F32 { }
        pub data F64 { }
        pub boundary operator F32::negate(value: f32) -> f32;
        pub boundary operator F32::square_root(value: f32) -> f32;
        pub boundary operator F32::square_root_toward_zero(value: f32) -> f32;
        pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
        pub boundary operator F32::maximum(left: f32, right: f32) -> f32;
        pub boundary operator F32::add_toward_positive(left: f32, right: f32) -> f32;
        pub boundary operator F32::divide_toward_negative(left: f32, right: f32) -> f32;
        pub boundary operator F32::multiply_then_add(left: f32, right: f32, addend: f32) -> f32;
        pub boundary operator F32::fused_multiply_add(left: f32, right: f32, addend: f32) -> f32;
        pub boundary operator F32::is_nan(value: f32) -> bool;
        pub boundary operator F32::is_finite(value: f32) -> bool;
        pub boundary operator F32::is_infinite(value: f32) -> bool;
        pub boundary operator F32::is_normal(value: f32) -> bool;
        pub boundary operator F32::is_subnormal(value: f32) -> bool;
        pub boundary operator F64::square_root(value: f64) -> f64;
        const NEGATED: f32 = F32::negate(1.5f32);
        const ROOT: f32 = F32::square_root(4.0f32);
        const DIRECTED_ROOT: f32 = F32::square_root_toward_zero(4.0f32);
        const LOW: f32 = F32::minimum(1.5f32, 2.5f32);
        const HIGH: f32 = F32::maximum(1.5f32, 2.5f32);
        const DIRECTED_SUM: f32 = F32::add_toward_positive(1.0f32, 2.0f32);
        const QUOTIENT: f32 = F32::divide_toward_negative(7.0f32, 2.0f32);
        const PAIRED: f32 = F32::multiply_then_add(2.0f32, 3.0f32, 1.0f32);
        const FUSED: f32 = F32::fused_multiply_add(2.0f32, 3.0f32, 1.0f32);
        const NEGATED_ZERO: f32 = F32::negate(0.0f32);
        const NESTED: f32 = F32::square_root(F32::square_root(16.0f32));
        const ROOT64: f64 = F64::square_root(4.0f64);
        const NAN_FLAG: bool = F32::is_nan(1.0f32);
        const FINITE: bool = F32::is_finite(1.5f32);
        const INFINITE: bool = F32::is_infinite(1.5f32);
        const NORMAL: bool = F32::is_normal(1.5f32);
        const SUBNORMAL: bool = F32::is_subnormal(1.5f32);";
    let syntax = evaluate(text).expect("named float boundary families evaluate");
    for (name, expected) in [
        ("NEGATED", "float:f32:bfc00000"),
        ("ROOT", "float:f32:40000000"),
        ("DIRECTED_ROOT", "float:f32:40000000"),
        ("LOW", "float:f32:3fc00000"),
        ("HIGH", "float:f32:40200000"),
        ("DIRECTED_SUM", "float:f32:40400000"),
        ("QUOTIENT", "float:f32:40600000"),
        ("PAIRED", "float:f32:40e00000"),
        ("FUSED", "float:f32:40e00000"),
        ("NEGATED_ZERO", "float:f32:80000000"),
        ("NESTED", "float:f32:40000000"),
        ("ROOT64", "float:f64:4000000000000000"),
    ] {
        assert_eq!(declared_encoding(&syntax, name), expected, "{name}");
    }
    for (name, expected) in [
        ("NAN_FLAG", false),
        ("FINITE", true),
        ("INFINITE", false),
        ("NORMAL", true),
        ("SUBNORMAL", false),
    ] {
        assert_eq!(
            declared_encoding(&syntax, name),
            CanonicalConstValue::boolean(expected).encoding,
            "{name}"
        );
    }
}

/// A named requirement nested inside a structured leaf still gets the full
/// admission/custody treatment: the aggregate leaf's interpreter supplies the
/// result while the call's authored occurrence↔declaration join is retained
/// for receipt replay — corrupting the materialized bits must reject.
#[test]
fn named_float_call_inside_an_aggregate_leaf_keeps_call_custody() {
    let text = "pub data F32 { }
        pub boundary operator F32::square_root(value: f32) -> f32;
        data Pair [copy] { root: f32; }
        const PAIR: Pair = Pair { root: F32::square_root(4.0f32) };";
    let original = evaluate_fully(&[("main.omg", text)], &[]);
    let declaration = original.const_declarations()[0].clone();
    let leaf = float_leaf(&original, declaration.materialized_initializer);
    let mut changed = original.clone();
    *changed.expression_table.expression_mut(leaf) = ExpressionNode::Float(
        FloatLiteral::parse("3.0")
            .unwrap()
            .with_landing(FloatFormat::F32),
    );
    assert!(
        super::super::replay::validate(&changed, None).is_err(),
        "corrupting a named-requirement leaf must reject on replay"
    );
}

/// Module-qualified boundary calls keep their imported declaration identity:
/// `geom::F32::square_root` selects the exact `geom` operator through the
/// source-aware signature-free lookup, not a same-spelled local family.
#[test]
fn module_qualified_float_boundary_keeps_its_imported_identity() {
    let typed = evaluate_fully(
        &[
            (
                "geom.omg",
                "module geom; pub data F32 { }
                 pub boundary operator F32::square_root(value: f32) -> f32;",
            ),
            (
                "mine.omg",
                "module mine;
                 pub const ROOT: f32 = geom::F32::square_root(4.0f32);",
            ),
        ],
        &[],
    );
    assert_eq!(
        typed_encoding(&typed, "ROOT").as_deref(),
        Some("float:f32:40000000")
    );
}

/// An ambiguously named family stays unresolved through symbol stamping so
/// path-and-arity named-call selection declines it — the pipeline must never
/// mint an early identity for `Math::same`.
#[test]
fn ambiguous_named_float_family_rejects_instead_of_manufacturing_identity() {
    let text = "data Math { }
        operator Math::same(value: f32) -> f32;
        operator Math::same(value: f64) -> f64;
        const SAME: f32 = Math::same(1.0f32);";
    let errors = evaluate(text).expect_err("ambiguous named operator call must reject");
    assert!(!errors.is_empty());
}

/// An evaluator NaN result has no authored raw-NaN realization custody, so it
/// must never be published into constant bytes — determined payloads only.
#[test]
fn evaluator_nan_payload_cannot_enter_published_constant_bytes() {
    let text = "pub data F32 { }
        pub boundary operator F32::divide_toward_zero(left: f32, right: f32) -> f32;
        const QUOTIENT: f32 = F32::divide_toward_zero(0.0f32, 0.0f32);";
    let errors = evaluate(text).expect_err("NaN without exact raw-NaN realization must reject");
    assert!(
        errors.iter().any(|error| error.message.contains("NaN")),
        "unexpected diagnostics: {errors:?}"
    );
}

#[test]
fn payloadless_nan_intermediates_produce_determined_observations() {
    let text = "pub data F32 {}
        pub data F64 {}
        pub boundary operator F32::square_root(value: f32) -> f32;
        pub boundary operator F64::square_root(value: f64) -> f64;
        pub boundary operator F32::is_nan(value: f32) -> bool;
        pub boundary operator F64::is_nan(value: f64) -> bool;
        machine classify(value: f32) -> bool { F32::is_nan(value) }
        machine guarded(value: f32, ready: bool) -> bool requires ready { F32::is_nan(value) }
        machine produce() -> f32 { F32::square_root(-1.0f32) }
        pub const CLASSIFIED: bool = F32::is_nan(F32::square_root(-1.0f32));
        pub const CLASSIFIED64: bool = F64::is_nan(F64::square_root(-1.0f64));
        pub const THROUGH_CALL: bool = classify(produce());
        pub const GUARDED: bool = guarded(produce(), true);
        pub const UNORDERED: bool = (0.0f32 / 0.0f32) != 1.0f32;
        pub const PROPAGATED: bool = F32::is_nan((0.0f32 / 0.0f32) + 1.0f32);";
    let typed = evaluate_fully(&[("main.omg", text)], &[]);
    let expected = CanonicalConstValue::boolean(true).encoding;
    for name in [
        "CLASSIFIED",
        "CLASSIFIED64",
        "THROUGH_CALL",
        "GUARDED",
        "UNORDERED",
        "PROPAGATED",
    ] {
        assert_eq!(
            typed_encoding(&typed, name).as_deref(),
            Some(expected.as_str()),
            "{name}"
        );
    }
    let declaration = typed
        .const_declarations()
        .iter()
        .find(|declaration| typed.symbols.name(declaration.symbol) == "CLASSIFIED")
        .expect("classified constant");
    let materialized = declaration.materialized_initializer;
    let mut forged = typed.clone();
    *forged.expression_table.expression_mut(materialized) = ExpressionNode::Boolean(false);
    assert!(
        super::super::replay::validate(&forged, None).is_err(),
        "semantic NaN evaluation must still independently replay its Boolean result"
    );
    let rejected = evaluate(&text.replace("guarded(produce(), true)", "guarded(produce(), false)"))
        .expect_err("a semantic NaN argument cannot waive an authored premise");
    assert!(
        rejected
            .iter()
            .any(|diagnostic| diagnostic.message.contains("failed checking")),
        "{rejected:?}"
    );
    let rejected = evaluate(
        "pub data F32 {}
        pub boundary operator F32::square_root(value: f32) -> f32;
        machine bits(value: f32) -> u32 {
            let view: &u32 = &value as &u32;
            let result: u32 = view;
            result
        }
        pub const BITS: u32 = bits(F32::square_root(-1.0f32));",
    )
    .expect_err("returning an integer cannot authorize undetermined NaN bits");
    assert!(
        rejected
            .iter()
            .any(|diagnostic| diagnostic.message.contains("NaN")),
        "{rejected:?}"
    );
}

/// A retained call whose stamped target is repointed at a different overload
/// smuggles a forged selection: the rederived roster must still match the
/// authored declaration-selection receipt, so replay rejects the substitution
/// rather than trusting the forged identity or its drifted value.
#[test]
fn forged_named_float_selection_rejects_on_replay() {
    let text = "pub data F32 { }
        pub boundary operator F32::negate(value: f32) -> f32;
        pub boundary operator F32::square_root(value: f32) -> f32;
        const ROOT: f32 = F32::square_root(4.0f32);";
    let original = evaluate_fully(&[("main.omg", text)], &[]);
    let declaration = original.const_declarations()[0].clone();
    let mut pending = vec![declaration.authored_initializer];
    let call = loop {
        let expression = pending.pop().expect("retained recipe contains the call");
        match original.expression_table.expression(expression) {
            ExpressionNode::Call(_) => break expression,
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            _ => {}
        }
    };
    let forged = original
        .operators()
        .iter()
        .find(|operator| {
            original
                .operator_path_members(operator.name)
                .last()
                .is_some_and(|member| member.as_str() == "negate")
        })
        .expect("negate requirement")
        .symbol;
    let mut changed = original.clone();
    let ExpressionNode::Call(node) = changed.expression_table.expression_mut(call) else {
        panic!("retained recipe lost its call");
    };
    node.target_symbol = forged;
    assert!(
        super::super::replay::validate(&changed, None).is_err(),
        "a repointed named-requirement target must reject on replay"
    );
}

/// A named requirement used as an argument inside an ordinary machine call
/// still lands through the shared route: the machine path evaluates its
/// operand snapshot first, then the ordinary body runs.
#[test]
fn named_float_result_feeds_an_ordinary_machine_call() {
    let text = "pub data F32 { }
        pub boundary operator F32::square_root(value: f32) -> f32;
        machine double(value: f32) -> f32 { value * 2.0f32 }
        const RESULT: f32 = double(F32::square_root(4.0f32));";
    let syntax = evaluate(text).expect("machine call over a named float requirement");
    assert_eq!(declared_encoding(&syntax, "RESULT"), "float:f32:40800000");
}
