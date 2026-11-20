//! External specialized static evaluation results ("receipts") enter L3 only through
//! `TypingEnvironment::specialized_indices`. `IndexSpecializations::build` admits them
//! before any other typing pass and refuses a receipt for one of five reasons, checked
//! in this order:
//! wrong source identity, unknown node or span, no generic owner, open arguments, and
//! a value that conflicts with an earlier receipt for the same evaluation.
//!
//! Each control starts from one accepted receipt and changes one field. The owner case
//! must move the node and its span together, because a span that disagrees with its node
//! is already an unknown expression. The refusal names the receipt's own claimed `Site`.
//! The tests assert the package, file, byte range and full reason separately.
//! Every case uses a public entry point. No internal boundary is needed.

use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, IndexedSource, ParsedSource, StageSource, SymbolResolvedTrees, TreeNodeId};
use omega_typer::{
    Site, SpecializedIndexValue, StaticValue, StaticValueRequest, TypeError, TypingEnvironment, TypingProgress, ValueType, resolve_source_package,
    type_source_package_for_preparation, type_stage_in_environment,
};

const PACKAGE: &str = "app";
const OWNER_PATH: &str = "a.omg";
const OTHER_PATH: &str = "b.omg";
/// `width(N)` occurs twice in `buffer`. Both occurrences denote the same evaluation.
/// `LIMIT`'s literal is a real expression outside every generic declaration.
const OWNER_SOURCE: &str = "data B<const K:u8>{} machine width(x:u8)->u8 requires x>0 {x-1} machine buffer<const N:u8>()->B<width(N)>{B::<width(N)>{}} machine make()->B<6>{buffer::<7>()} const LIMIT:u8=9;";
const OTHER_SOURCE: &str = "data Other{}";
const EXPRESSION: &str = "width(N)";

const UNADMITTED_FILE: &str = "evaluation result refers to an unadmitted file";
const UNKNOWN_EXPRESSION: &str = "evaluation result refers to an unknown static expression";
const NO_OWNER: &str = "evaluation result has no owning declaration";
const OPEN_ARGUMENTS: &str = "evaluation requires closed arguments";
const CONFLICT: &str = "conflicting evaluations of the same specialized expression";

/// Run `check` on the two admitted files. They are sorted, so `a.omg` has ordinal 0 and `b.omg` has ordinal 1.
fn with_sources<R>(check: impl FnOnce(&[StageSource<'_, '_>]) -> R) -> R {
    let texts = [(OWNER_PATH, OWNER_SOURCE), (OTHER_PATH, OTHER_SOURCE)];
    let tokens: Vec<_> = texts
        .iter()
        .map(|(_, text)| omega_lexer::lex(text.as_bytes()).unwrap())
        .collect();
    let trees: Vec<_> = texts
        .iter()
        .zip(&tokens)
        .map(|((_, text), tokens)| omega_parser::parse_file(text.as_bytes(), tokens).unwrap())
        .collect();
    let files: Vec<_> = texts
        .iter()
        .zip(tokens.iter().zip(&trees))
        .map(|(&(logical_path, text), (tokens, tree))| StageSource {
            package: PACKAGE,
            source: ParsedSource {
                logical_path,
                text,
                tokens: tokens.as_slice(),
                tree,
            },
        })
        .collect();
    check(files.as_slice())
}

fn owner_file<'n>(names: &'n SymbolResolvedTrees<'_>) -> &'n IndexedSource<'n> {
    names
        .trees()
        .find(|tree| tree.file.package == PACKAGE && tree.file.path == OWNER_PATH)
        .expect("the owner file is admitted")
        .file
}

/// Find a node in the production L2 index. The site is built from the expected text.
fn occurrence(
    names: &SymbolResolvedTrees<'_>,
    start: usize,
    text: &str,
    kind: NodeKind,
) -> (TreeNodeId, Site) {
    let file = owner_file(names);
    let span = SourceSpan::new(start, start + text.len());
    let node = (0..file.nodes.len())
        .find(|&n| file.nodes[n].kind == kind && file.span(n) == span)
        .unwrap_or_else(|| panic!("expected {kind:?} {text:?} at {span:?} in {OWNER_PATH}"));
    (
        file.node_id(node),
        Site {
            package: PACKAGE.into(),
            file: OWNER_PATH.into(),
            span,
        },
    )
}

