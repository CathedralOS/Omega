//! Reuse checked call occurrences and the existing structured mutation owner.

use crate::published::places::place_at;
use validation::CallSite;
use validation::CanonicalPlace;

use super::RangeFacts;

use typed_trees::{machine::Machine, state::State};

/// Immutable owner-local call identity, borrowed across incoming passes and branches.
pub(in super::super) struct RangeCallContext<'program> {
    machine: &'program Machine,
    state: &'program State,
    borrows: &'program checked_trees::BorrowFacts,
    borrow_calls: &'program [checked_trees::BorrowCallFact],
    flow_calls: &'program [checked_trees::FlowCallFact],
    flow_statements: &'program [checked_trees::FlowStatementFact],
    flow: &'program checked_trees::FlowFacts,
    semantic: &'program facts::FactPlan,
    call_frames: Option<&'program validation::CallFrameResolver<'program>>,
}

#[derive(Debug)]
enum CapturedIndexRefusal {
    NotBuiltin,
    NotScalarIndex,
    MissingState,
    MissingConsumer,
    UnavailableReceiver,
    MissingSelector,
    MismatchedOccurrence,
    Extent,
    NonIntegerSelector,
    UnavailableBounds,
}

impl<'program> RangeCallContext<'program> {
    pub(in super::super) fn new(
        machine: &'program Machine,
        state: &'program State,
        borrows: &'program checked_trees::BorrowFacts,
        flow: &'program checked_trees::FlowFacts,
        semantic: &'program facts::FactPlan,
        call_frames: Option<&'program validation::CallFrameResolver<'program>>,
    ) -> Self {
        let borrow_calls = borrows
            .first_state_borrow(machine.symbol, state.symbol)
            .map(|(_, owner)| borrows.calls.span_or_empty(owner.calls))
            .unwrap_or(&[]);
        let owner = flow.control.state(machine.symbol, state.symbol);
        let flow_calls = owner
            .map(|owner| flow.control.calls.span_or_empty(owner.calls))
            .unwrap_or(&[]);
        let flow_statements = owner
            .map(|owner| flow.control.statements.span_or_empty(owner.statements))
            .unwrap_or(&[]);
        Self {
            machine,
            state,
            borrows,
            borrow_calls,
            flow_calls,
            flow_statements,
            flow,
            semantic,
            call_frames,
        }
    }

