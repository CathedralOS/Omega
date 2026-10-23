use diagnostics::bracketed_display_names;

use crate::expression::{
    BinaryOperator, ExpressionNode, StaticMachineArgument, TableBinaryExpression,
    TableCallExpression, TableCastExpression, TableUnaryExpression, UnaryOperator,
};
use crate::identifier::Identifier;

impl ExpressionNode {
    pub fn display_name(&self, table: &crate::SyntaxTrees) -> String {
        match self {
            Self::Match(dispatch) => {
                let arms = table
                    .expressions
                    .match_arms(dispatch.arms)
                    .iter()
                    .map(|arm| {
                        let pattern = match arm.pattern {
                            crate::expression::MatchPattern::Value(value) => {
                                table.display_expression_name(value)
                            }
                            crate::expression::MatchPattern::Wildcard => "_".to_owned(),
                        };
                        format!("{pattern} => {}", table.display_expression_name(arm.value))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "match {} {{ {arms} }}",
                    table.display_expression_name(dispatch.subject)
                )
            }
            Self::ArrayLiteral(values) => bracketed_display_names(
                table.expressions.expression_handles(*values).iter(),
                |value| table.display_expression_name(*value),
            ),
            Self::Atomic(atomic) => format!(
                "atomic[{:?}]({})",
                atomic.ordering,
                table.display_expression_name(atomic.value)
            ),
            Self::Binary(binary) => binary.display_name(table),
            Self::Boolean(value) => value.to_string(),
            Self::Cast(cast) => cast.display_name(table),
            Self::Call(call) => call.display_name(table),
            Self::Float(value) => value.as_str(table).to_owned(),
            Self::UndeterminedNaN(format) => format!("<undetermined NaN {}>", format.name()),
            Self::Indexed(indexed) => {
                format!(
                    "{}[{}]",
                    table.display_expression_name(indexed.collection),
                    table.display_expression_name(indexed.index)
                )
            }
            Self::Integer(value) => value.to_string(),
            Self::Membership(membership) => {
                format!(
                    "{} in {}",
                    table.display_expression_name(membership.value),
                    display_identifier_path(
                        table,
                        table.expressions.identifier_path_members(membership.domain),
                        "::"
                    )
                )
            }
            Self::Member(member) => {
                format!(
                    "{}.{}",
                    table.display_expression_name(member.receiver),
                    member.member.as_str(table)
                )
            }
            Self::Borrow(expression) => format!(
                "{}{}",
                expression.access.borrow_prefix(),
                table.display_expression_name(expression.target)
            ),
            Self::Name(path) => display_identifier_path(
                table,
                table.expressions.identifier_path_members(*path),
                "::",
            ),
            Self::Range(range) => {
                let separator = if range.end_inclusive { "..=" } else { ".." };
                match (range.start.is_valid(), range.end.is_valid()) {
                    (true, true) => format!(
                        "{}{}{}",
                        table.display_expression_name(range.start),
                        separator,
                        table.display_expression_name(range.end)
                    ),
                    (true, false) => format!(
                        "{}{}",
                        table.display_expression_name(range.start),
                        separator
                    ),
                    (false, true) => {
                        format!("{}{}", separator, table.display_expression_name(range.end))
                    }
                    (false, false) => separator.to_owned(),
                }
            }
            Self::StructLiteral(struct_literal) => {
                struct_literal.constructor_name.as_str(table).to_owned()
            }
            Self::String(value) => source::display_literal_bytes(value),
            Self::Unary(unary) => unary.display_name(table),
            Self::ZeroValue(_) => "zero_value<type>()".to_owned(),
            Self::TypeExpression(_) => "<type expression>".to_owned(),
        }
    }
}

impl TableBinaryExpression {
    pub fn display_name(&self, table: &crate::SyntaxTrees) -> String {
        format!(
            "{} {} {}",
            table.display_expression_name(self.left),
            self.operator.display_name(),
            table.display_expression_name(self.right)
        )
    }
}

impl TableUnaryExpression {
    pub fn display_name(&self, table: &crate::SyntaxTrees) -> String {
        format!(
            "{}{}",
            self.operator.display_name(),
            table.display_expression_name(self.operand)
        )
    }
}

impl TableCastExpression {
    pub fn display_name(&self, table: &crate::SyntaxTrees) -> String {
        let target_type = display_identifier_path(
            table,
            table.expressions.identifier_path_members(self.target_label),
            "::",
        );
        format!(
            "{} as {}",
            table.display_expression_name(self.value),
            target_type
        )
    }
}

impl TableCallExpression {
    pub fn display_name(&self, table: &crate::SyntaxTrees) -> String {
        let machine_arguments = if self.machine_arguments.is_empty() {
            String::new()
        } else {
            format!(
                "<{}>",
                self.machine_arguments
                    .iter()
                    .map(|argument| argument.display_name(table))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let mut arguments = String::new();

        for (index, argument) in table
            .expressions
            .expression_handles(self.arguments)
            .iter()
            .enumerate()
        {
            if index > 0 {
                arguments.push_str(", ");
            }

            arguments.push_str(&table.display_expression_name(*argument));
        }

        if self.receiver.is_valid() {
            format!(
                "{}.{}{machine_arguments}({arguments})",
                table.display_expression_name(self.receiver),
                self.target.as_str(table)
            )
        } else {
            format!(
                "{}{machine_arguments}({arguments})",
                self.target.as_str(table)
            )
        }
    }
}

impl StaticMachineArgument {
    fn display_name(&self, table: &crate::SyntaxTrees) -> String {
        if self.type_reference.is_valid() {
            return "<structural type>".to_owned();
        }
        if let Some(literal) = &self.const_literal {
            return literal.to_string();
        }
        if let Some(projection) = &self.evidence_projection {
            return format!(
                "{}.{}",
                projection.term.as_str(table),
                projection.member.as_str(table)
            );
        }
        let mut rendered = display_identifier_path(table, &self.path, "::");
        if let Some(application) = &self.application {
            let mut arguments = application
                .lifetime_arguments
                .iter()
                .map(|lifetime| format!("'{}", lifetime.as_str(table)))
                .collect::<Vec<_>>();
            arguments.extend(
                application
                    .arguments
                    .iter()
                    .map(|argument| argument.display_name(table)),
            );
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

fn display_identifier_path(
    table: &crate::SyntaxTrees,
    path: &[Identifier],
    separator: &str,
) -> String {
    let byte_count = path
        .iter()
        .map(|identifier| identifier.as_str(table).len())
        .sum::<usize>()
        + separator.len().saturating_mul(path.len().saturating_sub(1));
    let mut display_name = String::with_capacity(byte_count);

    for (index, identifier) in path.iter().enumerate() {
        if index > 0 {
            display_name.push_str(separator);
        }

        display_name.push_str(identifier.as_str(table));
    }

    display_name
}
