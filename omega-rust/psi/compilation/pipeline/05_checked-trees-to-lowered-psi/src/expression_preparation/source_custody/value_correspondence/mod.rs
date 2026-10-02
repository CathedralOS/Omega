//! Value correspondence complements the shared occurrence and read custody.
//! This matches retained operations; it never produces replacement expressions.
//! The caller rejoins namespaces, destinations, and call operand scopes. This
//! module preserves their literal/operator meaning. Terminal independently
//! proves emitted partial-operation obligations; checked range annotations are
//! not substituted for those proofs.
//! An eliminated subslice extent additionally needs statically established
//! formation bounds and an effect-free source. General slice-backed extents
//! need a retained view/bounds execution plan, which array operands do not yet
//! carry; matching literal endpoints alone never supplies that evidence.
//! Array elements and case fields share this receiving check.

use crate::lowering_error::LoweringError;
use checked_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedCallScalarArgument, CheckedIeeeFloatComparisonKind,
    CheckedIntegerBinaryKind as IntegerBinary, CheckedIntegerComparisonKind,
    CheckedScalarComputationHandle, CheckedScalarComputationKind as Computation,
    CheckedScalarExpression as Scalar, CheckedTrees,
};
use language_semantics::declaration_selection::CollectionMeasure;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;

mod extents;
mod integer_math;
pub(crate) use integer_math::validate_embedding_sources;

pub(crate) fn validate(
    checked: &CheckedTrees,
    state: SymbolHandle,
    statement: u32,
    source_expression: ExpressionHandle,
    primitive: PrimitiveType,
    element: &CheckedCallScalarArgument,
) -> Result<(), LoweringError> {
    let context = Context {
        checked,
        state,
        statement,
    };
    context.validate_extent_sources(source_expression, element)?;
    let valid = match element {
        CheckedCallScalarArgument::Pure(value) => {
            value.primitive_type() == Some(primitive)
                && context.scalar(source_expression, value, &[], 0)
        }
        CheckedCallScalarArgument::Computation(root) => {
            let plans = &checked.facts.values.scalar_computations;
            plans.nodes.is_valid(*root)
                && plans.nodes.get(*root).primitive_type == primitive
                && context.computation(*root, &mut Vec::new())
        }
    };
    if valid {
        Ok(())
    } else {
        Err(LoweringError::Unsupported(
            "scalar operand value differs from its authored expression",
        ))
    }
}

struct Context<'a> {
    checked: &'a CheckedTrees,
    state: SymbolHandle,
    statement: u32,
}

#[cfg(test)]
mod byte_comparison_tests {
    use super::*;

    #[test]
    fn expanded_comparison_rejects_missing_bytes_and_substituted_storage() {
        let checked = crate::front_end::checked_program(
            r#"
            domain [u8; 8]::Utf8(bytes) requires valid_utf8(bytes);
            machine compare() -> bool {
                let text: [u8; 8] in Utf8 = "Ada";
                transition text == "Ada" { true -> (true) _ -> (false) }
            }
        "#,
        );
        let plans = &checked.facts.values.scalar_expressions;
        let (_, binding) = plans
            .source_bindings
            .iter()
            .find(|(_, binding)| binding.role == checked_trees::CheckedScalarExpressionRole::Guard)
            .expect("comparison source");
        let retained = plans
            .expression_at(binding.state, binding.statement_ordinal, binding.role)
            .expect("comparison expansion");
        let validate = |value: &Scalar| {
            super::super::storage_reads::validate_expression(
                &checked,
                binding.state,
                binding.statement_ordinal,
                super::super::storage_reads::guard_subject(&checked, binding.expression),
                value,
            )
        };
        validate(retained).expect("exact byte comparison");
        let Scalar::Boolean(value) = retained else {
            panic!("Boolean expansion")
        };
        let Boolean::And { left, .. } = value.as_ref() else {
            panic!("byte conjunction")
        };
        assert!(
            validate(&Scalar::Boolean(left.clone())).is_err(),
            "last byte cannot disappear"
        );

        let mut changed = retained.clone();
        let Scalar::Boolean(value) = &mut changed else {
            unreachable!()
        };
        let mut value = value.as_mut();
        while let Boolean::And { left, .. } = value {
            value = left.as_mut();
        }
        let Boolean::IntegerComparison { left, .. } = value else {
            panic!("length equality")
        };
        let Scalar::StructuralParameterByteLength { root, .. } = left.as_mut() else {
            panic!("live length")
        };
        *root = checked_trees::CheckedStorageRoot::OwnedLocal {
            symbol: SymbolHandle::invalid(),
        };
        assert!(
            validate(&changed).is_err(),
            "length must come from the compared local"
        );
    }
}

/// Content equality expands one source comparison into length and indexed
/// observations. Rejoin that complete expansion instead of treating its
/// synthetic reads as additional authored indexing operations.
pub(super) fn matches_expanded_byte_comparison(
    checked: &CheckedTrees,
    state: SymbolHandle,
    statement: u32,
    source: ExpressionHandle,
    retained: &Scalar,
) -> bool {
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(source) else {
        return false;
    };
    let Scalar::Boolean(value) = retained else {
        return false;
    };
    let value = match (binary.operator, value.as_ref()) {
        (BinaryOperator::Equal, value) => value,
        (BinaryOperator::NotEqual, Boolean::Not(value)) => value.as_ref(),
        _ => return false,
    };
    let context = Context {
        checked,
        state,
        statement,
    };
    context.carrier_literal_equality(source, binary.left, binary.right, value)
        || context.carrier_pair_equality(source, binary.left, binary.right, value)
}

