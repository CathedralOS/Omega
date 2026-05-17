use crate::name::DiagnosticName;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::source::SourceText;
use omega_core::symbols::SymbolHandle;
use std::fmt;

pub type ExpressionHandle = Handle<ExpressionNode>;

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

    pub fn reserve_name_path_members(&mut self, count: u32) -> HandleSpan<DiagnosticName> {
        self.spans.name_path_members.insert_many(
            std::iter::repeat_with(DiagnosticName::default)
                .take(usize::try_from(count).expect("name path member span count overflow")),
        )
    }

    pub fn set_name_path_member_at_offset(
        &mut self,
        members: HandleSpan<DiagnosticName>,
        offset: u32,
        member: DiagnosticName,
    ) {
        *self.spans.name_path_members.get_mut(Handle::from_parts(
            members
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("name path member index overflow"),
            members.start().generation(),
        )) = member;
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
                    is_self_value: path.is_self_value,
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

    fn copy_name_path_members(
        &mut self,
        source: &ExpressionTable,
        members: HandleSpan<DiagnosticName>,
    ) -> HandleSpan<DiagnosticName> {
        self.spans
            .name_path_members
            .insert_many(source.name_path_members(members).iter().cloned())
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

    pub fn name_path_member_at_offset(
        &self,
        members: HandleSpan<DiagnosticName>,
        offset: u32,
    ) -> &DiagnosticName {
        self.spans.name_path_members.get(Handle::from_parts(
            members
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("name path member index overflow"),
            members.start().generation(),
        ))
    }

    pub fn copy_name_path_members_with_suffix(
        &mut self,
        members: HandleSpan<DiagnosticName>,
        suffix: DiagnosticName,
    ) -> HandleSpan<DiagnosticName> {
        let span = self.reserve_name_path_members(
            members
                .count()
                .checked_add(1)
                .expect("name path member span count overflow"),
        );

        for offset in 0..members.count() {
            let member = self.name_path_member_at_offset(members, offset).clone();
            self.set_name_path_member_at_offset(span, offset, member);
        }

        self.set_name_path_member_at_offset(span, members.count(), suffix);

        span
    }

    pub fn copy_name_path_members_with_member_suffix(
        &mut self,
        members: HandleSpan<DiagnosticName>,
        suffix_members: HandleSpan<DiagnosticName>,
        suffix_start_offset: u32,
    ) -> HandleSpan<DiagnosticName> {
        let suffix_count = suffix_members.count().saturating_sub(suffix_start_offset);
        let span = self.reserve_name_path_members(
            members
                .count()
                .checked_add(suffix_count)
                .expect("name path member span count overflow"),
        );

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
            self.set_name_path_member_at_offset(span, offset, member);
        }

        for (target_offset, offset) in (suffix_start_offset..suffix_members.count()).enumerate() {
            let member = self
                .name_path_member_at_offset(suffix_members, offset)
                .clone();
            self.set_name_path_member_at_offset(
                span,
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
                    is_self_value: path.is_self_value,
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
            ExpressionNode::Indexed(indexed) => self
                .insert_indexed_expression_path_with_member_suffix(
                    &indexed,
                    suffix_members,
                    suffix_start_offset,
                )
                .unwrap_or_else(|| self.copy_from_self(expression)),
            _ => self.copy_from_self(expression),
        }
    }

    pub fn copy_from_self(&mut self, expression: ExpressionHandle) -> ExpressionHandle {
        match self.expression(expression).clone() {
            ExpressionNode::ArrayLiteral(values) => {
                let copied_values = self.reserve_expression_handles(values.count());

                for offset in 0..values.count() {
                    let value = *self.expression_handle_at_offset(values, offset);
                    let value = self.copy_from_self(value);
                    self.set_expression_handle_at_offset(copied_values, offset, value);
                }

                self.insert(ExpressionNode::ArrayLiteral(copied_values))
            }
            ExpressionNode::Binary(binary) => {
                let left = self.copy_from_self(binary.left);
                let right = self.copy_from_self(binary.right);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(value)),
            ExpressionNode::Cast(cast) => {
                let value = self.copy_from_self(cast.value);
                let target_type = self.copy_name_path_members_from_self(cast.target_type);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
                }))
            }
            ExpressionNode::Call(call) => {
                let receiver = call
                    .receiver
                    .is_valid()
                    .then(|| self.copy_from_self(call.receiver))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let arguments = self.copy_expression_handles_from_self(call.arguments);
                self.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target,
                    arguments,
                }))
            }
            ExpressionNode::Float(value) => self.insert(ExpressionNode::Float(value)),
            ExpressionNode::Indexed(indexed) => {
                let collection = self.copy_from_self(indexed.collection);
                let index = self.copy_from_self(indexed.index);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value)),
            ExpressionNode::Member(member) => {
                let receiver = self.copy_from_self(member.receiver);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                }))
            }
            ExpressionNode::Mutable(inner_expression) => {
                let inner_expression = self.copy_from_self(inner_expression);
                self.insert(ExpressionNode::Mutable(inner_expression))
            }
            ExpressionNode::Name(path) => {
                let members = self.copy_name_path_members_from_self(path.members);
                self.insert(ExpressionNode::Name(TableNamePath { members, ..path }))
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                let fields = self.copy_struct_literal_fields_from_self(struct_literal.fields);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name,
                    fields,
                }))
            }
            ExpressionNode::String(value) => self.insert(ExpressionNode::String(value)),
        }
    }

    fn copy_expression_handles_from_self(
        &mut self,
        expressions: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        let copied = self.reserve_expression_handles(expressions.count());

        for offset in 0..expressions.count() {
            let expression = *self.expression_handle_at_offset(expressions, offset);
            let expression = self.copy_from_self(expression);
            self.set_expression_handle_at_offset(copied, offset, expression);
        }

        copied
    }

    fn copy_name_path_members_from_self(
        &mut self,
        members: HandleSpan<DiagnosticName>,
    ) -> HandleSpan<DiagnosticName> {
        let copied = self.reserve_name_path_members(members.count());

        for offset in 0..members.count() {
            let member = self.name_path_member_at_offset(members, offset).clone();
            self.set_name_path_member_at_offset(copied, offset, member);
        }

        copied
    }

    fn copy_struct_literal_fields_from_self(
        &mut self,
        fields: HandleSpan<TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        let copied = self.reserve_struct_fields(fields.count());

        for offset in 0..fields.count() {
            let field = self.struct_field_at_offset(fields, offset).clone();
            let value = self.copy_from_self(field.value);
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

    fn insert_indexed_expression_path_with_member_suffix(
        &mut self,
        indexed: &TableIndexedExpression,
        suffix_members: HandleSpan<DiagnosticName>,
        suffix_start_offset: u32,
    ) -> Option<ExpressionHandle> {
        let path_len = self.storage_path_len(indexed.collection)?;
        let suffix_count = suffix_members.count().saturating_sub(suffix_start_offset);
        let members = self.reserve_name_path_members(
            path_len
                .checked_add(suffix_count)
                .expect("name path member span count overflow"),
        );
        self.fill_storage_path_members(indexed.collection, members, 0)?;

        for (target_offset, offset) in (suffix_start_offset..suffix_members.count()).enumerate() {
            let member = self
                .name_path_member_at_offset(suffix_members, offset)
                .clone();
            self.set_name_path_member_at_offset(
                members,
                path_len
                    .checked_add(
                        target_offset
                            .try_into()
                            .expect("name path member span count overflow"),
                    )
                    .expect("name path member span count overflow"),
                member,
            );
        }

        Some(self.insert(ExpressionNode::Name(TableNamePath {
            members,
            is_self_value: false,
            head_symbol: self.storage_path_head_symbol(indexed.collection),
            symbol: SymbolHandle::invalid(),
        })))
    }

    fn storage_path_len(&self, expression: ExpressionHandle) -> Option<u32> {
        match self.expression(expression) {
            ExpressionNode::Name(path) => Some(path.members.count()),
            ExpressionNode::Indexed(indexed) => self.storage_path_len(indexed.collection),
            ExpressionNode::Mutable(target) => self.storage_path_len(*target),
            _ => None,
        }
    }

    fn storage_path_head_symbol(&self, expression: ExpressionHandle) -> SymbolHandle {
        match self.expression(expression) {
            ExpressionNode::Name(path) => path.head_symbol,
            ExpressionNode::Indexed(indexed) => self.storage_path_head_symbol(indexed.collection),
            ExpressionNode::Mutable(target) => self.storage_path_head_symbol(*target),
            _ => SymbolHandle::invalid(),
        }
    }

    fn fill_storage_path_members(
        &mut self,
        expression: ExpressionHandle,
        members: HandleSpan<DiagnosticName>,
        offset: u32,
    ) -> Option<()> {
        match self.expression(expression).clone() {
            ExpressionNode::Name(path) => {
                for source_offset in 0..path.members.count() {
                    let member = self
                        .name_path_member_at_offset(path.members, source_offset)
                        .clone();
                    self.set_name_path_member_at_offset(
                        members,
                        offset
                            .checked_add(source_offset)
                            .expect("name path member span count overflow"),
                        member,
                    );
                }
                Some(())
            }
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) = self.expression(indexed.index) else {
                    return None;
                };
                let index = *index;
                self.fill_storage_path_members(indexed.collection, members, offset)?;
                let path_len = self.storage_path_len(indexed.collection)?;
                let last_offset = offset
                    .checked_add(path_len.checked_sub(1)?)
                    .expect("name path member span count overflow");
                let last_member = self.name_path_member_at_offset(members, last_offset);
                let indexed_member = DiagnosticName::generated(format!("{last_member}[{index}]"));
                self.set_name_path_member_at_offset(members, last_offset, indexed_member);
                Some(())
            }
            ExpressionNode::Mutable(target) => {
                self.fill_storage_path_members(target, members, offset)
            }
            _ => None,
        }
    }

    pub fn expression_count(&self) -> usize {
        self.nodes.expressions.len()
    }

    pub fn struct_field_count(&self) -> usize {
        self.spans.struct_fields.len()
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
pub struct TableMemberExpression {
    pub receiver: ExpressionHandle,
    pub member_symbol: SymbolHandle,
    pub member: DiagnosticName,
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
    pub is_self_value: bool,
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

impl TableCastExpression {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        let target_type = display_name_path(table.name_path_members(self.target_type), "::");
        format!("{} as {}", table.display_name(self.value), target_type)
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
        BinaryOperator, ExpressionNode, ExpressionTable, TableBinaryExpression, TableNamePath,
        TableStructLiteral, TableStructLiteralField,
    };
    use crate::name::DiagnosticName;
    use omega_core::source::SourceText;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn expression_table_stores_nested_expressions_as_handles() {
        let mut table = ExpressionTable::new();
        let one = table.insert(ExpressionNode::Integer(1));
        let two = table.insert(ExpressionNode::Integer(2));
        let three = table.insert(ExpressionNode::Integer(3));
        let right = table.insert(ExpressionNode::Binary(TableBinaryExpression {
            left: two,
            operator: BinaryOperator::Add,
            right: three,
        }));
        let root = table.insert(ExpressionNode::Binary(TableBinaryExpression {
            left: one,
            operator: BinaryOperator::Add,
            right,
        }));

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
        let mut table = ExpressionTable::new();
        let members = table.reserve_name_path_members(2);
        table.set_name_path_member_at_offset(members, 0, DiagnosticName::generated("player"));
        table.set_name_path_member_at_offset(members, 1, DiagnosticName::generated("inventory"));
        let root = table.insert(ExpressionNode::Name(TableNamePath {
            members,
            is_self_value: false,
            head_symbol: SymbolHandle::from_arena_index(1),
            symbol: SymbolHandle::from_arena_index(2),
        }));
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

        let mut source = ExpressionTable::new();
        let room_members = source.reserve_name_path_members(1);
        source.set_name_path_member_at_offset(room_members, 0, DiagnosticName::generated("room"));
        let room = source.insert(ExpressionNode::Name(TableNamePath {
            members: room_members,
            is_self_value: false,
            head_symbol: room_symbol,
            symbol: room_symbol,
        }));

        let field_members = source.reserve_name_path_members(2);
        source.set_name_path_member_at_offset(field_members, 0, DiagnosticName::generated("room"));
        source.set_name_path_member_at_offset(field_members, 1, DiagnosticName::generated("field"));
        let field = source.insert(ExpressionNode::Name(TableNamePath {
            members: field_members,
            is_self_value: false,
            head_symbol: room_symbol,
            symbol: field_symbol,
        }));

        let hall = source.insert(ExpressionNode::String(SourceText::generated("Hall")));
        let open = source.insert(ExpressionNode::Binary(TableBinaryExpression {
            left: room,
            operator: BinaryOperator::Equal,
            right: field,
        }));
        let fields = source.insert_struct_fields([
            TableStructLiteralField {
                name: DiagnosticName::generated("name"),
                value: hall,
            },
            TableStructLiteralField {
                name: DiagnosticName::generated("open"),
                value: open,
            },
        ]);
        let root = source.insert(ExpressionNode::StructLiteral(TableStructLiteral {
            type_name: DiagnosticName::generated("Room"),
            fields,
        }));

        let mut copied = ExpressionTable::new();
        let copied_root = copied.copy_from(&source, root);

        assert_eq!(source.display_name(root), copied.display_name(copied_root));

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
}
