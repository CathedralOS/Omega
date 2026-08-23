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

#[derive(Clone)]
struct NullaryErasedInitializer {
    field_name: resolved::name::DiagnosticName,
    type_name: resolved::name::DiagnosticName,
    type_symbol: psi_symbols::SymbolHandle,
    variant_name: resolved::name::DiagnosticName,
    variant_symbol: psi_symbols::SymbolHandle,
}

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
                let quotient_operation = self.lower_quotient_operation_request(call)?;
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
                                .map(crate::expression::lower_static_machine_argument)
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                            quotient_operation,
                            arguments,
                            evidence_arguments: call
                                .evidence_arguments
                                .iter()
                                .map(lower_name)
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
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
                let omitted = self.nullary_erased_initializers(struct_literal);
                let mut fields = self.lower_struct_literal_field_span(struct_literal.fields)?;
                for initializer in omitted {
                    let value = self.synthesize_nullary_initializer(&initializer);
                    self.target().push_struct_field(
                        &mut fields,
                        typed::expression::TableStructLiteralField {
                            name: lower_name(&initializer.field_name),
                            value,
                        },
                    );
                }
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
            resolved::expression::ExpressionNode::String(value) => Ok(self
                .target()
                .insert(typed::expression::ExpressionNode::String(value.clone()))),
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
                if let Some(quotient) = quotient_zero_value_target(
                    program,
                    program.child_type_reference(*type_reference),
                ) {
                    return Err(Diagnostic::error(format!(
                        "`zero_value<{quotient}>()` cannot observe or choose a retained quotient representative: quotient values are opaque and have no compiler-verified canonical representative; use a named lifted operation with its role-correctness contract"
                    )));
                }
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

    /// Recognize only the sealed source wrapper selected by the N6/N8
    /// quotient ruling. This retains the two exact resolved identities; it
    /// deliberately performs no quotient discovery and grants no executable
    /// lifting authority.
    fn lower_quotient_operation_request(
        &self,
        call: &resolved::expression::TableCallExpression,
    ) -> Result<Option<typed::expression::QuotientOperationRequest>, Diagnostic> {
        let kind = match call.target.as_str() {
            "lift" => typed::expression::QuotientOperationKind::Lift,
            "define" => typed::expression::QuotientOperationKind::Define,
            _ => return Ok(None),
        };
        if !call.receiver.is_valid() {
            return Ok(None);
        }
        let resolved::expression::ExpressionNode::Name(receiver) =
            self.source.expression(call.receiver)
        else {
            return Ok(None);
        };
        let [namespace] = self.source.name_path_members(receiver.members) else {
            return Ok(None);
        };
        if namespace.as_str() != "Quotient" {
            return Ok(None);
        }

        let Some(program) = self.program else {
            return Err(Diagnostic::error(
                "a sealed Quotient operation requires a complete resolved program",
            ));
        };
        if receiver.head_symbol.is_valid()
            || receiver.symbol.is_valid()
            || call.target_symbol.is_valid()
        {
            return Err(Diagnostic::error(
                "the sealed `Quotient` operation namespace cannot be shadowed by an authored declaration",
            ));
        }
        let [representative_operation, respect_conformance] = call.machine_arguments.as_ref()
        else {
            return Err(Diagnostic::error(format!(
                "`Quotient::{}` requires exactly two static arguments: one representative operation and one exact named conformance",
                call.target,
            )));
        };
        if representative_operation.const_literal.is_some()
            || representative_operation.evidence_projection.is_some()
            || !representative_operation.symbol.is_valid()
            || program.symbols.get(representative_operation.symbol).kind
                != psi_symbols::SymbolKind::State
        {
            return Err(Diagnostic::error(format!(
                "the first static argument to `Quotient::{}` must resolve exactly to a representative machine entry",
                call.target,
            )));
        }
        if respect_conformance.const_literal.is_some()
            || respect_conformance.evidence_projection.is_some()
            || !respect_conformance.symbol.is_valid()
            || program.symbols.get(respect_conformance.symbol).kind
                != psi_symbols::SymbolKind::Conformance
        {
            return Err(Diagnostic::error(format!(
                "the second static argument to `Quotient::{}` must resolve exactly to one named conformance; structural proof-machine discovery is not permitted",
                call.target,
            )));
        }

        Ok(Some(typed::expression::QuotientOperationRequest {
            kind,
            representative_operation: crate::expression::lower_static_machine_argument(
                representative_operation,
            ),
            respect_conformance: crate::expression::lower_static_machine_argument(
                respect_conformance,
            ),
        }))
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

    /// Elaborate the one settled omission rule for erased bindings. The
    /// semantic typed tree receives the same `Evidence::Only` term that an
    /// author could have supplied explicitly; runtime erasure happens later.
    /// No default-value or general inhabitance search is performed.
    fn nullary_erased_initializers(
        &self,
        literal: &resolved::expression::TableStructLiteral,
    ) -> Vec<NullaryErasedInitializer> {
        let Some(program) = self.program else {
            return Vec::new();
        };
        let Some(holder) = program.data_definitions.iter().find(|definition| {
            definition.name == literal.type_name
                && definition.symbol.is_valid()
                && definition.type_parameters.is_empty()
                && definition.supply_mode == psi_language_semantics::DataSupplyMode::CheckedShape
        }) else {
            return Vec::new();
        };
        let authored = self.source.struct_fields(literal.fields);
        let mut selected_fields = Vec::new();
        for member in program.data_members(holder.members) {
            match member {
                resolved::data::DataMember::Field(field) => selected_fields.push(field),
                resolved::data::DataMember::Variant(variant)
                    if literal
                        .case_name
                        .as_ref()
                        .is_some_and(|case_name| *case_name == variant.name) =>
                {
                    selected_fields.extend(program.data_payload_fields(variant.payload));
                }
                resolved::data::DataMember::Variant(_) => {}
            }
        }

        selected_fields
            .into_iter()
            .filter(|field| field.relevance.is_erased())
            .filter(|field| !authored.iter().any(|authored| authored.name == field.name))
            .filter_map(|field| {
                let evidence = resolved_data_definition_for_type(program, &field.type_reference)?;
                if evidence.supply_mode != psi_language_semantics::DataSupplyMode::CheckedShape
                    || !evidence.symbol.is_valid()
                    || !evidence.type_parameters.is_empty()
                    // Orchestration names a synthesized closed generic
                    // definition by its exact application. It is executable,
                    // but the settled omission rule still requires a
                    // non-generic evidence declaration rather than deriving
                    // inhabitance from a closed generic application.
                    || evidence.name.as_str().contains('<')
                    || program
                        .data_members(evidence.members)
                        .iter()
                        .any(|member| matches!(member, resolved::data::DataMember::Field(_)))
                {
                    return None;
                }
                let mut nullary = program.data_members(evidence.members).iter().filter_map(
                    |member| match member {
                        resolved::data::DataMember::Variant(variant)
                            if variant.payload.is_empty() && variant.symbol.is_valid() =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    },
                );
                let variant = nullary.next()?;
                if nullary.next().is_some() {
                    return None;
                }
                Some(NullaryErasedInitializer {
                    field_name: field.name.clone(),
                    type_name: evidence.name.clone(),
                    type_symbol: evidence.symbol,
                    variant_name: variant.name.clone(),
                    variant_symbol: variant.symbol,
                })
            })
            .collect()
    }

    fn synthesize_nullary_initializer(
        &mut self,
        initializer: &NullaryErasedInitializer,
    ) -> typed::expression::ExpressionHandle {
        let mut members = psi_arena::HandleSpan::empty();
        let mut member_symbols = psi_arena::HandleSpan::empty();
        self.target()
            .push_name_path_member(&mut members, lower_name(&initializer.type_name));
        self.target()
            .push_name_path_member_symbol(&mut member_symbols, initializer.type_symbol);
        self.target()
            .push_name_path_member(&mut members, lower_name(&initializer.variant_name));
        self.target()
            .push_name_path_member_symbol(&mut member_symbols, initializer.variant_symbol);
        self.target()
            .insert(typed::expression::ExpressionNode::Name(
                typed::expression::TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: initializer.type_symbol,
                    symbol: initializer.variant_symbol,
                },
            ))
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

fn quotient_zero_value_target(
    program: &resolved::SymbolResolvedTrees,
    type_reference: &resolved::types::TypeReference,
) -> Option<String> {
    resolved_data_definition_for_type(program, type_reference)
        .filter(|definition| definition.quotient.is_some())
        .map(|definition| definition.name.as_str().to_owned())
}

fn resolved_data_definition_for_type<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    type_reference: &'program resolved::types::TypeReference,
) -> Option<&'program resolved::data::DataDefinition> {
    let (symbol, name) = match type_reference {
        resolved::types::TypeReference::Named { symbol, name } => (*symbol, Some(name)),
        resolved::types::TypeReference::Generic(reference) => {
            (reference.base_symbol, Some(&reference.base_name))
        }
        resolved::types::TypeReference::SelfType { symbol } => (*symbol, None),
        resolved::types::TypeReference::Constrained(reference) => {
            return resolved_data_definition_for_type(
                program,
                program.child_type_reference(reference.base_type),
            );
        }
        _ => return None,
    };
    program.data_definitions.iter().find(|definition| {
        if symbol.is_valid() {
            definition.symbol == symbol
        } else {
            name.is_some_and(|name| definition.name == *name)
        }
    })
}

fn lower_unary_operator(
    operator: resolved::expression::UnaryOperator,
) -> typed::expression::UnaryOperator {
    match operator {
        resolved::expression::UnaryOperator::BitwiseNot => {
            typed::expression::UnaryOperator::BitwiseNot
        }
        resolved::expression::UnaryOperator::LogicalNot => {
            typed::expression::UnaryOperator::LogicalNot
        }
    }
}
