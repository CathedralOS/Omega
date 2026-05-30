use std::fmt;

use crate::expression::{
    BinaryExpression, BinaryOperator, CallExpression, CastExpression, Expression, ExpressionNode,
    ExpressionTable, FloatLiteral, RangeExpression, TableBinaryExpression, TableCallExpression,
    TableCastExpression,
};
use crate::name::Identifier;

impl fmt::Display for FloatLiteral {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.value())
    }
}

impl Expression {
    pub fn display_name(&self) -> String {
        match self {
            Expression::ArrayLiteral(values) => {
                bracketed_display_names(values.iter(), Expression::display_name)
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
            Expression::Range(range) => range.display_name(),
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
            Self::Range(range) => match (range.start.is_valid(), range.end.is_valid()) {
                (true, true) => format!(
                    "{}..{}",
                    table.display_name(range.start),
                    table.display_name(range.end)
                ),
                (true, false) => format!("{}..", table.display_name(range.start)),
                (false, true) => format!("..{}", table.display_name(range.end)),
                (false, false) => "..".to_string(),
            },
            Self::StructLiteral(struct_literal) => struct_literal.type_name.to_string(),
            Self::String(value) => format!("{value:?}"),
        }
    }
}

impl RangeExpression {
    pub fn display_name(&self) -> String {
        let separator = if self.end_inclusive { "..=" } else { ".." };
        match (&self.start, &self.end) {
            (Some(start), Some(end)) => {
                format!("{}{}{}", start.display_name(), separator, end.display_name())
            }
            (Some(start), None) => format!("{}{}", start.display_name(), separator),
            (None, Some(end)) => format!("{}{}", separator, end.display_name()),
            (None, None) => separator.to_string(),
        }
    }
}

pub fn display_name_path(path: &[Identifier], separator: &str) -> String {
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
        let arguments = comma_join_display_names(&*self.arguments, Expression::display_name);

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
