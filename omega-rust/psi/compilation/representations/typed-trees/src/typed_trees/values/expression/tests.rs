use super::{
    BinaryExpression, BinaryOperator, Expression, ExpressionNode, ExpressionTable, NamePath,
    StructLiteral, StructLiteralField, TableBinaryExpression, TableNamePath,
};
use crate::name::Identifier;
use crate::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelections,
};
use source::SourceSpan;
use std::sync::Arc;
use symbols::SymbolHandle;

#[test]
fn case_membership_meaning_survives_expression_copies_without_becoming_equality() {
    let mut source = ExpressionTable::new();
    let subject = source.insert(ExpressionNode::Boolean(false));
    let classifier = source.insert(ExpressionNode::Boolean(true));
    // The table owns operation identity, not type admission. These leaves keep
    // this storage test independent of nominal-case validation.
    let membership = source.insert(ExpressionNode::Binary(TableBinaryExpression {
        left: subject,
        operator: BinaryOperator::CaseMembership,
        right: classifier,
    }));
    let equality = source.insert(ExpressionNode::Binary(TableBinaryExpression {
        left: subject,
        operator: BinaryOperator::Equal,
        right: classifier,
    }));
    assert!(!source.expressions_structurally_equal(membership, equality));
    let copied = source.insert_copy(membership);
    assert!(source.expressions_structurally_equal(membership, copied));
    let tree = source.to_tree(membership);
    let roundtrip = source.insert_tree(&tree);
    assert!(source.expressions_structurally_equal(membership, roundtrip));
    assert!(!source.expressions_structurally_equal(equality, roundtrip));
    let mut target = ExpressionTable::new();
    let copied = target.copy_from(&source, membership);
    assert_eq!(target.to_tree(copied), tree);
}

fn authored_selection_occurrences() -> [AuthoredDeclarationSelectionOccurrenceId; 2] {
    let mut selections = AuthoredDeclarationSelections::default();
    let first = selections
        .record_resolved(
            SourceSpan::default(),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::StaticPathSegment,
            SymbolHandle::from_arena_index(41),
        )
        .expect("valid selected symbol");
    let second = selections
        .record_resolved(
            SourceSpan::default(),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            SymbolHandle::from_arena_index(42),
        )
        .expect("valid selected symbol");
    [first, second]
}

#[test]
fn expression_occurrences_survive_typed_clone_and_symbol_remap() {
    let occurrences = authored_selection_occurrences();
    let original_symbol = SymbolHandle::from_arena_index(50);
    let remapped_symbol = SymbolHandle::from_arena_index(51);

    let mut source = ExpressionTable::new();
    let members = source.reserve_name_path_members(1);
    source.set_name_path_member_at_offset(members, 0, Identifier::generated("value"));
    let member_symbols = source.reserve_name_path_member_symbols(1);
    source.set_name_path_member_symbol_at_offset(member_symbols, 0, original_symbol);
    let expression = source.insert(ExpressionNode::Name(TableNamePath {
        members,
        member_symbols,
        head_symbol: original_symbol,
        symbol: original_symbol,
    }));
    source.attach_authored_selection_occurrences(expression, occurrences);

    let mut cloned = ExpressionTable::with_capacities(source.copy_capacity());
    let cloned_expression = cloned.copy_from(&source, expression);
    cloned.remap_symbols_in(cloned_expression, &[(original_symbol, remapped_symbol)]);

    assert_eq!(
        cloned
            .authored_selection_occurrences(cloned_expression)
            .collect::<Vec<_>>(),
        occurrences
    );
    let ExpressionNode::Name(path) = cloned.expression(cloned_expression) else {
        panic!("cloned expression should remain a name path");
    };
    assert_eq!(path.head_symbol, remapped_symbol);
    assert_eq!(path.symbol, remapped_symbol);
    assert_eq!(
        cloned.name_path_member_symbols(path.member_symbols),
        [remapped_symbol]
    );

    let self_copied_expression = source.insert_copy(expression);
    source.remap_symbols_in(
        self_copied_expression,
        &[(original_symbol, remapped_symbol)],
    );
    assert_eq!(
        source
            .authored_selection_occurrences(self_copied_expression)
            .collect::<Vec<_>>(),
        occurrences
    );
}

