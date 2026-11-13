/*
    `RangeFacts`, the fact store the range check carries through one state's
    statements: collection lengths of fields and locals, integer values,
    Boolean guard locals, and the proven index, upper bound, non-negative,
    ordering, range bound, minimum length, exact length and subslice window
    relations, keyed by expression labels.

    `ranges::check_indexed_accesses` and `state_arguments` create one per
    state. Its methods are split by concern: `values` defines, assigns and
    reads locals and field integers; `proofs` records and queries the proven
    relations; `dependencies` records the typed reads behind each expression
    label, so a write can tell which labels it preserves or affects;
    scalar binding rows retain live descriptor measurements separately from
    the captured scalar's own storage, so guards never replay initializers;
    `invalidation` retires facts after an assignment or a call's writes; and
    `call_writes` provides `RangeCallContext`, the per-state view of checked
    call and borrow facts through which a call's writes are found.
*/

/* Membership sets take the integer hasher; none is iterated for output order. */
type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;
use crate::flow::StateMutationSummaryCache;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

mod call_writes {
    /*
        Reuse checked call occurrences and the existing structured mutation owner.
    */
    use crate::published::places::place_at;
    use validation::CallSite;
    use validation::CanonicalPlace;

    use super::RangeFacts;

    use typed_trees::{machine::Machine, state::State};

    /* Immutable owner-local call identity, borrowed across incoming passes and branches. */
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

        /*
            Completed immutable values do not depend on a source initializer's
            spelling. Their shared observation survives calls, copies and joins;
            mutable contents and reference paths keep the ordinary live range rules.
        */
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
            crate::values::immutable_owned_place_root(
                program,
                self.state,
                statement_index,
                &place,
            )?;
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

        /*
            Consume the selector completed by this exact indexed operation. Its
            scalar copy keeps the original constraints across later RHS effects.
            A declared always-full extent belongs to the captured receiver itself;
            slices and unknown geometry retain the existing ordered proof route.
        */
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
                && ::validation::trace_flags::check_sites()
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
                if validation::fixed_array_type_length(program, receiver.type_reference)
                    != Some(length)
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

        /*
            The pass-shared resolver this context already borrows; `None` leaves a
            caller to construct its own exactly as it did before the pass shared one.
        */
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

        /*
            Rejoin the exact checked call occurrence for one authored site in the
            owning state. Flow and borrow rows agree on statement and ordinal only
            for the same occurrence; anything else reports no evidence.
        */
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

        /*
            Materialize the checked operand-access rows of a located call as caller
            places. Every recorded access names storage the callee may reach
            through its operands; the access kind does not narrow the footprint
            because mutable and write-only borrows carry the callee into the same
            caller place as a read.
        */
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

    /*
        The checked storage ceiling can be complete while local alias expansion is
        opaque. Only owned scalar slots may use the ceiling without alias closure.
    */
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
}
mod closed_arrivals {
    /*
        Closed, formed integer arrivals use the existing strict contradiction judgment.
    */
    use super::RangeFacts;
    use typed_trees::expression::{BinaryOperator, TableBinaryExpression};

    impl RangeFacts<'_> {
        /*
            The caller established builtin comparison meaning and selected this
            exact arm. Literal operands are invocation-fixed; their source terms preserve
            formation. Address and IEEE operands cannot supply integer facts.
        */
        pub(in crate::checks::ranges) fn retain_closed_integer_arrival(
            &mut self,
            program: &dyn validation::CheckingInput,
            binary: &TableBinaryExpression,
            operator: BinaryOperator,
        ) {
            if !matches!(
                operator,
                BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
            ) {
                return;
            }
            let Some(left) = program.expression_table.integer_node(binary.left) else {
                return;
            };
            let Some(right) = program.expression_table.integer_node(binary.right) else {
                return;
            };
            if [left, right].into_iter().any(|literal| {
                literal.landing().is_some_and(|landing| {
                    landing.landed_type == numerics::literals::LandedIntegerType::Addr
                })
            }) {
                return;
            }
            let arrival = (binary.left, binary.right, operator);
            crate::shared::collections::push_unique(&mut self.closed_integer_arrivals, arrival);
        }

        /*
            Bounds remain ordinary obligations. An exact closed arrival can close
            them only when the existing scoped integer engine proves contradiction;
            unknown terms/operators decline and no source arm is pruned.
        */
        pub(in crate::checks::ranges) fn closed_integer_arrival_is_impossible(
            &self,
            program: &dyn validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
        ) -> bool {
            if self.closed_integer_arrivals.is_empty() {
                return false;
            }
            let term = |expression| {
                validation::ScopedArithmeticValue::Term(validation::ScopedArithmeticExpression {
                    owner: machine.symbol,
                    expression,
                    bindings: Vec::new(),
                })
            };
            let comparisons: Vec<_> = self
                .closed_integer_arrivals
                .iter()
                .map(
                    |(left, right, operator)| validation::ScopedArithmeticComparison {
                        operator: *operator,
                        left: term(*left),
                        right: term(*right),
                    },
                )
                .collect();
            validation::scoped_arithmetic_comparisons_unsatisfiable(program, machine, &comparisons)
        }
    }
}
pub(super) use call_writes::RangeCallContext;
mod dependencies {
    /*
        Exact typed reads behind the range checker's legacy expression labels.

        Incoming guards measure predecessor storage. Record those reads in their
        evaluation scope, then transport exact parameter roots and selector operands
        to the receiving state. Field/index paths survive that join; spelling alone cannot keep a
        stale predecessor parameter current or authorize an unrelated binding.
    */
    use super::super::types::expression_type_reference;
    use super::invalidation::write_covers_place;
    use crate::flow;
    use crate::published::places::place_at;
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use typed_trees::types::TypeReferenceNode;
    use validation::expression_traversal::single_segment_name_symbol;
    use validation::{
        CanonicalPlace, collection_length_receiver, has_builtin_bound_expression_meaning,
        length_member, unwrapped_type_reference,
    };

    mod captures {
        use super::{
            EXPRESSION_WALK_DEPTH_BOUND as ND, ExpressionHandle, ExpressionNode, Machine, State,
            SymbolHandle,
        };
        use crate::checks::ranges::RangeFacts;
        use crate::checks::ranges::facts::dependencies::ExpressionDependencies;
        use crate::checks::ranges::facts::dependencies::reads;
        use crate::checks::ranges::facts::dependencies::same_reads;
        use crate::flow::expressions;
        use crate::published::places::place_at;
        use facts::PlaceRoot;
        use facts::PlaceSegment;
        use validation::ImmutableBoundLookup;
        use validation::collection_length_receiver;
        use validation::expression_result_type_reference;
        use validation::first_valid_name_path_symbol;
        use validation::has_builtin_binary_expression_meaning;
        use validation::immutable_integer_bound_value_symbol;
        use validation::normalize_immutable_integer_bound_expression;
        use validation::place_has_builtin_coordinates;
        use validation::place_root_may_be_symbol;

        impl<'field> RangeFacts<'field> {
            pub(in crate::checks::ranges) fn alias_integer_value(
                &mut self,
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                expression: ExpressionHandle,
                symbol: SymbolHandle,
                name: &str,
            ) {
                if !has_scalar_snapshot_meaning(program, machine, state, expression, 0)
                    || !is_integer_value(program, machine, state, expression)
                {
                    return;
                }
                let mut reads = Vec::new();
                let complete = self.walk_reads(
                    program,
                    machine,
                    state,
                    self.statement_index,
                    expression,
                    &mut reads,
                );
                if complete {
                    self.alias_index(&program.expression_table.display_name(expression), name);
                    self.alias_captured_selector(program, machine, state, expression, symbol);
                }
            }
            /*
                Rejoin actual typed uses of a selector copied now. Never expand an old
                initializer later or substitute names inside rendered source.
            */
            fn alias_captured_selector(
                &mut self,
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                captured: ExpressionHandle,
                symbol: SymbolHandle,
            ) {
                if !symbol.is_valid() || self.expression_dependencies.is_empty() {
                    return;
                }
                let mut captured_reads = Vec::new();
                let complete = self.walk_reads(
                    program,
                    machine,
                    state,
                    self.statement_index,
                    captured,
                    &mut captured_reads,
                );
                if !complete {
                    return;
                }
                let transferable =
                    self.preserved_expression_labels(program, machine, state, Some(&[]));
                let originals = self.expression_dependencies.clone();
                let bound_lookup = self.bound_lookup();
                let mut matched_sources = Vec::new();
                for original in originals {
                    if !transferable.contains(&original.label) {
                        continue;
                    }
                    let source = collection_length_receiver(
                        program,
                        machine,
                        Some(state),
                        original.expression,
                    )
                    .unwrap_or(original.expression);
                    if matched_sources.contains(&source) {
                        continue;
                    }
                    matched_sources.push(source);
                    if !place_has_builtin_coordinates(program, machine, Some(state), source) {
                        continue;
                    }
                    let Some(source_place) =
                        place_at(program, state.symbol, self.statement_index, source)
                    else {
                        continue;
                    };
                    if !source_place.segments.iter().any(|segment| {
                        matches!(segment, PlaceSegment::Index { expression }
                            if program.expression_table.expressions_structurally_equal(*expression, captured))
                    }) {
                        continue;
                    }
                    let source_label = program.expression_table.display_name(source);
                    for (expression, _) in program.expression_table.iter_expressions() {
                        if let PlaceRoot::Symbol(root) = source_place.root
                            && !place_root_may_be_symbol(program, expression, root)
                        {
                            continue;
                        }
                        let Some(mut candidate) = place_at(
                            program,
                            state.symbol,
                            self.statement_index.saturating_add(1),
                            expression,
                        ) else {
                            continue;
                        };
                        if candidate.root != source_place.root
                            || candidate.segments.len() != source_place.segments.len()
                        {
                            continue;
                        }
                        let mut rebound = false;
                        let mut same_snapshot = false;
                        for (source_segment, candidate_segment) in
                            source_place.segments.iter().zip(&mut candidate.segments)
                        {
                            let (
                                PlaceSegment::Index {
                                    expression: source_index,
                                },
                                PlaceSegment::Index {
                                    expression: candidate_index,
                                },
                            ) = (source_segment, &*candidate_segment)
                            else {
                                continue;
                            };
                            if !expressions(program)
                                .expressions_structurally_equal(*source_index, captured)
                            {
                                continue;
                            }
                            let Some(selector) =
                                program.expression_table.name_path_node(*candidate_index)
                            else {
                                continue;
                            };
                            let selector_symbol =
                                first_valid_name_path_symbol(selector, &program.expression_table);
                            let shared_snapshot = selector.symbol.is_valid()
                                && selector.head_symbol == selector.symbol
                                && selector_symbol == Some(selector.symbol)
                                && integer_value_identity(
                                    program,
                                    bound_lookup,
                                    state,
                                    *candidate_index,
                                )
                                .is_some_and(|value| {
                                    value == symbol
                                        || integer_value_identity(
                                            program,
                                            bound_lookup,
                                            state,
                                            captured,
                                        ) == Some(value)
                                });
                            if (selector_symbol != Some(symbol) && !shared_snapshot)
                                || expressions(program)
                                    .name_path_members(selector.members)
                                    .len()
                                    != 1
                            {
                                continue;
                            }
                            let mut selector_reads = Vec::new();
                            let complete = self.walk_reads(
                                program,
                                machine,
                                state,
                                self.statement_index,
                                *source_index,
                                &mut selector_reads,
                            );
                            if !complete
                                || !same_reads(
                                    program,
                                    Some(&captured_reads),
                                    Some(&selector_reads),
                                )
                            {
                                continue;
                            }
                            *candidate_segment = *source_segment;
                            rebound = true;
                            same_snapshot |= shared_snapshot;
                        }
                        if !rebound
                            || !crate::flow::canonical_places_agree(
                                program,
                                &source_place,
                                &candidate,
                            )
                            || !place_has_builtin_coordinates(
                                program,
                                machine,
                                Some(state),
                                expression,
                            )
                        {
                            continue;
                        }
                        let mut reads = Vec::new();
                        let prefix = self.statement_index.saturating_add(1);
                        let complete = self.walk_reads(
                            program,
                            machine,
                            state,
                            if same_snapshot {
                                crate::flow::state_statements(program, state.statement_nodes).len()
                            } else {
                                prefix
                            },
                            expression,
                            &mut reads,
                        );
                        if !complete
                            || !reads.iter().all(|read| {
                                reads::root_is_current(program, machine, state, prefix, read.root)
                            })
                        {
                            continue;
                        }
                        let label = program.expression_table.display_name(expression);
                        self.alias_index(&source_label, &label);
                        self.prove_window_parent(label.clone(), source_label.clone(), None);
                        self.alias_collection(&source_label, &label);
                        if !self.expression_dependencies.iter().any(|row| {
                            row.expression == expression
                                && row.machine == machine.symbol
                                && row.state == state.symbol
                        }) {
                            self.expression_dependencies.push(ExpressionDependencies {
                                expression,
                                label,
                                machine: machine.symbol,
                                state: state.symbol,
                                descriptor: original.descriptor.as_ref().and_then(|_| {
                                    place_at(program, state.symbol, prefix, expression)
                                }),
                                reads: Some(reads),
                            });
                        }
                    }
                }
            }
        }

        /*
            Reuse the shared snapshot identity only through exact, immutable local
            copies in this state. Its contextual name fallback cannot supply missing
            typed identities for a dependency-preservation grant.
        */
        pub(super) fn integer_value_identity(
            program: &dyn ::validation::CheckingInput,
            bound_lookup: &ImmutableBoundLookup<'_>,
            state: &State,
            mut expression: ExpressionHandle,
        ) -> Option<SymbolHandle> {
            let value = immutable_integer_bound_value_symbol(program, bound_lookup, expression)
                .or_else(|| {
                    let normalized = normalize_immutable_integer_bound_expression(
                        program,
                        bound_lookup,
                        expression,
                    )?;
                    let path = program.expression_table.name_path_node(normalized)?;
                    Some(path.symbol)
                })?;
            for _ in 0..ND {
                let symbol = ::validation::single_segment_name_symbol(program, expression)?;
                let Some((_, local)) = program.state_local_declaration(state, symbol) else {
                    let mut parameters = program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| parameter.symbol == symbol);
                    let parameter = parameters.next()?;
                    return (parameter.symbol == value
                        && !parameter.is_mutable
                        && parameters.next().is_none())
                    .then_some(value);
                };
                if local.is_mutable {
                    return None;
                }
                if local.symbol == value {
                    return Some(value);
                }
                expression = local.ordinary_value_initializer()?;
            }
            None
        }

