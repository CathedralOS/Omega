use crate::name::ProgramName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    ArrayLiteral(Vec<Expression>),
    Binary(Box<BinaryExpression>),
    Boolean(bool),
    Float(String),
    Indexed(Box<IndexedExpression>),
    Integer(i64),
    Mutable(Box<Expression>),
    Name(Vec<ProgramName>),
    StructLiteral(StructLiteral),
    String(String),
}

impl Default for Expression {
    fn default() -> Self {
        Self::Integer(0)
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
                format!(
                    "[{}]",
                    values
                        .iter()
                        .map(Expression::display_name)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Expression::Binary(binary) => binary.display_name(),
            Expression::Boolean(value) => value.to_string(),
            Expression::Float(value) => value.clone(),
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
