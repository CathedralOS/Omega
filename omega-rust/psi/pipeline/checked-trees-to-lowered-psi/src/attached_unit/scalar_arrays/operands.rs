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

use crate::LoweringError;
use checked_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedCallScalarArgument,
    CheckedIntegerBinaryKind as IntegerBinary, CheckedIntegerComparisonKind,
    CheckedScalarComputationHandle, CheckedScalarComputationKind as Computation,
    CheckedScalarExpression as Scalar, CheckedTrees,
};
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;

mod extents;

pub(super) fn validate(
    checked: &CheckedTrees,
    state: SymbolHandle,
    _statement: u32,
    source_expression: ExpressionHandle,
    primitive: PrimitiveType,
    element: &CheckedCallScalarArgument,
) -> Result<(), LoweringError> {
    let context = Context { checked, state };
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
            "array operand value differs from its authored expression",
        ))
    }
}

struct Context<'a> {
    checked: &'a CheckedTrees,
    state: SymbolHandle,
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
            crate::scalar_source_custody::authored_state(self.checked, self.state)
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
            Computation::Call { arguments, .. } => plans
                .operands
                .span_or_empty(*arguments)
                .iter()
                .all(|argument| self.computation(*argument, active)),
            Computation::Apply {
                source_expression,
                expression,
                operands,
            } => {
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
                    | Scalar::IntegerTrappingCast { .. }
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
                if let Some(length) = self.subslice_length(source) {
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
                ) => literal.f32_bits() == *bits,
                (
                    ExpressionNode::Float(literal),
                    semantic_vocabulary::IeeeFloatValue::Binary64(bits),
                ) => literal.value_f64().to_bits() == *bits,
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
            | Scalar::IntegerTrappingCast {
                operand,
                primitive_type,
            } => {
                let ExpressionNode::Cast(cast) = node else {
                    return false;
                };
                let policy = match value {
                    Scalar::IntegerWrappingCast { .. } => cast.domain == ArithmeticDomain::Wrapping,
                    Scalar::IntegerTrappingCast { .. } => cast.domain == ArithmeticDomain::Trapping,
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
            // Exact read identities, paths and their namespaces are checked by
            // scalar_source_custody; these forms introduce no scalar operator.
            Scalar::Parameter { .. }
            | Scalar::Local { .. }
            | Scalar::StorageRead { .. }
            | Scalar::StructuralParameterField { .. }
            | Scalar::StructuralParameterByteLength { .. } => matches!(
                node,
                ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Borrow(_)
            ),
            Scalar::StructuralParameterIndexedRead { index, .. } => matches!(node,
                ExpressionNode::Indexed(indexed) if self.scalar(indexed.index, index, operands, depth + 1)),
        }
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
            Boolean::Equal { .. } | Boolean::IntegerComparison { .. } => {
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
                let expected = if matches!(value, Boolean::And { .. }) {
                    BinaryOperator::And
                } else {
                    BinaryOperator::Or
                };
                matches!(node, ExpressionNode::Binary(binary) if binary.operator == expected
                    && self.builtin(source) && self.boolean(binary.left, left, operands, depth + 1)
                    && self.boolean(binary.right, right, operands, depth + 1))
            }
            Boolean::Parameter { .. }
            | Boolean::Local { .. }
            | Boolean::StorageRead { .. }
            | Boolean::StructuralParameterField { .. } => matches!(
                node,
                ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Borrow(_)
            ),
            // These belong to structural predicate plans, not the scalar
            // expression producer used by array operands.
            Boolean::IeeeFloatComparison { .. }
            | Boolean::ByteSequenceEqual { .. }
            | Boolean::PayloadlessSumEqual { .. }
            | Boolean::StructuralCaseMembership { .. } => false,
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
            _ => false,
        }
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
                    crate::scalar_source_custody::authored_state(self.checked, self.state).ok()?;
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
            | IntegerBinary::SaturatingRemainder => ArithmeticDomain::Saturating,
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
            crate::scalar_graph_lowering::terminal_scalar_type(primitive)
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
        if !matches!(
            self.checked.expression_table.expression(source),
            ExpressionNode::Indexed(_)
        ) {
            return None;
        }
        let (machine, _) =
            crate::scalar_source_custody::authored_state(self.checked, self.state).ok()?;
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
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
        )
        .then_some(selected)
    }

    fn subslice_length(&self, source: ExpressionHandle) -> Option<u64> {
        let ExpressionNode::Member(member) = self.checked.expression_table.expression(source)
        else {
            return None;
        };
        if member.member.as_str() != "len"
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

fn integer_operator(kind: IntegerBinary) -> BinaryOperator {
    match kind {
        IntegerBinary::ExactAdd | IntegerBinary::WrappingAdd | IntegerBinary::SaturatingAdd => {
            BinaryOperator::Add
        }
        IntegerBinary::ExactSubtract
        | IntegerBinary::WrappingSubtract
        | IntegerBinary::SaturatingSubtract => BinaryOperator::Subtract,
        IntegerBinary::ExactMultiply
        | IntegerBinary::WrappingMultiply
        | IntegerBinary::SaturatingMultiply => BinaryOperator::Multiply,
        IntegerBinary::ExactDivide
        | IntegerBinary::WrappingDivide
        | IntegerBinary::SaturatingDivide => BinaryOperator::Divide,
        IntegerBinary::ExactRemainder
        | IntegerBinary::WrappingRemainder
        | IntegerBinary::SaturatingRemainder => BinaryOperator::Modulo,
        IntegerBinary::BitwiseAnd => BinaryOperator::BitwiseAnd,
        IntegerBinary::BitwiseOr => BinaryOperator::BitwiseOr,
        IntegerBinary::BitwiseXor => BinaryOperator::BitwiseXor,
        IntegerBinary::ExactShiftLeft | IntegerBinary::WrappingShiftLeft => {
            BinaryOperator::ShiftLeft
        }
        IntegerBinary::ExactShiftRight | IntegerBinary::WrappingShiftRight => {
            BinaryOperator::ShiftRight
        }
    }
}