        /*
            A copied selector can preserve a pure computation just as it preserves a
            direct parameter read. Calls and atomic observations need occurrence-specific
            result evidence; matching their operands does not establish the same value.
            This does not replay an initializer: the caller captures at this statement
            and retains the collected reads for subsequent write invalidation.
        */
        pub(super) fn has_scalar_snapshot_meaning(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            depth: usize,
        ) -> bool {
            if depth >= ND || !program.expression_table.expression_is_valid(expression) {
                return false;
            }
            let child = |expression| {
                has_scalar_snapshot_meaning(program, machine, state, expression, depth + 1)
            };
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Boolean(_) => true,
                ExpressionNode::Member(member) => child(member.receiver),
                ExpressionNode::Indexed(indexed) => {
                    place_has_builtin_coordinates(program, machine, Some(state), expression)
                        && child(indexed.collection)
                        && child(indexed.index)
                }
                ExpressionNode::Binary(binary) => {
                    has_builtin_binary_expression_meaning(program, machine, Some(state), expression)
                        && child(binary.left)
                        && child(binary.right)
                }
                ExpressionNode::Unary(unary) => child(unary.operand),
                ExpressionNode::Cast(cast) => child(cast.value),
                _ => false,
            }
        }

        pub(super) fn is_integer_value(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
        ) -> bool {
            use typed_trees::types::TypeReferenceNode;
            let Some(mut reference) =
                expression_result_type_reference(program, machine, state, expression)
            else {
                return false;
            };
            for _ in 0..ND {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::Named { symbol, .. } => {
                        return program
                            .symbols
                            .builtin_type_atom(*symbol)
                            .is_some_and(|atom| atom.is_fixed_integer());
                    }
                    _ => return false,
                }
            }
            false
        }
    }
    mod place_aliases {
        /*
            Reanchor live facts at a binding using resolved storage coordinates.

            The alias captures its source now. Future uses supply only typed destination
            coordinates, never facts or replayed initializers. Rendered labels remain
            keys for the legacy fact tables, not evidence that two places agree.
            A selector temporary can still equal its original observation at capture;
            only the recorded, write-invalidated scalar relation establishes that join.
            Once reanchored, the reference depends on its own captured descriptor and
            descendant selectors, not the variables that selected its original root.
        */
        use super::captures::integer_value_identity;
        use super::{ExpressionDependencies, reads};
        use crate::checks::ranges::RangeFacts;
        use crate::flow::expressions;
        use crate::published::places::place_at;
        use facts::PlaceRoot;
        use facts::PlaceSegment;
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::{machine::Machine, state::State};
        use validation::CheckingInput;
        use validation::collection_length_receiver;
        use validation::place_has_builtin_coordinates;
        use validation::place_root_may_be_symbol;
        use validation::single_segment_name_symbol;
        use validation::whole_collection_view_source;

        impl RangeFacts<'_> {
            pub(in crate::checks::ranges) fn alias_place_facts(
                &mut self,
                program: &dyn CheckingInput,
                machine: &Machine,
                state: &State,
                captured: ExpressionHandle,
                symbol: SymbolHandle,
            ) {
                if !symbol.is_valid()
                    || self.expression_dependencies.is_empty()
                    || !place_has_builtin_coordinates(program, machine, Some(state), captured)
                {
                    return;
                }
                let captured =
                    whole_collection_view_source(program, machine, Some(state), captured)
                        .unwrap_or(captured);
                let Some(source) = place_at(program, state.symbol, self.statement_index, captured)
                    .and_then(|place| {
                        crate::flow::rebase_exact_local_place(
                            program,
                            state.symbol,
                            self.statement_index,
                            place,
                            self.checked_calls.and_then(|context| context.call_frames()),
                        )
                    })
                else {
                    return;
                };
                if !matches!(source.root, PlaceRoot::Symbol(root) if root.is_valid()) {
                    return;
                }
                let transferable =
                    self.preserved_expression_labels(program, machine, state, Some(&[]));
                let call_frames = self.checked_calls.and_then(|context| context.call_frames());
                let originals: Vec<_> = self
                    .expression_dependencies
                    .iter()
                    .filter(|original| transferable.contains(&original.label))
                    .filter_map(|original| {
                        let subject = collection_length_receiver(
                            program,
                            machine,
                            Some(state),
                            original.expression,
                        )
                        .unwrap_or(original.expression);
                        let original_place =
                            place_at(program, state.symbol, self.statement_index, subject)
                                .and_then(|place| {
                                    crate::flow::rebase_exact_local_place(
                                        program,
                                        state.symbol,
                                        self.statement_index,
                                        place,
                                        call_frames,
                                    )
                                })?;
                        (original_place.root == source.root)
                            .then(|| (original.descriptor.is_some(), subject, original_place))
                    })
                    .collect();
                if originals.is_empty() {
                    return;
                }
                let prefix = self.statement_index.saturating_add(1);
                let bound_lookup = self.bound_lookup();
                for (expression, _) in program.expression_table.iter_expressions() {
                    if !place_root_may_be_symbol(program, expression, symbol) {
                        continue;
                    }
                    let Some(candidate) = place_at(program, state.symbol, prefix, expression)
                    else {
                        continue;
                    };
                    if candidate.root != PlaceRoot::Symbol(symbol)
                        || !place_has_builtin_coordinates(program, machine, Some(state), expression)
                    {
                        continue;
                    }
                    let Some(rebased) = crate::flow::project_captured_view_place(
                        source.clone(),
                        &candidate.segments,
                    ) else {
                        continue;
                    };
                    for &(has_descriptor, subject, ref original_place) in &originals {
                        let mut uses_frozen_selector = false;
                        let same_place = original_place.root == rebased.root
                            && original_place.segments.len() == rebased.segments.len()
                            && original_place.segments.iter().zip(&rebased.segments).all(
                                |(left, right)| {
                                    if let (
                                        PlaceSegment::Index { expression: left },
                                        PlaceSegment::Index { expression: right },
                                    ) = (left, right)
                                    {
                                        let left = self.current_scalar_observation(
                                            program, machine, state, *left,
                                        );
                                        let right = self.current_scalar_observation(
                                            program, machine, state, *right,
                                        );
                                        if expressions(program)
                                            .expressions_structurally_equal(left, right)
                                        {
                                            return true;
                                        }
                                        let same_value = integer_value_identity(
                                            program,
                                            bound_lookup,
                                            state,
                                            left,
                                        )
                                        .is_some_and(|value| {
                                            integer_value_identity(
                                                program,
                                                bound_lookup,
                                                state,
                                                right,
                                            ) == Some(value)
                                        });
                                        uses_frozen_selector |= same_value;
                                        same_value
                                    } else {
                                        crate::flow::canonical_place_segments_equal(
                                            program, *left, *right,
                                        )
                                    }
                                },
                            );
                        if !same_place {
                            continue;
                        }
                        let mut dependencies = Vec::new();
                        let complete = self.walk_reads(
                            program,
                            machine,
                            state,
                            if uses_frozen_selector {
                                crate::flow::state_statements(program, state.statement_nodes).len()
                            } else {
                                prefix
                            },
                            expression,
                            &mut dependencies,
                        );
                        if !complete
                            || !dependencies.iter().all(|read| {
                                reads::root_is_current(program, machine, state, prefix, read.root)
                            })
                        {
                            continue;
                        }
                        let source_label = program.expression_table.display_name(subject);
                        let label = program.expression_table.display_name(expression);
                        self.prove_window_parent(label.clone(), source_label.clone(), None);
                        self.alias_collection(&source_label, &label);
                        self.alias_index(&source_label, &label);
                        if !self.expression_dependencies.iter().any(|row| {
                            row.expression == expression
                                && row.machine == machine.symbol
                                && row.state == state.symbol
                        }) {
                            self.expression_dependencies.push(ExpressionDependencies {
                                expression,
                                label,
                                machine: machine.symbol,
                                state: state.symbol,
                                reads: Some(dependencies),
                                descriptor: has_descriptor.then(|| candidate.clone()),
                            });
                        }
                    }
                }
            }

            /*
                Follow only recorded, still-live equalities. This never replays an
                initializer after writes or guesses a selector from its rendered name.
            */
            fn current_scalar_observation(
                &self,
                program: &dyn CheckingInput,
                machine: &Machine,
                state: &State,
                mut expression: ExpressionHandle,
            ) -> ExpressionHandle {
                for _ in 0..super::EXPRESSION_WALK_DEPTH_BOUND {
                    let Some(symbol) = single_segment_name_symbol(program, expression) else {
                        break;
                    };
                    let Some(value) = self.scalar_binding_dependencies.iter().find_map(|row| {
                        (row.machine == machine.symbol
                            && row.state == state.symbol
                            && row.place.root == PlaceRoot::Symbol(symbol)
                            && row.place.segments.is_empty())
                        .then_some(row.value.as_ref())
                        .flatten()
                    }) else {
                        break;
                    };
                    expression = value.expression;
                }
                expression
            }
        }
    }
    mod reads {
        use super::{
            EXPRESSION_WALK_DEPTH_BOUND, ExpressionHandle, ExpressionNode, Machine, State,
        };
        use crate::checks::ranges::facts::RangeCallContext;
        use crate::flow::expressions;
        use crate::published::places::place_at;
        use crate::published::proof::contract_target_from_state_symbol;
        use crate::shared::collections::push_unique;
        use facts::PlaceRoot;
        use typed_trees::expression::MatchPattern;
        use validation::CanonicalPlace;
        use validation::{CallSite, ImmutableBoundLookup, find_state_in_machine};

        use checked_trees::CheckedOperatorFacts;
        mod atomics {
            /*
                Atomic read/write operand reads: carrier targets, local names, and the
                fetch/compare-exchange operand spellings an atomic call contributes.
            */
            use crate::flow::expressions;
            use validation::CanonicalPlace;

            use super::super::ExpressionHandle;
            use super::ReadWalk;

            use symbols::SymbolHandle;

            /*
                Whether a call's static application substitutes only storage-free
                selections, so the checked operand accesses still enumerate the complete
                caller footprint. A type argument names a declaration identity and a
                const argument names a compile-time value; neither is a place, and
                specialization "creates no runtime dictionary", so a statically applied
                call reads exactly what the same call without the application reads. A
                machine-valued argument is different in kind: it hands the callee a
                callable body this occurrence never authenticated, whose own reads no
                operand scan here describes. A nested static application or an evidence
                projection can carry such a binder below the argument this scan sees, so
                both stay unproven rather than being walked for a machine leaf.
            */
            pub(super) fn static_application_carries_no_caller_storage(
                program: &dyn ::validation::CheckingInput,
                call: &typed_trees::expression::TableCallExpression,
            ) -> bool {
                call.machine_arguments.iter().all(|argument| {
                    if argument.application.is_some() || argument.evidence_projection.is_some() {
                        return false;
                    }
                    if !argument.symbol.is_valid() {
                        return argument.const_literal.is_some();
                    }
                    matches!(
                        program.symbols.get(argument.symbol).kind,
                        symbols::SymbolKind::BuiltinType
                            | symbols::SymbolKind::Data
                            | symbols::SymbolKind::Const
                    )
                })
            }

            impl ReadWalk<'_, '_, '_> {
                /*
                    The footprint of a writing atomic (`store`, `swap`, fetch, or decisive
                    compare-exchange). All four exist only as an assignment carrier: the
                    parser desugars `place.store(v, ord)` and the `let r = place.op(..)`
                    forms into `target = Atomic { .. }`, so the resident place is that
                    carrier statement's `target`, never a child of `value`. The admitted
                    read set is every storage position the carrier names: the resident
                    place, the operand expressions the instruction consults besides the
                    resident place (`value` for store/swap, the fetch operand for
                    read-modify-write, the expected and replacement operands for a decisive
                    compare-exchange whose `value` is the exact prior-shaped update model),
                    and the `result` destination the carrier materializes into — a slot the
                    carrier owns, so a write reaching it can never slip past the recorded
                    label. The model's prior positions are placeholders for the
                    instruction-observed pre-write value naming the same local `result`
                    receives; they are never scanned as local reads. A missing carrier or a
                    result destination that is not current local storage leaves the
                    footprint incomplete.
                */
                pub(super) fn collect_atomic_write_reads(
                    &mut self,
                    expression: ExpressionHandle,
                    atomic: &typed_trees::expression::TableAtomicExpression,
                    depth: usize,
                ) -> bool {
                    let program = self.program;
                    use language_core::atomic::AtomicOrderingPlan;
                    if let Some(carrier) = validation::atomic_once_carrier(program, expression) {
                        return self.collect_place_read(carrier.place, depth + 1)
                            && carrier
                                .operands
                                .iter()
                                .all(|operand| self.collect_operand_reads(*operand, depth + 1));
                    }
                    let Some(resident) = self.atomic_carrier_target(expression) else {
                        return false;
                    };
                    let (operands, result_symbol): (Vec<ExpressionHandle>, Option<SymbolHandle>) =
                        match atomic.ordering {
                            AtomicOrderingPlan::Load(_)
                            | AtomicOrderingPlan::CompareExchangeOnce { .. } => {
                                return false;
                            }
                            AtomicOrderingPlan::Store(_) => (vec![atomic.value], None),
                            AtomicOrderingPlan::Swap(_) => {
                                let Some(result_symbol) =
                                    self.atomic_local_name_symbol(atomic.result)
                                else {
                                    return false;
                                };
                                (vec![atomic.value], Some(result_symbol))
                            }
                            AtomicOrderingPlan::ReadModifyWrite(_) => {
                                let Some(result_symbol) =
                                    self.atomic_local_name_symbol(atomic.result)
                                else {
                                    return false;
                                };
                                let Some(operand) = atomic_fetch_operand(program, atomic) else {
                                    return false;
                                };
                                (vec![operand], Some(result_symbol))
                            }
                            AtomicOrderingPlan::CompareExchange { .. } => {
                                let Some(result_symbol) =
                                    self.atomic_local_name_symbol(atomic.result)
                                else {
                                    return false;
                                };
                                let Some(operands) =
                                    atomic_compare_exchange_operands(program, atomic)
                                else {
                                    return false;
                                };
                                (operands.to_vec(), Some(result_symbol))
                            }
                        };
                    self.collect_place_read(resident, depth + 1)
                        && operands
                            .iter()
                            .all(|operand| self.collect_operand_reads(*operand, depth))
                        && result_symbol.is_none_or(|symbol| {
                            let place = CanonicalPlace {
                                root: facts::PlaceRoot::Symbol(symbol),
                                segments: Vec::new(),
                            };
                            crate::shared::collections::push_unique(self.reads, place);
                            true
                        })
                }

                /*
                    The resident place of a writing atomic is its carrier assignment's
                    target. An atomic not carried by the exact assignment at this statement
                    index has no provable resident place.
                */
                fn atomic_carrier_target(
                    &self,
                    expression: ExpressionHandle,
                ) -> Option<ExpressionHandle> {
                    let program = self.program;
                    let state = self.state;
                    let statement_index = self.statement_index;
                    let statement = program
                        .statement_table
                        .statement_at(state, statement_index)?;
                    let assignment = statement.as_assignment()?;
                    (assignment.value == expression).then_some(assignment.target)
                }

                /*
                    A single-member name's storage identity in this state: the resolved
                    symbol when the path carries one, else the nearest current local binding
                    the member text — the same name binding the desugar's generated result
                    and placeholder names receive, since those never carry resolved symbols.
                    The root must still be current storage at this statement; a member path,
                    an unresolved name, or a name bound only later is not a destination.
                */
                fn atomic_local_name_symbol(
                    &self,
                    expression: ExpressionHandle,
                ) -> Option<SymbolHandle> {
                    let program = self.program;
                    let state = self.state;
                    let statement_index = self.statement_index;
                    if !program.expression_table.expression_is_valid(expression) {
                        return None;
                    }
                    let path = program.expression_table.name_path_node(expression)?;
                    if path.members.count() != 1 {
                        return None;
                    }
                    let symbol =
                        validation::first_valid_name_path_symbol(path, &program.expression_table)
                            .or_else(|| {
                            let name = expressions(program)
                                .name_path_members(path.members)
                                .first()?;
                            crate::flow::state_statements(program, state.statement_nodes)
                                .iter()
                                .take(statement_index)
                                .rev()
                                .find_map(|statement| match statement {
                                    typed_trees::statement::StatementNode::LocalData(local)
                                        if local.name == *name =>
                                    {
                                        Some(local.symbol)
                                    }
                                    _ => None,
                                })
                        })?;
                    self.root_is_current(facts::PlaceRoot::Symbol(symbol))
                        .then_some(symbol)
                }
            }

            /*
                The fetch update model the parser's desugar builds is `prior OP operand`:
                its left operand is the observed-prior placeholder naming the result
                destination, and its right operand is the authored operand `d`.
            */
            pub(super) fn atomic_fetch_operand(
                program: &dyn ::validation::CheckingInput,
                atomic: &typed_trees::expression::TableAtomicExpression,
            ) -> Option<ExpressionHandle> {
                let binary = program.expression_table.binary_node(atomic.value)?;
                Some(binary.right)
            }

            /*
                The decisive compare-exchange update model the parser's desugar builds is
                `prior + (prior == expected) * (replacement - prior)`: the resident prior
                is read through placeholder positions naming the result destination, and
                the remaining reads are exactly the expected and replacement operands.
            */
            pub(super) fn atomic_compare_exchange_operands(
                program: &dyn ::validation::CheckingInput,
                atomic: &typed_trees::expression::TableAtomicExpression,
            ) -> Option<[ExpressionHandle; 2]> {
                let sum = program.expression_table.binary_node(atomic.value)?;
                let product = program.expression_table.binary_node(sum.right)?;
                let equal = program.expression_table.binary_node(product.left)?;
                let difference = program.expression_table.binary_node(product.right)?;
                Some([equal.right, difference.left])
            }
        }
        mod members {
            /*
                Member, index, and selector reads: projected places, temporary member
                symbols, and the operand rosters an operator call reads through.
            */
            use crate::flow::expressions;
            use crate::published::places::place_at;
            use validation::CanonicalPlace;

            use super::super::{
                EXPRESSION_WALK_DEPTH_BOUND as ND, ExpressionHandle, ExpressionNode, Machine, State,
            };
            use super::ReadWalk;

            use checked_trees::{
                CheckedOperatorResolutionStatus, CheckedValueOrigin, CheckedValueStatementRole,
            };
            use symbols::SymbolHandle;

            impl ReadWalk<'_, '_, '_> {
                /*
                    A member on a place chain reads its projected place. A member whose
                    receiver produces a temporary instead of naming storage — `compute().a`,
                    `Pair { .. }.a`, `compute().inner.a`, `match flag { .. }.a` — reads
                    exactly whatever producing that temporary read; the projection itself
                    touches no caller place, so the member expression's canonical place is
                    expression-rooted and the place path cannot describe it. That shape admits
                    only when the member still resolves to a declared field of the receiver's
                    exact type — the identity `temporary_member_symbol` recovers — and the
                    receiver's own read scan completes, so a missing call-occurrence custody
                    row or an unproven operand still leaves the set incomplete. A `Borrow`
                    receiver keeps the place floor either way: `(&x).f` is `x.f`, while a
                    borrow of a temporary has no statement-use place custody to lend the
                    projection.
                */
                pub(super) fn collect_member_reads(
                    &mut self,
                    expression: ExpressionHandle,
                    member: &typed_trees::expression::TableMemberExpression,
                    depth: usize,
                ) -> bool {
                    let program = self.program;
                    let machine = self.machine;
                    let state = self.state;
                    let statement_index = self.statement_index;
                    if let Some(receiver) = validation::collection_length_receiver(
                        program,
                        machine,
                        Some(state),
                        expression,
                    ) {
                        return self.collect_place_read(receiver, depth + 1);
                    }
                    let receiver_is_borrow = matches!(
                        program.expression_table.expression(member.receiver),
                        ExpressionNode::Borrow(_)
                    );
                    if receiver_is_borrow {
                        return self.collect_place_read(expression, depth);
                    }
                    if let Some(place) =
                        place_at(program, state.symbol, statement_index, expression)
                        && matches!(place.root, facts::PlaceRoot::Symbol(_))
                    {
                        return self.descend_and_capture_symbol_place(expression, place, depth);
                    }
                    self.temporary_member_symbol_at(member, 0).is_valid()
                        && self.collect_reads(member.receiver, depth + 1)
                }

                /*
                    The declared field identity of a member whose receiver produces a
                    temporary. `effective_member_symbol` already answers receivers that keep a
                    place position — names, calls, literals, indexed chains — plus any
                    binder-stamped `member_symbol`. A `match` receiver has neither: value
                    dispatch is a control-flow join the member binder never walked, so the
                    identity must come from the arms' declared result types instead, and the
                    same holds for a member receiver whose own receiver bottoms out below the
                    binder's reach — `match { .. }.inner.a` resolves `a` on the declared type
                    of the `inner` field the arm agreement supplied, so deeper chains keep
                    resolving while each hop names a declared field of the previous hop's
                    leaf. An arm whose result type cannot be recovered, leaves that disagree,
                    a case-qualified hop (whose variant disambiguation the leaf alone cannot
                    supply), or a member name no field of the agreed declaration owns all stay
                    unproven. The footprint itself still comes from the receiver's own read
                    scan — this answers only which field the projection selects.
                */
                fn temporary_member_symbol_at(
                    &self,
                    member: &typed_trees::expression::TableMemberExpression,
                    depth: usize,
                ) -> SymbolHandle {
                    let program = self.program;
                    if depth >= ND {
                        return SymbolHandle::invalid();
                    }
                    let direct =
                        validation::effective_member_symbol(program, member.receiver, member);
                    if direct.is_valid() || member.case_variant.is_some() {
                        return direct;
                    }
                    let Some(leaf) = self.temporary_receiver_leaf(member.receiver, depth + 1)
                    else {
                        return direct;
                    };
                    validation::resolve_member_symbol_from_type_symbol(
                        program,
                        leaf,
                        member.member.as_str(),
                    )
                    .unwrap_or_else(SymbolHandle::invalid)
                }

                /*
                    The declaration leaf a temporary-producing receiver's value stands on when
                    the member binder's contextual walk cannot type it. A `match` receiver's
                    leaf is the arm agreement: the checker's own result oracle is what
                    `validate_match_dispatch` consults for arm compatibility, so requiring
                    every arm's recovered leaf to name the same declaration is the
                    member-level statement of that agreement — judged on leaf symbols rather
                    than type handles, because separately authored references to one
                    declaration intern separately. A member receiver whose own receiver is
                    temporary recurses through the hop's declared type: `match { .. }.inner`
                    stands on the leaf of the `inner` field the agreement supplied. Receivers
                    the ordinary walk already types never reach this scan — the caller's
                    `effective_member_symbol` answer came first — and every other kind keeps
                    the leaf unproven.
                */
                fn temporary_receiver_leaf(
                    &self,
                    receiver: ExpressionHandle,
                    depth: usize,
                ) -> Option<SymbolHandle> {
                    let program = self.program;
                    let machine = self.machine;
                    let state = self.state;
                    if depth >= ND || !program.expression_table.expression_is_valid(receiver) {
                        return None;
                    }
                    match program.expression_table.expression(receiver) {
                        ExpressionNode::Match(dispatch) => {
                            let mut leaf = SymbolHandle::invalid();
                            for arm in program.expression_table.match_arms(dispatch.arms) {
                                let symbol = validation::expression_result_type_reference(
                                    program, machine, state, arm.value,
                                )
                                .map(|reference| {
                                    program.type_reference_table.type_symbol(reference)
                                })
                                .filter(|symbol| symbol.is_valid())?;
                                if leaf.is_valid() && leaf != symbol {
                                    return None;
                                }
                                leaf = symbol;
                            }
                            leaf.is_valid().then_some(leaf)
                        }
                        ExpressionNode::Member(inner) => {
                            let field = self.temporary_member_symbol_at(inner, depth + 1);
                            if !field.is_valid() {
                                return None;
                            }
                            validation::symbol_type_symbol(program, field)
                        }
                        _ => None,
                    }
                }

                /*
                    A selected `[]`/`[..]` application is a checked occurrence, not a place:
                    the exact `CheckedOperatorUseFact` at this statement authenticates which
                    declaration the occurrence resolved to, and the operand expressions
                    recovered from that row enumerate the caller storage the callee can
                    observe. Missing, ambiguous, or inconsistent selection custody leaves the
                    read set incomplete — the same evidence floor the checked-call join
                    applies. Each operand recurses through the ordinary read scan, so a
                    nested call or a selected nested indexing still has to prove its own
                    complete footprint.
                */
                pub(super) fn collect_selected_index_reads(
                    &mut self,
                    expression: ExpressionHandle,
                    indexed: &typed_trees::expression::TableIndexedExpression,
                    depth: usize,
                ) -> bool {
                    use language_core::OperatorSpelling;

                    let program = self.program;
                    let spelling = if matches!(
                        program.expression_table.expression(indexed.index),
                        ExpressionNode::Range(_)
                    ) {
                        OperatorSpelling::Range
                    } else {
                        OperatorSpelling::Index
                    };
                    let Some(operands) = self.selected_operator_operands(expression, spelling)
                    else {
                        return false;
                    };
                    operands
                        .iter()
                        .all(|operand| self.collect_reads(*operand, depth + 1))
                }

                /*
                    The operand expressions a selected operator application hands its
                    declaration, authenticated by the exact `CheckedOperatorUseFact` at this
                    statement occurrence. Use rows carry the enclosing statement's origin
                    even for nested operands, so this join covers selector and
                    subexpression positions; custody must agree across every recorded row
                    for the occurrence, resolve to a single valid declaration under the
                    expected spelling, and retain the candidate roster it was selected
                    from. The operand count still has to match the retained signature so a
                    drifted row cannot rename storage the operand scan never saw. Missing,
                    ambiguous, or inconsistent custody yields no operands — the same
                    evidence floor the checked-call join applies.
                */
                pub(super) fn selected_operator_operands(
                    &self,
                    expression: ExpressionHandle,
                    spelling: language_core::OperatorSpelling,
                ) -> Option<Vec<ExpressionHandle>> {
                    let program = self.program;
                    let machine = self.machine;
                    let state = self.state;
                    let statement_index = self.statement_index;
                    let operators = self.operators?;
                    let mut uses = operators.uses.iter().filter_map(|(_, selected)| {
                        (selected.expression == expression
                            && matches!(
                                selected.origin,
                                CheckedValueOrigin::StateStatement {
                                    machine_symbol,
                                    state_symbol,
                                    statement_index: index,
                                    ..
                                } if machine_symbol == machine.symbol
                                    && state_symbol == state.symbol
                                    && index == statement_index
                            ))
                        .then_some(selected)
                    });
                    let selected = uses.next()?;
                    if selected.spelling != spelling
                        || selected.status != CheckedOperatorResolutionStatus::Resolved
                        || !selected.selected_operator_symbol.is_valid()
                        || selected.candidate_count != operators.candidates(selected).len()
                        || uses.any(|other| other != selected)
                    {
                        return None;
                    }
                    let candidate = operators.selected_candidate(selected)?;
                    let operands = selected.operands(program)?;
                    (candidate.parameter_count == operands.len()).then_some(operands)
                }

                /*
                    Check typed identities before contextual spelling recovery, and collect the
                    storage read to select this place. Do not read the whole collection merely
                    to address one element: parent replacement overlaps its child path already.
                */
                pub(super) fn descend_and_capture_symbol_place(
                    &mut self,
                    expression: ExpressionHandle,
                    place: CanonicalPlace,
                    depth: usize,
                ) -> bool {
                    self.collect_selector_reads(expression, depth + 1)
                        && self.capture_place_read(place, expression)
                }

                pub(super) fn collect_selector_reads(
                    &mut self,
                    expression: ExpressionHandle,
                    depth: usize,
                ) -> bool {
                    let program = self.program;
                    let machine = self.machine;
                    let state = self.state;
                    let statement_index = self.statement_index;
                    if depth >= ND || !program.expression_table.expression_is_valid(expression) {
                        return false;
                    }
                    match program.expression_table.expression(expression) {
                        ExpressionNode::Name(path) => {
                            let members = program.expression_table.name_path_members(path.members);
                            let symbols =
                                expressions(program).name_path_member_symbols(path.member_symbols);
                            validation::first_valid_name_path_symbol(
                                path,
                                &program.expression_table,
                            )
                            .is_some()
                                && (members.len() <= 1 || members.len() == symbols.len())
                                && symbols.iter().all(|symbol| symbol.is_valid())
                        }
                        ExpressionNode::Member(member) => {
                            (member.member_symbol.is_valid()
                                || ((member.case_variant.is_some()
                                    || member_receiver_escapes_binder(
                                        program,
                                        member.receiver,
                                        0,
                                    ))
                                    && validation::effective_member_symbol(
                                        program,
                                        member.receiver,
                                        member,
                                    )
                                    .is_valid()))
                                && self.collect_selector_reads(member.receiver, depth + 1)
                        }
                        ExpressionNode::Borrow(inner) => {
                            self.collect_selector_reads(inner.target, depth + 1)
                        }
                        ExpressionNode::Indexed(indexed) => {
                            has_builtin_index_meaning(
                                program,
                                machine,
                                state,
                                statement_index,
                                expression,
                                indexed,
                            ) && self.collect_selector_reads(indexed.collection, depth + 1)
                                && self.collect_operand_reads(indexed.index, depth)
                        }
                        _ => false,
                    }
                }
            }

            /*
                Whether a member's receiver chain escaped the typed member binder, so an
                unset `member_symbol` on the member marks a hop the binder never typed
                rather than a name it saw and refused. The binder descends a member chain
                only while each hop resolves on the previous hop's declared type: a
                case-qualified hop is never stamped, so every member stacked above it —
                and above any earlier unwalked hop, recursively — keeps the contextual
                `effective_member_symbol` floor. An explicit `&`/`&mut` receiver peels
                before the binder's coverage test, so it escapes the same way. A receiver
                the binder did resolve (or whose own refusal was reachable) returns false:
                an unset symbol above it stays a refusal that contextual lookup must not
                repair — an unqualified `subject.f` naming a payload field, for example,
                cannot borrow `f@Case`'s identity merely because the name happens to
                resolve inside one variant.
            */
            pub(super) fn member_receiver_escapes_binder(
                program: &dyn ::validation::CheckingInput,
                receiver: ExpressionHandle,
                depth: usize,
            ) -> bool {
                if depth >= ND || !program.expression_table.expression_is_valid(receiver) {
                    return false;
                }
                match program.expression_table.expression(receiver) {
                    ExpressionNode::Member(inner) => {
                        inner.case_variant.is_some()
                            || (!inner.member_symbol.is_valid()
                                && member_receiver_escapes_binder(
                                    program,
                                    inner.receiver,
                                    depth + 1,
                                ))
                    }
                    ExpressionNode::Borrow(_) => true,
                    _ => false,
                }
            }

            pub(super) fn has_builtin_index_meaning(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                statement_index: usize,
                expression: ExpressionHandle,
                indexed: &typed_trees::expression::TableIndexedExpression,
            ) -> bool {
                use crate::checks::ranges::types::expression_type_reference;
                use language_core::OperatorSpelling;

                let spelling = if indexed.index.is_valid()
                    && matches!(
                        program.expression_table.expression(indexed.index),
                        ExpressionNode::Range(_)
                    ) {
                    OperatorSpelling::Range
                } else {
                    OperatorSpelling::Index
                };
                let operands = validation::indexed_operand_types(
                    program,
                    indexed,
                    CheckedValueOrigin::StateStatement {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        statement_index,
                        role: CheckedValueStatementRole::Expression,
                    },
                );
                let builtin_storage = if spelling == OperatorSpelling::Range {
                    expression_type_reference(program, machine, state, indexed.collection)
                        .and_then(|collection| {
                            validation::unwrapped_type_reference(program, collection)
                        })
                        .is_some_and(|collection| {
                            matches!(
                                program.type_reference_table.type_reference(collection),
                                typed_trees::types::TypeReferenceNode::FixedArray { .. }
                                    | typed_trees::types::TypeReferenceNode::Slice { .. }
                            )
                        })
                } else {
                    expression_type_reference(program, machine, state, expression).is_some()
                };
                let builtin_operator = if spelling == OperatorSpelling::Index {
                    typed_trees::operator::builtin_index_read_is_spelled(
                        program,
                        machine.symbol,
                        expression,
                        &operands,
                    )
                } else {
                    typed_trees::operator::resolve_indexed_spelling_for_operands(
                        program,
                        spelling,
                        &operands,
                        program.expression_table.source_span(expression),
                    )
                    .is_empty()
                        && typed_trees::operator::has_builtin_spelled_expression_meaning(
                            program,
                            machine.symbol,
                            expression,
                            spelling,
                            &operands,
                        )
                };
                builtin_storage && builtin_operator
            }
        }
        mod operands {
            /*
                Operand and place reads: the per-operand dispatch and the captured
                read each place contributes.
            */
            use crate::flow::expressions;
            use crate::published::places::place_at;
            use validation::CanonicalPlace;

            use super::super::{EXPRESSION_WALK_DEPTH_BOUND, ExpressionHandle, ExpressionNode};
            use super::ReadWalk;

            use symbols::SymbolHandle;

            impl ReadWalk<'_, '_, '_> {
                /*
                    A nested operand's reads count when that operand subtree keeps builtin
                    bound meaning — the same floor `record_dependencies` applies to the
                    top-level expression — and its own read scan completes. The general
                    bound-meaning walk does not descend through the compound nodes handled
                    above, so each operand under them has to carry the floor independently.
                    When the floor fails, the operand is walked node by node instead: a
                    builtin arithmetic node keeps recursing into its operands, and an
                    authored arithmetic application is call-shaped, so it admits exactly
                    the operand reads its exact checked operator-use row authenticates
                    (`collect_selected_arithmetic_reads`). A cast or unary wrapper standing
                    above such an application has one immediate runtime child and no
                    overloadable token of its own, so the walk keeps descending through it
                    to whatever the child can prove. Every other non-builtin node — an
                    authored comparison, an unresolved selection — stays incomplete rather
                    than pretending only its visible places were read.
                */
                pub(super) fn collect_operand_reads(
                    &mut self,
                    operand: ExpressionHandle,
                    depth: usize,
                ) -> bool {
                    let program = self.program;
                    let machine = self.machine;
                    let state = self.state;
                    if validation::has_builtin_bound_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        operand,
                    ) {
                        return self.collect_reads(operand, depth + 1);
                    }
                    if depth >= EXPRESSION_WALK_DEPTH_BOUND
                        || !program.expression_table.expression_is_valid(operand)
                    {
                        return false;
                    }
                    let binary = match program.expression_table.expression(operand) {
                        ExpressionNode::Cast(cast) => {
                            return self.collect_operand_reads(cast.value, depth + 1);
                        }
                        ExpressionNode::Unary(unary) => {
                            return self.collect_operand_reads(unary.operand, depth + 1);
                        }
                        ExpressionNode::Binary(binary) => binary,
                        _ => return false,
                    };
                    let Some(spelling) = typed_trees::operator::binary_operator_spelling(
                        binary.operator,
                    )
                    .filter(|spelling| {
                        use language_core::OperatorSpelling;
                        matches!(
                            spelling,
                            OperatorSpelling::Add
                                | OperatorSpelling::Subtract
                                | OperatorSpelling::Multiply
                                | OperatorSpelling::Divide
                                | OperatorSpelling::Modulo
                        )
                    }) else {
                        return false;
                    };
                    if validation::has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        operand,
                    ) {
                        return self.collect_operand_reads(binary.left, depth + 1)
                            && self.collect_operand_reads(binary.right, depth + 1);
                    }
                    self.collect_selected_arithmetic_reads(operand, spelling, depth)
                }

                /*
                    A selected `+`/`-`/`*`/`/`/`%` application is a checked occurrence, not
                    builtin arithmetic: the exact `CheckedOperatorUseFact` at this statement
                    authenticates which declaration governs it, and that declaration can
                    observe only the operands it receives, so its footprint is the operands'
                    own reads — each recursing through the operand gate, so a nested
                    authored application still proves its own custody. A constant-shaped
                    application (`1u64 + 0u64`) is refused even with custody: the place
                    algebra folds such a selector syntactically (`index_place_segment`), so
                    the recorded coordinate would be builtin arithmetic's value rather than
                    whatever the selected declaration produces, and the read set would
                    claim disjointness the declaration never established.
                */
                pub(super) fn collect_selected_arithmetic_reads(
                    &mut self,
                    expression: ExpressionHandle,
                    spelling: language_core::OperatorSpelling,
                    depth: usize,
                ) -> bool {
                    let program = self.program;
                    if expressions(program)
                        .constant_integer_value(expression)
                        .is_some()
                    {
                        return false;
                    }
                    let Some(operands) = self.selected_operator_operands(expression, spelling)
                    else {
                        return false;
                    };
                    operands
                        .iter()
                        .all(|operand| self.collect_operand_reads(*operand, depth + 1))
                }

                /*
                    A read place counts only when its root is current storage and every
                    segment carries resolved selector geometry. Normalization runs before the
                    current-root check so attached fields answer through the machine root.
                    The returned symbol is the pre-normalization root for callers that
                    distinguish a place's own root from a rebased copy identity.
                */
                pub(super) fn validate_place_read(
                    &self,
                    place: &mut CanonicalPlace,
                ) -> Option<SymbolHandle> {
                    let root = place.root.as_symbol()?;
                    if !root.is_valid()
                        || place.segments.iter().any(|segment| {
                            crate::flow::place_segment_has_unresolved_identity(*segment)
                        })
                    {
                        return None;
                    }

                    (self.root_is_current(place.root)
                        && place.segments.iter().all(|segment| {
                            matches!(
                                segment,
                                facts::PlaceSegment::Field { .. }
                                    | facts::PlaceSegment::Case { .. }
                                    | facts::PlaceSegment::FixedIndex { .. }
                                    | facts::PlaceSegment::FixedRange { .. }
                                    | facts::PlaceSegment::Index { .. }
                            )
                        }))
                    .then_some(root)
                }

                /*
                    A name, member, or builtin-indexed expression contributes its own storage
                    place plus the selector reads needed to address it.
                */
                pub(super) fn collect_place_read(
                    &mut self,
                    expression: ExpressionHandle,
                    depth: usize,
                ) -> bool {
                    let program = self.program;
                    let state = self.state;
                    let statement_index = self.statement_index;
                    if !self.collect_selector_reads(expression, depth + 1) {
                        return false;
                    }
                    let Some(place) = place_at(program, state.symbol, statement_index, expression)
                    else {
                        return false;
                    };
                    self.capture_place_read(place, expression)
                }

                /*
                    The tail of a place read once the canonical place is computed:
                    validation, the frozen-integer copy rewrite, and deduped capture.
                    `collect_member_reads` and the selector-chain split call this with the
                    place they already resolved for their root-kind gate, so the read is
                    canonicalized once instead of twice.
                */
                pub(super) fn capture_place_read(
                    &mut self,
                    mut place: CanonicalPlace,
                    expression: ExpressionHandle,
                ) -> bool {
                    let program = self.program;
                    let machine = self.machine;
                    let state = self.state;
                    let Some(root) = self.validate_place_read(&mut place) else {
                        return false;
                    };
                    if place.segments.is_empty()
                        && let ExpressionNode::Name(path) =
                            program.expression_table.expression(expression)
                        && path.symbol == root
                        && path.head_symbol == root
                        && super::super::captures::is_integer_value(
                            program, machine, state, expression,
                        )
                        && let Some(value) = super::super::captures::integer_value_identity(
                            program,
                            self.bound_lookup,
                            state,
                            expression,
                        )
                        && self.root_is_current(facts::PlaceRoot::Symbol(value))
                    {
                        place.root = facts::PlaceRoot::Symbol(value);
                    }
                    crate::shared::collections::push_unique(self.reads, place);
                    true
                }
            }
        }
        mod root_currency {
            /*
                A read's root is current only in its exact authored state prefix.

                Parameters and preceding local declarations already belong to typed spans.
                Reading those relationships needs no thread scope, copied symbol roster or
                program-identity invalidation. Duplicate local declarations preserve prefix
                membership here; the separate declaration-resolution query requires uniqueness.
            */
            use super::super::{Machine, State};

            pub(in crate::checks::ranges::facts::dependencies) fn root_is_current(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                statement_index: usize,
                root: facts::PlaceRoot,
            ) -> bool {
                let Some(symbol) = root.as_symbol() else {
                    return false;
                };
                symbol == machine.symbol
                    || program.state_parameter_by_symbol(state, symbol).is_some()
                    || crate::flow::state_statements(program, state.statement_nodes)
                        .iter()
                        .take(statement_index)
                        .any(|statement| {
                            matches!(statement,
                            typed_trees::statement::StatementNode::LocalData(local) if local.symbol == symbol)
                        })
            }
        }

        use atomics::static_application_carries_no_caller_storage;
        use members::has_builtin_index_meaning;
        pub(super) use root_currency::root_is_current;

        /*
            The range-read walk context: the program, the machine/state pair under
            evaluation, the statement occurrence, its checked call and operator
            custody, the read set the walk fills, and the bound index. Every recursive
            read step used to restate this tuple (up to eleven arguments) at each call
            site; the walk now carries it once, and each step names only the node it
            descends into and the depth.
        */
        pub(super) struct ReadWalk<'a, 'b, 'c> {
            program: &'a dyn ::validation::CheckingInput,
            machine: &'a Machine,
            state: &'a State,
            statement_index: usize,
            calls: Option<&'c RangeCallContext<'c>>,
            operators: Option<&'c CheckedOperatorFacts>,
            reads: &'c mut Vec<CanonicalPlace>,
            bound_lookup: &'b ImmutableBoundLookup<'b>,
        }

        impl<'a, 'b, 'c> ReadWalk<'a, 'b, 'c> {
            pub(super) fn new(
                program: &'a dyn ::validation::CheckingInput,
                machine: &'a Machine,
                state: &'a State,
                statement_index: usize,
                calls: Option<&'c RangeCallContext<'c>>,
                operators: Option<&'c CheckedOperatorFacts>,
                reads: &'c mut Vec<CanonicalPlace>,
                bound_lookup: &'b ImmutableBoundLookup<'b>,
            ) -> Self {
                Self {
                    program,
                    machine,
                    state,
                    statement_index,
                    calls,
                    operators,
                    reads,
                    bound_lookup,
                }
            }

            /*
                Whether a read's root is current in the exact authored state prefix —
                the shared form of the prefix scan `capture_place_read` and the
                operand capture both consult.
            */
            pub(super) fn root_is_current(&self, root: PlaceRoot) -> bool {
                root_currency::root_is_current(
                    self.program,
                    self.machine,
                    self.state,
                    self.statement_index,
                    root,
                )
            }
        }

        impl ReadWalk<'_, '_, '_> {
            pub(super) fn collect_reads(
                &mut self,
                expression: ExpressionHandle,
                depth: usize,
            ) -> bool {
                let program = self.program;
                let machine = self.machine;
                let state = self.state;
                let statement_index = self.statement_index;
                if depth >= EXPRESSION_WALK_DEPTH_BOUND
                    || !program.expression_table.expression_is_valid(expression)
                {
                    return false;
                }
                match program.expression_table.expression(expression) {
                    // A formed closed code selection reads no caller storage.
                    ExpressionNode::MachineValue(_) => true,
                    ExpressionNode::SavedSubject(_) => false,
                    ExpressionNode::Integer(_)
                    | ExpressionNode::Boolean(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::UndeterminedNaN(_)
                    | ExpressionNode::String(_)
                    | ExpressionNode::TypeExpression(_)
                    | ExpressionNode::ZeroValue(_) => true,
                    ExpressionNode::Binary(binary) => {
                        self.collect_reads(binary.left, depth + 1)
                            && self.collect_reads(binary.right, depth + 1)
                    }
                    ExpressionNode::Unary(unary) => self.collect_reads(unary.operand, depth + 1),
                    ExpressionNode::Cast(cast) => self.collect_reads(cast.value, depth + 1),
                    ExpressionNode::Borrow(inner) => self.collect_reads(inner.target, depth + 1),
                    ExpressionNode::Call(call) => {
                        let Some(calls) = self.calls else {
                            return false;
                        };
                        let site = CallSite::Expression { expression, call };
                        let Some(borrow_call) =
                            calls.find_call(program, machine, state, statement_index, &site)
                        else {
                            return false;
                        };
                        let runtime =
                            match validation::CallContractSource::runtime_expression(program, call)
                            {
                                Ok(Some(validation::CallContractSource::Runtime {
                                    binding,
                                    ..
                                })) => {
                                    if borrow_call.runtime_call != Some(binding) {
                                        return false;
                                    }
                                    true
                                }
                                Ok(Some(_)) => return false,
                                Ok(None) => {
                                    if borrow_call.runtime_call.is_some()
                                        || call.static_machine_parameter.is_valid()
                                        || !call.selects_only_nominal_route()
                                        || !static_application_carries_no_caller_storage(
                                            program, call,
                                        )
                                    {
                                        return false;
                                    }
                                    false
                                }
                                Err(_) => return false,
                            };
                        let callee = call.evaluated_callee();
                        if callee.is_valid() && !self.collect_reads(callee, depth + 1) {
                            return false;
                        }
                        for argument in program.expression_table.expression_handles(call.arguments)
                        {
                            if !self.collect_reads(*argument, depth + 1) {
                                return false;
                            }
                        }
                        for mut place in calls.operand_access_places(borrow_call) {
                            if self.validate_place_read(&mut place).is_none() {
                                return false;
                            }
                            push_unique(self.reads, place);
                        }
                        if runtime {
                            return true;
                        }
                        let Some((target_machine_symbol, target_state_symbol)) =
                            contract_target_from_state_symbol(program, borrow_call.target_symbol)
                        else {
                            return false;
                        };
                        let Some(_target_state) = find_state_in_machine(
                            program,
                            target_machine_symbol,
                            target_state_symbol,
                        ) else {
                            return false;
                        };
                        true
                    }
                    ExpressionNode::Name(_) => self.collect_place_read(expression, depth),
                    ExpressionNode::Member(member) => {
                        self.collect_member_reads(expression, member, depth)
                    }
                    ExpressionNode::Indexed(indexed) => {
                        if has_builtin_index_meaning(
                            program,
                            machine,
                            state,
                            statement_index,
                            expression,
                            indexed,
                        ) {
                            if let Some(place) =
                                place_at(program, state.symbol, statement_index, expression)
                                && matches!(place.root, PlaceRoot::Symbol(_))
                            {
                                self.descend_and_capture_symbol_place(expression, place, depth)
                            } else {
                                self.collect_reads(indexed.collection, depth + 1)
                                    && self.collect_operand_reads(indexed.index, depth)
                            }
                        } else {
                            self.collect_selected_index_reads(expression, indexed, depth)
                        }
                    }
                    ExpressionNode::Range(range) => {
                        (!range.start.is_valid() || self.collect_operand_reads(range.start, depth))
                            && (!range.end.is_valid()
                                || self.collect_operand_reads(range.end, depth))
                    }
                    ExpressionNode::ArrayLiteral(elements) => expressions(program)
                        .expression_handles(*elements)
                        .iter()
                        .all(|element| self.collect_operand_reads(*element, depth)),
                    ExpressionNode::StructLiteral(literal) => expressions(program)
                        .struct_fields(literal.fields)
                        .iter()
                        .all(|field| self.collect_operand_reads(field.value, depth)),
                    ExpressionNode::Match(dispatch) => {
                        self.collect_operand_reads(dispatch.subject, depth)
                            && program.expression_table.match_arms_all(
                                dispatch.arms,
                                |arm_pattern, arm_value| {
                                    (match arm_pattern {
                                        MatchPattern::Destructure(_) => expressions(program)
                                            .match_pattern_operands(arm_pattern)
                                            .all(|operand| {
                                                self.collect_operand_reads(operand, depth)
                                            }),
                                        MatchPattern::Value(pattern) => {
                                            self.collect_operand_reads(pattern, depth)
                                        }
                                        MatchPattern::Wildcard => true,
                                    }) && self.collect_operand_reads(arm_value, depth)
                                },
                            )
                    }
                    ExpressionNode::Atomic(atomic) => {
                        let footprint_start = self.reads.len();
                        match atomic.ordering {
                            language_core::atomic::AtomicOrderingPlan::Load(_) => {
                                self.collect_reads(atomic.value, depth + 1)
                                    && self.reads.len() > footprint_start
                            }
                            _ => self.collect_atomic_write_reads(expression, atomic, depth),
                        }
                    }
                }
            }
        }
    }
    mod state_transport {
        /*
            Translate an incoming observation through its admitted state bindings.

            A projected read owns both a storage root and expressions selecting within
            that storage. Rebinding only the root leaves a mixed-state place which cannot
            agree with the destination's own contract. Selectors rejoin existing typed
            expressions under the same simultaneous parameter map; no expression is
            evaluated, synthesized, or equated by spelling. The incoming-edge owner has
            already established the bindings, and the read collector still owns complete
            dependencies and subsequent write invalidation.
        */
        use super::{EXPRESSION_WALK_DEPTH_BOUND, ExpressionDependencyScope};
        use crate::checks::ranges::types::is_const_binder;
        use crate::flow::canonical_place_segments_equal;
        use crate::flow::expressions;
        use crate::published::places::place_at;
        use facts::{PlaceRoot, PlaceSegment};
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::type_identity::TypeIdentityRequest;
        use typed_trees::types::TypeReferenceHandle;
        use validation::{
            CanonicalPlace, CheckingInput, collection_length_receiver,
            expression_result_type_reference, has_builtin_binary_expression_meaning,
            place_has_builtin_coordinates,
        };

        pub(super) fn transport_reads(
            program: &dyn CheckingInput,
            machine: &Machine,
            scope: &ExpressionDependencyScope<'_>,
            statement_index: usize,
            reads: &mut [CanonicalPlace],
        ) -> bool {
            let transport = StateTransport {
                program,
                machine,
                scope,
                statement_index,
            };
            for read in reads {
                let Some(root) = transport.root(read.root) else {
                    return false;
                };
                read.root = root;
                for segment in &mut read.segments {
                    let PlaceSegment::Index { expression } = segment else {
                        continue;
                    };
                    let Some((destination, _)) =
                        expressions(program)
                            .iter_expressions()
                            .find(|(destination, _)| {
                                transport.expressions_correspond(*expression, *destination, 0)
                            })
                    else {
                        return false;
                    };
                    *expression = destination;
                }
            }
            true
        }

        struct StateTransport<'context, 'scope> {
            program: &'context dyn CheckingInput,
            machine: &'context Machine,
            scope: &'context ExpressionDependencyScope<'scope>,
            statement_index: usize,
        }

        impl StateTransport<'_, '_> {
            fn same_computed_type(
                &self,
                source: ExpressionHandle,
                destination: ExpressionHandle,
            ) -> bool {
                let result = |state, expression| {
                    expression_result_type_reference(self.program, self.machine, state, expression)
                        .map(|reference| {
                            self.program
                                .type_identity(TypeIdentityRequest::ordinary(reference))
                        })
                };
                let Some(source) = result(self.scope.evaluation, source) else {
                    return false;
                };
                result(self.scope.destination, destination) == Some(source)
            }

            fn root(&self, source: PlaceRoot) -> Option<PlaceRoot> {
                let symbol = source.as_symbol()?;
                if symbol == self.machine.symbol
                    || is_const_binder(self.program, self.machine, symbol)
                {
                    return Some(source);
                }
                self.scope
                    .parameter_roots
                    .iter()
                    .find(|(parameter, _)| *parameter == symbol)
                    .map(|(_, destination)| PlaceRoot::Symbol(*destination))
            }

            fn places_correspond(
                &self,
                source: &CanonicalPlace,
                destination: &CanonicalPlace,
                depth: usize,
            ) -> bool {
                self.root(source.root) == Some(destination.root)
                    && source.segments.len() == destination.segments.len()
                    && source.segments.iter().zip(&destination.segments).all(
                        |(source, destination)| match (source, destination) {
                            (
                                PlaceSegment::Index { expression: source },
                                PlaceSegment::Index {
                                    expression: destination,
                                },
                            ) => self.expressions_correspond(*source, *destination, depth + 1),
                            _ => {
                                canonical_place_segments_equal(self.program, *source, *destination)
                            }
                        },
                    )
            }

            fn expressions_correspond(
                &self,
                source: ExpressionHandle,
                destination: ExpressionHandle,
                depth: usize,
            ) -> bool {
                let program = self.program;
                if depth >= EXPRESSION_WALK_DEPTH_BOUND
                    || !program.expression_table.expression_is_valid(source)
                    || !program.expression_table.expression_is_valid(destination)
                {
                    return false;
                }
                let children = |source, destination| {
                    self.expressions_correspond(source, destination, depth + 1)
                };
                match (
                    program.expression_table.expression(source),
                    program.expression_table.expression(destination),
                ) {
                    (ExpressionNode::Name(_), ExpressionNode::Name(_))
                    | (ExpressionNode::Member(_), ExpressionNode::Member(_))
                    | (ExpressionNode::Indexed(_), ExpressionNode::Indexed(_)) => {
                        let source_length = collection_length_receiver(
                            program,
                            self.machine,
                            Some(self.scope.evaluation),
                            source,
                        );
                        let destination_length = collection_length_receiver(
                            program,
                            self.machine,
                            Some(self.scope.destination),
                            destination,
                        );
                        match (source_length, destination_length) {
                            (Some(source), Some(destination)) => {
                                return children(source, destination);
                            }
                            (None, None) => {}
                            _ => return false,
                        }
                        let place = |state: &State, expression| {
                            place_has_builtin_coordinates(
                                program,
                                self.machine,
                                Some(state),
                                expression,
                            )
                            .then(|| {
                                place_at(program, state.symbol, self.statement_index, expression)
                            })
                            .flatten()
                        };
                        match (
                            place(self.scope.evaluation, source),
                            place(self.scope.destination, destination),
                        ) {
                            (Some(source), Some(destination)) => {
                                self.places_correspond(&source, &destination, depth)
                            }
                            _ => false,
                        }
                    }
                    (
                        ExpressionNode::Binary(source_node),
                        ExpressionNode::Binary(destination_node),
                    ) => {
                        source_node.operator == destination_node.operator
                            && self.same_computed_type(source, destination)
                            && children(source_node.left, destination_node.left)
                            && children(source_node.right, destination_node.right)
                            && has_builtin_binary_expression_meaning(
                                program,
                                self.machine,
                                Some(self.scope.evaluation),
                                source,
                            )
                            && has_builtin_binary_expression_meaning(
                                program,
                                self.machine,
                                Some(self.scope.destination),
                                destination,
                            )
                    }
                    (
                        ExpressionNode::Unary(source_node),
                        ExpressionNode::Unary(destination_node),
                    ) => {
                        source_node.operator == destination_node.operator
                            && self.same_computed_type(source, destination)
                            && children(source_node.operand, destination_node.operand)
                    }
                    (ExpressionNode::Borrow(source), ExpressionNode::Borrow(destination)) => {
                        source.access == destination.access
                            && children(source.target, destination.target)
                    }
                    (ExpressionNode::Cast(source), ExpressionNode::Cast(destination)) => {
                        let same_type =
                            |source: TypeReferenceHandle, destination: TypeReferenceHandle| {
                                source.is_valid()
                                    && destination.is_valid()
                                    && program.type_identity(TypeIdentityRequest::ordinary(source))
                                        == program.type_identity(TypeIdentityRequest::ordinary(
                                            destination,
                                        ))
                            };
                        same_type(source.target_type, destination.target_type)
                            && same_type(source.result_type, destination.result_type)
                            && source.domain == destination.domain
                            && source.form == destination.form
                            && source.semantic_domain_id == destination.semantic_domain_id
                            && children(source.value, destination.value)
                    }
                    (ExpressionNode::Range(source), ExpressionNode::Range(destination)) => {
                        let endpoints =
                            |source: ExpressionHandle, destination: ExpressionHandle| {
                                if source.is_valid() || destination.is_valid() {
                                    children(source, destination)
                                } else {
                                    true
                                }
                            };
                        source.end_inclusive == destination.end_inclusive
                            && endpoints(source.start, destination.start)
                            && endpoints(source.end, destination.end)
                    }
                    (ExpressionNode::Integer(source), ExpressionNode::Integer(destination)) => {
                        source == destination
                    }
                    (ExpressionNode::Boolean(source), ExpressionNode::Boolean(destination)) => {
                        source == destination
                    }
                    (ExpressionNode::Float(source), ExpressionNode::Float(destination)) => {
                        source == destination
                    }
                    (ExpressionNode::String(source), ExpressionNode::String(destination)) => {
                        source == destination
                    }
                    _ => false,
                }
            }
        }
    }

    use reads::ReadWalk;

    use std::collections::{BTreeMap, BTreeSet};

    use super::RangeFacts;

    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::{machine::Machine, state::State};

    /*
        Bound shared by every walk below: `record_dependencies` and
        `collect_reads` recurse over expression trees, while the captures.rs
        chain walks unroll the same bound iteratively. Authored expressions
        deeper than the bound record the visited prefix and stop rather than
        recursing to the bottom of the arena; pinned by tests/depth.rs.
    */
    pub(super) const EXPRESSION_WALK_DEPTH_BOUND: usize = 128;

    #[derive(Clone)]
    pub(super) struct ExpressionDependencies {
        expression: ExpressionHandle,
        label: String,
        machine: SymbolHandle,
        state: SymbolHandle,
        /* None is an incomplete read set, not a storage-free computation. */
        reads: Option<Vec<CanonicalPlace>>,
        /*
            Metadata observes descriptor replacement, not writes to its elements.
            Selector reads remain ordinary value dependencies.
        */
        descriptor: Option<CanonicalPlace>,
    }

    /*
        A captured scalar reads its own storage, independently of its initializer.
        This is declaration identity for existing facts, not a replayable value.
    */
    #[derive(Clone)]
    pub(super) struct ScalarBindingDependency {
        label: String,
        machine: SymbolHandle,
        state: SymbolHandle,
        place: CanonicalPlace,
        length: Option<CapturedLength>,
        value: Option<CapturedScalarValue>,
    }

    /*
        Equality with a currently unchanged integer observation, captured when the
        binding is evaluated. Any write to its inputs or destination retires it.
    */
    #[derive(Clone)]
    struct CapturedScalarValue {
        expression: ExpressionHandle,
        reads: Vec<CanonicalPlace>,
    }

    /*
        A scalar snapshot may still measure a live descriptor. Keep that relation
        at evaluation, not by expanding its initializer when a later guard runs.
        Descriptor replacement or selector writes retire the relation without
        changing the captured scalar's own value facts; element writes do not.
    */
    #[derive(Clone)]
    pub(super) struct CapturedLength {
        collection: ExpressionHandle,
        descriptor: CanonicalPlace,
        selectors: Vec<CanonicalPlace>,
    }

    /*
        One row's typed meaning for label-collision judgment: machine, state and
        read set, `None` when the read set is incomplete.
    */
    type RowMeaning<'table> = (
        SymbolHandle,
        SymbolHandle,
        Option<&'table [CanonicalPlace]>,
        Option<&'table CanonicalPlace>,
    );

    /* Every row of one display label, both tables. */
    #[derive(Default)]
    struct LabelRows<'table> {
        expressions: Vec<&'table ExpressionDependencies>,
        bindings: Vec<&'table ScalarBindingDependency>,
    }

    /*
        Labels whose expression rows, or whose scalar-binding rows, carry no
        single typed meaning; see `RangeFacts::label_collisions`.
    */
    #[derive(Default)]
    struct LabelCollisions<'table> {
        expression_labels: BTreeSet<&'table str>,
        binding_labels: BTreeSet<&'table str>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(in super::super) struct ReceiverLength {
        expression: ExpressionHandle,
        label: String,
        place: CanonicalPlace,
        length: i64,
    }

    impl ReceiverLength {
        /*
            One exact-length row re-keyed onto a surviving destination place, as
            `storage_sub_places` renders argument storage.
        */
        pub(in super::super) fn at_place(
            expression: ExpressionHandle,
            place: CanonicalPlace,
            label: String,
            length: i64,
        ) -> Self {
            Self {
                expression,
                label,
                place,
                length,
            }
        }

        pub(in super::super) fn same_extent(&self, other: &Self) -> bool {
            self.place == other.place && self.length == other.length
        }

        /*
            Both rows seed the same destination facts: the same extent under the
            same label. Only the observing expression differs, and the seeded
            dependency row reads the place, not that expression.
        */
        pub(in super::super) fn same_seed(&self, other: &Self) -> bool {
            self.same_extent(other) && self.label == other.label
        }
    }

    struct ExpressionDependencyScope<'scope> {
        evaluation: &'scope State,
        destination: &'scope State,
        parameter_roots: &'scope [(SymbolHandle, SymbolHandle)],
    }

    impl<'field> RangeFacts<'field> {
        /*
            Walk one expression's exact typed reads under a statement bound, through
            this pass's checked calls and checked operators, and report whether the
            walk completed. The bound is the only part a capture site chooses — the
            current statement, a frozen selector prefix, or the whole state — and an
            incomplete walk keeps its walked prefix in `reads`. Every capture site
            built the walker itself and restated the same eight arguments; the read
            set a capture records is the walk's, not the site's, so one owner keeps
            the walker's operator and call context from drifting per site.
        */
        pub(super) fn walk_reads(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            statement_bound: usize,
            expression: ExpressionHandle,
            reads: &mut Vec<CanonicalPlace>,
        ) -> bool {
            ReadWalk::new(
                program,
                machine,
                state,
                statement_bound,
                self.checked_calls,
                self.checked_operators,
                reads,
                self.bound_lookup(),
            )
            .collect_reads(expression, 0)
        }

        pub(in super::super) fn forget_collection_expression(&mut self, label: &str) {
            self.forget_collection_facts(label);
            self.expression_dependencies
                .retain(|row| row.label != label);
            self.recorded_expressions = self
                .expression_dependencies
                .iter()
                .map(|row| (row.expression, row.machine, row.state))
                .collect();
        }

        pub(in super::super) fn expression_exact_length(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
        ) -> Option<i64> {
            if self.exact_lengths.is_empty() {
                return None;
            }
            let mut reads = Vec::new();
            let complete = self.walk_reads(
                program,
                machine,
                state,
                self.statement_index,
                expression,
                &mut reads,
            );
            if !complete || reads.len() != 1 {
                return None;
            }
            reads[0] = flow::rebase_exact_local_place(
                program,
                state.symbol,
                self.statement_index,
                reads[0].clone(),
                self.checked_calls.and_then(|context| context.call_frames()),
            )?;
            let mut length = None;
            for row in &self.expression_dependencies {
                if row.machine != machine.symbol || row.state != state.symbol {
                    continue;
                }
                let Some(candidate) = self.exact_length(&row.label) else {
                    continue;
                };
                let row_reads = row.reads.as_ref().and_then(|row_reads| {
                    let [place] = row_reads.as_slice() else {
                        return None;
                    };
                    flow::rebase_exact_local_place(
                        program,
                        state.symbol,
                        self.statement_index,
                        place.clone(),
                        self.checked_calls.and_then(|context| context.call_frames()),
                    )
                    .map(|place| vec![place])
                });
                if row.machine == machine.symbol
                    && row.state == state.symbol
                    && same_reads(program, row_reads.as_deref(), Some(&reads))
                {
                    if length.is_some_and(|known| known != candidate) {
                        return None;
                    }
                    length = Some(candidate);
                }
            }
            length
        }

        pub(in super::super) fn receiver_lengths(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
        ) -> Vec<ReceiverLength> {
            if self.exact_lengths.is_empty() || self.expression_dependencies.is_empty() {
                return Vec::new();
            }
            self.expression_dependencies
                .iter()
                .filter_map(|row| {
                    if row.machine != machine.symbol || row.state != state.symbol {
                        return None;
                    }
                    let [place] = row.reads.as_deref()? else {
                        return None;
                    };
                    let length = self.exact_length(&row.label)?;
                    let place = flow::rebase_exact_local_place(
                        program,
                        state.symbol,
                        self.statement_index,
                        place.clone(),
                        self.checked_calls.and_then(|context| context.call_frames()),
                    )?;
                    ((place.root) == PlaceRoot::Symbol(machine.symbol)
                        && !place.segments.is_empty()
                        && place
                            .segments
                            .iter()
                            .all(|segment| matches!(segment, PlaceSegment::Field { .. })))
                    .then(|| ReceiverLength {
                        expression: row.expression,
                        label: row.label.clone(),
                        place: CanonicalPlace {
                            root: PlaceRoot::Symbol(machine.symbol),
                            segments: place.segments,
                        },
                        length,
                    })
                })
                .collect()
        }

        /*
            The expression-dependency rows whose single read place sits under one
            of `storages`, re-keyed to that storage's destination place.

            `storages` pairs a caller-side canonical place with the destination
            root the edge binds it to: `(machine root + prefix, machine symbol)`
            for attached `self` storage (a no-op re-root; `self.line` stays
            `self.line`), or `(argument place, parameter symbol)` for a `&mut`
            transition argument (`buf.line` arrives as `param.line`). Each
            element is `(source_label, source_expression, destination_place)`;
            the caller re-spells the destination label from the place.
        */
        pub(in super::super) fn storage_sub_places(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            storages: &[(CanonicalPlace, SymbolHandle)],
        ) -> Vec<(String, ExpressionHandle, CanonicalPlace)> {
            if storages.is_empty()
                || self.expression_dependencies.is_empty()
                || (self.exact_lengths.is_empty()
                    && self.proven_index_upper_bounds.is_empty()
                    && self.proven_non_negatives.is_empty()
                    && self.minimum_lengths.is_empty()
                    && self.proven_indexes.is_empty())
            {
                return Vec::new();
            }
            self.expression_dependencies
                .iter()
                .filter(|row| row.machine == machine.symbol && row.state == state.symbol)
                .filter(|row| row.descriptor.is_none())
                .filter(|row| {
                    self.exact_length(&row.label).is_some()
                        || self.proven_index_upper_bound(&row.label).is_some()
                        || self.non_negative_is_proven(&row.label)
                        || self.minimum_length(&row.label).is_some()
                        || self.proven_indexes.iter().any(|(collection, index)| {
                            collection == &row.label || index == &row.label
                        })
                })
                .filter_map(|row| {
                    let [place] = row.reads.as_deref()? else {
                        return None;
                    };
                    let place = flow::rebase_exact_local_place(
                        program,
                        state.symbol,
                        self.statement_index,
                        place.clone(),
                        self.checked_calls.and_then(|context| context.call_frames()),
                    )?;

                    storages.iter().find_map(|(storage, destination)| {
                        if place.root != storage.root
                            || !place.segments.starts_with(&storage.segments)
                        {
                            return None;
                        }
                        let suffix = &place.segments[storage.segments.len()..];
                        if !suffix.iter().all(flow::fixed_selection_segment) {
                            return None;
                        }
                        Some((
                            row.label.clone(),
                            row.expression,
                            CanonicalPlace {
                                root: PlaceRoot::Symbol(*destination),
                                segments: suffix.to_vec(),
                            },
                        ))
                    })
                })
                .collect()
        }

        pub(in super::super) fn seed_receiver_lengths(
            &mut self,
            machine: SymbolHandle,
            state: SymbolHandle,
            lengths: &[ReceiverLength],
        ) {
            for length in lengths {
                self.prove_exact_length(length.label.clone(), length.length);
                self.seed_storage_dependency(
                    machine,
                    state,
                    length.expression,
                    &length.label,
                    &length.place,
                );
            }
        }

        /*
            Rebind a transported fact's read dependency to destination storage.
            The original expression identifies the observation, not a source-state
            expression to reevaluate. Mutation and later edges use the exact place.
        */
        pub(in super::super) fn seed_storage_dependency(
            &mut self,
            machine: SymbolHandle,
            state: SymbolHandle,
            expression: ExpressionHandle,
            label: &str,
            place: &CanonicalPlace,
        ) {
            self.recorded_expressions
                .insert((expression, machine, state));
            self.expression_dependencies.push(ExpressionDependencies {
                expression,
                label: label.to_owned(),
                machine,
                state,
                reads: Some(vec![place.clone()]),
                descriptor: None,
            });
        }

        pub(in super::super) fn record_scalar_binding_dependency(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            source: ExpressionHandle,
            symbol: SymbolHandle,
            label: &str,
        ) {
            use typed_trees::statement::StatementNode;
            let reference = match program
                .statement_table
                .statement_at(state, self.statement_index)
            {
                Some(StatementNode::LocalData(local))
                    if local.symbol == symbol
                        && local.initial_value == source
                        && local.name.as_str() == label =>
                {
                    Some(local.type_reference)
                }
                Some(StatementNode::Assignment(assignment))
                    if assignment.value == source
                        && single_segment_name_symbol(program, assignment.target)
                            == Some(symbol) =>
                {
                    expression_type_reference(program, machine, state, assignment.target)
                }
                _ => None,
            };
            if !symbol.is_valid()
                || program.symbols.get(symbol).kind != SymbolKind::Local
                || program.symbols.get(symbol).parent != state.symbol
                || !reference
                    .and_then(|reference| program.primitive_type_reference(reference))
                    .is_some_and(|primitive| primitive.is_integer())
            {
                return;
            }
            let Some(place) = flow::canonical_place_from_symbol(symbol) else {
                return;
            };
            let length = self.capture_length(program, machine, state, source);
            let mut reads = Vec::new();
            let value = (captures::has_scalar_snapshot_meaning(program, machine, state, source, 0)
                && self.walk_reads(
                    program,
                    machine,
                    state,
                    self.statement_index,
                    source,
                    &mut reads,
                )
                && !reads.iter().any(|read| {
                    ::validation::storage_path_relations::canonical_places_may_overlap(
                        program, read, &place,
                    )
                }))
            .then_some(CapturedScalarValue {
                expression: source,
                reads,
            });
            if let Some(row) = self.scalar_binding_dependencies.iter_mut().find(|row| {
                row.machine == machine.symbol
                    && row.state == state.symbol
                    && row.label == label
                    && row.place == place
            }) {
                row.length = length;
                row.value = value;
                return;
            }
            self.scalar_binding_dependencies
                .push(ScalarBindingDependency {
                    label: label.to_owned(),
                    machine: machine.symbol,
                    state: state.symbol,
                    place,
                    length,
                    value,
                });
        }

        /*
            All length comparisons consume the same live measurement, whether
            authored directly or retained by a scalar binding/copy. Symbols select
            snapshots; expression labels only address the existing fact tables.
        */
        pub(in super::super) fn length_collection(
            &self,
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            if let Some(member) = length_member(program, expression) {
                return Some(member.receiver);
            }
            if let Some((_, length)) = self
                .measured_results
                .iter()
                .find(|(result, _)| *result == expression)
            {
                return Some(length.collection);
            }
            self.captured_length_collection(program, expression)
        }

        /*
            A retained measurement names its descriptor only while the scalar,
            descriptor and captured selectors have not been replaced. This is the
            same snapshot edge consumed by guards, not a replay of an initializer.
        */
        pub(in super::super) fn captured_length_collection(
            &self,
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            let symbol = single_segment_name_symbol(program, expression)?;
            self.scalar_binding_dependencies.iter().find_map(|row| {
                (row.place.root == PlaceRoot::Symbol(symbol))
                    .then(|| row.length.as_ref().map(|length| length.collection))
                    .flatten()
            })
        }

        fn capture_length(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            source: ExpressionHandle,
        ) -> Option<CapturedLength> {
            if let Some(symbol) = single_segment_name_symbol(program, source) {
                return self.scalar_binding_dependencies.iter().find_map(|row| {
                    (row.machine == machine.symbol
                        && row.state == state.symbol
                        && row.place.root == PlaceRoot::Symbol(symbol))
                    .then(|| row.length.clone())
                    .flatten()
                });
            }
            if let Some((_, length)) = self
                .measured_results
                .iter()
                .find(|(result, _)| *result == source)
            {
                return Some(length.clone());
            }
            let collection = length_member(program, source)?.receiver;
            self.capture_collection_length(program, machine, state, collection)
        }

        fn capture_collection_length(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            collection: ExpressionHandle,
        ) -> Option<CapturedLength> {
            let reference = expression_type_reference(program, machine, state, collection)?;
            let reference = unwrapped_type_reference(program, reference)?;
            if !matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. }
            ) {
                return None;
            }
            let descriptor = place_at(program, state.symbol, self.statement_index, collection)?;
            let mut reads = Vec::new();
            let complete = self.walk_reads(
                program,
                machine,
                state,
                self.statement_index,
                collection,
                &mut reads,
            );
            if !complete
                || !reads
                    .iter()
                    .any(|read| flow::canonical_places_agree(program, read, &descriptor))
            {
                return None;
            }
            reads.retain(|read| !flow::canonical_places_agree(program, read, &descriptor));
            Some(CapturedLength {
                collection,
                descriptor,
                selectors: reads,
            })
        }

        pub(super) fn retire_captured_observations(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            writes: Option<&[CanonicalPlace]>,
        ) {
            if self.measured_results.is_empty() && self.scalar_binding_dependencies.is_empty() {
                return;
            }
            self.measured_results.retain(|(_, length)| {
                !writes.is_none_or(|writes| {
                    writes.iter().any(|write| {
                        write_covers_place(program, write, &length.descriptor)
                            || length.selectors.iter().any(|read| {
                                ::validation::storage_path_relations::canonical_places_may_overlap(
                                    program, read, write,
                                )
                            })
                    })
                })
            });
            for row in &mut self.scalar_binding_dependencies {
                if row.value.as_ref().is_some_and(|value| {
                    writes.is_none_or(|writes| {
                        writes.iter().any(|write| {
                            ::validation::storage_path_relations::canonical_places_may_overlap(
                                program, &row.place, write,
                            ) || value.reads.iter().any(|read| {
                                ::validation::storage_path_relations::canonical_places_may_overlap(
                                    program, read, write,
                                )
                            })
                        })
                    })
                }) {
                    row.value = None;
                }
                let Some(length) = &row.length else {
                    continue;
                };
                if writes.is_none_or(|writes| {
                    writes.iter().any(|write| {
                        ::validation::storage_path_relations::canonical_places_may_overlap(
                            program, &row.place, write,
                        ) || write_covers_place(program, write, &length.descriptor)
                            || length.selectors.iter().any(|read| {
                                ::validation::storage_path_relations::canonical_places_may_overlap(
                                    program, read, write,
                                )
                            })
                    })
                }) {
                    row.length = None;
                }
            }
        }

        pub(in super::super) fn begin_result_measurement(&mut self, expression: ExpressionHandle) {
            self.measured_results
                .retain(|(result, _)| *result != expression);
        }

        pub(in super::super) fn capture_result_measurement_for_collection(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            collection: ExpressionHandle,
            argument: ExpressionHandle,
        ) {
            let actual = match program.expression_table.expression(argument) {
                ExpressionNode::Borrow(borrow) => borrow.target,
                _ => argument,
            };
            if actual != collection {
                return;
            }
            if let Some(length) =
                self.capture_collection_length(program, machine, state, collection)
            {
                self.measured_results.push((expression, length));
            }
        }

        pub(in super::super) fn record_expression_dependencies(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
        ) {
            self.record_dependencies(
                program,
                machine,
                &ExpressionDependencyScope {
                    evaluation: state,
                    destination: state,
                    parameter_roots: &[],
                },
                expression,
                0,
            );
        }

        /*
            The caller supplies only parameter bindings already proved label-stable
            by the incoming-edge analysis. Incomplete or unmapped reads remain
            incomplete; they never become storage-free premises.
        */
        pub(in super::super) fn record_incoming_guard_dependencies(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            evaluation: &State,
            destination: &State,
            parameter_roots: &[(SymbolHandle, SymbolHandle)],
            expression: ExpressionHandle,
        ) {
            self.record_dependencies(
                program,
                machine,
                &ExpressionDependencyScope {
                    evaluation,
                    destination,
                    parameter_roots,
                },
                expression,
                0,
            );
        }

        fn record_dependencies(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            scope: &ExpressionDependencyScope<'_>,
            expression: ExpressionHandle,
            depth: usize,
        ) {
            let state = scope.destination;
            if depth >= EXPRESSION_WALK_DEPTH_BOUND
                || !program.expression_table.expression_is_valid(expression)
            {
                return;
            }
            if matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Name(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
            ) {
                return;
            }
            if self
                .recorded_expressions
                .contains(&(expression, machine.symbol, state.symbol))
            {
                return;
            }
            let mut reads = Vec::new();
            let mut complete = has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(scope.evaluation),
                expression,
            ) && self.walk_reads(
                program,
                machine,
                scope.evaluation,
                self.statement_index,
                expression,
                &mut reads,
            );
            let descriptor_read =
                collection_length_receiver(program, machine, Some(scope.evaluation), expression)
                    .and_then(|receiver| {
                        place_at(
                            program,
                            scope.evaluation.symbol,
                            self.statement_index,
                            receiver,
                        )
                    })
                    .and_then(|descriptor| {
                        reads.iter().position(|read| {
                            flow::canonical_places_agree(program, read, &descriptor)
                        })
                    });
            if complete && scope.evaluation.symbol != state.symbol {
                complete = state_transport::transport_reads(
                    program,
                    machine,
                    scope,
                    self.statement_index,
                    &mut reads,
                );
            }
            self.recorded_expressions
                .insert((expression, machine.symbol, state.symbol));
            self.expression_dependencies.push(ExpressionDependencies {
                expression,
                label: program.expression_table.display_name(expression),
                machine: machine.symbol,
                state: state.symbol,
                descriptor: descriptor_read
                    .filter(|_| complete)
                    .map(|position| reads[position].clone()),
                reads: complete.then_some(reads),
            });
            let children = match program.expression_table.expression(expression) {
                ExpressionNode::Binary(binary) => [binary.left, binary.right],
                ExpressionNode::Unary(unary) => [unary.operand, ExpressionHandle::invalid()],
                ExpressionNode::Cast(cast) => [cast.value, ExpressionHandle::invalid()],
                _ => return,
            };
            for child in children {
                self.record_dependencies(program, machine, scope, child, depth + 1);
            }
        }

        /*
            The display labels whose rows cannot select one typed meaning, judged
            over the whole table once per query instead of once per row.

            Two rows agree when they share machine and state and their read sets
            match (`same_reads`). That relation is symmetric and transitive but not
            reflexive: an incomplete read set, or a place with an unresolved
            segment, agrees with nothing, itself included. For such a relation the
            label's first row decides the whole label: when every row agrees with
            it, all rows agree pairwise; otherwise every row disagrees with some
            row. A row's own label therefore collides exactly when the per-row
            scan over every same-label row would find a disagreement. A scalar
            binding enters that comparison as its single place, and bindings
            sharing a label must also name one exact place among themselves.
        */
        fn label_collisions(
            &self,
            program: &dyn ::validation::CheckingInput,
        ) -> LabelCollisions<'_> {
            let mut groups = BTreeMap::<&str, LabelRows<'_>>::new();
            for row in &self.expression_dependencies {
                groups
                    .entry(row.label.as_str())
                    .or_default()
                    .expressions
                    .push(row);
            }
            for row in &self.scalar_binding_dependencies {
                groups
                    .entry(row.label.as_str())
                    .or_default()
                    .bindings
                    .push(row);
            }
            let mut collisions = LabelCollisions::default();
            for (label, rows) in groups {
                if rows.expressions.is_empty() && rows.bindings.len() == 1 {
                    continue;
                }
                let mut meanings = rows
                    .expressions
                    .iter()
                    .map(|row| {
                        (
                            row.machine,
                            row.state,
                            row.reads.as_deref(),
                            row.descriptor.as_ref(),
                        )
                    })
                    .chain(rows.bindings.iter().map(|row| {
                        (
                            row.machine,
                            row.state,
                            Some(std::slice::from_ref(&row.place)),
                            None,
                        )
                    }));
                let first = meanings.next().expect("a label group holds one row");
                let agree = |left: RowMeaning<'_>, right: RowMeaning<'_>| {
                    left.0 == right.0
                        && left.1 == right.1
                        && same_reads(program, left.2, right.2)
                        && match (left.3, right.3) {
                            (None, None) => true,
                            (Some(left), Some(right)) => {
                                flow::canonical_places_agree(program, left, right)
                            }
                            _ => false,
                        }
                };
                let meanings_agree =
                    agree(first, first) && meanings.all(|other| agree(first, other));
                if !meanings_agree && !rows.expressions.is_empty() {
                    collisions.expression_labels.insert(label);
                }
                let bindings_identical = rows.bindings.split_first().is_none_or(|(first, rest)| {
                    rest.iter().all(|other| {
                        other.machine == first.machine
                            && other.state == first.state
                            && other.place == first.place
                    })
                });
                if !bindings_identical || (!meanings_agree && !rows.expressions.is_empty()) {
                    collisions.binding_labels.insert(label);
                }
            }
            collisions
        }

        pub(super) fn preserved_expression_labels(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            writes: Option<&[CanonicalPlace]>,
        ) -> Vec<String> {
            let Some(writes) = writes else {
                return Vec::new();
            };
            if !self
                .expression_dependencies
                .iter()
                .any(|row| row.machine == machine.symbol && row.state == state.symbol)
                && !self
                    .scalar_binding_dependencies
                    .iter()
                    .any(|row| row.machine == machine.symbol && row.state == state.symbol)
            {
                return Vec::new();
            }
            for write in writes {
                if !matches!(write.root, PlaceRoot::Symbol(symbol) if symbol.is_valid() && program.symbols.get(symbol).kind != SymbolKind::Field)
                {
                    return Vec::new();
                }
            }
            let collisions = self.label_collisions(program);
            let mut preserved = Vec::new();
            for row in &self.expression_dependencies {
                if !row.reads.as_ref().is_some_and(|reads| {
                    row.machine == machine.symbol
                        && row.state == state.symbol
                        && program.expression_table.expression_is_valid(row.expression)
                        && reads.iter().all(|read| {
                            writes.iter().all(|write| {
                                if row.descriptor.as_ref().is_some_and(|descriptor| {
                                    flow::canonical_places_agree(program, descriptor, read)
                                }) {
                                    !write_covers_place(program, write, read)
                                } else {
                                    !::validation::storage_path_relations::canonical_places_may_overlap(program, read, write)
                                }
                            })
                        })
                }) {
                    continue;
                }
                if collisions.expression_labels.contains(row.label.as_str()) {
                    continue;
                }
                if !preserved.contains(&row.label) {
                    preserved.push(row.label.clone());
                }
            }
            for row in &self.scalar_binding_dependencies {
                if row.machine != machine.symbol
                    || row.state != state.symbol
                    || writes.iter().any(|write| {
                        ::validation::storage_path_relations::canonical_places_may_overlap(
                            program, &row.place, write,
                        )
                    })
                    || collisions.binding_labels.contains(row.label.as_str())
                {
                    continue;
                }
                if !preserved.contains(&row.label) {
                    preserved.push(row.label.clone());
                }
            }
            preserved
        }

        /*
            An owned integer slot has no referent: complete storage writes suffice
            even when other locals prevent alias expansion. Keep all the ordinary
            identity and same-label collision checks before selecting these rows.
        */
        pub(super) fn preserved_scalar_binding_labels(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            storage_writes: Option<&[CanonicalPlace]>,
        ) -> Vec<String> {
            if !self
                .scalar_binding_dependencies
                .iter()
                .any(|row| row.machine == machine.symbol && row.state == state.symbol)
            {
                return Vec::new();
            }
            let mut preserved =
                self.preserved_expression_labels(program, machine, state, storage_writes);
            preserved.retain(|label| {
                self.scalar_binding_dependencies.iter().any(|row| {
                    row.machine == machine.symbol
                        && row.state == state.symbol
                        && row.label == *label
                })
            });
            preserved
        }

        /*
            The labels this machine state's rows carry outside `preserved`, the
            caller's `preserved_expression_labels` answer for the same writes.
        */
        pub(super) fn affected_expression_labels(
            &self,
            machine: &Machine,
            state: &State,
            preserved: &[String],
        ) -> Vec<String> {
            self.expression_dependencies
                .iter()
                .filter(|row| {
                    row.machine == machine.symbol
                        && row.state == state.symbol
                        && !preserved.contains(&row.label)
                })
                .map(|row| row.label.clone())
                .chain(
                    self.scalar_binding_dependencies
                        .iter()
                        .filter(|row| {
                            row.machine == machine.symbol
                                && row.state == state.symbol
                                && !preserved.contains(&row.label)
                        })
                        .map(|row| row.label.clone()),
                )
                .collect()
        }

        pub(in super::super) fn expression_is_disjoint_from_writes(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            writes: Option<&[CanonicalPlace]>,
        ) -> bool {
            self.preserved_expression_labels(program, machine, state, writes)
                .contains(&program.expression_table.display_name(expression))
        }
    }

    fn same_reads(
        program: &dyn ::validation::CheckingInput,
        left: Option<&[CanonicalPlace]>,
        right: Option<&[CanonicalPlace]>,
    ) -> bool {
        let (Some(left), Some(right)) = (left, right) else {
            return false;
        };
        let equal = |left: &CanonicalPlace, right: &CanonicalPlace| {
            flow::canonical_places_agree(program, left, right)
        };
        if std::ptr::eq(left, right) {
            return left.iter().all(|read| equal(read, read));
        }
        left.iter()
            .all(|read| right.iter().any(|other| equal(read, other)))
            && right
                .iter()
                .all(|read| left.iter().any(|other| equal(read, other)))
    }
}
mod invalidation {
    use super::RangeFacts;
    use crate::checks::ranges::indexes::is_builtin_scalar_index;
    use crate::flow;
    use validation::CanonicalPlace;

