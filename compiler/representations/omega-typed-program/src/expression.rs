use crate::name::ProgramName;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use std::fmt;
use std::ops::{Deref, DerefMut};

pub type ExpressionHandle = Handle<ExpressionNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    ArrayLiteral(Vec<Expression>),
    Binary(Box<BinaryExpression>),
    Boolean(bool),
    Float(FloatLiteral),
    Indexed(Box<IndexedExpression>),
    Integer(i64),
    Mutable(Box<Expression>),
    Name(NamePath),
    StructLiteral(StructLiteral),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionTable {
    expressions: Arena<ExpressionNode>,
    expression_handles: Arena<ExpressionHandle>,
    name_path_members: Arena<ProgramName>,
    struct_fields: Arena<TableStructLiteralField>,
}

impl ExpressionTable {
    pub fn new() -> Self {
        Self {
            expressions: Arena::new(),
            expression_handles: Arena::new(),
            name_path_members: Arena::new(),
            struct_fields: Arena::new(),
        }
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

    pub fn insert_struct_fields(
        &mut self,
        fields: impl IntoIterator<Item = TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        self.struct_fields.insert_many(fields)
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
                    let handle = self.expression_handles.append(value);
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
            let handle = self.expression_handles.append(expression);
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
            let handle = self.name_path_members.append(member.clone());
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
            let handle = self.name_path_members.append(member.clone());
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
            let handle = self.struct_fields.append(TableStructLiteralField {
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
            let handle = self.expression_handles.append(expression);
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
            let handle = self.struct_fields.append(TableStructLiteralField {
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
        self.expressions.get(handle)
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

    pub fn name_path_members(&self, span: HandleSpan<ProgramName>) -> &[ProgramName] {
        self.name_path_members.span_or_empty(span)
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
            let handle = self.name_path_members.append(member);
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("name path member span count overflow");
        }

        let handle = self.name_path_members.append(suffix);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("name path member span count overflow");

        HandleSpan::from_parts(start, count)
    }

    pub fn expression_count(&self) -> usize {
        self.expressions.len()
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
            ExpressionNode::ArrayLiteral(values) => Expression::ArrayLiteral(
                self.expression_handles(*values)
                    .iter()
                    .map(|value| self.to_tree(*value))
                    .collect(),
            ),
            ExpressionNode::Binary(binary) => Expression::Binary(Box::new(BinaryExpression {
                left: self.to_tree(binary.left),
                operator: binary.operator,
                right: self.to_tree(binary.right),
            })),
            ExpressionNode::Boolean(value) => Expression::Boolean(*value),
            ExpressionNode::Float(value) => Expression::Float(*value),
            ExpressionNode::Indexed(indexed) => Expression::Indexed(Box::new(IndexedExpression {
                collection: self.to_tree(indexed.collection),
                index: self.to_tree(indexed.index),
            })),
            ExpressionNode::Integer(value) => Expression::Integer(*value),
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
                    type_name: struct_literal.type_name.clone(),
                    fields: self
                        .struct_fields(struct_literal.fields)
                        .iter()
                        .map(|field| StructLiteralField {
                            name: field.name.clone(),
                            value: self.to_tree(field.value),
                        })
                        .collect(),
                })
            }
            ExpressionNode::String(value) => Expression::String(value.clone()),
        }
    }

    pub fn display_name(&self, handle: ExpressionHandle) -> String {
        self.expression(handle).display_name(self)
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
    Float(FloatLiteral),
    Indexed(TableIndexedExpression),
    Integer(i64),
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
pub struct TableIndexedExpression {
    pub collection: ExpressionHandle,
    pub index: ExpressionHandle,
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
    members: Vec<ProgramName>,
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
}

impl NamePath {
    pub fn unresolved(members: Vec<ProgramName>) -> Self {
        Self {
            members,
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
            members,
            head_symbol,
            symbol,
        }
    }

    pub fn members(&self) -> &[ProgramName] {
        &self.members
    }

    pub fn as_slice(&self) -> &[ProgramName] {
        self.members()
    }

    pub fn into_members(self) -> Vec<ProgramName> {
        self.members
    }

    pub fn push(&mut self, member: ProgramName) {
        self.members.push(member);
        self.symbol = SymbolHandle::invalid();
    }

    pub fn extend_from_slice(&mut self, members: &[ProgramName]) {
        self.members.extend_from_slice(members);
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
        &mut self.members
    }
}

impl<'path> IntoIterator for &'path NamePath {
    type Item = &'path ProgramName;
    type IntoIter = std::slice::Iter<'path, ProgramName>;

    fn into_iter(self) -> Self::IntoIter {
        self.members.iter()
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
    pub left: Expression,
    pub operator: BinaryOperator,
    pub right: Expression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    And,
    Equal,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    NotEqual,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedExpression {
    pub collection: Expression,
    pub index: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteral {
    pub type_name: ProgramName,
    pub fields: Vec<StructLiteralField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteralField {
    pub name: ProgramName,
    pub value: Expression,
}

impl Expression {
    pub fn display_name(&self) -> String {
        match self {
            Expression::ArrayLiteral(values) => {
                bracketed_display_names(values.iter(), Expression::display_name)
            }
            Expression::Binary(binary) => binary.display_name(),
            Expression::Boolean(value) => value.to_string(),
            Expression::Float(value) => value.to_string(),
            Expression::Indexed(indexed) => {
                format!(
                    "{}[{}]",
                    indexed.collection.display_name(),
                    indexed.index.display_name()
                )
            }
            Expression::Integer(value) => value.to_string(),
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
            Self::Float(value) => value.to_string(),
            Self::Indexed(indexed) => {
                format!(
                    "{}[{}]",
                    table.display_name(indexed.collection),
                    table.display_name(indexed.index)
                )
            }
            Self::Integer(value) => value.to_string(),
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

impl BinaryOperator {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::And => "&&",
            Self::Equal => "==",
            Self::Greater => ">",
            Self::GreaterOrEqual => ">=",
            Self::Less => "<",
            Self::LessOrEqual => "<=",
            Self::NotEqual => "!=",
            Self::Or => "||",
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
        BinaryExpression, BinaryOperator, Expression, ExpressionNode, ExpressionTable, NamePath,
        StructLiteral, StructLiteralField, TableBinaryExpression, TableNamePath,
    };
    use crate::name::ProgramName;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn expression_table_stores_recursive_typed_expressions_as_handles() {
        let expression = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Integer(1),
            operator: BinaryOperator::Add,
            right: Expression::Binary(Box::new(BinaryExpression {
                left: Expression::Integer(2),
                operator: BinaryOperator::Add,
                right: Expression::Integer(3),
            })),
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
            type_name: ProgramName::generated("Room"),
            fields: vec![
                StructLiteralField {
                    name: ProgramName::generated("name"),
                    value: Expression::String("Hall".to_string()),
                },
                StructLiteralField {
                    name: ProgramName::generated("open"),
                    value: Expression::Binary(Box::new(BinaryExpression {
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
                    })),
                },
            ],
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
