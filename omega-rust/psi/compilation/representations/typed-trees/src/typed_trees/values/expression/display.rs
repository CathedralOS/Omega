use crate::expression::{
    BinaryExpression, BinaryOperator, CallExpression, CastExpression, Expression, ExpressionNode,
    ExpressionTable, RangeExpression, StaticMachineArgument, TableBinaryExpression,
    TableCallExpression, TableCastExpression, TableUnaryExpression, UnaryExpression, UnaryOperator,
};
use crate::name::Identifier;

impl Expression {
    pub fn display_name(&self) -> String {
        match self {
            Expression::Match(dispatch) => {
                let arms = dispatch
                    .arms
                    .iter()
                    .map(|arm| {
                        let pattern = match &arm.pattern {
                            crate::expression::OwnedMatchPattern::Value(value) => {
                                value.display_name()
                            }
                            crate::expression::OwnedMatchPattern::Wildcard => "_".to_owned(),
                        };
                        format!("{pattern} => {}", arm.value.display_name())
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("match {} {{ {arms} }}", dispatch.subject.display_name())
            }
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
            Expression::Member(member) => format!(
                "{}{}{}",
                member.receiver.display_name(),
                language_core::PLACE_MEMBER_SEPARATOR,
                member.member
            ),
            Expression::Borrow(expression) => format!(
                "{}{}",
                borrow_access_prefix(expression.access),
                expression.target.display_name()
            ),
            Expression::Name(path) => display_name_path(path, "::"),
            Expression::Range(range) => range.display_name(),
            Expression::StructLiteral(struct_literal) => struct_literal.type_name.to_string(),
            Expression::String(value) => source::display_literal_bytes(value),
            Expression::Unary(unary) => unary.display_name(),
            Expression::ZeroValue(_) => "zero_value<type>()".to_owned(),
        }
    }
}

impl ExpressionNode {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        let mut rendered = String::new();
        self.write_display_name(table, &mut rendered);
        rendered
    }

    /// Appends this node's display name to `out`. The whole tree renders into
    /// one buffer instead of formatting a `String` per node and copying it
    /// into its parent's.
    pub fn write_display_name(&self, table: &ExpressionTable, out: &mut String) {
        use std::fmt::Write as _;
        match self {
            Self::Match(dispatch) => {
                out.push_str("match ");
                table.write_display_name(dispatch.subject, out);
                out.push_str(" { ");
                for (index, arm) in table.match_arms(dispatch.arms).iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    match arm.pattern {
                        crate::expression::MatchPattern::Value(value) => {
                            table.write_display_name(value, out)
                        }
                        crate::expression::MatchPattern::Wildcard => out.push('_'),
                    }
                    out.push_str(" => ");
                    table.write_display_name(arm.value, out);
                }
                out.push_str(" }");
            }
            Self::ArrayLiteral(values) => {
                out.push('[');
                for (index, value) in table.expression_handles(*values).iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    table.write_display_name(*value, out);
                }
                out.push(']');
            }
            Self::Atomic(atomic) => {
                let _ = write!(out, "atomic[{:?}](", atomic.ordering);
                table.write_display_name(atomic.value, out);
                out.push(')');
            }
            Self::Binary(binary) => binary.write_display_name(table, out),
            Self::Boolean(value) => {
                let _ = write!(out, "{value}");
            }
            Self::Cast(cast) => cast.write_display_name(table, out),
            Self::Call(call) => call.write_display_name(table, out),
            Self::Float(value) => {
                let _ = write!(out, "{value}");
            }
            Self::Indexed(indexed) => {
                table.write_display_name(indexed.collection, out);
                out.push('[');
                table.write_display_name(indexed.index, out);
                out.push(']');
            }
            Self::Integer(value) => {
                let _ = write!(out, "{value}");
            }
            Self::Member(member) => {
                table.write_display_name(member.receiver, out);
                let _ = write!(
                    out,
                    "{}{}",
                    language_core::PLACE_MEMBER_SEPARATOR,
                    member.member
                );
            }
            Self::Borrow(expression) => {
                out.push_str(borrow_access_prefix(expression.access));
                table.write_display_name(expression.target, out);
            }
            Self::Name(path) => {
                write_display_name_path(table.name_path_members(path.members), "::", out)
            }
            Self::Range(range) => {
                if range.start.is_valid() {
                    table.write_display_name(range.start, out);
                }
                out.push_str("..");
                if range.end.is_valid() {
                    table.write_display_name(range.end, out);
                }
            }
            Self::StructLiteral(struct_literal) => {
                let _ = write!(out, "{}", struct_literal.type_name);
            }
            Self::String(value) => out.push_str(&source::display_literal_bytes(value)),
            Self::Unary(unary) => unary.write_display_name(table, out),
            Self::ZeroValue(_) => out.push_str("zero_value<type>()"),
        }
    }
}

fn borrow_access_prefix(access: language_core::ReferenceAccess) -> &'static str {
    match access {
        language_core::ReferenceAccess::Mutable => "&mut ",
        language_core::ReferenceAccess::WriteOnly => "&write ",
        language_core::ReferenceAccess::Shared => "&",
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
    write_display_name_path(path, separator, &mut display_name);
    display_name
}

fn write_display_name_path(path: &[Identifier], separator: &str, out: &mut String) {
    for (index, name) in path.iter().enumerate() {
        if index > 0 {
            out.push_str(separator);
        }

        out.push_str(name.as_str());
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

impl TableBinaryExpression {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        let mut rendered = String::new();
        self.write_display_name(table, &mut rendered);
        rendered
    }

    fn write_display_name(&self, table: &ExpressionTable, out: &mut String) {
        table.write_display_name(self.left, out);
        out.push(' ');
        out.push_str(self.operator.display_name());
        out.push(' ');
        table.write_display_name(self.right, out);
    }
}

impl TableUnaryExpression {
    pub fn display_name(&self, table: &ExpressionTable) -> String {
        let mut rendered = String::new();
        self.write_display_name(table, &mut rendered);
        rendered
    }

    fn write_display_name(&self, table: &ExpressionTable, out: &mut String) {
        out.push_str(self.operator.display_name());
        table.write_display_name(self.operand, out);
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
        let mut rendered = String::new();
        self.write_display_name(table, &mut rendered);
        rendered
    }

    fn write_display_name(&self, table: &ExpressionTable, out: &mut String) {
        table.write_display_name(self.value, out);
        out.push_str(" as ");
        write_display_name_path(table.name_path_members(self.target_label), "::", out);
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
        let mut rendered = String::new();
        self.write_display_name(table, &mut rendered);
        rendered
    }

    fn write_display_name(&self, table: &ExpressionTable, out: &mut String) {
        use std::fmt::Write as _;
        if self.receiver.is_valid() {
            table.write_display_name(self.receiver, out);
            out.push('.');
        }
        let _ = write!(out, "{}", self.target);
        let private_slot = self
            .private_layout_operation
            .as_ref()
            .map(|operation| &operation.selected_slot);
        if !self.machine_arguments.is_empty() || private_slot.is_some() {
            out.push('<');
            for (index, argument) in self
                .machine_arguments
                .iter()
                .chain(private_slot)
                .enumerate()
            {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&argument.display_name());
            }
            out.push('>');
        }
        out.push('(');
        for (index, argument) in table.expression_handles(self.arguments).iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            table.write_display_name(*argument, out);
        }
        out.push(')');
    }
}

impl StaticMachineArgument {
    pub fn display_name(&self) -> String {
        if self.type_reference.is_valid() {
            return "<structural type>".to_owned();
        }
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
            Self::CaseMembership => "in",
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