    use language_core::place_field;
    use typed_trees::{machine::Machine, state::State, statement::StatementNode};

    impl RangeFacts<'_> {
        /*
            Shared direct-assignment transfer for the checking and incoming-edge
            walks. Replacing an index invalidates its ordering/position premises;
            replacing a collection descriptor invalidates its extent/window rows.
            Value snapshots are replaced separately after the RHS is evaluated.
        */
        pub(in crate::checks::ranges) fn invalidate_assignment_bounds(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            statement: &StatementNode,
        ) {
            let Some(assignment) = statement.as_assignment() else {
                return;
            };
            if !self.has_relational_bounds() {
                if !self.measured_results.is_empty() || !self.scalar_binding_dependencies.is_empty()
                {
                    let mut owned_frames = None;
                    let shared_frames = flow::shared_call_frames_or(
                        self.checked_calls.and_then(|context| context.call_frames()),
                        program,
                        &mut owned_frames,
                    );
                    let writes = flow::statement_storage_writes(
                        program,
                        machine.symbol,
                        state.symbol,
                        self.statement_index,
                        statement,
                        shared_frames,
                    );
                    self.retire_captured_observations(program, writes.as_deref());
                }
                self.boolean_locals.clear();
                return;
            }
            let extent = self.mutable_byte_view_extent(program, machine, state, assignment.target);
            let target = program.expression_table.display_name(assignment.target);
            let mut owned_frames = None;
            let shared_frames = flow::shared_call_frames_or(
                self.checked_calls.and_then(|context| context.call_frames()),
                program,
                &mut owned_frames,
            );
            let bound_frame = (!self.proven_index_upper_bounds.is_empty()).then(|| {
                shared_frames.map_or_else(facts::NormalizedWriteFrame::opaque, |frames| {
                    frames.assignment_write_frame(machine, statement)
                })
            });
            let writes = (!self.expression_dependencies.is_empty()
                || !self.scalar_binding_dependencies.is_empty()
                || !self.measured_results.is_empty()
                || self.has_descriptor_facts())
            .then(|| {
                flow::statement_storage_writes(
                    program,
                    machine.symbol,
                    state.symbol,
                    self.statement_index,
                    statement,
                    shared_frames,
                )
            })
            .flatten();
            self.retire_captured_observations(program, writes.as_deref());
            let preserved =
                self.preserved_expression_labels(program, machine, state, writes.as_deref());
            let affected = self.affected_expression_labels(machine, state, &preserved);
            let statement_index = self.statement_index;
            let data_binding = program
                .state_data_binding(machine, state)
                .map(|name| name.as_str());
            let string_fallback = |name: &str| {
                !preserved.iter().any(|label| label == name)
                    && bound_frame.as_ref().map_or_else(
                        || write_affects_bound(data_binding, name, &target),
                        |frame| {
                            frame.complete_paths().is_none_or(|paths| {
                                paths
                                    .iter()
                                    .any(|path| write_affects_bound(data_binding, name, path))
                            })
                        },
                    )
            };
            let overlaps =
                |name: &str| affected.iter().any(|label| label == name) || string_fallback(name);
            let covers = |name: &str| {
                affected.iter().any(|label| label == name)
                    || match writes.as_deref() {
                        Some(writes) => descriptor_covered(
                            program,
                            state,
                            statement_index,
                            writes,
                            symbols::SymbolHandle::invalid(),
                            name,
                        )
                        .unwrap_or_else(|| string_fallback(name)),
                        None => string_fallback(name),
                    }
            };
            self.invalidate_relational_bounds(overlaps, covers);
            if let Some((collection, minimum, exact)) = extent {
                if let Some(minimum) = minimum {
                    self.prove_minimum_length(collection.clone(), minimum);
                }
                if let Some(exact) = exact {
                    self.prove_exact_length(collection, exact);
                }
            }
            self.boolean_locals.clear();
        }

