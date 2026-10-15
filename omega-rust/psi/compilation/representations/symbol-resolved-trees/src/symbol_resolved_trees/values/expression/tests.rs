use super::{
    BinaryOperator, ExpressionNode, ExpressionTable, TableBinaryExpression, TableNamePath,
    TableStructLiteral, TableStructLiteralField,
};
use crate::name::DiagnosticName;
use crate::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelections,
};
use language_semantics::declaration_selection::CompilerDerivedSelectionPartition;
use source::{SourceId, SourceSpan, Span};
use std::sync::Arc;
use symbols::SymbolHandle;

fn authored_selection_occurrences() -> [AuthoredDeclarationSelectionOccurrenceId; 2] {
    let mut selections = AuthoredDeclarationSelections::default();
    let first = selections
        .record_resolved(
            SourceSpan::default(),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            SymbolHandle::from_arena_index(31),
        )
        .expect("valid selected symbol");
    let second = selections
        .record_resolved(
            SourceSpan::default(),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            SymbolHandle::from_arena_index(32),
        )
        .expect("valid selected symbol");
    [first, second]
}

#[test]
fn expression_occurrences_support_multiple_ids_and_survive_resolved_copies() {
    let occurrences = authored_selection_occurrences();
    assert_eq!(occurrences[0].ordinal(), 0);

    let mut source = ExpressionTable::new();
    let expression = source.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(7),
    ));
    source.set_authored_expression_exposure(
        expression,
        AuthoredDeclarationSelectionExposure::PublicInterface,
    );
    let source_span = SourceSpan::new(SourceId(7), Span::new(11, 19));
    source.set_source_span(expression, source_span);
    let partition = CompilerDerivedSelectionPartition::from_compiler_ordinal(9);
    source.set_compiler_selection_partition(expression, partition);
    source.attach_authored_selection_occurrences(expression, occurrences);

    assert_eq!(
        source
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>(),
        occurrences
    );

    let mut copied = ExpressionTable::new();
    let copied_expression = copied.copy_from(&source, expression);
    assert_eq!(
        copied
            .authored_selection_occurrences(copied_expression)
            .collect::<Vec<_>>(),
        occurrences
    );
    assert_eq!(
        copied.authored_expression_exposure(copied_expression),
        Some(AuthoredDeclarationSelectionExposure::PublicInterface)
    );
    assert_eq!(copied.source_span(copied_expression), source_span);
    assert_eq!(
        copied.compiler_selection_partition(copied_expression),
        Some(partition)
    );

    let self_copied_expression = source.copy_from_self(expression);
    assert_eq!(
        source
            .authored_selection_occurrences(self_copied_expression)
            .collect::<Vec<_>>(),
        occurrences
    );
    assert_eq!(
        source.authored_expression_exposure(self_copied_expression),
        Some(AuthoredDeclarationSelectionExposure::PublicInterface)
    );
    assert_eq!(source.source_span(self_copied_expression), source_span);
    assert_eq!(
        source.compiler_selection_partition(self_copied_expression),
        Some(partition)
    );
}

