use crate::name::ProgramName;
use omega_core::arena::{Arena, Handle, HandleSpan};
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
    String(String),
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
    name_path_members: Arena<ProgramName>,
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

    pub fn insert_struct_fields(
        &mut self,
        fields: impl IntoIterator<Item = TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        self.spans.struct_fields.insert_many(fields)
    }

    pub fn copy_from(
        &mut self,
        source: &ExpressionTable,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        match source.expression(expression) {
            ExpressionNode::ArrayLiteral(values) => {
                let mut start = Handle::invalid();
                let mut count = 0u32;

                for value in source.expression_handles(*values) {
                    let value = self.copy_from(source, *value);
                    let handle = self.spans.expression_handles.append(value);
                    if count == 0 {
                        start = handle;
                    }
                    count = count
                        .checked_add(1)
                        .expect("expression handle span count overflow");
                }

                let values = if count == 0 {
                    HandleSpan::empty()
                } else {
                    HandleSpan::from_parts(start, count)
                };

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
                self.insert(ExpressionNode::Cast(TableCastExpression { value, target_type }))
            }
            ExpressionNode::Call(call) => {
                let receiver = call
                    .receiver
                    .is_valid()
                    .then(|| self.copy_from(source, call.receiver))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let arguments =
                    self.copy_expression_handles_from(source, call.arguments);
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
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for expression in expressions {
            let expression = self.copy_from(source, *expression);
            let handle = self.spans.expression_handles.append(expression);
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("expression handle span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn insert_name_path_members(&mut self, path: &NamePath) -> HandleSpan<ProgramName> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for member in path.members() {
            let handle = self.spans.name_path_members.append(member.clone());
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("name path member span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_name_path_members(
        &mut self,
        source: &ExpressionTable,
        members: HandleSpan<ProgramName>,
    ) -> HandleSpan<ProgramName> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for member in source.name_path_members(members) {
            let handle = self.spans.name_path_members.append(member.clone());
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("name path member span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_struct_literal_fields(
        &mut self,
        source: &ExpressionTable,
        fields: HandleSpan<TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for field in source.struct_fields(fields) {
            let value = self.copy_from(source, field.value);
            let handle = self.spans.struct_fields.append(TableStructLiteralField {
                name: field.name.clone(),
                value,
            });
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("struct field span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn insert_expression_handle_span_from_trees<'expression>(
        &mut self,
        expressions: impl IntoIterator<Item = &'expression Expression>,
    ) -> HandleSpan<ExpressionHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for expression in expressions {
            let expression = self.insert_tree(expression);
            let handle = self.spans.expression_handles.append(expression);
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("expression handle span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn insert_struct_field_span_from_tree(
        &mut self,
        fields: &[StructLiteralField],
    ) -> HandleSpan<TableStructLiteralField> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for field in fields {
            let value = self.insert_tree(&field.value);
            let handle = self.spans.struct_fields.append(TableStructLiteralField {
                name: field.name.clone(),
                value,
            });
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("struct field span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
        self.nodes.expressions.get(handle)
    }

    pub fn expression_handles(&self, span: HandleSpan<ExpressionHandle>) -> &[ExpressionHandle] {
        self.spans.expression_handles.span_or_empty(span)
    }

    pub fn struct_fields(
        &self,
        span: HandleSpan<TableStructLiteralField>,
    ) -> &[TableStructLiteralField] {
        self.spans.struct_fields.span_or_empty(span)
    }

    pub fn name_path_members(&self, span: HandleSpan<ProgramName>) -> &[ProgramName] {
        self.spans.name_path_members.span_or_empty(span)
    }

    pub fn copy_name_path_members_with_suffix(
        &mut self,
        members: HandleSpan<ProgramName>,
        suffix: ProgramName,
    ) -> HandleSpan<ProgramName> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

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
            let handle = self.spans.name_path_members.append(member);
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("name path member span count overflow");
        }

        let handle = self.spans.name_path_members.append(suffix);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("name path member span count overflow");

        HandleSpan::from_parts(start, count)
    }

    pub fn copy_name_path_members_with_member_suffix(
        &mut self,
        members: HandleSpan<ProgramName>,
        suffix_members: HandleSpan<ProgramName>,
        suffix_start_offset: u32,
    ) -> HandleSpan<ProgramName> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

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
            let handle = self.spans.name_path_members.append(member);
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("name path member span count overflow");
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
            let handle = self.spans.name_path_members.append(member);
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("name path member span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    pub fn insert_copy_with_member_suffix(
        &mut self,
        expression: ExpressionHandle,
        suffix_members: HandleSpan<ProgramName>,
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
                let suffix = self.name_path_members(suffix_members)
                    [usize::try_from(suffix_start_offset).expect("suffix offset overflow")..]
                    .to_vec();
                let expression = self.to_tree_with_place_suffix(expression, &suffix);
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
                self.insert(ExpressionNode::Cast(TableCastExpression { value, target_type }))
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
                    target_type: NamePath::unresolved(
                        self.name_path_members(cast.target_type).to_vec(),
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
            ExpressionNode::Name(path) => Expression::Name(NamePath::resolved(
                self.name_path_members(path.members).to_vec(),
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
        suffix: &[ProgramName],
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

    fn name_path_to_tree(&self, path: &TableNamePath) -> NamePath {
        NamePath::resolved(
            self.name_path_members(path.members).to_vec(),
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
        *last_segment = ProgramName::generated(format!("{last_segment}[{index}]"));
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
    String(String),
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
    pub target_type: HandleSpan<ProgramName>,
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
    pub member: ProgramName,
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
    pub member: ProgramName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallExpression {
    pub target_symbol: SymbolHandle,
    pub target: ProgramName,
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
    pub target: ProgramName,
    pub arguments: HandleSpan<ExpressionHandle>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableNamePath {
    pub members: HandleSpan<ProgramName>,
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStructLiteral {
    pub type_name: ProgramName,
    pub fields: HandleSpan<TableStructLiteralField>,
}

impl Default for TableStructLiteral {
    fn default() -> Self {
        Self {
            type_name: ProgramName::default(),
            fields: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStructLiteralField {
    pub name: ProgramName,
    pub value: ExpressionHandle,
}

impl Default for TableStructLiteralField {
    fn default() -> Self {
        Self {
            name: ProgramName::default(),
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
    members: Box<[ProgramName]>,
}

impl NamePath {
    pub fn unresolved(members: Vec<ProgramName>) -> Self {
        Self {
            storage: NamePathStorage {
                members: members.into_boxed_slice(),
            },
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }
    }

    pub fn resolved(
        members: Vec<ProgramName>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Self {
        Self {
            storage: NamePathStorage {
                members: members.into_boxed_slice(),
            },
            head_symbol,
            symbol,
        }
    }

    pub fn members(&self) -> &[ProgramName] {
        &self.storage.members
    }

    pub fn as_slice(&self) -> &[ProgramName] {
        self.members()
    }

    pub fn into_members(self) -> Vec<ProgramName> {
        self.storage.members.into_vec()
    }

    pub fn push(&mut self, member: ProgramName) {
        let mut members = self.storage.members.to_vec();
        members.push(member);
        self.storage.members = members.into_boxed_slice();
        self.symbol = SymbolHandle::invalid();
    }

    pub fn extend_from_slice(&mut self, members: &[ProgramName]) {
        let mut owned_members = self.storage.members.to_vec();
        owned_members.extend_from_slice(members);
        self.storage.members = owned_members.into_boxed_slice();
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
        self
    }
}

impl From<Vec<ProgramName>> for NamePath {
    fn from(members: Vec<ProgramName>) -> Self {
        Self::unresolved(members)
    }
}

impl Deref for NamePath {
    type Target = [ProgramName];

    fn deref(&self) -> &Self::Target {
        self.members()
    }
}

impl DerefMut for NamePath {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.symbol = SymbolHandle::invalid();
        &mut self.storage.members
    }
}

impl<'path> IntoIterator for &'path NamePath {
    type Item = &'path ProgramName;
    type IntoIter = std::slice::Iter<'path, ProgramName>;

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
    pub type_name: ProgramName,
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
    pub name: ProgramName,
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
            Expression::String(value) => format!("{value:?}"),
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
            Self::String(value) => format!("{value:?}"),
        }
    }
}

pub fn display_name_path(path: &[ProgramName], separator: &str) -> String {
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
        format!("{} as {}", self.value.display_name(), display_name_path(&self.target_type, "::"))
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
        let arguments = self
            .arguments
            .iter()
            .map(Expression::display_name)
            .collect::<Vec<_>>()
            .join(", ");

        if let Some(receiver) = &self.receiver {
            format!("{}.{}({arguments})", receiver.display_name(), self.target)
        } else {
            format!("{}({arguments})", self.target)
        }
    }
}

impl TableCallExpression {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        let arguments = table
            .expression_handles(self.arguments)
            .iter()
            .map(|argument| table.display_name(*argument))
            .collect::<Vec<_>>()
            .join(", ");

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

#[cfg(test)]
mod tests {
    use super::{
        BinaryExpression, BinaryExpressionStorage, BinaryOperator, Expression, ExpressionNode,
        ExpressionTable, NamePath, StructLiteral, StructLiteralField, StructLiteralStorage,
        TableBinaryExpression, TableNamePath,
    };
    use crate::name::ProgramName;
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
        let expression = Expression::Name(NamePath::resolved(
            vec![
                ProgramName::generated("player"),
                ProgramName::generated("inventory"),
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
                type_name: ProgramName::generated("Room"),
                fields: vec![
                    StructLiteralField {
                        name: ProgramName::generated("name"),
                        value: Expression::String("Hall".to_string()),
                    },
                    StructLiteralField {
                        name: ProgramName::generated("open"),
                        value: Expression::Binary(Box::new(BinaryExpression {
                            storage: BinaryExpressionStorage {
                                left: Expression::Name(NamePath::resolved(
                                    vec![ProgramName::generated("room")],
                                    room_symbol,
                                    room_symbol,
                                )),
                                operator: BinaryOperator::Equal,
                                right: Expression::Name(NamePath::resolved(
                                    vec![
                                        ProgramName::generated("room"),
                                        ProgramName::generated("field"),
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