#[test]
fn expression_table_copies_table_payloads_without_tree_roundtrip() {
    let room_symbol = SymbolHandle::from_arena_index(3);
    let field_symbol = SymbolHandle::from_arena_index(4);
    let expression = Expression::StructLiteral(StructLiteral {
        type_name: Identifier::generated("Room"),
        case_name: None,
        fields: Arc::from(
            vec![
                StructLiteralField {
                    name: Identifier::generated("name"),
                    value: Expression::String(Arc::from(&b"Hall"[..])),
                },
                StructLiteralField {
                    name: Identifier::generated("open"),
                    value: Expression::Binary(Box::new(BinaryExpression {
                        left: Expression::Name(NamePath::resolved(
                            vec![Identifier::generated("room")],
                            room_symbol,
                            room_symbol,
                        )),
                        operator: BinaryOperator::Equal,
                        right: Expression::Name(NamePath::resolved(
                            vec![
                                Identifier::generated("room"),
                                Identifier::generated("field"),
                            ],
                            room_symbol,
                            field_symbol,
                        )),
                    })),
                },
            ]
            .into_boxed_slice(),
        ),
    });

    let mut source = ExpressionTable::new();
    let root = source.insert_tree(&expression);

    let mut copied = ExpressionTable::new();
    let copied_root = copied.copy_from(&source, root);

    assert_eq!(source.display_name(root), copied.display_name(copied_root));
    assert_eq!(
        expression.display_name(),
        copied.to_tree(copied_root).display_name()
    );

    let ExpressionNode::StructLiteral(struct_literal) = copied.expression(copied_root) else {
        panic!("copied root should remain a struct literal");
    };
    assert_eq!(copied.struct_fields(struct_literal.fields).len(), 2);

    let open_field = &copied.struct_fields(struct_literal.fields)[1];
    let ExpressionNode::Binary(binary) = copied.expression(open_field.value) else {
        panic!("copied field should keep its binary expression");
    };
    let ExpressionNode::Name(TableNamePath {
        members,
        head_symbol,
        symbol,
        ..
    }) = copied.expression(binary.right)
    else {
        panic!("copied binary rhs should keep its name path");
    };

    assert_eq!(*head_symbol, room_symbol);
    assert_eq!(*symbol, field_symbol);
    assert_eq!(copied.name_path_members(*members).len(), 2);
}

#[test]
fn filtered_copy_never_inserts_rejected_field_subtrees() {
    let expression = Expression::StructLiteral(StructLiteral {
        type_name: Identifier::generated("Certified"),
        case_name: None,
        fields: Arc::from(
            vec![
                StructLiteralField {
                    name: Identifier::generated("value"),
                    value: Expression::Integer(numerics::literals::IntegerLiteral::from_value(7)),
                },
                StructLiteralField {
                    name: Identifier::generated("proof"),
                    value: Expression::Binary(Box::new(BinaryExpression {
                        left: Expression::Integer(numerics::literals::IntegerLiteral::from_value(
                            11,
                        )),
                        operator: BinaryOperator::Add,
                        right: Expression::Integer(numerics::literals::IntegerLiteral::from_value(
                            13,
                        )),
                    })),
                },
            ]
            .into_boxed_slice(),
        ),
    });
    let mut source = ExpressionTable::new();
    let root = source.insert_tree(&expression);
    assert_eq!(source.expression_count(), 5);

    let mut copied = ExpressionTable::new();
    let copied_root =
        copied.copy_from_filtering_struct_literal_fields(&source, root, &|_, field| {
            field.name.as_str() != "proof"
        });

    let ExpressionNode::StructLiteral(literal) = copied.expression(copied_root) else {
        panic!("root should remain a struct literal");
    };
    assert_eq!(copied.struct_fields(literal.fields).len(), 1);
    assert_eq!(copied.expression_count(), 2);
    assert!(
        copied
            .iter_expressions()
            .all(|(_, node)| !matches!(node, ExpressionNode::Binary(_))),
        "rejected initializer subtree must not exist even as unreachable arena nodes"
    );
}
#[test]
fn match_arms_preserve_order_source_and_nonnumeric_values() {
    use super::{MatchPattern, TableMatchArm, TableMatchExpression};
    let mut table = ExpressionTable::new();
    let subject = table.insert(ExpressionNode::Boolean(true));
    let pattern = table.insert(ExpressionNode::Boolean(false));
    let first = table.insert(ExpressionNode::String("first".as_bytes().into()));
    let fallback = table.insert(ExpressionNode::String("fallback".as_bytes().into()));
    let source_span = source::SourceSpan::new(source::SourceId(7), source::Span::new(11, 29));
    let arms = table.insert_match_arms([
        TableMatchArm {
            pattern: MatchPattern::Value(pattern),
            value: first,
            source_span,
        },
        TableMatchArm {
            pattern: MatchPattern::Wildcard,
            value: fallback,
            source_span,
        },
    ]);
    let expression = table.insert(ExpressionNode::Match(TableMatchExpression {
        subject,
        arms,
    }));
    table.set_source_span(expression, source_span);
    assert_eq!(
        table.match_arms(arms)[0].pattern,
        MatchPattern::Value(pattern)
    );
    assert_eq!(table.match_arms(arms)[1].pattern, MatchPattern::Wildcard);
    assert_eq!(table.match_arms(arms)[0].source_span, source_span);
    assert_eq!(
        table.display_name(expression),
        "match true { false => \"first\", _ => \"fallback\" }"
    );

    let mut copied = ExpressionTable::new();
    let copied_expression = copied.copy_from(&table, expression);
    assert_eq!(
        copied.display_name(copied_expression),
        table.display_name(expression)
    );
    assert_eq!(copied.source_span(copied_expression), source_span);
    let ExpressionNode::Match(dispatch) = copied.expression(copied_expression) else {
        panic!("expected match");
    };
    assert_eq!(copied.match_arms(dispatch.arms)[0].source_span, source_span);
    let own_copy = table.insert_copy(expression);
    assert_eq!(
        table.display_name(own_copy),
        copied.display_name(copied_expression)
    );
    assert_eq!(table.source_span(own_copy), source_span);

    let tree = table.to_tree(expression);
    let roundtrip = copied.insert_tree(&tree);
    assert_eq!(copied.to_tree(roundtrip), tree);
    assert!(table.expressions_structurally_equal(expression, own_copy));
}

