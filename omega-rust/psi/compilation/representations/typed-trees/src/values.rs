/*
We represent typed value expressions (`values::expression`), constant integers
(`const_integer`), subtree visitors (`subtrees`), and cross-table expression
copying in `TypedTrees`.
*/

pub(crate) mod captures {
    use crate::expression::ExpressionHandle;

    impl crate::TypedTrees {
        pub fn saved_value_source(
            &self,
            saved: &crate::expression::TableSavedSubject,
        ) -> Option<crate::expression::ExpressionHandle> {
            use crate::expression::ValueCaptureOwner;
            let table = &self.expression_table;
            if !table.contains_value_capture(saved.capture) {
                return None;
            }
            let capture = table.value_capture(saved.capture);
            let source = match capture.owner {
                ValueCaptureOwner::Match => return table.saved_subject_source(saved),
                ValueCaptureOwner::ExpressionArgument { call, position } => {
                    if !table.expression_is_valid(call) {
                        return None;
                    }
                    let call = table.call_node(call)?;
                    *table
                        .expression_handles(call.arguments)
                        .get(position as usize)?
                }
                ValueCaptureOwner::StatementArgument { call, position } => {
                    if !self.statement_table.statement_is_valid(call) {
                        return None;
                    }
                    let call = self.statement_table.statement(call).as_call()?;
                    *self
                        .statement_table
                        .expression_handles(call.arguments)
                        .get(position as usize)?
                }
            };
            (!saved.pattern_head.is_valid()
                && source == capture.subject
                && table.expression_is_valid(source))
            .then_some(source)
        }

        pub fn saved_argument_source(
            &self,
            expression: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            let crate::expression::ExpressionNode::SavedSubject(saved) =
                self.expression_table.expression(expression)
            else {
                return None;
            };
            if !self.expression_table.contains_value_capture(saved.capture)
                || matches!(
                    self.expression_table.value_capture(saved.capture).owner,
                    crate::expression::ValueCaptureOwner::Match
                )
            {
                return None;
            }
            self.saved_value_source(saved)
        }
    }
}
pub mod const_integer {
    use super::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable};
    use symbols::SymbolHandle;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ConstIntegerExpression {
        Closed(i128),
        Open,
    }

    pub fn classify(
        expressions: &ExpressionTable,
        expression: ExpressionHandle,
        is_binder: &impl Fn(SymbolHandle, &str) -> bool,
    ) -> Option<ConstIntegerExpression> {
        fold(
            expressions,
            expression,
            &|leaf| match leaf {
                IntegerLeaf::Literal(literal) => literal
                    .value_i64()
                    .map(i128::from)
                    .or_else(|| literal.value_u64().map(i128::from))
                    .map(ConstIntegerExpression::Closed),
                IntegerLeaf::Name { symbol, spelling } => {
                    if !symbol.is_valid()
                        && let Ok(value) = spelling.parse::<i128>()
                    {
                        return Some(ConstIntegerExpression::Closed(value));
                    }
                    is_binder(symbol, spelling).then_some(ConstIntegerExpression::Open)
                }
            },
            &|operator, left, right| match (left, right) {
                (ConstIntegerExpression::Closed(left), ConstIntegerExpression::Closed(right)) => {
                    apply(operator, left, right).map(ConstIntegerExpression::Closed)
                }
                _ => Some(ConstIntegerExpression::Open),
            },
        )
    }

    pub fn is_integer_expression(
        expressions: &ExpressionTable,
        expression: ExpressionHandle,
        is_binder: &impl Fn(SymbolHandle, &str) -> bool,
    ) -> bool {
        fold(
            expressions,
            expression,
            &|leaf| match leaf {
                IntegerLeaf::Literal(_) => Some(()),
                IntegerLeaf::Name { symbol, spelling } => ((!symbol.is_valid()
                    && spelling.parse::<i128>().is_ok())
                    || is_binder(symbol, spelling))
                .then_some(()),
            },
            &|_, (), ()| Some(()),
        )
        .is_some()
    }

    enum IntegerLeaf<'expression> {
        Literal(&'expression numerics::literals::IntegerLiteral),
        Name {
            symbol: SymbolHandle,
            spelling: &'expression str,
        },
    }

    fn fold<Value>(
        expressions: &ExpressionTable,
        expression: ExpressionHandle,
        leaf: &impl Fn(IntegerLeaf<'_>) -> Option<Value>,
        operation: &impl Fn(BinaryOperator, Value, Value) -> Option<Value>,
    ) -> Option<Value> {
        match expressions.expression(expression) {
            ExpressionNode::Integer(literal) => leaf(IntegerLeaf::Literal(literal)),
            ExpressionNode::Name(path) => {
                let [member] = expressions.name_path_members(path.members) else {
                    return None;
                };
                leaf(IntegerLeaf::Name {
                    symbol: path.symbol,
                    spelling: member.as_str(),
                })
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::Modulo
                ) =>
            {
                operation(
                    binary.operator,
                    fold(expressions, binary.left, leaf, operation)?,
                    fold(expressions, binary.right, leaf, operation)?,
                )
            }
            _ => None,
        }
    }

    fn apply(operator: BinaryOperator, left: i128, right: i128) -> Option<i128> {
        match operator {
            BinaryOperator::Add => left.checked_add(right),
            BinaryOperator::Subtract => left.checked_sub(right),
            BinaryOperator::Multiply => left.checked_mul(right),
            BinaryOperator::Divide => left.checked_div(right),
            BinaryOperator::Modulo => left.checked_rem(right),
            _ => None,
        }
    }
}
pub mod expression {

    use crate::AuthoredDeclarationSelectionOccurrenceId;
    use crate::name::Identifier;
    use crate::static_applications::{
        StaticMachineApplication, StaticMachineApplicationHandle, StaticMachineBinding,
        StaticMachineLifetimeBinding,
    };
    use arena::{Arena, Handle, HandleSpan};
    pub type BuildSelectionRequest =
        language_semantics::declaration_selection::BuildSelectionRequest<
            crate::types::TypeReferenceHandle,
        >;
    use numerics::literals::IntegerLiteral;
    use source::SourceSpan;
    use std::ops::Deref;
    use std::sync::Arc;
    use symbols::JoinedPath;
    use symbols::SymbolHandle;

    pub(crate) mod display {
        use diagnostics::{bracketed_display_names, comma_join_display};

        use crate::expression::OwnedMatchPattern;
        use crate::expression::{
            BinaryExpression, BinaryOperator, CallExpression, CastExpression, Expression,
            ExpressionHandle, ExpressionNode, ExpressionTable, RangeExpression,
            StaticMachineArgument, TableBinaryExpression, TableCallExpression, TableCastExpression,
            TableUnaryExpression, UnaryExpression, UnaryOperator,
        };
        use crate::name::Identifier;

        struct ExpressionDisplay<'table> {
            table: &'table ExpressionTable,
            displayed_expression: &'table dyn Fn(ExpressionHandle) -> ExpressionHandle,
        }

        impl std::ops::Deref for ExpressionDisplay<'_> {
            type Target = ExpressionTable;