    /// Completed immutable values do not depend on a source initializer's
    /// spelling. Their shared observation survives calls, copies and joins;
    /// mutable contents and reference paths keep the ordinary live range rules.
    pub(in super::super) fn immutable_integer_value(
        &self,
        program: &dyn validation::CheckingInput,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<i64> {
        if !validation::place_has_builtin_coordinates(
            program,
            self.machine,
            Some(self.state),
            expression,
        ) {
            return None;
        }
        let place = place_at(program, self.state.symbol, statement_index, expression)?;
        crate::values::immutable_owned_place_root(program, self.state, statement_index, &place)?;
        let statement = self
            .flow_statements
            .iter()
            .find(|statement| statement.statement_index == statement_index)?;
        let facts::ScalarValue::Integer(value) = crate::values::scalar_value_at_place(
            program,
            self.semantic,
            self.flow
                .semantic_constraint_contexts(statement.entry_constraints)
                .map(|context| self.semantic.contexts.get(context)),
            &place,
        )?
        else {
            return None;
        };
        value.to_i64()
    }

    /// Consume the selector completed by this exact indexed operation. Its
    /// scalar copy keeps the original constraints across later RHS effects.
    /// A declared always-full extent belongs to the captured receiver itself;
    /// slices and unknown geometry retain the existing ordered proof route.
    pub(in super::super) fn captured_fixed_index_bounds(
        &self,
        program: &dyn validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
        length: usize,
    ) -> Option<facts::IntegerRange> {
        let result = self.try_captured_fixed_index_bounds(
            program,
            operators,
            statement_index,
            expression,
            length,
        );
        if let Err(reason) = &result
            && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some()
        {
            eprintln!(
                "CHECKSITE captured-index-bound state={:?} statement={statement_index} expression={expression:?} length={length} refusal={reason:?}",
                self.state.symbol,
            );
        }
        result.ok()
    }

    fn try_captured_fixed_index_bounds(
        &self,
        program: &dyn validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
        length: usize,
    ) -> Result<facts::IntegerRange, CapturedIndexRefusal> {
        if !crate::values::operator_is_builtin(operators, expression)
            || !validation::place_has_builtin_coordinates(
                program,
                self.machine,
                Some(self.state),
                expression,
            )
        {
            return Err(CapturedIndexRefusal::NotBuiltin);
        }
        let indexed = program
            .expression_table
            .indexed_node(expression)
            .ok_or(CapturedIndexRefusal::NotScalarIndex)?;
        if matches!(
            program.expression_table.expression(indexed.index),
            typed_trees::expression::ExpressionNode::Range(_)
        ) {
            return Err(CapturedIndexRefusal::NotScalarIndex);
        }
        let state = self
            .flow
            .control
            .state(self.machine.symbol, self.state.symbol)
            .ok_or(CapturedIndexRefusal::MissingState)?;
        let captures = validation::completed_storage::consumer_captures(
            &self.flow.control,
            self.state.symbol,
            statement_index,
            expression,
        )
        .ok_or(CapturedIndexRefusal::MissingConsumer)?;
        let intervals = crate::values::CapturedIntegerIntervals::new(
            program,
            operators,
            self.semantic,
            self.flow,
            state,
        );
        let mut hull: Option<facts::IntegerRange> = None;
        for capture in captures {
            let operand = self.flow.control.operands.get(capture);
            if operand.expression != expression
                || operand.state_symbol != self.state.symbol
                || operand.statement_index != statement_index
                || operand.evaluation == checked_trees::FlowPermissionEvaluation::Unknown
                || operand.conditional_inputs_from.is_some()
                || !self.flow.control.operands.is_valid(operand.backing)
            {
                return Err(CapturedIndexRefusal::UnavailableReceiver);
            }
            let receiver = self.flow.control.operands.get(operand.backing);
            let [selector] = self
                .flow
                .control
                .operands
                .span(operand.evaluated_inputs)
                .ok_or(CapturedIndexRefusal::MissingSelector)?
            else {
                return Err(CapturedIndexRefusal::MissingSelector);
            };
            if receiver.expression != indexed.collection
                || selector.expression != indexed.index
                || receiver.state_symbol != self.state.symbol
                || selector.state_symbol != self.state.symbol
                || receiver.statement_index != statement_index
                || selector.statement_index != statement_index
                || receiver.evaluation != operand.evaluation
                || selector.evaluation != operand.evaluation
                || !receiver.type_reference.is_valid()
                || !selector.type_reference.is_valid()
            {
                return Err(CapturedIndexRefusal::MismatchedOccurrence);
            }
            if validation::fixed_array_type_length(program, receiver.type_reference) != Some(length)
            {
                return Err(CapturedIndexRefusal::Extent);
            }
            let primitive = program
                .primitive_type_reference(selector.type_reference)
                .ok_or(CapturedIndexRefusal::NonIntegerSelector)?;
            if !primitive.is_fixed_integer() {
                return Err(CapturedIndexRefusal::NonIntegerSelector);
            }
            let selector_capture = crate::flow::CapturedOperand::recorded(
                &self.flow.control,
                operand.evaluated_inputs.handle_at(0),
            )
            .ok_or(CapturedIndexRefusal::MissingSelector)?;
            let bounds = intervals
                .bounds(
                    selector_capture,
                    selector.type_reference,
                    &[],
                    selector.constraints,
                    selector,
                    primitive,
                )
                .ok_or(CapturedIndexRefusal::UnavailableBounds)?;
            hull = Some(match hull {
                Some(previous) => facts::IntegerRange {
                    minimum: previous.minimum.min(bounds.minimum),
                    maximum: previous.maximum.max(bounds.maximum),
                },
                None => bounds,
            });
        }
        hull.ok_or(CapturedIndexRefusal::MissingConsumer)
    }

    /// The pass-shared resolver this context already borrows; `None` leaves a
    /// caller to construct its own exactly as it did before the pass shared one.
    pub(in super::super) fn call_frames(&self) -> Option<&validation::CallFrameResolver<'_>> {
        self.call_frames
    }

