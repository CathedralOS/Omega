//! Expressions into the shared expression table.
//!
//! Authored state-body expressions are retained for declaration-selection
//! admission; compiler-generated inserts are not. Static call receivers are
//! recorded as pending module calls for `symbols` to normalize.

use crate::lowering::name::lower_name;
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable, FloatLiteral,
    TableAtomicExpression, TableBinaryExpression, TableCallExpression, TableCastExpression,
    TableIndexedExpression, TableMemberExpression, TableMembershipExpression, TableNamePath,
    TableRangeExpression, TableStructLiteral, TableStructLiteralField, TableUnaryExpression,
    UnaryOperator,
};
use symbols::SymbolHandle;
use syntax_trees as syntax;
use syntax_trees::SyntaxTrees;

pub(crate) fn lower_expression_into_table(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
) -> Result<ExpressionHandle, Diagnostic> {
    let source_span = syntax_trees.expressions.source_span(expression);
    // Parser-created payload receivers share the dispatch subject's exact
    // syntax handle. Once that occurrence has a capture, every later use reads
    // the capture, never a new evaluation of its collection or selector.
    let lowered = if let Some((name, _)) = lowerer.match_subject_temp(expression) {
        crate::lowering::statement::hoist_temps::reference_temp(
            lowerer,
            symbol_resolved_trees::name::DiagnosticName::generated(name),
        )
    } else {
        lower_expression_node_into_table(
            lowerer,
            syntax_trees,
            syntax_trees.expressions.expression(expression),
        )?
    };
    expression_table(lowerer).set_source_span(lowered, source_span);
    if let syntax::expression::ExpressionNode::Call(call) =
        syntax_trees.expressions.expression(expression)
        && call.target_is_static
        && call.receiver.is_valid()
        && let syntax::expression::ExpressionNode::Name(receiver) =
            syntax_trees.expressions.expression(call.receiver)
    {
        let mut path = syntax_trees
            .expressions
            .identifier_path_members(*receiver)
            .iter()
            .map(lower_name)
            .collect::<Vec<_>>();
        path.push(lower_name(&call.target));
        lowerer.pending_static_module_calls.push((lowered, path));
    }
    if let Some(exposure) = lowerer.current_authored_expression_exposure {
        expression_table(lowerer).set_authored_expression_exposure(lowered, exposure);
        lowerer.pending_authored_expressions.push(
            crate::resolution::lowerer::PendingAuthoredExpression {
                expression: lowered,
                exposure,
            },
        );
    }
    if let Some(partition) = lowerer.current_compiler_selection_partition {
        expression_table(lowerer).set_compiler_selection_partition(lowered, partition);
    }
    if let Some(sources) = &mut lowerer.equation_sources {
        sources.expressions.push((expression, lowered));
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
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;
    let exposure = if syntax_trees
        .roots
        .constant_extent_probes
        .iter()
        .any(|(root, public)| *root == expression && *public)
    {
        Exposure::PublicInterface
    } else {
        Exposure::PrivateImplementation
    };
    let previous = lowerer
        .current_authored_expression_exposure
        .replace(exposure);
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
        syntax::expression::ExpressionNode::TypeExpression(_) => Err(Diagnostic::error(
            "a type expression must be resolved by a static type equation; it is not a runtime value",
        )),
        syntax::expression::ExpressionNode::Match(dispatch) => {
            let subject = lower_expression_into_table(lowerer, syntax_trees, dispatch.subject)?;
            let mut arms = Vec::new();
            for arm in syntax_trees.expressions.match_arms(dispatch.arms) {
                let pattern = match arm.pattern {
                    syntax::expression::MatchPattern::Value(pattern) => {
                        symbol_resolved_trees::expression::MatchPattern::Value(
                            lower_expression_into_table(lowerer, syntax_trees, pattern)?,
                        )
                    }
                    syntax::expression::MatchPattern::Wildcard => {
                        symbol_resolved_trees::expression::MatchPattern::Wildcard
                    }
                };
                let value = lower_expression_into_table(lowerer, syntax_trees, arm.value)?;
                arms.push(symbol_resolved_trees::expression::TableMatchArm {
                    pattern,
                    value,
                    source_span: arm.source_span,
                });
            }
            let arms = expression_table(lowerer).insert_match_arms(arms);
            Ok(expression_table(lowerer).insert(ExpressionNode::Match(
                symbol_resolved_trees::expression::TableMatchExpression { subject, arms },
            )))
        }
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
            if !atomic.result_custody.is_valid_for(atomic.ordering)
                || (atomic.result_custody.requires_result_destination()
                    && !atomic.result.is_valid())
            {
                return Err(Diagnostic::error(
                    "atomic expression result custody does not match its operation axis",
                ));
            }
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
                    result_custody: atomic.result_custody,
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
            let target_type = crate::lowering::type_reference::lower_type_reference_handle(
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
            let semantic_domain_arguments =
                crate::lowering::type_reference::lower_child_type_references(
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
            // `Requirement<Arguments>::operation(...)`: the applied trait is a
            // static qualifier, not a runtime receiver. The call keeps the
            // trait path as its qualifier and the application as a child type
            // reference for typing to resolve the exact applied requirement.
            let (receiver, requirement_application) = match call
                .receiver
                .is_valid()
                .then(|| syntax_trees.expressions.expression(call.receiver))
            {
                Some(syntax::expression::ExpressionNode::TypeExpression(application))
                    if call.target_is_static =>
                {
                    lower_applied_requirement_qualifier(lowerer, syntax_trees, *application)?
                }
                Some(_) => (
                    lower_expression_into_table(lowerer, syntax_trees, call.receiver)?,
                    arena::Handle::invalid(),
                ),
                None => (ExpressionHandle::invalid(), arena::Handle::invalid()),
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
            let machine_arguments = call
                .machine_arguments
                .iter()
                .map(|argument| lower_static_machine_argument(lowerer, syntax_trees, argument))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: SymbolHandle::invalid(),
                    target: lower_name(&call.target),
                    requirement_application,
                    machine_arguments,
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
        syntax::expression::ExpressionNode::UndeterminedNaN(format) => {
            Ok(expression_table(lowerer).insert(ExpressionNode::UndeterminedNaN(*format)))
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
                symbol_resolved_trees::expression::TableBorrowExpression {
                    target,
                    access: expression.access,
                },
            )))
        }
        syntax::expression::ExpressionNode::Name(path) => {
            // Constants share ordinary lexical and namespace resolution. Their
            // selected initializer is copied only after exact symbols exist.
            let mut members = HandleSpan::empty();
            for member in syntax_trees.expressions.identifier_path_members(*path) {
                expression_table(lowerer).push_name_path_member(&mut members, lower_name(member));
            }
            let member_symbols =
                expression_table(lowerer).reserve_name_path_member_symbols(members.count());
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Name(TableNamePath::new(
                    members,
                    member_symbols,
                    SymbolHandle::invalid(),
                    SymbolHandle::invalid(),
                ))),
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
            let type_reference = crate::lowering::type_reference::lower_type_reference_handle(
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
            // Keep the complete spelling until symbol assignment selects the
            // exact nominal owner and its optional case.
            Ok(
                expression_table(lowerer).insert(ExpressionNode::StructLiteral(
                    TableStructLiteral {
                        type_name: lower_name(&struct_literal.constructor_name),
                        type_symbol: SymbolHandle::invalid(),
                        case_name: None,
                        case_symbol: None,
                        fields,
                    },
                )),
            )
        }
    }
}