            fn deref(&self) -> &Self::Target {
                self.table
            }
        }

        impl ExpressionDisplay<'_> {
            fn write_display_name(&self, expression: ExpressionHandle, rendered: &mut String) {
                self.table
                    .expression((self.displayed_expression)(expression))
                    .write_display_name_with(self, rendered);
            }
        }

        impl ExpressionTable {
            pub(crate) fn display_name_resolving(
                &self,
                expression: ExpressionHandle,
                displayed_expression: &dyn Fn(ExpressionHandle) -> ExpressionHandle,
            ) -> String {
                let mut rendered = String::new();
                ExpressionDisplay {
                    table: self,
                    displayed_expression,
                }
                .write_display_name(expression, &mut rendered);
                rendered
            }
        }

        macro_rules! display_name_via_writer {
            () => {
                pub fn display_name(&self, table: &ExpressionTable) -> String {
                    let mut rendered = String::new();
                    self.write_display_name_with(
                        &ExpressionDisplay {
                            table,
                            displayed_expression: &|expression| expression,
                        },
                        &mut rendered,
                    );
                    rendered
                }
            };
        }

        impl Expression {
            pub fn display_name(&self) -> String {
                match self {
                    Expression::Match(dispatch) => {
                        let arms = dispatch
                            .arms
                            .iter()
                            .map(|arm| {
                                let pattern = match &arm.pattern {
                                    OwnedMatchPattern::Value(value) => value.display_name(),
                                    OwnedMatchPattern::Wildcard => "_".to_owned(),
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
                    Expression::UndeterminedNaN(format) => {
                        format!("<undetermined NaN {}>", format.name())
                    }
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
                        expression.access.borrow_prefix(),
                        expression.target.display_name()
                    ),
                    Expression::Name(path) => display_name_path(path, "::"),
                    Expression::Range(range) => range.display_name(),
                    Expression::StructLiteral(struct_literal) => {
                        struct_literal.type_name.to_string()
                    }
                    Expression::String(value) => source::display_literal_bytes(value),
                    Expression::Unary(unary) => unary.display_name(),
                    Expression::TypeExpression(_) => "type".to_owned(),
                    Expression::ZeroValue(_) => "zero_value<type>()".to_owned(),
                }
            }
        }

        impl ExpressionNode {
            display_name_via_writer!();

            pub fn write_display_name(&self, table: &ExpressionTable, out: &mut String) {
                self.write_display_name_with(
                    &ExpressionDisplay {
                        table,
                        displayed_expression: &|expression| expression,
                    },
                    out,
                );
            }

            fn write_display_name_with(&self, table: &ExpressionDisplay<'_>, out: &mut String) {
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
                                crate::expression::MatchPattern::Destructure(_) => out.push_str(
                                    &display_match_pattern(table.table, arm.pattern, |value| {
                                        let mut rendered = String::new();
                                        table.write_display_name(value, &mut rendered);
                                        rendered
                                    }),
                                ),
                                crate::expression::MatchPattern::Wildcard => out.push('_'),
                            }
                            out.push_str(" => ");
                            table.write_display_name(arm.value, out);
                        }
                        out.push_str(" }");
                    }
                    Self::SavedSubject(saved) => {
                        let _ = write!(
                            out,
                            "<saved subject #{}:{}>",
                            saved.capture.arena_index(),
                            saved.capture.generation()
                        );
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
                    Self::Binary(binary) => binary.write_display_name_with(table, out),
                    Self::Boolean(value) => {
                        let _ = write!(out, "{value}");
                    }
                    Self::Cast(cast) => cast.write_display_name_with(table, out),
                    Self::Call(call) => call.write_display_name_with(table, out),
                    Self::Float(value) => {
                        let _ = write!(out, "{value}");
                    }
                    Self::UndeterminedNaN(format) => {
                        let _ = write!(out, "<undetermined NaN {}>", format.name());
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
                        out.push_str(expression.access.borrow_prefix());
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
                    Self::Unary(unary) => unary.write_display_name_with(table, out),
                    Self::TypeExpression(_) => out.push_str("type"),
                    Self::ZeroValue(_) => out.push_str("zero_value<type>()"),
                }
            }
        }

        pub fn display_match_pattern(
            table: &ExpressionTable,
            pattern: crate::expression::MatchPattern,
            render: impl Fn(ExpressionHandle) -> String,
        ) -> String {
            use crate::expression::{MatchFieldAction, MatchPattern};
            let pattern = match pattern {
                MatchPattern::Value(value) => return render(value),
                MatchPattern::Wildcard => return "_".to_owned(),
                MatchPattern::Destructure(pattern) => pattern,
            };
            let mut fields = table
                .match_fields(pattern.fields)
                .iter()
                .map(|field| {
                    let member = field.member.as_str();
                    match &field.action {
                        MatchFieldAction::Bind(binding) if binding.as_str() == member => {
                            member.to_owned()
                        }
                        MatchFieldAction::Bind(binding) => {
                            format!("{member} as {}", binding.as_str())
                        }
                        MatchFieldAction::Equal(value) => format!("{member}: {}", render(*value)),
                        MatchFieldAction::Waive => format!("{member} as _"),
                    }
                })
                .collect::<Vec<_>>();
            if pattern.has_rest {
                fields.push("..".to_owned());
            }
            let selectors = table.match_proof_selectors(pattern.proof_selectors);
            let proofs = if selectors.is_empty() {
                String::new()
            } else {
                format!(
                    "; {}",
                    selectors
                        .iter()
                        .map(|selector| format!(
                            "{}: {}",
                            selector.output_field.as_str(),
                            selector.binding.as_str()
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let guard = if pattern.guard.is_valid() {
                format!(" if {}", render(pattern.guard))
            } else {
                String::new()
            };
            format!(
                "{} {{ {}{proofs} }}{guard}",
                render(pattern.head),
                fields.join(", ")
            )
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

        pub fn display_call_label(
            table: &ExpressionTable,
            call: &TableCallExpression,
            render: impl Fn(ExpressionHandle) -> String,
        ) -> String {
            let arguments = table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| render(*argument))
                .collect::<Vec<_>>()
                .join(", ");
            if call.receiver.is_valid() {
                format!("{}.{}({arguments})", render(call.receiver), call.target)
            } else {
                format!("{}({arguments})", call.target)
            }
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
            display_name_via_writer!();

            fn write_display_name_with(&self, table: &ExpressionDisplay<'_>, out: &mut String) {
                table.write_display_name(self.left, out);
                out.push(' ');
                out.push_str(self.operator.display_name());
                out.push(' ');
                table.write_display_name(self.right, out);
            }
        }

        impl TableUnaryExpression {
            display_name_via_writer!();

            fn write_display_name_with(&self, table: &ExpressionDisplay<'_>, out: &mut String) {
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
            display_name_via_writer!();

            fn write_display_name_with(&self, table: &ExpressionDisplay<'_>, out: &mut String) {
                table.write_display_name(self.value, out);
                out.push_str(" as ");
                write_display_name_path(table.name_path_members(self.target_label), "::", out);
            }
        }

        impl CallExpression {
            pub fn display_name(&self) -> String {
                let arguments = comma_join_display(&*self.arguments, Expression::display_name);

                if let Some(receiver) = &self.receiver {
                    format!("{}.{}({arguments})", receiver.display_name(), self.target)
                } else {
                    format!("{}({arguments})", self.target)
                }
            }
        }

        impl TableCallExpression {
            display_name_via_writer!();

            fn write_display_name_with(&self, table: &ExpressionDisplay<'_>, out: &mut String) {
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
                let installed_operands = self
                    .installed_interrupt_operation
                    .as_ref()
                    .map(|request| [&request.registrar, &request.timer_slot]);
                if !self.machine_arguments.is_empty()
                    || private_slot.is_some()
                    || installed_operands.is_some()
                {
                    out.push('<');
                    for (index, argument) in installed_operands
                        .into_iter()
                        .flatten()
                        .chain(self.machine_arguments.iter())
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
                for (index, argument) in table.expression_handles(self.arguments).iter().enumerate()
                {
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
                self.display_name_with_owner(None)
            }

            pub fn display_name_in(&self, table: &ExpressionTable) -> String {
                self.display_name_with_owner(Some(table))
            }

            fn display_name_with_owner(&self, table: Option<&ExpressionTable>) -> String {
                // An authored type operand has no name path. A named operand may
                // also carry a type reference that records the scope it was
                // written in; it is still shown by its name.
                if self.type_reference.is_valid() && self.path.is_empty() {
                    return "<structural type>".to_owned();
                }
                if let Some(literal) = &self.const_literal {
                    return literal.to_string();
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
                    arguments.extend(
                        application
                            .arguments
                            .iter()
                            .map(|argument| argument.display_name_with_owner(table)),
                    );
                    rendered.push('<');
                    rendered.push_str(&arguments.join(", "));
                    rendered.push('>');
                    if application.member.is_valid() {
                        rendered.push_str("::");
                        match table.and_then(|table| table.static_member(application.member)) {
                            Some(member) => {
                                rendered.push_str(&member.display_name_with_owner(table))
                            }
                            None => rendered.push_str("<qualified member requires owning table>"),
                        }
                    }
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
    }
    pub(crate) mod index_authority {
        use super::{ExpressionHandle, ExpressionNode, ExpressionTable, source_span_index};
        use crate::types::TypeReferenceHandle;
        use arena::Handle;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct IndexExpressionAuthority {
            pub normalization: Option<OpenIndexNormalization>,
            pub operation: Option<OpenIndexOperationSelection>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct OpenIndexNormalization {
            pub index_type: TypeReferenceHandle,
            pub normalizer_version: u32,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct OpenIndexOperationSelection {
            pub expression: ExpressionHandle,
            pub spelling: language_core::operator_spelling::OperatorSpelling,
            pub operator: SymbolHandle,
            pub operand_type: TypeReferenceHandle,
            pub provider: SymbolHandle,
            pub application: Option<super::StaticMachineArgument>,
            pub algebra_trait: SymbolHandle,
            pub algebra_requirement: String,
            pub algebra_alias: Option<String>,
            pub commutativity_licensed: bool,
            pub associativity_licensed: bool,
            pub distributes_over: Vec<String>,
            pub left_identity_literal: String,
            pub right_identity_literal: String,
        }

        impl OpenIndexOperationSelection {
            pub fn apply_provider(&mut self, selected: &super::StaticMachineArgument, alias: &str) {
                self.provider = selected.symbol;
                self.application = selected.application.as_ref().map(|_| selected.clone());
                self.algebra_alias = Some(alias.to_owned());
            }

            pub fn operation_contract_identity(&self, program: &crate::TypedTrees) -> String {
                let subject = program.normalized_type_identity_text(self.operand_type);
                format!(
                    "trait::{}::{}({subject},{subject})->{subject}",
                    program.symbols.display_path(self.algebra_trait, "::"),
                    self.algebra_requirement,
                )
            }
        }

        impl ExpressionTable {
            pub fn index_expression_authority(
                &self,
                expression: ExpressionHandle,
            ) -> Option<&IndexExpressionAuthority> {
                if !self.expressions.is_valid(expression) {
                    return None;
                }
                let handle = *self
                    .index_expression_authority_handles
                    .get(source_span_index(expression))?;
                handle
                    .is_valid()
                    .then(|| self.index_expression_authorities.get(handle))
            }

            pub fn index_expression_authority_mut(
                &mut self,
                expression: ExpressionHandle,
            ) -> &mut IndexExpressionAuthority {
                assert!(
                    self.expressions.is_valid(expression),
                    "index authority requires a live expression"
                );
                let position = source_span_index(expression);
                if self.index_expression_authority_handles.len() <= position {
                    self.index_expression_authority_handles
                        .resize(position + 1, Handle::invalid());
                }
                let handle = &mut self.index_expression_authority_handles[position];
                if !handle.is_valid() {
                    *handle = self
                        .index_expression_authorities
                        .insert(IndexExpressionAuthority::default());
                }
                self.index_expression_authorities.get_mut(*handle)
            }

            pub fn open_index_operation(
                &self,
                expression: ExpressionHandle,
            ) -> Option<&OpenIndexOperationSelection> {
                self.binary_node(expression)?;
                self.index_expression_authority(expression)?
                    .operation
                    .as_ref()
            }

            pub fn open_index_operations(
                &self,
                root: ExpressionHandle,
            ) -> impl Iterator<Item = &OpenIndexOperationSelection> {
                let mut pending = vec![root];
                std::iter::from_fn(move || {
                    while let Some(expression) = pending.pop() {
                        match self.expression(expression) {
                            ExpressionNode::Binary(binary) => {
                                pending.push(binary.right);
                                pending.push(binary.left);
                            }
                            ExpressionNode::Unary(unary) => pending.push(unary.operand),
                            _ => {}
                        }
                        if let Some(operation) = self.open_index_operation(expression) {
                            return Some(operation);
                        }
                    }
                    None
                })
            }

            pub fn selected_index_operations(
                &self,
            ) -> impl Iterator<Item = &OpenIndexOperationSelection> {
                self.index_expression_authorities
                    .iter()
                    .filter_map(|(_, authority)| {
                        let operation = authority.operation.as_ref()?;
                        self.open_index_operation(operation.expression)
                    })
            }

            pub fn open_index_normalization_count(&self) -> usize {
                self.index_expression_authorities
                    .iter()
                    .filter(|(_, authority)| authority.normalization.is_some())
                    .count()
            }
        }
    }
    pub(crate) mod shape {
        use crate::expression::TableCallExpression;

        impl TableCallExpression {
            pub fn selects_only_nominal_route(&self) -> bool {
                self.static_requirement_dispatch.is_none()
                    && self.quotient_operation.is_none()
                    && self.private_layout_operation.is_none()
                    && self.installed_interrupt_operation.is_none()
                    && self.layout_query.is_none()
                    && self.native_code_layout.is_none()
            }

            pub fn carries_only_positional_arguments(&self) -> bool {
                self.machine_arguments.is_empty() && self.evidence_arguments.is_empty()
            }

            pub fn is_ordinary_positional_call(&self) -> bool {
                !self.receiver.is_valid()
                    && !self.static_machine_parameter.is_valid()
                    && self.selects_only_nominal_route()
                    && self.carries_only_positional_arguments()
            }
        }
    }
    pub(crate) mod static_members {
        use super::{ExpressionTable, StaticMachineArgument};
        use symbols::SymbolHandle;

        impl ExpressionTable {
            pub fn copy_static_argument_from(
                &mut self,
                source: &Self,
                argument: &StaticMachineArgument,
            ) -> StaticMachineArgument {
                self.copy_static_argument_members(Some(source), argument)
            }

            pub fn copy_static_argument_from_self(
                &mut self,
                argument: &StaticMachineArgument,
            ) -> StaticMachineArgument {
                self.copy_static_argument_members(None, argument)
            }

            fn copy_static_argument_members(
                &mut self,
                source: Option<&Self>,
                argument: &StaticMachineArgument,
            ) -> StaticMachineArgument {
                let mut copied = argument.clone();
                if let Some(application) = &mut copied.application {
                    application.arguments = application
                        .arguments
                        .iter()
                        .map(|argument| self.copy_static_argument_members(source, argument))
                        .collect();
                    if application.member.is_valid() {
                        let member = source
                            .unwrap_or(self)
                            .static_member(application.member)
                            .expect(
                                "qualified static member belongs to its source expression table",
                            )
                            .clone();
                        let copied_member = self.copy_static_argument_members(source, &member);
                        application.member = self.insert_static_member(copied_member);
                    }
                }
                copied
            }

            pub fn remap_static_argument_symbols(
                &mut self,
                argument: &mut StaticMachineArgument,
                symbols: &[(SymbolHandle, SymbolHandle)],
            ) {
                if let Some((_, replacement)) = symbols
                    .iter()
                    .find(|(source, _)| *source == argument.symbol)
                {
                    argument.symbol = *replacement;
                }
                if let Some(application) = &mut argument.application {
                    for argument in application.arguments.iter_mut() {
                        self.remap_static_argument_symbols(argument, symbols);
                    }
                    if application.member.is_valid() {
                        let mut member = self
                            .static_member(application.member)
                            .expect(
                                "qualified static member belongs to its receiving expression table",
                            )
                            .clone();
                        self.remap_static_argument_symbols(&mut member, symbols);
                        *self.static_member_mut(application.member).expect(
                            "qualified static member remains in its receiving expression table",
                        ) = member;
                    }
                }
            }
        }

        impl ExpressionTable {
            pub fn remap_static_application_operands<'scope>(
                &mut self,
                handle: crate::static_applications::StaticMachineApplicationHandle,
                symbols: impl Into<crate::SymbolRebinding<'scope>>,
            ) {
                let symbols = symbols.into();
                let mut arguments = self.static_machine_application_arguments(handle).to_vec();
                for argument in &mut arguments {
                    self.remap_static_argument_symbols(argument, symbols.authored());
                }
                self.static_machine_application_arguments_mut(handle)
                    .clone_from_slice(&arguments);
                let mut selected = self
                    .static_machine_application_selected_machines(handle)
                    .to_vec();
                for argument in &mut selected {
                    self.remap_static_argument_symbols(argument, symbols.execution());
                }
                self.static_machine_application_selected_machines_mut(handle)
                    .clone_from_slice(&selected);
            }

            pub fn remap_static_binding_symbols(
                &mut self,
                binding: &mut crate::static_applications::StaticMachineBinding,
                symbols: &[(SymbolHandle, SymbolHandle)],
            ) {
                use crate::static_applications::StaticMachineBindingValue;
                match &mut binding.value {
                    StaticMachineBindingValue::Machine(argument)
                    | StaticMachineBindingValue::Conformance(argument) => {
                        self.remap_static_argument_symbols(argument, symbols)
                    }
                    StaticMachineBindingValue::RuntimeValue { subject, .. } => {
                        if let Some((_, replacement)) =
                            symbols.iter().find(|(source, _)| *source == *subject)
                        {
                            *subject = *replacement;
                        }
                    }
                    StaticMachineBindingValue::Type(_) | StaticMachineBindingValue::Const(_) => {}
                }
            }
        }

        impl super::TableCallExpression {
            pub fn request_static_arguments(&self) -> impl Iterator<Item = &StaticMachineArgument> {
                self.native_code_layout
                    .iter()
                    .map(|request| &request.operand)
                    .chain(self.layout_query.iter().map(|request| &request.operand))
                    .chain(
                        self.private_layout_operation
                            .iter()
                            .map(|request| &request.selected_slot),
                    )
                    .chain(
                        self.installed_interrupt_operation
                            .iter()
                            .flat_map(|request| [&request.registrar, &request.timer_slot]),
                    )
                    .chain(self.quotient_operation.iter().flat_map(|request| {
                        std::iter::once(&request.representative_operation).chain(
                            request
                                .theorem_evidence
                                .iter()
                                .map(|theorem| &theorem.application),
                        )
                    }))
            }

            pub fn request_static_arguments_mut(
                &mut self,
            ) -> impl Iterator<Item = &mut StaticMachineArgument> {
                if let Some(request) = &mut self.installed_interrupt_operation {
                    request.invalidate_preparation();
                }
                self.native_code_layout
                    .iter_mut()
                    .map(|request| &mut request.operand)
                    .chain(
                        self.layout_query
                            .iter_mut()
                            .map(|request| &mut request.operand),
                    )
                    .chain(
                        self.private_layout_operation
                            .iter_mut()
                            .map(|request| &mut request.selected_slot),
                    )
                    .chain(
                        self.installed_interrupt_operation
                            .iter_mut()
                            .flat_map(|request| [&mut request.registrar, &mut request.timer_slot]),
                    )
                    .chain(self.quotient_operation.iter_mut().flat_map(|request| {
                        std::iter::once(&mut request.representative_operation).chain(
                            request
                                .theorem_evidence
                                .iter_mut()
                                .map(|theorem| &mut theorem.application),
                        )
                    }))
            }

            pub(crate) fn visit_static_arguments_mut(
                &mut self,
                mut visit: impl FnMut(&mut StaticMachineArgument),
            ) {
                for argument in &mut self.machine_arguments {
                    visit(argument);
                }
                for argument in self.request_static_arguments_mut() {
                    visit(argument);
                }
            }
        }

        impl crate::statement::TableCall {
            pub fn request_static_arguments(&self) -> impl Iterator<Item = &StaticMachineArgument> {
                self.installed_interrupt_operation
                    .iter()
                    .flat_map(|request| [&request.registrar, &request.timer_slot])
            }
            pub fn request_static_arguments_mut(
                &mut self,
            ) -> impl Iterator<Item = &mut StaticMachineArgument> {
                if let Some(request) = &mut self.installed_interrupt_operation {
                    request.invalidate_preparation();
                }
                self.installed_interrupt_operation
                    .iter_mut()
                    .flat_map(|request| [&mut request.registrar, &mut request.timer_slot])
            }
        }
    }
    pub(crate) mod table_copies {

        use super::CompletedValueCapture;
        use super::ValueCaptureOwner;
        use crate::name::Identifier;
        use crate::remapped;
        use crate::values::expression::{
            ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern, TableAtomicExpression,
            TableBinaryExpression, TableBorrowExpression, TableCallExpression, TableCastExpression,
            TableIndexedExpression, TableMatchArm, TableMatchExpression, TableMemberExpression,
            TableNamePath, TableRangeExpression, TableStructLiteral, TableStructLiteralField,
            TableUnaryExpression,
        };
        use arena::{Handle, HandleSpan};

        #[derive(Default)]
        struct CopyGraph {
            expressions: Vec<(ExpressionHandle, ExpressionHandle)>,
            subjects: Vec<(Handle<CompletedValueCapture>, Handle<CompletedValueCapture>)>,
        }
        use symbols::SymbolHandle;

        pub struct ExpressionCopy {
            pub root: ExpressionHandle,
            pub correspondence: Vec<(ExpressionHandle, ExpressionHandle)>,
        }

        #[derive(Clone, Copy)]
        enum CopySource<'source> {
            Own,
            Table(&'source ExpressionTable),
        }

        impl<'source> CopySource<'source> {
            fn table<'table>(self, destination: &'table ExpressionTable) -> &'table ExpressionTable
            where
                'source: 'table,
            {
                match self {
                    Self::Own => destination,
                    Self::Table(source) => source,
                }
            }
        }

        fn retain_every_field(_: &TableStructLiteral, _: &TableStructLiteralField) -> bool {
            true
        }

        #[cfg(test)]
        mod correspondence_tests {
            use super::*;
            use crate::expression::BinaryOperator;

            #[test]
            fn expression_copy_correspondence_preserves_shared_occurrences_in_both_sources() {
                let mut source = ExpressionTable::default();
                let child = source.insert(ExpressionNode::Boolean(true));
                let root = source.insert(ExpressionNode::Binary(TableBinaryExpression {
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    left: child,
                    operator: BinaryOperator::And,
                    right: child,
                }));
                let own = source.copy_graph(None, root);
                let mut destination = ExpressionTable::default();
                let external = destination.copy_graph(Some(&source), root);
                for (table, copy) in [(&source, own), (&destination, external)] {
                    assert_eq!(copy.correspondence.len(), 2);
                    let child_copy = copy
                        .correspondence
                        .iter()
                        .find(|(original, _)| *original == child)
                        .expect("child correspondence")
                        .1;
                    assert_eq!(
                        copy.correspondence
                            .iter()
                            .filter(|(original, _)| *original == root)
                            .count(),
                        1
                    );
                    let ExpressionNode::Binary(binary) = table.expression(copy.root) else {
                        panic!("copied binary");
                    };
                    assert_eq!(binary.left, child_copy);
                    assert_eq!(binary.right, child_copy);
                }
                let first = source.copy_graph(None, root);
                let second = source.copy_graph(None, root);
                assert_ne!(first.root, second.root);
                assert_eq!(source.expression(child), &ExpressionNode::Boolean(true));
            }

            #[test]
            fn copied_value_binding_names_the_same_evaluation_as_its_call_operand() {
                use crate::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
                let mut source = ExpressionTable::default();
                let value = source.insert(ExpressionNode::Boolean(true));
                let application = source.retain_static_machine_application(
                    SymbolHandle::invalid(),
                    Identifier::generated("invoke"),
                    &[],
                );
                source.set_static_machine_application_bindings(
                    application,
                    &[StaticMachineBinding {
                        parameter: SymbolHandle::invalid(),
                        value: StaticMachineBindingValue::RuntimeValue {
                            type_reference: crate::types::TypeReferenceHandle::invalid(),
                            subject: SymbolHandle::invalid(),
                            expression: value,
                        },
                    }],
                );
                let mut destination = ExpressionTable::default();
                destination.insert(ExpressionNode::Boolean(false));
                let (arguments, _, copied, _) = destination.copy_call_operands_from(
                    &source,
                    &[value, value],
                    application,
                    None,
                );
                let actuals = destination.expression_handles(arguments);
                assert_eq!(actuals[0], actuals[1]);
                let StaticMachineBindingValue::RuntimeValue { expression, .. } =
                    destination.static_machine_application_bindings(copied)[0].value
                else {
                    panic!("copied runtime binding");
                };
                assert_eq!(expression, actuals[0]);
                assert_eq!(
                    destination.expression(expression),
                    &ExpressionNode::Boolean(true)
                );
                assert_eq!(
                    source.static_machine_application_bindings(application)[0].value,
                    StaticMachineBindingValue::RuntimeValue {
                        type_reference: crate::types::TypeReferenceHandle::invalid(),
                        subject: SymbolHandle::invalid(),
                        expression: value,
                    }
                );
            }
        }

        impl ExpressionTable {
            pub fn copy_graph(
                &mut self,
                source: Option<&ExpressionTable>,
                expression: ExpressionHandle,
            ) -> ExpressionCopy {
                let source = source.map_or(CopySource::Own, CopySource::Table);
                let mut copies = CopyGraph::default();
                let root = self.copy_subtree_with_occurrences(
                    source,
                    expression,
                    &retain_every_field,
                    &mut copies,
                );
                ExpressionCopy {
                    root,
                    correspondence: copies.expressions,
                }
            }

            fn copy_call_static_applications(
                &mut self,
                source: CopySource<'_>,
                application: crate::static_applications::StaticMachineApplicationHandle,
                mut dispatch: Option<crate::StaticRequirementDispatch>,
                copies: &mut CopyGraph,
            ) -> (
                crate::static_applications::StaticMachineApplicationHandle,
                Option<crate::StaticRequirementDispatch>,
            ) {
                let copied = self.copy_static_machine_application(source, application, copies);
                if let Some(dispatch) = &mut dispatch {
                    dispatch.selection_application =
                        if dispatch.selection_application == application {
                            copied
                        } else {
                            self.copy_static_machine_application(
                                source,
                                dispatch.selection_application,
                                copies,
                            )
                        };
                }
                (copied, dispatch)
            }

            pub fn remap_call_static_applications<'scope>(
                &mut self,
                application: crate::static_applications::StaticMachineApplicationHandle,
                dispatch: Option<&crate::StaticRequirementDispatch>,
                symbols: impl Into<crate::SymbolRebinding<'scope>>,
            ) {
                let symbols = symbols.into();
                let selected = dispatch.map_or(Default::default(), |dispatch| {
                    dispatch.selection_application
                });
                if application.is_valid() {
                    self.remap_static_application(application, symbols);
                }
                if selected.is_valid() && selected != application {
                    self.remap_static_application(selected, symbols);
                }
            }

            fn remap_static_application(
                &mut self,
                handle: crate::static_applications::StaticMachineApplicationHandle,
                symbols: crate::SymbolRebinding<'_>,
            ) {
                let application = self.static_machine_application_mut(handle);
                application.lexical_owner = remapped(application.lexical_owner, symbols);
                self.remap_static_application_operands(handle, symbols);
                let subject_expressions = self
                    .static_machine_application_bindings(handle)
                    .iter()
                    .filter_map(|binding| match &binding.value {
                        crate::static_applications::StaticMachineBindingValue::RuntimeValue {
                            expression,
                            ..
                        } if expression.is_valid() => Some(*expression),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let mut bindings = self.static_machine_application_bindings(handle).to_vec();
                for binding in &mut bindings {
                    self.remap_static_binding_symbols(binding, symbols.authored());
                }
                self.static_machine_application_bindings_mut(handle)
                    .clone_from_slice(&bindings);
                for expression in subject_expressions {
                    self.remap_symbols_in(expression, symbols);
                }
            }

            fn copy_static_machine_application(
                &mut self,
                source: CopySource<'_>,
                application: crate::static_applications::StaticMachineApplicationHandle,
                copies: &mut CopyGraph,
            ) -> crate::static_applications::StaticMachineApplicationHandle {
                if !application.is_valid() {
                    return application;
                }
                let origin = source.table(self);
                let declaration = origin.static_machine_application(application).clone();
                let arguments = origin
                    .static_machine_application_arguments(application)
                    .to_vec();
                let bindings = origin
                    .static_machine_application_bindings(application)
                    .to_vec();
                let selected = origin
                    .static_machine_application_selected_machines(application)
                    .to_vec();
                let lifetimes = origin
                    .static_machine_application_lifetimes(application)
                    .to_vec();
                let arguments = arguments
                    .iter()
                    .map(|argument| match source {
                        CopySource::Own => self.copy_static_argument_from_self(argument),
                        CopySource::Table(table) => self.copy_static_argument_from(table, argument),
                    })
                    .collect::<Vec<_>>();
                let selected = selected
                    .iter()
                    .map(|argument| match source {
                        CopySource::Own => self.copy_static_argument_from_self(argument),
                        CopySource::Table(table) => self.copy_static_argument_from(table, argument),
                    })
                    .collect::<Vec<_>>();
                let mut bindings = bindings;
                for binding in &mut bindings {
                    if let crate::static_applications::StaticMachineBindingValue::RuntimeValue {
                        expression,
                        ..
                    } = &mut binding.value
                        && expression.is_valid()
                    {
                        *expression = self.copy_subtree_with_occurrences(
                            source,
                            *expression,
                            &retain_every_field,
                            copies,
                        );
                    }
                    if let crate::static_applications::StaticMachineBindingValue::Machine(
                        argument,
                    )
                    | crate::static_applications::StaticMachineBindingValue::Conformance(
                        argument,
                    ) = &mut binding.value
                    {
                        *argument = match source {
                            CopySource::Own => self.copy_static_argument_from_self(argument),
                            CopySource::Table(table) => {
                                self.copy_static_argument_from(table, argument)
                            }
                        };
                    }
                }
                let copy = self.retain_static_machine_application(
                    declaration.registration_operation,
                    declaration.target,
                    &arguments,
                );
                self.static_machine_application_mut(copy).lexical_owner = declaration.lexical_owner;
                self.static_machine_application_mut(copy)
                    .binding_declaration = declaration.binding_declaration;
                self.static_machine_application_mut(copy)
                    .conformance_selection =
                    declaration.conformance_selection.map(|mut selection| {
                        selection.application = None;
                        selection
                    });
                self.set_static_machine_application_bindings(copy, &bindings);
                self.set_static_machine_application_selected_machines(copy, &selected);
                self.set_static_machine_application_lifetimes(
                    copy,
                    declaration.lifetime_binding_declaration,
                    &lifetimes,
                );
                copy
            }

            pub fn replace_with_existing(
                &mut self,
                destination: ExpressionHandle,
                source: ExpressionHandle,
            ) {
                let component = if self.direct_pattern_component(destination)
                    == Some(language_core::PatternComponent::Case)
                    || self.direct_pattern_component(source)
                        == Some(language_core::PatternComponent::Case)
                {
                    language_core::PatternComponent::Case
                } else {
                    language_core::PatternComponent::Fields
                };
                let node = self.expression(source).clone();
                let authority = self.index_expression_authority(source).cloned();
                let source_index = super::source_span_index(source);
                let destination_index = super::source_span_index(destination);
                let occurrences = self.authored_selection_occurrences[source_index];
                let source_span = self.source_span(source);
                let materialization_source = self.materialization_source(source);
                let materialization_source = if materialization_source == destination {
                    self.insert_copy(materialization_source)
                } else {
                    materialization_source
                };
                let carrier = self.constructed_value_type_reference(source);
                *self.expression_mut(destination) = node;
                self.set_pattern_component(destination, component);
                self.set_source_span(destination, source_span);
                self.authored_selection_occurrences[destination_index] = occurrences;
                self.materialization_sources[destination_index] = materialization_source;
                if carrier.is_valid() {
                    self.set_constructed_value_type_reference(destination, carrier);
                } else if let Some(previous) = self
                    .constructed_value_type_handles
                    .get_mut(destination_index)
                {
                    *previous = Default::default();
                }
                if let Some(previous) = self
                    .construction_required_type_handles
                    .get_mut(destination_index)
                {
                    *previous = Default::default();
                }
                if let Some(mut authority) = authority {
                    if let Some(operation) = &mut authority.operation {
                        operation.expression = destination;
                    }
                    *self.index_expression_authority_mut(destination) = authority;
                } else if let Some(previous) = self
                    .index_expression_authority_handles
                    .get_mut(destination_index)
                {
                    *previous = Default::default();
                }
            }

            pub fn copy_from(
                &mut self,
                source: &ExpressionTable,
                expression: ExpressionHandle,
            ) -> ExpressionHandle {
                self.copy_subtree(CopySource::Table(source), expression, &retain_every_field)
            }

            pub fn insert_copy(&mut self, expression: ExpressionHandle) -> ExpressionHandle {
                self.copy_subtree(CopySource::Own, expression, &retain_every_field)
            }

            pub fn remap_symbols_in<'scope>(
                &mut self,
                root: ExpressionHandle,
                symbols: impl Into<crate::SymbolRebinding<'scope>>,
            ) {
                let symbols = symbols.into();
                let mut visited = Vec::new();
                self.remap_symbols_in_inner(root, symbols, &mut visited);
            }

            fn remap_symbols_in_inner(
                &mut self,
                root: ExpressionHandle,
                symbols: crate::SymbolRebinding<'_>,
                visited: &mut Vec<ExpressionHandle>,
            ) {
                if !root.is_valid() || visited.contains(&root) {
                    return;
                }
                visited.push(root);
                self.remap_symbols_in_inner(self.materialization_source(root), symbols, visited);

                let mut tests = self.executable_domain_tests(root).to_vec();
                for test in &mut tests {
                    self.remap_symbols_in_inner(test.subject, symbols, visited);
                    test.domain = remapped(test.domain, symbols);
                }
                if !tests.is_empty() {
                    self.replace_executable_domain_tests(root, tests);
                }

                if let Some(mut authority) = self.index_expression_authority(root).cloned() {
                    if let Some(operation) = &mut authority.operation {
                        if let Some(application) = &mut operation.application {
                            self.remap_static_argument_symbols(application, symbols.execution());
                        }
                        for symbol in [
                            &mut operation.provider,
                            &mut operation.operator,
                            &mut operation.algebra_trait,
                        ] {
                            if let Some((_, replacement)) =
                                symbols.iter().find(|(original, _)| original == symbol)
                            {
                                *symbol = *replacement;
                            }
                        }
                    }
                    *self.index_expression_authority_mut(root) = authority;
                }

                let node = self.expression(root).clone();
                match node {
                    ExpressionNode::Match(dispatch) => {
                        self.remap_symbols_in_inner(dispatch.subject, symbols, visited);
                        let arms = self.match_arms(dispatch.arms).to_vec();
                        for arm in arms {
                            for pattern in self
                                .match_pattern_dependencies(arm.pattern)
                                .collect::<Vec<_>>()
                            {
                                self.remap_symbols_in_inner(pattern, symbols, visited);
                            }
                            if let MatchPattern::Destructure(pattern) = arm.pattern {
                                for position in 0..pattern.fields.count() {
                                    let field =
                                        self.match_field_mut(pattern.fields.handle_at(position));
                                    field.field_symbol = remapped(field.field_symbol, symbols);
                                }
                            }
                            self.remap_symbols_in_inner(arm.value, symbols, visited);
                        }
                    }
                    ExpressionNode::SavedSubject(_) => {}
                    ExpressionNode::ArrayLiteral(values) => {
                        let children = self.expression_handles(values).to_vec();
                        for child in children {
                            self.remap_symbols_in_inner(child, symbols, visited);
                        }
                    }
                    ExpressionNode::Atomic(atomic) => {
                        self.remap_symbols_in_inner(atomic.value, symbols, visited);
                        self.remap_symbols_in_inner(atomic.result, symbols, visited);
                    }
                    ExpressionNode::Binary(binary) => {
                        self.remap_symbols_in_inner(binary.left, symbols, visited);
                        self.remap_symbols_in_inner(binary.right, symbols, visited);
                    }
                    ExpressionNode::Cast(cast) => {
                        self.remap_symbols_in_inner(cast.value, symbols, visited)
                    }
                    ExpressionNode::Call(call) => {
                        self.remap_symbols_in_inner(call.receiver, symbols, visited);
                        self.remap_call_static_applications(
                            call.authored_machine_application,
                            call.static_requirement_dispatch.as_ref(),
                            symbols,
                        );
                        let arguments = self.expression_handles(call.arguments).to_vec();
                        for argument in arguments {
                            self.remap_symbols_in_inner(argument, symbols, visited);
                        }
                        let mut call = call;
                        call.target_symbol = remapped(call.target_symbol, symbols);
                        if let Some(binding) = &mut call.authored_call_binding {
                            binding.selected = remapped(binding.selected, symbols);
                        }
                        call.visit_static_arguments_mut(|argument| {
                            self.remap_static_argument_symbols(argument, symbols.execution())
                        });
                        *self.expression_mut(root) = ExpressionNode::Call(call);
                    }
                    ExpressionNode::Indexed(indexed) => {
                        self.remap_symbols_in_inner(indexed.collection, symbols, visited);
                        self.remap_symbols_in_inner(indexed.index, symbols, visited);
                    }
                    ExpressionNode::Member(member) => {
                        self.remap_symbols_in_inner(member.receiver, symbols, visited);
                        let ExpressionNode::Member(member) = self.expression_mut(root) else {
                            unreachable!();
                        };
                        member.member_symbol = remapped(member.member_symbol, symbols);
                    }
                    ExpressionNode::Borrow(inner) => {
                        self.remap_symbols_in_inner(inner.target, symbols, visited)
                    }
                    ExpressionNode::Name(path) => {
                        let ExpressionNode::Name(current) = self.expression_mut(root) else {
                            unreachable!();
                        };
                        current.head_symbol = remapped(path.head_symbol, symbols);
                        current.symbol = remapped(path.symbol, symbols);
                        for member_symbol in self
                            .name_path_member_symbols
                            .span_mut_or_empty(path.member_symbols)
                        {
                            *member_symbol = remapped(*member_symbol, symbols);
                        }
                    }
                    ExpressionNode::Range(range) => {
                        self.remap_symbols_in_inner(range.start, symbols, visited);
                        self.remap_symbols_in_inner(range.end, symbols, visited);
                    }
                    ExpressionNode::StructLiteral(literal) => {
                        let children: Vec<_> = self
                            .struct_fields(literal.fields)
                            .iter()
                            .map(|field| field.value)
                            .collect();
                        for child in children {
                            self.remap_symbols_in_inner(child, symbols, visited);
                        }
                    }
                    ExpressionNode::Unary(unary) => {
                        self.remap_symbols_in_inner(unary.operand, symbols, visited)
                    }
                    ExpressionNode::TypeExpression(_)
                    | crate::spelled_literal_expression_pattern!() => {}
                }
            }

            pub fn copy_expression_handles_from_slice(
                &mut self,
                source: &ExpressionTable,
                expressions: &[ExpressionHandle],
            ) -> HandleSpan<ExpressionHandle> {
                self.copy_expression_list(
                    CopySource::Table(source),
                    expressions,
                    &retain_every_field,
                )
            }

            pub(crate) fn copy_call_operands_from(
                &mut self,
                source: &ExpressionTable,
                expressions: &[ExpressionHandle],
                application: crate::static_applications::StaticMachineApplicationHandle,
                dispatch: Option<crate::StaticRequirementDispatch>,
            ) -> (
                HandleSpan<ExpressionHandle>,
                Vec<(Handle<CompletedValueCapture>, ValueCaptureOwner)>,
                crate::static_applications::StaticMachineApplicationHandle,
                Option<crate::StaticRequirementDispatch>,
            ) {
                let mut copies = CopyGraph::default();
                let expressions = self.copy_expression_list_with_occurrences(
                    CopySource::Table(source),
                    expressions,
                    &retain_every_field,
                    &mut copies,
                );
                let (application, dispatch) = self.copy_call_static_applications(
                    CopySource::Table(source),
                    application,
                    dispatch,
                    &mut copies,
                );
                (
                    expressions,
                    copies
                        .subjects
                        .into_iter()
                        .map(|(original, capture)| (capture, source.value_capture(original).owner))
                        .collect(),
                    application,
                    dispatch,
                )
            }

            pub fn relocate_statement_operand_captures(
                &mut self,
                source: crate::statement::StatementHandle,
                target: crate::statement::StatementHandle,
            ) {
                let captures = self
                    .completed_values
                    .iter()
                    .filter_map(|(capture, value)| {
                        matches!(value.owner,
                            ValueCaptureOwner::StatementArgument { call, .. } if call == source
                        )
                        .then_some((capture, value.owner))
                    })
                    .collect::<Vec<_>>();
                self.rebind_statement_operand_captures(&captures, source, target);
            }

            pub(crate) fn rebind_statement_operand_captures(
                &mut self,
                captures: &[(Handle<CompletedValueCapture>, ValueCaptureOwner)],
                source: crate::statement::StatementHandle,
                target: crate::statement::StatementHandle,
            ) {
                for &(capture, owner) in captures {
                    if let ValueCaptureOwner::StatementArgument { call, position } = owner
                        && call == source
                    {
                        self.completed_values.get_mut(capture).owner =
                            ValueCaptureOwner::StatementArgument {
                                call: target,
                                position,
                            };
                    }
                }
            }

            fn copy_subtree<RetainField>(
                &mut self,
                source: CopySource<'_>,
                expression: ExpressionHandle,
                retain: &RetainField,
            ) -> ExpressionHandle
            where
                RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
            {
                self.copy_subtree_with_occurrences(
                    source,
                    expression,
                    retain,
                    &mut CopyGraph::default(),
                )
            }

            fn copy_expression_list<RetainField>(
                &mut self,
                source: CopySource<'_>,
                expressions: &[ExpressionHandle],
                retain: &RetainField,
            ) -> HandleSpan<ExpressionHandle>
            where
                RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
            {
                self.copy_expression_list_with_occurrences(
                    source,
                    expressions,
                    retain,
                    &mut CopyGraph::default(),
                )
            }

            fn copy_subtree_with_occurrences<RetainField>(
                &mut self,
                source: CopySource<'_>,
                expression: ExpressionHandle,
                retain: &RetainField,
                copies: &mut CopyGraph,
            ) -> ExpressionHandle
            where
                RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
            {
                if !expression.is_valid() {
                    return expression;
                }
                if let Some((_, copied)) = copies
                    .expressions
                    .iter()
                    .find(|(original, _)| *original == expression)
                {
                    return *copied;
                }
                let origin = source.table(self);
                let source_span = origin.source_span(expression);
                let component = origin
                    .direct_pattern_component(expression)
                    .expect("copied expression retains its component row");
                let materialization_source = origin.materialization_source(expression);
                let authority = origin.index_expression_authority(expression).cloned();
                let domain_tests = origin.executable_domain_tests(expression).to_vec();
                let query_carrier =
                    matches!(origin.expression(expression), ExpressionNode::Call(call)
            if call.native_code_layout.is_some() || call.layout_query.is_some())
                    .then(|| origin.constructed_value_type_reference(expression));
                let occurrences = origin
                    .authored_selection_occurrences(expression)
                    .collect::<Vec<_>>();
                let copied = match origin.expression(expression).clone() {
                    ExpressionNode::Match(dispatch) => {
                        let subject = self.copy_subtree_with_occurrences(
                            source,
                            dispatch.subject,
                            retain,
                            copies,
                        );
                        let capture =
                            self.copy_match_subject(source, dispatch.capture, retain, copies);
                        let source_arms = source.table(self).match_arms(dispatch.arms).to_vec();
                        let mut arms = Vec::with_capacity(source_arms.len());
                        for arm in source_arms {
                            let pattern = match arm.pattern {
                                MatchPattern::Value(value) => {
                                    MatchPattern::Value(self.copy_subtree_with_occurrences(
                                        source, value, retain, copies,
                                    ))
                                }
                                MatchPattern::Wildcard => MatchPattern::Wildcard,
                                MatchPattern::Destructure(pattern) => MatchPattern::Destructure(
                                    self.copy_destructure_pattern(source, pattern, retain, copies),
                                ),
                            };
                            let value = self
                                .copy_subtree_with_occurrences(source, arm.value, retain, copies);
                            arms.push(TableMatchArm {
                                pattern,
                                value,
                                source_span: arm.source_span,
                            });
                        }
                        let arms = self.insert_match_arms(arms);
                        self.insert(ExpressionNode::Match(TableMatchExpression {
                            subject,
                            capture,
                            arms,
                        }))
                    }
                    ExpressionNode::SavedSubject(saved) => {
                        let copied_capture =
                            copies.subjects.iter().find_map(|(original, copied)| {
                                (*original == saved.capture).then_some(*copied)
                            });
                        if copied_capture.is_none() && matches!(source, CopySource::Own) {
                            self.insert(ExpressionNode::SavedSubject(saved))
                        } else {
                            let capture =
                                self.copy_match_subject(source, saved.capture, retain, copies);
                            let pattern_head = self.copy_subtree_with_occurrences(
                                source,
                                saved.pattern_head,
                                retain,
                                copies,
                            );
                            self.insert(ExpressionNode::SavedSubject(super::TableSavedSubject {
                                component: saved.component,
                                capture,
                                pattern_head,
                            }))
                        }
                    }
                    ExpressionNode::ArrayLiteral(values) => {
                        let values = self.copy_expression_span(source, values, retain, copies);
                        self.insert(ExpressionNode::ArrayLiteral(values))
                    }
                    ExpressionNode::Atomic(atomic) => {
                        let value = self.copy_subtree_with_occurrences(
                            source,
                            atomic.value,
                            retain,
                            copies,
                        );
                        let result =
                            self.copy_optional_subtree(source, atomic.result, retain, copies);
                        self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                            value,
                            result,
                            ordering: atomic.ordering,
                            result_custody: atomic.result_custody,
                        }))
                    }
                    ExpressionNode::Binary(binary) => {
                        let left =
                            self.copy_subtree_with_occurrences(source, binary.left, retain, copies);
                        let right = self.copy_subtree_with_occurrences(
                            source,
                            binary.right,
                            retain,
                            copies,
                        );
                        self.insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: binary.policy,
                            left,
                            operator: binary.operator,
                            right,
                        }))
                    }
                    ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(value)),
                    ExpressionNode::Cast(cast) => {
                        let value =
                            self.copy_subtree_with_occurrences(source, cast.value, retain, copies);
                        let target_label = self.copy_name_path_members(source, cast.target_label);
                        let semantic_domain =
                            self.copy_name_path_members(source, cast.semantic_domain);
                        self.insert(ExpressionNode::Cast(TableCastExpression {
                            value,
                            target_type: cast.target_type,
                            result_type: cast.result_type,
                            target_label,
                            domain: cast.domain,
                            semantic_domain,
                            semantic_domain_arguments: cast.semantic_domain_arguments,
                            semantic_domain_symbol: cast.semantic_domain_symbol,
                            semantic_domain_id: cast.semantic_domain_id,
                            form: cast.form,
                        }))
                    }
                    ExpressionNode::Call(call) => {
                        let receiver =
                            self.copy_optional_subtree(source, call.receiver, retain, copies);
                        let arguments =
                            self.copy_expression_span(source, call.arguments, retain, copies);
                        let mut call = call;
                        if let Some(request) = call.layout_query.as_mut()
                            && request.nonempty_view.is_valid()
                        {
                            request.nonempty_view = self.copy_subtree_with_occurrences(
                                source,
                                request.nonempty_view,
                                retain,
                                copies,
                            );
                        }
                        call.visit_static_arguments_mut(|argument| {
                            *argument = match source {
                                CopySource::Own => self.copy_static_argument_from_self(argument),
                                CopySource::Table(table) => {
                                    self.copy_static_argument_from(table, argument)
                                }
                            };
                        });
                        let build_selection = source
                            .table(self)
                            .build_selection_request(call.build_selection)
                            .cloned()
                            .map(|request| self.insert_build_selection_request(request))
                            .unwrap_or_default();
                        let (authored_machine_application, static_requirement_dispatch) = self
                            .copy_call_static_applications(
                                source,
                                call.authored_machine_application,
                                call.static_requirement_dispatch,
                                copies,
                            );
                        self.insert(ExpressionNode::Call(TableCallExpression {
                            authored_call_selection: call.authored_call_selection,
                            authored_call_binding: call.authored_call_binding,
                            authored_machine_application,
                            requirement_application: call.requirement_application,
                            exact_coercion_target: call.exact_coercion_target,
                            receiver,
                            target_symbol: call.target_symbol,
                            target: call.target,
                            static_requirement_dispatch,
                            static_machine_parameter: call.static_machine_parameter,
                            machine_arguments: call.machine_arguments,
                            quotient_operation: call.quotient_operation,
                            private_layout_operation: call.private_layout_operation,
                            installed_interrupt_operation: call.installed_interrupt_operation,
                            layout_query: call.layout_query,
                            native_code_layout: call.native_code_layout,
                            build_selection,
                            arguments,
                            evidence_arguments: call.evidence_arguments,
                            operational_acknowledgement: call.operational_acknowledgement,
                        }))
                    }
                    ExpressionNode::Float(value) => self.insert(ExpressionNode::Float(value)),
                    ExpressionNode::UndeterminedNaN(format) => {
                        self.insert(ExpressionNode::UndeterminedNaN(format))
                    }
                    ExpressionNode::Indexed(indexed) => {
                        let collection = self.copy_subtree_with_occurrences(
                            source,
                            indexed.collection,
                            retain,
                            copies,
                        );
                        let index = self.copy_subtree_with_occurrences(
                            source,
                            indexed.index,
                            retain,
                            copies,
                        );
                        self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                            collection,
                            index,
                        }))
                    }
                    ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value)),
                    ExpressionNode::Member(member) => {
                        let receiver = self.copy_subtree_with_occurrences(
                            source,
                            member.receiver,
                            retain,
                            copies,
                        );
                        self.insert(ExpressionNode::Member(TableMemberExpression {
                            receiver,
                            member_symbol: member.member_symbol,
                            member: member.member,
                            case_variant: member.case_variant,
                        }))
                    }
                    ExpressionNode::Borrow(borrow) => {
                        let target = self.copy_subtree_with_occurrences(
                            source,
                            borrow.target,
                            retain,
                            copies,
                        );
                        self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                            target,
                            access: borrow.access,
                        }))
                    }
                    ExpressionNode::Name(path) => {
                        let members = self.copy_name_path_members(source, path.members);
                        let member_symbols =
                            self.copy_name_path_member_symbols(source, path.member_symbols);
                        self.insert(ExpressionNode::Name(TableNamePath {
                            members,
                            member_symbols,
                            head_symbol: path.head_symbol,
                            symbol: path.symbol,
                            reserved_result_of: path.reserved_result_of,
                        }))
                    }
                    ExpressionNode::Range(range) => {
                        let start = self.copy_optional_subtree(source, range.start, retain, copies);
                        let end = self.copy_optional_subtree(source, range.end, retain, copies);
                        self.insert(ExpressionNode::Range(TableRangeExpression {
                            start,
                            end,
                            end_inclusive: range.end_inclusive,
                        }))
                    }
                    ExpressionNode::StructLiteral(literal) => {
                        let fields =
                            self.copy_struct_literal_fields(source, &literal, retain, copies);
                        self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                            application_type: literal.application_type,
                            type_name: literal.type_name,
                            type_symbol: literal.type_symbol,
                            case_name: literal.case_name,
                            case_symbol: literal.case_symbol,
                            fields,
                        }))
                    }
                    ExpressionNode::String(value) => self.insert(ExpressionNode::String(value)),
                    ExpressionNode::Unary(unary) => {
                        let operand = self.copy_subtree_with_occurrences(
                            source,
                            unary.operand,
                            retain,
                            copies,
                        );
                        self.insert(ExpressionNode::Unary(TableUnaryExpression {
                            operator: unary.operator,
                            operand,
                        }))
                    }
                    ExpressionNode::TypeExpression(type_reference) => {
                        self.insert(ExpressionNode::TypeExpression(type_reference))
                    }
                    ExpressionNode::ZeroValue(type_reference) => {
                        self.insert(ExpressionNode::ZeroValue(type_reference))
                    }
                };
                self.set_source_span(copied, source_span);
                if let Some(carrier) = query_carrier {
                    self.set_constructed_value_type_reference(copied, carrier);
                }
                self.attach_authored_selection_occurrences(copied, occurrences);
                if let Some(mut authority) = authority {
                    if let Some(operation) = &mut authority.operation {
                        operation.expression = copied;
                        if let Some(argument) = &mut operation.application {
                            *argument = match source {
                                CopySource::Own => self.copy_static_argument_from_self(argument),
                                CopySource::Table(table) => {
                                    self.copy_static_argument_from(table, argument)
                                }
                            };
                        }
                    }
                    *self.index_expression_authority_mut(copied) = authority;
                }
                self.set_pattern_component(copied, component);
                copies.expressions.push((expression, copied));
                if materialization_source.is_valid() {
                    let authored = self.copy_subtree_with_occurrences(
                        source,
                        materialization_source,
                        retain,
                        copies,
                    );
                    self.retain_materialization_source(copied, authored);
                }
                for test in domain_tests {
                    let subject =
                        self.copy_subtree_with_occurrences(source, test.subject, retain, copies);
                    self.retain_executable_domain_test(
                        copied,
                        super::ExecutableDomainTest {
                            subject,
                            domain: test.domain,
                        },
                    );
                }
                for (original, capture) in &copies.subjects {
                    if let ValueCaptureOwner::ExpressionArgument { call, position } =
                        source.table(self).value_capture(*original).owner
                        && call == expression
                    {
                        self.completed_values.get_mut(*capture).owner =
                            ValueCaptureOwner::ExpressionArgument {
                                call: copied,
                                position,
                            };
                    }
                }
                copied
            }

            pub(crate) fn copy_transition_pattern_from(
                &mut self,
                source: &ExpressionTable,
                retained: crate::statement::TransitionPattern,
            ) -> crate::statement::TransitionPattern {
                let copies = &mut CopyGraph::default();
                let source = CopySource::Table(source);
                let subject = self.copy_subtree_with_occurrences(
                    source,
                    retained.subject,
                    &retain_every_field,
                    copies,
                );
                let pattern = self.copy_destructure_pattern(
                    source,
                    retained.pattern,
                    &retain_every_field,
                    copies,
                );
                crate::statement::TransitionPattern {
                    is_structural: retained.is_structural,
                    subject,
                    pattern,
                }
            }

            fn copy_match_subject<RetainField>(
                &mut self,
                source: CopySource<'_>,
                capture: Handle<CompletedValueCapture>,
                retain: &RetainField,
                copies: &mut CopyGraph,
            ) -> Handle<CompletedValueCapture>
            where
                RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
            {
                if !capture.is_valid() {
                    return capture;
                }
                if let Some((_, copied)) = copies
                    .subjects
                    .iter()
                    .find(|(original, _)| *original == capture)
                {
                    return *copied;
                }
                let captured = *source.table(self).value_capture(capture);
                let subject =
                    self.copy_subtree_with_occurrences(source, captured.subject, retain, copies);
                let copied = if matches!(captured.owner, ValueCaptureOwner::Match) {
                    self.insert_match_subject(subject)
                } else {
                    let owner = match captured.owner {
                        ValueCaptureOwner::ExpressionArgument { call, position } => {
                            let call = copies
                                .expressions
                                .iter()
                                .find(|(source, _)| *source == call)
                                .map(|(_, copied)| *copied)
                                .unwrap_or_else(ExpressionHandle::invalid);
                            ValueCaptureOwner::ExpressionArgument { call, position }
                        }
                        ValueCaptureOwner::StatementArgument { position, .. } => {
                            ValueCaptureOwner::StatementArgument {
                                call: crate::statement::StatementHandle::invalid(),
                                position,
                            }
                        }
                        owner => owner,
                    };
                    self.insert_call_operand_capture(subject, owner)
                };
                copies.subjects.push((capture, copied));
                copied
            }

            /// A record pattern's field roster: it names no expression, so
            /// the rows copy as they stand.
            pub fn copy_pattern_fields_from(
                &mut self,
                source: &ExpressionTable,
                fields: HandleSpan<super::TableMatchField>,
            ) -> HandleSpan<super::TableMatchField> {
                let fields = source.match_fields(fields).to_vec();
                self.insert_match_fields(fields)
            }

            fn copy_destructure_pattern<RetainField>(
                &mut self,
                source: CopySource<'_>,
                pattern: super::TableDestructurePattern,
                retain: &RetainField,
                copies: &mut CopyGraph,
            ) -> super::TableDestructurePattern
            where
                RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
            {
                let head = self.copy_subtree_with_occurrences(source, pattern.head, retain, copies);
                let mut fields = source.table(self).match_fields(pattern.fields).to_vec();
                for field in &mut fields {
                    if let super::MatchFieldAction::Equal(value) = &mut field.action {
                        *value = self.copy_subtree_with_occurrences(source, *value, retain, copies);
                    }
                }
                let fields = self.insert_match_fields(fields);
                let guard =
                    self.copy_subtree_with_occurrences(source, pattern.guard, retain, copies);
                let condition =
                    self.copy_subtree_with_occurrences(source, pattern.condition, retain, copies);
                let selectors = source
                    .table(self)
                    .match_proof_selectors(pattern.proof_selectors)
                    .to_vec();
                let proof_selectors = self.insert_match_proof_selectors(selectors);
                super::TableDestructurePattern {
                    application_type: pattern.application_type,
                    head,
                    fields,
                    has_rest: pattern.has_rest,
                    guard,
                    condition,
                    proof_selectors,
                }
            }

            fn copy_optional_subtree<RetainField>(
                &mut self,
                source: CopySource<'_>,
                expression: ExpressionHandle,
                retain: &RetainField,
                copies: &mut CopyGraph,
            ) -> ExpressionHandle
            where
                RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
            {
                if expression.is_valid() {
                    self.copy_subtree_with_occurrences(source, expression, retain, copies)
                } else {
                    ExpressionHandle::invalid()
                }
            }

            fn copy_expression_span<RetainField>(
                &mut self,
                source: CopySource<'_>,
                expressions: HandleSpan<ExpressionHandle>,
                retain: &RetainField,
                copies: &mut CopyGraph,
            ) -> HandleSpan<ExpressionHandle>
            where
                RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
            {
                let expressions = source.table(self).expression_handles(expressions).to_vec();
                self.copy_expression_list_with_occurrences(source, &expressions, retain, copies)
            }

            fn copy_expression_list_with_occurrences<RetainField>(
                &mut self,
                source: CopySource<'_>,
                expressions: &[ExpressionHandle],
                retain: &RetainField,
                copies: &mut CopyGraph,
            ) -> HandleSpan<ExpressionHandle>
            where
                RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
            {
                let copied = self.reserve_expression_handles(
                    expressions
                        .len()
                        .try_into()
                        .expect("expression handle span count overflow"),
                );

                for (offset, expression) in expressions.iter().enumerate() {
                    let expression =
                        self.copy_subtree_with_occurrences(source, *expression, retain, copies);
                    self.set_expression_handle_at_offset(
                        copied,
                        offset
                            .try_into()
                            .expect("expression handle span count overflow"),
                        expression,
                    );
                }

                copied
            }

            fn copy_name_path_members(
                &mut self,
                source: CopySource<'_>,
                members: HandleSpan<Identifier>,
            ) -> HandleSpan<Identifier> {
                let members = source.table(self).name_path_members(members).to_vec();
                let mut copied = HandleSpan::empty();

                for member in members {
                    self.name_path_members.append_to_span(&mut copied, member);
                }

                copied
            }

            fn copy_name_path_member_symbols(
                &mut self,
                source: CopySource<'_>,
                member_symbols: HandleSpan<SymbolHandle>,
            ) -> HandleSpan<SymbolHandle> {
                let member_symbols = source
                    .table(self)
                    .name_path_member_symbols(member_symbols)
                    .to_vec();
                let mut copied = HandleSpan::empty();

                for member_symbol in member_symbols {
                    self.name_path_member_symbols
                        .append_to_span(&mut copied, member_symbol);
                }

                copied
            }

            fn copy_struct_literal_fields<RetainField>(
                &mut self,
                source: CopySource<'_>,
                literal: &TableStructLiteral,
                retain: &RetainField,
                copies: &mut CopyGraph,
            ) -> HandleSpan<TableStructLiteralField>
            where
                RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
            {
                let retained = source
                    .table(self)
                    .struct_fields(literal.fields)
                    .iter()
                    .filter(|field| retain(literal, field))
                    .cloned()
                    .collect::<Vec<_>>();
                let copied = self.reserve_struct_fields(
                    retained
                        .len()
                        .try_into()
                        .expect("struct literal field span count overflow"),
                );

                for (offset, field) in retained.into_iter().enumerate() {
                    let value =
                        self.copy_subtree_with_occurrences(source, field.value, retain, copies);
                    self.set_struct_field_at_offset(
                        copied,
                        offset
                            .try_into()
                            .expect("struct literal field span count overflow"),
                        TableStructLiteralField {
                            name: field.name,
                            field_symbol: field.field_symbol,
                            value,
                        },
                    );
                }

                copied
            }
        }

        #[cfg(test)]
        mod materialization_tests {
            use super::*;

            /// A contract's reserved `result` is known by its owner identity,
            /// not its spelling, so a name rebuilt from an owned tree must
            /// keep that owner; an edited path is a different name and
            /// loses it.
            #[test]
            fn inserting_a_tree_keeps_the_reserved_result_owner() {
                use crate::expression::{Expression, NamePath};
                let owner = SymbolHandle::from_arena_index(7);
                let name = |text: &str| Identifier::from(text);
                let result =
                    NamePath::unresolved(vec![name("result")]).denoting_reserved_result_of(owner);
                let mut table = ExpressionTable::default();
                let stored = table.insert_tree(&Expression::Name(result.clone()));
                let ExpressionNode::Name(path) = table.expression(stored) else {
                    panic!("a name is stored as a name")
                };
                assert_eq!(path.reserved_result_of, owner);
                assert!(!path.symbol.is_valid());

                let plain = table.insert_tree(&Expression::Name(NamePath::unresolved(vec![name(
                    "result",
                )])));
                let ExpressionNode::Name(path) = table.expression(plain) else {
                    panic!("a name is stored as a name")
                };
                assert!(!path.reserved_result_of.is_valid());

                let mut edited = result;
                edited.push(name("field"));
                assert!(!edited.reserved_result_of().is_valid());
            }

            #[test]
            fn replacing_the_computation_retains_its_original_occurrence() {
                let mut table = ExpressionTable::default();
                let authored = table.insert(ExpressionNode::Boolean(false));
                let value = table.insert(ExpressionNode::Boolean(true));
                table.retain_materialization_source(value, authored);
                table.replace_with_existing(authored, value);
                let original = table.materialization_source(authored);
                assert!(original.is_valid() && original != authored);
                assert_eq!(table.expression(authored), &ExpressionNode::Boolean(true));
                assert_eq!(table.expression(original), &ExpressionNode::Boolean(false));
            }

            #[test]
            fn materialized_value_copy_retains_its_distinct_authored_computation() {
                let mut source = ExpressionTable::default();
                let authored = source.insert(ExpressionNode::Boolean(false));
                let value = source.insert(ExpressionNode::Boolean(true));
                source.retain_materialization_source(value, authored);
                let own = source.insert_copy(value);
                let mut destination = ExpressionTable::default();
                for _ in 0..8 {
                    destination.insert(ExpressionNode::Boolean(true));
                }
                let copied = destination.copy_from(&source, value);
                for (table, copied) in [(&source, own), (&destination, copied)] {
                    let authored = table.materialization_source(copied);
                    assert!(authored.is_valid() && authored != copied);
                    assert_eq!(table.expression(copied), &ExpressionNode::Boolean(true));
                    assert_eq!(table.expression(authored), &ExpressionNode::Boolean(false));
                }
            }
        }
    }
    pub(crate) mod tree_conversion {
        use crate::name::Identifier;
        use crate::values::expression::{
            Expression, ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern, NamePath,
            OwnedMatchPattern, StructLiteralField, TableAtomicExpression, TableBinaryExpression,
            TableBorrowExpression, TableCallExpression, TableCastExpression,
            TableIndexedExpression, TableMatchArm, TableMatchExpression, TableMemberExpression,
            TableNamePath, TableRangeExpression, TableStructLiteral, TableStructLiteralField,
            TableUnaryExpression,
        };
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        impl ExpressionTable {
            fn insert_name_path_members(&mut self, path: &NamePath) -> HandleSpan<Identifier> {
                let mut members = HandleSpan::empty();

                for member in path.members() {
                    self.name_path_members
                        .append_to_span(&mut members, member.clone());
                }

                members
            }

            fn insert_name_path_member_symbols(
                &mut self,
                path: &NamePath,
            ) -> HandleSpan<SymbolHandle> {
                let mut member_symbols = HandleSpan::empty();

                for member_symbol in path.member_symbols() {
                    self.name_path_member_symbols
                        .append_to_span(&mut member_symbols, *member_symbol);
                }

                member_symbols
            }

            fn insert_expression_handle_span_from_trees(
                &mut self,
                expressions: &[Expression],
            ) -> HandleSpan<ExpressionHandle> {
                let handles = self.reserve_expression_handles(
                    expressions
                        .len()
                        .try_into()
                        .expect("expression handle span count overflow"),
                );

                for (offset, expression) in expressions.iter().enumerate() {
                    let expression = self.insert_tree(expression);
                    self.set_expression_handle_at_offset(
                        handles,
                        offset
                            .try_into()
                            .expect("expression handle span count overflow"),
                        expression,
                    );
                }

                handles
            }

            fn insert_struct_field_span_from_tree(
                &mut self,
                fields: &[StructLiteralField],
            ) -> HandleSpan<TableStructLiteralField> {
                let field_span = self.reserve_struct_fields(
                    fields
                        .len()
                        .try_into()
                        .expect("struct literal field span count overflow"),
                );

                for (offset, field) in fields.iter().enumerate() {
                    let value = self.insert_tree(&field.value);
                    self.set_struct_field_at_offset(
                        field_span,
                        offset
                            .try_into()
                            .expect("struct literal field span count overflow"),
                        TableStructLiteralField {
                            name: field.name.clone(),
                            field_symbol: SymbolHandle::invalid(),
                            value,
                        },
                    );
                }

                field_span
            }

            pub fn insert_tree(&mut self, expression: &Expression) -> ExpressionHandle {
                match expression {
                    Expression::Match(dispatch) => {
                        let subject = self.insert_tree(&dispatch.subject);
                        let mut arms = Vec::with_capacity(dispatch.arms.len());
                        for arm in dispatch.arms.iter() {
                            let pattern = match &arm.pattern {
                                OwnedMatchPattern::Value(value) => {
                                    MatchPattern::Value(self.insert_tree(value))
                                }
                                OwnedMatchPattern::Wildcard => MatchPattern::Wildcard,
                            };
                            let value = self.insert_tree(&arm.value);
                            arms.push(TableMatchArm {
                                pattern,
                                value,
                                source_span: arm.source_span,
                            });
                        }
                        let arms = self.insert_match_arms(arms);
                        self.insert(ExpressionNode::Match(TableMatchExpression {
                            subject,
                            arms,
                            ..Default::default()
                        }))
                    }
                    Expression::ArrayLiteral(values) => {
                        let values = self.insert_expression_handle_span_from_trees(values);
                        self.insert(ExpressionNode::ArrayLiteral(values))
                    }
                    Expression::Atomic(atomic) => {
                        let value = self.insert_tree(&atomic.value);
                        let result = atomic
                            .result
                            .as_ref()
                            .map(|result| self.insert_tree(result))
                            .unwrap_or_else(ExpressionHandle::invalid);
                        self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                            value,
                            result,
                            ordering: atomic.ordering,
                            result_custody: atomic.result_custody,
                        }))
                    }
                    Expression::Binary(binary) => {
                        let left = self.insert_tree(&binary.left);
                        let right = self.insert_tree(&binary.right);
                        self.insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left,
                            operator: binary.operator,
                            right,
                        }))
                    }
                    Expression::Boolean(value) => self.insert(ExpressionNode::Boolean(*value)),
                    Expression::Cast(cast) => {
                        let value = self.insert_tree(&cast.value);
                        let target_label = self.insert_name_path_members(&cast.target_label);
                        self.insert(ExpressionNode::Cast(TableCastExpression {
                            value,
                            target_type: cast.target_type,
                            result_type: crate::types::TypeReferenceHandle::invalid(),
                            target_label,
                            domain: cast.domain,
                            semantic_domain: HandleSpan::empty(),
                            semantic_domain_arguments: HandleSpan::empty(),
                            semantic_domain_symbol: SymbolHandle::invalid(),
                            semantic_domain_id: language_semantics::SemanticDomainId::NULL,
                            form: cast.form,
                        }))
                    }
                    Expression::Call(call) => {
                        let receiver = call
                            .receiver
                            .as_ref()
                            .map(|receiver| self.insert_tree(receiver))
                            .unwrap_or_else(ExpressionHandle::invalid);
                        let arguments =
                            self.insert_expression_handle_span_from_trees(&call.arguments);
                        self.insert(ExpressionNode::Call(TableCallExpression {
                            authored_call_selection: call.authored_call_selection,
                            authored_call_binding: call.authored_call_binding,
                            authored_machine_application: Default::default(),
                            requirement_application: call.requirement_application,
                            exact_coercion_target: call.exact_coercion_target,
                            receiver,
                            target_symbol: call.target_symbol,
                            target: call.target.clone(),
                            static_requirement_dispatch: None,
                            static_machine_parameter: SymbolHandle::invalid(),
                            machine_arguments: Box::default(),
                            quotient_operation: None,
                            private_layout_operation: None,
                            installed_interrupt_operation: None,
                            layout_query: None,
                            native_code_layout: None,
                            build_selection: Default::default(),
                            arguments,
                            evidence_arguments: call.evidence_arguments.to_vec().into_boxed_slice(),
                            operational_acknowledgement: call.operational_acknowledgement,
                        }))
                    }
                    Expression::Float(value) => self.insert(ExpressionNode::Float(value.clone())),
                    Expression::UndeterminedNaN(format) => {
                        self.insert(ExpressionNode::UndeterminedNaN(*format))
                    }
                    Expression::Indexed(indexed) => {
                        let collection = self.insert_tree(&indexed.collection);
                        let index = self.insert_tree(&indexed.index);
                        self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                            collection,
                            index,
                        }))
                    }
                    Expression::Integer(value) => {
                        self.insert(ExpressionNode::Integer(value.clone()))
                    }
                    Expression::Member(member) => {
                        let receiver = self.insert_tree(&member.receiver);
                        self.insert(ExpressionNode::Member(TableMemberExpression {
                            receiver,
                            member_symbol: member.member_symbol,
                            member: member.member.clone(),
                            case_variant: member.case_variant.clone(),
                        }))
                    }
                    Expression::Borrow(inner_expression) => {
                        let target = self.insert_tree(&inner_expression.target);
                        self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                            target,
                            access: inner_expression.access,
                        }))
                    }
                    Expression::Name(path) => {
                        let members = self.insert_name_path_members(path);
                        let member_symbols = self.insert_name_path_member_symbols(path);
                        self.insert(ExpressionNode::Name(TableNamePath {
                            members,
                            member_symbols,
                            head_symbol: path.head_symbol(),
                            symbol: path.symbol(),
                            reserved_result_of: path.reserved_result_of(),
                        }))
                    }
                    Expression::Range(range) => {
                        let start = range
                            .start
                            .as_ref()
                            .map(|start| self.insert_tree(start))
                            .unwrap_or_else(ExpressionHandle::invalid);
                        let end = range
                            .end
                            .as_ref()
                            .map(|end| self.insert_tree(end))
                            .unwrap_or_else(ExpressionHandle::invalid);
                        self.insert(ExpressionNode::Range(TableRangeExpression {
                            start,
                            end,
                            end_inclusive: range.end_inclusive,
                        }))
                    }
                    Expression::StructLiteral(struct_literal) => {
                        let fields =
                            self.insert_struct_field_span_from_tree(&struct_literal.fields);
                        self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                            application_type: Default::default(),
                            type_name: struct_literal.type_name.clone(),
                            type_symbol: SymbolHandle::invalid(),
                            case_name: struct_literal.case_name.clone(),
                            case_symbol: None,
                            fields,
                        }))
                    }
                    Expression::String(value) => self.insert(ExpressionNode::String(value.clone())),
                    Expression::Unary(unary) => {
                        let operand = self.insert_tree(&unary.operand);
                        self.insert(ExpressionNode::Unary(TableUnaryExpression {
                            operator: unary.operator,
                            operand,
                        }))
                    }
                    Expression::TypeExpression(type_reference) => {
                        self.insert(ExpressionNode::TypeExpression(*type_reference))
                    }
                    Expression::ZeroValue(type_reference) => {
                        self.insert(ExpressionNode::ZeroValue(*type_reference))
                    }
                }
            }
        }
    }

    pub use table_copies::ExpressionCopy;

    pub use display::{display_call_label, display_match_pattern, display_name_path};
    pub use index_authority::{
        IndexExpressionAuthority, OpenIndexNormalization, OpenIndexOperationSelection,
    };

    pub type ExpressionHandle = Handle<ExpressionNode>;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    struct StoredAuthoredSelectionOccurrenceId(Option<AuthoredDeclarationSelectionOccurrenceId>);

    impl StoredAuthoredSelectionOccurrenceId {
        fn new(occurrence: AuthoredDeclarationSelectionOccurrenceId) -> Self {
            Self(Some(occurrence))
        }

        fn occurrence(self) -> AuthoredDeclarationSelectionOccurrenceId {
            self.0
                .expect("expression occurrence spans cannot contain the arena dummy sentinel")
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Expression {
        TypeExpression(crate::types::TypeReferenceHandle),
        Match(Box<MatchExpression>),
        ArrayLiteral(Arc<[Expression]>),
        Atomic(Box<AtomicExpression>),
        Binary(Box<BinaryExpression>),
        Boolean(bool),
        Cast(Box<CastExpression>),
        Call(Box<CallExpression>),
        Float(FloatLiteral),
        UndeterminedNaN(numerics::literals::FloatFormat),
        Indexed(Box<IndexedExpression>),
        Integer(IntegerLiteral),
        Member(Box<MemberExpression>),
        Borrow(Box<BorrowExpression>),
        Name(NamePath),
        Range(Box<RangeExpression>),
        StructLiteral(StructLiteral),
        String(source::SourceBytes),
        Unary(Box<UnaryExpression>),
        ZeroValue(crate::types::TypeReferenceHandle),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BorrowExpression {
        pub target: Expression,
        pub access: language_core::ReferenceAccess,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AtomicExpression {
        pub value: Expression,
        pub result: Option<Expression>,
        pub ordering: language_core::atomic::AtomicOrderingPlan,
        pub result_custody: language_core::atomic::AtomicExpressionResultCustody,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ExecutableDomainTest {
        pub subject: ExpressionHandle,
        pub domain: SymbolHandle,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ExpressionTable {
        expressions: Arena<ExpressionNode>,
        build_selection_requests: Arena<BuildSelectionRequest>,
        expression_roots: Arena<ExpressionHandle>,
        source_spans: Vec<SourceSpan>,
        pattern_components: Vec<language_core::PatternComponent>,
        /// Original checked computation of a materialized static value. This
        /// provenance edge preserves invocation custody without executing it again.
        materialization_sources: Vec<ExpressionHandle>,

        constructed_value_types: Arena<crate::types::TypeReferenceHandle>,
        constructed_value_type_handles: Vec<Handle<crate::types::TypeReferenceHandle>>,
        construction_required_types: Arena<crate::types::TypeReferenceHandle>,
        construction_required_type_handles: Vec<Handle<crate::types::TypeReferenceHandle>>,
        index_expression_authorities: Arena<IndexExpressionAuthority>,
        index_expression_authority_handles: Vec<Handle<IndexExpressionAuthority>>,
        authored_selection_occurrences: Vec<HandleSpan<StoredAuthoredSelectionOccurrenceId>>,
        authored_selection_occurrence_ids: Arena<StoredAuthoredSelectionOccurrenceId>,
        executable_domain_tests: Vec<HandleSpan<ExecutableDomainTest>>,
        executable_domain_test_rows: Arena<ExecutableDomainTest>,
        expression_handles: Arena<ExpressionHandle>,
        name_path_members: Arena<Identifier>,
        name_path_member_symbols: Arena<SymbolHandle>,
        struct_fields: Arena<TableStructLiteralField>,
        match_arms: Arena<TableMatchArm>,
        match_fields: Arena<TableMatchField>,
        completed_values: Arena<CompletedValueCapture>,
        match_proof_selectors: Arena<crate::statement::OutcomeProofSelector>,
        static_machine_applications: Arena<StaticMachineApplication>,
        static_machine_arguments: Arena<StaticMachineArgument>,
        static_machine_bindings: Arena<StaticMachineBinding>,
        static_machine_lifetime_bindings: Arena<StaticMachineLifetimeBinding>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct ExpressionTableCapacity {
        pub expressions: usize,
        pub authored_selection_occurrences: usize,
        pub expression_handles: usize,
        pub name_path_members: usize,
        pub name_path_member_symbols: usize,
        pub struct_fields: usize,
        pub match_arms: usize,
        pub match_fields: usize,
        pub completed_values: usize,
        pub match_proof_selectors: usize,
    }

    impl ExpressionTableCapacity {}

    macro_rules! expression_node_accessors {
    ($($(#[$doc:meta])* $name:ident($variant:ident) -> $node_type:ty;)*) => {
        $(
            $(#[$doc])*
            pub fn $name(&self, handle: ExpressionHandle) -> Option<&$node_type> {
                match self.expression(handle) {
                    ExpressionNode::$variant(node) => Some(node),
                    _ => None,
                }
            }
        )*
    };
}

    impl ExpressionTable {
        pub fn insert_static_member(
            &mut self,
            member: StaticMachineArgument,
        ) -> Handle<StaticMachineArgument> {
            self.static_machine_arguments.insert(member)
        }

        pub fn static_member(
            &self,
            member: Handle<StaticMachineArgument>,
        ) -> Option<&StaticMachineArgument> {
            self.static_machine_arguments
                .is_valid(member)
                .then(|| self.static_machine_arguments.get(member))
        }

        pub fn static_member_mut(
            &mut self,
            member: Handle<StaticMachineArgument>,
        ) -> Option<&mut StaticMachineArgument> {
            if self.static_machine_arguments.is_valid(member) {
                Some(self.static_machine_arguments.get_mut(member))
            } else {
                None
            }
        }

        pub fn insert_match_arms(
            &mut self,
            arms: impl IntoIterator<Item = TableMatchArm>,
        ) -> HandleSpan<TableMatchArm> {
            self.match_arms.insert_many(arms)
        }

        pub fn match_arms(&self, arms: HandleSpan<TableMatchArm>) -> &[TableMatchArm] {
            self.match_arms.span_or_empty(arms)
        }

        pub fn insert_match_fields(
            &mut self,
            fields: impl IntoIterator<Item = TableMatchField>,
        ) -> HandleSpan<TableMatchField> {
            self.match_fields.insert_many(fields)
        }

        pub fn match_fields(&self, fields: HandleSpan<TableMatchField>) -> &[TableMatchField] {
            self.match_fields.span_or_empty(fields)
        }

        pub fn match_field_mut(&mut self, field: Handle<TableMatchField>) -> &mut TableMatchField {
            self.match_fields.get_mut(field)
        }

        pub fn insert_match_subject(
            &mut self,
            subject: ExpressionHandle,
        ) -> Handle<CompletedValueCapture> {
            self.completed_values.insert(CompletedValueCapture {
                subject,
                owner: ValueCaptureOwner::Match,
                definition: ExpressionHandle::invalid(),
                value_anchor: ExpressionHandle::invalid(),
            })
        }

        pub fn insert_call_operand_capture(
            &mut self,
            subject: ExpressionHandle,
            owner: ValueCaptureOwner,
        ) -> Handle<CompletedValueCapture> {
            self.completed_values.insert(CompletedValueCapture {
                subject,
                owner,
                value_anchor: subject,
                ..Default::default()
            })
        }

        pub fn value_capture(
            &self,
            capture: Handle<CompletedValueCapture>,
        ) -> &CompletedValueCapture {
            self.completed_values.get(capture)
        }

        pub fn contains_value_capture(&self, capture: Handle<CompletedValueCapture>) -> bool {
            self.completed_values.is_valid(capture)
        }

        pub fn set_value_anchor(
            &mut self,
            capture: Handle<CompletedValueCapture>,
            anchor: ExpressionHandle,
        ) {
            self.completed_values.get_mut(capture).value_anchor = anchor;
        }

        pub fn saved_subject_source(&self, saved: &TableSavedSubject) -> Option<ExpressionHandle> {
            if !self.contains_value_capture(saved.capture) {
                return None;
            }
            let capture = self.value_capture(saved.capture);
            if !self.expression_is_valid(capture.definition) {
                return None;
            }
            let ExpressionNode::Match(dispatch) = self.expression(capture.definition) else {
                return None;
            };
            (dispatch.capture == saved.capture
                && dispatch.subject == capture.subject
                && self.expression_is_valid(capture.subject)
                && self.match_arms(dispatch.arms).iter().any(|arm| {
                    matches!(arm.pattern, MatchPattern::Destructure(pattern)
                    if pattern.head == saved.pattern_head
                    && (saved.component == language_core::PatternComponent::Fields
                        || self.match_fields(pattern.fields).iter().any(|field|
                            field.component == language_core::PatternComponent::Case
                                && matches!(field.action, MatchFieldAction::Bind(_)))))
                }))
            .then_some(capture.subject)
        }

        pub fn insert_match_proof_selectors(
            &mut self,
            selectors: impl IntoIterator<Item = crate::statement::OutcomeProofSelector>,
        ) -> HandleSpan<crate::statement::OutcomeProofSelector> {
            self.match_proof_selectors.insert_many(selectors)
        }

        pub fn match_proof_selectors(
            &self,
            selectors: HandleSpan<crate::statement::OutcomeProofSelector>,
        ) -> &[crate::statement::OutcomeProofSelector] {
            self.match_proof_selectors.span_or_empty(selectors)
        }

        pub fn match_pattern_dependencies(
            &self,
            pattern: MatchPattern,
        ) -> impl Iterator<Item = ExpressionHandle> + '_ {
            let (head, fields, guard) = match pattern {
                MatchPattern::Value(value) => {
                    (value, HandleSpan::empty(), ExpressionHandle::invalid())
                }
                MatchPattern::Destructure(pattern) => {
                    (pattern.head, HandleSpan::empty(), pattern.condition)
                }
                MatchPattern::Wildcard => (
                    ExpressionHandle::invalid(),
                    HandleSpan::empty(),
                    ExpressionHandle::invalid(),
                ),
            };
            head.is_valid()
                .then_some(head)
                .into_iter()
                .chain(
                    self.match_fields(fields)
                        .iter()
                        .filter_map(|field| match &field.action {
                            MatchFieldAction::Equal(value) => Some(*value),
                            _ => None,
                        }),
                )
                .chain(guard.is_valid().then_some(guard))
        }

        pub fn match_pattern_operands(
            &self,
            pattern: MatchPattern,
        ) -> impl Iterator<Item = ExpressionHandle> + '_ {
            let value = match pattern {
                MatchPattern::Value(value) => value,
                MatchPattern::Destructure(pattern) => pattern.condition,
                MatchPattern::Wildcard => ExpressionHandle::invalid(),
            };
            value.is_valid().then_some(value).into_iter()
        }

        pub fn match_arms_all(
            &self,
            arms: HandleSpan<TableMatchArm>,
            mut visit: impl FnMut(MatchPattern, ExpressionHandle) -> bool,
        ) -> bool {
            self.match_arms(arms)
                .iter()
                .all(|arm| visit(arm.pattern, arm.value))
        }

        pub fn match_deep_all(
            &self,
            dispatch: &TableMatchExpression,
            mut visit: impl FnMut(ExpressionHandle) -> bool,
        ) -> bool {
            visit(dispatch.subject)
                && self.match_arms_all(dispatch.arms, |pattern, value| {
                    self.match_pattern_dependencies(pattern).all(&mut visit) && visit(value)
                })
        }

        pub fn match_deep_any(
            &self,
            dispatch: &TableMatchExpression,
            mut visit: impl FnMut(ExpressionHandle) -> bool,
        ) -> bool {
            visit(dispatch.subject)
                || self.match_arms(dispatch.arms).iter().any(|arm| {
                    self.match_pattern_dependencies(arm.pattern).any(&mut visit) || visit(arm.value)
                })
        }

        pub fn match_pending(&self, dispatch: &TableMatchExpression) -> Vec<ExpressionHandle> {
            let mut pending = Vec::new();
            for arm in self.match_arms(dispatch.arms).iter().rev() {
                pending.push(arm.value);
                pending.extend(self.match_pattern_dependencies(arm.pattern));
            }
            pending.push(dispatch.subject);
            pending
        }

        pub fn new() -> Self {
            Self::with_expression_capacity(0)
        }

        pub fn revision(&self) -> u64 {
            self.expressions
                .revision()
                .wrapping_add(self.build_selection_requests.revision())
                .wrapping_add(self.expression_roots.revision())
                .wrapping_add(self.index_expression_authorities.revision())
                .wrapping_add(self.authored_selection_occurrence_ids.revision())
                .wrapping_add(self.executable_domain_test_rows.revision())
                .wrapping_add(self.expression_handles.revision())
                .wrapping_add(self.name_path_members.revision())
                .wrapping_add(self.name_path_member_symbols.revision())
                .wrapping_add(self.struct_fields.revision())
                .wrapping_add(self.match_arms.revision())
                .wrapping_add(self.match_fields.revision())
                .wrapping_add(self.completed_values.revision())
                .wrapping_add(self.match_proof_selectors.revision())
                .wrapping_add(self.static_machine_applications.revision())
                .wrapping_add(self.static_machine_arguments.revision())
                .wrapping_add(self.constructed_value_types.revision())
                .wrapping_add(self.construction_required_types.revision())
                .wrapping_add(self.static_machine_bindings.revision())
                .wrapping_add(self.static_machine_lifetime_bindings.revision())
                .wrapping_add(self.source_spans.len() as u64)
                .wrapping_add(self.authored_selection_occurrences.len() as u64)
                .wrapping_add(self.executable_domain_tests.len() as u64)
        }

        pub fn with_expression_capacity(expression_capacity: usize) -> Self {
            Self::with_expression_and_handle_capacity(expression_capacity, 0)
        }

        pub fn with_expression_and_handle_capacity(
            expression_capacity: usize,
            expression_handle_capacity: usize,
        ) -> Self {
            Self::with_capacities(ExpressionTableCapacity {
                expressions: expression_capacity,
                expression_handles: expression_handle_capacity,
                ..ExpressionTableCapacity::default()
            })
        }

        pub fn with_capacities(capacity: ExpressionTableCapacity) -> Self {
            Self {
                expressions: Arena::with_capacity(capacity.expressions),
                build_selection_requests: Arena::default(),
                expression_roots: Arena::with_capacity(capacity.expressions),
                source_spans: Vec::with_capacity(capacity.expressions),
                pattern_components: Vec::with_capacity(capacity.expressions),
                materialization_sources: Vec::new(),
                constructed_value_types: Arena::default(),
                constructed_value_type_handles: Vec::new(),
                construction_required_types: Arena::default(),
                construction_required_type_handles: Vec::new(),
                index_expression_authorities: Arena::default(),
                index_expression_authority_handles: Vec::new(),
                authored_selection_occurrences: Vec::with_capacity(capacity.expressions),
                authored_selection_occurrence_ids: Arena::with_capacity(
                    capacity.authored_selection_occurrences,
                ),
                executable_domain_tests: Vec::with_capacity(capacity.expressions),
                executable_domain_test_rows: Arena::default(),
                expression_handles: Arena::with_capacity(capacity.expression_handles),
                name_path_members: Arena::with_capacity(capacity.name_path_members),
                name_path_member_symbols: Arena::with_capacity(capacity.name_path_member_symbols),
                struct_fields: Arena::with_capacity(capacity.struct_fields),
                match_arms: Arena::with_capacity(capacity.match_arms),
                match_fields: Arena::with_capacity(capacity.match_fields),
                completed_values: Arena::with_capacity(capacity.completed_values),
                match_proof_selectors: Arena::with_capacity(capacity.match_proof_selectors),
                static_machine_applications: Arena::default(),
                static_machine_arguments: Arena::default(),
                static_machine_bindings: Arena::default(),
                static_machine_lifetime_bindings: Arena::default(),
            }
        }

        pub fn clear(&mut self) {
            self.expressions.reset_retain_capacity();
            self.build_selection_requests.reset_retain_capacity();
            self.expression_roots.reset_retain_capacity();
            self.source_spans.clear();
            self.materialization_sources.clear();
            self.constructed_value_types.reset_retain_capacity();
            self.constructed_value_type_handles.clear();
            self.construction_required_types.reset_retain_capacity();
            self.construction_required_type_handles.clear();
            self.index_expression_authorities.reset_retain_capacity();
            self.index_expression_authority_handles.clear();
            self.authored_selection_occurrences.clear();
            self.authored_selection_occurrence_ids
                .reset_retain_capacity();
            self.executable_domain_tests.clear();
            self.executable_domain_test_rows.reset_retain_capacity();
            self.expression_handles.reset_retain_capacity();
            self.name_path_members.reset_retain_capacity();
            self.name_path_member_symbols.reset_retain_capacity();
            self.struct_fields.reset_retain_capacity();
            self.match_arms.reset_retain_capacity();
            self.match_fields.reset_retain_capacity();
            self.completed_values.reset_retain_capacity();
            self.match_proof_selectors.reset_retain_capacity();
            self.static_machine_applications.reset_retain_capacity();
            self.static_machine_arguments.reset_retain_capacity();
            self.static_machine_bindings.reset_retain_capacity();
            self.static_machine_lifetime_bindings
                .reset_retain_capacity();
        }

        pub fn retain_static_machine_application(
            &mut self,
            registration_operation: SymbolHandle,
            target: Identifier,
            arguments: &[StaticMachineArgument],
        ) -> StaticMachineApplicationHandle {
            let arguments = self
                .static_machine_arguments
                .insert_many(arguments.iter().cloned());
            self.static_machine_applications
                .insert(StaticMachineApplication {
                    callable_contract_source: StaticMachineApplicationHandle::invalid(),
                    registration_operation,
                    lexical_owner: SymbolHandle::invalid(),
                    target,
                    arguments,
                    input_selections: HandleSpan::empty(),
                    selected_machine_arguments: HandleSpan::empty(),
                    bindings: HandleSpan::empty(),
                    lifetime_bindings: HandleSpan::empty(),
                    lifetime_binding_declaration: SymbolHandle::invalid(),
                    binding_declaration: SymbolHandle::invalid(),
                    conformance_selection: None,
                })
        }

        pub fn static_machine_applications(
            &self,
        ) -> impl Iterator<Item = (StaticMachineApplicationHandle, &StaticMachineApplication)>
        {
            self.static_machine_applications.iter()
        }

        pub fn static_machine_application(
            &self,
            application: StaticMachineApplicationHandle,
        ) -> &StaticMachineApplication {
            self.static_machine_applications.get(application)
        }

        pub fn static_machine_application_mut(
            &mut self,
            application: StaticMachineApplicationHandle,
        ) -> &mut StaticMachineApplication {
            self.static_machine_applications.get_mut(application)
        }

        pub fn static_machine_application_bindings(
            &self,
            application: StaticMachineApplicationHandle,
        ) -> &[StaticMachineBinding] {
            self.static_machine_bindings
                .span_or_empty(self.static_machine_application(application).bindings)
        }

        pub fn snapshot_static_machine_bindings(
            &mut self,
            application: StaticMachineApplicationHandle,
        ) -> HandleSpan<StaticMachineBinding> {
            let mut bindings = self
                .static_machine_application_bindings(application)
                .to_vec();
            for binding in &mut bindings {
                if let crate::static_applications::StaticMachineBindingValue::Machine(argument)
                | crate::static_applications::StaticMachineBindingValue::Conformance(
                    argument,
                ) = &mut binding.value
                {
                    *argument = self.copy_static_argument_from_self(argument);
                }
            }
            self.static_machine_bindings.insert_many(bindings)
        }

        pub fn static_machine_binding_snapshot(
            &self,
            bindings: HandleSpan<StaticMachineBinding>,
        ) -> &[StaticMachineBinding] {
            self.static_machine_bindings.span_or_empty(bindings)
        }

        pub fn static_machine_application_bindings_mut(
            &mut self,
            application: StaticMachineApplicationHandle,
        ) -> &mut [StaticMachineBinding] {
            let bindings = self.static_machine_applications.get(application).bindings;
            self.static_machine_bindings.span_mut_or_empty(bindings)
        }

        pub fn set_static_machine_application_bindings(
            &mut self,
            application: StaticMachineApplicationHandle,
            bindings: &[StaticMachineBinding],
        ) {
            let previous = self.static_machine_applications.get(application).bindings;
            if self.static_machine_bindings.span_or_empty(previous) == bindings {
                return;
            }
            if previous.len() == bindings.len() {
                self.static_machine_bindings
                    .span_mut_or_empty(previous)
                    .clone_from_slice(bindings);
            } else {
                let bindings = self
                    .static_machine_bindings
                    .insert_many(bindings.iter().cloned());
                self.static_machine_applications
                    .get_mut(application)
                    .bindings = bindings;
            }
        }

        pub fn static_machine_application_arguments(
            &self,
            application: StaticMachineApplicationHandle,
        ) -> &[StaticMachineArgument] {
            self.static_machine_arguments
                .span_or_empty(self.static_machine_application(application).arguments)
        }

        pub fn static_machine_application_selected_machines(
            &self,
            application: StaticMachineApplicationHandle,
        ) -> &[StaticMachineArgument] {
            self.static_machine_arguments.span_or_empty(
                self.static_machine_application(application)
                    .selected_machine_arguments,
            )
        }

        pub fn static_machine_application_selected_machines_mut(
            &mut self,
            application: StaticMachineApplicationHandle,
        ) -> &mut [StaticMachineArgument] {
            let selected = self
                .static_machine_applications
                .get(application)
                .selected_machine_arguments;
            self.static_machine_arguments.span_mut_or_empty(selected)
        }

        pub fn set_static_machine_application_selected_machines(
            &mut self,
            application: StaticMachineApplicationHandle,
            selected: &[StaticMachineArgument],
        ) {
            if self.static_machine_application_selected_machines(application) == selected {
                return;
            }
            let selected = self
                .static_machine_arguments
                .insert_many(selected.iter().cloned());
            self.static_machine_applications
                .get_mut(application)
                .selected_machine_arguments = selected;
        }

        pub fn static_machine_application_lifetimes(
            &self,
            application: StaticMachineApplicationHandle,
        ) -> &[StaticMachineLifetimeBinding] {
            self.static_machine_lifetime_bindings.span_or_empty(
                self.static_machine_application(application)
                    .lifetime_bindings,
            )
        }

        pub fn set_static_machine_application_lifetimes(
            &mut self,
            application: StaticMachineApplicationHandle,
            declaration: SymbolHandle,
            bindings: &[StaticMachineLifetimeBinding],
        ) {
            self.static_machine_applications
                .get_mut(application)
                .lifetime_binding_declaration = declaration;
            let previous = self
                .static_machine_applications
                .get(application)
                .lifetime_bindings;
            if self
                .static_machine_lifetime_bindings
                .span_or_empty(previous)
                == bindings
            {
                return;
            }
            if previous.len() == bindings.len() {
                self.static_machine_lifetime_bindings
                    .span_mut_or_empty(previous)
                    .clone_from_slice(bindings);
            } else {
                let bindings = self
                    .static_machine_lifetime_bindings
                    .insert_many(bindings.iter().cloned());
                self.static_machine_applications
                    .get_mut(application)
                    .lifetime_bindings = bindings;
            }
        }

        pub fn static_machine_application_arguments_mut(
            &mut self,
            application: StaticMachineApplicationHandle,
        ) -> &mut [StaticMachineArgument] {
            let arguments = self.static_machine_applications.get(application).arguments;
            self.static_machine_arguments.span_mut_or_empty(arguments)
        }

        pub fn direct_pattern_component(
            &self,
            expression: ExpressionHandle,
        ) -> Option<language_core::PatternComponent> {
            self.expression_is_valid(expression)
                .then(|| {
                    self.pattern_components
                        .get(source_span_index(expression))
                        .copied()
                })
                .flatten()
        }

        /// A component belongs to the produced occurrence, not the current node
        /// shape. Replacing a name by its captured place must retain this view.
        pub fn set_pattern_component(
            &mut self,
            expression: ExpressionHandle,
            component: language_core::PatternComponent,
        ) {
            let slot = &mut self.pattern_components[source_span_index(expression)];
            if *slot != component {
                *slot = component;
                // Role changes invalidate the same derived readers as node changes.
                let _ = self.expressions.get_mut(expression);
            }
        }

        pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
            let component = match &expression {
                ExpressionNode::SavedSubject(saved) => saved.component,
                _ => language_core::PatternComponent::Fields,
            };
            let capture = match &expression {
                ExpressionNode::Match(dispatch) => dispatch.capture,
                _ => Handle::invalid(),
            };
            let alias_capture = match &expression {
                ExpressionNode::SavedSubject(saved) => saved.capture,
                _ => Handle::invalid(),
            };
            let handle = self.expressions.insert(expression);
            if self.completed_values.is_valid(alias_capture) {
                let capture = self.completed_values.get_mut(alias_capture);
                if !capture.value_anchor.is_valid() {
                    capture.value_anchor = handle;
                }
            }
            if self.completed_values.is_valid(capture) {
                let subject = self.completed_values.get_mut(capture);
                if !subject.definition.is_valid() {
                    subject.definition = handle;
                }
            }
            self.expression_roots.insert(handle);
            self.source_spans.push(SourceSpan::default());
            self.pattern_components.push(component);
            self.materialization_sources
                .push(ExpressionHandle::invalid());
            self.authored_selection_occurrences
                .push(HandleSpan::empty());
            self.executable_domain_tests.push(HandleSpan::empty());
            debug_assert_eq!(source_span_index(handle), self.source_spans.len() - 1);
            debug_assert_eq!(
                source_span_index(handle),
                self.authored_selection_occurrences.len() - 1
            );
            handle
        }

        pub fn materialization_source(&self, handle: ExpressionHandle) -> ExpressionHandle {
            self.materialization_sources
                .get(source_span_index(handle))
                .copied()
                .unwrap_or_default()
        }

        pub fn retain_materialization_source(
            &mut self,
            value: ExpressionHandle,
            authored: ExpressionHandle,
        ) {
            assert!(value.is_valid() && authored.is_valid() && value != authored);
            assert!(!self.materialization_source(value).is_valid());
            self.materialization_sources[source_span_index(value)] = authored;
        }

        pub fn source_span(&self, handle: ExpressionHandle) -> SourceSpan {
            self.source_spans[source_span_index(handle)]
        }

        pub fn try_source_span(&self, handle: ExpressionHandle) -> Option<SourceSpan> {
            if !self.expressions.is_valid(handle) {
                return None;
            }
            self.source_spans.get(source_span_index(handle)).copied()
        }

        pub fn insert_build_selection_request(
            &mut self,
            request: BuildSelectionRequest,
        ) -> Handle<BuildSelectionRequest> {
            self.build_selection_requests.insert(request)
        }

        pub fn build_selection_request(
            &self,
            request: Handle<BuildSelectionRequest>,
        ) -> Option<&BuildSelectionRequest> {
            self.build_selection_requests
                .is_valid(request)
                .then(|| self.build_selection_requests.get(request))
        }

        pub fn set_source_span(&mut self, handle: ExpressionHandle, source_span: SourceSpan) {
            self.source_spans[source_span_index(handle)] = source_span;
        }

        pub fn constructed_value_type_reference(
            &self,
            handle: ExpressionHandle,
        ) -> crate::types::TypeReferenceHandle {
            if !self.expression_is_valid(handle) {
                return crate::types::TypeReferenceHandle::invalid();
            }
            let Some(reference) = self
                .constructed_value_type_handles
                .get(source_span_index(handle))
            else {
                return crate::types::TypeReferenceHandle::invalid();
            };
            if reference.is_valid() {
                *self.constructed_value_types.get(*reference)
            } else {
                crate::types::TypeReferenceHandle::invalid()
            }
        }

        pub fn set_constructed_value_type_reference(
            &mut self,
            handle: ExpressionHandle,
            type_reference: crate::types::TypeReferenceHandle,
        ) {
            let position = source_span_index(handle);
            if self.constructed_value_type_handles.len() <= position {
                self.constructed_value_type_handles
                    .resize(position + 1, Handle::invalid());
            }
            let reference = &mut self.constructed_value_type_handles[position];
            if reference.is_valid() {
                *self.constructed_value_types.get_mut(*reference) = type_reference;
            } else {
                *reference = self.constructed_value_types.insert(type_reference);
            }
        }

        pub fn construction_required_type_reference(
            &self,
            handle: ExpressionHandle,
        ) -> crate::types::TypeReferenceHandle {
            if !self.expression_is_valid(handle) {
                return crate::types::TypeReferenceHandle::invalid();
            }
            let Some(reference) = self
                .construction_required_type_handles
                .get(source_span_index(handle))
            else {
                return crate::types::TypeReferenceHandle::invalid();
            };
            if reference.is_valid() {
                *self.construction_required_types.get(*reference)
            } else {
                crate::types::TypeReferenceHandle::invalid()
            }
        }

        pub fn set_construction_required_type_reference(
            &mut self,
            handle: ExpressionHandle,
            type_reference: crate::types::TypeReferenceHandle,
        ) {
            let position = source_span_index(handle);
            if self.construction_required_type_handles.len() <= position {
                self.construction_required_type_handles
                    .resize(position + 1, Handle::invalid());
            }
            let reference = &mut self.construction_required_type_handles[position];
            if reference.is_valid() {
                *self.construction_required_types.get_mut(*reference) = type_reference;
            } else {
                *reference = self.construction_required_types.insert(type_reference);
            }
        }

        pub fn attach_authored_selection_occurrences(
            &mut self,
            handle: ExpressionHandle,
            occurrences: impl IntoIterator<Item = AuthoredDeclarationSelectionOccurrenceId>,
        ) {
            let index = source_span_index(handle);
            let mut combined = self
                .authored_selection_occurrences(handle)
                .collect::<Vec<_>>();

            for occurrence in occurrences {
                if !combined.contains(&occurrence) {
                    combined.push(occurrence);
                }
            }

            self.authored_selection_occurrences[index] =
                self.authored_selection_occurrence_ids.insert_many(
                    combined
                        .into_iter()
                        .map(StoredAuthoredSelectionOccurrenceId::new),
                );
        }

        /// Hands an expression's authored selection occurrences to the
        /// expression that now holds its operation, so a rewrite that wraps
        /// an authored operator keeps one owner for each occurrence.
        pub fn move_authored_selection_occurrences(
            &mut self,
            from: ExpressionHandle,
            to: ExpressionHandle,
        ) {
            let moved = std::mem::replace(
                &mut self.authored_selection_occurrences[source_span_index(from)],
                HandleSpan::empty(),
            );
            self.authored_selection_occurrences[source_span_index(to)] = moved;
        }

        pub fn authored_selection_occurrences(
            &self,
            handle: ExpressionHandle,
        ) -> impl ExactSizeIterator<Item = AuthoredDeclarationSelectionOccurrenceId> + '_ {
            self.authored_selection_occurrence_ids
                .span_or_empty(self.authored_selection_occurrences[source_span_index(handle)])
                .iter()
                .copied()
                .map(StoredAuthoredSelectionOccurrenceId::occurrence)
        }

        pub fn executable_domain_tests(
            &self,
            expression: ExpressionHandle,
        ) -> &[ExecutableDomainTest] {
            self.executable_domain_test_rows
                .span_or_empty(self.executable_domain_tests[source_span_index(expression)])
        }

        pub fn retain_executable_domain_test(
            &mut self,
            expression: ExpressionHandle,
            test: ExecutableDomainTest,
        ) {
            if !self.executable_domain_tests(expression).contains(&test) {
                self.executable_domain_test_rows.append_to_span(
                    &mut self.executable_domain_tests[source_span_index(expression)],
                    test,
                );
            }
        }

        fn replace_executable_domain_tests(
            &mut self,
            expression: ExpressionHandle,
            tests: impl IntoIterator<Item = ExecutableDomainTest>,
        ) {
            self.executable_domain_tests[source_span_index(expression)] =
                self.executable_domain_test_rows.insert_many(tests);
        }

        pub fn insert_expression_handles(
            &mut self,
            expressions: impl IntoIterator<Item = ExpressionHandle>,
        ) -> HandleSpan<ExpressionHandle> {
            self.expression_handles.insert_many(expressions)
        }

        crate::span_columns! {
            expression_handles(ExpressionHandle)
                reserve reserve_expression_handles("expression handle span count overflow")
                set set_expression_handle_at_offset
                push push_expression_handle;
            struct_fields(TableStructLiteralField)
                reserve reserve_struct_fields("struct literal field span count overflow")
                set set_struct_field_at_offset
                push push_struct_field;
            name_path_members(Identifier)
                reserve reserve_name_path_members("name path member span count overflow")
                set set_name_path_member_at_offset
                push push_name_path_member;
            name_path_member_symbols(SymbolHandle)
                reserve reserve_name_path_member_symbols("name path member symbol span count overflow")
                set set_name_path_member_symbol_at_offset
                push push_name_path_member_symbol;
        }

        pub fn insert_struct_fields(
            &mut self,
            fields: impl IntoIterator<Item = TableStructLiteralField>,
        ) -> HandleSpan<TableStructLiteralField> {
            self.struct_fields.insert_many(fields)
        }

        pub fn expression_handle_at_offset(
            &self,
            expressions: HandleSpan<ExpressionHandle>,
            offset: u32,
        ) -> ExpressionHandle {
            *self.expression_handles.get(expressions.handle_at(offset))
        }

        pub fn expression_mut(&mut self, handle: ExpressionHandle) -> &mut ExpressionNode {
            self.expressions.get_mut(handle)
        }

        #[inline]
        pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
            self.expressions.get(handle)
        }

        pub fn peeled_expression_handle(&self, mut handle: ExpressionHandle) -> ExpressionHandle {
            while let ExpressionNode::Borrow(inner) = self.expression(handle) {
                handle = inner.target;
            }
            handle
        }

        pub fn peeled_expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
            self.expression(self.peeled_expression_handle(handle))
        }

        expression_node_accessors! {
            name_path_node(Name) -> TableNamePath;

            range_node(Range) -> TableRangeExpression;

            call_node(Call) -> TableCallExpression;

            member_node(Member) -> TableMemberExpression;

            indexed_node(Indexed) -> TableIndexedExpression;

            cast_node(Cast) -> TableCastExpression;

            binary_node(Binary) -> TableBinaryExpression;

            boolean_node(Boolean) -> bool;

            match_node(Match) -> TableMatchExpression;

            array_literal_node(ArrayLiteral) -> HandleSpan<ExpressionHandle>;

            struct_literal_node(StructLiteral) -> TableStructLiteral;

            string_node(String) -> source::SourceBytes;

            unary_node(Unary) -> TableUnaryExpression;

            integer_node(Integer) -> IntegerLiteral;

        }

        pub fn indexed_on_range(&self, indexed: &TableIndexedExpression) -> bool {
            matches!(self.expression(indexed.index), ExpressionNode::Range(_))
        }

        #[inline]
        pub fn expression_is_valid(&self, handle: ExpressionHandle) -> bool {
            self.expressions.is_valid(handle)
        }

        pub fn borrow_expression(
            &self,
            handle: ExpressionHandle,
        ) -> Option<&TableBorrowExpression> {
            let ExpressionNode::Borrow(borrow) = self.expression(handle) else {
                return None;
            };
            Some(borrow)
        }

        pub fn expression_at_depth(
            &self,
            handle: ExpressionHandle,
            depth: usize,
        ) -> Option<&ExpressionNode> {
            if depth >= 128 || !self.expression_is_valid(handle) {
                return None;
            }
            Some(self.expression(handle))
        }

        pub fn authored_length_receiver(
            &self,
            expression: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            use language_semantics::declaration_selection::CollectionMeasure;
            let member = self.member_node(expression)?;
            if CollectionMeasure::from_authored_spelling(member.member.as_str())
                != Some(CollectionMeasure::Length)
                || member.member_symbol.is_valid()
                || member.case_variant.is_some()
            {
                return None;
            }
            Some(member.receiver)
        }

        pub fn name_path_string(&self, span: HandleSpan<Identifier>) -> String {
            self.name_path_members(span).joined_path()
        }

        pub fn iter_expressions(
            &self,
        ) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
            self.expression_roots
                .iter()
                .map(|(_, handle)| (*handle, self.expressions.get(*handle)))
        }

        pub fn iter_backing_expressions(
            &self,
        ) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
            self.expressions.iter()
        }

        pub fn iter_expressions_from(
            &self,
            start_arena_index: usize,
        ) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> + '_ {
            let start_root = self
                .expression_roots
                .storage_slice()
                .partition_point(|handle| (handle.arena_index() as usize) < start_arena_index);
            self.expression_roots
                .iter_from(start_root)
                .filter(move |(_, handle)| handle.arena_index() as usize >= start_arena_index)
                .map(|(_, handle)| (*handle, self.expressions.get(*handle)))
        }

        pub fn retain_expression_roots(
            &mut self,
            mut retain: impl FnMut(ExpressionHandle) -> bool,
        ) {
            let retained = self
                .expression_roots
                .iter()
                .map(|(_, handle)| *handle)
                .filter(|handle| retain(*handle))
                .collect::<Vec<_>>();
            self.expression_roots.reset_retain_capacity();
            self.expression_roots.insert_many(retained);
        }

        #[inline]
        pub fn expression_handles(
            &self,
            span: HandleSpan<ExpressionHandle>,
        ) -> &[ExpressionHandle] {
            self.expression_handles.span_or_empty(span)
        }

        #[inline]
        pub fn struct_fields(
            &self,
            span: HandleSpan<TableStructLiteralField>,
        ) -> &[TableStructLiteralField] {
            self.struct_fields.span_or_empty(span)
        }

        #[inline]
        pub fn single_name_member(
            &self,
            expression: ExpressionHandle,
        ) -> Option<(&TableNamePath, &Identifier)> {
            let path = self.name_path_node(expression)?;
            let [name] = self.name_path_members(path.members) else {
                return None;
            };
            Some((path, name))
        }

        pub fn name_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
            self.name_path_members.span_or_empty(span)
        }

        pub fn respell_single_member_names(&mut self, symbol: SymbolHandle, member: &Identifier) {
            let spans: Vec<HandleSpan<Identifier>> = self
                .expressions
                .iter()
                .filter_map(|(_, node)| match node {
                    ExpressionNode::Name(path)
                        if path.symbol == symbol
                            && path.head_symbol == symbol
                            && self.name_path_members(path.members).len() == 1 =>
                    {
                        Some(path.members)
                    }
                    _ => None,
                })
                .collect();
            for span in spans {
                for existing in self.name_path_members.span_mut_or_empty(span) {
                    *existing = member.clone();
                }
            }
        }

        #[inline]
        pub fn name_path_member_symbols(&self, span: HandleSpan<SymbolHandle>) -> &[SymbolHandle] {
            self.name_path_member_symbols.span_or_empty(span)
        }

        pub fn name_path_is_single_member(&self, path: &TableNamePath) -> bool {
            path.symbol.is_valid()
                && path.head_symbol == path.symbol
                && self.name_path_members(path.members).len() == 1
        }

        pub fn names_reserved_result(&self, expression: ExpressionHandle) -> bool {
            let Some(path) = self.name_path_node(expression) else {
                return false;
            };
            !path.symbol.is_valid()
                && !path.head_symbol.is_valid()
                && matches!(self.name_path_members(path.members), [name] if name.is_reserved_result())
                && self
                    .name_path_member_symbols(path.member_symbols)
                    .iter()
                    .all(|symbol| !symbol.is_valid())
        }

        pub fn names_parameter(
            &self,
            expression: ExpressionHandle,
            parameter: &crate::signature::StateParameter,
        ) -> bool {
            let Some(path) = self.name_path_node(expression) else {
                return false;
            };
            let [name] = self.name_path_members(path.members) else {
                return false;
            };
            (parameter.symbol.is_valid()
                && (path.symbol == parameter.symbol || path.head_symbol == parameter.symbol))
                || name.as_str() == parameter.name.as_str()
        }

        pub fn expressions_structurally_equal(
            &self,
            a: ExpressionHandle,
            b: ExpressionHandle,
        ) -> bool {
            if a == b {
                return true;
            }
            if !a.is_valid()
                || !b.is_valid()
                || self.direct_pattern_component(a) != self.direct_pattern_component(b)
            {
                return false;
            }
            match (self.expression(a), self.expression(b)) {
                (ExpressionNode::Match(x), ExpressionNode::Match(y)) => {
                    let x_arms = self.match_arms(x.arms);
                    let y_arms = self.match_arms(y.arms);
                    x.capture == y.capture
                        && self.expressions_structurally_equal(x.subject, y.subject)
                        && x_arms.len() == y_arms.len()
                        && x_arms.iter().zip(y_arms).all(|(x, y)| {
                            let patterns_equal = match (x.pattern, y.pattern) {
                                (MatchPattern::Value(x), MatchPattern::Value(y)) => {
                                    self.expressions_structurally_equal(x, y)
                                }
                                (MatchPattern::Wildcard, MatchPattern::Wildcard) => true,
                                (
                                    MatchPattern::Destructure(left),
                                    MatchPattern::Destructure(right),
                                ) => left == right,
                                _ => false,
                            };
                            patterns_equal && self.expressions_structurally_equal(x.value, y.value)
                        })
                }
                (ExpressionNode::SavedSubject(left), ExpressionNode::SavedSubject(right)) => {
                    if left.component != right.component {
                        return false;
                    }
                    left.capture == right.capture
                        && self
                            .expressions_structurally_equal(left.pattern_head, right.pattern_head)
                }
                (ExpressionNode::Integer(x), ExpressionNode::Integer(y)) => x == y,
                (ExpressionNode::Boolean(x), ExpressionNode::Boolean(y)) => x == y,
                (ExpressionNode::String(x), ExpressionNode::String(y)) => x == y,
                (ExpressionNode::Float(x), ExpressionNode::Float(y)) => x == y,
                (ExpressionNode::UndeterminedNaN(left), ExpressionNode::UndeterminedNaN(right)) => {
                    left == right
                }
                (ExpressionNode::Name(x), ExpressionNode::Name(y)) => {
                    x.head_symbol == y.head_symbol
                        && x.symbol == y.symbol
                        && identifier_texts_equal(
                            self.name_path_members(x.members),
                            self.name_path_members(y.members),
                        )
                }
                (ExpressionNode::Member(x), ExpressionNode::Member(y)) => {
                    x.member_symbol == y.member_symbol
                        && x.member.as_str() == y.member.as_str()
                        && self.expressions_structurally_equal(x.receiver, y.receiver)
                }
                (ExpressionNode::Borrow(x), ExpressionNode::Borrow(y)) => {
                    x.access == y.access && self.expressions_structurally_equal(x.target, y.target)
                }
                (ExpressionNode::Unary(x), ExpressionNode::Unary(y)) => {
                    x.operator == y.operator
                        && self.expressions_structurally_equal(x.operand, y.operand)
                }
                (ExpressionNode::Binary(x), ExpressionNode::Binary(y)) => {
                    x.operator == y.operator
                        && self.expressions_structurally_equal(x.left, y.left)
                        && self.expressions_structurally_equal(x.right, y.right)
                }
                (ExpressionNode::Indexed(x), ExpressionNode::Indexed(y)) => {
                    self.expressions_structurally_equal(x.collection, y.collection)
                        && self.expressions_structurally_equal(x.index, y.index)
                }
                (ExpressionNode::Range(x), ExpressionNode::Range(y)) => {
                    x.end_inclusive == y.end_inclusive
                        && self.optional_expression_structurally_equal(x.start, y.start)
                        && self.optional_expression_structurally_equal(x.end, y.end)
                }
                (ExpressionNode::Call(x), ExpressionNode::Call(y)) => {
                    x.target_symbol == y.target_symbol
                        && x.exact_coercion_target == y.exact_coercion_target
                        && x.target.as_str() == y.target.as_str()
                        && self.expressions_structurally_equal(x.receiver, y.receiver)
                        && self.expression_spans_structurally_equal(x.arguments, y.arguments)
                }
                _ => false,
            }
        }

        fn optional_expression_structurally_equal(
            &self,
            a: ExpressionHandle,
            b: ExpressionHandle,
        ) -> bool {
            if a.is_valid() != b.is_valid() {
                return false;
            }
            !a.is_valid() || self.expressions_structurally_equal(a, b)
        }

        fn expression_spans_structurally_equal(
            &self,
            a: HandleSpan<ExpressionHandle>,
            b: HandleSpan<ExpressionHandle>,
        ) -> bool {
            let a = self.expression_handles(a);
            let b = self.expression_handles(b);
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(x, y)| self.expressions_structurally_equal(*x, *y))
        }

        pub fn expression_count(&self) -> usize {
            self.expressions.len()
        }

        pub fn expression_nodes(&self) -> impl Iterator<Item = &ExpressionNode> {
            self.iter_expressions().map(|(_, node)| node)
        }

        pub fn expression_entries(
            &self,
        ) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
            self.iter_expressions()
        }

        pub fn struct_field_count(&self) -> usize {
            self.struct_fields.len()
        }

        pub fn display_name(&self, handle: ExpressionHandle) -> String {
            let mut rendered = String::new();
            self.write_display_name(handle, &mut rendered);
            rendered
        }

        pub fn write_display_name(&self, handle: ExpressionHandle, out: &mut String) {
            self.expression(handle).write_display_name(self, out);
        }

        pub fn constant_integer_value(&self, handle: ExpressionHandle) -> Option<i64> {
            if !handle.is_valid() {
                return None;
            }
            match self.expression(handle) {
                ExpressionNode::Integer(value) => value.value_i64(),
                ExpressionNode::Borrow(inner) => self.constant_integer_value(inner.target),
                ExpressionNode::Binary(binary) => {
                    let left = self.constant_integer_value(binary.left)?;
                    let right = self.constant_integer_value(binary.right)?;
                    match binary.operator {
                        BinaryOperator::Add => left.checked_add(right),
                        BinaryOperator::Subtract => left.checked_sub(right),
                        BinaryOperator::Multiply => left.checked_mul(right),
                        BinaryOperator::Divide => left.checked_div(right),
                        BinaryOperator::Modulo => left.checked_rem(right),
                        _ => None,
                    }
                }
                _ => None,
            }
        }

        pub fn string_literal(&self, handle: ExpressionHandle) -> Option<&[u8]> {
            match self.expression(handle) {
                ExpressionNode::String(value) => Some(value.as_ref()),
                _ => None,
            }
        }
    }

    fn source_span_index(handle: ExpressionHandle) -> usize {
        usize::try_from(handle.arena_index())
            .expect("expression index overflow")
            .checked_sub(1)
            .expect("invalid expression handle has no source span")
    }

    impl Default for ExpressionTable {
        fn default() -> Self {
            Self::new()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum ExpressionNode {
        TypeExpression(crate::types::TypeReferenceHandle),
        Match(TableMatchExpression),
        SavedSubject(TableSavedSubject),
        ArrayLiteral(HandleSpan<ExpressionHandle>),
        Atomic(TableAtomicExpression),
        Binary(TableBinaryExpression),
        Boolean(bool),
        Cast(TableCastExpression),
        Call(TableCallExpression),
        Float(FloatLiteral),
        UndeterminedNaN(numerics::literals::FloatFormat),
        Indexed(TableIndexedExpression),
        Integer(IntegerLiteral),
        Member(TableMemberExpression),
        Borrow(TableBorrowExpression),
        Name(TableNamePath),
        Range(TableRangeExpression),
        StructLiteral(TableStructLiteral),
        String(source::SourceBytes),
        Unary(TableUnaryExpression),
        ZeroValue(crate::types::TypeReferenceHandle),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableBorrowExpression {
        pub target: ExpressionHandle,
        pub access: language_core::ReferenceAccess,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableAtomicExpression {
        pub value: ExpressionHandle,
        pub result: ExpressionHandle,
        pub ordering: language_core::atomic::AtomicOrderingPlan,
        pub result_custody: language_core::atomic::AtomicExpressionResultCustody,
    }

    impl Default for ExpressionNode {
        fn default() -> Self {
            Self::Integer(IntegerLiteral::zero())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableBinaryExpression {
        pub left: ExpressionHandle,
        pub operator: BinaryOperator,
        pub right: ExpressionHandle,
        /// The policy an operator suffix chose for this one operation
        /// (`+%`, `+|`, `+!`); `Exact` for the bare operator. Preparation
        /// rewrites every suffixed operation in executable code onto
        /// policy-qualified operands, so checking sees `Exact` here and
        /// rejects anything else.
        pub policy: numerics::arithmetic::ArithmeticDomain,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct UnaryExpression {
        pub operator: UnaryOperator,
        pub operand: Expression,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableUnaryExpression {
        pub operator: UnaryOperator,
        pub operand: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableCastExpression {
        pub value: ExpressionHandle,
        pub target_type: crate::types::TypeReferenceHandle,
        pub result_type: crate::types::TypeReferenceHandle,
        pub target_label: HandleSpan<Identifier>,
        pub domain: numerics::arithmetic::ArithmeticDomain,
        pub semantic_domain: HandleSpan<Identifier>,
        pub semantic_domain_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
        pub semantic_domain_symbol: SymbolHandle,
        pub semantic_domain_id: language_semantics::SemanticDomainId,
        pub form: language_core::cast_form::CastForm,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableIndexedExpression {
        pub collection: ExpressionHandle,
        pub index: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableRangeExpression {
        pub start: ExpressionHandle,
        pub end: ExpressionHandle,
        pub end_inclusive: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RangeExpression {
        pub start: Option<Box<Expression>>,
        pub end: Option<Box<Expression>>,
        pub end_inclusive: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MemberExpression {
        pub receiver: Expression,
        pub member_symbol: SymbolHandle,
        pub member: Identifier,
        pub case_variant: Option<Identifier>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableMemberExpression {
        pub receiver: ExpressionHandle,
        pub member_symbol: SymbolHandle,
        pub member: Identifier,
        pub case_variant: Option<Identifier>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallExpression {
        pub authored_call_selection: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
        pub authored_call_binding:
            Option<language_semantics::declaration_selection::ResolvedAuthoredCallBinding>,
        pub exact_coercion_target: crate::types::TypeReferenceHandle,
        pub requirement_application: crate::types::TypeReferenceHandle,
        pub receiver: Option<Box<Expression>>,
        pub target_symbol: SymbolHandle,
        pub target: Identifier,
        pub arguments: Arc<[Expression]>,
        pub evidence_arguments: Arc<[Identifier]>,
        pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableCallExpression {
        pub authored_call_selection: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
        pub authored_call_binding:
            Option<language_semantics::declaration_selection::ResolvedAuthoredCallBinding>,
        pub build_selection: Handle<BuildSelectionRequest>,
        pub authored_machine_application: StaticMachineApplicationHandle,
        pub exact_coercion_target: crate::types::TypeReferenceHandle,
        pub requirement_application: crate::types::TypeReferenceHandle,
        pub receiver: ExpressionHandle,
        pub target_symbol: SymbolHandle,
        pub static_machine_parameter: SymbolHandle,
        pub target: Identifier,
        pub static_requirement_dispatch: Option<crate::StaticRequirementDispatch>,
        pub machine_arguments: Box<[StaticMachineArgument]>,
        pub quotient_operation: Option<QuotientOperationRequest>,
        pub private_layout_operation: Option<PrivateLayoutOperationRequest>,
        pub installed_interrupt_operation: Option<InstalledInterruptOperationRequest>,
        pub layout_query: Option<DataLayoutQueryRequest>,
        pub native_code_layout: Option<NativeCodeLayoutRequest>,
        pub arguments: HandleSpan<ExpressionHandle>,
        pub evidence_arguments: Box<[Identifier]>,
        pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct InstalledInterruptOperationRequest {
        pub registrar_application: crate::static_applications::StaticMachineApplicationHandle,
        pub registrar_target: SymbolHandle,
        pub registrar_receiving_scope: crate::trait_definition::RetainedTraitScope,
        pub registrar_selection: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
        pub registrar_trait: SymbolHandle,
        pub registrar: StaticMachineArgument,
        pub timer_slot: StaticMachineArgument,
    }

    impl InstalledInterruptOperationRequest {
        pub fn invalidate_preparation(&mut self) {
            self.registrar_application =
                crate::static_applications::StaticMachineApplicationHandle::invalid();
            self.registrar_target = SymbolHandle::invalid();
            self.registrar_receiving_scope = Default::default();
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct NativeCodeLayoutRequest {
        pub operand: StaticMachineArgument,
        pub discards_result: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct AccessBorrowOperationRequest {
        pub factory: SymbolHandle,
        pub access: language_core::ReferenceAccess,
        pub backing: AccessBorrowBacking,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum AccessBorrowBacking {
        Ordinary,
        Resident {
            extent: SymbolHandle,
            granted: SymbolHandle,
            resident: SymbolHandle,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct PrivateLayoutOperationRequest {
        pub selected_slot: StaticMachineArgument,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct DataLayoutQueryRequest {
        pub kind: language_core::data_layout_queries::DataLayoutQueryKind,
        pub operand: StaticMachineArgument,
        pub nonempty_view: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum QuotientOperationKind {
        Lift,
        Define,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum QuotientTheoremRole {
        Congruence,
        ForwardPreconditionTransport,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct QuotientTheoremSelection {
        pub role: QuotientTheoremRole,
        pub application: StaticMachineArgument,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct QuotientOperationRequest {
        pub kind: QuotientOperationKind,
        pub representative_operation: StaticMachineArgument,
        pub theorem_evidence: Box<[QuotientTheoremSelection]>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct StaticMachineArgument {
        pub type_reference: crate::types::TypeReferenceHandle,
        pub path: Box<[Identifier]>,
        pub application: Option<Box<StaticSymbolApplication>>,
        pub const_literal: Option<numerics::literals::IntegerLiteral>,
        pub evidence_projection: Option<EvidenceProjection>,
        pub symbol: SymbolHandle,
    }

    impl StaticMachineArgument {
        pub fn names_implementation(&self, symbols: &symbols::SymbolTable) -> bool {
            self.symbol.is_valid()
                && matches!(
                    symbols.get(self.symbol).kind,
                    symbols::SymbolKind::Conformance | symbols::SymbolKind::ConformanceParameter
                )
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct StaticSymbolApplication {
        pub member: Handle<StaticMachineArgument>,
        pub lifetime_arguments: Box<[Identifier]>,
        pub arguments: Box<[StaticMachineArgument]>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct EvidenceProjection {
        pub term: Identifier,
        pub member: Identifier,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableNamePath {
        pub members: HandleSpan<Identifier>,
        pub member_symbols: HandleSpan<SymbolHandle>,
        pub head_symbol: SymbolHandle,
        pub symbol: SymbolHandle,
        /// The callable whose reserved `result` this name denotes, or invalid
        /// when it denotes no result. Stage 02 sets it once, where it decides a
        /// contract name is the reserved result occurrence
        /// (`wiki/spec/terminal-psi/calls_and_outcomes.md`: "The reserved
        /// result occurrence belongs to its contract owner, not any local or
        /// parameter spelled `result`"), so a later reader takes the result
        /// by this identity instead of comparing the spelling. `symbol` stays
        /// invalid, as before, so readers of the spelling route are unchanged.
        pub reserved_result_of: SymbolHandle,
    }

    impl TableNamePath {
        pub const fn new(
            members: HandleSpan<Identifier>,
            member_symbols: HandleSpan<SymbolHandle>,
            head_symbol: SymbolHandle,
            symbol: SymbolHandle,
        ) -> Self {
            Self {
                members,
                member_symbols,
                head_symbol,
                symbol,
                reserved_result_of: SymbolHandle::invalid(),
            }
        }

        pub fn resolved_symbol(&self) -> Option<SymbolHandle> {
            self.head_symbol
                .is_valid()
                .then_some(self.head_symbol)
                .or_else(|| self.symbol.is_valid().then_some(self.symbol))
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableStructLiteral {
        pub application_type: crate::types::TypeReferenceHandle,
        pub type_name: Identifier,
        pub type_symbol: SymbolHandle,
        pub case_name: Option<Identifier>,
        pub case_symbol: Option<SymbolHandle>,
        pub fields: HandleSpan<TableStructLiteralField>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableStructLiteralField {
        pub name: Identifier,
        pub field_symbol: SymbolHandle,
        pub value: ExpressionHandle,
    }

    impl Default for Expression {
        fn default() -> Self {
            Self::Integer(IntegerLiteral::zero())
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct NamePath {
        members: Arc<[Identifier]>,
        member_symbols: Arc<[SymbolHandle]>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
        /// The contract owner whose reserved `result` this name denotes, as
        /// `TableNamePath::reserved_result_of`; invalid for every other name.
        /// Held here so `ExpressionTable::insert_tree` stores the identity
        /// instead of leaving a name that only spells `result`.
        reserved_result_of: SymbolHandle,
    }

    impl NamePath {
        pub fn unresolved(members: Vec<Identifier>) -> Self {
            let member_symbols = vec![SymbolHandle::invalid(); members.len()];
            Self {
                members: Arc::from(members.into_boxed_slice()),
                member_symbols: Arc::from(member_symbols.into_boxed_slice()),
                head_symbol: SymbolHandle::invalid(),
                symbol: SymbolHandle::invalid(),
                reserved_result_of: SymbolHandle::invalid(),
            }
        }

        pub fn resolved(
            members: Vec<Identifier>,
            head_symbol: SymbolHandle,
            symbol: SymbolHandle,
        ) -> Self {
            let member_symbols = vec![SymbolHandle::invalid(); members.len()];
            Self::resolved_with_member_symbols(members, member_symbols, head_symbol, symbol)
        }

        pub fn resolved_with_member_symbols(
            members: Vec<Identifier>,
            mut member_symbols: Vec<SymbolHandle>,
            head_symbol: SymbolHandle,
            symbol: SymbolHandle,
        ) -> Self {
            if member_symbols.len() != members.len() {
                member_symbols.resize(members.len(), SymbolHandle::invalid());
            }
            if let Some(first_symbol) = member_symbols.first_mut() {
                *first_symbol = head_symbol;
            }
            if let Some(last_symbol) = member_symbols.last_mut() {
                *last_symbol = symbol;
            }
            Self {
                members: Arc::from(members.into_boxed_slice()),
                member_symbols: Arc::from(member_symbols.into_boxed_slice()),
                head_symbol,
                symbol,
                reserved_result_of: SymbolHandle::invalid(),
            }
        }

        /// This name as the reserved `result` of the contract owned by
        /// `owner`. Any later edit of the path's members clears it, because
        /// the edited path is a different name.
        pub fn denoting_reserved_result_of(mut self, owner: SymbolHandle) -> Self {
            self.reserved_result_of = owner;
            self
        }

        pub fn reserved_result_of(&self) -> SymbolHandle {
            self.reserved_result_of
        }

        pub fn resolved_from_iter(
            members: impl IntoIterator<Item = Identifier>,
            head_symbol: SymbolHandle,
            symbol: SymbolHandle,
        ) -> Self {
            Self::resolved(members.into_iter().collect(), head_symbol, symbol)
        }

        pub fn members(&self) -> &[Identifier] {
            &self.members
        }

        pub fn member_symbols(&self) -> &[SymbolHandle] {
            &self.member_symbols
        }

        pub fn member_symbol(&self, index: usize) -> SymbolHandle {
            self.member_symbols
                .get(index)
                .copied()
                .unwrap_or_else(SymbolHandle::invalid)
        }

        pub fn len(&self) -> usize {
            self.members.len()
        }

        pub fn is_empty(&self) -> bool {
            self.members.is_empty()
        }

        pub fn first(&self) -> Option<&Identifier> {
            self.members.first()
        }

        pub fn last(&self) -> Option<&Identifier> {
            self.members.last()
        }

        pub fn last_mut(&mut self) -> Option<&mut Identifier> {
            self.symbol = SymbolHandle::invalid();
            self.reserved_result_of = SymbolHandle::invalid();
            if let Some(last_symbol) = Arc::make_mut(&mut self.member_symbols).last_mut() {
                *last_symbol = SymbolHandle::invalid();
            }
            Arc::make_mut(&mut self.members).last_mut()
        }

        pub fn push(&mut self, member: Identifier) {
            let mut members = self.members.iter().cloned().collect::<Vec<_>>();
            members.push(member);
            self.members = Arc::from(members.into_boxed_slice());

            let mut member_symbols = self.member_symbols.iter().copied().collect::<Vec<_>>();
            member_symbols.push(SymbolHandle::invalid());
            self.member_symbols = Arc::from(member_symbols.into_boxed_slice());
            self.symbol = SymbolHandle::invalid();
            self.reserved_result_of = SymbolHandle::invalid();
        }

        pub fn push_resolved(&mut self, member: Identifier, symbol: SymbolHandle) {
            let mut members = self.members.iter().cloned().collect::<Vec<_>>();
            members.push(member);
            self.members = Arc::from(members.into_boxed_slice());

            let mut member_symbols = self.member_symbols.iter().copied().collect::<Vec<_>>();
            member_symbols.push(symbol);
            self.member_symbols = Arc::from(member_symbols.into_boxed_slice());
            self.symbol = symbol;
            self.reserved_result_of = SymbolHandle::invalid();
        }

        pub fn extend_from_slice(&mut self, members: &[Identifier]) {
            let mut path = self.members.iter().cloned().collect::<Vec<_>>();
            path.extend_from_slice(members);
            self.members = Arc::from(path.into_boxed_slice());

            let mut member_symbols = self.member_symbols.iter().copied().collect::<Vec<_>>();
            member_symbols
                .extend(std::iter::repeat_with(SymbolHandle::invalid).take(members.len()));
            self.member_symbols = Arc::from(member_symbols.into_boxed_slice());
            self.symbol = SymbolHandle::invalid();
            self.reserved_result_of = SymbolHandle::invalid();
        }

        pub fn head_symbol(&self) -> SymbolHandle {
            self.head_symbol
        }

        pub fn symbol(&self) -> SymbolHandle {
            self.symbol
        }
    }

    impl Deref for NamePath {
        type Target = [Identifier];

        fn deref(&self) -> &Self::Target {
            self.members()
        }
    }

    impl<'path> IntoIterator for &'path NamePath {
        type Item = &'path Identifier;
        type IntoIter = std::slice::Iter<'path, Identifier>;

        fn into_iter(self) -> Self::IntoIter {
            self.members.iter()
        }
    }

    pub use numerics::literals::FloatLiteral;

    fn identifier_texts_equal(a: &[Identifier], b: &[Identifier]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b.iter())
                .all(|(left, right)| left.as_str() == right.as_str())
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BinaryExpression {
        pub left: Expression,
        pub operator: BinaryOperator,
        pub right: Expression,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum BinaryOperator {
        Add,
        And,
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
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
        CaseMembership,
    }

    impl BinaryOperator {
        pub fn is_arithmetic(self) -> bool {
            matches!(
                self,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
            )
        }

        pub fn is_comparison(self) -> bool {
            matches!(
                self,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            )
        }

        pub fn evaluate_comparison<T: Ord>(self, left: &T, right: &T) -> Option<bool> {
            Some(match self {
                BinaryOperator::Equal => left == right,
                BinaryOperator::NotEqual => left != right,
                BinaryOperator::Less => left < right,
                BinaryOperator::LessOrEqual => left <= right,
                BinaryOperator::Greater => left > right,
                BinaryOperator::GreaterOrEqual => left >= right,
                _ => return None,
            })
        }

        pub fn mirrored_comparison(self) -> Option<BinaryOperator> {
            self.complement().map(|_| self.mirrored())
        }

        pub fn mirrored(self) -> BinaryOperator {
            match self {
                BinaryOperator::Less => BinaryOperator::Greater,
                BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
                BinaryOperator::Greater => BinaryOperator::Less,
                BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
                other => other,
            }
        }

        pub fn complement(self) -> Option<BinaryOperator> {
            match self {
                BinaryOperator::Equal => Some(BinaryOperator::NotEqual),
                BinaryOperator::NotEqual => Some(BinaryOperator::Equal),
                BinaryOperator::Less => Some(BinaryOperator::GreaterOrEqual),
                BinaryOperator::LessOrEqual => Some(BinaryOperator::Greater),
                BinaryOperator::Greater => Some(BinaryOperator::LessOrEqual),
                BinaryOperator::GreaterOrEqual => Some(BinaryOperator::Less),
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum UnaryOperator {
        BitwiseNot,
        LogicalNot,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CastExpression {
        pub value: Expression,
        pub target_type: crate::types::TypeReferenceHandle,
        pub target_label: NamePath,
        pub domain: numerics::arithmetic::ArithmeticDomain,
        pub form: language_core::cast_form::CastForm,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct IndexedExpression {
        pub collection: Expression,
        pub index: Expression,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StructLiteral {
        pub type_name: Identifier,
        pub case_name: Option<Identifier>,
        pub fields: Arc<[StructLiteralField]>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StructLiteralField {
        pub name: Identifier,
        pub value: Expression,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableMatchExpression {
        pub subject: ExpressionHandle,
        pub capture: Handle<CompletedValueCapture>,
        pub arms: HandleSpan<TableMatchArm>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CompletedValueCapture {
        pub subject: ExpressionHandle,
        pub owner: ValueCaptureOwner,
        pub definition: ExpressionHandle,
        pub value_anchor: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ValueCaptureOwner {
        #[default]
        Match,
        ExpressionArgument {
            call: ExpressionHandle,
            position: u32,
        },
        StatementArgument {
            call: crate::statement::StatementHandle,
            position: u32,
        },
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableSavedSubject {
        pub component: language_core::PatternComponent,
        pub capture: Handle<CompletedValueCapture>,
        pub pattern_head: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableMatchArm {
        pub pattern: MatchPattern,
        pub value: ExpressionHandle,
        pub source_span: SourceSpan,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum MatchPattern {
        Value(ExpressionHandle),
        Destructure(TableDestructurePattern),
        #[default]
        Wildcard,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableDestructurePattern {
        pub application_type: crate::types::TypeReferenceHandle,
        pub head: ExpressionHandle,
        pub fields: HandleSpan<TableMatchField>,
        pub has_rest: bool,
        pub guard: ExpressionHandle,
        pub condition: ExpressionHandle,
        pub proof_selectors: HandleSpan<crate::statement::OutcomeProofSelector>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableMatchField {
        pub component: language_core::PatternComponent,
        pub member: Identifier,
        pub field_symbol: SymbolHandle,
        pub action: MatchFieldAction,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum MatchFieldAction {
        Bind(Identifier),
        Equal(ExpressionHandle),
        #[default]
        Waive,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MatchExpression {
        pub subject: Expression,
        pub arms: Arc<[MatchArm]>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MatchArm {
        pub pattern: OwnedMatchPattern,
        pub value: Expression,
        pub source_span: SourceSpan,
    }

    #[macro_export]
    macro_rules! executable_leaf_expression_pattern {
        () => {
            $crate::expression::ExpressionNode::Boolean(_)
                | $crate::expression::ExpressionNode::Float(_)
                | $crate::expression::ExpressionNode::UndeterminedNaN(_)
                | $crate::expression::ExpressionNode::Integer(_)
                | $crate::expression::ExpressionNode::Name(_)
                | $crate::expression::ExpressionNode::String(_)
        };
    }

    #[macro_export]
    macro_rules! spelled_literal_expression_pattern {
        () => {
            $crate::expression::ExpressionNode::Boolean(_)
                | $crate::expression::ExpressionNode::Float(_)
                | $crate::expression::ExpressionNode::UndeterminedNaN(_)
                | $crate::expression::ExpressionNode::Integer(_)
                | $crate::expression::ExpressionNode::String(_)
                | $crate::expression::ExpressionNode::ZeroValue(_)
        };
    }

    #[macro_export]
    macro_rules! literal_expression_pattern {
        () => {
            $crate::expression::ExpressionNode::Boolean(_)
                | $crate::expression::ExpressionNode::Float(_)
                | $crate::expression::ExpressionNode::UndeterminedNaN(_)
                | $crate::expression::ExpressionNode::Integer(_)
                | $crate::expression::ExpressionNode::Name(_)
                | $crate::expression::ExpressionNode::String(_)
                | $crate::expression::ExpressionNode::ZeroValue(_)
        };
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OwnedMatchPattern {
        Value(Expression),
        Wildcard,
    }

    pub fn ieee_float_value(
        literal: &numerics::literals::FloatLiteral,
        format: numerics::literals::FloatFormat,
    ) -> semantic_vocabulary::IeeeFloatValue {
        match format {
            numerics::literals::FloatFormat::F32 => {
                semantic_vocabulary::IeeeFloatValue::Binary32(literal.f32_bits())
            }
            numerics::literals::FloatFormat::F64 => {
                semantic_vocabulary::IeeeFloatValue::Binary64(literal.value_f64().to_bits())
            }
        }
    }
}
mod pattern_components {
    //! A case view keeps its decomposition contract when a reference aliases it.
    //!
    //! Carrier geometry does not turn that view into an owned copy of the whole
    //! data. Locals retain the component established by their initializer, and
    //! receiving checks enforce it on later writes. This is a binding contract,
    //! not a claim that the initializer's contents or old referents remain live.
    //! Ordinary loan and storage-version checks still authenticate each use.

    use crate::expression::{ExpressionHandle, ExpressionNode};
    use language_core::PatternComponent;
    use std::collections::BTreeMap;

    impl crate::TypedTrees {
        pub fn pattern_component(&self, expression: ExpressionHandle) -> Option<PatternComponent> {
            Query {
                program: self,
                visited: BTreeMap::new(),
            }
            .read(expression)
        }
    }

    struct Query<'a> {
        program: &'a crate::TypedTrees,
        // Only actual transparent dependencies enter this request-local map.
        // Full coordinates retain generations without allocating global ID gaps.
        // The initial None also refuses cycles, and shared aliases are read once.
        visited: BTreeMap<(u32, u32), Option<PatternComponent>>,
    }

    impl Query<'_> {
        fn read(&mut self, expression: ExpressionHandle) -> Option<PatternComponent> {
            let program = self.program;
            let table = &program.expression_table;
            if !table.expression_is_valid(expression) {
                return None;
            }
            let node = table.expression(expression);
            let direct = table.direct_pattern_component(expression)?;
            if let ExpressionNode::SavedSubject(saved) = node {
                self.program.saved_value_source(saved)?;
                return Some(if direct == PatternComponent::Case {
                    direct
                } else {
                    saved.component
                });
            }
            if direct == PatternComponent::Case {
                return Some(direct);
            }
            let source = match node {
                ExpressionNode::Name(name) => {
                    if table.names_reserved_result(expression) {
                        return Some(PatternComponent::Fields);
                    }
                    let symbol = name.resolved_symbol()?;
                    if self.program.symbols.get(symbol).kind != symbols::SymbolKind::Local {
                        return Some(PatternComponent::Fields);
                    }
                    let local = self.program.unique_local_data_for_symbol(symbol)?;
                    if !local.role.is_runtime_binding() {
                        return None;
                    }
                    if !local.initial_value.is_valid() {
                        return Some(PatternComponent::Fields);
                    }
                    Some(local.initial_value)
                }
                ExpressionNode::Borrow(borrow) => Some(borrow.target),
                ExpressionNode::Cast(cast) => Some(cast.value),
                ExpressionNode::Match(_) => None,
                _ => return Some(PatternComponent::Fields),
            };
            if let Some(source) = source {
                if !table.expression_is_valid(source) {
                    return None;
                }
                let leaf = match table.expression(source) {
                    ExpressionNode::Borrow(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Match(_) => false,
                    ExpressionNode::Name(name) => {
                        name.resolved_symbol().is_none_or(|symbol| {
                            self.program.symbols.get(symbol).kind != symbols::SymbolKind::Local
                        }) || table.direct_pattern_component(source) == Some(PatternComponent::Case)
                    }
                    _ => true,
                };
                if leaf {
                    return self.read(source);
                }
            }
            let coordinate = (expression.arena_index(), expression.generation());
            if let Some(component) = self.visited.get(&coordinate) {
                return *component;
            }
            self.visited.insert(coordinate, None);
            let result = if let Some(source) = source {
                self.read(source)
            } else {
                let ExpressionNode::Match(dispatch) = node else {
                    return None;
                };
                let mut selected = None;
                for arm in table.match_arms(dispatch.arms) {
                    let component = self.read(arm.value)?;
                    if selected.is_some_and(|prior| prior != component) {
                        return None;
                    }
                    selected = Some(component);
                }
                selected
            };
            self.visited.insert(coordinate, result);
            result
        }
    }
}
pub mod subtrees {
    use crate::expression::{ExpressionHandle, ExpressionNode};

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct SubtreeEdges {
        pub saved_sources: bool,
        pub atomic_results: bool,
        pub pattern_dependencies: bool,
    }

    impl SubtreeEdges {
        pub const VALUES: Self = Self {
            saved_sources: false,
            atomic_results: false,
            pattern_dependencies: false,
        };

        pub const STRUCTURAL: Self = Self {
            saved_sources: false,
            atomic_results: true,
            pattern_dependencies: false,
        };

        pub const SAVED_SOURCES: Self = Self {
            saved_sources: true,
            atomic_results: false,
            pattern_dependencies: false,
        };
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum SubtreeVisit {
        Found,
        Descend,
        Prune,
        Instead(ExpressionHandle),
        InsteadEach(Vec<ExpressionHandle>),
    }

    impl crate::TypedTrees {
        /*
            Visits `root`'s subtree in pre-order, children in edge order, and
            stops at the first `Found`. The walk keeps its pending nodes in a
            stack it owns rather than on the thread's call stack, so a long
            operator chain or deep nesting costs memory, not stack depth.
        */
        pub fn subtree_any(
            &self,
            root: ExpressionHandle,
            edges: SubtreeEdges,
            mut visit: impl FnMut(ExpressionHandle, &ExpressionNode) -> SubtreeVisit,
        ) -> bool {
            let mut pending = vec![root];
            let mut children = Vec::new();
            while let Some(expression) = pending.pop() {
                if !expression.is_valid() {
                    continue;
                }
                let node = self.expression_table.expression(expression);
                match visit(expression, node) {
                    SubtreeVisit::Found => return true,
                    SubtreeVisit::Prune => continue,
                    SubtreeVisit::Instead(child) => pending.push(child),
                    SubtreeVisit::InsteadEach(replacements) => {
                        pending.extend(replacements.into_iter().rev());
                    }
                    SubtreeVisit::Descend => {
                        self.subtree_children(node, edges, |child| {
                            children.push(child);
                            false
                        });
                        pending.extend(children.drain(..).rev());
                    }
                }
            }
            false
        }

        pub fn subtree_for_each(
            &self,
            root: ExpressionHandle,
            edges: SubtreeEdges,
            mut visit: impl FnMut(ExpressionHandle, &ExpressionNode),
        ) {
            self.subtree_any(root, edges, |expression, node| {
                visit(expression, node);
                SubtreeVisit::Descend
            });
        }

        pub fn subtree_children(
            &self,
            node: &ExpressionNode,
            edges: SubtreeEdges,
            mut child: impl FnMut(ExpressionHandle) -> bool,
        ) {
            let table = &self.expression_table;
            match node {
                ExpressionNode::SavedSubject(saved) => {
                    if edges.saved_sources
                        && let Some(source) = self.saved_value_source(saved)
                    {
                        child(source);
                    }
                }
                ExpressionNode::Match(dispatch) => {
                    if child(dispatch.subject) {
                        return;
                    }
                    for arm in table.match_arms(dispatch.arms) {
                        let stop = if edges.pattern_dependencies {
                            table
                                .match_pattern_dependencies(arm.pattern)
                                .any(&mut child)
                        } else {
                            table.match_pattern_operands(arm.pattern).any(&mut child)
                        };
                        if stop || child(arm.value) {
                            return;
                        }
                    }
                }
                ExpressionNode::Atomic(atomic) => {
                    let _ = child(atomic.value) || (edges.atomic_results && child(atomic.result));
                }
                ExpressionNode::ArrayLiteral(values) => {
                    let _ = table
                        .expression_handles(*values)
                        .iter()
                        .any(|value| child(*value));
                }
                ExpressionNode::Binary(binary) => {
                    let _ = child(binary.left) || child(binary.right);
                }
                ExpressionNode::Call(call) => {
                    let _ = child(call.receiver)
                        || table
                            .expression_handles(call.arguments)
                            .iter()
                            .any(|argument| child(*argument));
                }
                ExpressionNode::Cast(cast) => {
                    child(cast.value);
                }
                ExpressionNode::Indexed(indexed) => {
                    let _ = child(indexed.collection) || child(indexed.index);
                }
                ExpressionNode::Range(range) => {
                    let _ = child(range.start) || child(range.end);
                }
                ExpressionNode::Member(member) => {
                    child(member.receiver);
                }
                ExpressionNode::Borrow(borrow) => {
                    child(borrow.target);
                }
                ExpressionNode::Unary(unary) => {
                    child(unary.operand);
                }
                ExpressionNode::StructLiteral(literal) => {
                    let _ = table
                        .struct_fields(literal.fields)
                        .iter()
                        .any(|field| child(field.value));
                }
                ExpressionNode::TypeExpression(_) | crate::literal_expression_pattern!() => {}
            }
        }
    }
}
