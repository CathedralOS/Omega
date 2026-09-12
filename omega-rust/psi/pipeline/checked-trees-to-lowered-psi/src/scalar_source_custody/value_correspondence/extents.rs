//! Static source evaluation and bounds custody for eliminated subslice extents.

use super::*;

#[cfg(test)]
mod tests;

impl Context<'_> {
    pub(super) fn validate_extent_sources(
        &self,
        source: ExpressionHandle,
        element: &CheckedCallScalarArgument,
    ) -> Result<(), LoweringError> {
        let mut sources = Vec::new();
        match element {
            CheckedCallScalarArgument::Pure(_) => sources.push(source),
            CheckedCallScalarArgument::Computation(root) => {
                let plans = &self.checked.facts.values.scalar_computations;
                let mut pending = vec![*root];
                let mut visited = Vec::new();
                while let Some(handle) = pending.pop() {
                    if !plans.nodes.is_valid(handle) || visited.contains(&handle) {
                        continue;
                    }
                    visited.push(handle);
                    let node = plans.nodes.get(handle);
                    match &node.kind {
                        Computation::CaseMembership { subject, .. } => {
                            pending.extend(
                                crate::scalar_computations::cases::operand_fields(
                                    self.checked,
                                    subject,
                                )?
                                .iter()
                                .map(|field| field.value),
                            );
                        }
                        Computation::Dispatch { subject, arms, .. } => {
                            pending.push(*subject);
                            for arm in plans.dispatch_arms.span(*arms).ok_or(
                                LoweringError::Unsupported("array operand dispatch has stale arms"),
                            )? {
                                if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) =
                                    arm.pattern
                                {
                                    pending.push(pattern);
                                }
                                pending.push(arm.value);
                            }
                        }
                        Computation::Value(_) if node.value_source.is_valid() => {
                            sources.push(node.value_source)
                        }
                        Computation::Call {
                            arguments: operands,
                            ..
                        }
                        | Computation::Apply { operands, .. } => {
                            pending.extend_from_slice(plans.operands.span_or_empty(*operands))
                        }
                        Computation::Select {
                            condition,
                            when_true,
                            when_false,
                            ..
                        } => pending.extend([*condition, *when_true, *when_false]),
                        _ => {}
                    }
                }
            }
        }
        let mut visited = Vec::new();
        while let Some(source) = sources.pop() {
            if !source.is_valid() || visited.contains(&source) {
                continue;
            }
            visited.push(source);
            match self.checked.expression_table.expression(source) {
                ExpressionNode::Member(member) => {
                    if self.subslice_length(source).is_some()
                        && !self.static_subslice(member.receiver, 0)
                    {
                        return Err(LoweringError::Unsupported(
                            "eliminated subslice extent requires retained source evaluation and view bounds custody",
                        ));
                    }
                    sources.push(member.receiver);
                }
                ExpressionNode::Binary(binary) => sources.extend([binary.left, binary.right]),
                ExpressionNode::Unary(unary) => sources.push(unary.operand),
                ExpressionNode::Cast(cast) => sources.push(cast.value),
                ExpressionNode::Borrow(borrow) => sources.push(borrow.target),
                ExpressionNode::Indexed(indexed) => {
                    sources.extend([indexed.collection, indexed.index])
                }
                ExpressionNode::Range(range) => sources.extend([range.start, range.end]),
                _ => {}
            }
        }
        Ok(())
    }

    fn indexed_builtin(
        &self,
        source: ExpressionHandle,
        spelling: language_core::OperatorSpelling,
    ) -> bool {
        self.checked
            .facts
            .operators
            .uses
            .iter()
            .all(|(_, selected)| {
                selected.expression != source
                    || (selected.spelling == spelling
                        && !selected.selected_operator_symbol.is_valid()
                        && selected.candidate_count == 0
                        && self
                            .checked
                            .facts
                            .operators
                            .candidates
                            .span_or_empty(selected.candidates)
                            .is_empty()
                        && matches!(
                            selected.status,
                            checked_trees::CheckedOperatorResolutionStatus::Missing
                                | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                        ))
            })
    }

    fn static_subslice(&self, source: ExpressionHandle, depth: usize) -> bool {
        if self.exhausted(depth) {
            return false;
        }
        let ExpressionNode::Indexed(indexed) = self.checked.expression_table.expression(source)
        else {
            return false;
        };
        let ExpressionNode::Range(range) = self.checked.expression_table.expression(indexed.index)
        else {
            return false;
        };
        let (ExpressionNode::Integer(start), ExpressionNode::Integer(end)) = (
            self.checked.expression_table.expression(range.start),
            self.checked.expression_table.expression(range.end),
        ) else {
            return false;
        };
        let (Some(start), Some(end), Some(length)) = (
            start.value_u64(),
            end.value_u64(),
            self.static_extent(indexed.collection, depth + 1),
        ) else {
            return false;
        };
        let Ok((machine, state)) =
            crate::scalar_source_custody::authored_state(self.checked, self.state)
        else {
            return false;
        };
        !range.end_inclusive
            && start <= end
            && end <= length
            && self.indexed_builtin(source, language_core::OperatorSpelling::Range)
            && validation::has_builtin_subslice_meaning(
                &self.checked.typed,
                machine,
                Some(state),
                source,
            )
    }

    fn static_extent(&self, source: ExpressionHandle, depth: usize) -> Option<u64> {
        if self.exhausted(depth) {
            return None;
        }
        let (machine, state) =
            crate::scalar_source_custody::authored_state(self.checked, self.state).ok()?;
        match self.checked.expression_table.expression(source) {
            ExpressionNode::String(bytes) => return u64::try_from(bytes.len()).ok(),
            ExpressionNode::Borrow(borrow) => return self.static_extent(borrow.target, depth + 1),
            ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                if !validation::place_has_builtin_coordinates(
                    &self.checked.typed,
                    machine,
                    Some(state),
                    source,
                ) || !self.inert_place(source, depth + 1)
                {
                    return None;
                }
            }
            ExpressionNode::ArrayLiteral(_) => {
                let reference =
                    validation::declared_constant_array_type(&self.checked.typed, source)?;
                validation::closed_literal_array_elements(&self.checked.typed, source, reference)?;
            }
            ExpressionNode::Indexed(indexed) => {
                if let ExpressionNode::Range(_) =
                    self.checked.expression_table.expression(indexed.index)
                {
                    if !self.static_subslice(source, depth + 1) {
                        return None;
                    }
                    let ExpressionNode::Range(range) =
                        self.checked.expression_table.expression(indexed.index)
                    else {
                        return None;
                    };
                    let (ExpressionNode::Integer(start), ExpressionNode::Integer(end)) = (
                        self.checked.expression_table.expression(range.start),
                        self.checked.expression_table.expression(range.end),
                    ) else {
                        return None;
                    };
                    return end.value_u64()?.checked_sub(start.value_u64()?);
                }
                let ExpressionNode::Integer(index) =
                    self.checked.expression_table.expression(indexed.index)
                else {
                    return None;
                };
                if index.value_u64()? >= self.static_extent(indexed.collection, depth + 1)?
                    || !self.indexed_builtin(source, language_core::OperatorSpelling::Index)
                    || !(validation::place_has_builtin_coordinates(
                        &self.checked.typed,
                        machine,
                        Some(state),
                        source,
                    ) || validation::builtin_constant_array_projection_type(
                        &self.checked.typed,
                        machine.symbol,
                        source,
                    )
                    .is_some())
                {
                    return None;
                }
            }
            _ => return None,
        }
        let mut reference =
            validation::declared_place_type_raw(&self.checked.typed, machine, Some(state), source)?;
        for _ in 0..self.checked.type_reference_table.type_reference_count() {
            match self.checked.type_reference_table.type_reference(reference) {
                checked_trees::types::TypeReferenceNode::Reference { referee, .. }
                | checked_trees::types::TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => reference = *referee,
                checked_trees::types::TypeReferenceNode::FixedArray {
                    length: checked_trees::types::FixedArrayLength::Literal(length),
                    ..
                } => return u64::try_from(*length).ok(),
                _ => return None,
            }
        }
        None
    }

    fn inert_place(&self, source: ExpressionHandle, depth: usize) -> bool {
        if self.exhausted(depth) {
            return false;
        }
        match self.checked.expression_table.expression(source) {
            ExpressionNode::Name(_) => true,
            ExpressionNode::Member(member) => self.inert_place(member.receiver, depth + 1),
            ExpressionNode::Borrow(borrow) => self.inert_place(borrow.target, depth + 1),
            ExpressionNode::Indexed(_) => self.static_extent(source, depth + 1).is_some(),
            _ => false,
        }
    }
}