        fn mutable_byte_view_extent(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            target: typed_trees::expression::ExpressionHandle,
        ) -> Option<(String, Option<i64>, Option<i64>)> {
            if !is_builtin_scalar_index(program, machine, state, self, target) {
                return None;
            }
            let indexed = program.expression_table.indexed_node(target)?;
            let name = program
                .expression_table
                .name_path_node(indexed.collection)?;
            ::validation::mutable_byte_slice_parameter_element(program, state, name.symbol)?;
            let collection = program.expression_table.display_name(indexed.collection);
            let minimum = self.minimum_length(&collection);
            let exact = self.exact_length(&collection);
            Some((collection, minimum, exact))
        }

        /*
            A complete call frame includes both caller storage and overlapping live
            alias spellings. Opaque frames retain the checked signature's write
            ceiling; an unrepresentable ceiling retires value-dependent premises.
            Declared fixed-array extents remain true regardless of element writes.
        */
        pub(in crate::checks::ranges) fn invalidate_call_writes(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            paths: Option<&[String]>,
            site: Option<&validation::CallSite<'_>>,
        ) {
            if paths.is_some_and(|paths| paths.is_empty()) {
                return;
            }
            if !self.has_relational_bounds()
                && self.integer_locals.is_empty()
                && self.locals.is_empty()
            {
                if !self.measured_results.is_empty() || !self.scalar_binding_dependencies.is_empty()
                {
                    let call_writes = site
                        .and_then(|site| self.structured_call_writes(program, machine, state, site))
                        .and_then(|writes| writes.aliases);
                    self.retire_captured_observations(program, call_writes.as_deref());
                }
                self.integer_fields.clear();
                self.boolean_locals.clear();
                return;
            }
            let call_writes = site
                .filter(|_| {
                    !self.expression_dependencies.is_empty()
                        || !self.scalar_binding_dependencies.is_empty()
                        || !self.measured_results.is_empty()
                        || self.has_descriptor_facts()
                })
                .and_then(|site| self.structured_call_writes(program, machine, state, site));
            let scalar_preserved = self.preserved_scalar_binding_labels(
                program,
                machine,
                state,
                call_writes.as_ref().map(|writes| writes.storage.as_slice()),
            );
            let call_writes = call_writes.and_then(|writes| writes.aliases);
            self.retire_captured_observations(program, call_writes.as_deref());
            let mut owned_frames = None;
            let shared_frames = flow::shared_call_frames_or(
                self.checked_calls.and_then(|context| context.call_frames()),
                program,
                &mut owned_frames,
            );
            let writes = call_writes.clone().or_else(|| {
                paths
                    .filter(|_| !self.expression_dependencies.is_empty())
                    .and_then(|paths| {
                        flow::frame_storage_writes(
                            program,
                            machine.symbol,
                            state.symbol,
                            self.statement_index,
                            &facts::NormalizedWriteFrame::complete(paths.to_vec()),
                            shared_frames,
                        )
                    })
            });
            let preserved =
                self.preserved_expression_labels(program, machine, state, writes.as_deref());
            let affected = self.affected_expression_labels(machine, state, &preserved);
            let statement_index = self.statement_index;
            let data_binding = program
                .state_data_binding(machine, state)
                .map(|name| name.as_str());
            let string_fallback = |name: &str| {
                !preserved.iter().any(|label| label == name)
                    && paths.is_none_or(|paths| {
                        paths
                            .iter()
                            .any(|path| write_affects_bound(data_binding, name, path))
                    })
            };
            let overlaps = |name: &str| {
                !scalar_preserved.iter().any(|label| label == name)
                    && (affected.iter().any(|label| label == name) || string_fallback(name))
            };
            let covers = |name: &str| {
                affected.iter().any(|label| label == name)
                    || match call_writes.as_deref() {
                        Some(writes) => descriptor_covered(
                            program,
                            state,
                            statement_index,
                            writes,
                            symbols::SymbolHandle::invalid(),
                            name,
                        )
                        .unwrap_or_else(|| string_fallback(name)),
                        None => string_fallback(name),
                    }
            };
            self.integer_fields.clear();
            self.integer_locals.retain(|(symbol, name, _)| {
                let declaration = program.symbols.get(*symbol);
                matches!(
                    declaration.kind,
                    symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
                ) && declaration.parent == state.symbol
                    && !overlaps(name)
            });
            self.locals.retain(|(symbol, name, _)| {
                let covered = affected.iter().any(|label| label == name.as_str())
                    || match call_writes.as_deref() {
                        Some(writes) => descriptor_covered(
                            program,
                            state,
                            statement_index,
                            writes,
                            *symbol,
                            name,
                        )
                        .unwrap_or_else(|| string_fallback(name)),
                        None => string_fallback(name),
                    };
                !covered || {
                    let reference = program
                        .state_parameter_by_symbol(state, *symbol)
                        .map(|parameter| parameter.type_reference)
                        .or_else(|| {
                            program
                                .state_local_declaration(state, *symbol)
                                .map(|(_, local)| local.type_reference)
                        });
                    reference.is_some_and(|reference| {
                        super::super::arrays::fixed_array_type_length(program, reference).is_some()
                    })
                }
            });
            self.invalidate_relational_bounds(overlaps, covers);
            self.boolean_locals.clear();
        }