/// A bare call carries none of the redirecting requests or extra operand
/// kinds, so both shape predicates admit it; each disqualifying field, set on
/// its own, must turn exactly one of them off.
#[test]
fn call_shape_predicates_reject_each_disqualifying_field_alone() {
    use super::{
        PrivateLayoutOperationRequest, QuotientOperationKind, QuotientOperationRequest,
        StaticMachineArgument, TableCallExpression,
    };
    use crate::typed_trees::StaticRequirementDispatch;

    let static_argument = || StaticMachineArgument {
        type_reference: Default::default(),
        path: Box::default(),
        application: None,
        const_literal: None,
        evidence_projection: None,
        symbol: SymbolHandle::invalid(),
    };
    let bare = || TableCallExpression {
        receiver: Default::default(),
        target_symbol: SymbolHandle::invalid(),
        static_machine_parameter: SymbolHandle::invalid(),
        target: Identifier::generated_static("target"),
        static_requirement_dispatch: None,
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        layout_query: None,
        arguments: Default::default(),
        evidence_arguments: Box::default(),
        operational_acknowledgement: Default::default(),
    };

    assert!(bare().selects_only_nominal_route());
    assert!(bare().carries_only_positional_arguments());

    let dispatched = TableCallExpression {
        static_requirement_dispatch: Some(StaticRequirementDispatch::default()),
        ..bare()
    };
    assert!(!dispatched.selects_only_nominal_route());
    assert!(dispatched.carries_only_positional_arguments());

    let quotient = TableCallExpression {
        quotient_operation: Some(QuotientOperationRequest {
            kind: QuotientOperationKind::Lift,
            representative_operation: static_argument(),
            theorem_evidence: Box::default(),
        }),
        ..bare()
    };
    assert!(!quotient.selects_only_nominal_route());
    assert!(quotient.carries_only_positional_arguments());

    let private_layout = TableCallExpression {
        private_layout_operation: Some(PrivateLayoutOperationRequest {
            selected_slot: static_argument(),
        }),
        ..bare()
    };
    assert!(!private_layout.selects_only_nominal_route());
    assert!(private_layout.carries_only_positional_arguments());

    let applied = TableCallExpression {
        machine_arguments: Box::from([static_argument()]),
        ..bare()
    };
    assert!(applied.selects_only_nominal_route());
    assert!(!applied.carries_only_positional_arguments());

    let evidence = TableCallExpression {
        evidence_arguments: Box::from([Identifier::generated_static("witness")]),
        ..bare()
    };
    assert!(evidence.selects_only_nominal_route());
    assert!(!evidence.carries_only_positional_arguments());
}

/// Row counts of every arena a copy appends to, taken before the copy so a
/// rendering shows exactly the rows the copy added.
struct RowFrontier {
    expressions: usize,
    occurrence_ids: usize,
    expression_handles: usize,
    name_path_members: usize,
    name_path_member_symbols: usize,
    struct_fields: usize,
    match_arms: usize,
}

impl RowFrontier {
    fn of(table: &ExpressionTable) -> Self {
        Self {
            expressions: table.expressions.storage_slice().len(),
            occurrence_ids: table
                .authored_selection_occurrence_ids
                .storage_slice()
                .len(),
            expression_handles: table.expression_handles.storage_slice().len(),
            name_path_members: table.name_path_members.storage_slice().len(),
            name_path_member_symbols: table.name_path_member_symbols.storage_slice().len(),
            struct_fields: table.struct_fields.storage_slice().len(),
            match_arms: table.match_arms.storage_slice().len(),
        }
    }
}