#[test]
fn expression_table_copies_table_payloads_without_tree_roundtrip() {
    let room_symbol = SymbolHandle::from_arena_index(3);
    let field_symbol = SymbolHandle::from_arena_index(4);

    let mut source = ExpressionTable::new();
    let room_members = source.reserve_name_path_members(1);
    source.set_name_path_member_at_offset(room_members, 0, DiagnosticName::generated("room"));
    let room_member_symbols = source.reserve_name_path_member_symbols(1);
    source.set_name_path_member_symbol_at_offset(room_member_symbols, 0, room_symbol);
    let room = source.insert(ExpressionNode::Name(TableNamePath {
        members: room_members,
        member_symbols: room_member_symbols,
        head_symbol: room_symbol,
        symbol: room_symbol,
    }));

    let field_members = source.reserve_name_path_members(2);
    source.set_name_path_member_at_offset(field_members, 0, DiagnosticName::generated("room"));
    source.set_name_path_member_at_offset(field_members, 1, DiagnosticName::generated("field"));
    let field_member_symbols = source.reserve_name_path_member_symbols(2);
    source.set_name_path_member_symbol_at_offset(field_member_symbols, 0, room_symbol);
    source.set_name_path_member_symbol_at_offset(field_member_symbols, 1, field_symbol);
    let field = source.insert(ExpressionNode::Name(TableNamePath {
        members: field_members,
        member_symbols: field_member_symbols,
        head_symbol: room_symbol,
        symbol: field_symbol,
    }));

    let hall = source.insert(ExpressionNode::String(Arc::from(&b"Hall"[..])));
    let open = source.insert(ExpressionNode::Binary(TableBinaryExpression {
        left: room,
        operator: BinaryOperator::Equal,
        right: field,
    }));
    let fields = source.insert_struct_fields([
        TableStructLiteralField {
            name: DiagnosticName::generated("name"),
            field_symbol: SymbolHandle::from_arena_index(6),
            value: hall,
        },
        TableStructLiteralField {
            name: DiagnosticName::generated("open"),
            field_symbol: SymbolHandle::from_arena_index(7),
            value: open,
        },
    ]);
    let root = source.insert(ExpressionNode::StructLiteral(TableStructLiteral {
        type_name: DiagnosticName::generated("Room"),
        type_symbol: SymbolHandle::from_arena_index(5),
        case_name: None,
        case_symbol: None,
        fields,
    }));

    let mut copied = ExpressionTable::new();
    let copied_root = copied.copy_from(&source, root);

    assert_eq!(source.display_name(root), copied.display_name(copied_root));

    let ExpressionNode::StructLiteral(struct_literal) = copied.expression(copied_root) else {
        panic!("copied root should remain a struct literal");
    };
    assert_eq!(copied.struct_fields(struct_literal.fields).len(), 2);
    assert_eq!(
        struct_literal.type_symbol,
        SymbolHandle::from_arena_index(5)
    );
    assert_eq!(
        copied.struct_fields(struct_literal.fields)[0].field_symbol,
        SymbolHandle::from_arena_index(6)
    );

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
    assert_eq!(
        copied.name_path_member_symbols(match copied.expression(binary.right) {
            ExpressionNode::Name(path) => path.member_symbols,
            _ => unreachable!(),
        }),
        [room_symbol, field_symbol]
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
    let own_copy = table.copy_from_self(expression);
    assert_eq!(
        table.display_name(own_copy),
        copied.display_name(copied_expression)
    );
    assert_eq!(table.source_span(own_copy), source_span);
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
            expressions: table.nodes.expressions.storage_slice().len(),
            occurrence_ids: table
                .spans
                .authored_selection_occurrence_ids
                .storage_slice()
                .len(),
            expression_handles: table.spans.expression_handles.storage_slice().len(),
            name_path_members: table.spans.name_path_members.storage_slice().len(),
            name_path_member_symbols: table.spans.name_path_member_symbols.storage_slice().len(),
            struct_fields: table.spans.struct_fields.storage_slice().len(),
            match_arms: table.spans.match_arms.storage_slice().len(),
        }
    }
}

