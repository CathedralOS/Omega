use crate::lowerer::Lowerer;
use crate::name::lower_name;
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable, FloatLiteral,
    TableAtomicExpression, TableBinaryExpression, TableCallExpression, TableCastExpression,
    TableIndexedExpression, TableMemberExpression, TableMembershipExpression, TableNamePath,
    TableRangeExpression, TableStructLiteral, TableStructLiteralField, TableUnaryExpression,
    UnaryOperator,
};
use psi_symbol_resolved_trees::name::DiagnosticName;
use psi_symbols::SymbolHandle;
use psi_syntax_trees as syntax;
use psi_syntax_trees::SyntaxTrees;

pub(crate) fn lower_expression_into_table(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
) -> Result<ExpressionHandle, Diagnostic> {
    let source_span = syntax_trees.expressions.source_span(expression);
    let lowered = lower_expression_node_into_table(
        lowerer,
        syntax_trees,
        syntax_trees.expressions.expression(expression),
    )?;
    expression_table(lowerer).set_source_span(lowered, source_span);
    if let Some(exposure) = lowerer.current_authored_expression_exposure {
        expression_table(lowerer).set_authored_expression_exposure(lowered, exposure);
        lowerer
            .pending_authored_expressions
            .push(crate::lowerer::PendingAuthoredExpression {
                expression: lowered,
                exposure,
            });
    }
    Ok(lowered)
}

/// Lower an authored state-body expression and retain it for package
/// declaration-selection admission. The temporary context propagates through
/// every recursively lowered syntax child; direct compiler-generated inserts
/// remain outside the authored set.
pub(crate) fn lower_private_expression_into_table(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
) -> Result<ExpressionHandle, Diagnostic> {
    let previous = lowerer.current_authored_expression_exposure.replace(
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
    );
    let result = lower_expression_into_table(lowerer, syntax_trees, expression);
    lowerer.current_authored_expression_exposure = previous;
    result
}

fn lower_expression_node_into_table(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expression: &syntax::expression::ExpressionNode,
) -> Result<ExpressionHandle, Diagnostic> {
    let syntax::expression::ExpressionNode::Binary(binary) = expression else {
        return lower_nonbinary_expression_node_into_table(lowerer, syntax_trees, expression);
    };
    lower_binary_expression_into_table(lowerer, syntax_trees, binary)
}

fn lower_binary_expression_into_table(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    binary: &syntax::expression::TableBinaryExpression,
) -> Result<ExpressionHandle, Diagnostic> {
    let left = lower_expression_into_table(lowerer, syntax_trees, binary.left)?;
    let right = lower_expression_into_table(lowerer, syntax_trees, binary.right)?;
    Ok(
        expression_table(lowerer).insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: lower_binary_operator(binary.operator),
            right,
        })),
    )
}