/// One line per row `table` holds past `frontier`: every expression with its
/// source span and occurrence span, then every span arena row, with handles
/// shortened to `#index` (and `#index@generation` off generation 1).
fn render_rows_past(table: &ExpressionTable, frontier: &RowFrontier) -> String {
    let mut lines = Vec::new();
    for (handle, node) in table.iter_expressions().skip(frontier.expressions) {
        let span = table.source_span(handle);
        let occurrences = table.authored_selection_occurrences[super::source_span_index(handle)];
        lines.push(format!(
            "expression {handle:?} = {node:?} at {}:{}..{} occurrences {occurrences:?}",
            span.source_id.0, span.span.start, span.span.end
        ));
    }
    let occurrence_ids = table
        .authored_selection_occurrence_ids
        .storage_slice()
        .iter()
        .map(|stored| stored.occurrence())
        .collect::<Vec<_>>();
    push_rows_past(
        &mut lines,
        "occurrence",
        &occurrence_ids,
        frontier.occurrence_ids,
    );
    push_rows_past(
        &mut lines,
        "handle",
        table.expression_handles.storage_slice(),
        frontier.expression_handles,
    );
    push_rows_past(
        &mut lines,
        "member",
        table.name_path_members.storage_slice(),
        frontier.name_path_members,
    );
    push_rows_past(
        &mut lines,
        "member symbol",
        table.name_path_member_symbols.storage_slice(),
        frontier.name_path_member_symbols,
    );
    push_rows_past(
        &mut lines,
        "field",
        table.struct_fields.storage_slice(),
        frontier.struct_fields,
    );
    push_rows_past(
        &mut lines,
        "arm",
        table.match_arms.storage_slice(),
        frontier.match_arms,
    );
    shorten_handles(&lines.join("\n"))
}

fn push_rows_past<Row: std::fmt::Debug>(
    lines: &mut Vec<String>,
    label: &str,
    rows: &[Row],
    frontier: usize,
) {
    for (row_index, row) in rows.iter().enumerate().skip(frontier) {
        lines.push(format!("{label} #{} = {row:?}", row_index + 1));
    }
}

fn shorten_handles(rendered: &str) -> String {
    let handles = shorten_records(
        rendered,
        "Handle { arena_index: ",
        ", generation: ",
        |index, generation| {
            if generation == "1" {
                format!("#{index}")
            } else {
                format!("#{index}@{generation}")
            }
        },
    );
    shorten_records(
        &handles,
        "HandleSpan { start: ",
        ", count: ",
        |start, count| format!("{start}+{count}"),
    )
}

fn shorten_records(
    rendered: &str,
    opening: &str,
    separator: &str,
    shorten: impl Fn(&str, &str) -> String,
) -> String {
    let mut shortened = String::with_capacity(rendered.len());
    let mut rest = rendered;
    while let Some(start) = rest.find(opening) {
        shortened.push_str(&rest[..start]);
        let fields = &rest[start + opening.len()..];
        let (first, after_first) = fields
            .split_once(separator)
            .expect("debug record keeps its field separator");
        let (second, after_record) = after_first
            .split_once(" }")
            .expect("debug record keeps its closing brace");
        shortened.push_str(&shorten(first, second));
        rest = after_record;
    }
    shortened.push_str(rest);
    shortened
}

fn insert_members(table: &mut ExpressionTable, members: &[&str]) -> arena::HandleSpan<Identifier> {
    let span = table.reserve_name_path_members(members.len() as u32);
    for (offset, member) in members.iter().enumerate() {
        table.set_name_path_member_at_offset(span, offset as u32, Identifier::generated(*member));
    }
    span
}

fn insert_name(
    table: &mut ExpressionTable,
    members: &[&str],
    member_symbols: &[SymbolHandle],
    symbol: SymbolHandle,
) -> super::ExpressionHandle {
    let members = insert_members(table, members);
    let symbol_span = table.reserve_name_path_member_symbols(member_symbols.len() as u32);
    for (offset, member_symbol) in member_symbols.iter().enumerate() {
        table.set_name_path_member_symbol_at_offset(symbol_span, offset as u32, *member_symbol);
    }
    table.insert(ExpressionNode::Name(TableNamePath {
        members,
        member_symbols: symbol_span,
        head_symbol: member_symbols.first().copied().unwrap_or_default(),
        symbol,
    }))
}

