use crate::name::DiagnosticName;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::source::SourceText;
use omega_core::symbols::SymbolHandle;
use std::fmt;
use std::ops::{Deref, DerefMut};

pub type ExpressionHandle = Handle<ExpressionNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    ArrayLiteral(ArrayLiteralExpression),
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
    StructLiteral(StructLiteral),
    String(SourceText),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayLiteralExpression {
    pub storage: ArrayLiteralExpressionStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArrayLiteralExpressionStorage {
    pub values: Box<[Expression]>,
}

impl Deref for ArrayLiteralExpression {
    type Target = ArrayLiteralExpressionStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for ArrayLiteralExpression {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionTable {
    nodes: ExpressionNodeStorage,
    spans: ExpressionSpanStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpressionNodeStorage {
    expressions: Arena<ExpressionNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpressionSpanStorage {
    expression_handles: Arena<ExpressionHandle>,
    name_path_members: Arena<DiagnosticName>,
    struct_fields: Arena<TableStructLiteralField>,
}

impl ExpressionTable {
    pub fn new() -> Self {
        Self {
            nodes: ExpressionNodeStorage {
                expressions: Arena::new(),
            },
            spans: ExpressionSpanStorage {
                expression_handles: Arena::new(),
                name_path_members: Arena::new(),
                struct_fields: Arena::new(),
            },
        }
    }

    pub fn clear(&mut self) {
        self.nodes.expressions.reset_retain_capacity();
        self.spans.expression_handles.reset_retain_capacity();
        self.spans.name_path_members.reset_retain_capacity();
        self.spans.struct_fields.reset_retain_capacity();
    }

    pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
        self.nodes.expressions.insert(expression)
    }

    pub fn insert_expression_handles(
        &mut self,
        expressions: impl IntoIterator<Item = ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.spans.expression_handles.insert_many(expressions)
    }

    pub fn reserve_expression_handles(&mut self, count: u32) -> HandleSpan<ExpressionHandle> {
        self.spans.expression_handles.insert_many(
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
        *self.spans.expression_handles.get_mut(Handle::from_parts(
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
        self.spans
            .expression_handles
            .append_to_span(span, expression);
    }

    pub fn insert_struct_fields(
        &mut self,
        fields: impl IntoIterator<Item = TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        self.spans.struct_fields.insert_many(fields)
    }

    pub fn reserve_struct_fields(&mut self, count: u32) -> HandleSpan<TableStructLiteralField> {
        self.spans.struct_fields.insert_many(
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
        *self.spans.struct_fields.get_mut(Handle::from_parts(
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
        self.spans.struct_fields.append_to_span(span, field);
    }

    pub fn push_name_path_member(
        &mut self,
        span: &mut HandleSpan<DiagnosticName>,
        member: DiagnosticName,
    ) {
        self.spans.name_path_members.append_to_span(span, member);
    }

    pub fn copy_from(
        &mut self,
        source: &ExpressionTable,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        match source.expression(expression) {
            ExpressionNode::ArrayLiteral(values) => {
                let copied_values = self.reserve_expression_handles(values.count());

                for offset in 0..values.count() {
                    let value = source.expression_handle_at_offset(*values, offset);
                    let value = self.copy_from(source, *value);
                    self.set_expression_handle_at_offset(copied_values, offset, value);
                }

                self.insert(ExpressionNode::ArrayLiteral(copied_values))
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
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    head_symbol: path.head_symbol,
                    symbol: path.symbol,
                }))
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                let fields = self.copy_struct_literal_fields(source, struct_literal.fields);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    fields,
                }))
            }
            ExpressionNode::String(value) => self.insert(ExpressionNode::String(value.clone())),
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
        let span = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression = self.copy_from(source, *expression);
            self.set_expression_handle_at_offset(
                span,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
        }

        span
    }

    fn insert_name_path_members(&mut self, path: &NamePath) -> HandleSpan<DiagnosticName> {
        let mut span = HandleSpan::empty();

        for member in path.members() {
            self.spans
                .name_path_members
                .append_to_span(&mut span, member.clone());
        }

        span
    }

    fn copy_name_path_members(
        &mut self,
        source: &ExpressionTable,
        members: HandleSpan<DiagnosticName>,
    ) -> HandleSpan<DiagnosticName> {
        let mut span = HandleSpan::empty();

        for member in source.name_path_members(members) {
            self.spans
                .name_path_members
                .append_to_span(&mut span, member.clone());
        }

        span
    }

    fn copy_struct_literal_fields(
        &mut self,
        source: &ExpressionTable,
        fields: HandleSpan<TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        let span = self.reserve_struct_fields(fields.count());

        for offset in 0..fields.count() {
            let field = source.struct_field_at_offset(fields, offset);
            let value = self.copy_from(source, field.value);
            self.set_struct_field_at_offset(
                span,
                offset,
                TableStructLiteralField {
                    name: field.name.clone(),
                    value,
                },
            );
        }

        span
    }

    fn insert_expression_handle_span_from_trees(
        &mut self,
        expressions: &[Expression],
    ) -> HandleSpan<ExpressionHandle> {
        let span = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression = self.insert_tree(expression);
            self.set_expression_handle_at_offset(
                span,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
        }

        span
    }

    fn insert_struct_field_span_from_tree(
        &mut self,
        fields: &[StructLiteralField],
    ) -> HandleSpan<TableStructLiteralField> {
        let span = self.reserve_struct_fields(
            fields
                .len()
                .try_into()
                .expect("struct literal field span count overflow"),
        );

        for (offset, field) in fields.iter().enumerate() {
            let value = self.insert_tree(&field.value);
            self.set_struct_field_at_offset(
                span,
                offset
                    .try_into()
                    .expect("struct literal field span count overflow"),
                TableStructLiteralField {
                    name: field.name.clone(),
                    value,
                },
            );
        }

        span
    }

    pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
        self.nodes.expressions.get(handle)
    }

    pub fn expression_mut(&mut self, handle: ExpressionHandle) -> &mut ExpressionNode {
        self.nodes.expressions.get_mut(handle)
    }

    pub fn expression_handles(&self, span: HandleSpan<ExpressionHandle>) -> &[ExpressionHandle] {
        self.spans.expression_handles.span_or_empty(span)
    }

    pub fn expression_handle_at_offset(
        &self,
        expressions: HandleSpan<ExpressionHandle>,
        offset: u32,
    ) -> &ExpressionHandle {
        self.spans.expression_handles.get(Handle::from_parts(
            expressions
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("expression handle index overflow"),
            expressions.start().generation(),
        ))
    }

    pub fn struct_fields(
        &self,
        span: HandleSpan<TableStructLiteralField>,
    ) -> &[TableStructLiteralField] {
        self.spans.struct_fields.span_or_empty(span)
    }

    pub fn struct_field_at_offset(
        &self,
        fields: HandleSpan<TableStructLiteralField>,
        offset: u32,
    ) -> &TableStructLiteralField {
        self.spans.struct_fields.get(Handle::from_parts(
            fields
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("struct literal field index overflow"),
            fields.start().generation(),
        ))
    }

    pub fn name_path_members(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
        self.spans.name_path_members.span_or_empty(span)
    }

    pub fn copy_name_path_members_with_suffix(
        &mut self,
        members: HandleSpan<DiagnosticName>,
        suffix: DiagnosticName,
    ) -> HandleSpan<DiagnosticName> {
        let mut span = HandleSpan::empty();

        for offset in 0..members.count() {
            let member = self
                .spans
                .name_path_members
                .get(Handle::from_parts(
                    members
                        .start()
                        .arena_index()
                        .checked_add(offset)
                        .expect("name path member index overflow"),
                    members.start().generation(),
                ))
                .clone();
            self.spans
                .name_path_members
                .append_to_span(&mut span, member);
        }

        self.spans
            .name_path_members
            .append_to_span(&mut span, suffix);

        span
    }

    pub fn copy_name_path_members_with_member_suffix(
        &mut self,
        members: HandleSpan<DiagnosticName>,
        suffix_members: HandleSpan<DiagnosticName>,
        suffix_start_offset: u32,
    ) -> HandleSpan<DiagnosticName> {
        let mut span = HandleSpan::empty();

        for offset in 0..members.count() {
            let member = self
                .spans
                .name_path_members
                .get(Handle::from_parts(
                    members
                        .start()
                        .arena_index()
                        .checked_add(offset)
                        .expect("name path member index overflow"),
                    members.start().generation(),
                ))
                .clone();
            self.spans
                .name_path_members
                .append_to_span(&mut span, member);
        }

        for offset in suffix_start_offset..suffix_members.count() {
            let member = self
                .spans
                .name_path_members
                .get(Handle::from_parts(
                    suffix_members
                        .start()
                        .arena_index()
                        .checked_add(offset)
                        .expect("name path member index overflow"),
                    suffix_members.start().generation(),
                ))
                .clone();
            self.spans
                .name_path_members
                .append_to_span(&mut span, member);
        }

        span
    }

    pub fn insert_copy_with_member_suffix(
        &mut self,
        expression: ExpressionHandle,
        suffix_members: HandleSpan<DiagnosticName>,
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
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    head_symbol: path.head_symbol,
                    symbol: SymbolHandle::invalid(),
                }))
            }
            ExpressionNode::Mutable(target) => {
                let target = self.insert_copy_with_member_suffix(
                    target,
                    suffix_members,
                    suffix_start_offset,
                );
                self.insert(ExpressionNode::Mutable(target))
            }
            _ => {
                let expression = self.to_tree_with_place_suffix_members(
                    expression,
                    suffix_members,
                    suffix_start_offset,
                );
                self.insert_tree(&expression)
            }
        }
    }

    pub fn expression_count(&self) -> usize {
        self.nodes.expressions.len()
    }

    pub fn struct_field_count(&self) -> usize {
        self.spans.struct_fields.len()
    }

    pub fn insert_tree(&mut self, expression: &Expression) -> ExpressionHandle {
        match expression {
            Expression::ArrayLiteral(array_literal) => {
                let values = self.insert_expression_handle_span_from_trees(&array_literal.values);
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
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    head_symbol: path.head_symbol(),
                    symbol: path.symbol(),
                }))
            }
            Expression::StructLiteral(struct_literal) => {
                let fields = self.insert_struct_field_span_from_tree(&struct_literal.fields);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    fields,
                }))
            }
            Expression::String(value) => self.insert(ExpressionNode::String(value.clone())),
        }
    }

    pub fn to_tree(&self, expression: ExpressionHandle) -> Expression {
        match self.expression(expression) {
            ExpressionNode::ArrayLiteral(values) => {
                Expression::ArrayLiteral(ArrayLiteralExpression {
                    storage: ArrayLiteralExpressionStorage {
                        values: self
                            .expression_handles(*values)
                            .iter()
                            .map(|value| self.to_tree(*value))
                            .collect(),
                    },
                })
            }
            ExpressionNode::Binary(binary) => Expression::Binary(Box::new(BinaryExpression {
                storage: BinaryExpressionStorage {
                    left: self.to_tree(binary.left),
                    operator: binary.operator,
                    right: self.to_tree(binary.right),
                },
            })),
            ExpressionNode::Boolean(value) => Expression::Boolean(*value),
            ExpressionNode::Cast(cast) => Expression::Cast(Box::new(CastExpression {
                storage: CastExpressionStorage {
                    value: self.to_tree(cast.value),
                    target_type: NamePath::unresolved_from_iter(
                        self.name_path_members(cast.target_type).iter().cloned(),
                    ),
                },
            })),
            ExpressionNode::Call(call) => Expression::Call(Box::new(CallExpression {
                target_symbol: call.target_symbol,
                target: call.target.clone(),
                storage: CallExpressionStorage {
                    receiver: call
                        .receiver
                        .is_valid()
                        .then(|| Box::new(self.to_tree(call.receiver))),
                    arguments: self
                        .expression_handles(call.arguments)
                        .iter()
                        .map(|argument| self.to_tree(*argument))
                        .collect(),
                },
            })),
            ExpressionNode::Float(value) => Expression::Float(*value),
            ExpressionNode::Indexed(indexed) => Expression::Indexed(Box::new(IndexedExpression {
                storage: IndexedExpressionStorage {
                    collection: self.to_tree(indexed.collection),
                    index: self.to_tree(indexed.index),
                },
            })),
            ExpressionNode::Integer(value) => Expression::Integer(*value),
            ExpressionNode::Member(member) => Expression::Member(Box::new(MemberExpression {
                storage: MemberExpressionStorage {
                    receiver: self.to_tree(member.receiver),
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                },
            })),
            ExpressionNode::Mutable(inner_expression) => {
                Expression::Mutable(Box::new(self.to_tree(*inner_expression)))
            }
            ExpressionNode::Name(path) => Expression::Name(NamePath::resolved_from_iter(
                self.name_path_members(path.members).iter().cloned(),
                path.head_symbol,
                path.symbol,
            )),
            ExpressionNode::StructLiteral(struct_literal) => {
                Expression::StructLiteral(StructLiteral {
                    storage: StructLiteralStorage {
                        type_name: struct_literal.type_name.clone(),
                        fields: self
                            .struct_fields(struct_literal.fields)
                            .iter()
                            .map(|field| StructLiteralField {
                                name: field.name.clone(),
                                value: self.to_tree(field.value),
                            })
                            .collect(),
                    },
                })
            }
            ExpressionNode::String(value) => Expression::String(value.clone()),
        }
    }

    pub fn to_tree_with_place_suffix(
        &self,
        expression: ExpressionHandle,
        suffix: &[DiagnosticName],
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
                    self.to_tree(expression)
                }
            }
            ExpressionNode::Mutable(target) => {
                Expression::Mutable(Box::new(self.to_tree_with_place_suffix(*target, suffix)))
            }
            _ => self.to_tree(expression),
        }
    }

    pub fn to_tree_with_place_suffix_members(
        &self,
        expression: ExpressionHandle,
        suffix_members: HandleSpan<DiagnosticName>,
        suffix_start_offset: u32,
    ) -> Expression {
        if suffix_start_offset >= suffix_members.count() {
            return self.to_tree(expression);
        }

        let suffix_start = usize::try_from(suffix_start_offset).expect("suffix offset overflow");
        let suffix = &self.name_path_members(suffix_members)[suffix_start..];

        match self.expression(expression) {
            ExpressionNode::Name(path) => {
                let mut resolved_path = self.name_path_to_tree(path);
                resolved_path.extend_from_iter(suffix.iter().cloned());
                Expression::Name(resolved_path)
            }
            ExpressionNode::Indexed(indexed) => {
                if let Some(mut indexed_path) = self.indexed_expression_path(indexed) {
                    indexed_path.extend_from_iter(suffix.iter().cloned());
                    Expression::Name(indexed_path)
                } else {
                    self.to_tree(expression)
                }
            }
            ExpressionNode::Mutable(target) => {
                Expression::Mutable(Box::new(self.to_tree_with_place_suffix_members(
                    *target,
                    suffix_members,
                    suffix_start_offset,
                )))
            }
            _ => self.to_tree(expression),
        }
    }

    fn name_path_to_tree(&self, path: &TableNamePath) -> NamePath {
        NamePath::resolved_from_iter(
            self.name_path_members(path.members).iter().cloned(),
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
        let last_segment = path.last_mut()?;
        *last_segment = DiagnosticName::generated(format!("{last_segment}[{index}]"));
        Some(path)
    }

    pub fn display_name(&self, handle: ExpressionHandle) -> String {
        self.expression(handle).display_name(self)
    }

    pub fn string_literal(&self, handle: ExpressionHandle) -> Option<&str> {
        match self.expression(handle) {
            ExpressionNode::String(value) => Some(value.as_str()),
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
    StructLiteral(TableStructLiteral),
    String(SourceText),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCastExpression {
    pub value: ExpressionHandle,
    pub target_type: HandleSpan<DiagnosticName>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableIndexedExpression {
    pub collection: ExpressionHandle,
    pub index: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberExpression {
    pub storage: MemberExpressionStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemberExpressionStorage {
    pub receiver: Expression,
    pub member_symbol: SymbolHandle,
    pub member: DiagnosticName,
}

impl Deref for MemberExpression {
    type Target = MemberExpressionStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for MemberExpression {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMemberExpression {
    pub receiver: ExpressionHandle,
    pub member_symbol: SymbolHandle,
    pub member: DiagnosticName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallExpression {
    pub target_symbol: SymbolHandle,
    pub target: DiagnosticName,
    pub storage: CallExpressionStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallExpressionStorage {
    pub receiver: Option<Box<Expression>>,
    pub arguments: Box<[Expression]>,
}

impl Deref for CallExpression {
    type Target = CallExpressionStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for CallExpression {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCallExpression {
    pub receiver: ExpressionHandle,
    pub target_symbol: SymbolHandle,
    pub target: DiagnosticName,
    pub arguments: HandleSpan<ExpressionHandle>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableNamePath {
    pub members: HandleSpan<DiagnosticName>,
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStructLiteral {
    pub type_name: DiagnosticName,
    pub fields: HandleSpan<TableStructLiteralField>,
}

impl Default for TableStructLiteral {
    fn default() -> Self {
        Self {
            type_name: DiagnosticName::default(),
            fields: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStructLiteralField {
    pub name: DiagnosticName,
    pub value: ExpressionHandle,
}

impl Default for TableStructLiteralField {
    fn default() -> Self {
        Self {
            name: DiagnosticName::default(),
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
    storage: NamePathStorage,
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamePathStorage {
    members: Box<[DiagnosticName]>,
}

impl NamePath {
    pub fn unresolved_from_iter(members: impl IntoIterator<Item = DiagnosticName>) -> Self {
        Self::unresolved_from_boxed_slice(members.into_iter().collect())
    }

    fn unresolved_from_boxed_slice(members: Box<[DiagnosticName]>) -> Self {
        Self {
            storage: NamePathStorage { members },
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }
    }

    pub fn resolved_from_iter(
        members: impl IntoIterator<Item = DiagnosticName>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Self {
        Self::resolved_from_boxed_slice(members.into_iter().collect(), head_symbol, symbol)
    }

    fn resolved_from_boxed_slice(
        members: Box<[DiagnosticName]>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Self {
        Self {
            storage: NamePathStorage { members },
            head_symbol,
            symbol,
        }
    }

    pub fn members(&self) -> &[DiagnosticName] {
        &self.storage.members
    }

    pub fn extend_from_slice(&mut self, members: &[DiagnosticName]) {
        self.extend_from_iter(members.iter().cloned());
    }

    pub fn extend_from_iter(&mut self, members: impl IntoIterator<Item = DiagnosticName>) {
        let mut owned_members = std::mem::take(&mut self.storage.members).into_vec();
        owned_members.extend(members);
        self.storage.members = owned_members.into_boxed_slice();
        self.symbol = SymbolHandle::invalid();
    }

    pub fn len(&self) -> usize {
        self.storage.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.storage.members.is_empty()
    }

    pub fn first(&self) -> Option<&DiagnosticName> {
        self.storage.members.first()
    }

    pub fn last(&self) -> Option<&DiagnosticName> {
        self.storage.members.last()
    }

    pub fn last_mut(&mut self) -> Option<&mut DiagnosticName> {
        self.symbol = SymbolHandle::invalid();
        self.storage.members.last_mut()
    }

    pub fn head_symbol(&self) -> SymbolHandle {
        self.head_symbol
    }

    pub fn symbol(&self) -> SymbolHandle {
        self.symbol
    }

    pub fn set_symbols(&mut self, head_symbol: SymbolHandle, symbol: SymbolHandle) {
        self.head_symbol = head_symbol;
        self.symbol = symbol;
    }

    pub fn with_symbols(mut self, head_symbol: SymbolHandle, symbol: SymbolHandle) -> Self {
        self.set_symbols(head_symbol, symbol);
        self
    }
}

impl Deref for NamePath {
    type Target = [DiagnosticName];

    fn deref(&self) -> &Self::Target {
        self.members()
    }
}

impl<'path> IntoIterator for &'path NamePath {
    type Item = &'path DiagnosticName;
    type IntoIter = std::slice::Iter<'path, DiagnosticName>;

    fn into_iter(self) -> Self::IntoIter {
        self.storage.members.iter()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FloatLiteral {
    bits: u64,
}

impl FloatLiteral {
    pub fn new(value: f64) -> Self {
        Self {
            bits: value.to_bits(),
        }
    }

    pub fn parse(source: &str) -> Option<Self> {
        let normalized = source.trim_end_matches(['f', 'F']);
        normalized.parse::<f64>().ok().map(Self::new)
    }

    pub fn value(self) -> f64 {
        f64::from_bits(self.bits)
    }
}

impl fmt::Display for FloatLiteral {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.value())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryExpression {
    pub storage: BinaryExpressionStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryExpressionStorage {
    pub left: Expression,
    pub operator: BinaryOperator,
    pub right: Expression,
}

impl Default for BinaryExpressionStorage {
    fn default() -> Self {
        Self {
            left: Expression::Integer(0),
            operator: BinaryOperator::Add,
            right: Expression::Integer(0),
        }
    }
}

impl Deref for BinaryExpression {
    type Target = BinaryExpressionStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for BinaryExpression {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastExpression {
    pub storage: CastExpressionStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CastExpressionStorage {
    pub value: Expression,
    pub target_type: NamePath,
}

impl Deref for CastExpression {
    type Target = CastExpressionStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for CastExpression {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedExpression {
    pub storage: IndexedExpressionStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexedExpressionStorage {
    pub collection: Expression,
    pub index: Expression,
}

impl Deref for IndexedExpression {
    type Target = IndexedExpressionStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for IndexedExpression {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteral {
    pub storage: StructLiteralStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StructLiteralStorage {
    pub type_name: DiagnosticName,
    pub fields: Box<[StructLiteralField]>,
}

impl Deref for StructLiteral {
    type Target = StructLiteralStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for StructLiteral {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteralField {
    pub name: DiagnosticName,
    pub value: Expression,
}

impl Expression {
    pub fn display_name(&self) -> String {
        match self {
            Expression::ArrayLiteral(array_literal) => {
                bracketed_display_names(array_literal.values.iter(), Expression::display_name)
            }
            Expression::Binary(binary) => binary.display_name(),
            Expression::Boolean(value) => value.to_string(),
            Expression::Cast(cast) => cast.display_name(),
            Expression::Call(call) => call.display_name(),
            Expression::Float(value) => value.to_string(),
            Expression::Indexed(indexed) => {
                format!(
                    "{}[{}]",
                    indexed.collection.display_name(),
                    indexed.index.display_name()
                )
            }
            Expression::Integer(value) => value.to_string(),
            Expression::Member(member) => {
                format!("{}.{}", member.receiver.display_name(), member.member)
            }
            Expression::Mutable(expression) => format!("mut {}", expression.display_name()),
            Expression::Name(path) => display_name_path(path, "::"),
            Expression::StructLiteral(struct_literal) => struct_literal.type_name.to_string(),
            Expression::String(value) => format!("{:?}", value.as_str()),
        }
    }
}

impl ExpressionNode {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        match self {
            Self::ArrayLiteral(values) => {
                bracketed_display_names(table.expression_handles(*values).iter(), |value| {
                    table.display_name(*value)
                })
            }
            Self::Binary(binary) => binary.display_name(table),
            Self::Boolean(value) => value.to_string(),
            Self::Cast(cast) => cast.display_name(table),
            Self::Call(call) => call.display_name(table),
            Self::Float(value) => value.to_string(),
            Self::Indexed(indexed) => {
                format!(
                    "{}[{}]",
                    table.display_name(indexed.collection),
                    table.display_name(indexed.index)
                )
            }
            Self::Integer(value) => value.to_string(),
            Self::Member(member) => {
                format!("{}.{}", table.display_name(member.receiver), member.member)
            }
            Self::Mutable(expression) => format!("mut {}", table.display_name(*expression)),
            Self::Name(path) => display_name_path(table.name_path_members(path.members), "::"),
            Self::StructLiteral(struct_literal) => struct_literal.type_name.to_string(),
            Self::String(value) => format!("{:?}", value.as_str()),
        }
    }
}

pub fn display_name_path(path: &[DiagnosticName], separator: &str) -> String {
    let byte_count = path.iter().map(|name| name.as_str().len()).sum::<usize>()
        + separator.len().saturating_mul(path.len().saturating_sub(1));
    let mut display_name = String::with_capacity(byte_count);

    for (index, name) in path.iter().enumerate() {
        if index > 0 {
            display_name.push_str(separator);
        }

        display_name.push_str(name.as_str());
    }

    display_name
}

impl BinaryExpression {
    pub fn display_name(&self) -> String {
        format!(
            "{} {} {}",
            self.left.display_name(),
            self.operator.display_name(),
            self.right.display_name()
        )
    }
}

impl TableBinaryExpression {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        format!(
            "{} {} {}",
            table.display_name(self.left),
            self.operator.display_name(),
            table.display_name(self.right)
        )
    }
}

impl CastExpression {
    pub fn display_name(&self) -> String {
        format!(
            "{} as {}",
            self.value.display_name(),
            display_name_path(&self.target_type, "::")
        )
    }
}

impl TableCastExpression {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        let target_type = display_name_path(table.name_path_members(self.target_type), "::");
        format!("{} as {}", table.display_name(self.value), target_type)
    }
}

impl CallExpression {
    pub fn display_name(&self) -> String {
        let arguments = comma_join_display_names(&self.arguments, Expression::display_name);

        if let Some(receiver) = &self.receiver {
            format!("{}.{}({arguments})", receiver.display_name(), self.target)
        } else {
            format!("{}({arguments})", self.target)
        }
    }
}

impl TableCallExpression {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        let arguments =
            comma_join_display_names(table.expression_handles(self.arguments), |argument| {
                table.display_name(*argument)
            });

        if self.receiver.is_valid() {
            format!(
                "{}.{}({arguments})",
                table.display_name(self.receiver),
                self.target
            )
        } else {
            format!("{}({arguments})", self.target)
        }
    }
}

impl BinaryOperator {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::And => "&&",
            Self::Divide => "/",
            Self::Equal => "==",
            Self::Greater => ">",
            Self::GreaterOrEqual => ">=",
            Self::Less => "<",
            Self::LessOrEqual => "<=",
            Self::Modulo => "%",
            Self::Multiply => "*",
            Self::NotEqual => "!=",
            Self::Or => "||",
            Self::ShiftLeft => "<<",
            Self::ShiftRight => ">>",
            Self::Subtract => "-",
        }
    }
}

fn bracketed_display_names<'item, I, T>(
    values: I,
    mut display_name: impl FnMut(&'item T) -> String,
) -> String
where
    I: IntoIterator<Item = &'item T>,
    T: 'item,
{
    let mut output = String::from("[");
    let mut first = true;

    for value in values {
        if first {
            first = false;
        } else {
            output.push_str(", ");
        }

        output.push_str(&display_name(value));
    }

    output.push(']');
    output
}

fn comma_join_display_names<'item, I, T>(
    values: I,
    mut display_name: impl FnMut(&'item T) -> String,
) -> String
where
    I: IntoIterator<Item = &'item T>,
    T: 'item,
{
    let mut output = String::new();
    let mut first = true;

    for value in values {
        if first {
            first = false;
        } else {
            output.push_str(", ");
        }

        output.push_str(&display_name(value));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::{
        BinaryExpression, BinaryExpressionStorage, BinaryOperator, Expression, ExpressionNode,
        ExpressionTable, NamePath, StructLiteral, StructLiteralField, StructLiteralStorage,
        TableBinaryExpression, TableNamePath,
    };
    use crate::name::DiagnosticName;
    use omega_core::source::SourceText;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn expression_table_stores_recursive_typed_expressions_as_handles() {
        let expression = Expression::Binary(Box::new(BinaryExpression {
            storage: BinaryExpressionStorage {
                left: Expression::Integer(1),
                operator: BinaryOperator::Add,
                right: Expression::Binary(Box::new(BinaryExpression {
                    storage: BinaryExpressionStorage {
                        left: Expression::Integer(2),
                        operator: BinaryOperator::Add,
                        right: Expression::Integer(3),
                    },
                })),
            },
        }));

        let mut table = ExpressionTable::new();
        let root = table.insert_tree(&expression);

        assert_eq!(table.expression_count(), 5);
        assert_eq!(table.display_name(root), "1 + 2 + 3");

        let ExpressionNode::Binary(TableBinaryExpression { left, right, .. }) =
            table.expression(root)
        else {
            panic!("root expression should be binary");
        };

        assert!(left.is_valid());
        assert!(right.is_valid());
    }

    #[test]
    fn expression_table_stores_name_paths_as_member_spans() {
        let expression = Expression::Name(NamePath::resolved_from_iter(
            [
                DiagnosticName::generated("player"),
                DiagnosticName::generated("inventory"),
            ],
            SymbolHandle::from_arena_index(1),
            SymbolHandle::from_arena_index(2),
        ));

        let mut table = ExpressionTable::new();
        let root = table.insert_tree(&expression);
        let ExpressionNode::Name(path) = table.expression(root) else {
            panic!("root expression should be a name path");
        };

        assert_eq!(path.members.count(), 2);
        assert_eq!(path.head_symbol, SymbolHandle::from_arena_index(1));
        assert_eq!(path.symbol, SymbolHandle::from_arena_index(2));
        assert_eq!(table.display_name(root), "player::inventory");
    }

    #[test]
    fn expression_table_copies_table_payloads_without_tree_roundtrip() {
        let room_symbol = SymbolHandle::from_arena_index(3);
        let field_symbol = SymbolHandle::from_arena_index(4);
        let expression = Expression::StructLiteral(StructLiteral {
            storage: StructLiteralStorage {
                type_name: DiagnosticName::generated("Room"),
                fields: vec![
                    StructLiteralField {
                        name: DiagnosticName::generated("name"),
                        value: Expression::String(SourceText::generated("Hall")),
                    },
                    StructLiteralField {
                        name: DiagnosticName::generated("open"),
                        value: Expression::Binary(Box::new(BinaryExpression {
                            storage: BinaryExpressionStorage {
                                left: Expression::Name(NamePath::resolved_from_iter(
                                    [DiagnosticName::generated("room")],
                                    room_symbol,
                                    room_symbol,
                                )),
                                operator: BinaryOperator::Equal,
                                right: Expression::Name(NamePath::resolved_from_iter(
                                    [
                                        DiagnosticName::generated("room"),
                                        DiagnosticName::generated("field"),
                                    ],
                                    room_symbol,
                                    field_symbol,
                                )),
                            },
                        })),
                    },
                ]
                .into_boxed_slice(),
            },
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
        }) = copied.expression(binary.right)
        else {
            panic!("copied binary rhs should keep its name path");
        };

        assert_eq!(*head_symbol, room_symbol);
        assert_eq!(*symbol, field_symbol);
        assert_eq!(copied.name_path_members(*members).len(), 2);
    }
}