        /*
            Any live row whose key is a collection's own storage — a view extent,
            a window parent edge, or the collection key of an index/range proof —
            deserves the structured-write coverage answer rather than the coarse
            frame paths.
        */
        fn has_descriptor_facts(&self) -> bool {
            !self.locals.is_empty()
                || !self.minimum_lengths.is_empty()
                || !self.exact_lengths.is_empty()
                || !self.window_parents.is_empty()
                || !self.proven_indexes.is_empty()
                || !self.proven_range_bounds.is_empty()
        }

        fn has_relational_bounds(&self) -> bool {
            !self.proven_indexes.is_empty()
                || !self.proven_index_upper_bounds.is_empty()
                || !self.below_const_binders.is_empty()
                || !self.proven_non_negatives.is_empty()
                || !self.proven_orderings.is_empty()
                || !self.proven_range_bounds.is_empty()
                || !self.minimum_lengths.is_empty()
                || !self.exact_lengths.is_empty()
                || !self.window_parents.is_empty()
        }

        /*
            `overlaps` decides value-level rows (indices, bounds, orderings);
            `covers` decides descriptor rows whose key names a collection place.
            A write below the collection — `rooms[0]` inside `rooms` — retires the
            element's own facts but leaves the collection's extent true.
        */
        pub(super) fn invalidate_relational_bounds(
            &mut self,
            overlaps: impl Fn(&str) -> bool,
            covers: impl Fn(&str) -> bool,
        ) {
            self.proven_indexes
                .retain(|(collection, index)| !covers(collection) && !overlaps(index));
            self.proven_index_upper_bounds
                .retain(|(index, _)| !overlaps(index));
            self.below_const_binders
                .retain(|(index, _)| !overlaps(index));
            self.proven_non_negatives.retain(|index| !overlaps(index));
            self.proven_orderings
                .retain(|(left, right, _)| !overlaps(left) && !overlaps(right));
            self.proven_range_bounds
                .retain(|(collection, bound)| !covers(collection) && !overlaps(bound));
            self.minimum_lengths
                .retain(|(collection, _)| !covers(collection));
            self.exact_lengths
                .retain(|(collection, _)| !covers(collection));
            self.window_parents
                .retain(|(child, parent, _)| !covers(child) && !covers(parent));
        }
    }

    /*
        Resolve a descriptor row's key to the caller place it measures, then ask
        whether any write reaches that place itself. A declared local symbol is
        the identity when present; otherwise the dotted label resolves through
        the same parameter/local namespace the write paths use. `None` reports
        "unresolvable — keep the caller's conservative answer".
    */
    fn descriptor_covered(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statement_index: usize,
        writes: &[CanonicalPlace],
        symbol: symbols::SymbolHandle,
        label: &str,
    ) -> Option<bool> {
        let place = if symbol.is_valid() {
            flow::canonical_place_from_symbol(symbol)?
        } else {
            flow::place_from_origin_path(program, state, statement_index, label)?
        };
        Some(
            writes
                .iter()
                .any(|write| write_covers_place(program, write, &place)),
        )
    }

    /*
        A write covers a descriptor place only by reaching it or an ancestor:
        `rooms` covers `rooms`, but `rooms[0].tag` — any depth below — does not.
    */
    pub(super) fn write_covers_place(
        program: &dyn ::validation::CheckingInput,
        write: &CanonicalPlace,
        place: &CanonicalPlace,
    ) -> bool {
        (write.root) == (place.root)
            && write.segments.len() <= place.segments.len()
            && flow::canonical_place_segments_may_overlap(program, &write.segments, &place.segments)
    }

    /*
        Whether a write to `path` expires the bound labelled `name`.

        `data_binding` is the name of the writing state's data record, so a write
        spelled through it (`main.i`) expires a bound spelled plainly (`i`), and a
        write of the whole record expires every unqualified bound. A state with no
        data binding writes no such place.
    */
    fn write_affects_bound(data_binding: Option<&str>, name: &str, path: &str) -> bool {
        !name.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|character| character.is_alphanumeric() || character == '_')
        }) || validation::frame_paths_overlap(name, path)
            || data_binding
                .and_then(|root| place_field(root, path))
                .is_some_and(|path| validation::frame_paths_overlap(name, path))
            || (data_binding == Some(path) && !name.contains('.'))
    }
}
mod proofs {
    use super::RangeFacts;
    use symbols::SymbolHandle;

