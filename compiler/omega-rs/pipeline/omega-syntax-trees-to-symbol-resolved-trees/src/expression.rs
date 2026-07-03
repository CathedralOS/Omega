use crate::name::lower_name;
use omega_core::arena::HandleSpan;
use omega_symbol_resolved_trees::name::DiagnosticName;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable, FloatLiteral,
    TableBinaryExpression, TableCallExpression, TableCastExpression, TableIndexedExpression,
    TableMemberExpression, TableMembershipExpression, TableNamePath, TableRangeExpression,
    TableStructLiteral, TableStructLiteralField, TableUnaryExpression, UnaryOperator,
};
use omega_syntax_trees as syntax;
use omega_syntax_trees::SyntaxTrees;

pub(crate) fn lower_expression_into_table(
    syntax_trees: &SyntaxTrees,
    expressions: &mut ExpressionTable,
    expression: syntax::expression::ExpressionHandle,
) -> Result<ExpressionHandle, Diagnostic> {
    lower_expression_node_into_table(
        syntax_trees,
        expressions,
        syntax_trees.expressions.expression(expression),
    )
}

fn lower_expression_node_into_table(
    syntax_trees: &SyntaxTrees,
    expressions: &mut ExpressionTable,
    expression: &syntax::expression::ExpressionNode,
) -> Result<ExpressionHandle, Diagnostic> {
    match expression {
        syntax::expression::ExpressionNode::ArrayLiteral(values) => {
            let span = expressions.reserve_expression_handles(values.count());
            for (offset, value) in syntax_trees
                .expressions
                .expression_handles(*values)
                .iter()
                .enumerate()
            {
                let value = lower_expression_into_table(syntax_trees, expressions, *value)?;
                expressions.set_expression_handle_at_offset(
                    span,
                    offset
                        .try_into()
                        .expect("expression handle span count overflow"),
                    value,
                );
            }
            Ok(expressions.insert(ExpressionNode::ArrayLiteral(span)))
        }
        syntax::expression::ExpressionNode::Binary(binary) => {
            let left = lower_expression_into_table(syntax_trees, expressions, binary.left)?;
            let right = lower_expression_into_table(syntax_trees, expressions, binary.right)?;
            Ok(
                expressions.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: lower_binary_operator(binary.operator),
                    right,
                })),
            )
        }
        syntax::expression::ExpressionNode::Boolean(value) => {
            Ok(expressions.insert(ExpressionNode::Boolean(*value)))
        }
        syntax::expression::ExpressionNode::Cast(cast) => {
            let value = lower_expression_into_table(syntax_trees, expressions, cast.value)?;
            let mut target_type = HandleSpan::empty();
            for member in syntax_trees
                .expressions
                .identifier_path_members(cast.target_type)
            {
                expressions.push_name_path_member(&mut target_type, lower_name(member));
            }
            Ok(
                expressions.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
                    domain: cast.domain,
                })),
            )
        }
        syntax::expression::ExpressionNode::Call(call) => {
            // `abs(x)` desugars to `max(x, 0 - x)` -- absolute value with ZERO
            // new backend/interp machinery (min/max are binary builtins). The
            // reservation is narrow: only a receiver-less single-argument
            // `abs` is intercepted (a method call `foo.abs()` keeps its
            // receiver and is untouched). Sound across domains: Wrapping
            // abs(MIN)=MIN; Exact abs(MIN) fires the `0 - x` overflow proof
            // (|MIN| is unrepresentable). `x` becomes a SHARED subtree, so a
            // call argument (side effects / a single result slot) would run
            // twice -- rejected with a bind-to-a-local diagnostic.
            if !call.receiver.is_valid()
                && call.target.as_str() == "abs"
                && call.arguments.count() == 1
            {
                let argument_handle =
                    syntax_trees.expressions.expression_handles(call.arguments)[0];
                if matches!(
                    syntax_trees.expressions.expression(argument_handle),
                    syntax::expression::ExpressionNode::Call(_)
                ) {
                    return Err(Diagnostic::error(
                        "abs(...) duplicates its argument, so a call argument would run twice; \
                         bind it to a local first (`let v = f(); abs(v)`)",
                    ));
                }
                let x = lower_expression_into_table(syntax_trees, expressions, argument_handle)?;
                let zero = expressions.insert(ExpressionNode::Integer(0));
                let negated = expressions.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: zero,
                    operator: BinaryOperator::Subtract,
                    right: x,
                }));
                let arguments = expressions.reserve_expression_handles(2);
                expressions.set_expression_handle_at_offset(arguments, 0, x);
                expressions.set_expression_handle_at_offset(arguments, 1, negated);
                return Ok(expressions.insert(ExpressionNode::Call(TableCallExpression {
                    receiver: ExpressionHandle::invalid(),
                    target_symbol: SymbolHandle::invalid(),
                    target: DiagnosticName::new("max", call.target.source_span()),
                    arguments,
                })));
            }
            // `clamp(x, lo, hi)` desugars to `min(max(x, lo), hi)` -- also
            // pure min/max reuse. Each argument appears EXACTLY ONCE (no
            // shared subtree), so a call argument is safe here (no double-eval
            // reject needed, unlike abs). Same narrow reservation: receiver-
            // less, exactly three arguments.
            if !call.receiver.is_valid()
                && call.target.as_str() == "clamp"
                && call.arguments.count() == 3
            {
                let argument_handles = syntax_trees
                    .expressions
                    .expression_handles(call.arguments)
                    .to_vec();
                let x = lower_expression_into_table(syntax_trees, expressions, argument_handles[0])?;
                let lo =
                    lower_expression_into_table(syntax_trees, expressions, argument_handles[1])?;
                let hi =
                    lower_expression_into_table(syntax_trees, expressions, argument_handles[2])?;
                let max_arguments = expressions.reserve_expression_handles(2);
                expressions.set_expression_handle_at_offset(max_arguments, 0, x);
                expressions.set_expression_handle_at_offset(max_arguments, 1, lo);
                let max_call = expressions.insert(ExpressionNode::Call(TableCallExpression {
                    receiver: ExpressionHandle::invalid(),
                    target_symbol: SymbolHandle::invalid(),
                    target: DiagnosticName::new("max", call.target.source_span()),
                    arguments: max_arguments,
                }));
                let min_arguments = expressions.reserve_expression_handles(2);
                expressions.set_expression_handle_at_offset(min_arguments, 0, max_call);
                expressions.set_expression_handle_at_offset(min_arguments, 1, hi);
                return Ok(expressions.insert(ExpressionNode::Call(TableCallExpression {
                    receiver: ExpressionHandle::invalid(),
                    target_symbol: SymbolHandle::invalid(),
                    target: DiagnosticName::new("min", call.target.source_span()),
                    arguments: min_arguments,
                })));
            }

            let receiver = if call.receiver.is_valid() {
                lower_expression_into_table(syntax_trees, expressions, call.receiver)?
            } else {
                ExpressionHandle::invalid()
            };
            let arguments = expressions.reserve_expression_handles(call.arguments.count());
            for (offset, argument) in syntax_trees
                .expressions
                .expression_handles(call.arguments)
                .iter()
                .enumerate()
            {
                let argument = lower_expression_into_table(syntax_trees, expressions, *argument)?;
                expressions.set_expression_handle_at_offset(
                    arguments,
                    offset
                        .try_into()
                        .expect("expression handle span count overflow"),
                    argument,
                );
            }
            Ok(
                expressions.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: SymbolHandle::invalid(),
                    target: lower_name(&call.target),
                    arguments,
                })),
            )
        }
        syntax::expression::ExpressionNode::Float(value) => {
            let Some(value) = FloatLiteral::parse(value.as_str()) else {
                return Err(Diagnostic::error(format!(
                    "invalid float literal `{}`",
                    value.as_str()
                )));
            };
            Ok(expressions.insert(ExpressionNode::Float(value)))
        }
        syntax::expression::ExpressionNode::Indexed(indexed) => {
            let collection =
                lower_expression_into_table(syntax_trees, expressions, indexed.collection)?;
            let index = lower_expression_into_table(syntax_trees, expressions, indexed.index)?;
            Ok(
                expressions.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                })),
            )
        }
        syntax::expression::ExpressionNode::Integer(value) => {
            Ok(expressions.insert(ExpressionNode::Integer(*value)))
        }
        syntax::expression::ExpressionNode::Membership(membership) => {
            let value = lower_expression_into_table(syntax_trees, expressions, membership.value)?;
            let mut domain = HandleSpan::empty();
            for member in syntax_trees
                .expressions
                .identifier_path_members(membership.domain)
            {
                expressions.push_name_path_member(&mut domain, lower_name(member));
            }
            Ok(
                expressions.insert(ExpressionNode::Membership(TableMembershipExpression {
                    value,
                    domain,
                    domain_symbol: SymbolHandle::invalid(),
                })),
            )
        }
        syntax::expression::ExpressionNode::Member(member) => {
            let receiver = lower_expression_into_table(syntax_trees, expressions, member.receiver)?;
            Ok(
                expressions.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: SymbolHandle::invalid(),
                    member: lower_name(&member.member),
                    case_variant: member.case_variant.as_ref().map(lower_name),
                })),
            )
        }
        syntax::expression::ExpressionNode::Mutable(expression) => {
            let expression = lower_expression_into_table(syntax_trees, expressions, *expression)?;
            Ok(expressions.insert(ExpressionNode::Mutable(expression)))
        }
        syntax::expression::ExpressionNode::Name(path) => {
            let mut members = HandleSpan::empty();
            for member in syntax_trees.expressions.identifier_path_members(*path) {
                expressions.push_name_path_member(&mut members, lower_name(member));
            }
            Ok(expressions.insert(ExpressionNode::Name(TableNamePath {
                members,
                is_self_value: false,
                head_symbol: SymbolHandle::invalid(),
                symbol: SymbolHandle::invalid(),
            })))
        }
        syntax::expression::ExpressionNode::Range(range) => {
            let start = range
                .start
                .is_valid()
                .then(|| lower_expression_into_table(syntax_trees, expressions, range.start))
                .transpose()?
                .unwrap_or_else(ExpressionHandle::invalid);
            let end = range
                .end
                .is_valid()
                .then(|| lower_expression_into_table(syntax_trees, expressions, range.end))
                .transpose()?
                .unwrap_or_else(ExpressionHandle::invalid);
            Ok(
                expressions.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                })),
            )
        }
        syntax::expression::ExpressionNode::SelfValue => {
            let mut members = HandleSpan::empty();
            expressions.push_name_path_member(
                &mut members,
                omega_symbol_resolved_trees::name::DiagnosticName::generated_static("self"),
            );
            Ok(expressions.insert(ExpressionNode::Name(TableNamePath {
                members,
                is_self_value: true,
                head_symbol: SymbolHandle::invalid(),
                symbol: SymbolHandle::invalid(),
            })))
        }
        syntax::expression::ExpressionNode::String(value) => {
            Ok(expressions.insert(ExpressionNode::String(value.clone())))
        }
        syntax::expression::ExpressionNode::Unary(unary) => {
            let operand = lower_expression_into_table(syntax_trees, expressions, unary.operand)?;
            Ok(
                expressions.insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: lower_unary_operator(unary.operator),
                    operand,
                })),
            )
        }
        syntax::expression::ExpressionNode::StructLiteral(struct_literal) => {
            let fields = expressions.reserve_struct_fields(struct_literal.fields.count());
            for (offset, field) in syntax_trees
                .expressions
                .struct_fields(struct_literal.fields)
                .iter()
                .enumerate()
            {
                let value = lower_expression_into_table(syntax_trees, expressions, field.value)?;
                expressions.set_struct_field_at_offset(
                    fields,
                    offset
                        .try_into()
                        .expect("struct literal field span count overflow"),
                    TableStructLiteralField {
                        name: lower_name(&field.name),
                        value,
                    },
                );
            }
            let (type_name, case_name) =
                lower_struct_literal_shape_names(syntax_trees, struct_literal)?;
            Ok(
                expressions.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name,
                    case_name,
                    fields,
                })),
            )
        }
    }
}