    pub(in super::super) fn collection_extent(
        &self,
        program: &dyn ::validation::CheckingInput,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<usize> {
        let statement = self
            .flow_statements
            .iter()
            .find(|statement| statement.statement_index == statement_index)?;
        let place = place_at(program, self.state.symbol, statement_index, expression)?;
        crate::checks::collection_extents::from_live_loans(
            program,
            self.state.symbol,
            statement_index,
            self.borrows,
            self.flow,
            &place,
            self.flow
                .contexts
                .constraint_refs
                .span_or_empty(statement.entry_constraints),
        )
    }

    /// Rejoin the exact checked call occurrence for one authored site in the
    /// owning state. Flow and borrow rows agree on statement and ordinal only
    /// for the same occurrence; anything else reports no evidence.
    pub(in super::super) fn find_call(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        site: &CallSite<'_>,
    ) -> Option<&checked_trees::BorrowCallFact> {
        if machine.symbol != self.machine.symbol || state.symbol != self.state.symbol {
            return None;
        }
        let call = self.flow_calls.iter().find(|call| {
            call.statement_index == statement_index && match site {
                CallSite::Expression { expression, .. } => expression.is_valid()
                    && call.authored_expression == *expression,
                CallSite::Statement(statement_call) => call.call_ordinal == 0
                    && !call.authored_expression.is_valid()
                    && matches!(program.statement_table.statement_at(state, statement_index),
                        Some(typed_trees::statement::StatementNode::Call(candidate))
                            if std::ptr::eq(*statement_call, candidate)),
                // The named target call is the one occurrence in a transition
                // statement whose checked row has no authored expression;
                // nested argument calls still join by expression handle.
                // `transition_call_target` maps the row's ordinal back to the
                // enclosing target handle, and the path field address pins the
                // exact authored node (target and continuation are distinct
                // nodes even when spelled identically).
                CallSite::TransitionNamed { path, .. } => {
                    !call.authored_expression.is_valid()
                        && validation::transition_call_target(
                            program,
                            machine,
                            state,
                            statement_index,
                            call.call_ordinal,
                        )
                        .is_some_and(|target| {
                            target.is_valid()
                                && matches!(
                                    program.statement_table.transition_target(target),
                                    typed_trees::statement::TransitionTargetNode::Named {
                                        path: candidate,
                                        ..
                                    } if std::ptr::eq(*path, candidate)
                                )
                        })
                }
            }
        })?;
        self.borrow_calls.iter().find(|borrow_call| {
            borrow_call.statement_index == call.statement_index
                && borrow_call.call_ordinal == call.call_ordinal
                && borrow_call.target_symbol == call.target_symbol
        })
    }

    /// Materialize the checked operand-access rows of a located call as caller
    /// places. Every recorded access names storage the callee may reach
    /// through its operands; the access kind does not narrow the footprint
    /// because mutable and write-only borrows carry the callee into the same
    /// caller place as a read.
    pub(in super::super) fn operand_access_places(
        &self,
        call: &checked_trees::BorrowCallFact,
    ) -> Vec<CanonicalPlace> {
        self.borrows
            .argument_accesses
            .span_or_empty(call.accesses)
            .iter()
            .map(|access| CanonicalPlace {
                root: access.root,
                segments: self
                    .borrows
                    .access_segments
                    .span_or_empty(access.segments)
                    .to_vec(),
            })
            .collect()
    }
}

/// The checked storage ceiling can be complete while local alias expansion is
/// opaque. Only owned scalar slots may use the ceiling without alias closure.
pub(super) struct CallStorageWrites {
    pub storage: Vec<CanonicalPlace>,
    pub aliases: Option<Vec<CanonicalPlace>>,
}

impl RangeFacts<'_> {
    pub(super) fn structured_call_writes(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        site: &CallSite<'_>,
    ) -> Option<CallStorageWrites> {
        let context = self.checked_calls?;
        let call = context.find_call(program, machine, state, self.statement_index, site)?;
        let mut writes = crate::flow::call_mutated_places(
            program,
            machine.symbol,
            state.symbol,
            context.borrows,
            call,
            &self.mutation_summaries,
            context.call_frames,
        )?;
        // Callee expressions and previously captured selector expressions do
        // not execute in the caller's current value namespace. Unknown index
        // coordinates may overlap; only retained fixed selectors narrow writes.
        for write in &mut writes {
            for segment in &mut write.segments {
                if let facts::PlaceSegment::Index { expression } = segment {
                    *expression = typed_trees::expression::ExpressionHandle::invalid();
                }
            }
        }
        let aliases = crate::flow::close_storage_places_over_aliases(
            program,
            machine.symbol,
            state.symbol,
            self.statement_index,
            writes.clone(),
            context.call_frames,
        );
        Some(CallStorageWrites {
            storage: writes,
            aliases,
        })
    }
}
