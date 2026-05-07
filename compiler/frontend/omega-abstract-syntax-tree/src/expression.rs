#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    ArrayLiteral(Vec<Expression>),
    Binary(Box<BinaryExpression>),
    Boolean(bool),
    Float(String),
    Indexed(Box<IndexedExpression>),
    Integer(i64),
    Mutable(Box<Expression>),
    Name(Vec<String>),
    StructLiteral(StructLiteral),
    String(String),
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
    pub type_name: String,
    pub fields: Vec<StructLiteralField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteralField {
    pub name: String,
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
            Expression::Name(path) => path.join("::"),
            Expression::StructLiteral(struct_literal) => struct_literal.type_name.clone(),
            Expression::String(value) => format!("{value:?}"),
        }
    }
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