impl Context<'_> {
    fn selected_builtin(&self, source: ExpressionHandle) -> bool {
        let expected = match self.checked.expression_table.expression(source) {
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::Add => Some(language_core::OperatorSpelling::Add),
                BinaryOperator::Subtract => Some(language_core::OperatorSpelling::Subtract),
                BinaryOperator::Multiply => Some(language_core::OperatorSpelling::Multiply),
                BinaryOperator::Divide => Some(language_core::OperatorSpelling::Divide),
                BinaryOperator::Modulo => Some(language_core::OperatorSpelling::Modulo),
                BinaryOperator::Equal => Some(language_core::OperatorSpelling::Equal),
                BinaryOperator::NotEqual => Some(language_core::OperatorSpelling::NotEqual),
                BinaryOperator::Less => Some(language_core::OperatorSpelling::Less),
                BinaryOperator::LessOrEqual => Some(language_core::OperatorSpelling::LessEqual),
                BinaryOperator::Greater => Some(language_core::OperatorSpelling::Greater),
                BinaryOperator::GreaterOrEqual => {
                    Some(language_core::OperatorSpelling::GreaterEqual)
                }
                _ => None,
            },
            _ => None,
        };
        let mut matching = self
            .checked
            .facts
            .operators
            .uses
            .iter()
            .map(|(_, selected)| selected)
            .filter(|selected| selected.expression == source);
        let Some(selected) = matching.next() else {
            return true;
        };
        if matching.next().is_some()
            || Some(selected.spelling) != expected
            || selected.status != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            || selected.selected_operator_symbol.is_valid()
        {
            return false;
        }
        let Some(candidates) = self
            .checked
            .facts
            .operators
            .candidates
            .span(selected.candidates)
        else {
            return false;
        };
        // Fallback may retain inadmissible domain candidates, but never a root
        // candidate or a contradictory selected declaration.
        candidates.len() == selected.candidate_count
            && candidates
                .iter()
                .all(|candidate| candidate.is_domain_owned())
    }

    fn anonymous_builtin(&self, source: ExpressionHandle) -> bool {
        if !self.selected_builtin(source) {
            return false;
        }
        match self.checked.facts.operators.expression_use(source) {
            Some(selected) => {
                selected.status == checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            }
            None => validation::has_anonymous_operator_meaning(&self.checked.typed, source),
        }
    }

    fn builtin(&self, source: ExpressionHandle) -> bool {
        if !self.selected_builtin(source) {
            return false;
        }
        let Ok((machine, state)) =
            crate::expression_preparation::source_custody::authored_state(self.checked, self.state)
        else {
            return false;
        };
        // Children are matched by this traversal. The whole-tree bound helper
        // has a separate proof-query depth limit, not an operand admission rule.
        match self.checked.expression_table.expression(source) {
            ExpressionNode::Binary(_) => validation::has_builtin_binary_expression_meaning(
                &self.checked.typed,
                machine,
                Some(state),
                source,
            ),
            // Neither unary spelling is overloadable; casts retain policy in
            // their own typed node and are checked by scalar correspondence.
            _ => true,
        }
    }

    fn computation(
        &self,
        root: CheckedScalarComputationHandle,
        active: &mut Vec<CheckedScalarComputationHandle>,
    ) -> bool {
        let plans = &self.checked.facts.values.scalar_computations;
        if !plans.nodes.is_valid(root) || active.contains(&root) {
            return false;
        }
        active.push(root);
        let node = plans.nodes.get(root);
        let valid = match &node.kind {
            Computation::ByteSequenceLiteralComparison {
                operand_type,
                source_expression,
                subject,
            } => {
                node.primitive_type == PrimitiveType::Bool
                    && super::computation_calls::byte_comparisons::validate(
                        self.checked,
                        self.state,
                        self.statement,
                        *source_expression,
                        *operand_type,
                        subject,
                    )
                    .is_ok()
            }
            Computation::StructuralField {
                source_expression,
                subject,
                field,
            } => crate::expression_preparation::source_custody::authored_state(
                self.checked,
                self.state,
            )
            .is_ok_and(|(machine, _)| {
                crate::expression_preparation::source_custody::record_fields::validate_source(
                    self.checked,
                    machine.symbol,
                    self.state,
                    self.statement,
                    *source_expression,
                    subject,
                    *field,
                    node.primitive_type,
                )
                .is_ok()
            }),
            Computation::CaseMembership {
                source_expression,
                subject,
                case,
            } => {
                // Membership produces a Boolean only after exact source/case
                // replay and ordinary value correspondence for every field.
                // Its truth is not assumed, and field bounds remain owed.
                node.primitive_type == PrimitiveType::Bool
                    && crate::expression_preparation::source_custody::authored_state(
                        self.checked,
                        self.state,
                    )
                    .is_ok_and(|(machine, _)| {
                        crate::expression_preparation::source_custody::case_sources::membership(
                            self.checked,
                            machine.symbol,
                            self.state,
                            *source_expression,
                            subject,
                            *case,
                        )
                        .is_ok_and(|fields| {
                            fields
                                .iter()
                                .all(|(_, field)| self.computation(*field, active))
                        })
                    })
            }
            Computation::SelectedComparison {
                operator_use,
                left,
                right,
            } => {
                self.checked
                    .facts
                    .operators
                    .selected_float_comparison(&self.checked.typed, *operator_use)
                    .is_some_and(|(_, primitive)| {
                        node.primitive_type == PrimitiveType::Bool
                            && plans.nodes.get(*left).primitive_type == primitive
                            && plans.nodes.get(*right).primitive_type == primitive
                    })
                    && self.computation(*left, active)
                    && self.computation(*right, active)
            }
            // Membership and the exact cast operand are replayed by shared
            // source custody; qualification does not change payload meaning.
            Computation::Qualification { operand, .. } => {
                plans.nodes.is_valid(*operand)
                    && plans.nodes.get(*operand).primitive_type == node.primitive_type
                    && self.computation(*operand, active)
            }
            // The authored conversion owns its cast occurrence; the operand
            // keeps the cast's source carrier — replayed by shared source
            // custody — while the node still lands at its typed result.
            Computation::AuthoredCast {
                operand,
                result_type,
                ..
            } => {
                plans.nodes.is_valid(*operand)
                    && self.checked.primitive_type_reference(*result_type)
                        == Some(node.primitive_type)
                    && self.computation(*operand, active)
            }
            // The authored occurrence is an ordinary value cast to a fixed
            // integer target; the operand's own correspondence replays the
            // Boolean computation at cast.value. The 0/1 landing is
            // definitional, so no policy or literal evidence is owed here.
            Computation::BooleanToInteger {
                source_expression,
                operand,
            } => {
                let ExpressionNode::Cast(cast) =
                    self.checked.expression_table.expression(*source_expression)
                else {
                    active.pop();
                    return false;
                };
                !cast.form.is_recast()
                    && cast.semantic_domain.is_empty()
                    && plans.nodes.is_valid(*operand)
                    && plans.nodes.get(*operand).primitive_type == PrimitiveType::Bool
                    && matches!(
                        node.primitive_type,
                        PrimitiveType::I8
                            | PrimitiveType::I16
                            | PrimitiveType::I32
                            | PrimitiveType::I64
                            | PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                    )
                    && self.checked.primitive_type_reference(cast.target_type)
                        == Some(node.primitive_type)
                    && self.computation(*operand, active)
            }
            Computation::Dispatch { subject, arms, .. } => {
                // The caller independently rejoins ordered source arms. Replay
                // every retained child's literal/operator meaning here.
                self.computation(*subject, active)
                    && plans.dispatch_arms.span(*arms).is_some_and(|arms| {
                        arms.iter().all(|arm| {
                            (match arm.pattern {
                                checked_trees::CheckedScalarDispatchPattern::Value(pattern) => {
                                    self.computation(pattern, active)
                                }
                                checked_trees::CheckedScalarDispatchPattern::Wildcard => true,
                            }) && self.computation(arm.value, active)
                        })
                    })
            }
            Computation::Value(value) => {
                value.primitive_type() == Some(node.primitive_type)
                    && node.value_source.is_valid()
                    && self.scalar(node.value_source, value, &[], 0)
            }
            Computation::Call { arguments, .. } | Computation::BoundaryCall { arguments, .. } => {
                plans
                    .operands
                    .span_or_empty(*arguments)
                    .iter()
                    .all(|argument| self.computation(*argument, active))
            }
            Computation::Apply {
                source_expression,
                expression,
                operands,
            } => {
                // A synthesized enum comparison owns its complete ordered
                // observation roster. Its children cannot independently claim
                // to implement the enclosing equality source occurrence.
                match super::enum_equality::validate_application(self.checked, self.state, root) {
                    Ok(true) => {
                        active.pop();
                        return true;
                    }
                    Err(_) => {
                        active.pop();
                        return false;
                    }
                    Ok(false) => {}
                }
                let operands = plans.operands.span_or_empty(*operands);
                let sources = match self.checked.expression_table.expression(*source_expression) {
                    ExpressionNode::Binary(binary) if operands.len() == 2 => {
                        vec![binary.left, binary.right]
                    }
                    ExpressionNode::Unary(unary) if operands.len() == 1 => vec![unary.operand],
                    ExpressionNode::Cast(cast) if operands.len() == 1 => vec![cast.value],
                    _ => {
                        active.pop();
                        return false;
                    }
                };
                let Some(sources) = sources
                    .into_iter()
                    .zip(operands)
                    .map(|(source, operand)| {
                        plans
                            .nodes
                            .is_valid(*operand)
                            .then(|| (source, plans.nodes.get(*operand).primitive_type))
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    active.pop();
                    return false;
                };
                expression.primitive_type() == Some(node.primitive_type)
                    && self.scalar(*source_expression, expression, &sources, 0)
                    && operands
                        .iter()
                        .all(|operand| self.computation(*operand, active))
            }
            Computation::Select {
                source_expression,
                condition,
                when_true,
                when_false,
            } => {
                let ExpressionNode::Binary(binary) =
                    self.checked.expression_table.expression(*source_expression)
                else {
                    active.pop();
                    return false;
                };
                let (selected, skipped, skipped_value) = match binary.operator {
                    BinaryOperator::And => (*when_true, *when_false, false),
                    BinaryOperator::Or => (*when_false, *when_true, true),
                    _ => {
                        active.pop();
                        return false;
                    }
                };
                node.primitive_type == PrimitiveType::Bool
                    && self.builtin(*source_expression)
                    && plans.nodes.is_valid(skipped)
                    && plans.nodes.get(skipped).primitive_type == PrimitiveType::Bool
                    && matches!(&plans.nodes.get(skipped).kind, Computation::Value(Scalar::Boolean(value)) if **value == Boolean::Constant(skipped_value))
                    && plans.nodes.is_valid(*condition)
                    && plans.nodes.get(*condition).primitive_type == PrimitiveType::Bool
                    && plans.nodes.is_valid(selected)
                    && plans.nodes.get(selected).primitive_type == PrimitiveType::Bool
                    && self.computation(*condition, active)
                    && self.computation(selected, active)
            }
        };
        active.pop();
        valid
    }

    fn scalar(
        &self,
        source: ExpressionHandle,
        value: &Scalar,
        operands: &[(ExpressionHandle, PrimitiveType)],
        depth: usize,
    ) -> bool {
        if self.exhausted(depth) || !self.checked.expression_table.expression_is_valid(source) {
            return false;
        }
        if let Some((selected, primitive)) =
            validation::closed_record_scalar_projection(&self.checked.typed, source)
        {
            return value.primitive_type() == Some(primitive)
                && self.scalar(selected, value, operands, depth + 1);
        }
        if let Some(selected) = self.projected_literal(source) {
            return self.scalar(selected, value, operands, depth + 1);
        }
        let node = self.checked.expression_table.expression(source);
        if let Scalar::Parameter {
            position,
            primitive_type,
        } = value
            && !operands.is_empty()
        {
            return operands.get(*position) == Some(&(source, *primitive_type));
        }
        // Same-carrier qualification casts retain their operand payload.
        if let ExpressionNode::Cast(cast) = node
            && !matches!(
                value,
                Scalar::IntegerWiden { .. }
                    | Scalar::IntegerExactCast { .. }
                    | Scalar::IntegerWrappingCast { .. }
                    | Scalar::IntegerSaturatingCast { .. }
                    | Scalar::IntegerTrappingCast { .. }
                    | Scalar::RepresentationView { .. }
            )
            && self.checked.primitive_type_reference(cast.target_type) == value.primitive_type()
        {
            if let Scalar::IntegerLiteral { literal } = value
                && self.retagged_literal(source, literal, depth + 1)
            {
                return true;
            }
            return self.scalar(cast.value, value, operands, depth + 1);
        }
        match value {
            Scalar::IntegerLiteral { literal } => {
                if let Some(length) = self
                    .type_owned_length(source)
                    .or_else(|| self.subslice_length(source))
                    .or_else(|| self.fixed_place_length(source))
                {
                    return value.primitive_type() == Some(PrimitiveType::U64)
                        && literal.value_u64() == Some(length);
                }
                if let ExpressionNode::Integer(authored) = node {
                    return authored.value_bignum() == literal.value_bignum()
                        && authored.landing().is_none_or(|landing| {
                            literal
                                .landing()
                                .is_some_and(|retained| landing.landed_type == retained.landed_type)
                        });
                }
                value
                    .primitive_type()
                    .and_then(|primitive| {
                        validation::land_anonymous_integer_expression(
                            &self.checked.typed,
                            source,
                            primitive,
                            |expression| self.anonymous_builtin(expression),
                        )
                    })
                    .is_some_and(|authored| authored == *literal)
            }
            Scalar::IeeeFloatLiteral { value } => match (node, value) {
                (
                    ExpressionNode::Float(literal),
                    semantic_vocabulary::IeeeFloatValue::Binary32(bits),
                ) => {
                    literal.landing() == Some(numerics::literals::FloatFormat::F32)
                        && literal.f32_bits() == *bits
                }
                (
                    ExpressionNode::Float(literal),
                    semantic_vocabulary::IeeeFloatValue::Binary64(bits),
                ) => {
                    literal.landing() == Some(numerics::literals::FloatFormat::F64)
                        && literal.value_f64().to_bits() == *bits
                }
                _ => false,
            },
            Scalar::IntegerBinary {
                kind, left, right, ..
            } => {
                let ExpressionNode::Binary(binary) = node else {
                    return false;
                };
                self.builtin(source)
                    && integer_operator(*kind) == binary.operator
                    && self.policy(source, *kind)
                    && self.scalar(binary.left, left, operands, depth + 1)
                    && self.scalar(binary.right, right, operands, depth + 1)
            }
            Scalar::IntegerBitwiseNot { operand, .. } => matches!(node,
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::BitwiseNot
                    && self.builtin(source) && self.scalar(unary.operand, operand, operands, depth + 1)),
            Scalar::IntegerWiden {
                operand,
                primitive_type,
            }
            | Scalar::IntegerExactCast {
                operand,
                primitive_type,
                ..
            }
            | Scalar::IntegerWrappingCast {
                operand,
                primitive_type,
            }
            | Scalar::IntegerSaturatingCast {
                operand,
                primitive_type,
            }
            | Scalar::IntegerTrappingCast {
                operand,
                primitive_type,
            } => {
                let ExpressionNode::Cast(cast) = node else {
                    return false;
                };
                // The checked plan retains no node for a cast whose operand
                // already carries the target primitive; that authored
                // occurrence is matched transparently against the scalar's
                // own operand rather than against a policy.
                if !cast.form.is_recast()
                    && cast.semantic_domain.is_empty()
                    && let Some(target) = self.checked.primitive_type_reference(cast.target_type)
                    && let Ok((machine, state)) =
                        crate::expression_preparation::source_custody::authored_state(
                            self.checked,
                            self.state,
                        )
                    && validation::expression_result_type_reference(
                        &self.checked.typed,
                        machine,
                        state,
                        cast.value,
                    )
                    .and_then(|reference| self.checked.primitive_type_reference(reference))
                        == Some(target)
                {
                    return self.scalar(cast.value, value, operands, depth + 1);
                }
                let policy = match value {
                    Scalar::IntegerWrappingCast { .. } => cast.domain == ArithmeticDomain::Wrapping,
                    Scalar::IntegerTrappingCast { .. } => cast.domain == ArithmeticDomain::Trapping,
                    Scalar::IntegerSaturatingCast { .. } => {
                        cast.domain == ArithmeticDomain::Saturating
                    }
                    // The range payload is not authority at this boundary:
                    // scalar_graph_lowering discards it and emits a fresh
                    // Terminal exact-cast obligation for the actual operand.
                    Scalar::IntegerExactCast { .. } => cast.domain == ArithmeticDomain::Exact,
                    Scalar::IntegerWiden { .. } => {
                        operand.primitive_type().is_some_and(|source_type| {
                            validation::integer_widen_is_total(source_type, *primitive_type)
                        })
                    }
                    _ => false,
                };
                policy
                    && !cast.form.is_recast()
                    && cast.semantic_domain.is_empty()
                    && self.checked.primitive_type_reference(cast.target_type)
                        == Some(*primitive_type)
                    && self.scalar(cast.value, operand, operands, depth + 1)
            }
            Scalar::Boolean(boolean) => self.boolean(source, boolean, operands, depth + 1),
            // A retained re-view rejoins one authored recast to the same view
            // carrier -- inline, or through the loan local bound to it -- and
            // its operand rejoins the viewed place itself. A different view
            // carrier, a non-recast cast, or an identity pair is not the
            // judgment stage 04 retained.
            Scalar::RepresentationView {
                primitive_type,
                operand,
            } => self
                .authored_recast(source)
                .is_some_and(|(target_type, viewed)| {
                    self.checked.primitive_type_reference(target_type) == Some(*primitive_type)
                        && operand.primitive_type() != Some(*primitive_type)
                        && self.scalar(viewed, operand, operands, depth + 1)
                }),
            // Exact read identities, paths and their namespaces are checked by
            // scalar_source_custody; these forms introduce no scalar operator.
            Scalar::Parameter { .. } | Scalar::Local { .. } | Scalar::StorageRead { .. } => {
                matches!(
                    node,
                    ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Borrow(_)
                )
            }
            Scalar::StructuralParameterByteLength {
                root:
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: parameter_position,
                    },
                path,
            } => self
                .byte_length(source, *parameter_position, path)
                .unwrap_or(false),
            // Local lengths observe the exact authored collection, never a
            // projected path. The established carrier separately determines
            // whether this names a borrowed view or a bounded owner's live
            // length; neither may substitute the owner's capacity.
            Scalar::StructuralParameterByteLength {
                root:
                    checked_trees::CheckedStorageRoot::ViewLocal { symbol }
                    | checked_trees::CheckedStorageRoot::OwnedLocal { symbol },
                path,
            } => path.is_empty() && self.view_local_length(source, *symbol).unwrap_or(false),
            Scalar::StructuralParameterField { .. } => matches!(
                node,
                ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
            ),
            // Authored indexing nests outermost-last: `self.a[i][k]` is
            // `Indexed(k)` over `Indexed(i)`, so the retained selectors, in
            // application order `i, k`, rejoin the authored chain from its
            // top node down. Every selector must match its own authored
            // position; none may be dropped, reordered or substituted.
            Scalar::StructuralParameterIndexedRead {
                root,
                path,
                index,
                inner_indexes,
                element_path,
                ..
            } => {
                let mut authored = source;
                if !element_path.is_empty() {
                    let Ok((machine, state)) = super::authored_state(self.checked, self.state)
                    else {
                        return false;
                    };
                    let Ok((complete_path, _)) = super::structural_places::walk_exact_place(
                        self.checked,
                        machine,
                        state,
                        source,
                        false,
                    ) else {
                        return false;
                    };
                    let Some(prefix_length) = complete_path.len().checked_sub(element_path.len())
                    else {
                        return false;
                    };
                    if !paths_match(element_path, &complete_path[prefix_length..]) {
                        return false;
                    }
                    for segment in element_path.iter().rev() {
                        authored = match (
                            segment,
                            self.checked.expression_table.expression(authored),
                        ) {
                            (
                                checked_trees::CheckedStructuralPredicatePathSegment::Field(_),
                                ExpressionNode::Member(member),
                            ) => member.receiver,
                            (
                                checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_)
                                | checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                                    _,
                                ),
                                ExpressionNode::Indexed(indexed),
                            ) => indexed.collection,
                            _ => return false,
                        };
                    }
                }
                for selector in inner_indexes
                    .iter()
                    .rev()
                    .chain(std::iter::once(index.as_ref()))
                {
                    let ExpressionNode::Indexed(indexed) =
                        self.checked.expression_table.expression(authored)
                    else {
                        return false;
                    };
                    if !self.scalar(indexed.index, selector, operands, depth + 1) {
                        return false;
                    }
                    authored = indexed.collection;
                }
                // The selection directly over the static place is the
                // outermost retained selector. Below it only the static path
                // remains -- fields and literal indexes (`self.a[0][i][k]`);
                // one more runtime indexing would be a selector the read
                // dropped.
                if inner_indexes.is_empty() && element_path.is_empty() {
                    return true;
                }
                let checked_trees::CheckedStorageRoot::Parameter { index: position } = root else {
                    return false;
                };
                let Ok((machine, state)) = super::authored_state(self.checked, self.state) else {
                    return false;
                };
                let Some(parameter) = self.checked.state_parameters(state).get(*position as usize)
                else {
                    return false;
                };
                let Ok(place) =
                    crate::emission::call_source_custody::projected_receivers::store_destination(
                        self.checked,
                        machine.symbol,
                        self.state,
                        Some(self.statement as usize),
                        authored,
                    )
                else {
                    return false;
                };
                // Selectors alone cannot identify the collection: a same-shaped
                // sibling parameter or field must not replace its source.
                !parameter.relevance.is_erased()
                    && parameter.symbol == place.root
                    && paths_match(path, &place.path)
            }
            // Erased formals have no authored runtime expression to match.
            Scalar::ErasedParameter { .. } => false,
        }
    }

    fn view_local_length(
        &self,
        source: ExpressionHandle,
        symbol: symbols::SymbolHandle,
    ) -> Option<bool> {
        let (machine, state) = super::authored_state(self.checked, self.state).ok()?;
        let receiver = validation::collection_length_receiver(
            &self.checked.typed,
            machine,
            Some(state),
            source,
        )?;
        Some(matches!(
            self.checked.expression_table.expression(receiver),
            ExpressionNode::Name(path)
                if symbol.is_valid()
                    && path.symbol == symbol
                    && path.head_symbol == symbol
                    && self.checked.expression_table.name_path_members(path.members).len() == 1
        ))
    }

    fn byte_length(
        &self,
        source: ExpressionHandle,
        parameter_position: u32,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    ) -> Option<bool> {
        let (machine, state) = super::authored_state(self.checked, self.state).ok()?;
        let receiver = validation::collection_length_receiver(
            &self.checked.typed,
            machine,
            Some(state),
            source,
        )?;
        let authored =
            crate::emission::call_source_custody::projected_receivers::store_destination(
                self.checked,
                machine.symbol,
                self.state,
                None,
                receiver,
            )
            .ok()?;
        let parameter = self
            .checked
            .state_parameters(state)
            .get(parameter_position as usize)?;
        Some(parameter.symbol == authored.root && paths_match(path, &authored.path))
    }

    fn boolean(
        &self,
        source: ExpressionHandle,
        value: &Boolean,
        operands: &[(ExpressionHandle, PrimitiveType)],
        depth: usize,
    ) -> bool {
        if self.exhausted(depth) {
            return false;
        }
        if let Some(selected) = self.projected_literal(source) {
            return self.boolean(selected, value, operands, depth + 1);
        }
        let node = self.checked.expression_table.expression(source);
        if let Boolean::Parameter { position } = value
            && !operands.is_empty()
        {
            return operands.get(*position) == Some(&(source, PrimitiveType::Bool));
        }
        match value {
            Boolean::Constant(value) => {
                matches!(node, ExpressionNode::Boolean(authored) if value == authored)
                    || validation::evaluate_anonymous_numeric_comparison(
                        &self.checked.typed,
                        source,
                        |expression| self.anonymous_builtin(expression),
                    ) == Some(*value)
            }
            Boolean::Not(operand) => match node {
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                    self.builtin(source)
                        && self.boolean(unary.operand, operand, operands, depth + 1)
                }
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::NotEqual => {
                    self.equality(
                        source,
                        binary.left,
                        binary.right,
                        operand,
                        operands,
                        depth + 1,
                    )
                }
                _ => false,
            },
            Boolean::IntegerMathComparison { kind, left, right } => {
                !matches!(node, ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::NotEqual)
                    && self.integer_math_comparison(source, *kind, left, right, operands, depth + 1)
            }
            Boolean::Equal { .. }
            | Boolean::IntegerComparison { .. }
            | Boolean::ScalarIeeeFloatComparison { .. } => {
                let ExpressionNode::Binary(binary) = node else {
                    return false;
                };
                if binary.operator == BinaryOperator::NotEqual {
                    return false;
                }
                self.equality(
                    source,
                    binary.left,
                    binary.right,
                    value,
                    operands,
                    depth + 1,
                )
            }
            Boolean::And { left, right } | Boolean::Or { left, right } => {
                if let ExpressionNode::Binary(binary) = node
                    && binary.operator == BinaryOperator::Equal
                    && matches!(value, Boolean::And { .. })
                    && (self.carrier_literal_equality(source, binary.left, binary.right, value)
                        || self.carrier_pair_equality(source, binary.left, binary.right, value))
                {
                    return true;
                }
                let expected = if matches!(value, Boolean::And { .. }) {
                    BinaryOperator::And
                } else {
                    BinaryOperator::Or
                };
                matches!(node, ExpressionNode::Binary(binary) if binary.operator == expected
                    && self.builtin(source) && self.boolean(binary.left, left, operands, depth + 1)
                    && self.boolean(binary.right, right, operands, depth + 1))
            }
            Boolean::Parameter { .. } | Boolean::Local { .. } | Boolean::StorageRead { .. } => {
                matches!(
                    node,
                    ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Borrow(_)
                )
            }
            Boolean::StructuralParameterField { .. } => matches!(
                node,
                ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
            ),
            // A retained tag observation replays its authored membership:
            // the exact case reference and subject projection must rejoin this
            // parameter's authored namespace position, path, and case identity.
            Boolean::StructuralCaseMembership { subject, case } => {
                let Ok((_, state)) = crate::expression_preparation::source_custody::authored_state(
                    self.checked,
                    self.state,
                ) else {
                    return false;
                };
                let Ok(Some((symbol, path, authored_case))) =
                    super::storage_reads::case_membership::authored(self.checked, state, source)
                else {
                    return false;
                };
                self.checked
                    .state_parameters(state)
                    .get(subject.parameter_position as usize)
                    .is_some_and(|parameter| parameter.symbol == symbol)
                    && subject.path == path
                    && *case == authored_case
            }
            // These belong to structural predicate plans, not the scalar
            // expression producer used by array operands.
            Boolean::IeeeFloatComparison { .. }
            | Boolean::ByteSequenceEqual { .. }
            | Boolean::ErasedParameter { .. }
            | Boolean::PayloadlessSumEqual { .. } => false,
        }
    }

    fn equality(
        &self,
        source: ExpressionHandle,
        mut left_source: ExpressionHandle,
        mut right_source: ExpressionHandle,
        value: &Boolean,
        operands: &[(ExpressionHandle, PrimitiveType)],
        depth: usize,
    ) -> bool {
        let ExpressionNode::Binary(binary) = self.checked.expression_table.expression(source)
        else {
            return false;
        };
        // Mathematical equality has its own selected builtin meaning; it is
        // not an executable finite-carrier comparison. In particular the
        // enclosing Not routes an authored != here with its polarity intact.
        if let Boolean::IntegerMathComparison { kind, left, right } = value {
            return self.integer_math_comparison(source, *kind, left, right, operands, depth + 1);
        }
        if !self.builtin(source) {
            return false;
        }
        match value {
            Boolean::Equal { left, right } => {
                matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) && self.boolean(left_source, left, operands, depth + 1)
                    && self.boolean(right_source, right, operands, depth + 1)
            }
            Boolean::IntegerComparison { kind, left, right } => {
                // An empty byte literal leaves only the live-length equality:
                // there are no indexed-byte conjuncts to reach the And arm.
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) && (self.carrier_literal_equality(source, left_source, right_source, value)
                    || self.carrier_pair_equality(source, left_source, right_source, value))
                {
                    return true;
                }
                let expected = match binary.operator {
                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                        CheckedIntegerComparisonKind::Equal
                    }
                    BinaryOperator::Less | BinaryOperator::Greater => {
                        CheckedIntegerComparisonKind::LessThan
                    }
                    BinaryOperator::LessOrEqual | BinaryOperator::GreaterOrEqual => {
                        CheckedIntegerComparisonKind::LessOrEqual
                    }
                    _ => return false,
                };
                if matches!(
                    binary.operator,
                    BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
                ) {
                    std::mem::swap(&mut left_source, &mut right_source);
                }
                *kind == expected
                    && self.scalar(left_source, left, operands, depth + 1)
                    && self.scalar(right_source, right, operands, depth + 1)
            }
            Boolean::ScalarIeeeFloatComparison { kind, left, right } => {
                let expected = match binary.operator {
                    BinaryOperator::Equal => CheckedIeeeFloatComparisonKind::Equal,
                    BinaryOperator::NotEqual => CheckedIeeeFloatComparisonKind::NotEqual,
                    _ => return false,
                };
                *kind == expected
                    && self.scalar(left_source, left, operands, depth + 1)
                    && self.scalar(right_source, right, operands, depth + 1)
            }
            Boolean::And { .. } if binary.operator == BinaryOperator::NotEqual => {
                self.carrier_literal_equality(source, left_source, right_source, value)
                    || self.carrier_pair_equality(source, left_source, right_source, value)
            }
            _ => false,
        }
    }

    /// `carrier == "literal"` on a byte field or shared view local has no scalar
    /// operand pair, so checking decomposes it into the live-length comparison
    /// folded with one indexed-read equality per literal byte
    /// (typed-trees-to-checked-trees `byte_carrier_literal_equality`). The
    /// authored expression retains no conjunct nodes, so correspondence
    /// replays that fold: the leftmost conjunct is the length equality and
    /// each subsequent right child is the byte equality at its literal index,
    /// in literal order. `!=` retains the same conjunction under `Not`, which
    /// reaches here through the equality route above.
    fn carrier_literal_equality(
        &self,
        source: ExpressionHandle,
        left_source: ExpressionHandle,
        right_source: ExpressionHandle,
        value: &Boolean,
    ) -> bool {
        if !self.builtin(source) {
            return false;
        }
        let mut conjuncts = Vec::new();
        let mut spine = value;
        while let Boolean::And { left, right } = spine {
            conjuncts.push(right.as_ref());
            spine = left;
        }
        conjuncts.push(spine);
        conjuncts.reverse();
        for (carrier, literal) in [(left_source, right_source), (right_source, left_source)] {
            let ExpressionNode::String(bytes) = self.checked.expression_table.expression(literal)
            else {
                continue;
            };
            let Some((root, path)) = self.carrier_literal_place(carrier) else {
                continue;
            };
            if conjuncts.len() != bytes.len() + 1 {
                return false;
            }
            let Boolean::IntegerComparison { kind, left, right } = conjuncts[0] else {
                return false;
            };
            if *kind != CheckedIntegerComparisonKind::Equal
                || !matches!(
                    left.as_ref(),
                    Scalar::StructuralParameterByteLength {
                        root: retained_root,
                        path: retained,
                    } if *retained_root == root && paths_match(retained, &path)
                )
                || !matches!(
                    right.as_ref(),
                    Scalar::IntegerLiteral { literal }
                        if literal.value_u64() == Some(bytes.len() as u64)
                            && literal.landing().is_some_and(|landing| {
                                landing.landed_type
                                    == numerics::literals::LandedIntegerType::U64
                            })
                )
            {
                return false;
            }
            return conjuncts[1..]
                .iter()
                .copied()
                .zip(bytes.iter())
                .enumerate()
                .all(|(index, (conjunct, byte))| {
                    let Boolean::IntegerComparison { kind, left, right } = conjunct else {
                        return false;
                    };
                    *kind == CheckedIntegerComparisonKind::Equal
                        && matches!(
                            left.as_ref(),
                            Scalar::StructuralParameterIndexedRead {
                                root: retained_root,
                                path: retained,
                                index: retained_index,
                                inner_indexes,
                                element_path,
                                primitive_type,
                            } if *retained_root == root
                                && inner_indexes.is_empty()
                                && element_path.is_empty()
                                && paths_match(retained, &path)
                                && *primitive_type == PrimitiveType::U8
                                && matches!(
                                    retained_index.as_ref(),
                                    Scalar::IntegerLiteral { literal }
                                        if literal.value_u64() == Some(index as u64)
                                            && literal.landing().is_some_and(|landing| {
                                                landing.landed_type
                                                    == numerics::literals::LandedIntegerType::U64
                                            })
                                )
                        )
                        && matches!(
                            right.as_ref(),
                            Scalar::IntegerLiteral { literal }
                                if literal.value_u64() == Some(u64::from(*byte))
                                    && literal.landing().is_some_and(|landing| {
                                        landing.landed_type
                                            == numerics::literals::LandedIntegerType::U8
                                    })
                        )
                });
        }
        false
    }

    /// `left == right` over two bounded owned byte fields has no scalar
    /// operand pair either: checking decomposes it into the live-length
    /// equality, then a chain with one guarded position per byte below the
    /// smaller declared capacity, each later position nested in the previous
    /// one's agreeing branch (typed-trees-to-checked-trees
    /// `byte_carrier_pair_equality`). Correspondence replays that chain
    /// against each operand's declared capacity, so a decomposition that
    /// stops short of a position equal lengths can reach cannot pass.
    fn carrier_pair_equality(
        &self,
        source: ExpressionHandle,
        left_source: ExpressionHandle,
        right_source: ExpressionHandle,
        value: &Boolean,
    ) -> bool {
        if !self.builtin(source) {
            return false;
        }
        let Ok((machine, state)) = super::authored_state(self.checked, self.state) else {
            return false;
        };
        let capacity = |operand| {
            validation::declared_place_type_raw(&self.checked.typed, machine, Some(state), operand)
                .and_then(|type_reference| {
                    validation::bounded_byte_buffer_capacity(&self.checked.typed, type_reference)
                })
                .and_then(|capacity| u64::try_from(capacity).ok())
        };
        let (Some(left_capacity), Some(right_capacity)) =
            (capacity(left_source), capacity(right_source))
        else {
            return false;
        };
        let (Some(left_place), Some(right_place)) = (
            self.carrier_literal_place(left_source),
            self.carrier_literal_place(right_source),
        ) else {
            return false;
        };
        let length =
            |scalar: &Scalar, (root, path): &(checked_trees::CheckedStorageRoot, Vec<_>)| {
                matches!(
                    scalar,
                    Scalar::StructuralParameterByteLength {
                        root: retained_root,
                        path: retained,
                    } if retained_root == root && paths_match(retained, path)
                )
            };
        let u64_literal = |scalar: &Scalar, value: u64| {
            matches!(
                scalar,
                Scalar::IntegerLiteral { literal }
                    if literal.value_u64() == Some(value)
                        && literal.landing().is_some_and(|landing| {
                            landing.landed_type == numerics::literals::LandedIntegerType::U64
                        })
            )
        };
        let read = |scalar: &Scalar,
                    (root, path): &(checked_trees::CheckedStorageRoot, Vec<_>),
                    index: u64| {
            matches!(
                scalar,
                Scalar::StructuralParameterIndexedRead {
                    root: retained_root,
                    path: retained,
                    index: retained_index,
                    inner_indexes,
                    element_path,
                    primitive_type,
                } if retained_root == root
                    && inner_indexes.is_empty()
                    && element_path.is_empty()
                    && paths_match(retained, path)
                    && *primitive_type == PrimitiveType::U8
                    && u64_literal(retained_index, index)
            )
        };
        let lengths_equal = |boolean: &Boolean| {
            matches!(
                boolean,
                Boolean::IntegerComparison {
                    kind: CheckedIntegerComparisonKind::Equal,
                    left,
                    right,
                } if length(left, &left_place) && length(right, &right_place)
            )
        };
        let positions = left_capacity.min(right_capacity);
        let mut remaining = match value {
            Boolean::And { left, right } if positions > 0 && lengths_equal(left) => right.as_ref(),
            lengths if positions == 0 => return lengths_equal(lengths),
            _ => return false,
        };
        for index in 0..positions {
            let Boolean::Or {
                left: ended,
                right: continued,
            } = remaining
            else {
                return false;
            };
            let ended_here = matches!(
                ended.as_ref(),
                Boolean::IntegerComparison {
                    kind: CheckedIntegerComparisonKind::LessOrEqual,
                    left,
                    right,
                } if length(left, &left_place) && u64_literal(right, index)
            );
            let (agreeing, next) = match continued.as_ref() {
                Boolean::And { left, right } if index + 1 < positions => {
                    (left.as_ref(), Some(right.as_ref()))
                }
                agreeing if index + 1 == positions => (agreeing, None),
                _ => return false,
            };
            let agrees = matches!(
                agreeing,
                Boolean::And { left: bound, right: bytes }
                    if matches!(
                        bound.as_ref(),
                        Boolean::IntegerComparison {
                            kind: CheckedIntegerComparisonKind::LessThan,
                            left,
                            right,
                        } if u64_literal(left, index) && length(right, &right_place)
                    ) && matches!(
                        bytes.as_ref(),
                        Boolean::IntegerComparison {
                            kind: CheckedIntegerComparisonKind::Equal,
                            left,
                            right,
                        } if read(left, &left_place, index) && read(right, &right_place, index)
                    )
            );
            if !ended_here || !agrees {
                return false;
            }
            match next {
                Some(next) => remaining = next,
                None => return true,
            }
        }
        false
    }

    /// Local equality names the earlier authored binding, not any other place
    /// with equal bytes or the same type. Its establishment independently
    /// rejoins the initializer and loan; this replay checks every observation
    /// in the comparison reads that exact byte carrier.
    fn carrier_literal_place(
        &self,
        source: ExpressionHandle,
    ) -> Option<(
        checked_trees::CheckedStorageRoot,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    )> {
        if let Some((position, path)) = self.carrier_parameter_place(source) {
            return Some((
                checked_trees::CheckedStorageRoot::Parameter { index: position },
                path,
            ));
        }
        let ExpressionNode::Name(name) = self.checked.expression_table.expression(source) else {
            return None;
        };
        if !name.symbol.is_valid()
            || name.symbol != name.head_symbol
            || self
                .checked
                .expression_table
                .name_path_members(name.members)
                .len()
                != 1
        {
            return None;
        }
        let (_, state) = super::authored_state(self.checked, self.state).ok()?;
        if self
            .checked
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == name.symbol)
        {
            return None;
        }
        let mut locals = self
            .checked
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .take(usize::try_from(self.statement).ok()?)
            .filter_map(|statement| match statement {
                checked_trees::statement::StatementNode::LocalData(local)
                    if local.symbol == name.symbol =>
                {
                    Some(local)
                }
                _ => None,
            });
        let local = locals.next()?;
        if locals.next().is_some() || !local.initial_value.is_valid() {
            return None;
        }
        if validation::bounded_byte_buffer_capacity(&self.checked.typed, local.type_reference)
            .is_some()
        {
            return Some((
                checked_trees::CheckedStorageRoot::OwnedLocal {
                    symbol: name.symbol,
                },
                Vec::new(),
            ));
        }
        if local.is_mutable {
            return None;
        }
        let view = validation::slice_view_shell(&self.checked.typed, local.type_reference, &[])?;
        if view.access != language_semantics::ReferenceAccess::Shared
            || self.checked.primitive_type_reference(view.element) != Some(PrimitiveType::U8)
        {
            return None;
        }
        Some((
            checked_trees::CheckedStorageRoot::ViewLocal {
                symbol: name.symbol,
            },
            Vec::new(),
        ))
    }

    fn carrier_parameter_place(
        &self,
        source: ExpressionHandle,
    ) -> Option<(u32, Vec<checked_trees::CheckedUnitStructuralPathSegment>)> {
        let (machine, state) = super::authored_state(self.checked, self.state).ok()?;
        let authored =
            crate::emission::call_source_custody::projected_receivers::store_destination(
                self.checked,
                machine.symbol,
                self.state,
                None,
                source,
            )
            .ok()?;
        let position = self
            .checked
            .state_parameters(state)
            .iter()
            .position(|parameter| parameter.symbol == authored.root)?;
        Some((position as u32, authored.path))
    }

    fn domain(&self, source: ExpressionHandle, depth: usize) -> Option<ArithmeticDomain> {
        if self.exhausted(depth) {
            return None;
        }
        if let Some(selected) = self.projected_literal(source) {
            return self.domain(selected, depth + 1);
        }
        if self.subslice_length(source).is_some() {
            return Some(ArithmeticDomain::Exact);
        }
        match self.checked.expression_table.expression(source) {
            ExpressionNode::Integer(literal) => Some(
                literal
                    .landing()
                    .map_or(ArithmeticDomain::Exact, |landing| landing.domain),
            ),
            ExpressionNode::Cast(cast) => Some(cast.domain),
            ExpressionNode::Unary(unary) => self.domain(unary.operand, depth + 1),
            ExpressionNode::Binary(binary) => {
                let left = self.domain(binary.left, depth + 1)?;
                if matches!(
                    binary.operator,
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                ) {
                    return Some(left);
                }
                let right = self.domain(binary.right, depth + 1)?;
                match (left, right) {
                    (ArithmeticDomain::Exact, domain) | (domain, ArithmeticDomain::Exact) => {
                        Some(domain)
                    }
                    (left, right) if left == right => Some(left),
                    _ => None,
                }
            }
            _ => {
                let (machine, state) =
                    crate::expression_preparation::source_custody::authored_state(
                        self.checked,
                        self.state,
                    )
                    .ok()?;
                // This shared query includes resolved call results and retains
                // their qualification shells; the caller's destination is not
                // an arithmetic-domain witness for a completed call.
                validation::declared_place_type_raw(
                    &self.checked.typed,
                    machine,
                    Some(state),
                    source,
                )
                .map(|reference| self.checked.arithmetic_domain_for_type_reference(reference))
                .or_else(|| {
                    validation::collection_length_receiver(
                        &self.checked.typed,
                        machine,
                        Some(state),
                        source,
                    )
                    .map(|_| ArithmeticDomain::Exact)
                })
            }
        }
    }

    fn policy(&self, source: ExpressionHandle, kind: IntegerBinary) -> bool {
        let expected = match kind {
            IntegerBinary::BitwiseAnd | IntegerBinary::BitwiseOr | IntegerBinary::BitwiseXor => {
                return true;
            }
            IntegerBinary::WrappingAdd
            | IntegerBinary::WrappingSubtract
            | IntegerBinary::WrappingMultiply
            | IntegerBinary::WrappingDivide
            | IntegerBinary::WrappingRemainder
            | IntegerBinary::WrappingShiftLeft
            | IntegerBinary::WrappingShiftRight => ArithmeticDomain::Wrapping,
            IntegerBinary::SaturatingAdd
            | IntegerBinary::SaturatingSubtract
            | IntegerBinary::SaturatingMultiply
            | IntegerBinary::SaturatingDivide
            | IntegerBinary::SaturatingShiftLeft
            | IntegerBinary::SaturatingShiftRight
            | IntegerBinary::SaturatingRemainder => ArithmeticDomain::Saturating,
            IntegerBinary::TrappingShiftLeft
            | IntegerBinary::TrappingShiftRight
            | IntegerBinary::TrappingAdd
            | IntegerBinary::TrappingSubtract
            | IntegerBinary::TrappingMultiply
            | IntegerBinary::TrappingDivide
            | IntegerBinary::TrappingRemainder => ArithmeticDomain::Trapping,
            _ => ArithmeticDomain::Exact,
        };
        self.domain(source, 0) == Some(expected)
    }

    fn exhausted(&self, depth: usize) -> bool {
        // Normalized Boolean matching can visit one source node through scalar,
        // Boolean, and comparison layers. Four visits per authored node cover
        // that representation normalization without a source-depth ceiling.
        depth
            > self
                .checked
                .expression_table
                .expression_count()
                .saturating_mul(4)
    }

    /// The authored recast a re-view read rejoins: the stated view type and
    /// the viewed place, with the authored `&`/`&mut` spellings peeled from
    /// both sides of the cast. A bare name resolves through the immutable
    /// loan local of this state that binds it; any other spelling is not a
    /// recast.
    fn authored_recast(
        &self,
        source: ExpressionHandle,
    ) -> Option<(checked_trees::types::TypeReferenceHandle, ExpressionHandle)> {
        let table = &self.checked.expression_table;
        let mut expression = source;
        if let ExpressionNode::Name(name) = table.expression(expression) {
            if !name.symbol.is_valid()
                || name.symbol != name.head_symbol
                || table.name_path_members(name.members).len() != 1
            {
                return None;
            }
            let (_, state) = crate::expression_preparation::source_custody::authored_state(
                self.checked,
                self.state,
            )
            .ok()?;
            let mut locals = self
                .checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(usize::try_from(self.statement).ok()?)
                .filter_map(|statement| match statement {
                    checked_trees::statement::StatementNode::LocalData(local)
                        if local.symbol == name.symbol =>
                    {
                        Some(local)
                    }
                    _ => None,
                });
            let local = locals.next()?;
            if locals.next().is_some() || local.is_mutable || !local.initial_value.is_valid() {
                return None;
            }
            expression = local.initial_value;
        }
        while let ExpressionNode::Borrow(borrow) = table.expression(expression) {
            expression = borrow.target;
        }
        let ExpressionNode::Cast(cast) = table.expression(expression) else {
            return None;
        };
        if !cast.form.is_recast() {
            return None;
        }
        let mut viewed = cast.value;
        while let ExpressionNode::Borrow(borrow) = table.expression(viewed) {
            viewed = borrow.target;
        }
        Some((cast.target_type, viewed))
    }

    fn retagged_literal(
        &self,
        source: ExpressionHandle,
        literal: &numerics::literals::IntegerLiteral,
        depth: usize,
    ) -> bool {
        if self.exhausted(depth) {
            return false;
        }
        let ExpressionNode::Cast(cast) = self.checked.expression_table.expression(source) else {
            return false;
        };
        if cast.form.is_recast() || !cast.semantic_domain.is_empty() {
            return false;
        }
        let Some(primitive) = self.checked.primitive_type_reference(cast.target_type) else {
            return false;
        };
        if primitive == PrimitiveType::Addr {
            return false;
        }
        let source_primitive = match self.checked.expression_table.expression(cast.value) {
            ExpressionNode::Integer(authored) => Scalar::IntegerLiteral {
                literal: authored.clone(),
            }
            .primitive_type(),
            ExpressionNode::Cast(inner) => self.checked.primitive_type_reference(inner.target_type),
            _ => None,
        };
        let anonymous = || {
            validation::land_anonymous_integer_expression(
                &self.checked.typed,
                cast.value,
                primitive,
                |expression| self.anonymous_builtin(expression),
            )
        };
        if cast.domain != ArithmeticDomain::Exact
            && source_primitive != Some(primitive)
            && anonymous().is_none()
        {
            return false;
        }
        let Ok(semantic_vocabulary::ScalarType::Integer(target)) =
            crate::emission::scalar_types::terminal_scalar_type(primitive)
        else {
            return false;
        };
        let value = match target.sign() {
            semantic_vocabulary::IntegerSign::Signed => literal
                .value_i64()
                .map(|value| semantic_vocabulary::IntegerValue::Signed(i128::from(value))),
            semantic_vocabulary::IntegerSign::Unsigned => literal
                .value_u64()
                .map(|value| semantic_vocabulary::IntegerValue::Unsigned(u128::from(value))),
        };
        if !value.is_some_and(|value| target.admits(value)) {
            return false;
        }
        match self.checked.expression_table.expression(cast.value) {
            ExpressionNode::Integer(authored) => authored.value_bignum() == literal.value_bignum(),
            ExpressionNode::Cast(_) => self.retagged_literal(cast.value, literal, depth + 1),
            _ => anonymous()
                .is_some_and(|authored| authored.value_bignum() == literal.value_bignum()),
        }
    }

    fn projected_literal(&self, source: ExpressionHandle) -> Option<ExpressionHandle> {
        if let Some((selected, _)) =
            validation::closed_record_scalar_projection(&self.checked.typed, source)
        {
            return Some(selected);
        }
        if !matches!(
            self.checked.expression_table.expression(source),
            ExpressionNode::Indexed(_)
        ) {
            return None;
        }
        let (machine, _) =
            crate::expression_preparation::source_custody::authored_state(self.checked, self.state)
                .ok()?;
        validation::builtin_constant_array_projection_type(
            &self.checked.typed,
            machine.symbol,
            source,
        )?;
        let mut selected = source;
        let mut indices = Vec::new();
        while let ExpressionNode::Indexed(indexed) =
            self.checked.expression_table.expression(selected)
        {
            if indices.len() >= self.checked.expression_table.expression_count() {
                return None;
            }
            if self
                .checked
                .facts
                .operators
                .expression_use(selected)
                .is_some_and(|operator| {
                    operator.spelling != language_core::OperatorSpelling::Index
                        || operator.selected_operator_symbol.is_valid()
                        || operator.candidate_count != 0
                        || !matches!(
                            operator.status,
                            checked_trees::CheckedOperatorResolutionStatus::Missing
                                | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                        )
                })
            {
                return None;
            }
            let ExpressionNode::Integer(index) =
                self.checked.expression_table.expression(indexed.index)
            else {
                return None;
            };
            indices.push(usize::try_from(index.value_u64()?).ok()?);
            selected = indexed.collection;
        }
        let reference = validation::declared_constant_array_type(&self.checked.typed, selected)?;
        validation::closed_literal_array_elements(&self.checked.typed, selected, reference)?;
        for index in indices.into_iter().rev() {
            let ExpressionNode::ArrayLiteral(elements) =
                self.checked.expression_table.expression(selected)
            else {
                return None;
            };
            selected = *self
                .checked
                .expression_table
                .expression_handles(*elements)
                .get(index)?;
        }
        matches!(
            self.checked.expression_table.expression(selected),
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_)
        )
        .then_some(selected)
    }

    fn subslice_length(&self, source: ExpressionHandle) -> Option<u64> {
        let ExpressionNode::Member(member) = self.checked.expression_table.expression(source)
        else {
            return None;
        };
        if CollectionMeasure::from_authored_spelling(member.member.as_str())
            != Some(CollectionMeasure::Length)
            || member.member_symbol.is_valid()
            || member.case_variant.is_some()
        {
            return None;
        }
        let ExpressionNode::Indexed(indexed) =
            self.checked.expression_table.expression(member.receiver)
        else {
            return None;
        };
        let ExpressionNode::Range(range) = self.checked.expression_table.expression(indexed.index)
        else {
            return None;
        };
        if range.end_inclusive || !range.start.is_valid() || !range.end.is_valid() {
            return None;
        }
        let ExpressionNode::Integer(start) = self.checked.expression_table.expression(range.start)
        else {
            return None;
        };
        let ExpressionNode::Integer(end) = self.checked.expression_table.expression(range.end)
        else {
            return None;
        };
        let length = end.value_u64()?.checked_sub(start.value_u64()?)?;
        i64::try_from(length).ok()?;
        Some(length)
    }
}