/// The lowered `(type_name, case_name)` pair of a brace literal. A two-member
/// literal `Type::M { ... }` constructs a CASE of `Type` unless `M` is a
/// version selector (`v1`, `v2`, ...): then it constructs the HISTORICAL
/// SHAPE the data's `version vN { ... }` block declares, and lowers as a
/// plain record literal of the root-level shape definition `Type::vN` (no
/// case tag is involved). Constructing an undeclared version is a compile
/// error -- there is no historical shape to build.
fn lower_struct_literal_shape_names(
    syntax_trees: &SyntaxTrees,
    struct_literal: &syntax::expression::TableStructLiteral,
) -> Result<
    (
        omega_symbol_resolved_trees::name::DiagnosticName,
        Option<omega_symbol_resolved_trees::name::DiagnosticName>,
    ),
    Diagnostic,
> {
    let Some(member) = &struct_literal.case_name else {
        return Ok((lower_name(&struct_literal.type_name), None));
    };
    if !syntax::item::is_version_selector(member.as_str()) {
        return Ok((
            lower_name(&struct_literal.type_name),
            Some(lower_name(member)),
        ));
    }

    let data_name = struct_literal.type_name.as_str();
    let Some(data_definition) = syntax_trees.root_items().find_map(|item| match item {
        syntax::item::Item::Data(data_definition) if data_definition.name.as_str() == data_name => {
            Some(data_definition)
        }
        _ => None,
    }) else {
        // The head type is not a data declaration in this compilation unit;
        // leave the literal as written and let downstream resolution judge it.
        return Ok((
            lower_name(&struct_literal.type_name),
            Some(lower_name(member)),
        ));
    };

    let version = syntax_trees
        .items
        .data_members(data_definition.members)
        .iter()
        .find_map(|data_member| match data_member {
            syntax::item::DataMember::Version(version)
                if version.name.as_str() == member.as_str() =>
            {
                Some(version)
            }
            _ => None,
        });
    if let Some(version) = version {
        return Ok((
            omega_symbol_resolved_trees::name::DiagnosticName::from(
                version.shape_name(data_name).as_str(),
            ),
            None,
        ));
    }

    Err(Diagnostic::error(format!(
        "cannot construct `{data_name}::{}`: data `{data_name}` does not declare a `version {}` block",
        member.as_str(),
        member.as_str()
    )))
}

