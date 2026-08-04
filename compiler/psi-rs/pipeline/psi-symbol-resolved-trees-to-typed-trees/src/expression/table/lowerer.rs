use crate::equatable::EqualityScope;
use crate::expression::domain_membership::{
    lower_case_membership_expression, lower_domain_membership_expression,
};
use crate::expression::name_paths::{
    lower_name_path_members_into_table, lower_table_name_path_node_into_table,
};
use crate::expression::operators::lower_binary_operator;
use crate::name::lower_name;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

pub(super) struct ExpressionTableLowerer<'program, 'target, 'scope> {
    pub(super) program: Option<&'program resolved::SymbolResolvedTrees>,
    pub(super) source: &'program resolved::expression::ExpressionTable,
    pub(super) target_trees: &'target mut typed::TypedTrees,
    self_substitution: Option<typed::expression::ExpressionHandle>,
    pub(super) equality_scope: Option<&'scope EqualityScope>,
    /// PROOF-FACT position (contract/domain facts): equality over RECURSIVE
    /// (proof-only) data stays a raw Binary for the structural entailment
    /// judge instead of demanding runtime synthesis (math roster N3).
    pub(super) fact_position: bool,
}

impl<'program, 'target, 'scope> ExpressionTableLowerer<'program, 'target, 'scope> {
    pub(super) fn new(
        program: Option<&'program resolved::SymbolResolvedTrees>,
        source: &'program resolved::expression::ExpressionTable,
        target_trees: &'target mut typed::TypedTrees,
        self_substitution: Option<typed::expression::ExpressionHandle>,
        equality_scope: Option<&'scope EqualityScope>,
    ) -> Self {
        Self {
            program,
            source,
            target_trees,
            self_substitution,
            equality_scope,
            fact_position: false,
        }
    }

    pub(super) fn target(&mut self) -> &mut typed::expression::ExpressionTable {
        &mut self.target_trees.expression_table
    }

    pub(super) fn in_fact_position(mut self) -> Self {
        self.fact_position = true;
        self
    }

    pub(super) fn lower(
        &mut self,
        expression: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let source_span = self.source.source_span(expression);
        let lowered = self.lower_node(expression)?;
        self.target().set_source_span(lowered, source_span);
        Ok(lowered)
    }