fn paths_match(
    retained: &[checked_trees::CheckedStructuralPredicatePathSegment],
    authored: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> bool {
    retained.len() == authored.len()
        && retained
            .iter()
            .zip(authored)
            .all(|(retained, authored)| match (retained, authored) {
                (
                    checked_trees::CheckedStructuralPredicatePathSegment::Field(left),
                    checked_trees::CheckedUnitStructuralPathSegment::Field(right),
                ) => left == right,
                (
                    checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(left),
                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(right),
                ) => left == right,
                (
                    checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(left),
                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(right),
                ) => left == right,
                _ => false,
            })
}

fn integer_operator(kind: IntegerBinary) -> BinaryOperator {
    match kind {
        IntegerBinary::ExactAdd
        | IntegerBinary::WrappingAdd
        | IntegerBinary::SaturatingAdd
        | IntegerBinary::TrappingAdd => BinaryOperator::Add,
        IntegerBinary::ExactSubtract
        | IntegerBinary::WrappingSubtract
        | IntegerBinary::SaturatingSubtract
        | IntegerBinary::TrappingSubtract => BinaryOperator::Subtract,
        IntegerBinary::ExactMultiply
        | IntegerBinary::WrappingMultiply
        | IntegerBinary::SaturatingMultiply
        | IntegerBinary::TrappingMultiply => BinaryOperator::Multiply,
        IntegerBinary::ExactDivide
        | IntegerBinary::WrappingDivide
        | IntegerBinary::SaturatingDivide
        | IntegerBinary::TrappingDivide => BinaryOperator::Divide,
        IntegerBinary::ExactRemainder
        | IntegerBinary::WrappingRemainder
        | IntegerBinary::SaturatingRemainder
        | IntegerBinary::TrappingRemainder => BinaryOperator::Modulo,
        IntegerBinary::BitwiseAnd => BinaryOperator::BitwiseAnd,
        IntegerBinary::BitwiseOr => BinaryOperator::BitwiseOr,
        IntegerBinary::BitwiseXor => BinaryOperator::BitwiseXor,
        IntegerBinary::ExactShiftLeft
        | IntegerBinary::SaturatingShiftLeft
        | IntegerBinary::WrappingShiftLeft
        | IntegerBinary::TrappingShiftLeft => BinaryOperator::ShiftLeft,
        IntegerBinary::ExactShiftRight
        | IntegerBinary::SaturatingShiftRight
        | IntegerBinary::WrappingShiftRight
        | IntegerBinary::TrappingShiftRight => BinaryOperator::ShiftRight,
    }
}