/// Lower an applied requirement qualifier into the trait path a static call
/// names and the exact application kept beside it. Only a generic trait path
/// qualifies; any other type expression stays an invalid runtime receiver.
fn lower_applied_requirement_qualifier(
    lowerer: &mut Lowerer,
    syntax_trees: &syntax::SyntaxTrees,
    application: syntax::types::TypeReferenceHandle,
) -> Result<
    (
        ExpressionHandle,
        arena::Handle<symbol_resolved_trees::types::TypeReference>,
    ),
    Diagnostic,
> {
    let syntax::types::TypeReferenceNode::Generic { base_name, .. } =
        syntax_trees.type_references.type_reference(application)
    else {
        return Err(Diagnostic::error(
            "an applied requirement call names a generic trait application before `::`",
        ));
    };
    let mut path = Default::default();
    for member in base_name.as_str().split("::") {
        expression_table(lowerer).push_name_path_member(
            &mut path,
            symbol_resolved_trees::name::DiagnosticName::from_str(member, base_name.source_span()),
        );
    }
    let receiver = expression_table(lowerer).insert(ExpressionNode::Name(
        symbol_resolved_trees::expression::TableNamePath {
            members: path,
            ..Default::default()
        },
    ));
    expression_table(lowerer).set_source_span(receiver, base_name.source_span());
    let reference = crate::lowering::type_reference::lower_type_reference_handle(
        lowerer,
        syntax_trees,
        application,
    )?;
    let application = lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .child_type_references
        .append(reference);
    Ok((receiver, application))
}

pub(crate) fn lower_static_machine_argument(
    lowerer: &mut Lowerer,
    syntax_trees: &syntax::SyntaxTrees,
    argument: &syntax::expression::StaticMachineArgument,
) -> Result<symbol_resolved_trees::expression::StaticMachineArgument, Diagnostic> {
    let type_reference = if argument.type_reference.is_valid() {
        let reference = crate::lowering::type_reference::lower_type_reference_handle(
            lowerer,
            syntax_trees,
            argument.type_reference,
        )?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .child_type_references
            .append(reference)
    } else {
        arena::Handle::invalid()
    };
    Ok(symbol_resolved_trees::expression::StaticMachineArgument {
        type_reference,
        path: argument
            .path
            .iter()
            .map(lower_name)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        application: argument
            .application
            .as_ref()
            .map(|application| {
                Ok::<_, Diagnostic>(Box::new(
                    symbol_resolved_trees::expression::StaticSymbolApplication {
                        lifetime_arguments: application
                            .lifetime_arguments
                            .iter()
                            .map(lower_name)
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        arguments: application
                            .arguments
                            .iter()
                            .map(|argument| {
                                lower_static_machine_argument(lowerer, syntax_trees, argument)
                            })
                            .collect::<Result<Vec<_>, _>>()?
                            .into_boxed_slice(),
                    },
                ))
            })
            .transpose()?,
        const_literal: argument.const_literal.clone(),
        evidence_projection: argument.evidence_projection.as_ref().map(|projection| {
            symbol_resolved_trees::expression::EvidenceProjection {
                term: lower_name(&projection.term),
                member: lower_name(&projection.member),
            }
        }),
        symbol: SymbolHandle::invalid(),
    })
}

pub(super) fn expression_table(lowerer: &mut Lowerer) -> &mut ExpressionTable {
    &mut lowerer.symbol_resolved_trees.tables.bodies.expressions
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