    mod aliases {
        use super::super::RangeFacts;

        impl RangeFacts<'_> {
            pub(in crate::checks::ranges) fn alias_collection(
                &mut self,
                original: &str,
                alias: &str,
            ) {
                if original == alias {
                    return;
                }

                if !self.windows_may_overlap(original, alias) {
                    if let Some(minimum_length) = self.minimum_length(original) {
                        self.prove_minimum_length(alias.to_owned(), minimum_length);
                    }
                    if let Some(exact_length) = self.exact_length(original) {
                        self.prove_exact_length(alias.to_owned(), exact_length);
                    }
                    return;
                }

                for (_, index) in self
                    .proven_indexes
                    .clone()
                    .into_iter()
                    .filter(|(collection, _)| collection == original)
                {
                    self.prove_index(alias.to_owned(), index);
                }
                for (_, bound) in self
                    .proven_range_bounds
                    .clone()
                    .into_iter()
                    .filter(|(collection, _)| collection == original)
                {
                    self.prove_range_bound(alias.to_owned(), bound);
                }
                if let Some(minimum_length) = self.minimum_length(original) {
                    self.prove_minimum_length(alias.to_owned(), minimum_length);
                }
                if let Some(exact_length) = self.exact_length(original) {
                    self.prove_exact_length(alias.to_owned(), exact_length);
                }
            }