/// One subtree holding every expression node kind, with source spans,
/// occurrence identities, name-path member and symbol spans, call argument
/// and array spans, struct literal fields and match arms.
fn insert_every_node_kind(table: &mut ExpressionTable) -> super::ExpressionHandle {
    use super::{
        MatchPattern, TableAtomicExpression, TableBorrowExpression, TableCallExpression,
        TableCastExpression, TableIndexedExpression, TableMatchArm, TableMatchExpression,
        TableMemberExpression, TableRangeExpression, TableStructLiteral, TableStructLiteralField,
        TableUnaryExpression, UnaryOperator,
    };
    use crate::types::TypeReferenceHandle;
    use language_core::atomic::{
        AtomicExpressionResultCustody, AtomicOrderingPlan, MemoryOrdering,
    };
    use numerics::literals::{FloatFormat, FloatLiteral, IntegerLiteral};
    use source::{SourceId, Span};

    let [root_occurrence, call_occurrence] = authored_selection_occurrences();
    let room_symbol = SymbolHandle::from_arena_index(3);
    let count_symbol = SymbolHandle::from_arena_index(4);
    let span = |start| SourceSpan::new(SourceId(7), Span::new(start, start + 5));

    let receiver = insert_name(
        table,
        &["self", "room"],
        &[room_symbol, room_symbol],
        room_symbol,
    );
    table.attach_authored_selection_occurrences(receiver, [root_occurrence]);
    let one = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
    let half = table.insert(ExpressionNode::Float(FloatLiteral::new(2.5)));
    let array_values = table.insert_expression_handles([one, half]);
    let array = table.insert(ExpressionNode::ArrayLiteral(array_values));
    let flag = table.insert(ExpressionNode::Boolean(true));
    let not_flag = table.insert(ExpressionNode::Unary(TableUnaryExpression {
        operator: UnaryOperator::LogicalNot,
        operand: flag,
    }));
    let target_label = insert_members(table, &["u8"]);
    let semantic_domain = insert_members(table, &["Meters"]);
    let cast = table.insert(ExpressionNode::Cast(TableCastExpression {
        value: not_flag,
        target_type: TypeReferenceHandle::from_arena_index(5),
        result_type: TypeReferenceHandle::from_arena_index(6),
        target_label,
        domain: numerics::arithmetic::ArithmeticDomain::Exact,
        semantic_domain,
        semantic_domain_arguments: arena::HandleSpan::empty(),
        semantic_domain_symbol: SymbolHandle::from_arena_index(6),
        semantic_domain_id: language_semantics::SemanticDomainId::NULL,
        form: language_core::cast_form::CastForm::Value,
    }));
    table.set_source_span(cast, span(40));
    let arguments = table.insert_expression_handles([array, cast]);
    let call = table.insert(ExpressionNode::Call(TableCallExpression {
        receiver,
        target_symbol: SymbolHandle::from_arena_index(7),
        static_machine_parameter: SymbolHandle::invalid(),
        target: Identifier::generated("measure"),
        static_requirement_dispatch: None,
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        layout_query: None,
        arguments,
        evidence_arguments: Box::from([Identifier::generated("witness")]),
        operational_acknowledgement: Default::default(),
    }));
    table.set_source_span(call, span(30));
    table.attach_authored_selection_occurrences(call, [call_occurrence, root_occurrence]);

    let zero = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
    let range = table.insert(ExpressionNode::Range(TableRangeExpression {
        start: zero,
        end: super::ExpressionHandle::invalid(),
        end_inclusive: false,
    }));
    let hall = table.insert(ExpressionNode::String(Arc::from(&b"Hall"[..])));
    let eleven = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(11)));
    let thirteen = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(13)));
    let proof = table.insert(ExpressionNode::Binary(TableBinaryExpression {
        left: eleven,
        operator: BinaryOperator::Add,
        right: thirteen,
    }));
    let grid = insert_name(table, &["grid"], &[room_symbol], room_symbol);
    let three = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(3)));
    let cell = table.insert(ExpressionNode::Indexed(TableIndexedExpression {
        collection: grid,
        index: three,
    }));
    let fields = table.insert_struct_fields([
        TableStructLiteralField {
            name: Identifier::generated("name"),
            field_symbol: SymbolHandle::from_arena_index(8),
            value: hall,
        },
        TableStructLiteralField {
            name: Identifier::generated("proof"),
            field_symbol: SymbolHandle::from_arena_index(9),
            value: proof,
        },
        TableStructLiteralField {
            name: Identifier::generated("size"),
            field_symbol: SymbolHandle::from_arena_index(10),
            value: cell,
        },
    ]);
    let literal = table.insert(ExpressionNode::StructLiteral(TableStructLiteral {
        type_name: Identifier::generated("Room"),
        type_symbol: SymbolHandle::from_arena_index(11),
        case_name: Some(Identifier::generated("Open")),
        case_symbol: Some(SymbolHandle::from_arena_index(12)),
        fields,
    }));
    table.set_source_span(literal, span(60));

    let counter = insert_name(table, &["cell"], &[], SymbolHandle::invalid());
    let count = table.insert(ExpressionNode::Member(TableMemberExpression {
        receiver: counter,
        member_symbol: count_symbol,
        member: Identifier::generated("count"),
        case_variant: Some(Identifier::generated("Full")),
    }));
    let borrowed = table.insert(ExpressionNode::Borrow(TableBorrowExpression {
        target: count,
        access: language_core::ReferenceAccess::Mutable,
    }));
    let previous = insert_name(table, &["previous"], &[count_symbol], count_symbol);
    let atomic = table.insert(ExpressionNode::Atomic(TableAtomicExpression {
        value: borrowed,
        result: previous,
        ordering: AtomicOrderingPlan::Load(MemoryOrdering::GlobalOrder),
        result_custody: AtomicExpressionResultCustody::Scalar,
    }));
    let nan = table.insert(ExpressionNode::UndeterminedNaN(FloatFormat::F32));
    let zero_value = table.insert(ExpressionNode::ZeroValue(
        TypeReferenceHandle::from_arena_index(13),
    ));

    let arms = table.insert_match_arms([
        TableMatchArm {
            pattern: MatchPattern::Value(range),
            value: literal,
            source_span: span(70),
        },
        TableMatchArm {
            pattern: MatchPattern::Value(atomic),
            value: nan,
            source_span: span(80),
        },
        TableMatchArm {
            pattern: MatchPattern::Wildcard,
            value: zero_value,
            source_span: span(90),
        },
    ]);
    let root = table.insert(ExpressionNode::Match(TableMatchExpression {
        subject: call,
        arms,
    }));
    table.set_source_span(root, span(20));
    table.attach_authored_selection_occurrences(root, [root_occurrence]);
    root
}

