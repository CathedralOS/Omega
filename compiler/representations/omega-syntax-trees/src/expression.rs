use crate::identifier::Identifier;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::source::SourceText;

pub type ExpressionHandle = Handle<ExpressionNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionTable {
    expressions: Arena<ExpressionNode>,
    expression_handles: Arena<ExpressionHandle>,
    identifier_path_members: Arena<Identifier>,
    struct_fields: Arena<TableStructLiteralField>,
}

impl ExpressionTable {
    pub fn new() -> Self {
        Self {
            expressions: Arena::new(),
            expression_handles: Arena::new(),
            identifier_path_members: Arena::new(),
            struct_fields: Arena::new(),
        }
    }

    pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
        self.expressions.insert(expression)
    }

    pub fn append_expression_handle(
        &mut self,
        expression: ExpressionHandle,
    ) -> Handle<ExpressionHandle> {
        self.expression_handles.append(expression)
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

    pub fn append_struct_field(
        &mut self,
        field: TableStructLiteralField,
    ) -> Handle<TableStructLiteralField> {
        self.struct_fields.append(field)
    }

    pub fn append_identifier_path_member(&mut self, member: Identifier) -> Handle<Identifier> {
        self.identifier_path_members.append(member)
    }

    pub fn copy_identifier_path_prefix(
        &mut self,
        span: HandleSpan<Identifier>,
        count: usize,
    ) -> HandleSpan<Identifier> {
        let count = count.min(span.len());
        if count == 0 {
            return HandleSpan::empty();
        }

        self.identifier_path_members.copy_span_pair(
            HandleSpan::from_parts(span.start(), count as u32),
            HandleSpan::empty(),
        )
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

    pub fn identifier_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
        self.identifier_path_members.span_or_empty(span)
    }

    pub fn expression_count(&self) -> usize {
        self.expressions.len()
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
    Cast(TableCastExpression),
    Call(TableCallExpression),
    Float(SourceText),
    Indexed(TableIndexedExpression),
    Integer(i64),
    Membership(TableMembershipExpression),
    Member(TableMemberExpression),
    Mutable(ExpressionHandle),
    Name(HandleSpan<Identifier>),
    SelfValue,
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
    pub target_type: HandleSpan<Identifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableIndexedExpression {
    pub collection: ExpressionHandle,
    pub index: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableMembershipExpression {
    pub value: ExpressionHandle,
    pub domain: HandleSpan<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMemberExpression {
    pub receiver: ExpressionHandle,
    pub member: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCallExpression {
    pub receiver: ExpressionHandle,
    pub target: Identifier,
    pub arguments: HandleSpan<ExpressionHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStructLiteral {
    pub type_name: Identifier,
    pub fields: HandleSpan<TableStructLiteralField>,
}

impl Default for TableStructLiteral {
    fn default() -> Self {
        Self {
            type_name: Identifier::generated(""),
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
            name: Identifier::generated(""),
            value: ExpressionHandle::invalid(),
        }
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
            Self::Membership(membership) => {
                format!(
                    "{} in {}",
                    table.display_name(membership.value),
                    display_identifier_path(table.identifier_path_members(membership.domain), "::")
                )
            }
            Self::Member(member) => {
                format!("{}.{}", table.display_name(member.receiver), member.member)
            }
            Self::Mutable(expression) => format!("mut {}", table.display_name(*expression)),
            Self::Name(path) => display_identifier_path(table.identifier_path_members(*path), "::"),
            Self::SelfValue => "self".to_owned(),
            Self::StructLiteral(struct_literal) => struct_literal.type_name.to_string(),
            Self::String(value) => format!("{:?}", value.as_str()),
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

fn display_identifier_path(path: &[Identifier], separator: &str) -> String {
    let byte_count = path
        .iter()
        .map(|identifier| identifier.as_str().len())
        .sum::<usize>()
        + separator.len().saturating_mul(path.len().saturating_sub(1));
    let mut display_name = String::with_capacity(byte_count);

    for (index, identifier) in path.iter().enumerate() {
        if index > 0 {
            display_name.push_str(separator);
        }

        display_name.push_str(identifier.as_str());
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
        let target_type =
            display_identifier_path(table.identifier_path_members(self.target_type), "::");
        format!("{} as {}", table.display_name(self.value), target_type)
    }
}

impl TableCallExpression {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        let mut arguments = String::new();

        for (index, argument) in table.expression_handles(self.arguments).iter().enumerate() {
            if index > 0 {
                arguments.push_str(", ");
            }

            arguments.push_str(&table.display_name(*argument));
        }

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

#[cfg(test)]
mod tests {
    use super::{BinaryOperator, ExpressionNode, ExpressionTable, TableBinaryExpression};
    use crate::identifier::Identifier;
    use omega_core::arena::HandleSpan;

    #[test]
    fn expression_table_stores_recursive_expressions_as_handles() {
        let mut table = ExpressionTable::new();
        let one = table.insert(ExpressionNode::Integer(1));
        let two = table.insert(ExpressionNode::Integer(2));
        let three = table.insert(ExpressionNode::Integer(3));
        let nested = table.insert(ExpressionNode::Binary(TableBinaryExpression {
            left: two,
            operator: BinaryOperator::Add,
            right: three,
        }));
        let root = table.insert(ExpressionNode::Binary(TableBinaryExpression {
            left: one,
            operator: BinaryOperator::Add,
            right: nested,
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
    fn expression_table_stores_array_children_as_handle_spans() {
        let mut table = ExpressionTable::new();
        let one = table.insert(ExpressionNode::Integer(1));
        let two = table.insert(ExpressionNode::Integer(2));
        let three = table.insert(ExpressionNode::Integer(3));
        let values = table.insert_expression_handles([one, two, three]);
        let root = table.insert(ExpressionNode::ArrayLiteral(values));
        let ExpressionNode::ArrayLiteral(values) = table.expression(root) else {
            panic!("root expression should be array literal");
        };

        assert_eq!(values.count(), 3);
        assert_eq!(table.display_name(root), "[1, 2, 3]");
    }

    #[test]
    fn expression_table_stores_name_paths_as_member_spans() {
        let mut table = ExpressionTable::new();
        let first = table.append_identifier_path_member(Identifier::generated("player"));
        let _second = table.append_identifier_path_member(Identifier::generated("inventory"));
        let root = table.insert(ExpressionNode::Name(HandleSpan::from_parts(first, 2)));
        let ExpressionNode::Name(path) = table.expression(root) else {
            panic!("root expression should be a name path");
        };

        assert_eq!(path.count(), 2);
        assert_eq!(table.display_name(root), "player::inventory");
    }
}
