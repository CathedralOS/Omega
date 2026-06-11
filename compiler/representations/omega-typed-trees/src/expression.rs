use crate::name::Identifier;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
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
    Binary(Box<BinaryExpression>),
    Boolean(bool),
    Cast(Box<CastExpression>),
    Call(Box<CallExpression>),
    Float(FloatLiteral),
    Indexed(Box<IndexedExpression>),
    Integer(i64),
    Member(Box<MemberExpression>),
    Mutable(Box<Expression>),
    Name(NamePath),
    Range(Box<RangeExpression>),
    StructLiteral(StructLiteral),
    String(Arc<str>),
    Unary(Box<UnaryExpression>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionTable {
    expressions: Arena<ExpressionNode>,
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
            expression_handles: Arena::with_capacity(capacity.expression_handles),
            name_path_members: Arena::with_capacity(capacity.name_path_members),
            name_path_member_symbols: Arena::with_capacity(capacity.name_path_member_symbols),
            struct_fields: Arena::with_capacity(capacity.struct_fields),
        }
    }

    pub fn clear(&mut self) {
        self.expressions.reset_retain_capacity();
        self.expression_handles.reset_retain_capacity();
        self.name_path_members.reset_retain_capacity();
        self.name_path_member_symbols.reset_retain_capacity();
        self.struct_fields.reset_retain_capacity();
    }

    pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
        self.expressions.insert(expression)
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
        match source.expression(expression) {
            ExpressionNode::ArrayLiteral(source_values) => {
                let values = self.copy_expression_handles_from(source, *source_values);
                self.insert(ExpressionNode::ArrayLiteral(values))
            }
            ExpressionNode::Binary(binary) => {
                let left = self.copy_from(source, binary.left);
                let right = self.copy_from(source, binary.right);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(*value)),
            ExpressionNode::Cast(cast) => {
                let value = self.copy_from(source, cast.value);
                let target_type = self.copy_name_path_members(source, cast.target_type);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
                }))
            }
            ExpressionNode::Call(call) => {
                let receiver = call
                    .receiver
                    .is_valid()
                    .then(|| self.copy_from(source, call.receiver))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let arguments = self.copy_expression_handles_from(source, call.arguments);
                self.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target.clone(),
                    arguments,
                }))
            }
            ExpressionNode::Float(value) => self.insert(ExpressionNode::Float(*value)),
            ExpressionNode::Indexed(indexed) => {
                let collection = self.copy_from(source, indexed.collection);
                let index = self.copy_from(source, indexed.index);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(*value)),
            ExpressionNode::Member(member) => {
                let receiver = self.copy_from(source, member.receiver);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                }))
            }
            ExpressionNode::Mutable(inner_expression) => {
                let inner_expression = self.copy_from(source, *inner_expression);
                self.insert(ExpressionNode::Mutable(inner_expression))
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
                    .then(|| self.copy_from(source, range.start))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let end = range
                    .end
                    .is_valid()
                    .then(|| self.copy_from(source, range.end))
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                let fields = self.copy_struct_literal_fields(source, struct_literal.fields);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    case_name: struct_literal.case_name.clone(),
                    fields,
                }))
            }
            ExpressionNode::String(value) => self.insert(ExpressionNode::String(value.clone())),
            ExpressionNode::Unary(unary) => {
                let operand = self.copy_from(source, unary.operand);
                self.insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                }))
            }
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

    fn copy_struct_literal_fields(
        &mut self,
        source: &ExpressionTable,
        fields: HandleSpan<TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        let mut copied = HandleSpan::empty();

        for field in source.struct_fields(fields) {
            let value = self.copy_from(source, field.value);
            self.struct_fields.append_to_span(
                &mut copied,
                TableStructLiteralField {
                    name: field.name.clone(),
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
                    value,
                },
            );
        }

        field_span
    }

    pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
        self.expressions.get(handle)
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
            ExpressionNode::Mutable(inner) => self.expression_is_direct_place_path(*inner),
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
            (ExpressionNode::Mutable(x), ExpressionNode::Mutable(y)) => {
                self.expressions_structurally_equal(*x, *y)
            }
            (ExpressionNode::Unary(x), ExpressionNode::Unary(y)) => {
                x.operator == y.operator && self.expressions_structurally_equal(x.operand, y.operand)
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
            ExpressionNode::Mutable(target) => {
                let target = self.insert_copy_with_member_suffix(
                    target,
                    suffix_members,
                    suffix_member_symbols,
                    suffix_start_offset,
                );
                self.insert(ExpressionNode::Mutable(target))
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
            _ => self.insert_copy(expression),
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
                let target_type = self.copy_own_name_path_members(cast.target_type);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
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
                    arguments,
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
                }))
            }
            ExpressionNode::Mutable(inner_expression) => {
                let inner_expression = self.insert_copy(inner_expression);
                self.insert(ExpressionNode::Mutable(inner_expression))
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
                    case_name: struct_literal.case_name,
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
        }
    }

    fn copy_indexed_expression_path(
        &mut self,
        indexed: TableIndexedExpression,
    ) -> Option<TableNamePath> {
        let ExpressionNode::Integer(index) = self.expression(indexed.index) else {
            return None;
        };
        let index = *index;

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
                let target_type = self.insert_name_path_members(&cast.target_type);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
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
                    arguments,
                }))
            }
            Expression::Float(value) => self.insert(ExpressionNode::Float(*value)),
            Expression::Indexed(indexed) => {
                let collection = self.insert_tree(&indexed.collection);
                let index = self.insert_tree(&indexed.index);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            Expression::Integer(value) => self.insert(ExpressionNode::Integer(*value)),
            Expression::Member(member) => {
                let receiver = self.insert_tree(&member.receiver);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                }))
            }
            Expression::Mutable(inner_expression) => {
                let inner_expression = self.insert_tree(inner_expression);
                self.insert(ExpressionNode::Mutable(inner_expression))
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
                    case_name: struct_literal.case_name.clone(),
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
            ExpressionNode::Binary(binary) => Expression::Binary(Box::new(BinaryExpression {
                left: self.to_tree(binary.left),
                operator: binary.operator,
                right: self.to_tree(binary.right),
            })),
            ExpressionNode::Boolean(value) => Expression::Boolean(*value),
            ExpressionNode::Cast(cast) => Expression::Cast(Box::new(CastExpression {
                value: self.to_tree(cast.value),
                target_type: NamePath::unresolved_from_iter(
                    self.name_path_members(cast.target_type).iter().cloned(),
                ),
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
            })),
            ExpressionNode::Float(value) => Expression::Float(*value),
            ExpressionNode::Indexed(indexed) => Expression::Indexed(Box::new(IndexedExpression {
                collection: self.to_tree(indexed.collection),
                index: self.to_tree(indexed.index),
            })),
            ExpressionNode::Integer(value) => Expression::Integer(*value),
            ExpressionNode::Member(member) => Expression::Member(Box::new(MemberExpression {
                receiver: self.to_tree(member.receiver),
                member_symbol: member.member_symbol,
                member: member.member.clone(),
            })),
            ExpressionNode::Mutable(inner_expression) => {
                Expression::Mutable(Box::new(self.to_tree(*inner_expression)))
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
                            }))
                        })
                }
            }
            ExpressionNode::Mutable(target) => {
                Expression::Mutable(Box::new(self.to_tree_with_place_suffix(*target, suffix)))
            }
            _ => self.to_tree(expression),
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

    pub fn string_literal(&self, handle: ExpressionHandle) -> Option<&str> {
        match self.expression(handle) {
            ExpressionNode::String(value) => Some(value.as_ref()),
            _ => None,
        }
    }

    pub fn string_literal_value(&self, handle: ExpressionHandle) -> Option<Arc<str>> {
        match self.expression(handle) {
            ExpressionNode::String(value) => Some(value.clone()),
            _ => None,
        }
    }
}