fn lower_unary_operator(operator: syntax::expression::UnaryOperator) -> UnaryOperator {
    match operator {
        syntax::expression::UnaryOperator::LogicalNot => UnaryOperator::LogicalNot,
    }
}

fn lower_binary_operator(operator: syntax::expression::BinaryOperator) -> BinaryOperator {
    match operator {
        syntax::expression::BinaryOperator::Add => BinaryOperator::Add,
        syntax::expression::BinaryOperator::And => BinaryOperator::And,
        syntax::expression::BinaryOperator::BitwiseAnd => BinaryOperator::BitwiseAnd,
        syntax::expression::BinaryOperator::BitwiseOr => BinaryOperator::BitwiseOr,
        syntax::expression::BinaryOperator::BitwiseXor => BinaryOperator::BitwiseXor,
        syntax::expression::BinaryOperator::Divide => BinaryOperator::Divide,
        syntax::expression::BinaryOperator::Equal => BinaryOperator::Equal,
        syntax::expression::BinaryOperator::Greater => BinaryOperator::Greater,
        syntax::expression::BinaryOperator::GreaterOrEqual => BinaryOperator::GreaterOrEqual,
        syntax::expression::BinaryOperator::Less => BinaryOperator::Less,
        syntax::expression::BinaryOperator::LessOrEqual => BinaryOperator::LessOrEqual,
        syntax::expression::BinaryOperator::Modulo => BinaryOperator::Modulo,
        syntax::expression::BinaryOperator::Multiply => BinaryOperator::Multiply,
        syntax::expression::BinaryOperator::NotEqual => BinaryOperator::NotEqual,
        syntax::expression::BinaryOperator::Or => BinaryOperator::Or,
        syntax::expression::BinaryOperator::ShiftLeft => BinaryOperator::ShiftLeft,
        syntax::expression::BinaryOperator::ShiftRight => BinaryOperator::ShiftRight,
        syntax::expression::BinaryOperator::Subtract => BinaryOperator::Subtract,
    }
}