fn lower_nonbinary_expression_node_into_table(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expression: &syntax::expression::ExpressionNode,
) -> Result<ExpressionHandle, Diagnostic> {
    match expression {
        syntax::expression::ExpressionNode::ArrayLiteral(values) => {
            let span = expression_table(lowerer).reserve_expression_handles(values.count());
            for (offset, value) in syntax_trees
                .expressions
                .expression_handles(*values)
                .iter()
                .enumerate()
            {
                let value = lower_expression_into_table(lowerer, syntax_trees, *value)?;
                expression_table(lowerer).set_expression_handle_at_offset(
                    span,
                    offset
                        .try_into()
                        .expect("expression handle span count overflow"),
                    value,
                );
            }
            Ok(expression_table(lowerer).insert(ExpressionNode::ArrayLiteral(span)))
        }
        syntax::expression::ExpressionNode::Atomic(atomic) => {
            let value = lower_expression_into_table(lowerer, syntax_trees, atomic.value)?;
            let result = if atomic.result.is_valid() {
                lower_expression_into_table(lowerer, syntax_trees, atomic.result)?
            } else {
                ExpressionHandle::invalid()
            };
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                })),
            )
        }
        syntax::expression::ExpressionNode::Binary(_) => {
            unreachable!("binary expressions use the stack-bounded lowering path")
        }
        syntax::expression::ExpressionNode::Boolean(value) => {
            Ok(expression_table(lowerer).insert(ExpressionNode::Boolean(*value)))
        }
        syntax::expression::ExpressionNode::Cast(cast) => {
            let value = lower_expression_into_table(lowerer, syntax_trees, cast.value)?;
            let target_type = crate::type_reference::lower_type_reference_handle(
                lowerer,
                syntax_trees,
                cast.target_type,
            )?;
            let target_type = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .child_type_references
                .append(target_type);
            let mut target_label = HandleSpan::empty();
            for member in syntax_trees
                .expressions
                .identifier_path_members(cast.target_label)
            {
                expression_table(lowerer)
                    .push_name_path_member(&mut target_label, lower_name(member));
            }
            let mut semantic_domain = HandleSpan::empty();
            for member in syntax_trees
                .expressions
                .identifier_path_members(cast.semantic_domain)
            {
                expression_table(lowerer)
                    .push_name_path_member(&mut semantic_domain, lower_name(member));
            }
            let semantic_domain_arguments = crate::type_reference::lower_child_type_references(
                lowerer,
                syntax_trees,
                cast.semantic_domain_arguments,
            )?;
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
                    target_label,
                    domain: cast.domain,
                    semantic_domain,
                    semantic_domain_arguments,
                    semantic_domain_symbol: SymbolHandle::invalid(),
                    form: cast.form,
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
                let x = lower_expression_into_table(lowerer, syntax_trees, argument_handle)?;
                let zero = expression_table(lowerer).insert(ExpressionNode::Integer(
                    psi_numerics::literals::IntegerLiteral::zero(),
                ));
                let negated = expression_table(lowerer).insert(ExpressionNode::Binary(
                    TableBinaryExpression {
                        left: zero,
                        operator: BinaryOperator::Subtract,
                        right: x,
                    },
                ));
                // The subtraction is compiler-generated, but it realizes this
                // exact authored `abs` occurrence. Retain the token span so
                // checked operator evidence can be matched after later tables
                // copy the normalized tree; a zero-span synthetic node would
                // otherwise force backend shape guessing.
                expression_table(lowerer).set_source_span(negated, call.target.source_span());
                let arguments = expression_table(lowerer).reserve_expression_handles(2);
                expression_table(lowerer).set_expression_handle_at_offset(arguments, 0, x);
                expression_table(lowerer).set_expression_handle_at_offset(arguments, 1, negated);
                return Ok(expression_table(lowerer).insert(ExpressionNode::Call(
                    TableCallExpression {
                        receiver: ExpressionHandle::invalid(),
                        target_symbol: SymbolHandle::invalid(),
                        target: DiagnosticName::new("max", call.target.source_span()),
                        machine_arguments: Box::default(),
                        arguments,
                        evidence_arguments: Box::default(),
                        operational_acknowledgement:
                            psi_language_semantics::CallOperationalAcknowledgement {
                                origin: psi_language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                                ..Default::default()
                            },
                    },
                )));
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
                let x = lower_expression_into_table(lowerer, syntax_trees, argument_handles[0])?;
                let lo = lower_expression_into_table(lowerer, syntax_trees, argument_handles[1])?;
                let hi = lower_expression_into_table(lowerer, syntax_trees, argument_handles[2])?;
                let max_arguments = expression_table(lowerer).reserve_expression_handles(2);
                expression_table(lowerer).set_expression_handle_at_offset(max_arguments, 0, x);
                expression_table(lowerer).set_expression_handle_at_offset(max_arguments, 1, lo);
                let max_call =
                    expression_table(lowerer).insert(ExpressionNode::Call(TableCallExpression {
                        receiver: ExpressionHandle::invalid(),
                        target_symbol: SymbolHandle::invalid(),
                        target: DiagnosticName::new("max", call.target.source_span()),
                        machine_arguments: Box::default(),
                        arguments: max_arguments,
                        evidence_arguments: Box::default(),
                        operational_acknowledgement:
                            psi_language_semantics::CallOperationalAcknowledgement {
                                origin: psi_language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                                ..Default::default()
                            },
                    }));
                let min_arguments = expression_table(lowerer).reserve_expression_handles(2);
                expression_table(lowerer).set_expression_handle_at_offset(
                    min_arguments,
                    0,
                    max_call,
                );
                expression_table(lowerer).set_expression_handle_at_offset(min_arguments, 1, hi);
                return Ok(expression_table(lowerer).insert(ExpressionNode::Call(
                    TableCallExpression {
                        receiver: ExpressionHandle::invalid(),
                        target_symbol: SymbolHandle::invalid(),
                        target: DiagnosticName::new("min", call.target.source_span()),
                        machine_arguments: Box::default(),
                        arguments: min_arguments,
                        evidence_arguments: Box::default(),
                        operational_acknowledgement:
                            psi_language_semantics::CallOperationalAcknowledgement {
                                origin: psi_language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                                ..Default::default()
                            },
                    },
                )));
            }

            let receiver = if call.receiver.is_valid() {
                lower_expression_into_table(lowerer, syntax_trees, call.receiver)?
            } else {
                ExpressionHandle::invalid()
            };
            let arguments =
                expression_table(lowerer).reserve_expression_handles(call.arguments.count());
            for (offset, argument) in syntax_trees
                .expressions
                .expression_handles(call.arguments)
                .iter()
                .enumerate()
            {
                let argument = lower_expression_into_table(lowerer, syntax_trees, *argument)?;
                expression_table(lowerer).set_expression_handle_at_offset(
                    arguments,
                    offset
                        .try_into()
                        .expect("expression handle span count overflow"),
                    argument,
                );
            }
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: SymbolHandle::invalid(),
                    target: lower_name(&call.target),
                    machine_arguments: call
                        .machine_arguments
                        .iter()
                        .map(lower_static_machine_argument)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    arguments,
                    evidence_arguments: call
                        .evidence_arguments
                        .iter()
                        .map(lower_name)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    operational_acknowledgement: call.operational_acknowledgement,
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
            Ok(expression_table(lowerer).insert(ExpressionNode::Float(value)))
        }
        syntax::expression::ExpressionNode::Indexed(indexed) => {
            let collection =
                lower_expression_into_table(lowerer, syntax_trees, indexed.collection)?;
            let index = lower_expression_into_table(lowerer, syntax_trees, indexed.index)?;
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                })),
            )
        }
        syntax::expression::ExpressionNode::Integer(value) => {
            Ok(expression_table(lowerer).insert(ExpressionNode::Integer(value.clone())))
        }
        syntax::expression::ExpressionNode::Membership(membership) => {
            let value = lower_expression_into_table(lowerer, syntax_trees, membership.value)?;
            let mut domain = HandleSpan::empty();
            for member in syntax_trees
                .expressions
                .identifier_path_members(membership.domain)
            {
                expression_table(lowerer).push_name_path_member(&mut domain, lower_name(member));
            }
            Ok(expression_table(lowerer).insert(ExpressionNode::Membership(
                TableMembershipExpression {
                    value,
                    domain,
                    domain_symbol: SymbolHandle::invalid(),
                    case_type_symbol: SymbolHandle::invalid(),
                    case_symbol: SymbolHandle::invalid(),
                },
            )))
        }
        syntax::expression::ExpressionNode::Member(member) => {
            let receiver = lower_expression_into_table(lowerer, syntax_trees, member.receiver)?;
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: SymbolHandle::invalid(),
                    member: lower_name(&member.member),
                    case_variant: member.case_variant.as_ref().map(lower_name),
                })),
            )
        }
        syntax::expression::ExpressionNode::Borrow(expression) => {
            let target = lower_expression_into_table(lowerer, syntax_trees, expression.target)?;
            Ok(expression_table(lowerer).insert(ExpressionNode::Borrow(
                psi_symbol_resolved_trees::expression::TableBorrowExpression {
                    target,
                    access: expression.access,
                },
            )))
        }
        syntax::expression::ExpressionNode::Name(path) => {
            // A `Type::NAME` path naming a const substitutes a fresh copy of
            // its literal initializer (const-v0, crate::constant) -- only its
            // declaration-provenance symbol survives. Locals/fields are
            // single-segment paths and case constructors are checked against
            // consts at the const's declaration, so the intercept is
            // unambiguous.
            if let Some(substituted) = crate::constant::try_lower_const_reference(
                lowerer,
                syntax_trees,
                syntax_trees.expressions.identifier_path_members(*path),
            ) {
                return substituted;
            }
            let mut members = HandleSpan::empty();
            for member in syntax_trees.expressions.identifier_path_members(*path) {
                expression_table(lowerer).push_name_path_member(&mut members, lower_name(member));
            }
            let member_symbols =
                expression_table(lowerer).reserve_name_path_member_symbols(members.count());
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    is_self_value: false,
                    head_symbol: SymbolHandle::invalid(),
                    symbol: SymbolHandle::invalid(),
                })),
            )
        }
        syntax::expression::ExpressionNode::Range(range) => {
            let start = range
                .start
                .is_valid()
                .then(|| lower_expression_into_table(lowerer, syntax_trees, range.start))
                .transpose()?
                .unwrap_or_else(ExpressionHandle::invalid);
            let end = range
                .end
                .is_valid()
                .then(|| lower_expression_into_table(lowerer, syntax_trees, range.end))
                .transpose()?
                .unwrap_or_else(ExpressionHandle::invalid);
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                })),
            )
        }
        syntax::expression::ExpressionNode::SelfValue => {
            let mut members = HandleSpan::empty();
            expression_table(lowerer).push_name_path_member(
                &mut members,
                psi_symbol_resolved_trees::name::DiagnosticName::generated_static("self"),
            );
            let member_symbols =
                expression_table(lowerer).reserve_name_path_member_symbols(members.count());
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    is_self_value: true,
                    head_symbol: SymbolHandle::invalid(),
                    symbol: SymbolHandle::invalid(),
                })),
            )
        }
        syntax::expression::ExpressionNode::String(value) => {
            Ok(expression_table(lowerer).insert(ExpressionNode::String(value.clone())))
        }
        syntax::expression::ExpressionNode::Unary(unary) => {
            let operand = lower_expression_into_table(lowerer, syntax_trees, unary.operand)?;
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: lower_unary_operator(unary.operator),
                    operand,
                })),
            )
        }
        syntax::expression::ExpressionNode::ZeroValue(type_reference) => {
            let type_reference = crate::type_reference::lower_type_reference_handle(
                lowerer,
                syntax_trees,
                *type_reference,
            )?;
            let type_reference = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .child_type_references
                .append(type_reference);
            Ok(expression_table(lowerer).insert(ExpressionNode::ZeroValue(type_reference)))
        }
        syntax::expression::ExpressionNode::StructLiteral(struct_literal) => {
            let fields =
                expression_table(lowerer).reserve_struct_fields(struct_literal.fields.count());
            for (offset, field) in syntax_trees
                .expressions
                .struct_fields(struct_literal.fields)
                .iter()
                .enumerate()
            {
                let value = lower_expression_into_table(lowerer, syntax_trees, field.value)?;
                expression_table(lowerer).set_struct_field_at_offset(
                    fields,
                    offset
                        .try_into()
                        .expect("struct literal field span count overflow"),
                    TableStructLiteralField {
                        name: lower_name(&field.name),
                        field_symbol: SymbolHandle::invalid(),
                        value,
                    },
                );
            }
            let (type_name, case_name) =
                lower_struct_literal_shape_names(syntax_trees, struct_literal)?;
            Ok(
                expression_table(lowerer).insert(ExpressionNode::StructLiteral(
                    TableStructLiteral {
                        type_name,
                        type_symbol: SymbolHandle::invalid(),
                        case_name,
                        case_symbol: None,
                        fields,
                    },
                )),
            )
        }
    }
}