/// Copying from another table and copying within one table share one walk:
/// both append children before parents, allocate each span in the same order
/// and carry every node payload, source span and occurrence identity. The
/// expected rows pin the exact placement the copies produced before the two
/// walks were merged.
#[test]
fn table_and_own_copies_append_identical_rows_in_child_first_order() {
    let mut source = ExpressionTable::new();
    let root = insert_every_node_kind(&mut source);
    let frontier = RowFrontier::of(&source);

    let mut across = source.clone();
    let across_root = across.copy_from(&source, root);
    let mut within = source.clone();
    let within_root = within.insert_copy(root);

    assert_eq!(across_root, within_root);
    let rendered = render_rows_past(&across, &frontier);
    assert_eq!(rendered, render_rows_past(&within, &frontier));
    assert!(across == within, "both copies must leave identical tables");
    assert_eq!(rendered, EXPECTED_COPY_ROWS.trim());
}

/// A filtered copy omits the rejected field and never visits its value, so the
/// remaining rows shift down without a gap.
#[test]
fn filtered_copy_appends_rows_without_the_rejected_subtree() {
    let mut source = ExpressionTable::new();
    let root = insert_every_node_kind(&mut source);
    let frontier = RowFrontier::of(&source);

    let mut filtered = source.clone();
    filtered.copy_from_filtering_struct_literal_fields(&source, root, &|_, field| {
        field.name.as_str() != "proof"
    });

    let rendered = render_rows_past(&filtered, &frontier);
    assert_eq!(rendered, EXPECTED_FILTERED_COPY_ROWS.trim());
}