            pub(in crate::checks::ranges) fn alias_index(&mut self, original: &str, alias: &str) {
                if original == alias {
                    return;
                }

                if self.non_negative_is_proven(original)
                    || self.non_negative_is_proven_via_ordering(original)
                {
                    self.prove_non_negative(alias.to_owned());
                }

                for (_, upper_bound) in self
                    .proven_index_upper_bounds
                    .clone()
                    .into_iter()
                    .filter(|(index, _)| index == original)
                {
                    self.prove_index_upper_bound(alias.to_owned(), upper_bound);
                }
                for (_, binder) in self
                    .below_const_binders
                    .clone()
                    .into_iter()
                    .filter(|(index, _)| index == original)
                {
                    self.prove_below_const_binder(alias.to_owned(), binder);
                }
                for (collection, _) in self
                    .proven_indexes
                    .clone()
                    .into_iter()
                    .filter(|(_, index)| index == original)
                {
                    self.prove_index(collection, alias.to_owned());
                }
                for (collection, _) in self
                    .proven_range_bounds
                    .clone()
                    .into_iter()
                    .filter(|(_, bound)| bound == original)
                {
                    self.prove_range_bound(collection, alias.to_owned());
                }
                for (lower, upper, strict) in self
                    .proven_orderings
                    .clone()
                    .into_iter()
                    .filter(|(lower, upper, _)| lower == original || upper == original)
                {
                    let lower = if lower == original {
                        alias.to_owned()
                    } else {
                        lower
                    };
                    let upper = if upper == original {
                        alias.to_owned()
                    } else {
                        upper
                    };
                    if strict {
                        self.prove_strictly_less(lower, upper);
                    } else {
                        self.prove_at_most(lower, upper);
                    }
                }
            }
        }
    }

    impl RangeFacts<'_> {
        pub(in super::super) fn prove_const_binder_floor(
            &mut self,
            symbol: SymbolHandle,
            floor: i64,
        ) {
            if let Some((_, known)) = self
                .const_binder_floors
                .iter_mut()
                .find(|(known, _)| *known == symbol)
            {
                *known = (*known).max(floor);
            } else {
                self.const_binder_floors.push((symbol, floor));
            }
        }

        pub(in super::super) fn const_binder_floor(&self, symbol: SymbolHandle) -> Option<i64> {
            self.const_binder_floors
                .iter()
                .find_map(|(known, floor)| (*known == symbol).then_some(*floor))
        }

        /*
            `index` is strictly less than this const binder. The binder's symbol,
            not its spelling, is the identity: a same-named local is not an extent.
        */
        pub(in super::super) fn prove_below_const_binder(
            &mut self,
            index: String,
            binder: SymbolHandle,
        ) {
            if !binder.is_valid()
                || self
                    .below_const_binders
                    .iter()
                    .any(|(known_index, known_binder)| {
                        known_index == &index && *known_binder == binder
                    })
            {
                return;
            }
            self.below_const_binders.push((index, binder));
        }

        pub(in super::super) fn below_const_binder(
            &self,
            index: &str,
            binder: SymbolHandle,
        ) -> bool {
            binder.is_valid()
                && self
                    .below_const_binders
                    .iter()
                    .any(|(known_index, known_binder)| {
                        known_index == index && *known_binder == binder
                    })
        }

        pub(in super::super) fn const_binders_strictly_above(
            &self,
            index: &str,
        ) -> Vec<SymbolHandle> {
            self.below_const_binders
                .iter()
                .filter(|(known_index, _)| known_index == index)
                .map(|(_, binder)| *binder)
                .collect()
        }

        pub(in super::super) fn prove_index(&mut self, collection: String, index: String) {
            if !self
                .proven_indexes
                .iter()
                .any(|(known_collection, known_index)| {
                    known_collection == &collection && known_index == &index
                })
            {
                self.proven_indexes.push((collection, index));
                self.close_ordering_bounds();
            }
        }

        pub(in super::super) fn index_is_proven(&self, collection: &str, index: &str) -> bool {
            self.proven_indexes
                .iter()
                .any(|(known_collection, known_index)| {
                    known_collection == collection && known_index == index
                })
        }

        /*
            Every proven `(collection, index)` label pair, for callers that test
            many pairs against one unchanged roster: one set lookup per pair
            instead of a scan of the whole roster per pair.
        */
        pub(in super::super) fn proven_index_pairs(
            &self,
        ) -> std::collections::BTreeSet<(&str, &str)> {
            self.proven_indexes
                .iter()
                .map(|(collection, index)| (collection.as_str(), index.as_str()))
                .collect()
        }

        /*
            The index labels proven valid inside `collection`, in seed order. Edge
            transport re-keys these pairs onto the callee's parameter names.
        */
        pub(in super::super) fn proven_index_labels<'a>(
            &'a self,
            collection: &'a str,
        ) -> impl Iterator<Item = &'a String> + 'a {
            self.proven_indexes
                .iter()
                .filter(move |(known_collection, _)| known_collection == collection)
                .map(|(_, index)| index)
        }

        /* The collection labels `index` is proven inside, in seed order. */
        pub(in super::super) fn proven_collections_for_index<'a>(
            &'a self,
            index: &'a str,
        ) -> impl Iterator<Item = &'a String> + 'a {
            self.proven_indexes
                .iter()
                .filter(move |(_, known_index)| known_index == index)
                .map(|(collection, _)| collection)
        }

        pub(in super::super) fn prove_index_upper_bound(
            &mut self,
            index: String,
            exclusive_upper_bound: i64,
        ) {
            if exclusive_upper_bound <= 0 {
                return;
            }

            if let Some((_, known_upper_bound)) = self
                .proven_index_upper_bounds
                .iter_mut()
                .find(|(known_index, _)| known_index == &index)
            {
                *known_upper_bound = (*known_upper_bound).min(exclusive_upper_bound);
                return;
            }

            self.proven_index_upper_bounds
                .push((index, exclusive_upper_bound));
        }

        pub(in super::super) fn index_upper_bound_is_proven(
            &self,
            index: &str,
            length: usize,
        ) -> bool {
            let Ok(length) = i64::try_from(length) else {
                return false;
            };

            self.proven_index_upper_bounds
                .iter()
                .any(|(known_index, upper_bound)| known_index == index && *upper_bound <= length)
        }

        /*
            The tightest (smallest) proven EXCLUSIVE upper bound for `index`, if any.
            Used to carry a bound across `field = otherfield + const`.
        */
        pub(in super::super) fn proven_index_upper_bound(&self, index: &str) -> Option<i64> {
            self.proven_index_upper_bounds
                .iter()
                .filter(|(known_index, _)| known_index == index)
                .map(|(_, upper_bound)| *upper_bound)
                .min()
        }

        pub(in super::super) fn index_value_is_proven(&self, collection: &str, index: i64) -> bool {
            if index < 0 {
                return false;
            }
            self.minimum_length(collection)
                .into_iter()
                .chain(self.exact_length(collection))
                .any(|length| index < length)
        }

        /* Records that `name` is provably `>= 0`. */
        pub(in super::super) fn prove_non_negative(&mut self, name: String) {
            if !self.proven_non_negatives.iter().any(|known| known == &name) {
                self.proven_non_negatives.push(name);
            }
        }

        pub(in super::super) fn non_negative_is_proven(&self, name: &str) -> bool {
            self.proven_non_negatives.iter().any(|known| known == name)
        }

        fn record_ordering(&mut self, lower: String, upper: String, strict: bool) {
            if let Some((_, _, known_strict)) =
                self.proven_orderings
                    .iter_mut()
                    .find(|(known_lower, known_upper, _)| {
                        known_lower == &lower && known_upper == &upper
                    })
            {
                *known_strict |= strict;
            } else {
                self.proven_orderings.push((lower, upper, strict));
            }
            self.close_ordering_bounds();
        }

        /*
            Forward-chains collection bounds through ordering facts until
            fixpoint. `index <= pivot && pivot < len` proves `index < len` (a
            proven index pair); `index < pivot && pivot <= len` proves the same
            (strictness pays the one element); `index <= pivot && pivot <= len`
            proves `index <= len` (a proven range bound). Minting the derived
            pair eagerly — rather than only answering it at query time — is what
            lets the proof ride the parameter-position transport across a state
            edge: the transported fact is the (collection, index) pair, not the
            ordering. Every minted fact is a true consequence of its premises,
            so eager closure cannot make an unsound read provable.
        */
        fn close_ordering_bounds(&mut self) {
            validation::propagate_integer_upper_bounds(
                &self.proven_orderings,
                &mut self.proven_indexes,
                &mut self.proven_range_bounds,
            );
        }

        pub(in super::super) fn prove_at_most(&mut self, lower: String, upper: String) {
            self.record_ordering(lower, upper, false);
        }

        /*
            `lower < upper` — the strict counterpart of `prove_at_most`. Every
            consumer that accepts `<=` also accepts `<` (it is the stronger fact);
            only the bound-chaining consumers below read the flag.
        */
        pub(in super::super) fn prove_strictly_less(&mut self, lower: String, upper: String) {
            self.record_ordering(lower, upper, true);
        }

        pub(in super::super) fn at_most_is_proven(&self, lower: &str, upper: &str) -> bool {
            self.proven_orderings
                .iter()
                .any(|(known_lower, known_upper, _)| known_lower == lower && known_upper == upper)
        }

        /*
            Proves `index < length` by chaining an ordering with a bound: if
            `index <= j` (an `at_most` fact, e.g. from a two-pointer loop guard
            `index < j`) and `j` has a proven exclusive upper bound `<= length`, then
            `index <= j < length`, so `index < length`. One level of chaining --
            enough for the palindrome / two-pointer case where the increasing pointer
            is bounded only relative to the decreasing pointer (which carries the loop
            invariant). Sound because `at_most` is seeded from `<`/`<=` guards and
            dropped on reassignment, so it reflects a fact live at the access.
        */
        pub(in super::super) fn index_upper_bound_is_proven_via_ordering(
            &self,
            index: &str,
            collection: &str,
            length: usize,
        ) -> bool {
            self.proven_orderings
                .iter()
                .filter(|(lower, _, _)| lower == index)
                .any(|(_, upper, strict)| {
                    if *strict {
                        self.index_is_proven(collection, upper)
                            || self.range_bound_is_proven(collection, upper)
                            || self.index_upper_bound_is_proven(upper, length.saturating_add(1))
                    } else {
                        self.index_is_proven(collection, upper)
                            || self.index_upper_bound_is_proven(upper, length)
                    }
                })
        }

        /*
            Proves `index < len` for an UNKNOWN-extent collection from a label-keyed
            exclusive upper bound: `index < u` (seeded by a `let` alias of an ensured
            call, an `i < K` guard, or a bound carried through a transition's
            argument transport) and `u <= floor` — where `floor` is the collection's
            `minimum_length`/`exact_length` lower-bounding its live length — give
            `index < u <= floor <= len`. The `u - 1` spelling reuses the strict
            index-value floor check: `u - 1 < floor` is `u <= floor` for integers,
            and stored bounds are always positive so `u - 1` cannot underflow.
        */
        pub(in super::super) fn index_upper_bound_within_length_floor(
            &self,
            index: &str,
            collection: &str,
        ) -> bool {
            self.proven_index_upper_bound(index)
                .is_some_and(|upper| self.index_value_is_proven(collection, upper - 1))
        }

        /*
            The ordering-chained counterpart of `index_upper_bound_within_length_floor`:
            `index <= pivot` (an `at_most` fact) plus `pivot`'s exclusive bound meeting
            the floor gives `index <= pivot < u <= floor <= len`. One level of
            chaining, the same soundness argument as
            `index_upper_bound_is_proven_via_ordering` over a symbolic length.
        */
        pub(in super::super) fn index_upper_bound_within_length_floor_via_ordering(
            &self,
            index: &str,
            collection: &str,
        ) -> bool {
            self.proven_orderings
                .iter()
                .filter(|(lower, _, _)| lower == index)
                .any(|(_, pivot, strict)| {
                    if *strict {
                        self.index_is_proven(collection, pivot)
                            || self.range_bound_is_proven(collection, pivot)
                            || self.proven_index_upper_bound(pivot).is_some_and(|upper| {
                                self.range_bound_value_is_proven(collection, upper - 1)
                            })
                    } else {
                        self.index_is_proven(collection, pivot)
                            || self.index_upper_bound_within_length_floor(pivot, collection)
                    }
                })
        }

        /*
            Proves `index >= 0` by chaining an ordering with non-negativity: if `x <= index`
            (an `at_most` fact, e.g. from a two-pointer guard `x < index`) and `x` is proven
            non-negative, then `index >= x >= 0`. The symmetric counterpart of
            `index_upper_bound_is_proven_via_ordering` -- it carries the DECREASING pointer's
            lower bound from the INCREASING pointer that sits below it (the palindrome /
            two-pointer case where `j` runs down but stays `> i >= 0`). Sound because
            `at_most` is seeded from `<`/`<=` guards and dropped on reassignment, so it
            reflects a relation live at the access.
        */
        pub(in super::super) fn non_negative_is_proven_via_ordering(&self, index: &str) -> bool {
            self.proven_orderings
                .iter()
                .filter(|(_, upper, _)| upper == index)
                .any(|(lower, _, _)| self.non_negative_is_proven(lower))
        }

        pub(in super::super) fn prove_range_bound(&mut self, collection: String, bound: String) {
            if !self
                .proven_range_bounds
                .iter()
                .any(|(known_collection, known_bound)| {
                    known_collection == &collection && known_bound == &bound
                })
            {
                self.proven_range_bounds.push((collection, bound));
                self.close_ordering_bounds();
            }
        }

        pub(in super::super) fn range_bound_is_proven(
            &self,
            collection: &str,
            bound: &str,
        ) -> bool {
            self.proven_range_bounds
                .iter()
                .any(|(known_collection, known_bound)| {
                    known_collection == collection && known_bound == bound
                })
        }

        pub(in super::super) fn range_bound_value_is_proven(
            &self,
            collection: &str,
            bound: i64,
        ) -> bool {
            if bound < 0 {
                return false;
            }
            if bound == 0 {
                return true;
            }
            self.minimum_length(collection)
                .into_iter()
                .chain(self.exact_length(collection))
                .any(|length| bound <= length)
        }

        pub(in super::super) fn prove_minimum_length(
            &mut self,
            collection: String,
            minimum_length: i64,
        ) {
            if minimum_length <= 0 {
                return;
            }

            if let Some((_, known_minimum)) = self
                .minimum_lengths
                .iter_mut()
                .find(|(known_collection, _)| known_collection == &collection)
            {
                *known_minimum = (*known_minimum).max(minimum_length);
                return;
            }

            self.minimum_lengths.push((collection, minimum_length));
        }

        pub(in super::super) fn minimum_length(&self, collection: &str) -> Option<i64> {
            self.minimum_lengths
                .iter()
                .find_map(|(known_collection, minimum_length)| {
                    (known_collection == collection).then_some(*minimum_length)
                })
        }

        /*
            Records an exact-length fact: `collection` provably has exactly
            `length` elements (e.g. the subslice `a..b` over constant bounds has
            length `b - a`). Zero represents an empty live prefix; negative lengths
            are not valid collection extents.
        */
        pub(in super::super) fn prove_exact_length(&mut self, collection: String, length: i64) {
            if length < 0 {
                return;
            }

            if let Some(position) = self
                .exact_lengths
                .iter()
                .position(|(known_collection, _)| known_collection == &collection)
            {
                if self.exact_lengths[position].1 != length {
                    self.exact_lengths.remove(position);
                }
                return;
            }

            self.exact_lengths.push((collection, length));
        }

        pub(in super::super) fn exact_length(&self, collection: &str) -> Option<i64> {
            self.exact_lengths
                .iter()
                .find_map(|(known_collection, length)| {
                    (known_collection == collection).then_some(*length)
                })
        }

        /*
            Forgets every label-keyed collection fact about `collection`: length
            floors, exact lengths, the window-parent relation it was carved with,
            and element-position proofs keyed on it. Called when a local collection
            is reassigned — the facts described the old value, and keeping them
            (the merge in `prove_minimum_length` keeps the larger floor) would let a
            stale floor prove indexes into the new, possibly shorter value.
        */
        pub(in super::super) fn forget_collection_facts(&mut self, collection: &str) {
            self.minimum_lengths
                .retain(|(known, _)| known != collection);
            self.exact_lengths.retain(|(known, _)| known != collection);
            self.window_parents
                .retain(|(child, _, _)| child != collection);
            self.proven_indexes.retain(|(known, _)| known != collection);
            self.proven_range_bounds
                .retain(|(known, _)| known != collection);
        }

        /*
            Derives the carved tail window's length facts from its parent's length
            facts (window-shrinking length vocabulary): `parent[start..]` drops
            exactly `start` elements, so a parent with at least `m` elements leaves
            at least `m - start`, and a parent with exactly `n` elements leaves
            exactly `n - start`. Only meaningful for a start-only window — a
            constant-bounded window `[a..b)` already pins its exact length `b - a`
            independent of the parent.
        */
        pub(in super::super) fn prove_shrunk_window_length(
            &mut self,
            child: &str,
            parent: &str,
            start: i64,
        ) {
            if start < 0 {
                return;
            }
            if let Some(exact) = self.exact_length(parent)
                && exact >= start
            {
                self.prove_exact_length(child.to_owned(), exact - start);
            }
            if let Some(minimum) = self.minimum_length(parent) {
                self.prove_minimum_length(child.to_owned(), minimum - start);
            }
        }

        /*
            Records a window-shrinking fact: `child` is a subslice of `parent` and is
            therefore no longer than it. `bounds` are the child's constant `[start,
            end)` offsets into the parent when known. The relation is the basis for
            conservative subslice-overlap reasoning (two windows of the same base are
            assumed to overlap unless proven disjoint).
        */
        pub(in super::super) fn prove_window_parent(
            &mut self,
            child: String,
            parent: String,
            bounds: Option<(i64, i64)>,
        ) {
            if child == parent {
                return;
            }
            if let Some((known_parent, known_bounds)) = self.window_parents.iter_mut().find_map(
                |(known_child, known_parent, known_bounds)| {
                    (known_child == &child).then_some((known_parent, known_bounds))
                },
            ) {
                *known_parent = parent;
                *known_bounds = bounds.or(*known_bounds);
                return;
            }
            self.window_parents.push((child, parent, bounds));
        }

        /* The recorded parent (base) collection a window was carved from, if any. */
        pub(in super::super) fn window_parent(&self, child: &str) -> Option<&str> {
            self.window_parents
                .iter()
                .find_map(|(known_child, known_parent, _)| {
                    (known_child == child).then_some(known_parent.as_str())
                })
        }

        /* The recorded `[start, end)` offsets of a window into its base, if known. */
        fn window_bounds(&self, child: &str) -> Option<(i64, i64)> {
            self.window_parents
                .iter()
                .find_map(|(known_child, _, bounds)| (known_child == child).then_some(*bounds))
                .flatten()
        }

        /*
            Conservative subslice overlap: two windows/indices of the *same* base are
            assumed to overlap unless they are provably disjoint. Distinct bases never
            overlap; identical labels always overlap.

            The sharpened provable-disjoint cases derive each side's resolved `[start,
            end)` over the shared base from the recorded window bounds (literal vs.
            literal, disjoint windows). Any side with unknown bounds falls back to the
            conservative assumption that the windows overlap.
        */
        pub(in super::super) fn windows_may_overlap(&self, left: &str, right: &str) -> bool {
            if left == right {
                return true;
            }

            let left_base = self.window_parent(left).unwrap_or(left);
            let right_base = self.window_parent(right).unwrap_or(right);
            if left_base != right_base {
                return false;
            }

            match (self.window_bounds(left), self.window_bounds(right)) {
                (Some((left_start, left_end)), Some((right_start, right_end))) => {
                    !(left_end <= right_start || right_end <= left_start)
                }
                _ => true,
            }
        }
    }
}
mod values {
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    use super::RangeFacts;