impl Default for ExpressionTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionNode {
    ArrayLiteral(HandleSpan<ExpressionHandle>),
    Binary(TableBinaryExpression),
    Boolean(bool),
    Cast(TableCastExpression),
    Call(TableCallExpression),
    Float(FloatLiteral),
    Indexed(TableIndexedExpression),
    Integer(i64),
    Member(TableMemberExpression),
    Mutable(ExpressionHandle),
    Name(TableNamePath),
    Range(TableRangeExpression),
    StructLiteral(TableStructLiteral),
    String(Arc<str>),
    Unary(TableUnaryExpression),
}

impl Default for ExpressionNode {
    fn default() -> Self {
        Self::Integer(0)
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
    pub target_type: HandleSpan<Identifier>,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMemberExpression {
    pub receiver: ExpressionHandle,
    pub member_symbol: SymbolHandle,
    pub member: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallExpression {
    pub receiver: Option<Box<Expression>>,
    pub target_symbol: SymbolHandle,
    pub target: Identifier,
    pub arguments: Arc<[Expression]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCallExpression {
    pub receiver: ExpressionHandle,
    pub target_symbol: SymbolHandle,
    pub target: Identifier,
    pub arguments: HandleSpan<ExpressionHandle>,
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
    /// `Some` when the literal constructs a CASE of `type_name`
    /// (`Command::Say { text: ... }`); `None` for a plain record literal.
    pub case_name: Option<Identifier>,
    pub fields: HandleSpan<TableStructLiteralField>,
}

impl Default for TableStructLiteral {
    fn default() -> Self {
        Self {
            type_name: Identifier::default(),
            case_name: None,
            fields: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStructLiteralField {
    pub name: Identifier,
    pub value: ExpressionHandle,
}

impl Default for TableStructLiteralField {
    fn default() -> Self {
        Self {
            name: Identifier::default(),
            value: ExpressionHandle::invalid(),
        }
    }
}

impl Default for Expression {
    fn default() -> Self {
        Self::Integer(0)
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FloatLiteral {
    bits: u64,
}

fn identifier_texts_equal(a: &[Identifier], b: &[Identifier]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(left, right)| left.as_str() == right.as_str())
}

impl FloatLiteral {
    pub fn new(value: f64) -> Self {
        Self {
            bits: value.to_bits(),
        }
    }

    pub fn parse(source: &str) -> Option<Self> {
        let normalized = strip_float_literal_suffix(source);
        normalized.parse::<f64>().ok().map(Self::new)
    }

    pub fn value(self) -> f64 {
        f64::from_bits(self.bits)
    }
}

fn strip_float_literal_suffix(source: &str) -> &str {
    for suffix in ["real", "Real", "f32", "f64"] {
        if let Some(value) = source.strip_suffix(suffix) {
            return value;
        }
    }

    source.trim_end_matches(['f', 'F'])
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
    LogicalNot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastExpression {
    pub value: Expression,
    pub target_type: NamePath,
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