const EXPECTED_COPY_ROWS: &str = r#"
expression #27 = Name(TableNamePath { members: #8+2, member_symbols: #5+2, head_symbol: #3, symbol: #3 }) at 0:0..0 occurrences #5+1
expression #28 = Integer(IntegerLiteral { text: "1", landing: None }) at 0:0..0 occurrences #0@0+0
expression #29 = Float(FloatLiteral { text: "2.5", landing: None }) at 0:0..0 occurrences #0@0+0
expression #30 = ArrayLiteral(#7+2) at 0:0..0 occurrences #0@0+0
expression #31 = Boolean(true) at 0:0..0 occurrences #0@0+0
expression #32 = Unary(TableUnaryExpression { operator: LogicalNot, operand: #31 }) at 0:0..0 occurrences #0@0+0
expression #33 = Cast(TableCastExpression { value: #32, target_type: #5, result_type: #6, target_label: #10+1, domain: Exact, semantic_domain: #11+1, semantic_domain_arguments: #0@0+0, semantic_domain_symbol: #6, semantic_domain_id: SemanticDomainId(0), form: Value }) at 7:40..45 occurrences #0@0+0
expression #34 = Call(TableCallExpression { receiver: #27, target_symbol: #7, static_machine_parameter: #0@0, target: "measure", static_requirement_dispatch: None, machine_arguments: [], quotient_operation: None, private_layout_operation: None, layout_query: None, arguments: #5+2, evidence_arguments: ["witness"], operational_acknowledgement: CallOperationalAcknowledgement { origin: Source, acknowledges_suspend: false, acknowledges_block: false } }) at 7:30..35 occurrences #6+2
expression #35 = Integer(IntegerLiteral { text: "0", landing: None }) at 0:0..0 occurrences #0@0+0
expression #36 = Range(TableRangeExpression { start: #35, end: #0@0, end_inclusive: false }) at 0:0..0 occurrences #0@0+0
expression #37 = String([72, 97, 108, 108]) at 0:0..0 occurrences #0@0+0
expression #38 = Integer(IntegerLiteral { text: "11", landing: None }) at 0:0..0 occurrences #0@0+0
expression #39 = Integer(IntegerLiteral { text: "13", landing: None }) at 0:0..0 occurrences #0@0+0
expression #40 = Binary(TableBinaryExpression { left: #38, operator: Add, right: #39 }) at 0:0..0 occurrences #0@0+0
expression #41 = Name(TableNamePath { members: #12+1, member_symbols: #7+1, head_symbol: #3, symbol: #3 }) at 0:0..0 occurrences #0@0+0
expression #42 = Integer(IntegerLiteral { text: "3", landing: None }) at 0:0..0 occurrences #0@0+0
expression #43 = Indexed(TableIndexedExpression { collection: #41, index: #42 }) at 0:0..0 occurrences #0@0+0
expression #44 = StructLiteral(TableStructLiteral { type_name: "Room", type_symbol: #11, case_name: Some("Open"), case_symbol: Some(#12), fields: #4+3 }) at 7:60..65 occurrences #0@0+0
expression #45 = Name(TableNamePath { members: #13+1, member_symbols: #0@0+0, head_symbol: #0@0, symbol: #0@0 }) at 0:0..0 occurrences #0@0+0
expression #46 = Member(TableMemberExpression { receiver: #45, member_symbol: #4, member: "count", case_variant: Some("Full") }) at 0:0..0 occurrences #0@0+0
expression #47 = Borrow(TableBorrowExpression { target: #46, access: Mutable }) at 0:0..0 occurrences #0@0+0
expression #48 = Name(TableNamePath { members: #14+1, member_symbols: #8+1, head_symbol: #4, symbol: #4 }) at 0:0..0 occurrences #0@0+0
expression #49 = Atomic(TableAtomicExpression { value: #47, result: #48, ordering: Load(GlobalOrder), result_custody: Scalar }) at 0:0..0 occurrences #0@0+0
expression #50 = UndeterminedNaN(F32) at 0:0..0 occurrences #0@0+0
expression #51 = ZeroValue(#13) at 0:0..0 occurrences #0@0+0
expression #52 = Match(TableMatchExpression { subject: #34, arms: #4+3 }) at 7:20..25 occurrences #8+1
occurrence #5 = AuthoredDeclarationSelectionOccurrenceId(0)
occurrence #6 = AuthoredDeclarationSelectionOccurrenceId(1)
occurrence #7 = AuthoredDeclarationSelectionOccurrenceId(0)
occurrence #8 = AuthoredDeclarationSelectionOccurrenceId(0)
handle #5 = #30
handle #6 = #33
handle #7 = #28
handle #8 = #29
member #8 = "self"
member #9 = "room"
member #10 = "u8"
member #11 = "Meters"
member #12 = "grid"
member #13 = "cell"
member #14 = "previous"
member symbol #5 = #3
member symbol #6 = #3
member symbol #7 = #3
member symbol #8 = #4
field #4 = TableStructLiteralField { name: "name", field_symbol: #8, value: #37 }
field #5 = TableStructLiteralField { name: "proof", field_symbol: #9, value: #40 }
field #6 = TableStructLiteralField { name: "size", field_symbol: #10, value: #43 }
arm #4 = TableMatchArm { pattern: Value(#36), value: #44, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 70, end: 75 } } }
arm #5 = TableMatchArm { pattern: Value(#49), value: #50, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 80, end: 85 } } }
arm #6 = TableMatchArm { pattern: Wildcard, value: #51, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 90, end: 95 } } }
"#;

