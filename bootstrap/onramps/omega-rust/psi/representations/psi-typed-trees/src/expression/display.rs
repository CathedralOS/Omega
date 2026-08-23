use crate::expression::{
    BinaryExpression, BinaryOperator, CallExpression, CastExpression, Expression, ExpressionNode,
    ExpressionTable, RangeExpression, StaticMachineArgument, TableBinaryExpression,
    TableCallExpression, TableCastExpression, TableUnaryExpression, UnaryExpression, UnaryOperator,
};
use crate::name::Identifier;

impl Expression {
    pub fn display_name(&self) -> String {
        match self {
            Expression::ArrayLiteral(values) => {
                bracketed_display_names(values.iter(), Expression::display_name)
            }
            Expression::Atomic(atomic) => format!(
                "atomic[{:?}]({})",
                atomic.ordering,
                atomic.value.display_name()
            ),
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
            Expression::Borrow(expression) => format!(
                "{}{}",
                borrow_access_prefix(expression.access),
                expression.target.display_name()
            ),
            Expression::Name(path) => display_name_path(path, "::"),
            Expression::Range(range) => range.display_name(),
            Expression::StructLiteral(struct_literal) => struct_literal.type_name.to_string(),
            Expression::String(value) => psi_source::display_literal_bytes(value),
            Expression::Unary(unary) => unary.display_name(),
            Expression::ZeroValue(_) => "zero_value<type>()".to_owned(),
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
            Self::Atomic(atomic) => format!(
                "atomic[{:?}]({})",
                atomic.ordering,
                table.display_name(atomic.value)
            ),
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
            Self::Borrow(expression) => format!(
                "{}{}",
                borrow_access_prefix(expression.access),
                table.display_name(expression.target)
            ),
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
            Self::String(value) => psi_source::display_literal_bytes(value),
            Self::Unary(unary) => unary.display_name(table),
            Self::ZeroValue(_) => "zero_value<type>()".to_owned(),
        }
    }
}

fn borrow_access_prefix(access: psi_language_core::ReferenceAccess) -> &'static str {
    match access {
        psi_language_core::ReferenceAccess::Mutable => "&mut ",
        psi_language_core::ReferenceAccess::WriteOnly => "&write ",
        psi_language_core::ReferenceAccess::Shared => "&",
    }
}

impl RangeExpression {
    pub fn display_name(&self) -> String {
        let separator = if self.end_inclusive { "..=" } else { ".." };
        match (&self.start, &self.end) {
            (Some(start), Some(end)) => {
                format!(
                    "{}{}{}",
                    start.display_name(),
                    separator,
                    end.display_name()
                )
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

impl TableUnaryExpression {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        format!(
            "{}{}",
            self.operator.display_name(),
            table.display_name(self.operand)
        )
    }
}

impl UnaryExpression {
    pub fn display_name(&self) -> String {
        format!(
            "{}{}",
            self.operator.display_name(),
            self.operand.display_name()
        )
    }
}

impl CastExpression {
    pub fn display_name(&self) -> String {
        format!(
            "{} as {}",
            self.value.display_name(),
            display_name_path(&self.target_label, "::")
        )
    }
}

impl TableCastExpression {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        let target_type = display_name_path(table.name_path_members(self.target_label), "::");
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
        let machine_arguments = if self.machine_arguments.is_empty() {
            String::new()
        } else {
            format!(
                "<{}>",
                self.machine_arguments
                    .iter()
                    .map(StaticMachineArgument::display_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let arguments =
            comma_join_display_names(table.expression_handles(self.arguments), |argument| {
                table.display_name(*argument)
            });

        if self.receiver.is_valid() {
            format!(
                "{}.{}{machine_arguments}({arguments})",
                table.display_name(self.receiver),
                self.target
            )
        } else {
            format!("{}{machine_arguments}({arguments})", self.target)
        }
    }
}

impl StaticMachineArgument {
    pub fn display_name(&self) -> String {
        if let Some(literal) = &self.const_literal {
            return literal.text().to_owned();
        }
        if let Some(projection) = &self.evidence_projection {
            return format!("{}.{}", projection.term, projection.member);
        }
        let mut rendered = display_name_path(&self.path, "::");
        if let Some(application) = &self.application {
            let mut arguments = application
                .lifetime_arguments
                .iter()
                .map(|lifetime| format!("'{lifetime}"))
                .collect::<Vec<_>>();
            arguments.extend(application.arguments.iter().map(Self::display_name));
            rendered.push('<');
            rendered.push_str(&arguments.join(", "));
            rendered.push('>');
        }
        rendered
    }
}

impl BinaryOperator {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::And => "&&",
            Self::BitwiseAnd => "&",
            Self::BitwiseOr => "|",
            Self::BitwiseXor => "^",
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

impl UnaryOperator {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::BitwiseNot => "~",
            Self::LogicalNot => "!",
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