    fn lower_node(
        &mut self,
        expression: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        match self.source.expression(expression) {
            resolved::expression::ExpressionNode::ArrayLiteral(values) => {
                let values = self.lower_expression_handle_span(*values)?;
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::ArrayLiteral(values)))
            }
            resolved::expression::ExpressionNode::Atomic(atomic) => {
                let value = self.lower(atomic.value)?;
                let result = if atomic.result.is_valid() {
                    self.lower(atomic.result)?
                } else {
                    typed::expression::ExpressionHandle::invalid()
                };
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Atomic(
                        typed::expression::TableAtomicExpression {
                            value,
                            result,
                            ordering: atomic.ordering,
                        },
                    )))
            }
            resolved::expression::ExpressionNode::Binary(binary) => {
                // `==`/`!=` on a conforming record / payload-bearing sum
                // expands to synthesized structural equality (decision 11).
                if matches!(
                    binary.operator,
                    resolved::expression::BinaryOperator::Equal
                        | resolved::expression::BinaryOperator::NotEqual
                ) && let Some(lowered) = self.try_lower_structural_equality(binary)?
                {
                    return Ok(lowered);
                }
                let left = self.lower(binary.left)?;
                let right = self.lower(binary.right)?;
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Binary(
                        typed::expression::TableBinaryExpression {
                            left,
                            operator: lower_binary_operator(binary.operator),
                            right,
                        },
                    )))
            }
            resolved::expression::ExpressionNode::Boolean(value) => Ok(self
                .target()
                .insert(typed::expression::ExpressionNode::Boolean(*value))),
            resolved::expression::ExpressionNode::Cast(cast) => {
                let value = self.lower(cast.value)?;
                let program = self.program.ok_or_else(|| {
                    Diagnostic::error(
                        "cast target type lowering requires the enclosing resolved program",
                    )
                })?;
                let target_type = crate::type_reference::lower_type_reference_into_trees(
                    program,
                    self.target_trees,
                    program.child_type_reference(cast.target_type),
                )?;
                let target_label = lower_name_path_members_into_table(
                    self.source,
                    self.target(),
                    cast.target_label,
                );
                let semantic_domain = lower_name_path_members_into_table(
                    self.source,
                    self.target(),
                    cast.semantic_domain,
                );
                let semantic_domain_arguments = program
                    .child_type_references(cast.semantic_domain_arguments)
                    .iter()
                    .map(|argument| {
                        crate::type_reference::lower_type_reference_into_trees(
                            program,
                            self.target_trees,
                            argument,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let semantic_domain_arguments = self
                    .target_trees
                    .type_reference_table
                    .insert_type_reference_handles(semantic_domain_arguments);
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Cast(
                        typed::expression::TableCastExpression {
                            value,
                            target_type,
                            target_label,
                            domain: cast.domain,
                            semantic_domain,
                            semantic_domain_arguments,
                            semantic_domain_symbol: cast.semantic_domain_symbol,
                            semantic_domain_id: psi_language_semantics::SemanticDomainId::NULL,
                            form: cast.form,
                        },
                    )))
            }
            resolved::expression::ExpressionNode::Call(call) => {
                if let Some(program) = self.program
                    && call.target_symbol.is_valid()
                    && matches!(
                        program.symbols.get(call.target_symbol).kind,
                        psi_symbols::SymbolKind::Proposition
                            | psi_symbols::SymbolKind::PropositionParameter
                    )
                {
                    return Err(Diagnostic::error(if self.fact_position {
                        "a proposition application must be the complete fact in this implementation slice"
                    } else {
                        "a proposition application is proof-only and cannot appear in a runtime value expression"
                    }));
                }
                if let Some(lowered) = self.try_lower_synthesized_equatable_call(call)? {
                    return Ok(lowered);
                }
                let receiver = self.lower_optional(call.receiver)?;
                let arguments = self.lower_expression_handle_span(call.arguments)?;
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Call(
                        typed::expression::TableCallExpression {
                            receiver,
                            target_symbol: call.target_symbol,
                            target: lower_name(&call.target),
                            machine_arguments: call
                                .machine_arguments
                                .iter()
                                .map(|argument| typed::expression::StaticMachineArgument {
                                    path: argument
                                        .path
                                        .iter()
                                        .map(lower_name)
                                        .collect::<Vec<_>>()
                                        .into_boxed_slice(),
                                    symbol: argument.symbol,
                                })
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                            arguments,
                            operational_acknowledgement: call.operational_acknowledgement,
                        },
                    )))
            }
            resolved::expression::ExpressionNode::Float(value) => {
                // The carrier CLONES across layers (spelling + landing ride);
                // rebuilding from the f64 value was the strip-on-lowering
                // disease the shared payload exists to kill.
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Float(value.clone())))
            }
            resolved::expression::ExpressionNode::Indexed(indexed) => {
                let collection = self.lower(indexed.collection)?;
                let index = self.lower(indexed.index)?;
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Indexed(
                        typed::expression::TableIndexedExpression { collection, index },
                    )))
            }
            resolved::expression::ExpressionNode::Integer(value) => Ok(self
                .target()
                .insert(typed::expression::ExpressionNode::Integer(value.clone()))),
            resolved::expression::ExpressionNode::Membership(membership) => {
                self.lower_membership_expression(membership)
            }
            resolved::expression::ExpressionNode::Member(member) => {
                let receiver = self.lower(member.receiver)?;
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Member(
                        typed::expression::TableMemberExpression {
                            receiver,
                            member_symbol: member.member_symbol,
                            member: lower_name(&member.member),
                            case_variant: member.case_variant.as_ref().map(lower_name),
                        },
                    )))
            }
            resolved::expression::ExpressionNode::Mutable(expression) => {
                let expression = self.lower(*expression)?;
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Mutable(expression)))
            }
            resolved::expression::ExpressionNode::Name(path) => {
                if path.is_self_value
                    && path.members.count() == 1
                    && let Some(substitution) = self.self_substitution
                {
                    return Ok(substitution);
                }
                let path = lower_table_name_path_node_into_table(self.source, self.target(), path);
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Name(path)))
            }
            resolved::expression::ExpressionNode::Range(range) => {
                let start = self.lower_optional(range.start)?;
                let end = self.lower_optional(range.end)?;
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Range(
                        typed::expression::TableRangeExpression {
                            start,
                            end,
                            end_inclusive: range.end_inclusive,
                        },
                    )))
            }
            resolved::expression::ExpressionNode::StructLiteral(struct_literal) => {
                let fields = self.lower_struct_literal_field_span(struct_literal.fields)?;
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::StructLiteral(
                        typed::expression::TableStructLiteral {
                            type_name: lower_name(&struct_literal.type_name),
                            case_name: struct_literal.case_name.as_ref().map(lower_name),
                            fields,
                        },
                    )))
            }
            resolved::expression::ExpressionNode::String(value) => {
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::String(
                        value.shared_text(),
                    )))
            }
            resolved::expression::ExpressionNode::Unary(unary) => {
                let operand = self.lower(unary.operand)?;
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Unary(
                        typed::expression::TableUnaryExpression {
                            operator: lower_unary_operator(unary.operator),
                            operand,
                        },
                    )))
            }
            resolved::expression::ExpressionNode::ZeroValue(type_reference) => {
                let program = self.program.ok_or_else(|| {
                    Diagnostic::error(
                        "zero-value type lowering requires the enclosing resolved program",
                    )
                })?;
                let type_reference = crate::type_reference::lower_type_reference_into_trees(
                    program,
                    self.target_trees,
                    program.child_type_reference(*type_reference),
                )?;
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::ZeroValue(type_reference)))
            }
        }
    }

    fn lower_optional(
        &mut self,
        expression: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        if !expression.is_valid() {
            return Ok(typed::expression::ExpressionHandle::invalid());
        }

        self.lower(expression)
    }

    fn lower_expression_handle_span(
        &mut self,
        expressions: psi_arena::HandleSpan<resolved::expression::ExpressionHandle>,
    ) -> Result<psi_arena::HandleSpan<typed::expression::ExpressionHandle>, Diagnostic> {
        let lowered = self
            .source
            .expression_handles(expressions)
            .iter()
            .copied()
            .map(|expression| self.lower(expression))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(self.target().insert_expression_handles(lowered))
    }

    fn lower_struct_literal_field_span(
        &mut self,
        fields: psi_arena::HandleSpan<resolved::expression::TableStructLiteralField>,
    ) -> Result<psi_arena::HandleSpan<typed::expression::TableStructLiteralField>, Diagnostic> {
        let lowered = self
            .source
            .struct_fields(fields)
            .iter()
            .map(|field| {
                let value = self.lower(field.value)?;
                Ok(typed::expression::TableStructLiteralField {
                    name: lower_name(&field.name),
                    value,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;

        Ok(self.target().insert_struct_fields(lowered))
    }

    fn lower_membership_expression(
        &mut self,
        membership: &resolved::expression::TableMembershipExpression,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let Some(program) = self.program else {
            return Err(Diagnostic::error(
                "cannot lower executable domain membership without a resolved program context",
            ));
        };
        if membership.domain_symbol.is_valid() {
            let value = self.lower(membership.value)?;
            return lower_domain_membership_expression(
                program,
                self.target_trees,
                value,
                membership.domain_symbol,
            );
        }

        // No DECLARED domain matches: a `Type::Case` path is an implicit
        // case domain (decision 11), lowered to the tag-equality compare. Guard
        // against a CROSS-TYPE case test (`color in Direction::North`) first -- it
        // would otherwise compare tags across unrelated enums (the membership
        // sibling of the cross-enum `==` check).
        self.reject_cross_type_case_membership(membership.value, membership.domain)?;
        let value = self.lower(membership.value)?;
        if let Some(lowered) = lower_case_membership_expression(
            program,
            self.source,
            self.target_trees,
            value,
            membership.domain,
        ) {
            return Ok(lowered);
        }

        let domain_name = resolved::expression::display_name_path(
            self.source.name_path_members(membership.domain),
            "::",
        );
        Err(Diagnostic::error(format!(
            "unknown domain `{domain_name}` in executable membership expression"
        )))
    }
}

fn lower_unary_operator(
    operator: resolved::expression::UnaryOperator,
) -> typed::expression::UnaryOperator {
    match operator {
        resolved::expression::UnaryOperator::LogicalNot => {
            typed::expression::UnaryOperator::LogicalNot
        }
    }
}
