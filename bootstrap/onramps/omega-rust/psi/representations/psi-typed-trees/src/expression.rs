use crate::name::Identifier;
use psi_arena::{Arena, Handle, HandleSpan};
use psi_numerics::literals::IntegerLiteral;
use psi_source::SourceSpan;
use psi_symbols::SymbolHandle;
use std::ops::Deref;
use std::sync::Arc;

mod display;
#[cfg(test)]
mod tests;

pub use display::display_name_path;

pub type ExpressionHandle = Handle<ExpressionNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    ArrayLiteral(Arc<[Expression]>),
    Atomic(Box<AtomicExpression>),
    Binary(Box<BinaryExpression>),
    Boolean(bool),
    Cast(Box<CastExpression>),
    Call(Box<CallExpression>),
    Float(FloatLiteral),
    Indexed(Box<IndexedExpression>),
    Integer(IntegerLiteral),
    Member(Box<MemberExpression>),
    Borrow(Box<BorrowExpression>),
    Name(NamePath),
    Range(Box<RangeExpression>),
    StructLiteral(StructLiteral),
    String(Arc<[u8]>),
    Unary(Box<UnaryExpression>),
    ZeroValue(crate::types::TypeReferenceHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowExpression {
    pub target: Expression,
    pub access: psi_language_core::ReferenceAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicExpression {
    pub value: Expression,
    pub result: Option<Expression>,
    pub ordering: psi_language_core::atomic::AtomicOrderingPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionTable {
    expressions: Arena<ExpressionNode>,
    source_spans: Vec<SourceSpan>,
    expression_handles: Arena<ExpressionHandle>,
    name_path_members: Arena<Identifier>,
    name_path_member_symbols: Arena<SymbolHandle>,
    struct_fields: Arena<TableStructLiteralField>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExpressionTableCapacity {
    pub expressions: usize,
    pub expression_handles: usize,
    pub name_path_members: usize,
    pub name_path_member_symbols: usize,
    pub struct_fields: usize,
}

impl ExpressionTableCapacity {
    pub fn saturating_add_assign(&mut self, other: Self) {
        self.expressions = self.expressions.saturating_add(other.expressions);
        self.expression_handles = self
            .expression_handles
            .saturating_add(other.expression_handles);
        self.name_path_members = self
            .name_path_members
            .saturating_add(other.name_path_members);
        self.name_path_member_symbols = self
            .name_path_member_symbols
            .saturating_add(other.name_path_member_symbols);
        self.struct_fields = self.struct_fields.saturating_add(other.struct_fields);
    }
}

impl ExpressionTable {
    pub fn new() -> Self {
        Self::with_expression_capacity(0)
    }

    pub fn with_expression_capacity(expression_capacity: usize) -> Self {
        Self::with_expression_and_handle_capacity(expression_capacity, 0)
    }

    pub fn with_expression_and_handle_capacity(
        expression_capacity: usize,
        expression_handle_capacity: usize,
    ) -> Self {
        Self::with_capacities(ExpressionTableCapacity {
            expressions: expression_capacity,
            expression_handles: expression_handle_capacity,
            ..ExpressionTableCapacity::default()
        })
    }

    pub fn with_capacities(capacity: ExpressionTableCapacity) -> Self {
        Self {
            expressions: Arena::with_capacity(capacity.expressions),
            source_spans: Vec::with_capacity(capacity.expressions),
            expression_handles: Arena::with_capacity(capacity.expression_handles),
            name_path_members: Arena::with_capacity(capacity.name_path_members),
            name_path_member_symbols: Arena::with_capacity(capacity.name_path_member_symbols),
            struct_fields: Arena::with_capacity(capacity.struct_fields),
        }
    }

    pub fn clear(&mut self) {
        self.expressions.reset_retain_capacity();
        self.source_spans.clear();
        self.expression_handles.reset_retain_capacity();
        self.name_path_members.reset_retain_capacity();
        self.name_path_member_symbols.reset_retain_capacity();
        self.struct_fields.reset_retain_capacity();
    }

    pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
        let handle = self.expressions.insert(expression);
        self.source_spans.push(SourceSpan::default());
        debug_assert_eq!(source_span_index(handle), self.source_spans.len() - 1);
        handle
    }

    pub fn source_span(&self, handle: ExpressionHandle) -> SourceSpan {
        self.source_spans[source_span_index(handle)]
    }

    pub fn set_source_span(&mut self, handle: ExpressionHandle, source_span: SourceSpan) {
        self.source_spans[source_span_index(handle)] = source_span;
    }

    pub fn insert_expression_handles(
        &mut self,
        expressions: impl IntoIterator<Item = ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.expression_handles.insert_many(expressions)
    }

    pub fn reserve_expression_handles(&mut self, count: u32) -> HandleSpan<ExpressionHandle> {
        self.expression_handles.insert_many(
            std::iter::repeat_with(ExpressionHandle::invalid)
                .take(usize::try_from(count).expect("expression handle span count overflow")),
        )
    }

    pub fn set_expression_handle_at_offset(
        &mut self,
        expressions: HandleSpan<ExpressionHandle>,
        offset: u32,
        expression: ExpressionHandle,
    ) {
        *self.expression_handles.get_mut(Handle::from_parts(
            expressions
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("expression handle index overflow"),
            expressions.start().generation(),
        )) = expression;
    }

    pub fn push_expression_handle(
        &mut self,
        span: &mut HandleSpan<ExpressionHandle>,
        expression: ExpressionHandle,
    ) {
        self.expression_handles.append_to_span(span, expression);
    }

    pub fn insert_struct_fields(
        &mut self,
        fields: impl IntoIterator<Item = TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        self.struct_fields.insert_many(fields)
    }

    pub fn reserve_struct_fields(&mut self, count: u32) -> HandleSpan<TableStructLiteralField> {
        self.struct_fields.insert_many(
            std::iter::repeat_with(TableStructLiteralField::default)
                .take(usize::try_from(count).expect("struct literal field span count overflow")),
        )
    }

    pub fn set_struct_field_at_offset(
        &mut self,
        fields: HandleSpan<TableStructLiteralField>,
        offset: u32,
        field: TableStructLiteralField,
    ) {
        *self.struct_fields.get_mut(Handle::from_parts(
            fields
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("struct literal field index overflow"),
            fields.start().generation(),
        )) = field;
    }

    pub fn push_struct_field(
        &mut self,
        span: &mut HandleSpan<TableStructLiteralField>,
        field: TableStructLiteralField,
    ) {
        self.struct_fields.append_to_span(span, field);
    }

    pub fn push_name_path_member(&mut self, span: &mut HandleSpan<Identifier>, member: Identifier) {
        self.name_path_members.append_to_span(span, member);
    }

    pub fn reserve_name_path_members(&mut self, count: u32) -> HandleSpan<Identifier> {
        self.name_path_members.insert_many(
            std::iter::repeat_with(Identifier::default)
                .take(usize::try_from(count).expect("name path member span count overflow")),
        )
    }

    pub fn set_name_path_member_at_offset(
        &mut self,
        members: HandleSpan<Identifier>,
        offset: u32,
        member: Identifier,
    ) {
        *self.name_path_members.get_mut(Handle::from_parts(
            members
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("name path member index overflow"),
            members.start().generation(),
        )) = member;
    }

    pub fn push_name_path_member_symbol(
        &mut self,
        span: &mut HandleSpan<SymbolHandle>,
        member_symbol: SymbolHandle,
    ) {
        self.name_path_member_symbols
            .append_to_span(span, member_symbol);
    }

    pub fn reserve_name_path_member_symbols(&mut self, count: u32) -> HandleSpan<SymbolHandle> {
        self.name_path_member_symbols.insert_many(
            std::iter::repeat_with(SymbolHandle::invalid)
                .take(usize::try_from(count).expect("name path member symbol span count overflow")),
        )
    }

    pub fn set_name_path_member_symbol_at_offset(
        &mut self,
        member_symbols: HandleSpan<SymbolHandle>,
        offset: u32,
        member_symbol: SymbolHandle,
    ) {
        *self.name_path_member_symbols.get_mut(Handle::from_parts(
            member_symbols
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("name path member symbol index overflow"),
            member_symbols.start().generation(),
        )) = member_symbol;
    }

    pub fn copy_from(
        &mut self,
        source: &ExpressionTable,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        self.copy_from_filtering_struct_literal_fields(source, expression, &|_, _| true)
    }

    /// Copy one expression graph while omitting rejected struct-literal fields
    /// before their values are visited. This is intentionally a copy-time
    /// filter: consumers which iterate the destination arena must never observe
    /// the omitted value subtrees, even as unreachable nodes.
    pub fn copy_from_filtering_struct_literal_fields(
        &mut self,
        source: &ExpressionTable,
        expression: ExpressionHandle,
        retain: &impl Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    ) -> ExpressionHandle {
        let source_span = source.source_span(expression);
        let copied = match source.expression(expression) {
            ExpressionNode::ArrayLiteral(source_values) => {
                let values = self
                    .copy_expression_handles_from_slice_filtering_struct_literal_fields(
                        source,
                        source.expression_handles(*source_values),
                        retain,
                    );
                self.insert(ExpressionNode::ArrayLiteral(values))
            }
            ExpressionNode::Atomic(atomic) => {
                let value =
                    self.copy_from_filtering_struct_literal_fields(source, atomic.value, retain);
                let result = atomic
                    .result
                    .is_valid()
                    .then(|| {
                        self.copy_from_filtering_struct_literal_fields(
                            source,
                            atomic.result,
                            retain,
                        )
                    })
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                }))
            }
            ExpressionNode::Binary(binary) => {
                let left =
                    self.copy_from_filtering_struct_literal_fields(source, binary.left, retain);
                let right =
                    self.copy_from_filtering_struct_literal_fields(source, binary.right, retain);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(*value)),
            ExpressionNode::Cast(cast) => {
                let value =
                    self.copy_from_filtering_struct_literal_fields(source, cast.value, retain);
                let target_type = cast.target_type;
                let target_label = self.copy_name_path_members(source, cast.target_label);
                let semantic_domain = self.copy_name_path_members(source, cast.semantic_domain);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
                    target_label,
                    domain: cast.domain,
                    semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    semantic_domain_id: cast.semantic_domain_id,
                    form: cast.form,
                }))
            }
            ExpressionNode::Call(call) => {
                let receiver = call
                    .receiver
                    .is_valid()
                    .then(|| {
                        self.copy_from_filtering_struct_literal_fields(
                            source,
                            call.receiver,
                            retain,
                        )
                    })
                    .unwrap_or_else(ExpressionHandle::invalid);
                let arguments = self
                    .copy_expression_handles_from_slice_filtering_struct_literal_fields(
                        source,
                        source.expression_handles(call.arguments),
                        retain,
                    );
                self.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target.clone(),
                    machine_arguments: call.machine_arguments.clone(),
                    quotient_operation: call.quotient_operation.clone(),
                    arguments,
                    evidence_arguments: call.evidence_arguments.clone(),
                    operational_acknowledgement: call.operational_acknowledgement,
                }))
            }
            ExpressionNode::Float(value) => self.insert(ExpressionNode::Float(value.clone())),
            ExpressionNode::Indexed(indexed) => {
                let collection = self.copy_from_filtering_struct_literal_fields(
                    source,
                    indexed.collection,
                    retain,
                );
                let index =
                    self.copy_from_filtering_struct_literal_fields(source, indexed.index, retain);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value.clone())),
            ExpressionNode::Member(member) => {
                let receiver =
                    self.copy_from_filtering_struct_literal_fields(source, member.receiver, retain);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                    case_variant: member.case_variant.clone(),
                }))
            }
            ExpressionNode::Borrow(inner_expression) => {
                let target = self.copy_from_filtering_struct_literal_fields(
                    source,
                    inner_expression.target,
                    retain,
                );
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access: inner_expression.access,
                }))
            }
            ExpressionNode::Name(path) => {
                let members = self.copy_name_path_members(source, path.members);
                let member_symbols =
                    self.copy_name_path_member_symbols(source, path.member_symbols);
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: path.head_symbol,
                    symbol: path.symbol,
                }))
            }
            ExpressionNode::Range(range) => {
                let start = range
                    .start
                    .is_valid()
                    .then(|| {
                        self.copy_from_filtering_struct_literal_fields(source, range.start, retain)
                    })
                    .unwrap_or_else(ExpressionHandle::invalid);
                let end = range
                    .end
                    .is_valid()
                    .then(|| {
                        self.copy_from_filtering_struct_literal_fields(source, range.end, retain)
                    })
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                let fields =
                    self.copy_struct_literal_fields_filtering(source, struct_literal, retain);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    type_symbol: struct_literal.type_symbol,
                    case_name: struct_literal.case_name.clone(),
                    case_symbol: struct_literal.case_symbol,
                    fields,
                }))
            }
            ExpressionNode::String(value) => self.insert(ExpressionNode::String(value.clone())),
            ExpressionNode::Unary(unary) => {
                let operand =
                    self.copy_from_filtering_struct_literal_fields(source, unary.operand, retain);
                self.insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                }))
            }
            ExpressionNode::ZeroValue(type_reference) => {
                self.insert(ExpressionNode::ZeroValue(*type_reference))
            }
        };
        self.set_source_span(copied, source_span);
        copied
    }

    /// Remap lexical identities throughout one expression graph. The graph's
    /// arena handles and authored names stay stable; only normalized symbols
    /// change. This is the second half of cloning a lexical scope after
    /// [`Self::copy_from`] has made its payload handles independent.
    pub fn remap_symbols_in(
        &mut self,
        root: ExpressionHandle,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        let mut visited = Vec::new();
        self.remap_symbols_in_inner(root, symbols, &mut visited);
    }

    fn remap_symbols_in_inner(
        &mut self,
        root: ExpressionHandle,
        symbols: &[(SymbolHandle, SymbolHandle)],
        visited: &mut Vec<ExpressionHandle>,
    ) {
        if !root.is_valid() || visited.contains(&root) {
            return;
        }
        visited.push(root);

        let node = self.expression(root).clone();
        match node {
            ExpressionNode::ArrayLiteral(values) => {
                let children = self.expression_handles(values).to_vec();
                for child in children {
                    self.remap_symbols_in_inner(child, symbols, visited);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                self.remap_symbols_in_inner(atomic.value, symbols, visited)
            }
            ExpressionNode::Binary(binary) => {
                self.remap_symbols_in_inner(binary.left, symbols, visited);
                self.remap_symbols_in_inner(binary.right, symbols, visited);
            }
            ExpressionNode::Cast(cast) => self.remap_symbols_in_inner(cast.value, symbols, visited),
            ExpressionNode::Call(call) => {
                self.remap_symbols_in_inner(call.receiver, symbols, visited);
                let arguments = self.expression_handles(call.arguments).to_vec();
                for argument in arguments {
                    self.remap_symbols_in_inner(argument, symbols, visited);
                }
                let ExpressionNode::Call(call) = self.expression_mut(root) else {
                    unreachable!();
                };
                call.target_symbol = remapped(call.target_symbol, symbols);
                for argument in &mut call.machine_arguments {
                    argument.symbol = remapped(argument.symbol, symbols);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                self.remap_symbols_in_inner(indexed.collection, symbols, visited);
                self.remap_symbols_in_inner(indexed.index, symbols, visited);
            }
            ExpressionNode::Member(member) => {
                self.remap_symbols_in_inner(member.receiver, symbols, visited);
                let ExpressionNode::Member(member) = self.expression_mut(root) else {
                    unreachable!();
                };
                member.member_symbol = remapped(member.member_symbol, symbols);
            }
            ExpressionNode::Borrow(inner) => {
                self.remap_symbols_in_inner(inner.target, symbols, visited)
            }
            ExpressionNode::Name(path) => {
                let ExpressionNode::Name(current) = self.expression_mut(root) else {
                    unreachable!();
                };
                current.head_symbol = remapped(path.head_symbol, symbols);
                current.symbol = remapped(path.symbol, symbols);
                for member_symbol in self
                    .name_path_member_symbols
                    .span_mut_or_empty(path.member_symbols)
                {
                    *member_symbol = remapped(*member_symbol, symbols);
                }
            }
            ExpressionNode::Range(range) => {
                self.remap_symbols_in_inner(range.start, symbols, visited);
                self.remap_symbols_in_inner(range.end, symbols, visited);
            }
            ExpressionNode::StructLiteral(literal) => {
                let children: Vec<_> = self
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value)
                    .collect();
                for child in children {
                    self.remap_symbols_in_inner(child, symbols, visited);
                }
            }
            ExpressionNode::Unary(unary) => {
                self.remap_symbols_in_inner(unary.operand, symbols, visited)
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }

    pub fn copy_expression_handles_from(
        &mut self,
        source: &ExpressionTable,
        expressions: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.copy_expression_handles_from_slice(source, source.expression_handles(expressions))
    }

    pub fn copy_expression_handles_from_slice(
        &mut self,
        source: &ExpressionTable,
        expressions: &[ExpressionHandle],
    ) -> HandleSpan<ExpressionHandle> {
        let copied = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression = self.copy_from(source, *expression);
            self.set_expression_handle_at_offset(
                copied,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
        }

        copied
    }

    fn copy_expression_handles_from_slice_filtering_struct_literal_fields(
        &mut self,
        source: &ExpressionTable,
        expressions: &[ExpressionHandle],
        retain: &impl Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    ) -> HandleSpan<ExpressionHandle> {
        let copied = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression =
                self.copy_from_filtering_struct_literal_fields(source, *expression, retain);
            self.set_expression_handle_at_offset(
                copied,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
        }

        copied
    }

    fn insert_name_path_members(&mut self, path: &NamePath) -> HandleSpan<Identifier> {
        let mut members = HandleSpan::empty();

        for member in path.members() {
            self.name_path_members
                .append_to_span(&mut members, member.clone());
        }

        members
    }

    fn insert_name_path_member_symbols(&mut self, path: &NamePath) -> HandleSpan<SymbolHandle> {
        let mut member_symbols = HandleSpan::empty();

        for member_symbol in path.member_symbols() {
            self.name_path_member_symbols
                .append_to_span(&mut member_symbols, *member_symbol);
        }

        member_symbols
    }

    fn copy_name_path_members(
        &mut self,
        source: &ExpressionTable,
        members: HandleSpan<Identifier>,
    ) -> HandleSpan<Identifier> {
        let mut copied = HandleSpan::empty();

        for member in source.name_path_members(members) {
            self.name_path_members
                .append_to_span(&mut copied, member.clone());
        }

        copied
    }

    fn copy_name_path_member_symbols(
        &mut self,
        source: &ExpressionTable,
        member_symbols: HandleSpan<SymbolHandle>,
    ) -> HandleSpan<SymbolHandle> {
        let mut copied = HandleSpan::empty();

        for member_symbol in source.name_path_member_symbols(member_symbols) {
            self.name_path_member_symbols
                .append_to_span(&mut copied, *member_symbol);
        }

        copied
    }

    fn copy_own_name_path_members(
        &mut self,
        members: HandleSpan<Identifier>,
    ) -> HandleSpan<Identifier> {
        let mut copied = HandleSpan::empty();

        for offset in 0..members.count() {
            let member = self.name_path_member_at_offset(members, offset).clone();
            self.name_path_members.append_to_span(&mut copied, member);
        }

        copied
    }

    fn copy_own_name_path_member_symbols(
        &mut self,
        member_symbols: HandleSpan<SymbolHandle>,
    ) -> HandleSpan<SymbolHandle> {
        let mut copied = HandleSpan::empty();

        for offset in 0..member_symbols.count() {
            let member_symbol = *self.name_path_member_symbol_at_offset(member_symbols, offset);
            self.name_path_member_symbols
                .append_to_span(&mut copied, member_symbol);
        }

        copied
    }

    fn copy_own_name_path_members_with_index_suffix(
        &mut self,
        members: HandleSpan<Identifier>,
        index: i64,
    ) -> HandleSpan<Identifier> {
        if members.is_empty() {
            return HandleSpan::empty();
        }

        let mut copied = HandleSpan::empty();
        let last_offset = members.count() - 1;

        for offset in 0..members.count() {
            let member = self.name_path_member_at_offset(members, offset);
            let member = if offset == last_offset {
                Identifier::generated(format!("{member}[{index}]"))
            } else {
                member.clone()
            };

            self.name_path_members.append_to_span(&mut copied, member);
        }

        copied
    }

    fn copy_own_name_path_member_symbols_with_index_suffix(
        &mut self,
        member_symbols: HandleSpan<SymbolHandle>,
    ) -> HandleSpan<SymbolHandle> {
        if member_symbols.is_empty() {
            return HandleSpan::empty();
        }

        self.copy_own_name_path_member_symbols(member_symbols)
    }

    fn name_path_member_at_offset(
        &self,
        members: HandleSpan<Identifier>,
        offset: u32,
    ) -> &Identifier {
        self.name_path_members.get(Handle::from_parts(
            members
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("name path member index overflow"),
            members.start().generation(),
        ))
    }

    fn name_path_member_symbol_at_offset(
        &self,
        member_symbols: HandleSpan<SymbolHandle>,
        offset: u32,
    ) -> &SymbolHandle {
        self.name_path_member_symbols.get(Handle::from_parts(
            member_symbols
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("name path member symbol index overflow"),
            member_symbols.start().generation(),
        ))
    }

    fn copy_struct_literal_fields_filtering(
        &mut self,
        source: &ExpressionTable,
        literal: &TableStructLiteral,
        retain: &impl Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    ) -> HandleSpan<TableStructLiteralField> {
        let retained = source
            .struct_fields(literal.fields)
            .iter()
            .filter(|field| retain(literal, field))
            .collect::<Vec<_>>();
        let copied = self.reserve_struct_fields(
            retained
                .len()
                .try_into()
                .expect("struct literal field span count overflow"),
        );

        for (offset, field) in retained.into_iter().enumerate() {
            let value = self.copy_from_filtering_struct_literal_fields(source, field.value, retain);
            self.set_struct_field_at_offset(
                copied,
                offset
                    .try_into()
                    .expect("struct literal field span count overflow"),
                TableStructLiteralField {
                    name: field.name.clone(),
                    field_symbol: field.field_symbol,
                    value,
                },
            );
        }

        copied
    }

    fn copy_own_expression_handles(
        &mut self,
        expressions: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        let copied = self.reserve_expression_handles(expressions.count());

        for offset in 0..expressions.count() {
            let expression = self.expression_handle_at_offset(expressions, offset);
            let expression = self.insert_copy(expression);
            self.set_expression_handle_at_offset(copied, offset, expression);
        }

        copied
    }

    fn copy_own_struct_literal_fields(
        &mut self,
        fields: HandleSpan<TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        let copied = self.reserve_struct_fields(fields.count());

        for offset in 0..fields.count() {
            let field = self.struct_field_at_offset(fields, offset).clone();
            let value = self.insert_copy(field.value);
            self.set_struct_field_at_offset(
                copied,
                offset,
                TableStructLiteralField {
                    name: field.name,
                    field_symbol: field.field_symbol,
                    value,
                },
            );
        }

        copied
    }

    pub fn expression_handle_at_offset(
        &self,
        expressions: HandleSpan<ExpressionHandle>,
        offset: u32,
    ) -> ExpressionHandle {
        *self.expression_handles.get(Handle::from_parts(
            expressions
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("expression handle index overflow"),
            expressions.start().generation(),
        ))
    }

    pub fn struct_field_at_offset(
        &self,
        fields: HandleSpan<TableStructLiteralField>,
        offset: u32,
    ) -> &TableStructLiteralField {
        self.struct_fields.get(Handle::from_parts(
            fields
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("struct literal field index overflow"),
            fields.start().generation(),
        ))
    }

    fn insert_expression_handle_span_from_trees(
        &mut self,
        expressions: &[Expression],
    ) -> HandleSpan<ExpressionHandle> {
        let handles = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression = self.insert_tree(expression);
            self.set_expression_handle_at_offset(
                handles,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
        }

        handles
    }

    fn insert_struct_field_span_from_tree(
        &mut self,
        fields: &[StructLiteralField],
    ) -> HandleSpan<TableStructLiteralField> {
        let field_span = self.reserve_struct_fields(
            fields
                .len()
                .try_into()
                .expect("struct literal field span count overflow"),
        );

        for (offset, field) in fields.iter().enumerate() {
            let value = self.insert_tree(&field.value);
            self.set_struct_field_at_offset(
                field_span,
                offset
                    .try_into()
                    .expect("struct literal field span count overflow"),
                TableStructLiteralField {
                    name: field.name.clone(),
                    field_symbol: SymbolHandle::invalid(),
                    value,
                },
            );
        }

        field_span
    }

    /// Mutable node access for tree-normalization passes (the F2b float
    /// destination stamp rewrites an unlanded literal in place; handles and
    /// spans are untouched).
    pub fn expression_mut(&mut self, handle: ExpressionHandle) -> &mut ExpressionNode {
        self.expressions.get_mut(handle)
    }

    pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
        self.expressions.get(handle)
    }

    pub fn iter_expressions(&self) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
        self.expressions.iter()
    }

    pub fn expression_is_literal(&self, handle: ExpressionHandle) -> bool {
        matches!(
            self.expression(handle),
            ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::String(_)
        )
    }

    pub fn expression_is_direct_place_path(&self, handle: ExpressionHandle) -> bool {
        match self.expression(handle) {
            ExpressionNode::Name(_) => true,
            ExpressionNode::Member(member) => self.expression_is_direct_place_path(member.receiver),
            ExpressionNode::Borrow(inner) => self.expression_is_direct_place_path(inner.target),
            _ => false,
        }
    }

    pub fn expression_is_stored_place(&self, handle: ExpressionHandle) -> bool {
        matches!(
            self.expression(handle),
            ExpressionNode::Name(_) | ExpressionNode::Indexed(_) | ExpressionNode::Member(_)
        )
    }

    pub fn expression_handles(&self, span: HandleSpan<ExpressionHandle>) -> &[ExpressionHandle] {
        self.expression_handles.span_or_empty(span)
    }

    pub fn struct_fields(
        &self,
        span: HandleSpan<TableStructLiteralField>,
    ) -> &[TableStructLiteralField] {
        self.struct_fields.span_or_empty(span)
    }

    pub fn name_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
        self.name_path_members.span_or_empty(span)
    }

    pub fn name_path_member_symbols(&self, span: HandleSpan<SymbolHandle>) -> &[SymbolHandle] {
        self.name_path_member_symbols.span_or_empty(span)
    }

    /// Whether two expressions in this table are structurally identical: same node
    /// shapes, literals, names/symbols, and operators, recursively. Lowering copies
    /// expression trees per use site, so a SHARED source expression (e.g. the subject
    /// of `transition self.f(x) { true -> a false -> b }`, which the parser inserts
    /// once and references from every arm's guard) arrives as several distinct
    /// handles; structural equality recognizes those copies so consumers can evaluate
    /// the source expression ONCE. Conservative: node kinds not listed compare as not
    /// equal.
    pub fn expressions_structurally_equal(&self, a: ExpressionHandle, b: ExpressionHandle) -> bool {
        if a == b {
            return true;
        }
        if !a.is_valid() || !b.is_valid() {
            return false;
        }
        match (self.expression(a), self.expression(b)) {
            (ExpressionNode::Integer(x), ExpressionNode::Integer(y)) => x == y,
            (ExpressionNode::Boolean(x), ExpressionNode::Boolean(y)) => x == y,
            (ExpressionNode::String(x), ExpressionNode::String(y)) => x == y,
            (ExpressionNode::Float(x), ExpressionNode::Float(y)) => x == y,
            (ExpressionNode::Name(x), ExpressionNode::Name(y)) => {
                x.head_symbol == y.head_symbol
                    && x.symbol == y.symbol
                    && identifier_texts_equal(
                        self.name_path_members(x.members),
                        self.name_path_members(y.members),
                    )
            }
            (ExpressionNode::Member(x), ExpressionNode::Member(y)) => {
                x.member_symbol == y.member_symbol
                    && x.member.as_str() == y.member.as_str()
                    && self.expressions_structurally_equal(x.receiver, y.receiver)
            }
            (ExpressionNode::Borrow(x), ExpressionNode::Borrow(y)) => {
                x.access == y.access && self.expressions_structurally_equal(x.target, y.target)
            }
            (ExpressionNode::Unary(x), ExpressionNode::Unary(y)) => {
                x.operator == y.operator
                    && self.expressions_structurally_equal(x.operand, y.operand)
            }
            (ExpressionNode::Binary(x), ExpressionNode::Binary(y)) => {
                x.operator == y.operator
                    && self.expressions_structurally_equal(x.left, y.left)
                    && self.expressions_structurally_equal(x.right, y.right)
            }
            (ExpressionNode::Indexed(x), ExpressionNode::Indexed(y)) => {
                self.expressions_structurally_equal(x.collection, y.collection)
                    && self.expressions_structurally_equal(x.index, y.index)
            }
            (ExpressionNode::Call(x), ExpressionNode::Call(y)) => {
                x.target_symbol == y.target_symbol
                    && x.target.as_str() == y.target.as_str()
                    && self.expressions_structurally_equal(x.receiver, y.receiver)
                    && self.expression_spans_structurally_equal(x.arguments, y.arguments)
            }
            _ => false,
        }
    }

    fn expression_spans_structurally_equal(
        &self,
        a: HandleSpan<ExpressionHandle>,
        b: HandleSpan<ExpressionHandle>,
    ) -> bool {
        let a = self.expression_handles(a);
        let b = self.expression_handles(b);
        a.len() == b.len()
            && a.iter()
                .zip(b.iter())
                .all(|(x, y)| self.expressions_structurally_equal(*x, *y))
    }

    pub fn copy_name_path_members_with_suffix(
        &mut self,
        members: HandleSpan<Identifier>,
        suffix: Identifier,
    ) -> HandleSpan<Identifier> {
        let copied = self.reserve_name_path_members(
            members
                .count()
                .checked_add(1)
                .expect("name path member span count overflow"),
        );

        for offset in 0..members.count() {
            let member = self.name_path_member_at_offset(members, offset).clone();
            self.set_name_path_member_at_offset(copied, offset, member);
        }

        self.set_name_path_member_at_offset(copied, members.count(), suffix);

        copied
    }

    pub fn copy_name_path_members_with_member_suffix(
        &mut self,
        members: HandleSpan<Identifier>,
        suffix_members: HandleSpan<Identifier>,
        suffix_start_offset: u32,
    ) -> HandleSpan<Identifier> {
        let suffix_count = suffix_members.count().saturating_sub(suffix_start_offset);
        let copied = self.reserve_name_path_members(
            members
                .count()
                .checked_add(suffix_count)
                .expect("name path member span count overflow"),
        );

        for offset in 0..members.count() {
            let member = self.name_path_member_at_offset(members, offset).clone();
            self.set_name_path_member_at_offset(copied, offset, member);
        }

        for (target_offset, offset) in (suffix_start_offset..suffix_members.count()).enumerate() {
            let member = self
                .name_path_member_at_offset(suffix_members, offset)
                .clone();
            self.set_name_path_member_at_offset(
                copied,
                members
                    .count()
                    .checked_add(
                        target_offset
                            .try_into()
                            .expect("name path member span count overflow"),
                    )
                    .expect("name path member span count overflow"),
                member,
            );
        }

        copied
    }

    pub fn copy_name_path_member_symbols_with_member_suffix(
        &mut self,
        member_symbols: HandleSpan<SymbolHandle>,
        suffix_member_symbols: HandleSpan<SymbolHandle>,
        suffix_start_offset: u32,
    ) -> HandleSpan<SymbolHandle> {
        let suffix_count = suffix_member_symbols
            .count()
            .saturating_sub(suffix_start_offset);
        let copied = self.reserve_name_path_member_symbols(
            member_symbols
                .count()
                .checked_add(suffix_count)
                .expect("name path member symbol span count overflow"),
        );

        for offset in 0..member_symbols.count() {
            let member_symbol = *self.name_path_member_symbol_at_offset(member_symbols, offset);
            self.set_name_path_member_symbol_at_offset(copied, offset, member_symbol);
        }

        for (target_offset, offset) in
            (suffix_start_offset..suffix_member_symbols.count()).enumerate()
        {
            let member_symbol =
                *self.name_path_member_symbol_at_offset(suffix_member_symbols, offset);
            self.set_name_path_member_symbol_at_offset(
                copied,
                member_symbols
                    .count()
                    .checked_add(
                        target_offset
                            .try_into()
                            .expect("name path member symbol span count overflow"),
                    )
                    .expect("name path member symbol span count overflow"),
                member_symbol,
            );
        }

        copied
    }

    pub fn insert_copy_with_member_suffix(
        &mut self,
        expression: ExpressionHandle,
        suffix_members: HandleSpan<Identifier>,
        suffix_member_symbols: HandleSpan<SymbolHandle>,
        suffix_start_offset: u32,
    ) -> ExpressionHandle {
        if suffix_start_offset >= suffix_members.count() {
            return expression;
        }

        match self.expression(expression).clone() {
            ExpressionNode::Name(path) => {
                let members = self.copy_name_path_members_with_member_suffix(
                    path.members,
                    suffix_members,
                    suffix_start_offset,
                );
                let member_symbols = self.copy_name_path_member_symbols_with_member_suffix(
                    path.member_symbols,
                    suffix_member_symbols,
                    suffix_start_offset,
                );
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: path.head_symbol,
                    symbol: SymbolHandle::invalid(),
                }))
            }
            ExpressionNode::Borrow(target) => {
                let access = target.access;
                let target = self.insert_copy_with_member_suffix(
                    target.target,
                    suffix_members,
                    suffix_member_symbols,
                    suffix_start_offset,
                );
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access,
                }))
            }
            ExpressionNode::Indexed(indexed) => {
                if let Some(path) = self.copy_indexed_expression_path(indexed) {
                    let members = self.copy_name_path_members_with_member_suffix(
                        path.members,
                        suffix_members,
                        suffix_start_offset,
                    );
                    let member_symbols = self.copy_name_path_member_symbols_with_member_suffix(
                        path.member_symbols,
                        suffix_member_symbols,
                        suffix_start_offset,
                    );
                    self.insert(ExpressionNode::Name(TableNamePath {
                        members,
                        member_symbols,
                        head_symbol: path.head_symbol,
                        symbol: SymbolHandle::invalid(),
                    }))
                } else {
                    let copied = self.insert_copy(expression);
                    self.insert_member_suffix_chain(
                        copied,
                        suffix_members,
                        suffix_member_symbols,
                        suffix_start_offset,
                    )
                }
            }
            // Member receivers (and anything else) get the member-suffix
            // chain. The old catch-all copied the receiver WITHOUT the suffix
            // -- silently dropping it -- so an alias substitution through here
            // resolved to the RECEIVER's place instead of the member's (the
            // append_place_suffix clobber class; the suffix must never be
            // dropped).
            _ => {
                let copied = self.insert_copy(expression);
                self.insert_member_suffix_chain(
                    copied,
                    suffix_members,
                    suffix_member_symbols,
                    suffix_start_offset,
                )
            }
        }
    }

    fn insert_member_suffix_chain(
        &mut self,
        expression: ExpressionHandle,
        suffix_members: HandleSpan<Identifier>,
        suffix_member_symbols: HandleSpan<SymbolHandle>,
        suffix_start_offset: u32,
    ) -> ExpressionHandle {
        let mut expression = expression;
        for offset in suffix_start_offset..suffix_members.count() {
            let member = self
                .name_path_member_at_offset(suffix_members, offset)
                .clone();
            let member_symbol = if offset < suffix_member_symbols.count() {
                *self.name_path_member_symbol_at_offset(suffix_member_symbols, offset)
            } else {
                SymbolHandle::invalid()
            };
            expression = self.insert(ExpressionNode::Member(TableMemberExpression {
                receiver: expression,
                member_symbol,
                member,
                case_variant: None,
            }));
        }
        expression
    }

    pub fn insert_copy(&mut self, expression: ExpressionHandle) -> ExpressionHandle {
        match self.expression(expression).clone() {
            ExpressionNode::ArrayLiteral(values) => {
                let values = self.copy_own_expression_handles(values);
                self.insert(ExpressionNode::ArrayLiteral(values))
            }
            ExpressionNode::Atomic(atomic) => {
                let value = self.insert_copy(atomic.value);
                let result = atomic
                    .result
                    .is_valid()
                    .then(|| self.insert_copy(atomic.result))
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                }))
            }
            ExpressionNode::Binary(binary) => {
                let left = self.insert_copy(binary.left);
                let right = self.insert_copy(binary.right);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(value)),
            ExpressionNode::Cast(cast) => {
                let value = self.insert_copy(cast.value);
                let target_type = cast.target_type;
                let target_label = self.copy_own_name_path_members(cast.target_label);
                let semantic_domain = self.copy_own_name_path_members(cast.semantic_domain);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
                    target_label,
                    domain: cast.domain,
                    semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    semantic_domain_id: cast.semantic_domain_id,
                    form: cast.form,
                }))
            }
            ExpressionNode::Call(call) => {
                let receiver = call
                    .receiver
                    .is_valid()
                    .then(|| self.insert_copy(call.receiver))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let arguments = self.copy_own_expression_handles(call.arguments);
                self.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target,
                    machine_arguments: call.machine_arguments,
                    quotient_operation: call.quotient_operation,
                    arguments,
                    evidence_arguments: call.evidence_arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                }))
            }
            ExpressionNode::Float(value) => self.insert(ExpressionNode::Float(value)),
            ExpressionNode::Indexed(indexed) => {
                let collection = self.insert_copy(indexed.collection);
                let index = self.insert_copy(indexed.index);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value)),
            ExpressionNode::Member(member) => {
                let receiver = self.insert_copy(member.receiver);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                    case_variant: member.case_variant,
                }))
            }
            ExpressionNode::Borrow(inner_expression) => {
                let target = self.insert_copy(inner_expression.target);
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access: inner_expression.access,
                }))
            }
            ExpressionNode::Name(path) => {
                let members = self.copy_own_name_path_members(path.members);
                let member_symbols = self.copy_own_name_path_member_symbols(path.member_symbols);
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: path.head_symbol,
                    symbol: path.symbol,
                }))
            }
            ExpressionNode::Range(range) => {
                let start = range
                    .start
                    .is_valid()
                    .then(|| self.insert_copy(range.start))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let end = range
                    .end
                    .is_valid()
                    .then(|| self.insert_copy(range.end))
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                let fields = self.copy_own_struct_literal_fields(struct_literal.fields);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name,
                    type_symbol: struct_literal.type_symbol,
                    case_name: struct_literal.case_name,
                    case_symbol: struct_literal.case_symbol,
                    fields,
                }))
            }
            ExpressionNode::String(value) => self.insert(ExpressionNode::String(value)),
            ExpressionNode::Unary(unary) => {
                let operand = self.insert_copy(unary.operand);
                self.insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                }))
            }
            ExpressionNode::ZeroValue(type_reference) => {
                self.insert(ExpressionNode::ZeroValue(type_reference))
            }
        }
    }

    fn copy_indexed_expression_path(
        &mut self,
        indexed: TableIndexedExpression,
    ) -> Option<TableNamePath> {
        let ExpressionNode::Integer(index) = self.expression(indexed.index) else {
            return None;
        };
        // An index beyond i64 cannot name a real element; treat it like any
        // non-constant index (no synthetic const-index path).
        let index = index.value_i64()?;

        let base = match self.expression(indexed.collection).clone() {
            ExpressionNode::Name(path) => path,
            ExpressionNode::Indexed(inner_indexed) => {
                self.copy_indexed_expression_path(inner_indexed)?
            }
            _ => return None,
        };
        let members = self.copy_own_name_path_members_with_index_suffix(base.members, index);
        let member_symbols =
            self.copy_own_name_path_member_symbols_with_index_suffix(base.member_symbols);
        if members.is_empty() {
            return None;
        }

        Some(TableNamePath {
            members,
            member_symbols,
            head_symbol: base.head_symbol,
            symbol: SymbolHandle::invalid(),
        })
    }

    pub fn expression_count(&self) -> usize {
        self.expressions.len()
    }

    pub fn expression_nodes(&self) -> impl Iterator<Item = &ExpressionNode> {
        self.expressions.iter().map(|(_, node)| node)
    }

    pub fn expression_entries(&self) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
        self.expressions.iter()
    }

    pub fn copy_capacity(&self) -> ExpressionTableCapacity {
        ExpressionTableCapacity {
            expressions: self.expression_count(),
            expression_handles: self.expression_handle_count(),
            name_path_members: self.name_path_member_count(),
            name_path_member_symbols: self.name_path_member_symbol_count(),
            struct_fields: self.struct_field_count(),
        }
    }

    pub fn expression_handle_count(&self) -> usize {
        self.expression_handles.len()
    }

    pub fn name_path_member_count(&self) -> usize {
        self.name_path_members.len()
    }

    pub fn name_path_member_symbol_count(&self) -> usize {
        self.name_path_member_symbols.len()
    }

    pub fn struct_field_count(&self) -> usize {
        self.struct_fields.len()
    }

    pub fn insert_tree(&mut self, expression: &Expression) -> ExpressionHandle {
        match expression {
            Expression::ArrayLiteral(values) => {
                let values = self.insert_expression_handle_span_from_trees(values);
                self.insert(ExpressionNode::ArrayLiteral(values))
            }
            Expression::Atomic(atomic) => {
                let value = self.insert_tree(&atomic.value);
                let result = atomic
                    .result
                    .as_ref()
                    .map(|result| self.insert_tree(result))
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                }))
            }
            Expression::Binary(binary) => {
                let left = self.insert_tree(&binary.left);
                let right = self.insert_tree(&binary.right);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            Expression::Boolean(value) => self.insert(ExpressionNode::Boolean(*value)),
            Expression::Cast(cast) => {
                let value = self.insert_tree(&cast.value);
                let target_label = self.insert_name_path_members(&cast.target_label);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label,
                    domain: cast.domain,
                    // Tree-built casts are compiler-internal (tests/builders)
                    // and never carry the qualification suffix.
                    semantic_domain: HandleSpan::empty(),
                    semantic_domain_arguments: HandleSpan::empty(),
                    semantic_domain_symbol: SymbolHandle::invalid(),
                    semantic_domain_id: psi_language_semantics::SemanticDomainId::NULL,
                    form: cast.form,
                }))
            }
            Expression::Call(call) => {
                let receiver = call
                    .receiver
                    .as_ref()
                    .map(|receiver| self.insert_tree(receiver))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let arguments = self.insert_expression_handle_span_from_trees(&call.arguments);
                self.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target.clone(),
                    machine_arguments: Box::default(),
                    quotient_operation: None,
                    arguments,
                    evidence_arguments: call.evidence_arguments.to_vec().into_boxed_slice(),
                    operational_acknowledgement: call.operational_acknowledgement,
                }))
            }
            Expression::Float(value) => self.insert(ExpressionNode::Float(value.clone())),
            Expression::Indexed(indexed) => {
                let collection = self.insert_tree(&indexed.collection);
                let index = self.insert_tree(&indexed.index);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            Expression::Integer(value) => self.insert(ExpressionNode::Integer(value.clone())),
            Expression::Member(member) => {
                let receiver = self.insert_tree(&member.receiver);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                    case_variant: member.case_variant.clone(),
                }))
            }
            Expression::Borrow(inner_expression) => {
                let target = self.insert_tree(&inner_expression.target);
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access: inner_expression.access,
                }))
            }
            Expression::Name(path) => {
                let members = self.insert_name_path_members(path);
                let member_symbols = self.insert_name_path_member_symbols(path);
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: path.head_symbol(),
                    symbol: path.symbol(),
                }))
            }
            Expression::Range(range) => {
                let start = range
                    .start
                    .as_ref()
                    .map(|start| self.insert_tree(start))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let end = range
                    .end
                    .as_ref()
                    .map(|end| self.insert_tree(end))
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            Expression::StructLiteral(struct_literal) => {
                let fields = self.insert_struct_field_span_from_tree(&struct_literal.fields);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    type_symbol: SymbolHandle::invalid(),
                    case_name: struct_literal.case_name.clone(),
                    case_symbol: None,
                    fields,
                }))
            }
            Expression::String(value) => self.insert(ExpressionNode::String(value.clone())),
            Expression::Unary(unary) => {
                let operand = self.insert_tree(&unary.operand);
                self.insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                }))
            }
            Expression::ZeroValue(type_reference) => {
                self.insert(ExpressionNode::ZeroValue(*type_reference))
            }
        }
    }

    pub fn to_tree(&self, expression: ExpressionHandle) -> Expression {
        match self.expression(expression) {
            ExpressionNode::ArrayLiteral(values) => Expression::ArrayLiteral(
                self.expression_handles(*values)
                    .iter()
                    .map(|value| self.to_tree(*value))
                    .collect::<Arc<[_]>>(),
            ),
            ExpressionNode::Atomic(atomic) => Expression::Atomic(Box::new(AtomicExpression {
                value: self.to_tree(atomic.value),
                result: atomic
                    .result
                    .is_valid()
                    .then(|| self.to_tree(atomic.result)),
                ordering: atomic.ordering,
            })),
            ExpressionNode::Binary(binary) => Expression::Binary(Box::new(BinaryExpression {
                left: self.to_tree(binary.left),
                operator: binary.operator,
                right: self.to_tree(binary.right),
            })),
            ExpressionNode::Boolean(value) => Expression::Boolean(*value),
            ExpressionNode::Cast(cast) => Expression::Cast(Box::new(CastExpression {
                value: self.to_tree(cast.value),
                target_type: cast.target_type,
                target_label: NamePath::unresolved_from_iter(
                    self.name_path_members(cast.target_label).iter().cloned(),
                ),
                domain: cast.domain,
                form: cast.form,
            })),
            ExpressionNode::Call(call) => Expression::Call(Box::new(CallExpression {
                receiver: call
                    .receiver
                    .is_valid()
                    .then(|| Box::new(self.to_tree(call.receiver))),
                target_symbol: call.target_symbol,
                target: call.target.clone(),
                arguments: self
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| self.to_tree(*argument))
                    .collect::<Arc<[_]>>(),
                evidence_arguments: Arc::from(call.evidence_arguments.clone()),
                operational_acknowledgement: call.operational_acknowledgement,
            })),
            ExpressionNode::Float(value) => Expression::Float(value.clone()),
            ExpressionNode::Indexed(indexed) => Expression::Indexed(Box::new(IndexedExpression {
                collection: self.to_tree(indexed.collection),
                index: self.to_tree(indexed.index),
            })),
            ExpressionNode::Integer(value) => Expression::Integer(value.clone()),
            ExpressionNode::Member(member) => Expression::Member(Box::new(MemberExpression {
                receiver: self.to_tree(member.receiver),
                member_symbol: member.member_symbol,
                member: member.member.clone(),
                case_variant: member.case_variant.clone(),
            })),
            ExpressionNode::Borrow(inner_expression) => {
                Expression::Borrow(Box::new(BorrowExpression {
                    target: self.to_tree(inner_expression.target),
                    access: inner_expression.access,
                }))
            }
            ExpressionNode::Name(path) => Expression::Name(NamePath::resolved_with_member_symbols(
                self.name_path_members(path.members).to_vec(),
                self.name_path_member_symbols(path.member_symbols).to_vec(),
                path.head_symbol,
                path.symbol,
            )),
            ExpressionNode::Range(range) => Expression::Range(Box::new(RangeExpression {
                start: range
                    .start
                    .is_valid()
                    .then(|| Box::new(self.to_tree(range.start))),
                end: range
                    .end
                    .is_valid()
                    .then(|| Box::new(self.to_tree(range.end))),
                end_inclusive: range.end_inclusive,
            })),
            ExpressionNode::StructLiteral(struct_literal) => {
                Expression::StructLiteral(StructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    case_name: struct_literal.case_name.clone(),
                    fields: self
                        .struct_fields(struct_literal.fields)
                        .iter()
                        .map(|field| StructLiteralField {
                            name: field.name.clone(),
                            value: self.to_tree(field.value),
                        })
                        .collect::<Arc<[_]>>(),
                })
            }
            ExpressionNode::String(value) => Expression::String(value.clone()),
            ExpressionNode::Unary(unary) => Expression::Unary(Box::new(UnaryExpression {
                operator: unary.operator,
                operand: self.to_tree(unary.operand),
            })),
            ExpressionNode::ZeroValue(type_reference) => Expression::ZeroValue(*type_reference),
        }
    }

    pub fn to_tree_with_place_suffix(
        &self,
        expression: ExpressionHandle,
        suffix: &[Identifier],
    ) -> Expression {
        if suffix.is_empty() {
            return self.to_tree(expression);
        }

        match self.expression(expression) {
            ExpressionNode::Name(path) => {
                let mut resolved_path = self.name_path_to_tree(path);
                resolved_path.extend_from_slice(suffix);
                Expression::Name(resolved_path)
            }
            ExpressionNode::Indexed(indexed) => {
                if let Some(mut indexed_path) = self.indexed_expression_path(indexed) {
                    indexed_path.extend_from_slice(suffix);
                    Expression::Name(indexed_path)
                } else {
                    suffix
                        .iter()
                        .cloned()
                        .fold(self.to_tree(expression), |receiver, member| {
                            Expression::Member(Box::new(MemberExpression {
                                receiver,
                                member_symbol: SymbolHandle::invalid(),
                                member,
                                case_variant: None,
                            }))
                        })
                }
            }
            ExpressionNode::Borrow(target) => Expression::Borrow(Box::new(BorrowExpression {
                target: self.to_tree_with_place_suffix(target.target, suffix),
                access: target.access,
            })),
            // Same rule as the Indexed non-path arm above: NEVER drop the
            // suffix (the old catch-all returned the bare tree, so the caller
            // read the RECEIVER's place instead of the member's).
            _ => suffix
                .iter()
                .cloned()
                .fold(self.to_tree(expression), |receiver, member| {
                    Expression::Member(Box::new(MemberExpression {
                        receiver,
                        member_symbol: SymbolHandle::invalid(),
                        member,
                        case_variant: None,
                    }))
                }),
        }
    }

    fn name_path_to_tree(&self, path: &TableNamePath) -> NamePath {
        NamePath::resolved_with_member_symbols(
            self.name_path_members(path.members).to_vec(),
            self.name_path_member_symbols(path.member_symbols).to_vec(),
            path.head_symbol,
            path.symbol,
        )
    }

    fn indexed_expression_path(&self, indexed: &TableIndexedExpression) -> Option<NamePath> {
        let ExpressionNode::Integer(index) = self.expression(indexed.index) else {
            return None;
        };
        let mut path = match self.expression(indexed.collection) {
            ExpressionNode::Name(path) => self.name_path_to_tree(path),
            ExpressionNode::Indexed(inner_indexed) => {
                self.indexed_expression_path(inner_indexed)?
            }
            _ => return None,
        };
        let last_segment = path.last()?.clone();
        path.replace_last_preserving_symbol(Identifier::generated(format!(
            "{last_segment}[{index}]"
        )))?;
        Some(path)
    }

    pub fn display_name(&self, handle: ExpressionHandle) -> String {
        self.expression(handle).display_name(self)
    }

    /// Const-evaluate a PURE integer expression: a literal, or checked
    /// `+ - * / %` over constant integer subtrees (peeling `Mutable`). `None`
    /// the moment any leaf is not a constant integer (a place read, a call, a
    /// float) or an operation overflows / divides by zero -- callers treat
    /// `None` strictly as "not a constant", never as a value. The first
    /// consumer is range-constraint bounds (`[0 - 1..=40]` folds to
    /// `[-1..=40]` instead of silently behaving unbounded).
    pub fn constant_integer_value(&self, handle: ExpressionHandle) -> Option<i64> {
        if !handle.is_valid() {
            return None;
        }
        match self.expression(handle) {
            // The i64 window is this helper's CONTRACT (its consumers are
            // parse-time-numeric positions like range bounds); an oversize
            // literal reads as "not a constant here", which its callers
            // already reject loudly.
            ExpressionNode::Integer(value) => value.value_i64(),
            ExpressionNode::Borrow(inner) => self.constant_integer_value(inner.target),
            ExpressionNode::Binary(binary) => {
                let left = self.constant_integer_value(binary.left)?;
                let right = self.constant_integer_value(binary.right)?;
                match binary.operator {
                    BinaryOperator::Add => left.checked_add(right),
                    BinaryOperator::Subtract => left.checked_sub(right),
                    BinaryOperator::Multiply => left.checked_mul(right),
                    BinaryOperator::Divide => left.checked_div(right),
                    BinaryOperator::Modulo => left.checked_rem(right),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn string_literal(&self, handle: ExpressionHandle) -> Option<&[u8]> {
        match self.expression(handle) {
            ExpressionNode::String(value) => Some(value.as_ref()),
            _ => None,
        }
    }

    pub fn string_literal_value(&self, handle: ExpressionHandle) -> Option<Arc<[u8]>> {
        match self.expression(handle) {
            ExpressionNode::String(value) => Some(value.clone()),
            _ => None,
        }
    }
}

fn source_span_index(handle: ExpressionHandle) -> usize {
    usize::try_from(handle.arena_index())
        .expect("expression index overflow")
        .checked_sub(1)
        .expect("invalid expression handle has no source span")
}

impl Default for ExpressionTable {
    fn default() -> Self {
        Self::new()
    }
}

fn remapped(symbol: SymbolHandle, symbols: &[(SymbolHandle, SymbolHandle)]) -> SymbolHandle {
    symbols
        .iter()
        .find_map(|(source, target)| (*source == symbol).then_some(*target))
        .unwrap_or(symbol)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionNode {
    ArrayLiteral(HandleSpan<ExpressionHandle>),
    Atomic(TableAtomicExpression),
    Binary(TableBinaryExpression),
    Boolean(bool),
    Cast(TableCastExpression),
    Call(TableCallExpression),
    Float(FloatLiteral),
    Indexed(TableIndexedExpression),
    Integer(IntegerLiteral),
    Member(TableMemberExpression),
    Borrow(TableBorrowExpression),
    Name(TableNamePath),
    Range(TableRangeExpression),
    StructLiteral(TableStructLiteral),
    /// Exact decoded literal octets. Text interpretation, when required by a
    /// particular language construct, is an explicit checked operation.
    String(Arc<[u8]>),
    Unary(TableUnaryExpression),
    /// Proof-only observation of a type's normalized all-zero home value.
    ZeroValue(crate::types::TypeReferenceHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableBorrowExpression {
    pub target: ExpressionHandle,
    pub access: psi_language_core::ReferenceAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableAtomicExpression {
    pub value: ExpressionHandle,
    pub result: ExpressionHandle,
    pub ordering: psi_language_core::atomic::AtomicOrderingPlan,
}

impl Default for ExpressionNode {
    fn default() -> Self {
        Self::Integer(IntegerLiteral::zero())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableBinaryExpression {
    pub left: ExpressionHandle,
    pub operator: BinaryOperator,
    pub right: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaryExpression {
    pub operator: UnaryOperator,
    pub operand: Expression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableUnaryExpression {
    pub operator: UnaryOperator,
    pub operand: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCastExpression {
    pub value: ExpressionHandle,
    pub target_type: crate::types::TypeReferenceHandle,
    /// Diagnostic spelling only; semantic identity uses `target_type`.
    pub target_label: HandleSpan<Identifier>,
    /// Arithmetic domain cast (`x as u8 in Saturating`), decision 17 S2.
    pub domain: psi_numerics::arithmetic::ArithmeticDomain,
    /// A NON-policy `in <Name>` suffix -- the semantic-domain qualification
    /// spelling (decision 19), judged at validation (the staged mint fence).
    /// EMPTY = no suffix.
    pub semantic_domain: HandleSpan<Identifier>,
    /// PDI2 proof-static family arguments in the typed type-reference table.
    pub semantic_domain_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    /// Carrier-aware declared-domain identity, normalized once before
    /// validation. Invalid for an unknown or ambiguous spelling.
    pub semantic_domain_symbol: SymbolHandle,
    /// Exact normalized family-instance identity. Equal to the declaration ID
    /// for a monomorphic domain and distinct per canonical closed index pack.
    pub semantic_domain_id: psi_language_semantics::SemanticDomainId,
    /// Value conversion vs §5b borrow recast (`&x as &T`). Only `Value`
    /// survives past the typed trees today: the resolved->typed lowering is
    /// the recast judgment's choke point (rung A).
    pub form: psi_language_core::cast_form::CastForm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableIndexedExpression {
    pub collection: ExpressionHandle,
    pub index: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRangeExpression {
    pub start: ExpressionHandle,
    pub end: ExpressionHandle,
    pub end_inclusive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeExpression {
    pub start: Option<Box<Expression>>,
    pub end: Option<Box<Expression>>,
    pub end_inclusive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberExpression {
    pub receiver: Expression,
    pub member_symbol: SymbolHandle,
    pub member: Identifier,
    /// Case variant a destructure-bound payload field came from, so the backend
    /// offset resolver picks THAT variant's field (decision 17 / case payloads).
    pub case_variant: Option<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMemberExpression {
    pub receiver: ExpressionHandle,
    pub member_symbol: SymbolHandle,
    pub member: Identifier,
    /// Case variant a destructure-bound payload field came from, so the backend
    /// offset resolver picks THAT variant's field (decision 17 / case payloads).
    pub case_variant: Option<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallExpression {
    pub receiver: Option<Box<Expression>>,
    pub target_symbol: SymbolHandle,
    pub target: Identifier,
    pub arguments: Arc<[Expression]>,
    pub evidence_arguments: Arc<[Identifier]>,
    pub operational_acknowledgement: psi_language_semantics::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCallExpression {
    pub receiver: ExpressionHandle,
    pub target_symbol: SymbolHandle,
    pub target: Identifier,
    pub machine_arguments: Box<[StaticMachineArgument]>,
    /// An explicitly authored sealed quotient operation request. Retention is
    /// not admission: semantic validation must independently check quotient
    /// formation, operation correspondence, and the named conformance before
    /// this request can become executable.
    pub quotient_operation: Option<QuotientOperationRequest>,
    pub arguments: HandleSpan<ExpressionHandle>,
    pub evidence_arguments: Box<[Identifier]>,
    pub operational_acknowledgement: psi_language_semantics::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotientOperationKind {
    Lift,
    Define,
}

/// Exact source-selected identities for `Quotient::lift<F, Respect>` and
/// `Quotient::define<F, Respect>`. This checked-tree boundary deliberately
/// carries no derived quotient admission or executable lowering authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotientOperationRequest {
    pub kind: QuotientOperationKind,
    pub representative_operation: StaticMachineArgument,
    pub respect_conformance: StaticMachineArgument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticMachineArgument {
    /// Historical storage name shared by type/const/machine proposition
    /// arguments; proposition proof facts retain their final category.
    pub path: Box<[Identifier]>,
    pub application: Option<Box<StaticSymbolApplication>>,
    pub const_literal: Option<psi_numerics::literals::IntegerLiteral>,
    /// Proof-static projection from one named evidence term. The checked proof
    /// layer binds it to one stable opaque member of that retained term.
    pub evidence_projection: Option<EvidenceProjection>,
    /// Entry-state symbol of the selected concrete machine.
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticSymbolApplication {
    pub lifetime_arguments: Box<[Identifier]>,
    pub arguments: Box<[StaticMachineArgument]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceProjection {
    pub term: Identifier,
    pub member: Identifier,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableNamePath {
    pub members: HandleSpan<Identifier>,
    pub member_symbols: HandleSpan<SymbolHandle>,
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStructLiteral {
    pub type_name: Identifier,
    pub type_symbol: SymbolHandle,
    /// `Some` when the literal constructs a CASE of `type_name`
    /// (`Command::Say { text: ... }`); `None` for a plain record literal.
    pub case_name: Option<Identifier>,
    pub case_symbol: Option<SymbolHandle>,
    pub fields: HandleSpan<TableStructLiteralField>,
}

impl Default for TableStructLiteral {
    fn default() -> Self {
        Self {
            type_name: Identifier::default(),
            type_symbol: SymbolHandle::invalid(),
            case_name: None,
            case_symbol: None,
            fields: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStructLiteralField {
    pub name: Identifier,
    pub field_symbol: SymbolHandle,
    pub value: ExpressionHandle,
}

impl Default for TableStructLiteralField {
    fn default() -> Self {
        Self {
            name: Identifier::default(),
            field_symbol: SymbolHandle::invalid(),
            value: ExpressionHandle::invalid(),
        }
    }
}

impl Default for Expression {
    fn default() -> Self {
        Self::Integer(IntegerLiteral::zero())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamePath {
    members: Arc<[Identifier]>,
    member_symbols: Arc<[SymbolHandle]>,
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
}

impl NamePath {
    pub fn unresolved(members: Vec<Identifier>) -> Self {
        let member_symbols = vec![SymbolHandle::invalid(); members.len()];
        Self {
            members: Arc::from(members.into_boxed_slice()),
            member_symbols: Arc::from(member_symbols.into_boxed_slice()),
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }
    }

    pub fn unresolved_from_iter(members: impl IntoIterator<Item = Identifier>) -> Self {
        Self::unresolved(members.into_iter().collect())
    }

    pub fn resolved(
        members: Vec<Identifier>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Self {
        let mut member_symbols = vec![SymbolHandle::invalid(); members.len()];
        if let Some(first_symbol) = member_symbols.first_mut() {
            *first_symbol = head_symbol;
        }
        if let Some(last_symbol) = member_symbols.last_mut() {
            *last_symbol = symbol;
        }
        Self {
            members: Arc::from(members.into_boxed_slice()),
            member_symbols: Arc::from(member_symbols.into_boxed_slice()),
            head_symbol,
            symbol,
        }
    }

    pub fn resolved_with_member_symbols(
        members: Vec<Identifier>,
        mut member_symbols: Vec<SymbolHandle>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Self {
        if member_symbols.len() != members.len() {
            member_symbols.resize(members.len(), SymbolHandle::invalid());
        }
        if let Some(first_symbol) = member_symbols.first_mut() {
            *first_symbol = head_symbol;
        }
        if let Some(last_symbol) = member_symbols.last_mut() {
            *last_symbol = symbol;
        }
        Self {
            members: Arc::from(members.into_boxed_slice()),
            member_symbols: Arc::from(member_symbols.into_boxed_slice()),
            head_symbol,
            symbol,
        }
    }

    pub fn resolved_from_iter(
        members: impl IntoIterator<Item = Identifier>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Self {
        Self::resolved(members.into_iter().collect(), head_symbol, symbol)
    }

    pub fn members(&self) -> &[Identifier] {
        &self.members
    }

    pub fn member_symbols(&self) -> &[SymbolHandle] {
        &self.member_symbols
    }

    pub fn member_symbol(&self, index: usize) -> SymbolHandle {
        self.member_symbols
            .get(index)
            .copied()
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn first(&self) -> Option<&Identifier> {
        self.members.first()
    }

    pub fn last(&self) -> Option<&Identifier> {
        self.members.last()
    }

    pub fn last_mut(&mut self) -> Option<&mut Identifier> {
        self.symbol = SymbolHandle::invalid();
        if let Some(last_symbol) = Arc::make_mut(&mut self.member_symbols).last_mut() {
            *last_symbol = SymbolHandle::invalid();
        }
        Arc::make_mut(&mut self.members).last_mut()
    }

    pub fn replace_last_preserving_symbol(&mut self, member: Identifier) -> Option<()> {
        let last = Arc::make_mut(&mut self.members).last_mut()?;
        *last = member;
        Some(())
    }

    pub fn push(&mut self, member: Identifier) {
        let mut members = self.members.iter().cloned().collect::<Vec<_>>();
        members.push(member);
        self.members = Arc::from(members.into_boxed_slice());

        let mut member_symbols = self.member_symbols.iter().copied().collect::<Vec<_>>();
        member_symbols.push(SymbolHandle::invalid());
        self.member_symbols = Arc::from(member_symbols.into_boxed_slice());
        self.symbol = SymbolHandle::invalid();
    }

    pub fn push_resolved(&mut self, member: Identifier, symbol: SymbolHandle) {
        let mut members = self.members.iter().cloned().collect::<Vec<_>>();
        members.push(member);
        self.members = Arc::from(members.into_boxed_slice());

        let mut member_symbols = self.member_symbols.iter().copied().collect::<Vec<_>>();
        member_symbols.push(symbol);
        self.member_symbols = Arc::from(member_symbols.into_boxed_slice());
        self.symbol = symbol;
    }

    pub fn extend_from_slice(&mut self, members: &[Identifier]) {
        let mut path = self.members.iter().cloned().collect::<Vec<_>>();
        path.extend_from_slice(members);
        self.members = Arc::from(path.into_boxed_slice());

        let mut member_symbols = self.member_symbols.iter().copied().collect::<Vec<_>>();
        member_symbols.extend(std::iter::repeat_with(SymbolHandle::invalid).take(members.len()));
        self.member_symbols = Arc::from(member_symbols.into_boxed_slice());
        self.symbol = SymbolHandle::invalid();
    }

    pub fn head_symbol(&self) -> SymbolHandle {
        self.head_symbol
    }

    pub fn symbol(&self) -> SymbolHandle {
        self.symbol
    }

    pub fn with_symbols(mut self, head_symbol: SymbolHandle, symbol: SymbolHandle) -> Self {
        self.head_symbol = head_symbol;
        self.symbol = symbol;
        if let Some(first_symbol) = Arc::make_mut(&mut self.member_symbols).first_mut() {
            *first_symbol = head_symbol;
        }
        if let Some(last_symbol) = Arc::make_mut(&mut self.member_symbols).last_mut() {
            *last_symbol = symbol;
        }
        self
    }
}

impl Deref for NamePath {
    type Target = [Identifier];

    fn deref(&self) -> &Self::Target {
        self.members()
    }
}

impl<'path> IntoIterator for &'path NamePath {
    type Item = &'path Identifier;
    type IntoIter = std::slice::Iter<'path, Identifier>;

    fn into_iter(self) -> Self::IntoIter {
        self.members.iter()
    }
}

/// The shared TEXT-based float carrier (F2): the source spelling plus an
/// optional format landing ride every tree layer, exactly like
/// IntegerLiteral -- per-format reads are each correctly rounded from the
/// spelling, so f32 never routes through f64.
pub use psi_numerics::literals::FloatLiteral;

fn identifier_texts_equal(a: &[Identifier], b: &[Identifier]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(left, right)| left.as_str() == right.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryExpression {
    pub left: Expression,
    pub operator: BinaryOperator,
    pub right: Expression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    And,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Divide,
    Equal,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Modulo,
    Multiply,
    NotEqual,
    Or,
    ShiftLeft,
    ShiftRight,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    BitwiseNot,
    LogicalNot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastExpression {
    pub value: Expression,
    pub target_type: crate::types::TypeReferenceHandle,
    /// Diagnostic spelling only; semantic identity uses `target_type`.
    pub target_label: NamePath,
    /// Arithmetic domain cast (`x as u8 in Saturating`), decision 17 S2.
    pub domain: psi_numerics::arithmetic::ArithmeticDomain,
    /// Value conversion vs §5b borrow recast (`&x as &T`).
    pub form: psi_language_core::cast_form::CastForm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedExpression {
    pub collection: Expression,
    pub index: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteral {
    pub type_name: Identifier,
    /// `Some` when the literal constructs a CASE of `type_name`; `None` for a
    /// plain record literal.
    pub case_name: Option<Identifier>,
    pub fields: Arc<[StructLiteralField]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteralField {
    pub name: Identifier,
    pub value: Expression,
}