    /*
        Reverse-ordered lookup in one `(symbol, name, value)` roster: exact
        symbol first, then a name fallback.
    */
    fn roster_lookup<T: Copy>(
        roster: &[(SymbolHandle, String, T)],
        symbol: SymbolHandle,
        name: Option<&str>,
    ) -> Option<T> {
        if symbol.is_valid()
            && let Some(value) = roster
                .iter()
                .rev()
                .find_map(|(local, _, value)| (*local == symbol).then_some(*value))
        {
            return Some(value);
        }

        roster.iter().rev().find_map(|(_, local_name, value)| {
            name.is_some_and(|name| name == local_name)
                .then_some(*value)
        })
    }

    impl RangeFacts<'_> {
        pub(in crate::checks::ranges) fn field_length(
            &self,
            symbol: SymbolHandle,
        ) -> Option<usize> {
            if !symbol.is_valid() {
                return None;
            }
            self.fields
                .iter()
                .find_map(|(field, _, length)| (*field == symbol).then_some(*length))
        }

        pub(in crate::checks::ranges) fn local_length(
            &self,
            symbol: SymbolHandle,
            name: Option<&str>,
        ) -> Option<usize> {
            roster_lookup(&self.locals, symbol, name)
        }

        pub(in crate::checks::ranges) fn local_integer(
            &self,
            symbol: SymbolHandle,
            name: Option<&str>,
        ) -> Option<i64> {
            roster_lookup(&self.integer_locals, symbol, name)
        }

        pub(in crate::checks::ranges) fn field_integer(
            &self,
            symbol: SymbolHandle,
            name: Option<&str>,
        ) -> Option<i64> {
            roster_lookup(&self.integer_fields, symbol, name)
        }

        pub(in crate::checks::ranges) fn define_local(
            &mut self,
            symbol: SymbolHandle,
            name: impl Into<String>,
            length: Option<usize>,
            integer: Option<i64>,
        ) {
            let name = name.into();
            if let Some(length) = length {
                self.locals.push((symbol, name.clone(), length));
            }
            if let Some(value) = integer {
                self.integer_locals.push((symbol, name, value));
            }
        }

        pub(in crate::checks::ranges) fn assign_local(
            &mut self,
            symbol: SymbolHandle,
            name: Option<&str>,
            length: Option<usize>,
            integer: Option<i64>,
        ) {
            self.forget_local(symbol, name);
            if let Some(name) = name {
                self.forget_collection_facts(name);
            }
            self.define_local(symbol, name.unwrap_or_default().to_owned(), length, integer);
        }

        pub(in crate::checks::ranges) fn define_field_integer(
            &mut self,
            symbol: SymbolHandle,
            name: impl Into<String>,
            integer: i64,
        ) {
            self.integer_fields.push((symbol, name.into(), integer));
        }

        pub(in crate::checks::ranges) fn assign_field_integer(
            &mut self,
            symbol: SymbolHandle,
            name: Option<&str>,
            integer: Option<i64>,
        ) {
            self.forget_field_integer(symbol, name);
            if let Some(integer) = integer {
                self.define_field_integer(symbol, name.unwrap_or_default().to_owned(), integer);
            }
        }

        pub(in crate::checks::ranges) fn define_boolean_guard_local(
            &mut self,
            symbol: SymbolHandle,
            name: impl Into<String>,
            guard: ExpressionHandle,
        ) {
            self.boolean_locals.push((symbol, name.into(), guard));
        }

        pub(in crate::checks::ranges) fn boolean_guard_local(
            &self,
            symbol: SymbolHandle,
            name: Option<&str>,
        ) -> Option<ExpressionHandle> {
            roster_lookup(&self.boolean_locals, symbol, name)
        }

        fn forget_local(&mut self, symbol: SymbolHandle, name: Option<&str>) {
            self.locals.retain(|(local, local_name, _)| {
                !value_fact_matches(*local, local_name, symbol, name)
            });
            self.integer_locals.retain(|(local, local_name, _)| {
                !value_fact_matches(*local, local_name, symbol, name)
            });
            self.boolean_locals.retain(|(local, local_name, _)| {
                !value_fact_matches(*local, local_name, symbol, name)
            });
        }

        fn forget_field_integer(&mut self, symbol: SymbolHandle, name: Option<&str>) {
            self.integer_fields.retain(|(field, field_name, _)| {
                !value_fact_matches(*field, field_name, symbol, name)
            });
        }
    }

    fn value_fact_matches(
        candidate_symbol: SymbolHandle,
        candidate_name: &str,
        symbol: SymbolHandle,
        name: Option<&str>,
    ) -> bool {
        candidate_symbol == symbol || name.is_some_and(|name| name == candidate_name)
    }
}
pub(super) use dependencies::ReceiverLength;

#[derive(Clone)]
pub(super) struct RangeFacts<'field> {
    pub(super) caller: SymbolHandle,
    pub(super) collection_bounds: Option<&'field validation::CollectionBoundsObservations<'field>>,
    pub(super) checked_operators: Option<&'field checked_trees::CheckedOperatorFacts>,
    pub(super) checked_calls: Option<&'field RangeCallContext<'field>>,
    pub(super) mutation_summaries: std::borrow::Cow<'field, StateMutationSummaryCache>,
    pub(super) statement_index: usize,
    expression_dependencies: Vec<dependencies::ExpressionDependencies>,
    scalar_binding_dependencies: Vec<dependencies::ScalarBindingDependency>,
    measured_results: Vec<(ExpressionHandle, dependencies::CapturedLength)>,
    recorded_expressions: HashSet<(ExpressionHandle, SymbolHandle, SymbolHandle)>,
    fields: &'field [(SymbolHandle, String, usize)],
    integer_fields: Vec<(SymbolHandle, String, i64)>,
    locals: Vec<(SymbolHandle, String, usize)>,
    integer_locals: Vec<(SymbolHandle, String, i64)>,
    proven_indexes: Vec<(String, String)>,
    proven_index_upper_bounds: Vec<(String, i64)>,
    /*
        Invocation-fixed const binder floors established by this branch.
        Symbol identity prevents same-spelled locals or unrelated binders from
        refining an array extent; immutable binders require no write retirement.
    */
    const_binder_floors: Vec<(SymbolHandle, i64)>,
    /*
        An index label strictly below a const binder (`index < Count`, or
        `index <= Count - k` for k >= 1). The binder is the symbolic extent
        of `[T; Count]`, so the pair is the index obligation itself. Binders
        are immutable; only a write of the index label retires the row.
    */
    below_const_binders: Vec<(String, SymbolHandle)>,
    /*
        Selected closed integer comparisons retain their exact source operands
        and asserted/refuted ordering. They contain no mutable observation.
    */
    closed_integer_arrivals: Vec<(
        ExpressionHandle,
        ExpressionHandle,
        typed_trees::expression::BinaryOperator,
    )>,
    /*
        Names provably `>= 0`. A SIGNED index access needs `0 <= i < len`; the
        upper-bound facts only give `i < len`, so a signed (negative-capable)
        index also requires its name here (seeded from `i >= 0`-style guards).
        Unsigned indices are non-negative by type and never consult this.
    */
    proven_non_negatives: Vec<String>,
    /*
        `(lower, upper, strict)`: `lower <= upper`, or `lower < upper` when
        strict. Strict entries are the stronger fact — a `<` guard mints one
        and a later `<=` on the same pair must not weaken it.
    */
    proven_orderings: Vec<(String, String, bool)>,
    proven_range_bounds: Vec<(String, String)>,
    minimum_lengths: Vec<(String, i64)>,
    /*
        Exact-length facts: a collection whose length is provably an exact value.
        A subslice `a..b` over constant bounds has exact length `b - a`; this is
        strictly stronger than a `minimum_length` floor because it also bounds
        the length from above, letting a literal index `i < len` be proven and a
        range bound `b <= len` be discharged against the precise length.
    */
    exact_lengths: Vec<(String, i64)>,
    /*
        Window-shrinking facts: `(child, parent, bounds)` records that `child` is
        a subslice of `parent` and is therefore no longer than `parent`. `bounds`
        holds the child's constant-folded `[start, end)` offsets into the parent
        when both are known, enabling provable-disjoint overlap reasoning between
        two windows of the same base. A `None` bounds is the conservative case
        (offsets unknown — assume overlap with any sibling).
    */
    window_parents: Vec<(String, String, Option<(i64, i64)>)>,
    boolean_locals: Vec<(
        SymbolHandle,
        String,
        typed_trees::expression::ExpressionHandle,
    )>,
    /*
        Branches borrow one worker-owned lookup over the immutable program.
        The lookup itself lazily obtains the invocation's shared bound maps;
        fact snapshots never acquire mutable access to query storage.
    */
    bound_lookup: &'field validation::ImmutableBoundLookup<'field>,
}

impl<'field> RangeFacts<'field> {
    pub(super) fn new(
        caller: SymbolHandle,
        fields: &'field [(SymbolHandle, String, usize)],
        bound_lookup: &'field validation::ImmutableBoundLookup<'field>,
    ) -> Self {
        Self {
            caller,
            collection_bounds: None,
            checked_operators: None,
            checked_calls: None,
            mutation_summaries: std::borrow::Cow::Owned(StateMutationSummaryCache::default()),
            statement_index: 0,
            expression_dependencies: Vec::new(),
            scalar_binding_dependencies: Vec::new(),
            measured_results: Vec::new(),
            recorded_expressions: HashSet::default(),
            fields,
            integer_fields: Vec::new(),
            locals: Vec::new(),
            integer_locals: Vec::new(),
            proven_indexes: Vec::new(),
            proven_index_upper_bounds: Vec::new(),
            const_binder_floors: Vec::new(),
            below_const_binders: Vec::new(),
            closed_integer_arrivals: Vec::new(),
            proven_non_negatives: Vec::new(),
            proven_orderings: Vec::new(),
            proven_range_bounds: Vec::new(),
            minimum_lengths: Vec::new(),
            exact_lengths: Vec::new(),
            window_parents: Vec::new(),
            boolean_locals: Vec::new(),
            bound_lookup,
        }
    }

    pub(super) fn bound_lookup(&self) -> &'field validation::ImmutableBoundLookup<'field> {
        self.bound_lookup
    }
}

/*
    Count derived snapshots without changing production fact storage or counting
    unrelated allocations performed by guard and expression checking.
*/