/// One line per row `table` holds past `frontier`: every expression with its
/// source span, authored exposure, compiler partition and occurrence span,
/// then every span arena row, with handles shortened to `#index` (and
/// `#index@generation` off generation 1).
fn render_rows_past(table: &ExpressionTable, frontier: &RowFrontier) -> String {
    let mut lines = Vec::new();
    for (handle, node) in table.iter_expressions().skip(frontier.expressions) {
        let span = table.source_span(handle);
        let occurrences =
            table.nodes.authored_selection_occurrences[super::source_span_index(handle)];
        lines.push(format!(
            "expression {handle:?} = {node:?} at {}:{}..{} exposure {:?} partition {:?} occurrences {occurrences:?}",
            span.source_id.0,
            span.span.start,
            span.span.end,
            table.authored_expression_exposure(handle),
            table.compiler_selection_partition(handle),
        ));
    }
    let occurrence_ids = table
        .spans
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
        table.spans.expression_handles.storage_slice(),
        frontier.expression_handles,
    );
    push_rows_past(
        &mut lines,
        "member",
        table.spans.name_path_members.storage_slice(),
        frontier.name_path_members,
    );
    push_rows_past(
        &mut lines,
        "member symbol",
        table.spans.name_path_member_symbols.storage_slice(),
        frontier.name_path_member_symbols,
    );
    push_rows_past(
        &mut lines,
        "field",
        table.spans.struct_fields.storage_slice(),
        frontier.struct_fields,
    );
    push_rows_past(
        &mut lines,
        "arm",
        table.spans.match_arms.storage_slice(),
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

fn insert_members(
    table: &mut ExpressionTable,
    members: &[&str],
) -> arena::HandleSpan<DiagnosticName> {
    let span = table.reserve_name_path_members(members.len() as u32);
    for (offset, member) in members.iter().enumerate() {
        table.set_name_path_member_at_offset(
            span,
            offset as u32,
            DiagnosticName::generated(*member),
        );
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
/// authored exposures, compiler partitions, occurrence identities, name-path
/// member and symbol spans, call argument and array spans, struct literal
/// fields and match arms.
fn insert_every_node_kind(table: &mut ExpressionTable) -> super::ExpressionHandle {
    use super::{
        MatchPattern, TableAtomicExpression, TableBorrowExpression, TableCallExpression,
        TableCastExpression, TableIndexedExpression, TableMatchArm, TableMatchExpression,
        TableMemberExpression, TableMembershipExpression, TableRangeExpression,
        TableUnaryExpression, UnaryOperator,
    };
    use language_core::atomic::{
        AtomicExpressionResultCustody, AtomicOrderingPlan, MemoryOrdering,
    };
    use numerics::literals::{FloatFormat, FloatLiteral, IntegerLiteral};

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
    table.set_authored_expression_exposure(
        receiver,
        AuthoredDeclarationSelectionExposure::PrivateImplementation,
    );
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
        target_type: arena::Handle::from_arena_index(5),
        target_label,
        domain: numerics::arithmetic::ArithmeticDomain::Exact,
        semantic_domain,
        semantic_domain_arguments: arena::HandleSpan::empty(),
        semantic_domain_symbol: SymbolHandle::from_arena_index(6),
        form: language_core::cast_form::CastForm::Value,
    }));
    table.set_source_span(cast, span(40));
    table.set_compiler_selection_partition(
        cast,
        CompilerDerivedSelectionPartition::from_compiler_ordinal(9),
    );
    let arguments = table.insert_expression_handles([array, cast]);
    let call = table.insert(ExpressionNode::Call(TableCallExpression {
        receiver,
        target_symbol: SymbolHandle::from_arena_index(7),
        target: DiagnosticName::generated("measure"),
        requirement_application: arena::Handle::invalid(),
        machine_arguments: Box::default(),
        arguments,
        evidence_arguments: Box::from([DiagnosticName::generated("witness")]),
        operational_acknowledgement: Default::default(),
    }));
    table.set_source_span(call, span(30));
    table.attach_authored_selection_occurrences(call, [call_occurrence, root_occurrence]);
    table.set_authored_expression_exposure(
        call,
        AuthoredDeclarationSelectionExposure::PublicInterface,
    );

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
            name: DiagnosticName::generated("name"),
            field_symbol: SymbolHandle::from_arena_index(8),
            value: hall,
        },
        TableStructLiteralField {
            name: DiagnosticName::generated("proof"),
            field_symbol: SymbolHandle::from_arena_index(9),
            value: proof,
        },
        TableStructLiteralField {
            name: DiagnosticName::generated("size"),
            field_symbol: SymbolHandle::from_arena_index(10),
            value: cell,
        },
    ]);
    let literal = table.insert(ExpressionNode::StructLiteral(TableStructLiteral {
        type_name: DiagnosticName::generated("Room"),
        type_symbol: SymbolHandle::from_arena_index(11),
        case_name: Some(DiagnosticName::generated("Open")),
        case_symbol: Some(SymbolHandle::from_arena_index(12)),
        fields,
    }));
    table.set_source_span(literal, span(60));

    let counter = insert_name(table, &["cell"], &[], SymbolHandle::invalid());
    let count = table.insert(ExpressionNode::Member(TableMemberExpression {
        receiver: counter,
        member_symbol: count_symbol,
        member: DiagnosticName::generated("count"),
        case_variant: Some(DiagnosticName::generated("Full")),
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
    let domain = insert_members(table, &["Small"]);
    let membership = table.insert(ExpressionNode::Membership(TableMembershipExpression {
        value: nan,
        domain,
        domain_symbol: SymbolHandle::from_arena_index(14),
        case_type_symbol: SymbolHandle::invalid(),
        case_symbol: SymbolHandle::invalid(),
    }));
    let zero_value = table.insert(ExpressionNode::ZeroValue(arena::Handle::from_arena_index(
        13,
    )));

    let arms = table.insert_match_arms([
        TableMatchArm {
            pattern: MatchPattern::Value(range),
            value: literal,
            source_span: span(70),
        },
        TableMatchArm {
            pattern: MatchPattern::Value(atomic),
            value: membership,
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
    table.set_compiler_selection_partition(
        root,
        CompilerDerivedSelectionPartition::from_compiler_ordinal(4),
    );
    root
}

/// Copying from another table and copying within one table share one walk:
/// both append children before parents, allocate each span in the same order
/// and carry every node payload, source span, exposure, partition and
/// occurrence identity. The expected rows pin the exact placement the copies
/// produced before the two walks were merged.
#[test]
fn table_and_own_copies_append_identical_rows_in_child_first_order() {
    let mut source = ExpressionTable::new();
    let root = insert_every_node_kind(&mut source);
    let frontier = RowFrontier::of(&source);

    let mut across = source.clone();
    let across_root = across.copy_from(&source, root);
    let mut within = source.clone();
    let within_root = within.copy_from_self(root);

    assert_eq!(across_root, within_root);
    let rendered = render_rows_past(&across, &frontier);
    assert_eq!(rendered, render_rows_past(&within, &frontier));
    assert!(across == within, "both copies must leave identical tables");
    assert_eq!(rendered, EXPECTED_COPY_ROWS.trim());
}

const EXPECTED_COPY_ROWS: &str = r#"
expression #28 = Name(TableNamePath { members: #9+2, member_symbols: #5+2, head_symbol: #3, symbol: #3 }) at 0:0..0 exposure Some(PrivateImplementation) partition None occurrences #5+1
expression #29 = Integer(IntegerLiteral { text: "1", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #30 = Float(FloatLiteral { text: "2.5", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #31 = ArrayLiteral(#7+2) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #32 = Boolean(true) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #33 = Unary(TableUnaryExpression { operator: LogicalNot, operand: #32 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #34 = Cast(TableCastExpression { value: #33, target_type: #5, target_label: #11+1, domain: Exact, semantic_domain: #12+1, semantic_domain_arguments: #0@0+0, semantic_domain_symbol: #6, form: Value }) at 7:40..45 exposure None partition Some(CompilerDerivedSelectionPartition(9)) occurrences #0@0+0
expression #35 = Call(TableCallExpression { receiver: #28, target_symbol: #7, target: "measure", requirement_application: #0@0, machine_arguments: [], arguments: #5+2, evidence_arguments: ["witness"], operational_acknowledgement: CallOperationalAcknowledgement { origin: Source, acknowledges_suspend: false, acknowledges_block: false } }) at 7:30..35 exposure Some(PublicInterface) partition None occurrences #6+2
expression #36 = Integer(IntegerLiteral { text: "0", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #37 = Range(TableRangeExpression { start: #36, end: #0@0, end_inclusive: false }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #38 = String([72, 97, 108, 108]) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #39 = Integer(IntegerLiteral { text: "11", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #40 = Integer(IntegerLiteral { text: "13", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #41 = Binary(TableBinaryExpression { left: #39, operator: Add, right: #40 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #42 = Name(TableNamePath { members: #13+1, member_symbols: #7+1, head_symbol: #3, symbol: #3 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #43 = Integer(IntegerLiteral { text: "3", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #44 = Indexed(TableIndexedExpression { collection: #42, index: #43 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #45 = StructLiteral(TableStructLiteral { type_name: "Room", type_symbol: #11, case_name: Some("Open"), case_symbol: Some(#12), fields: #4+3 }) at 7:60..65 exposure None partition None occurrences #0@0+0
expression #46 = Name(TableNamePath { members: #14+1, member_symbols: #0@0+0, head_symbol: #0@0, symbol: #0@0 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #47 = Member(TableMemberExpression { receiver: #46, member_symbol: #4, member: "count", case_variant: Some("Full") }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #48 = Borrow(TableBorrowExpression { target: #47, access: Mutable }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #49 = Name(TableNamePath { members: #15+1, member_symbols: #8+1, head_symbol: #4, symbol: #4 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #50 = Atomic(TableAtomicExpression { value: #48, result: #49, ordering: Load(GlobalOrder), result_custody: Scalar }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #51 = UndeterminedNaN(F32) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #52 = Membership(TableMembershipExpression { value: #51, domain: #16+1, domain_symbol: #14, case_type_symbol: #0@0, case_symbol: #0@0 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #53 = ZeroValue(#13) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #54 = Match(TableMatchExpression { subject: #35, arms: #4+3 }) at 7:20..25 exposure None partition Some(CompilerDerivedSelectionPartition(4)) occurrences #8+1
occurrence #5 = AuthoredDeclarationSelectionOccurrenceId(0)
occurrence #6 = AuthoredDeclarationSelectionOccurrenceId(1)
occurrence #7 = AuthoredDeclarationSelectionOccurrenceId(0)
occurrence #8 = AuthoredDeclarationSelectionOccurrenceId(0)
handle #5 = #31
handle #6 = #34
handle #7 = #29
handle #8 = #30
member #9 = "self"
member #10 = "room"
member #11 = "u8"
member #12 = "Meters"
member #13 = "grid"
member #14 = "cell"
member #15 = "previous"
member #16 = "Small"
member symbol #5 = #3
member symbol #6 = #3
member symbol #7 = #3
member symbol #8 = #4
field #4 = TableStructLiteralField { name: "name", field_symbol: #8, value: #38 }
field #5 = TableStructLiteralField { name: "proof", field_symbol: #9, value: #41 }
field #6 = TableStructLiteralField { name: "size", field_symbol: #10, value: #44 }
arm #4 = TableMatchArm { pattern: Value(#37), value: #45, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 70, end: 75 } } }
arm #5 = TableMatchArm { pattern: Value(#50), value: #52, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 80, end: 85 } } }
arm #6 = TableMatchArm { pattern: Wildcard, value: #53, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 90, end: 95 } } }
"#;