const EXPECTED_FILTERED_COPY_ROWS: &str = r#"
expression #27 = Name(TableNamePath { members: #8+2, member_symbols: #5+2, head_symbol: #3, symbol: #3 }) at 0:0..0 occurrences #5+1
expression #28 = Integer(IntegerLiteral { text: "1", landing: None }) at 0:0..0 occurrences #0@0+0
expression #29 = Float(FloatLiteral { text: "2.5", landing: None }) at 0:0..0 occurrences #0@0+0
expression #30 = ArrayLiteral(#7+2) at 0:0..0 occurrences #0@0+0
expression #31 = Boolean(true) at 0:0..0 occurrences #0@0+0
expression #32 = Unary(TableUnaryExpression { operator: LogicalNot, operand: #31 }) at 0:0..0 occurrences #0@0+0
expression #33 = Cast(TableCastExpression { value: #32, target_type: #5, result_type: #6, target_label: #10+1, domain: Exact, semantic_domain: #11+1, semantic_domain_arguments: #0@0+0, semantic_domain_symbol: #6, semantic_domain_id: SemanticDomainId(0), form: Value }) at 7:40..45 occurrences #0@0+0
expression #34 = Call(TableCallExpression { receiver: #27, target_symbol: #7, static_machine_parameter: #0@0, target: "measure", static_requirement_dispatch: None, machine_arguments: [], quotient_operation: None, private_layout_operation: None, layout_query: None, arguments: #5+2, evidence_arguments: ["witness"], operational_acknowledgement: CallOperationalAcknowledgement { origin: Source, acknowledges_suspend: false, acknowledges_block: false } }) at 7:30..35 occurrences #6+2
expression #35 = Integer(IntegerLiteral { text: "0", landing: None }) at 0:0..0 occurrences #0@0+0
expression #36 = Range(TableRangeExpression { start: #35, end: #0@0, end_inclusive: false }) at 0:0..0 occurrences #0@0+0
expression #37 = String([72, 97, 108, 108]) at 0:0..0 occurrences #0@0+0
expression #38 = Name(TableNamePath { members: #12+1, member_symbols: #7+1, head_symbol: #3, symbol: #3 }) at 0:0..0 occurrences #0@0+0
expression #39 = Integer(IntegerLiteral { text: "3", landing: None }) at 0:0..0 occurrences #0@0+0
expression #40 = Indexed(TableIndexedExpression { collection: #38, index: #39 }) at 0:0..0 occurrences #0@0+0
expression #41 = StructLiteral(TableStructLiteral { type_name: "Room", type_symbol: #11, case_name: Some("Open"), case_symbol: Some(#12), fields: #4+2 }) at 7:60..65 occurrences #0@0+0
expression #42 = Name(TableNamePath { members: #13+1, member_symbols: #0@0+0, head_symbol: #0@0, symbol: #0@0 }) at 0:0..0 occurrences #0@0+0
expression #43 = Member(TableMemberExpression { receiver: #42, member_symbol: #4, member: "count", case_variant: Some("Full") }) at 0:0..0 occurrences #0@0+0
expression #44 = Borrow(TableBorrowExpression { target: #43, access: Mutable }) at 0:0..0 occurrences #0@0+0
expression #45 = Name(TableNamePath { members: #14+1, member_symbols: #8+1, head_symbol: #4, symbol: #4 }) at 0:0..0 occurrences #0@0+0
expression #46 = Atomic(TableAtomicExpression { value: #44, result: #45, ordering: Load(GlobalOrder), result_custody: Scalar }) at 0:0..0 occurrences #0@0+0
expression #47 = UndeterminedNaN(F32) at 0:0..0 occurrences #0@0+0
expression #48 = ZeroValue(#13) at 0:0..0 occurrences #0@0+0
expression #49 = Match(TableMatchExpression { subject: #34, arms: #4+3 }) at 7:20..25 occurrences #8+1
occurrence #5 = AuthoredDeclarationSelectionOccurrenceId(0)
occurrence #6 = AuthoredDeclarationSelectionOccurrenceId(1)
occurrence #7 = AuthoredDeclarationSelectionOccurrenceId(0)
occurrence #8 = AuthoredDeclarationSelectionOccurrenceId(0)
handle #5 = #30
handle #6 = #33
handle #7 = #28
handle #8 = #29
member #8 = "self"
member #9 = "room"
member #10 = "u8"
member #11 = "Meters"
member #12 = "grid"
member #13 = "cell"
member #14 = "previous"
member symbol #5 = #3
member symbol #6 = #3
member symbol #7 = #3
member symbol #8 = #4
field #4 = TableStructLiteralField { name: "name", field_symbol: #8, value: #37 }
field #5 = TableStructLiteralField { name: "size", field_symbol: #10, value: #40 }
arm #4 = TableMatchArm { pattern: Value(#36), value: #41, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 70, end: 75 } } }
arm #5 = TableMatchArm { pattern: Value(#46), value: #47, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 80, end: 85 } } }
arm #6 = TableMatchArm { pattern: Wildcard, value: #48, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 90, end: 95 } } }
"#;