fn u8_type() -> ValueType {
    ValueType::Integer { signed: false, width: Some(8) }
}

fn static_u8(value: u8) -> ValueType {
    ValueType::StaticInteger {
        carrier: Box::new(u8_type()),
        value: value.into(),
    }
}

fn receipt_at(
    (node, expression): (TreeNodeId, Site),
    argument: ValueType,
    value: u8,
) -> SpecializedIndexValue {
    SpecializedIndexValue {
        request: StaticValueRequest {
            node,
            expression,
            carrier: u8_type(),
        },
        arguments: vec![argument],
        value: StaticValue::Integer(value.into()),
    }
}

struct Fixture {
    /// The accepted receipt: `width(N)` in `buffer`'s result, N = 7, value 6.
    valid: SpecializedIndexValue,
    /// The second `width(N)` in `buffer`'s body.
    second: (TreeNodeId, Site),
    /// The literal initializer of the top-level `const LIMIT`.
    ownerless_literal: (TreeNodeId, Site),
    /// The file root.
    root: (TreeNodeId, Site),
    owner_node_count: usize,
    buffer: DeclarationId,
}

fn fixture(names: &SymbolResolvedTrees<'_>) -> Fixture {
    let first = OWNER_SOURCE.find(EXPRESSION).unwrap();
    let second = OWNER_SOURCE.rfind(EXPRESSION).unwrap();
    assert_ne!(first, second);
    let literal = OWNER_SOURCE.find("=9;").unwrap() + 1;
    let file = owner_file(names);
    assert_eq!(file.nodes[0].kind, NodeKind::File);
    assert_eq!(file.nodes[0].parent, None);
    Fixture {
        valid: receipt_at(occurrence(names, first, EXPRESSION, NodeKind::Call), static_u8(7), 6),
        second: occurrence(names, second, EXPRESSION, NodeKind::Call),
        ownerless_literal: occurrence(names, literal, "9", NodeKind::Literal),
        root: (
            file.node_id(0),
            Site {
                package: PACKAGE.into(),
                file: OWNER_PATH.into(),
                span: file.span(0),
            },
        ),
        owner_node_count: file.nodes.len(),
        buffer: names
            .declarations
            .iter()
            .find(|id| id.path == "buffer")
            .expect("buffer is declared")
            .clone(),
    }
}

fn raw(
    names: &SymbolResolvedTrees<'_>,
    receipts: &[SpecializedIndexValue],
) -> Result<(), TypeError> {
    type_stage_in_environment(
        names,
        &TypingEnvironment {
            specialized_indices: receipts,
            ..Default::default()
        },
    )
    .map(|_| ())
}

