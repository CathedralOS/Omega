//! Static source evaluation and bounds custody for eliminated subslice extents.

use super::{
    CheckedCallScalarArgument, Computation, Context, ExpressionHandle, ExpressionNode,
    LoweringError,
};
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
                        Computation::StructuralField {
                            source_expression, ..
                        } => sources.push(*source_expression),
                        Computation::CaseMembership { subject, .. } => {
                            pending.extend(
                                crate::expression_preparation::computation_graph::operand_fields(
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
                        Computation::BooleanToInteger { operand, .. } => pending.push(*operand),
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
                        && !self.slice_backed_subslice(member.receiver)
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
            crate::expression_preparation::source_custody::authored_state(self.checked, self.state)
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
            crate::expression_preparation::source_custody::authored_state(self.checked, self.state)
                .ok()?;
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

    /// A slice-backed eliminated extent is sound only while the same authored
    /// view carries a retained bounds plan at this statement: the operand's
    /// subslice formation still evaluates the endpoints and discharges
    /// `start <= end <= source.len` at runtime, so folding `end - start` keeps
    /// that observable check instead of erasing the view's only bounds
    /// evidence. Without the plan the slice is never materialized and its
    /// bounds go unchecked.
    fn slice_backed_subslice(&self, source: ExpressionHandle) -> bool {
        let ExpressionNode::Indexed(indexed) = self.checked.expression_table.expression(source)
        else {
            return false;
        };
        let ExpressionNode::Range(range) = self.checked.expression_table.expression(indexed.index)
        else {
            return false;
        };
        let Ok((machine, state)) =
            crate::expression_preparation::source_custody::authored_state(self.checked, self.state)
        else {
            return false;
        };
        if range.end_inclusive
            || !self.indexed_builtin(source, language_core::OperatorSpelling::Range)
            || !validation::has_builtin_subslice_meaning(
                &self.checked.typed,
                machine,
                Some(state),
                source,
            )
        {
            return false;
        }
        let mut views = Vec::new();
        self.statement_subslice_views(&mut views);
        views
            .iter()
            .any(|view| self.same_subslice_view(*view, source))
    }

    /// Authored `collection[start..end]` occurrences that a retained
    /// view/bounds plan materializes at this exact statement. Each family
    /// that can own the enclosing statement's structural operands reports its
    /// subslice sources.
    fn statement_subslice_views(&self, views: &mut Vec<ExpressionHandle>) {
        let scalar_graphs = &self.checked.facts.flow.terminal_scalar_graphs;
        for machine in &scalar_graphs.machines {
            for state in machine
                .states
                .iter()
                .filter(|state| state.state == self.state)
            {
                for operation in &state.unit_operations {
                    self.operation_subslice_views(operation, views);
                }
                self.scalar_terminator_subslice_views(&state.terminator, views);
            }
        }
        for tail in scalar_graphs
            .guarded_tails
            .iter()
            .filter(|tail| tail.state == self.state)
        {
            self.scalar_guard_subslice_views(
                scalar_graphs.guarded_exits.span_or_empty(tail.arms),
                tail.fallback.as_ref(),
                views,
            );
        }
        let effects = &self.checked.facts.flow.terminal_unit_effects;
        for machine in effects
            .machines
            .iter()
            .filter(|machine| machine.state == self.state)
        {
            for operation in &machine.operations {
                self.operation_subslice_views(operation, views);
            }
        }
        for machine in &effects.composed_machines {
            for state in machine
                .states
                .iter()
                .filter(|state| state.state == self.state)
            {
                for operation in state.operation_dependencies() {
                    self.operation_subslice_views(operation, views);
                }
                self.composed_terminator_subslice_views(&state.terminator, views);
            }
        }
    }

    /// Structural call arguments on one operation at this statement's
    /// coordinate retain their authored subslice source.
    fn operation_subslice_views(
        &self,
        operation: &checked_trees::CheckedUnitEffectOperationPlan,
        views: &mut Vec<ExpressionHandle>,
    ) {
        use checked_trees::CheckedUnitEffectOperationPlan as Plan;
        let (coordinate, arguments) = match operation {
            Plan::CallUnit {
                coordinate,
                structural_arguments,
                ..
            }
            | Plan::ScalarCall {
                coordinate,
                structural_arguments,
                ..
            }
            | Plan::StructuralCall {
                coordinate,
                structural_arguments,
                ..
            }
            | Plan::BoundaryCall {
                coordinate,
                structural_arguments,
                ..
            }
            | Plan::BoundaryScalarCall {
                coordinate,
                structural_arguments,
                ..
            }
            | Plan::BoundaryStructuralCall {
                coordinate,
                structural_arguments,
                ..
            }
            | Plan::SelectedOperatorStructuralScalarCall {
                coordinate,
                structural_arguments,
                ..
            }
            | Plan::SelectedOperatorStructuralCall {
                coordinate,
                structural_arguments,
                ..
            } => (coordinate.statement_index, structural_arguments),
            Plan::EstablishStructuralValue { calls, .. } => {
                for call in calls {
                    self.operation_subslice_views(call.operation(), views);
                }
                return;
            }
            _ => return,
        };
        if coordinate != self.statement {
            return;
        }
        for argument in arguments {
            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                expression,
                ..
            } = &argument.source
            {
                views.push(*expression);
            }
        }
    }

    fn scalar_terminator_subslice_views(
        &self,
        terminator: &checked_trees::CheckedScalarStateTerminator,
        views: &mut Vec<ExpressionHandle>,
    ) {
        match terminator {
            checked_trees::CheckedScalarStateTerminator::Jump(successor) => {
                self.scalar_successor_subslice_views(successor, views);
            }
            checked_trees::CheckedScalarStateTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                self.scalar_destination_subslice_views(when_true, views);
                self.scalar_destination_subslice_views(when_false, views);
            }
            checked_trees::CheckedScalarStateTerminator::Guarded { arms, fallback } => {
                let arms = self
                    .checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_exits
                    .span_or_empty(*arms);
                self.scalar_guard_subslice_views(arms, fallback.as_ref(), views);
            }
            _ => {}
        }
    }

    fn scalar_guard_subslice_views(
        &self,
        arms: &[checked_trees::CheckedScalarGuardedExit],
        fallback: Option<&checked_trees::CheckedScalarBranchDestination>,
        views: &mut Vec<ExpressionHandle>,
    ) {
        for arm in arms {
            self.scalar_destination_subslice_views(&arm.destination, views);
        }
        if let Some(destination) = fallback {
            self.scalar_destination_subslice_views(destination, views);
        }
    }

    fn scalar_destination_subslice_views(
        &self,
        destination: &checked_trees::CheckedScalarBranchDestination,
        views: &mut Vec<ExpressionHandle>,
    ) {
        let checked_trees::CheckedScalarBranchDestination::Jump(successor) = destination else {
            return;
        };
        self.scalar_successor_subslice_views(successor, views);
    }

    /// A scalar-graph edge retains subslice transfer sources in the shared
    /// transfer arena keyed by the edge's authored statement ordinal.
    fn scalar_successor_subslice_views(
        &self,
        successor: &checked_trees::CheckedScalarSuccessor,
        views: &mut Vec<ExpressionHandle>,
    ) {
        if successor.statement_ordinal != self.statement {
            return;
        }
        for transfer in self
            .checked
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_transfers
            .span_or_empty(successor.structural_transfers)
        {
            if let checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
                expression,
                ..
            } = &transfer.source
            {
                views.push(*expression);
            }
        }
    }

    fn composed_terminator_subslice_views(
        &self,
        terminator: &checked_trees::CheckedComposedUnitControlTerminatorPlan,
        views: &mut Vec<ExpressionHandle>,
    ) {
        use checked_trees::CheckedComposedUnitControlTerminatorPlan as Plan;
        match terminator {
            Plan::Jump { successor } => {
                self.composed_successor_subslice_views(successor, views);
            }
            Plan::Conditional {
                when_true,
                when_false,
                ..
            } => {
                self.composed_successor_subslice_views(when_true, views);
                self.composed_successor_subslice_views(when_false, views);
            }
            Plan::ConditionalReturn { jump, .. } => {
                self.composed_successor_subslice_views(jump, views);
            }
            Plan::Guarded { arms, fallback, .. } => {
                let arms = self
                    .checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_exits
                    .span_or_empty(*arms);
                self.scalar_guard_subslice_views(arms, fallback.as_ref(), views);
            }
            Plan::GuardedJumps { arms, fallback } => {
                for arm in arms {
                    self.composed_successor_subslice_views(&arm.successor, views);
                }
                self.composed_successor_subslice_views(fallback, views);
            }
            Plan::ClosedSum { subject, cases } => {
                if !cases
                    .iter()
                    .any(|case| case.successor.statement_ordinal == self.statement)
                {
                    return;
                }
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    expression,
                    ..
                } = &subject.source
                {
                    views.push(*expression);
                }
                for case in cases {
                    self.composed_successor_subslice_views(&case.successor, views);
                }
            }
            _ => {}
        }
    }

    fn composed_successor_subslice_views(
        &self,
        successor: &checked_trees::CheckedStructuralControlSuccessorPlan,
        views: &mut Vec<ExpressionHandle>,
    ) {
        if successor.statement_ordinal != self.statement {
            return;
        }
        for transfer in &successor.transfers {
            if let checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
                expression,
                ..
            } = &transfer.source
            {
                views.push(*expression);
            }
        }
    }

    /// The retained plan's authored view is a separate occurrence naming the
    /// same subslice: the same single-segment parameter place and the same
    /// literal bounds.
    fn same_subslice_view(&self, retained: ExpressionHandle, source: ExpressionHandle) -> bool {
        let (ExpressionNode::Indexed(retained), ExpressionNode::Indexed(authored)) = (
            self.checked.expression_table.expression(retained),
            self.checked.expression_table.expression(source),
        ) else {
            return false;
        };
        let (ExpressionNode::Range(retained_bounds), ExpressionNode::Range(authored_bounds)) = (
            self.checked.expression_table.expression(retained.index),
            self.checked.expression_table.expression(authored.index),
        ) else {
            return false;
        };
        if retained_bounds.end_inclusive != authored_bounds.end_inclusive
            || retained_bounds.start.is_valid() != authored_bounds.start.is_valid()
            || retained_bounds.end.is_valid() != authored_bounds.end.is_valid()
        {
            return false;
        }
        let (ExpressionNode::Name(retained_collection), ExpressionNode::Name(authored_collection)) = (
            self.checked
                .expression_table
                .expression(retained.collection),
            self.checked
                .expression_table
                .expression(authored.collection),
        ) else {
            return false;
        };
        if retained_collection.symbol != authored_collection.symbol
            || retained_collection.head_symbol != authored_collection.head_symbol
            || self
                .checked
                .expression_table
                .name_path_member_symbols(retained_collection.member_symbols)
                != self
                    .checked
                    .expression_table
                    .name_path_member_symbols(authored_collection.member_symbols)
        {
            return false;
        }
        let same_bound = |left: ExpressionHandle, right: ExpressionHandle| match (
            self.checked.expression_table.expression(left),
            self.checked.expression_table.expression(right),
        ) {
            (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => {
                left.value_u64() == right.value_u64()
            }
            _ => false,
        };
        (!retained_bounds.start.is_valid()
            || same_bound(retained_bounds.start, authored_bounds.start))
            && (!retained_bounds.end.is_valid()
                || same_bound(retained_bounds.end, authored_bounds.end))
    }
}