pub(crate) fn lower_static_machine_argument(
    argument: &syntax::expression::StaticMachineArgument,
) -> psi_symbol_resolved_trees::expression::StaticMachineArgument {
    psi_symbol_resolved_trees::expression::StaticMachineArgument {
        path: argument
            .path
            .iter()
            .map(lower_name)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        application: argument.application.as_ref().map(|application| {
            Box::new(
                psi_symbol_resolved_trees::expression::StaticSymbolApplication {
                    lifetime_arguments: application
                        .lifetime_arguments
                        .iter()
                        .map(lower_name)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    arguments: application
                        .arguments
                        .iter()
                        .map(lower_static_machine_argument)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                },
            )
        }),
        const_literal: argument.const_literal.clone(),
        evidence_projection: argument.evidence_projection.as_ref().map(|projection| {
            psi_symbol_resolved_trees::expression::EvidenceProjection {
                term: lower_name(&projection.term),
                member: lower_name(&projection.member),
            }
        }),
        symbol: SymbolHandle::invalid(),
    }
}

fn expression_table(lowerer: &mut Lowerer) -> &mut ExpressionTable {
    &mut lowerer.symbol_resolved_trees.tables.bodies.expressions
}

/// The lowered `(type_name, case_name)` pair of a brace literal. A two-member
/// literal `Type::Case { ... }` constructs an ordinary sum case.
fn lower_struct_literal_shape_names(
    _syntax_trees: &SyntaxTrees,
    struct_literal: &syntax::expression::TableStructLiteral,
) -> Result<
    (
        psi_symbol_resolved_trees::name::DiagnosticName,
        Option<psi_symbol_resolved_trees::name::DiagnosticName>,
    ),
    Diagnostic,
> {
    Ok((
        lower_name(&struct_literal.type_name),
        struct_literal.case_name.as_ref().map(lower_name),
    ))
}

fn lower_unary_operator(operator: syntax::expression::UnaryOperator) -> UnaryOperator {
    match operator {
        syntax::expression::UnaryOperator::BitwiseNot => UnaryOperator::BitwiseNot,
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