fn with_raw_stage(check: impl FnOnce(&SymbolResolvedTrees<'_>, &Fixture)) {
    with_sources(|files| {
        let names = omega_resolver::resolve_stage(files, &[]).unwrap();
        let fixture = fixture(&names);
        check(&names, &fixture);
    });
}

/// Assert the package, file, byte range and reason of one refusal.
#[track_caller]
fn assert_refusal(
    result: Result<(), TypeError>,
    package: &str,
    file: &str,
    (start, end): (usize, usize),
    reason: &str,
) {
    let error = result
        .err()
        .unwrap_or_else(|| panic!("expected refusal {reason:?}"));
    assert_eq!(error.message, reason);
    assert_eq!(error.package, package, "package for {reason:?}");
    assert_eq!(error.file, file, "file for {reason:?}");
    assert_eq!(error.span.get_start_offset(), start, "start byte for {reason:?}");
    assert_eq!(error.span.get_end_offset(), end, "end byte for {reason:?}");
    assert!(!error.waiting_for_static_value(), "{reason:?} is a refusal, not a wait");
}

fn first_range() -> (usize, usize) {
    let start = OWNER_SOURCE.find(EXPRESSION).unwrap();
    (start, start + EXPRESSION.len())
}

fn second_range() -> (usize, usize) {
    let start = OWNER_SOURCE.rfind(EXPRESSION).unwrap();
    (start, start + EXPRESSION.len())
}

#[test]
fn specialization_receipt_baseline_is_accepted_and_load_bearing() {
    with_raw_stage(|names, f| {
        // Without the receipt, `make` cannot close `buffer::<7>` to `B<6>`.
        let missing = raw(names, &[]).expect_err("typing needs the receipt");
        assert!(
            missing
                .message
                .contains("specialized static expression in buffer needs an admitted evaluation result"),
            "{missing:?}"
        );
        raw(names, std::slice::from_ref(&f.valid)).expect("the valid receipt is admitted");
        // A repeated receipt with the same value is not a conflict.
        raw(names, &[f.valid.clone(), f.valid.clone()]).expect("a duplicate receipt is admitted");
        // A different argument names a different evaluation. L3 checks only its carrier.
        let mut other_arguments = f.valid.clone();
        other_arguments.arguments = vec![static_u8(8)];
        other_arguments.value = StaticValue::Integer(5.into());
        raw(names, &[f.valid.clone(), other_arguments]).expect("distinct arguments do not conflict");
    });
}

#[test]
fn specialization_receipt_wrong_source_identity_is_an_unadmitted_file() {
    with_raw_stage(|names, f| {
        let range = first_range();
        let mut package = f.valid.clone();
        package.request.expression.package = "other".into();
        assert_refusal(raw(names, &[package]), "other", OWNER_PATH, range, UNADMITTED_FILE);

        // `b.omg` is admitted, but node ordinal 0 is `a.omg`.
        let mut path = f.valid.clone();
        path.request.expression.file = OTHER_PATH.into();
        assert_refusal(raw(names, &[path]), PACKAGE, OTHER_PATH, range, UNADMITTED_FILE);

        // Ordinal 1 is the admitted `b.omg`, not the claimed `a.omg`.
        let mut ordinal = f.valid.clone();
        ordinal.request.node.file = 1;
        assert_refusal(raw(names, &[ordinal]), PACKAGE, OWNER_PATH, range, UNADMITTED_FILE);

        let mut missing = f.valid.clone();
        missing.request.node.file = 2;
        assert_refusal(raw(names, &[missing]), PACKAGE, OWNER_PATH, range, UNADMITTED_FILE);
    });
}

#[test]
fn specialization_receipt_nonexistent_node_or_span_is_an_unknown_expression() {
    with_raw_stage(|names, f| {
        let range = first_range();
        for node in [f.owner_node_count, usize::MAX] {
            let mut past_end = f.valid.clone();
            past_end.request.node.node = node;
            assert_refusal(raw(names, &[past_end]), PACKAGE, OWNER_PATH, range, UNKNOWN_EXPRESSION);
        }

        let mut shifted = f.valid.clone();
        shifted.request.expression.span = SourceSpan::new(range.0 + 1, range.1);
        assert_refusal(raw(names, &[shifted]), PACKAGE, OWNER_PATH, (range.0 + 1, range.1), UNKNOWN_EXPRESSION);

        // The span of the other real occurrence does not belong to this node.
        let mut other_span = f.valid.clone();
        other_span.request.expression.span = f.second.1.span;
        assert_refusal(raw(names, &[other_span]), PACKAGE, OWNER_PATH, second_range(), UNKNOWN_EXPRESSION);

        let mut other_node = f.valid.clone();
        other_node.request.node = f.second.0;
        assert_refusal(raw(names, &[other_node]), PACKAGE, OWNER_PATH, range, UNKNOWN_EXPRESSION);
    });
}

#[test]
fn specialization_receipt_outside_a_generic_declaration_has_no_owner() {
    with_raw_stage(|names, f| {
        let literal = OWNER_SOURCE.find("=9;").unwrap() + 1;
        let mut constant = f.valid.clone();
        (constant.request.node, constant.request.expression) = f.ownerless_literal.clone();
        assert_refusal(raw(names, &[constant]), PACKAGE, OWNER_PATH, (literal, literal + 1), NO_OWNER);

        let mut root = f.valid.clone();
        (root.request.node, root.request.expression) = f.root.clone();
        let root_range = (f.root.1.span.get_start_offset(), f.root.1.span.get_end_offset());
        assert_eq!(root_range, (0, OWNER_SOURCE.len()));
        assert_refusal(raw(names, &[root]), PACKAGE, OWNER_PATH, root_range, NO_OWNER);
    });
}

#[test]
fn specialization_receipt_open_arguments_are_refused() {
    with_raw_stage(|names, f| {
        let parameter = ValueType::Parameter {
            owner: Box::new(f.buffer.clone()),
            index: 0,
        };
        let nested = ValueType::StaticInteger {
            carrier: Box::new(parameter.clone()),
            value: 7.into(),
        };
        for argument in [parameter, nested] {
            let mut open = f.valid.clone();
            open.arguments = vec![argument];
            assert_refusal(raw(names, &[open]), PACKAGE, OWNER_PATH, first_range(), OPEN_ARGUMENTS);
        }
    });
}

#[test]
fn specialization_receipt_conflicting_values_are_refused_at_the_later_receipt() {
    with_raw_stage(|names, f| {
        let mut five = f.valid.clone();
        five.value = StaticValue::Integer(5.into());
        assert_refusal(raw(names, &[f.valid.clone(), five.clone()]), PACKAGE, OWNER_PATH, first_range(), CONFLICT);
        assert_refusal(raw(names, &[five, f.valid.clone()]), PACKAGE, OWNER_PATH, first_range(), CONFLICT);

        // The body occurrence has the same owner and resolved syntax, so it names the same evaluation.
        let second = receipt_at(f.second.clone(), static_u8(7), 5);
        assert_refusal(raw(names, &[f.valid.clone(), second.clone()]), PACKAGE, OWNER_PATH, second_range(), CONFLICT);
        assert_refusal(raw(names, &[second, f.valid.clone()]), PACKAGE, OWNER_PATH, first_range(), CONFLICT);
    });
}

#[test]
fn specialization_receipt_refusal_names_the_earliest_failing_check() {
    with_raw_stage(|names, f| {
        let open = ValueType::Parameter {
            owner: Box::new(f.buffer.clone()),
            index: 0,
        };
        let mut identity_before_node = f.valid.clone();
        identity_before_node.request.expression.package = "other".into();
        identity_before_node.request.node.node = usize::MAX;
        assert_refusal(raw(names, &[identity_before_node]), "other", OWNER_PATH, first_range(), UNADMITTED_FILE);

        let mut node_before_arguments = f.valid.clone();
        node_before_arguments.request.node.node = usize::MAX;
        node_before_arguments.arguments = vec![open.clone()];
        assert_refusal(raw(names, &[node_before_arguments]), PACKAGE, OWNER_PATH, first_range(), UNKNOWN_EXPRESSION);

        let literal = OWNER_SOURCE.find("=9;").unwrap() + 1;
        let mut owner_before_arguments = f.valid.clone();
        (owner_before_arguments.request.node, owner_before_arguments.request.expression) = f.ownerless_literal.clone();
        owner_before_arguments.arguments = vec![open.clone()];
        assert_refusal(raw(names, &[owner_before_arguments]), PACKAGE, OWNER_PATH, (literal, literal + 1), NO_OWNER);

        let mut arguments_before_conflict = f.valid.clone();
        arguments_before_conflict.arguments = vec![open];
        arguments_before_conflict.value = StaticValue::Integer(5.into());
        assert_refusal(
            raw(names, &[f.valid.clone(), arguments_before_conflict]),
            PACKAGE,
            OWNER_PATH,
            first_range(),
            OPEN_ARGUMENTS,
        );

        // Receipts are admitted in order. The first refusal ends admission.
        let mut five = f.valid.clone();
        five.value = StaticValue::Integer(5.into());
        let mut foreign = f.valid.clone();
        foreign.request.expression.package = "other".into();
        assert_refusal(raw(names, &[f.valid.clone(), five, foreign]), PACKAGE, OWNER_PATH, first_range(), CONFLICT);
    });
}

#[test]
fn specialization_receipt_preparation_entry_point_applies_the_same_boundary() {
    with_sources(|files| {
        let names = omega_resolver::resolve_stage(files, &[]).unwrap();
        let package = resolve_source_package(files).unwrap();
        let f = fixture(&names);
        let prepared = |receipts: &[SpecializedIndexValue]| {
            type_source_package_for_preparation(
                &package,
                &TypingEnvironment {
                    specialized_indices: receipts,
                    ..Default::default()
                },
            )
        };
        assert!(
            matches!(prepared(std::slice::from_ref(&f.valid)), Ok(TypingProgress::Complete(_))),
            "the valid receipt completes package typing"
        );

        let mut identity = f.valid.clone();
        identity.request.node.file = 1;
        let mut unknown = f.valid.clone();
        unknown.request.node.node = f.owner_node_count;
        let mut ownerless = f.valid.clone();
        (ownerless.request.node, ownerless.request.expression) = f.ownerless_literal.clone();
        let mut open = f.valid.clone();
        open.arguments = vec![ValueType::Parameter {
            owner: Box::new(f.buffer.clone()),
            index: 0,
        }];
        let mut five = f.valid.clone();
        five.value = StaticValue::Integer(5.into());
        let literal = OWNER_SOURCE.find("=9;").unwrap() + 1;
        for (receipts, range, reason) in [
            (vec![identity], first_range(), UNADMITTED_FILE),
            (vec![unknown], first_range(), UNKNOWN_EXPRESSION),
            (vec![ownerless], (literal, literal + 1), NO_OWNER),
            (vec![open], first_range(), OPEN_ARGUMENTS),
            (vec![f.valid.clone(), five], first_range(), CONFLICT),
        ] {
            let result = prepared(&receipts).map(|_| ());
            assert_eq!(result, raw(&names, &receipts), "{reason:?}");
            spec_corpus::observe_handoff!(
                "specialized-evaluation-input/refusal",
                assert_refusal(result, PACKAGE, OWNER_PATH, range, reason)
            );
        }
    });
}

// Owner-tuple arguments. After typing, L3 checks every receipt's closed arguments against the
// binders of each specialization scope of the owner, outermost first. An integer or Boolean static
// binder needs a value of its own carrier. The normal producer (L4) copies the tuple from L3's
// `StaticValueDemand::Specialized`, so it always satisfies this. A type-correct tuple that the
// current pass does not use stays admissible. Each forged receipt below sits beside a correct
// receipt that serves the application in use, so typing completes and only the forged tuple can refuse.

const ARGUMENT_MISMATCH: &str = "specialized evaluation argument does not match its owner's static parameter";

/// Outer and inner scopes both bind `u64`. Only `Outer<1>::Inner<2>` is applied.
const NESTED_PATH: &str = "nested.omg";
const NESTED_SOURCE: &str =
    "machine width(a:u64,b:u64)->u64{a+b} data Outer<const A:u64>{data Inner<const B:u64>{v:i<width(A,B)>;}} machine consume(x:Outer<1>::Inner<2>){}";
const NESTED_EXPRESSION: &str = "width(A,B)";
/// The outer scope binds `bool` and the inner scope binds `u64`. The expression reads only `B`,
/// but its owner tuple still starts with the outer `A`.
const MIXED_PATH: &str = "mixed.omg";
const MIXED_SOURCE: &str =
    "machine size(b:u64)->u64{b} data Outer<const A:bool>{data Inner<const B:u64>{v:i<size(B)>;}} machine consume(x:Outer<true>::Inner<2>){}";
const MIXED_EXPRESSION: &str = "size(B)";

fn with_file<R>(
    path: &str,
    text: &str,
    check: impl FnOnce(&[StageSource<'_, '_>]) -> R,
) -> R {
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    let files = [StageSource {
        package: PACKAGE,
        source: ParsedSource {
            logical_path: path,
            text,
            tokens: tokens.as_slice(),
            tree: &tree,
        },
    }];
    check(files.as_slice())
}

fn u64_type() -> ValueType {
    ValueType::Integer {
        signed: false,
        width: Some(64),
    }
}

fn static_u64(value: u64) -> ValueType {
    ValueType::StaticInteger {
        carrier: Box::new(u64_type()),
        value: value.into(),
    }
}

fn static_bool(value: bool) -> ValueType {
    ValueType::StaticValue {
        carrier: Box::new(ValueType::Bool),
        value: StaticValue::Bool(value),
    }
}

/// The specialized demands that L3 publishes for `expression` when preparation supplies no receipt.
fn demanded(
    files: &[StageSource<'_, '_>],
    source: &str,
    expression: &str,
) -> Vec<(StaticValueRequest, Vec<ValueType>)> {
    let (start, end) = range_of(source, expression);
    let span = SourceSpan::new(start, end);
    let package = resolve_source_package(files).unwrap();
    let Ok(TypingProgress::Pending(pending)) = type_source_package_for_preparation(&package, &TypingEnvironment::default()) else {
        panic!("{expression} needs a specialized evaluation");
    };
    pending
        .demands()
        .iter()
        .filter_map(|demand| match demand {
            omega_typer::StaticValueDemand::Specialized { request, arguments } if request.expression.span == span => Some((request.clone(), arguments.clone())),
            _ => None,
        })
        .collect()
}

fn prepared_error(
    files: &[StageSource<'_, '_>],
    receipts: &[SpecializedIndexValue],
) -> Option<TypeError> {
    let package = resolve_source_package(files).unwrap();
    type_source_package_for_preparation(
        &package,
        &TypingEnvironment {
            specialized_indices: receipts,
            ..Default::default()
        },
    )
    .err()
}

fn with_arguments(
    receipt: &SpecializedIndexValue,
    arguments: Vec<ValueType>,
) -> SpecializedIndexValue {
    SpecializedIndexValue { arguments, ..receipt.clone() }
}

fn range_of(
    source: &str,
    expression: &str,
) -> (usize, usize) {
    let start = source.find(expression).unwrap();
    (start, start + expression.len())
}

/// Assert a forged tuple refuses beside the used receipt, in either order.
#[track_caller]
fn assert_argument_refusal(
    names: &SymbolResolvedTrees<'_>,
    valid: &SpecializedIndexValue,
    forged: &SpecializedIndexValue,
    file: &str,
    range: (usize, usize),
    message: &str,
) {
    for receipts in [[valid.clone(), forged.clone()], [forged.clone(), valid.clone()]] {
        let result = raw(names, &receipts);
        assert_refusal(result, PACKAGE, file, range, message);
    }
}

#[test]
fn specialization_receipt_owner_tuple_matches_the_normal_producer_demand() {
    with_sources(|files| {
        let names = omega_resolver::resolve_stage(files, &[]).unwrap();
        let f = fixture(&names);
        // L4 answers this demand by copying its request and arguments into the receipt.
        let demands = demanded(files, OWNER_SOURCE, EXPRESSION);
        assert!(
            demands
                .iter()
                .any(|(request, arguments)| request == &f.valid.request && arguments == &f.valid.arguments),
            "buffer::<7> demands width(N) under [7u8]: {demands:?}"
        );
    });
    with_file(NESTED_PATH, NESTED_SOURCE, |files| {
        let demands = demanded(files, NESTED_SOURCE, NESTED_EXPRESSION);
        assert_eq!(demands.len(), 1, "{demands:?}");
        assert_eq!(demands[0].1, vec![static_u64(1), static_u64(2)], "outer argument first");
    });
    with_file(MIXED_PATH, MIXED_SOURCE, |files| {
        let demands = demanded(files, MIXED_SOURCE, MIXED_EXPRESSION);
        assert_eq!(demands.len(), 1, "{demands:?}");
        assert_eq!(
            demands[0].1,
            vec![static_bool(true), static_u64(2)],
            "the unread outer binder stays in the tuple"
        );
    });
}

#[test]
fn specialization_receipt_argument_of_another_kind_is_refused_beside_a_used_receipt() {
    with_raw_stage(|names, f| {
        let integer = |signed, width, value: i64| ValueType::StaticInteger {
            carrier: Box::new(ValueType::Integer { signed, width: Some(width) }),
            value: value.into(),
        };
        for (argument, message) in [
            (static_bool(true), ARGUMENT_MISMATCH),
            (ValueType::Bool, ARGUMENT_MISMATCH),
            (u8_type(), ARGUMENT_MISMATCH),
            (
                ValueType::InvalidStatic("unavailable".into()),
                "invalid evaluation argument 1 of buffer: unavailable",
            ),
            (static_u64(7), ARGUMENT_MISMATCH),
            (integer(true, 32, 7), ARGUMENT_MISMATCH),
            (integer(false, 8, 256), "invalid evaluation argument 1 of buffer: static index 256 exceeds u8"),
            (integer(false, 8, -1), "invalid evaluation argument 1 of buffer: static index -1 exceeds u8"),
        ] {
            let forged = with_arguments(&f.valid, vec![argument]);
            assert_argument_refusal(names, &f.valid, &forged, OWNER_PATH, first_range(), message);
        }
    });
}

#[test]
fn specialization_receipt_type_correct_unused_tuples_stay_admitted() {
    with_raw_stage(|names, f| {
        for n in [0u8, 6, 255] {
            let unused = SpecializedIndexValue {
                value: StaticValue::Integer(n.saturating_sub(1).into()),
                ..with_arguments(&f.valid, vec![static_u8(n)])
            };
            raw(names, &[f.valid.clone(), unused.clone()]).unwrap_or_else(|error| panic!("unused [{n}u8]: {error:?}"));
            raw(names, &[unused, f.valid.clone()]).unwrap_or_else(|error| panic!("unused [{n}u8] first: {error:?}"));
        }
    });
}

#[test]
fn specialization_receipt_arguments_are_checked_in_every_enclosing_scope() {
    with_file(NESTED_PATH, NESTED_SOURCE, |files| {
        let names = omega_resolver::resolve_stage(files, &[]).unwrap();
        let (request, arguments) = demanded(files, NESTED_SOURCE, NESTED_EXPRESSION).remove(0);
        let valid = SpecializedIndexValue {
            request,
            arguments,
            value: StaticValue::Integer(3.into()),
        };
        raw(&names, std::slice::from_ref(&valid)).expect("the demanded tuple is admitted");
        let unused = SpecializedIndexValue {
            value: StaticValue::Integer(14.into()),
            ..with_arguments(&valid, vec![static_u64(5), static_u64(9)])
        };
        raw(&names, &[valid.clone(), unused]).expect("a type-correct unused tuple is admitted");

        let range = range_of(NESTED_SOURCE, NESTED_EXPRESSION);
        for (label, arguments) in [
            ("outer Boolean", vec![static_bool(true), static_u64(2)]),
            ("inner Boolean", vec![static_u64(1), static_bool(true)]),
            ("outer type", vec![u64_type(), static_u64(2)]),
            ("inner type", vec![static_u64(1), u64_type()]),
            ("outer u8", vec![static_u8(1), static_u64(2)]),
            ("inner u8", vec![static_u64(1), static_u8(2)]),
        ] {
            let forged = with_arguments(&valid, arguments);
            assert_argument_refusal(&names, &valid, &forged, NESTED_PATH, range, ARGUMENT_MISMATCH);
            // Preparation reaches the same check after typing completes.
            let receipts = [valid.clone(), forged];
            assert_eq!(prepared_error(files, &receipts), raw(&names, &receipts).err(), "{label}");
        }
    });
}

#[test]
fn specialization_receipt_boolean_and_integer_scopes_keep_their_own_carriers() {
    with_file(MIXED_PATH, MIXED_SOURCE, |files| {
        let names = omega_resolver::resolve_stage(files, &[]).unwrap();
        let (request, arguments) = demanded(files, MIXED_SOURCE, MIXED_EXPRESSION).remove(0);
        let valid = SpecializedIndexValue {
            request,
            arguments,
            value: StaticValue::Integer(2.into()),
        };
        raw(&names, std::slice::from_ref(&valid)).expect("the demanded tuple is admitted");
        for (outer, inner) in [(false, 2u64), (true, 3), (false, 3)] {
            let unused = SpecializedIndexValue {
                value: StaticValue::Integer(inner.into()),
                ..with_arguments(&valid, vec![static_bool(outer), static_u64(inner)])
            };
            raw(&names, &[valid.clone(), unused]).unwrap_or_else(|error| panic!("unused [{outer}, {inner}]: {error:?}"));
        }

        let range = range_of(MIXED_SOURCE, MIXED_EXPRESSION);
        for arguments in [
            vec![static_u64(1), static_u64(2)],
            vec![ValueType::Bool, static_u64(2)],
            vec![static_bool(true), static_bool(false)],
            vec![static_u64(2), static_bool(true)],
        ] {
            let forged = with_arguments(&valid, arguments);
            assert_argument_refusal(&names, &valid, &forged, MIXED_PATH, range, ARGUMENT_MISMATCH);
        }
    });
}
