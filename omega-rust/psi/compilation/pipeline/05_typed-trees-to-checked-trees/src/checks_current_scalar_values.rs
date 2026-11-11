/*
    Current scalar values and descriptor measures across statement transfers.

    A hypothesis captures its input roster when it is established. Assignment
    captures the RHS under that roster before replacing its destination; later
    reads never reinterpret an earlier name as the replacement. This is private
    proof construction scratch, not an invocation-memory snapshot or a retained
    certificate. Arrivals, exits and ranks share this transfer. Unknown effects
    retire current bindings; they cannot revive an entry fact about new storage.
    Saved Booleans retain conditional propositions under that same captured
    roster. Guard selection consumes one polarity without rereading its numeric
    sources; Boolean replacement retires the association like any other store.

    A fixed array's extent is metadata for every reference access and every
    qualification. A stable slice descriptor retains its observed length under
    the existing access and effect checks. Local descriptors use the same
    replacement and effect retirement as scalar bindings; their captured
    measures survive endpoint stores, never descriptor replacement.

    A match result has one length only when every arm's length is the same
    observation, or each arm folds to the same exact integer. Differing arms
    stay unknown rather than publishing one arm's extent. A plain record
    value captures each slice field's length when that value is transferred;
    a later overlapping write retires the affected field observations. A returned
    record publishes the same field lengths on `result.field`, including when every match arm
    builds that field from an equal window. An `ensures` bound that names a
    parameter is that call argument's value, so `result.values.len == count`
    arrives as the length the caller passed.

    An exact or trapping integer cast denotes its operand when the operand's
    carrier already fits in the target, or the live facts at that statement
    prove it does. Wrapping and saturating casts stay outside this equality:
    they are different values. The window length `end - start` can then meet
    `count as u64` for a nonnegative wrapping count, through a local, a field,
    a parameter, or a call, because each spelling is that same integer.
*/

use super::ranges::types::expression_integer_carrier_maximum;
use super::ranges::types::expression_integer_carrier_minimum;
use crate::checks::unsigned;
use crate::flow::expressions;
use crate::values::scalar::structural_fields;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetHandle};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{machine::Machine, state::State};
use validation::BigIntInterval;
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
};

mod application_values {
    /*
        Read a dependent application recipe under completed caller operands. A
        structural operand supplies captured projections, never a fresh observation
        of a same-named callee parameter or the caller's later storage version.
    */
    use super::{CurrentScalarValues, descriptor_storage_path, field_storage_path};
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use validation::{
        ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
        ScopedArithmeticValue,
    };

    impl CurrentScalarValues<'_> {
        pub(crate) fn application_value_with_captured(
            &self,
            expression: ExpressionHandle,
            arguments: &[(SymbolHandle, Option<ExpressionHandle>)],
            captured: &[(SymbolHandle, Option<ScopedArithmeticValue>)],
        ) -> Option<ScopedArithmeticExpression> {
            if arguments.is_empty() && captured.is_empty() {
                return self.current(expression);
            }
            let mut bindings = Vec::new();
            let mut pending = vec![expression];
            let mut remaining = 4096usize;
            let mut owner = arguments
                .first()
                .map_or(self.state.symbol, |(parameter, _)| {
                    self.program.symbols.get(*parameter).parent
                });
            while let Some(occurrence) = pending.pop() {
                remaining = remaining.checked_sub(1)?;
                match self.program.expression_table.expression(occurrence) {
                    ExpressionNode::Name(path) => {
                        let value = if let Some((_, value)) =
                            captured.iter().find(|(formal, _)| *formal == path.symbol)
                        {
                            owner = self.program.symbols.get(path.symbol).parent;
                            value.clone()?
                        } else if let Some((_, actual)) =
                            arguments.iter().find(|(formal, _)| *formal == path.symbol)
                        {
                            owner = self.program.symbols.get(path.symbol).parent;
                            ScopedArithmeticValue::Term(self.current((*actual)?)?)
                        } else {
                            self.current_symbol(path.symbol)?
                        };
                        bindings.push(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Symbol(path.symbol),
                            value,
                        });
                    }
                    ExpressionNode::Member(member) => {
                        let (root, receiver_path) =
                            descriptor_storage_path(self.program, member.receiver)?;
                        let Some((_, actual)) =
                            arguments.iter().find(|(formal, _)| *formal == root)
                        else {
                            bindings.push(ScopedArithmeticBinding {
                                binder: ScopedArithmeticBinder::Occurrence(occurrence),
                                value: ScopedArithmeticValue::Term(self.current(occurrence)?),
                            });
                            continue;
                        };
                        let actual = (*actual)?;
                        owner = self.program.symbols.get(root).parent;
                        let parameters = validation::call_target_parameters(self.program, owner)?;
                        validation::parameter_source_type_application(
                            self.program,
                            owner,
                            parameters,
                            member.receiver,
                        )?;
                        let receiver = validation::parameter_expression_result_type_reference(
                            self.program,
                            owner,
                            parameters,
                            member.receiver,
                        )?;
                        let value = if validation::collection_operations::collection_measure_member(
                            self.program,
                            member,
                            validation::collection_operations::MeasureReceiver::Declared(receiver),
                        ) == Some(
                            language_semantics::declaration_selection::CollectionMeasure::Length,
                        ) {
                            self.projected_collection_length(actual, &receiver_path)?
                        } else {
                            let (_, mut path) = field_storage_path(self.program, member.receiver)?;
                            path.push(member.member_symbol);
                            self.captured_scalar_fields(actual, 0)
                                .into_iter()
                                .find(|field| {
                                    field.path
                                        == path
                                            .iter()
                                            .map(|symbol| super::ScalarProjection::Field(*symbol))
                                            .collect::<Vec<_>>()
                                })?
                                .value
                        };
                        bindings.push(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Occurrence(occurrence),
                            value,
                        });
                    }
                    ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
                    ExpressionNode::Binary(binary) => {
                        if !validation::has_builtin_parameter_bound_expression_meaning(
                            self.program,
                            owner,
                            &[],
                            occurrence,
                        ) {
                            return None;
                        }
                        pending.push(binary.left);
                        pending.push(binary.right);
                    }
                    _ => return None,
                }
            }
            Some(ScopedArithmeticExpression {
                owner,
                expression,
                bindings,
            })
        }
    }
}
mod call_contracts {
    /*
        Call contracts bind both machine value coordinates and ordinary operands.
        Each actual is a current snapshot after the caller's ordered prefix; unknown
        writes withdraw addressable captures, while invocation indices remain exact.
        Requires use entry substitution; completed invocations transport guarantees
        through the shared public-contract owner and its post-state formal roster.
    */
    use super::{
        Binding, CurrentScalarValues, Length, LengthSubject, ScalarField, admitted, fixed_length,
        integer_carrier, length_occurrence, literal_window_length, scalar_field_occurrence,
        scoped_operations,
    };
    use crate::flow::expressions;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, StaticMachineArgument};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::types::TypeReferenceNode;
    use validation::BigIntInterval;
    use validation::{
        CallSite, ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
        ScopedArithmeticValue,
    };

    /*
        One state pass's completed-call hypotheses. Proving a call's
        precondition replays the caller's prefix, and the replay re-derived
        every earlier completed call's guarantees, so a state with n calls paid
        n^2 derivations. The pass lends one of these to each replay instead.

        `scoped_completed_call_guarantees` is a function of the frozen program
        and the inputs a row records: the caller machine and state, statement,
        call site, captured actuals, machine values and result. Its result
        observations are a function of the program, machine, state and site
        expression. A row answers only when every recorded input is equal to
        the current one; otherwise the hypotheses are derived again and replace
        the row. Nothing outlives the state's pass.
    */
    #[derive(Default)]
    pub(crate) struct CompletedCallGuarantees {
        /* Indexed by statement; few rows each. */
        by_statement: Vec<Vec<CompletedCall>>,
    }

    struct CompletedCall {
        machine: SymbolHandle,
        state: SymbolHandle,
        site: CompletedSite,
        actuals: Vec<ScopedArithmeticBinding>,
        machine_values: Vec<(SymbolHandle, ScopedArithmeticValue)>,
        result: Option<ScopedArithmeticValue>,
        hypotheses: Vec<validation::ScopedArithmeticHypothesis>,
    }

    /* A call site by identity: its expression, or its statement call row. */
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum CompletedSite {
        Expression(ExpressionHandle),
        /* The row's address in the frozen statement table. */
        Statement(usize),
    }

    impl CompletedSite {
        /* Named transitions have no row identity here and are never reused. */
        pub(super) fn of(site: CallSite<'_>) -> Option<Self> {
            match site {
                CallSite::Expression { expression, .. } => Some(Self::Expression(expression)),
                CallSite::Statement(call) => {
                    Some(Self::Statement(std::ptr::from_ref(call) as usize))
                }
                CallSite::TransitionNamed { .. } => None,
            }
        }
    }

    pub(super) struct CompletedCallInputs<'inputs> {
        pub(super) machine: SymbolHandle,
        pub(super) state: SymbolHandle,
        pub(super) statement_index: usize,
        pub(super) site: CompletedSite,
        pub(super) actuals: &'inputs [ScopedArithmeticBinding],
        pub(super) machine_values: &'inputs [(SymbolHandle, ScopedArithmeticValue)],
        pub(super) result: &'inputs Option<ScopedArithmeticValue>,
    }

    impl CompletedCallGuarantees {
        pub(super) fn hypotheses(
            &mut self,
            inputs: CompletedCallInputs<'_>,
            derive: impl FnOnce() -> Vec<validation::ScopedArithmeticHypothesis>,
        ) -> &[validation::ScopedArithmeticHypothesis] {
            if self.by_statement.len() <= inputs.statement_index {
                self.by_statement
                    .resize_with(inputs.statement_index + 1, Vec::new);
            }
            let rows = &mut self.by_statement[inputs.statement_index];
            let keyed = rows.iter().position(|row| {
                row.machine == inputs.machine
                    && row.state == inputs.state
                    && row.site == inputs.site
            });
            let index = match keyed {
                Some(index)
                    if rows[index].actuals == inputs.actuals
                        && rows[index].machine_values == inputs.machine_values
                        && rows[index].result == *inputs.result =>
                {
                    index
                }
                keyed => {
                    let row = CompletedCall {
                        machine: inputs.machine,
                        state: inputs.state,
                        site: inputs.site,
                        actuals: inputs.actuals.to_vec(),
                        machine_values: inputs.machine_values.to_vec(),
                        result: inputs.result.clone(),
                        hypotheses: derive(),
                    };
                    match keyed {
                        Some(index) => {
                            rows[index] = row;
                            index
                        }
                        None => {
                            rows.push(row);
                            rows.len() - 1
                        }
                    }
                }
            };
            &rows[index].hypotheses
        }
    }

    struct CapturedParameterGoal {
        goal: ScopedArithmeticExpression,
        /* Exact actuals aligned with goal bindings; non-parameter rows are absent. */
        arguments_by_binding: Vec<ExpressionHandle>,
    }

    impl CurrentScalarValues<'_> {
        pub(crate) fn bind_call_parameters(
            &self,
            callee: &Machine,
            state: &State,
            expression: ExpressionHandle,
            parameters: &[typed_trees::signature::StateParameter],
            arguments: &[ExpressionHandle],
            site: CallSite<'_>,
        ) -> Option<ScopedArithmeticExpression> {
            let machine_values = self.machine_value_bindings(callee, site)?;
            let captured = self.capture_parameter_goal(
                callee,
                state,
                expression,
                parameters,
                arguments,
                &machine_values,
                &[],
                false,
            )?;
            Some(self.enrich_parameter_goal(captured))
        }

        /*
            Capture a declaration recipe under the same simultaneous parameter
            substitution as requires, leaving its operations to formation.
        */
        pub(crate) fn bind_integer_expression_parameters(
            &self,
            callee: &Machine,
            state: &State,
            expression: ExpressionHandle,
            arguments: &[ExpressionHandle],
        ) -> Option<ScopedArithmeticExpression> {
            self.bind_integer_expression_formals(
                callee,
                state,
                expression,
                self.program.state_parameters(state),
                arguments,
            )
        }

        /*
            The same capture over an explicit subset of the declaring state's
            formals. A formal left out of the roster stays unbound, so a recipe
            that reads it fails to capture rather than reading another value.
        */
        pub(crate) fn bind_integer_expression_formals(
            &self,
            callee: &Machine,
            state: &State,
            expression: ExpressionHandle,
            parameters: &[typed_trees::signature::StateParameter],
            arguments: &[ExpressionHandle],
        ) -> Option<ScopedArithmeticExpression> {
            let captured = self.capture_parameter_goal(
                callee,
                state,
                expression,
                parameters,
                arguments,
                &[],
                &[],
                true,
            )?;
            Some(self.enrich_parameter_goal(captured))
        }

        /*
            The same substitution when a formal is a field of the actual rather than
            the actual itself. `paths` aligns with `parameters`; an empty path is
            the whole actual. A nonempty path selects that projection before the
            formal's length or scalar value is read.
        */
        pub(crate) fn bind_projected_formals(
            &self,
            callee: &Machine,
            state: &State,
            expression: ExpressionHandle,
            parameters: &[typed_trees::signature::StateParameter],
            arguments: &[ExpressionHandle],
            paths: &[Vec<facts::PlaceSegment>],
        ) -> Option<ScopedArithmeticExpression> {
            let captured = self.capture_parameter_goal(
                callee,
                state,
                expression,
                parameters,
                arguments,
                &[],
                paths,
                true,
            )?;
            Some(self.enrich_parameter_goal(captured))
        }

        /*
            Prove a simultaneous substitution from its independently captured actuals.
            Current facts may suffice before discovering extra occurrence bounds;
            every unsuccessful attempt retains the original enriched proof.
        */
        pub(crate) fn proves_parameter_substitution(
            &self,
            callee: &Machine,
            state: &State,
            expression: ExpressionHandle,
            parameters: &[typed_trees::signature::StateParameter],
            arguments: &[ExpressionHandle],
            site: CallSite<'_>,
        ) -> bool {
            let Some(machine_values) = self.machine_value_bindings(callee, site) else {
                return false;
            };
            let Some(captured) = self.capture_parameter_goal(
                callee,
                state,
                expression,
                parameters,
                arguments,
                &machine_values,
                &[],
                false,
            ) else {
                return false;
            };
            if self.proves(self.program, self.machine, &captured.goal) {
                return true;
            }
            let goal = self.enrich_parameter_goal(captured);
            self.proves(self.program, self.machine, &goal)
        }

        fn enrich_parameter_goal(
            &self,
            captured: CapturedParameterGoal,
        ) -> ScopedArithmeticExpression {
            let CapturedParameterGoal {
                mut goal,
                arguments_by_binding,
            } = captured;
            let bounds = validation::occurrence_integer_bounds(
                self.program,
                self.machine.symbol,
                self.state.symbol,
                self.statement_index,
                goal.bindings
                    .iter()
                    .zip(&arguments_by_binding)
                    .filter_map(|(binding, argument)| {
                        (argument.is_valid()
                            && !matches!(binding.value, ScopedArithmeticValue::Integer(_)))
                        .then_some(*argument)
                    })
                    .collect(),
            );
            goal.bindings = goal
                .bindings
                .into_iter()
                .zip(arguments_by_binding)
                .map(|(mut binding, argument)| {
                    if let Some((minimum, maximum)) = bounds.get(argument) {
                        binding.value = ScopedArithmeticValue::BoundedInteger {
                            value: Box::new(binding.value),
                            interval: BigIntInterval {
                                low: Some(numerics::bignum::BigInt::from_i64(minimum)),
                                high: Some(numerics::bignum::BigInt::from_i64(maximum)),
                            },
                        };
                    }
                    binding
                })
                .collect();
            goal
        }

        pub(crate) fn machine_value_bindings(
            &self,
            callee: &Machine,
            site: CallSite<'_>,
        ) -> Option<Vec<(SymbolHandle, ScopedArithmeticValue)>> {
            let (application, explicit) = match site {
                CallSite::Statement(call) => (
                    call.authored_machine_application,
                    call.machine_arguments.as_ref(),
                ),
                CallSite::Expression { call, .. } => (
                    call.authored_machine_application,
                    call.machine_arguments.as_ref(),
                ),
                CallSite::TransitionNamed { .. } => (Default::default(), &[][..]),
            };
            let (source, arguments) = if application.is_valid() {
                let row = self
                    .program
                    .expression_table
                    .static_machine_application(application);
                (
                    self.program
                        .machine_by_own_or_state_symbol(row.registration_operation)?,
                    expressions(self.program).static_machine_application_arguments(application),
                )
            } else {
                (callee, explicit)
            };
            let parameters = self.program.machine_type_parameters(source);
            let internal = site.is_internal_state_transfer(self.program, self.machine.symbol);
            if !internal && parameters.len() != arguments.len() {
                let integer_static = parameters.iter().any(|parameter| {
                    let reference = match parameter.kind {
                        typed_trees::data::TypeParameterKind::Value { type_reference }
                        | typed_trees::data::TypeParameterKind::Const { type_reference } => {
                            type_reference
                        }
                        _ => return false,
                    };
                    integer_carrier(self.program, reference).is_some()
                });
                if integer_static || !arguments.is_empty() {
                    return None;
                }
            }
            let mut values = Vec::new();
            for (ordinal, parameter) in parameters.iter().enumerate() {
                let reference = match parameter.kind {
                    typed_trees::data::TypeParameterKind::Value { type_reference }
                    | typed_trees::data::TypeParameterKind::Const { type_reference } => {
                        type_reference
                    }
                    _ => continue,
                };
                if integer_carrier(self.program, reference).is_none() {
                    continue;
                }
                let value = if internal {
                    self.current_symbol(parameter.symbol)?
                } else {
                    self.static_argument_value(arguments.get(ordinal)?)?
                };
                values.push((parameter.symbol, value));
            }
            Some(values)
        }

        pub(crate) fn static_argument_value(
            &self,
            argument: &StaticMachineArgument,
        ) -> Option<ScopedArithmeticValue> {
            use language_semantics::const_value::{
                CanonicalConstValue, DecodedCanonicalConstValue,
            };
            use numerics::bignum::BigInt;
            if argument.application.is_some() || argument.evidence_projection.is_some() {
                return None;
            }
            if let Some(literal) = &argument.const_literal {
                return literal
                    .value_bignum()
                    .or_else(|| literal.value_i64().map(BigInt::from_i64))
                    .map(ScopedArithmeticValue::Integer);
            }
            let mut symbol = argument.symbol;
            if argument.type_reference.is_valid() {
                match self
                    .program
                    .type_reference_table
                    .type_reference(argument.type_reference)
                {
                    TypeReferenceNode::ConstExpression(expression) => {
                        return self.capture(self.program, self.machine, self.state, *expression);
                    }
                    TypeReferenceNode::Named {
                        symbol: selected, ..
                    } if selected.is_valid() => symbol = *selected,
                    TypeReferenceNode::Named {
                        symbol: selected,
                        name,
                    } if !selected.is_valid() => {
                        if let Ok(value) = name.as_str().parse::<i128>() {
                            return Some(ScopedArithmeticValue::Integer(BigInt::from_i128(value)));
                        }
                    }
                    _ => {}
                }
            }
            if symbol.is_valid() {
                if let Some(value) = self.current_symbol(symbol) {
                    return Some(value);
                }
                let declaration = self.program.const_declaration_by_symbol(symbol)?;
                let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
                return match CanonicalConstValue::new("", encoding.to_owned(), "")
                    .decode_encoding()?
                {
                    DecodedCanonicalConstValue::Integer { value, .. } => {
                        Some(ScopedArithmeticValue::Integer(BigInt::from_i128(value)))
                    }
                    _ => None,
                };
            }
            let spelling = typed_trees::typed_trees::static_const_argument_spelling(
                self.program.typed_program(),
                argument,
            )?;
            let value = if let Some(canonical) = CanonicalConstValue::from_atom(&spelling) {
                match canonical.decode_encoding()? {
                    DecodedCanonicalConstValue::Integer { value, .. } => value,
                    _ => return None,
                }
            } else {
                spelling.parse::<i128>().ok()?
            };
            Some(ScopedArithmeticValue::Integer(BigInt::from_i128(value)))
        }

        pub(super) fn capture_call_actuals(
            &self,
            expression: ExpressionHandle,
        ) -> Vec<ScopedArithmeticBinding> {
            let Some(call) = self.program.expression_table.call_node(expression) else {
                return Vec::new();
            };
            self.capture_site_actuals(CallSite::Expression { expression, call })
        }

        fn capture_site_actuals(&self, site: CallSite<'_>) -> Vec<ScopedArithmeticBinding> {
            validation::call_site_argument_expressions(self.program, &site)
                .iter()
                .filter_map(|argument| {
                    self.capture(self.program, self.machine, self.state, *argument)
                        .map(|value| ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Occurrence(*argument),
                            value,
                        })
                })
                .collect()
        }

        pub(super) fn assume_call_ensures(
            &mut self,
            expression: ExpressionHandle,
            result: &ScopedArithmeticValue,
            actuals: &[ScopedArithmeticBinding],
            statement_index: usize,
        ) {
            let Some(call) = self.program.expression_table.call_node(expression) else {
                return;
            };
            self.assume_completed_guarantees(
                CallSite::Expression { expression, call },
                actuals,
                Some(result.clone()),
                statement_index,
            );
        }

        pub(super) fn assume_aggregate_call_ensures(
            &mut self,
            expression: ExpressionHandle,
            actuals: &[ScopedArithmeticBinding],
            statement_index: usize,
        ) {
            if let Some(call) = self.program.expression_table.call_node(expression) {
                self.assume_completed_guarantees(
                    CallSite::Expression { expression, call },
                    actuals,
                    None,
                    statement_index,
                );
            }
        }

        pub(super) fn assume_statement_ensures(
            &mut self,
            call: &typed_trees::statement::TableCall,
            statement_index: usize,
        ) {
            let site = CallSite::Statement(call);
            let actuals = self.capture_site_actuals(site);
            self.assume_completed_guarantees(site, &actuals, None, statement_index);
        }

        fn assume_completed_guarantees(
            &mut self,
            site: CallSite<'_>,
            actuals: &[ScopedArithmeticBinding],
            result: Option<ScopedArithmeticValue>,
            statement_index: usize,
        ) {
            let target = site.target_symbol(self.program);
            let machine_values = if let Some((callee, _)) =
                validation::find_state(self.program, target)
                    .or_else(|| validation::find_machine_head(self.program, target))
            {
                let Some(values) = self.machine_value_bindings(callee, site) else {
                    return;
                };
                values
            } else {
                let has_static_arguments = match site {
                    CallSite::Statement(call) => !call.machine_arguments.is_empty(),
                    CallSite::Expression { call, .. } => !call.machine_arguments.is_empty(),
                    CallSite::TransitionNamed { .. } => return,
                };
                if has_static_arguments {
                    return;
                }
                Vec::new()
            };
            let (program, machine, state) = (self.program, self.machine, self.state);
            let derive = |result: Option<ScopedArithmeticValue>| {
                let result_observations = match site {
                    CallSite::Expression { expression, .. } if result.is_none() => {
                        validation::scoped_call_result_observations(
                            program, machine, state, expression,
                        )
                    }
                    _ => Vec::new(),
                };
                validation::scoped_completed_call_guarantees(
                    program,
                    machine,
                    validation::ScopedCallCompletion {
                        state,
                        statement_index,
                        site,
                        actuals,
                        machine_values: &machine_values,
                        result,
                        result_observations: &result_observations,
                    },
                )
            };
            let Some(key) = CompletedSite::of(site) else {
                for hypothesis in derive(result) {
                    crate::shared::collections::push_unique(&mut self.hypotheses, hypothesis);
                }
                return;
            };
            let hypotheses = self.completed_calls.hypotheses(
                CompletedCallInputs {
                    machine: machine.symbol,
                    state: state.symbol,
                    statement_index,
                    site: key,
                    actuals,
                    machine_values: &machine_values,
                    result: &result,
                },
                || derive(result.clone()),
            );
            for hypothesis in hypotheses {
                crate::shared::collections::push_unique(&mut self.hypotheses, hypothesis.clone());
            }
        }

        /*
            Simultaneous parameter substitution with independently retained actual
            snapshots. The caller owns selection and exact formal/actual roster
            identity; this owner checks arithmetic meaning before exposing a term.
        */
        fn capture_parameter_goal(
            &self,
            callee: &Machine,
            state: &State,
            expression: ExpressionHandle,
            parameters: &[typed_trees::signature::StateParameter],
            arguments: &[ExpressionHandle],
            machine_values: &[(SymbolHandle, ScopedArithmeticValue)],
            paths: &[Vec<facts::PlaceSegment>],
            form_operations: bool,
        ) -> Option<CapturedParameterGoal> {
            if parameters.len() != arguments.len() {
                return None;
            }
            let mut formals = Vec::with_capacity(parameters.len());
            let mut lengths = Vec::new();
            let mut fields = Vec::new();
            for (index, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
                let path = paths.get(index).map(Vec::as_slice).unwrap_or(&[]);
                let Some(_primitive) = integer_carrier(self.program, parameter.type_reference)
                else {
                    let measured = if path.is_empty() {
                        self.collection_length(*argument, 0)
                    } else {
                        self.projected_collection_length(*argument, path)
                    };
                    if let Some(value) = measured
                        && fixed_length(self.program, parameter.type_reference, String::new)
                            .is_some()
                        && (!path.is_empty()
                            || validation::state_reference_parameter_binding_is_stable(
                                self.program,
                                callee,
                                state,
                                parameter.symbol,
                            )
                            || matches!(
                                value,
                                ScopedArithmeticValue::Integer(_)
                                    | ScopedArithmeticValue::BoundedInteger { .. }
                                    | ScopedArithmeticValue::IntegerOperation { .. }
                            ))
                    {
                        lengths.push(Length {
                            subject: LengthSubject::Storage(parameter.symbol),
                            name: parameter.name.to_string(),
                            stable: true,
                            type_reference: parameter.type_reference,
                            value: Some(value),
                        });
                    }
                    if path.is_empty() {
                        for field in self.captured_scalar_fields(*argument, 0) {
                            fields.push(ScalarField::for_owner(parameter.symbol, field));
                        }
                        for field in self.captured_field_lengths(*argument, 0) {
                            lengths.push(Length {
                                subject: LengthSubject::Field {
                                    owner: parameter.symbol,
                                    path: field.path,
                                },
                                name: format!("{}.{}", parameter.name, field.name),
                                stable: true,
                                type_reference: field.type_reference,
                                value: Some(field.value),
                            });
                        }
                    }
                    continue;
                };
                let value = if path.is_empty() {
                    self.capture(self.program, self.machine, self.state, *argument)
                } else {
                    self.projected_scalar_value_in_call(
                        *argument, path, callee, state, parameters, arguments,
                    )
                };
                if !path.is_empty() && value.is_none() {
                    return None;
                }
                formals.push(Binding {
                    symbol: parameter.symbol,
                    name: parameter.name.to_string(),
                    value,
                    alternatives: Vec::new(),
                    immutable_coordinate: false,
                    unwritable: false,
                });
            }
            formals.extend(machine_values.iter().map(|(symbol, value)| Binding {
                symbol: *symbol,
                name: self.program.symbols.name(*symbol).to_owned(),
                value: Some(value.clone()),
                alternatives: Vec::new(),
                immutable_coordinate: true,
                unwritable: true,
            }));
            let occurrence = self.occurrence_at(self.program, callee, state, &lengths);
            if !form_operations && !admitted(occurrence, expression, &formals, &fields, 0) {
                return None;
            }
            let mut goal =
                scoped_operations(occurrence, expression, &formals, &fields, form_operations)?;
            let mut captured_actuals = Vec::new();
            for (binding_index, binding) in goal.bindings.iter_mut().enumerate() {
                if let ScopedArithmeticBinder::Occurrence(occurrence) = binding.binder
                    && length_occurrence(self.program, callee, state, occurrence, &lengths)
                        .is_none()
                    && scalar_field_occurrence(
                        self.occurrence_at(self.program, callee, state, &lengths),
                        occurrence,
                        &formals,
                        &fields,
                    )
                    .is_none()
                    && literal_window_length(self.program, occurrence).is_none()
                {
                    return None;
                }
                if let ScopedArithmeticBinder::Symbol(symbol) = binding.binder {
                    if let Some((_, actual)) =
                        machine_values.iter().find(|(formal, _)| *formal == symbol)
                    {
                        binding.value = actual.clone();
                        continue;
                    }
                    let position = parameters
                        .iter()
                        .position(|parameter| parameter.symbol == symbol)?;
                    let captured =
                        self.capture(self.program, self.machine, self.state, arguments[position])?;
                    captured_actuals.push((binding_index, arguments[position], captured));
                }
            }
            let mut arguments_by_binding = vec![ExpressionHandle::invalid(); goal.bindings.len()];
            for (binding_index, argument, captured) in captured_actuals {
                goal.bindings[binding_index].value = captured;
                arguments_by_binding[binding_index] = argument;
            }
            Some(CapturedParameterGoal {
                goal,
                arguments_by_binding,
            })
        }
    }

    #[cfg(test)]
    mod completed_call_tests {
        use super::{CompletedCallGuarantees, CompletedCallInputs, CompletedSite};
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;
        use validation::{
            ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
            ScopedArithmeticHypothesis, ScopedArithmeticValue,
        };

        fn hypothesis(holds: bool) -> ScopedArithmeticHypothesis {
            ScopedArithmeticHypothesis {
                proposition: ScopedArithmeticExpression {
                    owner: SymbolHandle::invalid(),
                    expression: ExpressionHandle::invalid(),
                    bindings: Vec::new(),
                },
                holds,
            }
        }

        #[test]
        fn completed_call_rows_answer_only_equal_inputs() {
            let actual = |value: i128| {
                vec![ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Result,
                    value: ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_i128(
                        value,
                    )),
                }]
            };
            let inputs = |actuals| CompletedCallInputs {
                machine: SymbolHandle::invalid(),
                state: SymbolHandle::invalid(),
                statement_index: 3,
                site: CompletedSite::Expression(ExpressionHandle::invalid()),
                actuals,
                machine_values: &[],
                result: &None,
            };
            let mut memo = CompletedCallGuarantees::default();
            let mut derived = 0;
            let (one, two) = (actual(1), actual(2));
            let first = memo
                .hypotheses(inputs(&one), || {
                    derived += 1;
                    vec![hypothesis(true)]
                })
                .to_vec();
            let again = memo
                .hypotheses(inputs(&one), || unreachable!("equal inputs reuse the row"))
                .to_vec();
            assert_eq!(
                (first, again, derived),
                (vec![hypothesis(true)], vec![hypothesis(true)], 1)
            );
            let changed = memo
                .hypotheses(inputs(&two), || vec![hypothesis(false)])
                .to_vec();
            assert_eq!(changed, vec![hypothesis(false)]);
            let replaced = memo
                .hypotheses(inputs(&one), || vec![hypothesis(true), hypothesis(false)])
                .to_vec();
            assert_eq!(
                replaced.len(),
                2,
                "a changed row is replaced, not kept beside"
            );
        }
    }
}
mod collections {
    /*
        Descriptor observations use the same captured roster as scalar values.
        A range's endpoints are captured when the descriptor is formed; later
        endpoint stores cannot enlarge that descriptor. Construction supplies no
        bounds authority: selected builtin meaning and the current hypotheses must
        independently establish the range. Returning a descriptor binds its actual
        observation to the exact contract owner, never to the backing capacity.
    */
    use super::{CurrentScalarValues, Length, LengthSubject, fixed_length, integer_carrier};
    use symbols::SymbolHandle;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
    };
    use typed_trees::types::TypeReferenceHandle;
    use validation::BigIntInterval;
    use validation::{
        ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
        ScopedArithmeticValue,
    };

    pub(super) struct CapturedFieldLength {
        pub(super) path: Vec<facts::PlaceSegment>,
        pub(super) name: String,
        pub(super) type_reference: TypeReferenceHandle,
        pub(super) value: ScopedArithmeticValue,
    }

    impl CurrentScalarValues<'_> {
        pub(super) fn capture_collection(
            &mut self,
            symbol: SymbolHandle,
            name: &str,
            type_reference: TypeReferenceHandle,
            expression: ExpressionHandle,
            statement: usize,
        ) {
            self.lengths
                .retain(|length| length.subject != LengthSubject::Storage(symbol));
            if fixed_length(self.program, type_reference, String::new).is_some() {
                let value = self.collection_length(expression, 0);
                self.lengths.push(Length {
                    subject: LengthSubject::Storage(symbol),
                    name: name.to_owned(),
                    stable: false,
                    type_reference,
                    value: Some(value.unwrap_or_else(|| ScopedArithmeticValue::Atom {
                        identity: format!(
                            "state:{:?}:store:{statement}:descriptor:{symbol:?}:occurrence:{expression:?}:length",
                            self.state.symbol
                        ),
                        unsigned: true,
                    })),
                });
            }
        }

        pub(crate) fn byte_length_bound(&self, expression: ExpressionHandle) -> Option<u64> {
            let value = self.collection_length(expression, 0)?;
            let interval = validation::scoped_arithmetic_value_bounds(
                self.program,
                self.machine,
                &self.hypotheses,
                &value,
            )?;
            interval.high?.to_u64()
        }

        /*
            The arrival observation remains bound to this formal even if its
            descriptor is subsequently replaced. Current operands use the ordinary
            transfer roster below, so replacement cannot manufacture descent.
        */
        pub(crate) fn initial_collection_length(
            &self,
            parameter: SymbolHandle,
        ) -> Option<ScopedArithmeticValue> {
            self.initial_lengths
                .iter()
                .find(|length| length.subject == LengthSubject::Storage(parameter))?
                .value
                .clone()
        }

        /*
            The current frame owns exact descriptor observations. Missing retained
            geometry is not an unknown capacity that can authorize an entry read.
        */
        pub(super) fn coordinate_collection_length(
            &self,
            owner: SymbolHandle,
            path: &[facts::PlaceSegment],
        ) -> Option<ScopedArithmeticValue> {
            let subject = if path.is_empty() {
                LengthSubject::Storage(owner)
            } else {
                LengthSubject::Field {
                    owner,
                    path: super::static_descriptor_path(path).then(|| path.to_vec())?,
                }
            };
            self.lengths
                .iter()
                .find(|length| length.subject == subject)?
                .value
                .clone()
        }

        pub(crate) fn current_collection_length(
            &self,
            expression: ExpressionHandle,
        ) -> Option<ScopedArithmeticValue> {
            self.collection_length(expression, 0)
        }

        /*
            Inline descriptor fields carry their own extent at this invocation.
            The selected frame decides whether this is an entry observation or a
            current association that survived the executed prefix.
        */
        pub(crate) fn length_field_observations(
            &self,
            parameter: SymbolHandle,
        ) -> impl Iterator<Item = (&[facts::PlaceSegment], &ScopedArithmeticValue)> {
            self.lengths.iter().filter_map(move |length| {
                let LengthSubject::Field { owner, path } = &length.subject else {
                    return None;
                };
                (*owner == parameter)
                    .then_some(length.value.as_ref())?
                    .map(|value| (path.as_slice(), value))
            })
        }

        /*
            Project the destination's selected descriptor from the completed
            actual. Use the same captured roster as ordinary member reads, so
            copies, constructors, joins and replacement share one extent owner.
        */
        pub(crate) fn projected_collection_length(
            &self,
            actual: ExpressionHandle,
            path: &[facts::PlaceSegment],
        ) -> Option<ScopedArithmeticValue> {
            if path.is_empty() {
                return self.current_collection_length(actual);
            }
            self.captured_field_lengths(actual, 0)
                .into_iter()
                .find(|field| field.path == path)
                .map(|field| field.value)
        }

        pub(super) fn collection_length(
            &self,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Option<ScopedArithmeticValue> {
            if depth >= 64
                || !self
                    .program
                    .expression_table
                    .expression_is_valid(expression)
            {
                return None;
            }
            if validation::value_has_builtin_coordinates(
                self.program,
                self.machine,
                self.state,
                expression,
            ) && let Some((owner, path)) =
                super::descriptor_storage_path(self.program, expression)
                && !path.is_empty()
                && let Some(value) = self.coordinate_collection_length(owner, &path)
            {
                return Some(value);
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Borrow(borrow) => self.collection_length(borrow.target, depth + 1),
                ExpressionNode::Name(name) if name.symbol == name.head_symbol => self
                    .lengths
                    .iter()
                    .find(|length| length.subject == LengthSubject::Storage(name.symbol))
                    .and_then(|length| length.value.clone())
                    .or_else(|| self.declared_extent(expression)),
                ExpressionNode::Indexed(indexed)
                    if validation::has_builtin_subslice_meaning(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    ) || validation::range_operator_establishes_endpoint_extent(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    ) =>
                {
                    self.window_length(expression, indexed, depth, None, None)
                }
                ExpressionNode::Match(dispatch) => self.matched_length(dispatch, depth),
                ExpressionNode::Member(member) => self
                    .saved_field_length(member, depth)
                    .or_else(|| {
                        super::field_collection_length(
                            self.program,
                            self.machine,
                            self.state,
                            expression,
                        )
                    })
                    .or_else(|| self.declared_extent(expression)),
                ExpressionNode::Call(call) => {
                    if let Some(length) = self.ensured_range_call_length(expression) {
                        return Some(length);
                    }
                    let parameters =
                        validation::call_target_parameters(self.program, call.target_symbol)?;
                    let arguments = self
                        .program
                        .expression_table
                        .expression_handles(call.arguments);
                    if parameters.len() != arguments.len() {
                        return None;
                    }
                    let mut bindings = Vec::new();
                    for (parameter, argument) in parameters.iter().zip(arguments) {
                        let value =
                            if integer_carrier(self.program, parameter.type_reference).is_some() {
                                self.capture(self.program, self.machine, self.state, *argument)
                            } else if !parameter.is_mutable
                                && fixed_length(self.program, parameter.type_reference, String::new)
                                    .is_some()
                            {
                                self.collection_length(*argument, depth + 1)
                            } else {
                                None
                            };
                        if let Some(value) = value {
                            bindings.push(ScopedArithmeticBinding {
                                binder: ScopedArithmeticBinder::Occurrence(*argument),
                                value,
                            });
                        }
                    }
                    Some(ScopedArithmeticValue::CollectionLength(
                        ScopedArithmeticExpression {
                            owner: self.state.symbol,
                            expression,
                            bindings,
                        },
                    ))
                }
                _ => super::field_collection_length(
                    self.program,
                    self.machine,
                    self.state,
                    expression,
                )
                .or_else(|| self.declared_extent(expression)),
            }
        }

        /*
            A range call that ensures `result.len == end - start` publishes that
            difference as its length. The operator's requires already own ordering
            and source inclusion; this observation does not replay them.
        */
        fn ensured_range_call_length(
            &self,
            expression: ExpressionHandle,
        ) -> Option<ScopedArithmeticValue> {
            let (start, end) = validation::ensured_range_call_endpoints(self.program, expression)?;
            let start = self.capture(self.program, self.machine, self.state, start)?;
            let end = self.capture(self.program, self.machine, self.state, end)?;
            Some(ScopedArithmeticValue::IntegerOperation {
                operator: BinaryOperator::Subtract,
                left: Box::new(end),
                right: Box::new(start),
            })
        }

        /*
            Every arm contributes its own length. One shared observation is the
            result. Identical observations agree immediately. Arms that are not
            identical still agree when each folds to the same integer, so
            `row[0..2]` and `row[2..4]` both publish length 2. A value arm may use
            the subject: `match which { 0 -> row[0..which + 2] }` is length 2
            because that arm's subject is 0. The failed-pattern path does not
            receive that value. Arms whose lengths are proved equal publish the
            first arm's length, so `bytes[0..count]` and `bytes[16..16 + count]`
            both publish that length when `count` is one value. An arm without a
            length, or two arms with different lengths, publishes nothing.
        */
        fn matched_length(
            &self,
            dispatch: &TableMatchExpression,
            depth: usize,
        ) -> Option<ScopedArithmeticValue> {
            let arms = self.program.expression_table.match_arms(dispatch.arms);
            if arms.is_empty() {
                return None;
            }
            let subject = self.capture(self.program, self.machine, self.state, dispatch.subject);
            let mut lengths = Vec::new();
            for arm in arms {
                let replacement = match arm.pattern {
                    MatchPattern::Value(pattern) => self.pattern_integer(pattern),
                    MatchPattern::Destructure(_) | MatchPattern::Wildcard => None,
                };
                lengths.push(self.arm_length(
                    arm.value,
                    depth + 1,
                    subject.as_ref(),
                    replacement.as_ref(),
                )?);
            }
            if lengths.iter().skip(1).all(|length| length == &lengths[0]) {
                return Some(lengths[0].clone());
            }
            let folded: Option<Vec<_>> = lengths
                .iter()
                .map(|length| self.exact_length(length))
                .collect();
            if let Some(folded) = folded {
                let first = &folded[0];
                return folded
                    .iter()
                    .all(|length| length == first)
                    .then(|| ScopedArithmeticValue::Integer(first.clone()));
            }
            lengths
                .iter()
                .skip(1)
                .all(|length| {
                    self.proves_values_relation(
                        self.program,
                        self.machine,
                        BinaryOperator::Equal,
                        length,
                        &lengths[0],
                    )
                })
                .then(|| lengths[0].clone())
        }

        fn pattern_integer(&self, pattern: ExpressionHandle) -> Option<ScopedArithmeticValue> {
            if let ExpressionNode::Integer(literal) =
                self.program.expression_table.expression(pattern)
            {
                return Some(ScopedArithmeticValue::Integer(
                    numerics::bignum::BigInt::from_i64(literal.value_i64()?),
                ));
            }
            let captured = self.capture(self.program, self.machine, self.state, pattern)?;
            Some(ScopedArithmeticValue::Integer(
                self.exact_length(&captured)?,
            ))
        }

        pub(super) fn exact_length(
            &self,
            value: &ScopedArithmeticValue,
        ) -> Option<numerics::bignum::BigInt> {
            let interval = validation::scoped_arithmetic_value_bounds(
                self.program,
                self.machine,
                &self.hypotheses,
                value,
            )?;
            let low = interval.low?;
            let high = interval.high?;
            (low == high).then_some(low)
        }

        /*
            Slice fields of the returned record. A match publishes a field only
            when every arm builds that same field with one shared length.
        */
        pub(super) fn record_returned_field_lengths(&mut self, returned: ExpressionHandle) {
            for field in self.captured_field_lengths(returned, 0) {
                self.lengths.push(Length {
                    subject: LengthSubject::ResultField {
                        owner: self.machine.symbol,
                        path: field.path,
                    },
                    name: format!("result.{}", field.name),
                    stable: true,
                    type_reference: field.type_reference,
                    value: Some(field.value),
                });
            }
        }

        pub(super) fn captured_field_lengths(
            &self,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Vec<CapturedFieldLength> {
            if depth >= 64 {
                return Vec::new();
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::StructLiteral(literal) if literal.case_symbol.is_none() => {
                    self.literal_field_lengths(literal, depth + 1)
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    let mut fields = Vec::new();
                    for (index, value) in self
                        .program
                        .expression_table
                        .expression_handles(*elements)
                        .iter()
                        .enumerate()
                    {
                        self.append_child_lengths(
                            &mut fields,
                            *value,
                            facts::PlaceSegment::FixedIndex { index },
                            depth,
                        );
                    }
                    fields
                }
                ExpressionNode::Borrow(borrow) => {
                    self.captured_field_lengths(borrow.target, depth + 1)
                }
                ExpressionNode::Call(call) => self.call_field_lengths(expression, call),
                ExpressionNode::Member(member)
                    if super::descriptor_storage_path(self.program, expression).is_none() =>
                {
                    self.captured_field_lengths(member.receiver, depth + 1)
                        .into_iter()
                        .filter_map(|mut field| {
                            let facts::PlaceSegment::Field { symbol: selected } =
                                *field.path.first()?
                            else {
                                return None;
                            };
                            if field.path.len() <= 1 || !self.selects_field(member, selected) {
                                return None;
                            }
                            field.path.remove(0);
                            field.name = self.descriptor_path_name(&field.path)?;
                            Some(field)
                        })
                        .collect()
                }
                ExpressionNode::Name(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_) => {
                    if !validation::value_has_builtin_coordinates(
                        self.program,
                        self.machine,
                        self.state,
                        expression,
                    ) {
                        return Vec::new();
                    }
                    let Some((owner, prefix)) =
                        super::descriptor_storage_path(self.program, expression)
                    else {
                        return Vec::new();
                    };
                    self.lengths
                        .iter()
                        .filter_map(|length| {
                            let LengthSubject::Field {
                                owner: existing,
                                path,
                            } = &length.subject
                            else {
                                return None;
                            };
                            if *existing != owner
                                || !path.starts_with(&prefix)
                                || path.len() <= prefix.len()
                            {
                                return None;
                            }
                            let path = path[prefix.len()..].to_vec();
                            let name = self.descriptor_path_name(&path)?;
                            Some(CapturedFieldLength {
                                path,
                                name,
                                type_reference: length.type_reference,
                                value: length.value.clone()?,
                            })
                        })
                        .collect()
                }
                ExpressionNode::Match(dispatch) => {
                    let arms = self.program.expression_table.match_arms(dispatch.arms);
                    let mut joined: Option<Vec<CapturedFieldLength>> = None;
                    for arm in arms {
                        let fields = self.captured_field_lengths(arm.value, depth + 1);
                        joined = Some(match joined {
                            None => fields,
                            Some(existing) => existing
                                .into_iter()
                                .filter(|field| {
                                    fields.iter().any(|candidate| {
                                        candidate.path == field.path
                                            && self
                                                .slice_lengths_agree(&candidate.value, &field.value)
                                    })
                                })
                                .map(|mut field| {
                                    if let Some(length) = self.exact_length(&field.value) {
                                        field.value = ScopedArithmeticValue::Integer(length);
                                    }
                                    field
                                })
                                .collect(),
                        });
                    }
                    joined.unwrap_or_default()
                }
                _ => Vec::new(),
            }
        }

        fn literal_field_lengths(
            &self,
            literal: &typed_trees::expression::TableStructLiteral,
            depth: usize,
        ) -> Vec<CapturedFieldLength> {
            let mut fields = Vec::new();
            for field in self.program.expression_table.struct_fields(literal.fields) {
                if !field.field_symbol.is_valid() {
                    continue;
                }
                self.append_child_lengths(
                    &mut fields,
                    field.value,
                    facts::PlaceSegment::Field {
                        symbol: field.field_symbol,
                    },
                    depth,
                );
            }
            fields
        }

        /*
            Each constructor child contributes its own completed descriptor and
            descendant observations. Fields and array cells retain the same typed
            coordinate and current-value rules through every later transfer.
        */
        fn append_child_lengths(
            &self,
            fields: &mut Vec<CapturedFieldLength>,
            expression: ExpressionHandle,
            prefix: facts::PlaceSegment,
            depth: usize,
        ) {
            for mut nested in self.captured_field_lengths(expression, depth + 1) {
                nested.path.insert(0, prefix);
                if let Some(name) = self.descriptor_path_name(&nested.path) {
                    nested.name = name;
                    fields.push(nested);
                }
            }
            let Some(type_reference) = validation::expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                expression,
            ) else {
                return;
            };
            if fixed_length(self.program, type_reference, String::new).is_none() {
                return;
            }
            let Some(value) = self.collection_length(expression, depth + 1) else {
                return;
            };
            let value = self
                .exact_length(&value)
                .map(ScopedArithmeticValue::Integer)
                .unwrap_or(value);
            let path = vec![prefix];
            let Some(name) = self.descriptor_path_name(&path) else {
                return;
            };
            fields.push(CapturedFieldLength {
                path,
                name,
                type_reference,
                value,
            });
        }

        fn slice_lengths_agree(
            &self,
            left: &ScopedArithmeticValue,
            right: &ScopedArithmeticValue,
        ) -> bool {
            left == right
                || matches!(
                    (self.exact_length(left), self.exact_length(right)),
                    (Some(left), Some(right)) if left == right
                )
                || self.proves_values_relation(
                    self.program,
                    self.machine,
                    BinaryOperator::Equal,
                    left,
                    right,
                )
        }

        /*
            Length of one match arm. A window whose endpoint mentions the subject
            is measured after that arm's pattern replaces the subject. Other arms
            keep `collection_length`.
        */
        fn arm_length(
            &self,
            expression: ExpressionHandle,
            depth: usize,
            subject: Option<&ScopedArithmeticValue>,
            replacement: Option<&ScopedArithmeticValue>,
        ) -> Option<ScopedArithmeticValue> {
            if depth >= 64 || !expression.is_valid() {
                return None;
            }
            if let ExpressionNode::Borrow(borrow) =
                self.program.expression_table.expression(expression)
            {
                return self.arm_length(borrow.target, depth + 1, subject, replacement);
            }
            if let ExpressionNode::Match(dispatch) =
                self.program.expression_table.expression(expression)
            {
                return self.matched_length(dispatch, depth);
            }
            let Some(indexed) = self.program.expression_table.indexed_node(expression) else {
                return self.collection_length(expression, depth);
            };
            if !validation::has_builtin_subslice_meaning(
                self.program,
                self.machine,
                Some(self.state),
                expression,
            ) {
                return self.collection_length(expression, depth);
            }
            self.window_length(expression, indexed, depth, subject, replacement)
        }

        /*
            The length of one builtin subslice window: both endpoints applied to
            the source's length, an inclusive end adjusted by one, and the four
            ordered relations that make the span real. A range operator whose own
            definition ensures `result.len == end - start` has already discharged
            ordering and inclusion, so its window owes no second proof; a builtin
            spelling has no such contract. A match arm substitutes its pattern
            value for the subject at each endpoint; an ordinary capture passes none.
        */
        fn window_length(
            &self,
            expression: ExpressionHandle,
            indexed: &typed_trees::expression::TableIndexedExpression,
            depth: usize,
            subject: Option<&ScopedArithmeticValue>,
            replacement: Option<&ScopedArithmeticValue>,
        ) -> Option<ScopedArithmeticValue> {
            let (start, end) =
                self.window_bounds(expression, indexed, depth, subject, replacement)?;
            Some(ScopedArithmeticValue::IntegerOperation {
                operator: BinaryOperator::Subtract,
                left: Box::new(end),
                right: Box::new(start),
            })
        }

        /*
            The same formation decision supplies both descriptor length and exact
            retained element coordinates; neither consumer grants its own bounds.
        */
        pub(super) fn window_bounds(
            &self,
            expression: ExpressionHandle,
            indexed: &typed_trees::expression::TableIndexedExpression,
            depth: usize,
            subject: Option<&ScopedArithmeticValue>,
            replacement: Option<&ScopedArithmeticValue>,
        ) -> Option<(ScopedArithmeticValue, ScopedArithmeticValue)> {
            let range = self.program.expression_table.range_node(indexed.index)?;
            let source = self.collection_length(indexed.collection, depth + 1)?;
            let zero = ScopedArithmeticValue::Integer(numerics::bignum::BigInt::zero());
            let start = self.replaced_endpoint(range.start, &zero, subject, replacement)?;
            let authored_end = if range.end.is_valid() {
                self.replaced_endpoint(range.end, &source, subject, replacement)?
            } else {
                source.clone()
            };
            let end = if range.end_inclusive {
                if !range.end.is_valid() {
                    return None;
                }
                ScopedArithmeticValue::IntegerOperation {
                    operator: BinaryOperator::Add,
                    left: Box::new(authored_end.clone()),
                    right: Box::new(ScopedArithmeticValue::Integer(
                        numerics::bignum::BigInt::from_u64(1),
                    )),
                }
            } else {
                authored_end.clone()
            };
            if !validation::range_operator_establishes_endpoint_extent(
                self.program,
                self.machine,
                Some(self.state),
                expression,
            ) {
                for (left, right) in [
                    (&zero, &start),
                    (&start, &end),
                    (&end, &source),
                    (&zero, &authored_end),
                ] {
                    if !self.proves_values_relation(
                        self.program,
                        self.machine,
                        BinaryOperator::LessOrEqual,
                        left,
                        right,
                    ) {
                        return None;
                    }
                }
            }
            Some((start, end))
        }

        fn replaced_endpoint(
            &self,
            endpoint: ExpressionHandle,
            omitted: &ScopedArithmeticValue,
            subject: Option<&ScopedArithmeticValue>,
            replacement: Option<&ScopedArithmeticValue>,
        ) -> Option<ScopedArithmeticValue> {
            let captured = if endpoint.is_valid() {
                self.capture(self.program, self.machine, self.state, endpoint)?
            } else {
                omitted.clone()
            };
            let call_may_store_field = matches!(subject, Some(ScopedArithmeticValue::Atom { .. }))
                && endpoint.is_valid()
                && expression_contains_call(self.program, endpoint);
            Some(match (subject, replacement) {
                (Some(subject), Some(replacement)) if !call_may_store_field => {
                    substitute_matched_subject(&captured, subject, replacement)
                }
                _ => captured,
            })
        }

        /*
            Exit contracts name post-state formals. Only immutable scalar formals
            retain their entry argument value. The scoped arithmetic owner resolves
            selected operators and representability in the callee's declaration;
            this transport supplies captured arguments, not another evaluator.
        */
        fn substituted_length_bound(
            &self,
            bound: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            owner: SymbolHandle,
        ) -> Option<ScopedArithmeticValue> {
            let parameters = validation::call_target_parameters(self.program, call.target_symbol)?;
            let arguments = self
                .program
                .expression_table
                .expression_handles(call.arguments);
            if parameters.len() != arguments.len() {
                return None;
            }
            let bindings = parameters
                .iter()
                .zip(arguments)
                .filter_map(|(parameter, argument)| {
                    if parameter.is_mutable
                        || integer_carrier(self.program, parameter.type_reference).is_none()
                    {
                        return None;
                    }
                    Some(ScopedArithmeticBinding {
                        binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                        value: self.capture(self.program, self.machine, self.state, *argument)?,
                    })
                })
                .collect();
            Some(ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                owner,
                expression: bound,
                bindings,
            }))
        }

        /*
            Read result guarantees once at the exact completed call occurrence.

            A literal bound stays that integer. A bound that names a parameter, or
            exact arithmetic of such operands, is the corresponding argument value
            at this call. The callee already proved the guarantee; this substitution
            is how the caller receives it. A lower bound is published only when
            that value is one integer, so `len >= count` does not become `len == count`.
        */
        fn call_field_lengths(
            &self,
            occurrence: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
        ) -> Vec<CapturedFieldLength> {
            let Some((machine, _)) = validation::find_state(self.program, call.target_symbol)
                .or_else(|| validation::find_machine_head(self.program, call.target_symbol))
            else {
                return Vec::new();
            };
            let mut fields = Vec::new();
            for contract in self.program.machine_contracts(machine) {
                if contract.kind != typed_trees::signature::SignatureContractKind::Ensures {
                    continue;
                }
                for fact in self.program.proof_facts.span_or_empty(contract.facts) {
                    let Some(expression) = fact.as_expression() else {
                        continue;
                    };
                    let Some(parameters) =
                        validation::call_target_parameters(self.program, call.target_symbol)
                    else {
                        continue;
                    };
                    if !validation::has_builtin_parameter_bound_expression_meaning(
                        self.program,
                        machine.symbol,
                        parameters,
                        *expression,
                    ) {
                        continue;
                    }
                    let Some(comparison) = self.program.expression_table.binary_node(*expression)
                    else {
                        continue;
                    };
                    let candidates = match comparison.operator {
                        BinaryOperator::Equal => vec![
                            (comparison.left, comparison.right, false),
                            (comparison.right, comparison.left, false),
                        ],
                        BinaryOperator::GreaterOrEqual => {
                            vec![(comparison.left, comparison.right, true)]
                        }
                        BinaryOperator::LessOrEqual => {
                            vec![(comparison.right, comparison.left, true)]
                        }
                        _ => continue,
                    };
                    for (measure, bound, lower_bound) in candidates {
                        let Some(length) = self.program.expression_table.member_node(measure)
                        else {
                            continue;
                        };
                        let Some(place) =
                            validation::reserved_result_place(self.program, length.receiver)
                        else {
                            continue;
                        };
                        if !super::static_descriptor_path(&place.segments) {
                            continue;
                        }
                        let path = place.segments.clone();
                        let reference = place.type_reference;
                        if place.machine_symbol != machine.symbol
                            || path.is_empty()
                            || validation::collection_operations::collection_measure_member(
                                self.program,
                                length,
                                validation::collection_operations::MeasureReceiver::Declared(reference),
                            ) != Some(
                                language_semantics::declaration_selection::CollectionMeasure::Length,
                            )
                        {
                            continue;
                        }
                        let Some(value) =
                            self.substituted_length_bound(bound, call, machine.symbol)
                        else {
                            continue;
                        };
                        let Some(interval) = validation::scoped_arithmetic_value_bounds(
                            self.program,
                            self.machine,
                            &self.hypotheses,
                            &value,
                        ) else {
                            continue;
                        };
                        let value = match (interval.low, interval.high) {
                            (Some(low), Some(high)) if low == high => {
                                ScopedArithmeticValue::Integer(low)
                            }
                            _ => value,
                        };
                        let value = if lower_bound {
                            let ScopedArithmeticValue::Integer(minimum) = value else {
                                continue;
                            };
                            if minimum < numerics::bignum::BigInt::zero() {
                                continue;
                            }
                            ScopedArithmeticValue::BoundedInteger {
                                value: Box::new(ScopedArithmeticValue::Atom {
                                    identity: format!(
                                        "state:{:?}:call:{occurrence:?}:field:{path:?}:length",
                                        self.state.symbol
                                    ),
                                    unsigned: true,
                                }),
                                interval: BigIntInterval {
                                    low: Some(minimum),
                                    high: None,
                                },
                            }
                        } else {
                            value
                        };
                        let Some(name) = self.descriptor_path_name(&path) else {
                            continue;
                        };
                        fields.push(CapturedFieldLength {
                            path,
                            name,
                            type_reference: reference,
                            value,
                        });
                    }
                }
            }
            fields
        }

        /* Read only the current captured field roster, never declaration initializers. */
        fn saved_field_length(
            &self,
            member: &typed_trees::expression::TableMemberExpression,
            depth: usize,
        ) -> Option<ScopedArithmeticValue> {
            self.captured_field_lengths(member.receiver, depth + 1)
                .into_iter()
                .find(|field| field.path.len() == 1 && matches!(field.path[0], facts::PlaceSegment::Field { symbol } if self.selects_field(member, symbol)))
                .map(|field| field.value)
        }

        fn descriptor_path_name(&self, path: &[facts::PlaceSegment]) -> Option<String> {
            let mut name = String::new();
            for segment in path {
                match segment {
                    facts::PlaceSegment::Field { symbol } if symbol.is_valid() => {
                        if !name.is_empty() {
                            name.push('.');
                        }
                        name.push_str(self.program.symbols.name(*symbol));
                    }
                    facts::PlaceSegment::FixedIndex { index } => {
                        use std::fmt::Write;
                        write!(&mut name, "[{index}]").ok()?;
                    }
                    _ => return None,
                }
            }
            Some(name)
        }

        fn selects_field(
            &self,
            member: &typed_trees::expression::TableMemberExpression,
            field: SymbolHandle,
        ) -> bool {
            if member.member_symbol.is_valid() {
                member.member_symbol == field
            } else {
                self.program.symbols.name(field) == member.member.as_str()
            }
        }

        pub(super) fn capture_record_fields(
            &mut self,
            owner: SymbolHandle,
            path: &[facts::PlaceSegment],
            initializer: ExpressionHandle,
        ) {
            let fields = self.captured_field_lengths(initializer, 0);
            self.replace_record_fields(owner, path, fields);
        }

        pub(super) fn replace_record_fields(
            &mut self,
            owner: SymbolHandle,
            prefix: &[facts::PlaceSegment],
            fields: Vec<CapturedFieldLength>,
        ) {
            self.lengths.retain(|length| !matches!(&length.subject,
                LengthSubject::Field { owner: existing, path } if *existing == owner && path.starts_with(prefix)));
            for field in fields {
                let mut path = prefix.to_vec();
                path.extend(field.path);
                let coordinate = path.clone();
                let Some(name) =
                    validation::frame_length_coordinate_path(self.program, owner, &coordinate)
                else {
                    continue;
                };
                self.lengths.push(Length {
                    subject: LengthSubject::Field { owner, path },
                    name,
                    stable: false,
                    type_reference: field.type_reference,
                    value: Some(field.value),
                });
            }
        }

        fn declared_extent(&self, expression: ExpressionHandle) -> Option<ScopedArithmeticValue> {
            let extent = validation::declared_collection_extent(
                self.program,
                self.machine,
                Some(self.state),
                expression,
            )?;
            Some(ScopedArithmeticValue::Integer(
                numerics::bignum::BigInt::from_u64(u64::try_from(extent).ok()?),
            ))
        }
    }

    /*
        Replace the matched subject with the arm's pattern value. Only the
        selected arm's copy is rewritten; the subject's other occurrences stay.
    */
    fn substitute_matched_subject(
        value: &ScopedArithmeticValue,
        subject: &ScopedArithmeticValue,
        replacement: &ScopedArithmeticValue,
    ) -> ScopedArithmeticValue {
        if value == subject || same_symbol_term(value, subject) {
            return replacement.clone();
        }
        match value {
            ScopedArithmeticValue::IntegerOperation {
                operator,
                left,
                right,
            } => ScopedArithmeticValue::IntegerOperation {
                operator: *operator,
                left: Box::new(substitute_matched_subject(left, subject, replacement)),
                right: Box::new(substitute_matched_subject(right, subject, replacement)),
            },
            ScopedArithmeticValue::BoundedInteger { value, interval } => {
                ScopedArithmeticValue::BoundedInteger {
                    value: Box::new(substitute_matched_subject(value, subject, replacement)),
                    interval: interval.clone(),
                }
            }
            other => other.clone(),
        }
    }

    /*
        Two name occurrences of one parameter are different expression handles.
        They are the same subject when each term binds that parameter and nothing else.
    */
    fn same_symbol_term(value: &ScopedArithmeticValue, subject: &ScopedArithmeticValue) -> bool {
        let (ScopedArithmeticValue::Term(value), ScopedArithmeticValue::Term(subject)) =
            (value, subject)
        else {
            return false;
        };
        let value_symbols = symbol_binders(value);
        let subject_symbols = symbol_binders(subject);
        !subject_symbols.is_empty() && value_symbols == subject_symbols
    }

    fn expression_contains_call(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        if !expression.is_valid() {
            return false;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(_) => true,
            ExpressionNode::Binary(binary) => {
                expression_contains_call(program, binary.left)
                    || expression_contains_call(program, binary.right)
            }
            ExpressionNode::Unary(unary) => expression_contains_call(program, unary.operand),
            ExpressionNode::Member(member) => expression_contains_call(program, member.receiver),
            ExpressionNode::Borrow(borrow) => expression_contains_call(program, borrow.target),
            ExpressionNode::Indexed(indexed) => {
                expression_contains_call(program, indexed.collection)
                    || expression_contains_call(program, indexed.index)
            }
            ExpressionNode::Range(range) => {
                expression_contains_call(program, range.start)
                    || expression_contains_call(program, range.end)
            }
            _ => false,
        }
    }

    fn symbol_binders(expression: &ScopedArithmeticExpression) -> Vec<symbols::SymbolHandle> {
        expression
            .bindings
            .iter()
            .filter_map(|binding| match binding.binder {
                ScopedArithmeticBinder::Symbol(symbol) => Some(symbol),
                _ => None,
            })
            .collect()
    }
}
mod conditions {
    /*
        A stored Boolean retains conditional arithmetic observations at evaluation.
        The two polarities are consequences, not asserted facts. Selecting a guard
        imports only its polarity under the original captured scalar roster. Writes
        retire the Boolean's storage association, while writing an earlier numeric
        source cannot change the historical values already captured by that Boolean.
        Local bindings, record fields and copied records use the same transfer;
        unknown producers contribute no implication.
    */
    use super::{CurrentScalarValues, field_storage_path};
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::PrimitiveType;
    use validation::{ScopedArithmeticHypothesis, expression_result_type_reference};

    #[derive(Clone)]
    pub(super) struct ConditionBinding {
        pub(super) owner: SymbolHandle,
        pub(super) path: Vec<SymbolHandle>,
        when_true: Vec<ScopedArithmeticHypothesis>,
        when_false: Vec<ScopedArithmeticHypothesis>,
    }

    impl ConditionBinding {
        pub(super) fn hypotheses(&self, holds: bool) -> &[ScopedArithmeticHypothesis] {
            if holds {
                &self.when_true
            } else {
                &self.when_false
            }
        }
    }

    pub(super) struct CapturedCondition {
        path: Vec<SymbolHandle>,
        when_true: Vec<ScopedArithmeticHypothesis>,
        when_false: Vec<ScopedArithmeticHypothesis>,
    }

    impl CurrentScalarValues<'_> {
        pub(super) fn capture_conditions(
            &self,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Vec<CapturedCondition> {
            if depth >= 64 {
                return Vec::new();
            }
            let reference = expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                expression,
            );
            if reference.and_then(|reference| self.program.primitive_type_reference(reference))
                == Some(PrimitiveType::Bool)
            {
                let capture = |holds| {
                    self.capture_condition(
                        self.program,
                        self.machine,
                        self.state,
                        expression,
                        holds,
                        depth + 1,
                    )
                };
                let when_true = capture(true);
                let when_false = capture(false);
                return if when_true.is_empty() && when_false.is_empty() {
                    Vec::new()
                } else {
                    vec![CapturedCondition {
                        path: Vec::new(),
                        when_true,
                        when_false,
                    }]
                };
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::StructLiteral(literal) if literal.case_symbol.is_none() => {
                    let mut captured = Vec::new();
                    for field in self.program.expression_table.struct_fields(literal.fields) {
                        if !field.field_symbol.is_valid() {
                            continue;
                        }
                        for mut condition in self.capture_conditions(field.value, depth + 1) {
                            condition.path.insert(0, field.field_symbol);
                            captured.push(condition);
                        }
                    }
                    captured
                }
                ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                    let Some((owner, prefix)) = field_storage_path(self.program, expression) else {
                        return Vec::new();
                    };
                    self.conditions
                        .iter()
                        .filter(|condition| {
                            condition.owner == owner && condition.path.starts_with(&prefix)
                        })
                        .map(|condition| CapturedCondition {
                            path: condition.path[prefix.len()..].to_vec(),
                            when_true: condition.when_true.clone(),
                            when_false: condition.when_false.clone(),
                        })
                        .collect()
                }
                _ => Vec::new(),
            }
        }

        pub(super) fn replace_conditions(
            &mut self,
            owner: SymbolHandle,
            prefix: &[SymbolHandle],
            conditions: Vec<CapturedCondition>,
        ) {
            self.conditions.retain(|condition| {
                condition.owner != owner
                    || !(condition.path.starts_with(prefix) || prefix.starts_with(&condition.path))
            });
            for condition in conditions {
                let mut path = prefix.to_vec();
                path.extend(condition.path);
                self.conditions.push(ConditionBinding {
                    owner,
                    path,
                    when_true: condition.when_true,
                    when_false: condition.when_false,
                });
            }
        }
    }
}
mod declaration_reads {
    /*
        Rejoin a demanded declaration read with one admitted invocation observation.
        The declaration's selector uses its supplying telescope, while the selected
        field keeps its own storage, entry-bound and scalar-value receipts. Resolving
        a selector cannot invent an observation or grant access to its record.
    */
    use super::scalar_fields::{
        CapturedScalarField, ScalarField, capture_path, same_projection, source_path,
    };
    use super::{CurrentScalarValues, ScalarProjection};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use validation::{DeclaredInputRead, RankSubjectCoordinate, ScopedArithmeticValue};

    impl CurrentScalarValues<'_> {
        /*
            Entry rank observation may demand indexed declaration reads before any
            statement executes. Seed any missing scalar leaf the declared projection
            already resolves from entry storage and selector receipts.
        */
        pub(crate) fn seed_declared_read_entry_fields(
            &mut self,
            root: &RankSubjectCoordinate,
            reads: &[DeclaredInputRead],
        ) {
            for read in reads {
                let Some(projection) = self.declared_projection(root, read, &read.coordinate, 0)
                else {
                    continue;
                };
                if self.fields.iter().any(|field| {
                    field.owner == root.parameter
                        && field.carrier == read.primitive_type
                        && field.value.is_some()
                        && same_projection(self.occurrence(), &field.path, &projection)
                }) {
                    continue;
                }
                self.fields.push(ScalarField::for_owner(
                    root.parameter,
                    CapturedScalarField {
                        path: projection,
                        carrier: read.primitive_type,
                        value: ScopedArithmeticValue::Atom {
                            identity: format!(
                                "declared-read:parameter:{:?}:calls:{:?}",
                                root.parameter, read.calls
                            ),
                            unsigned: super::unsigned(read.primitive_type),
                        },
                    },
                ));
            }
        }

        pub(crate) fn declared_read_path(
            &self,
            root: &RankSubjectCoordinate,
            read: &DeclaredInputRead,
        ) -> Option<Vec<facts::PlaceSegment>> {
            let selected = self.declared_projection(root, read, &read.coordinate, 0)?;
            self.fields
                .iter()
                .find(|field| {
                    field.owner == root.parameter
                        && field.carrier == read.primitive_type
                        && field.value.is_some()
                        && same_projection(self.occurrence(), &field.path, &selected)
                })
                .map(|field| source_path(&field.path))
        }

        fn declared_projection(
            &self,
            root: &RankSubjectCoordinate,
            read: &DeclaredInputRead,
            coordinate: &RankSubjectCoordinate,
            depth: usize,
        ) -> Option<Vec<ScalarProjection>> {
            if depth >= 64 {
                return None;
            }
            if depth == 0 && coordinate.parameter != read.coordinate.parameter {
                return None;
            }
            let mut prefix = capture_path(&root.path, |selector| {
                super::scalar_fields::coordinates::selector_value(
                    self.occurrence(),
                    selector,
                    &self.current,
                    &self.fields,
                )
            })?;
            prefix.extend(capture_path(&coordinate.path, |selector| {
                self.declared_read_selector_value(read, root, depth, selector)
            })?);
            Some(prefix)
        }

        fn declared_read_selector_value(
            &self,
            read: &DeclaredInputRead,
            root: &RankSubjectCoordinate,
            depth: usize,
            selector: ExpressionHandle,
        ) -> Option<ScopedArithmeticValue> {
            let supplying = match self.program.expression_table.expression(selector) {
                ExpressionNode::Name(name) => read
                    .selector_arguments
                    .iter()
                    .find(|(formal, _)| *formal == name.symbol)
                    .map(|(_, actual)| *actual)
                    .unwrap_or(selector),
                _ => selector,
            };
            for candidate in [supplying, selector] {
                if let Some(value) = self
                    .telescoped_selector_leaf_value_from_expression(read, root, depth, candidate)
                {
                    return Some(value);
                }
                if let Some(value) = read.selector_value(self.program, candidate, &mut |input| {
                    self.telescoped_selector_leaf_value_from_coordinate(read, root, depth, input)
                }) {
                    return Some(value);
                }
                if let Some(value) = super::scalar_fields::coordinates::entry_selector_value(
                    self.occurrence(),
                    candidate,
                    &self.current,
                    &self.fields,
                ) {
                    return Some(value);
                }
            }
            read.selector_value(self.program, selector, &mut |input| {
                self.telescoped_selector_leaf_value_from_coordinate(read, root, depth, input)
            })
            .or_else(|| {
                let field_symbol =
                    read.coordinate
                        .path
                        .first()
                        .and_then(|segment| match segment {
                            facts::PlaceSegment::Field { symbol } => Some(*symbol),
                            _ => None,
                        })?;
                self.fields.iter().find_map(|field| {
                    if field.owner != root.parameter {
                        return None;
                    }
                    match field.path.as_slice() {
                        [
                            ScalarProjection::Field(symbol),
                            ScalarProjection::Element { selector, .. },
                        ] if *symbol == field_symbol => Some(selector.clone()),
                        _ => None,
                    }
                })
            })
            .or_else(|| {
                self.fields.iter().find_map(|field| {
                    if field.owner != root.parameter {
                        return None;
                    }
                    let ScalarProjection::Element {
                        selector: observed, ..
                    } = field.path.last()?
                    else {
                        return None;
                    };
                    let demanded = self.telescoped_selector_leaf_value_from_expression(
                        read, root, depth, supplying,
                    )?;
                    (validation::scoped_arithmetic_values_relation(
                        self.occurrence().program,
                        self.occurrence().machine,
                        self.occurrence().hypotheses,
                        typed_trees::expression::BinaryOperator::Equal,
                        &demanded,
                        observed,
                    ) == validation::StrictArithmeticImplicationJudgment::Proven)
                        .then(|| observed.clone())
                })
            })
        }

        fn telescoped_selector_leaf_value_from_expression(
            &self,
            read: &DeclaredInputRead,
            root: &RankSubjectCoordinate,
            depth: usize,
            expression: ExpressionHandle,
        ) -> Option<ScopedArithmeticValue> {
            let place = validation::canonical_place_from_expression(self.program, expression)
                .or_else(|| {
                    validation::canonical_place_from_expression_in_state(
                        self.program,
                        self.state.symbol,
                        self.statement_index,
                        expression,
                    )
                })?;
            let formal_root = place.root.as_symbol()?;
            let (_, input) = read
                .selector_inputs
                .iter()
                .find(|(formal, _)| *formal == formal_root)?;
            let mut coordinate = input.clone();
            coordinate.path.extend(place.segments);
            self.telescoped_selector_leaf_value_from_coordinate(read, root, depth, &coordinate)
        }

        fn telescoped_selector_leaf_value_from_coordinate(
            &self,
            read: &DeclaredInputRead,
            root: &RankSubjectCoordinate,
            depth: usize,
            input: &RankSubjectCoordinate,
        ) -> Option<ScopedArithmeticValue> {
            let selected = self.declared_projection(root, read, input, depth + 1)?;
            self.fields
                .iter()
                .find(|field| {
                    field.owner == root.parameter
                        && same_projection(self.occurrence(), &field.path, &selected)
                })
                .and_then(|field| field.value.clone())
        }
    }
}
mod entry_fields {
    /*
        Invocation entry observations use a finite coordinate demand. Owned storage
        supplies its ordinary field frontier; observing beyond it requires a matched
        frozen reference load from the frame owner. Never expand pointer-type graphs:
        recursive types do not define a finite set of observable field paths.
    */
    use super::CurrentScalarValues;
    use super::LengthSubject;
    use super::scalar_fields::CapturedScalarField;
    use super::scalar_fields::ScalarField;
    use super::scalar_fields::entry_selector_value;
    use super::scalar_fields::same_projection;
    use super::scalar_fields::{ScalarProjection, capture_path};
    use crate::flow::indexed_statements;
    use crate::shared::collections::UniqueRoster;
    use validation::BigIntInterval;
    use validation::ScopedArithmeticValue;

    /*
        Paths are declaration identities, shared across a call component so a private
        forwarder need not repeat the caller's rank expression. Suffixes let an actual
        projected from a larger record retain the same selected declaration path.
    */
    pub(crate) fn parameter_projection_paths(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> Vec<Vec<facts::PlaceSegment>> {
        let mut pending = Vec::new();
        for (_, statement) in indexed_statements(program, state.statement_nodes) {
            pending.extend(
                program
                    .tables
                    .expression_roots(statement)
                    .into_iter()
                    .map(|(_, expression)| expression),
            );
        }
        for contract in program
            .state_contracts(state)
            .iter()
            .chain(program.machine_contracts(machine))
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let Some(expression) = fact.as_expression() {
                    pending.push(*expression);
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        let mut paths = UniqueRoster::default();
        while let Some(expression) = pending.pop() {
            if !expression.is_valid() || !seen.insert(expression) {
                continue;
            }
            if let Some(place) = validation::canonical_place_from_expression(program, expression)
                && let facts::PlaceRoot::Symbol(root) = place.root
                && program.state_parameter_by_symbol(state, root).is_some()
            {
                let path = place.segments;
                for start in 0..path.len() {
                    paths.push(path[start..].to_vec());
                }
            }
            validation::expression_traversal::children(
                program,
                program.expression_table.expression(expression),
                |child| pending.push(child),
            );
        }
        paths.into_vec()
    }

    impl CurrentScalarValues<'_> {
        /*
            Owned records and readable borrowed records expose typed entry fields.
            A borrowed root requires its own stable reference binding; it does not
            freeze the referent. These current rows still expire on overlapping
            writes, while a selected entry value is saved independently by ranking.
            Only the formal's authenticated boundary is crossed here: reference
            leaves inside the record need separate load evidence and are not walked.
        */
        pub(super) fn seed_parameter_fields(
            &mut self,
            requested: &[Vec<facts::PlaceSegment>],
            frames: Option<&validation::CallFrameResolver<'_>>,
        ) {
            for parameter in self.program.state_parameters(self.state) {
                if parameter.is_const || parameter.relevance.is_erased() {
                    continue;
                }
                let reference = if let Some(shell) =
                    validation::reference_shell(self.program, parameter.type_reference)
                {
                    if !shell.access.is_readable()
                        || !validation::state_reference_parameter_binding_is_stable(
                            self.program,
                            self.machine,
                            self.state,
                            parameter.symbol,
                        )
                    {
                        continue;
                    }
                    shell.referee
                } else {
                    parameter.type_reference
                };
                let mut pending = vec![(reference, Vec::new(), 0usize)];
                while let Some((reference, path, depth)) = pending.pop() {
                    if depth >= 64 {
                        continue;
                    }
                    let declared = self
                        .program
                        .type_reference_table
                        .unconstrained_type_reference(reference);
                    if self
                        .program
                        .access_view_plan_for_type_reference(declared)
                        .is_some()
                        || self
                            .program
                            .access_view_field_plan_for_type_reference(declared)
                            .is_some()
                        || matches!(self.program.type_reference_table.type_reference(declared),
                            typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                                if self.program.symbols.builtin_type_atom(*symbol)
                                    .is_some_and(|atom| atom.is_atomic()))
                    {
                        continue;
                    }
                    let identity = format!(
                        "state:{:?}:parameter:{:?}:field:{path:?}",
                        self.state.symbol, parameter.symbol
                    );
                    if let Some(value) =
                        super::entry_collection_length(self.program, reference, || {
                            format!("{identity}:length")
                        })
                    {
                        if !path.is_empty() {
                            let coordinate = path
                                .iter()
                                .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                                .collect::<Vec<_>>();
                            let Some(name) = validation::frame_length_coordinate_path(
                                self.program,
                                parameter.symbol,
                                &coordinate,
                            ) else {
                                continue;
                            };
                            self.lengths.push(super::Length {
                                subject: LengthSubject::Field {
                                    owner: parameter.symbol,
                                    path: coordinate,
                                },
                                name,
                                stable: false,
                                type_reference: reference,
                                value: Some(value),
                            });
                        }
                        continue;
                    }
                    if let Some(carrier) = super::integer_carrier(self.program, reference) {
                        if !path.is_empty() {
                            self.fields.push(ScalarField::for_owner(
                                parameter.symbol,
                                CapturedScalarField {
                                    path: path
                                        .iter()
                                        .map(|symbol| ScalarProjection::Field(*symbol))
                                        .collect(),
                                    carrier,
                                    value: entry_value(
                                        self.program,
                                        reference,
                                        path.last().copied(),
                                        ScopedArithmeticValue::Atom {
                                            identity,
                                            unsigned: super::unsigned(carrier),
                                        },
                                    ),
                                },
                            ));
                        }
                        continue;
                    }
                    let reference = self
                        .program
                        .type_reference_table
                        .unconstrained_type_reference(reference);
                    if matches!(
                        self.program.type_reference_table.type_reference(reference),
                        typed_trees::types::TypeReferenceNode::Reference { .. }
                    ) {
                        continue;
                    }
                    let Some(data) =
                        self.program
                            .data_application_definition(reference)
                            .or_else(|| {
                                self.program
                                    .type_reference_table
                                    .type_head_symbol(reference)
                                    .and_then(|symbol| {
                                        self.program.data_definition_by_symbol(symbol)
                                    })
                            })
                    else {
                        continue;
                    };
                    for member in self.program.data_members(data) {
                        if let typed_trees::data::DataMember::Field(field) = member
                            && !field.relevance.is_erased()
                        {
                            let mut child = path.clone();
                            child.push(field.symbol);
                            pending.push((field.type_reference, child, depth + 1));
                        }
                    }
                }
                let Some(frames) = frames else { continue };
                let order = self
                    .program
                    .statement_table
                    .admitted_execution_order(self.state.statement_nodes);
                let Some(&first) = order.indices().first() else {
                    continue;
                };
                for coordinate in requested {
                    let Some(path) = capture_path(coordinate, |selector| {
                        entry_selector_value(
                            self.occurrence(),
                            selector,
                            &self.current,
                            &self.fields,
                        )
                    }) else {
                        continue;
                    };
                    if self.fields.iter().any(|field| {
                        field.owner == parameter.symbol
                            && same_projection(self.occurrence(), &field.path, &path)
                    }) {
                        continue;
                    }
                    let Some(field) = validation::structural_scalar_coordinate(
                        self.program,
                        self.program.state_parameters(self.state),
                        parameter.symbol,
                        coordinate,
                    ) else {
                        continue;
                    };
                    if !field.source_requirements.iter().all(|requirement| {
                        frames
                            .coordinate_sources_before_statement(
                                self.machine,
                                self.state,
                                first,
                                parameter.symbol,
                                requirement,
                            )
                            .is_some_and(|sources| !sources.is_empty())
                    }) {
                        continue;
                    }
                    self.fields.push(ScalarField::for_owner(
                        parameter.symbol,
                        CapturedScalarField {
                            path,
                            carrier: field.primitive_type,
                            value: entry_value(
                                self.program,
                                field.result_type_reference,
                                coordinate.last().and_then(|step| match step {
                                    facts::PlaceSegment::Field { symbol } => Some(*symbol),
                                    _ => None,
                                }),
                                ScopedArithmeticValue::Atom {
                                    identity: format!(
                                        "state:{:?}:parameter:{:?}:field:{coordinate:?}",
                                        self.state.symbol, parameter.symbol
                                    ),
                                    unsigned: super::unsigned(field.primitive_type),
                                },
                            ),
                        },
                    ));
                }
            }
        }
    }

    /*
        Incoming values have already established their declared field and standing
        data predicates. Carry those bounds on the saved entry value itself. Stores
        replace this record, so an open invariant window acquires no live bound from
        this entry evidence. These are declaration facts, never a proposed rank range.
    */
    pub(super) fn entry_value(
        program: &dyn validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
        field: Option<symbols::SymbolHandle>,
        value: ScopedArithmeticValue,
    ) -> ScopedArithmeticValue {
        let declared = validation::enforced_integer_type_bounds(program, reference);
        let (mut minimum, mut maximum) =
            declared.map_or((None, None), |(lo, hi)| (Some(lo), Some(hi)));
        if let Some(field) = field {
            let owner = program.symbols.get(field).parent;
            if let Some(data) = program.data_definition_by_symbol(owner)
                && let Some(intervals) = validation::data_where_field_intervals(program, data)
                && let Some((_, lo, hi)) = intervals.iter().find(|(symbol, ..)| *symbol == field)
            {
                if let Some(lo) = lo {
                    minimum = Some(minimum.map_or(*lo, |old| old.max(*lo)));
                }
                if let Some(hi) = hi {
                    maximum = Some(maximum.map_or(*hi, |old| old.min(*hi)));
                }
            }
        }
        if minimum.is_none() && maximum.is_none() {
            return value;
        }
        ScopedArithmeticValue::BoundedInteger {
            value: Box::new(value),
            interval: BigIntInterval {
                low: minimum.map(numerics::bignum::BigInt::from_i64),
                high: maximum.map(numerics::bignum::BigInt::from_i64),
            },
        }
    }
}
pub(crate) use entry_fields::parameter_projection_paths;
mod joins {
    /*
        A completed scalar dispatch owns one value, independent of which arm won.
        Retain only bounds established for every captured arm. Anonymous integer
        literals need no fabricated runtime type to contribute mathematical bounds;
        source checking independently lands the completed result in its carrier.
    */
    use super::CurrentScalarValues;
    use numerics::bignum::BigInt;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, TableMatchExpression};
    use typed_trees::{machine::Machine, state::State};
    use validation::ScopedArithmeticValue;

    impl CurrentScalarValues<'_> {
        pub(super) fn capture_join(
            &self,
            program: &dyn validation::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            dispatch: &TableMatchExpression,
            depth: usize,
        ) -> Option<ScopedArithmeticValue> {
            let arms = program.expression_table.match_arms(dispatch.arms);
            if arms.is_empty() {
                return None;
            }
            let captured = arms
                .iter()
                .map(|arm| self.capture_at_depth(program, machine, state, arm.value, depth + 1))
                .collect::<Option<Vec<_>>>()?;
            joined_values(
                program,
                machine,
                &self.hypotheses,
                format!("state:{:?}:join-result:{expression:?}", state.symbol),
                captured,
            )
        }
    }

    pub(super) fn joined_values(
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        hypotheses: &[validation::ScopedArithmeticHypothesis],
        identity: String,
        mut captured: Vec<ScopedArithmeticValue>,
    ) -> Option<ScopedArithmeticValue> {
        if captured.is_empty() {
            return None;
        }
        if captured.iter().skip(1).all(|value| {
            value == &captured[0]
                || validation::scoped_arithmetic_values_relation(
                    program,
                    machine,
                    hypotheses,
                    BinaryOperator::Equal,
                    value,
                    &captured[0],
                ) == validation::StrictArithmeticImplicationJudgment::Proven
        }) {
            return Some(captured.swap_remove(0));
        }
        let bounds = captured
            .iter()
            .map(|value| {
                validation::scoped_arithmetic_value_bounds(program, machine, hypotheses, value)
            })
            .collect::<Option<Vec<_>>>()?;
        if bounds
            .iter()
            .any(|bound| bound.low.is_none() || bound.high.is_none())
        {
            return None;
        }
        let interval = bounds
            .into_iter()
            .reduce(|left, right| left.union(&right))?;
        let unsigned = interval.low.as_ref()? >= &BigInt::zero();
        Some(ScopedArithmeticValue::BoundedInteger {
            value: Box::new(ScopedArithmeticValue::Atom { identity, unsigned }),
            interval,
        })
    }
}
mod normal_completion {
    /*
        Normal-completion observations bind the exact outgoing result.

        Current stores already retain bounded joins as well as concrete values.
        Rejoin those observations to the declaration's result paths after all return
        operand writes have retired old facts. The guarantee contributes no bound;
        this roster comes only from the completed value's existing typed capture.
        It is neither a descriptor-content permission nor an executed scalar read.
    */
    use super::scalar_fields::{CapturedScalarField, capture_path, same_projection};
    use super::{CurrentScalarValues, Length, LengthSubject, ScopedOccurrence, admitted, scoped};
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use validation::{
        ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
        ScopedArithmeticValue,
    };

    pub(super) struct CapturedResultFields {
        pub(super) owner: SymbolHandle,
        fields: Vec<CapturedScalarField>,
    }

    /*
        Only this invocation's exact result namespace may consume this roster.
        A same-shaped result on a selected callee is a different subject.
    */
    pub(super) fn result_scalar_field_occurrence(
        occurrence: ScopedOccurrence<'_, '_>,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticValue> {
        let result = occurrence.result_fields?;
        let place = validation::reserved_result_place(occurrence.program, expression)?;
        if place.machine_symbol != result.owner || place.machine_symbol != occurrence.machine.symbol
        {
            return None;
        }
        let path = capture_path(&place.segments, |_| None)?;
        let carrier = super::integer_carrier(occurrence.program, place.type_reference)?;
        let mut fields = result.fields.iter().filter(|field| {
            field.carrier == carrier && same_projection(occurrence, &field.path, &path)
        });
        let field = fields.next()?;
        fields.next().is_none().then(|| field.value.clone())
    }

    impl CurrentScalarValues<'_> {
        pub(crate) fn prepare_exit_return(&mut self, returned: ExpressionHandle) {
            if let Some(value) = self.collection_length(returned, 0)
                && let Some(entry) = self.program.machine_states(self.machine).first()
            {
                self.lengths.push(Length {
                    subject: LengthSubject::Result(self.machine.symbol),
                    name: String::new(),
                    stable: true,
                    type_reference: entry.return_type,
                    value: Some(value),
                });
            }
            self.record_returned_field_lengths(returned);
            self.result_fields = self
                .program
                .machine_states(self.machine)
                .first()
                .map(|entry| CapturedResultFields {
                    owner: self.machine.symbol,
                    fields: self.captured_scalar_fields_of_type(returned, entry.return_type, 0),
                });
        }

        pub(crate) fn postcondition(
            &self,
            expression: ExpressionHandle,
            returned: Option<ExpressionHandle>,
            origins: impl Iterator<Item = (SymbolHandle, SymbolHandle)>,
        ) -> Option<ScopedArithmeticExpression> {
            let entry = self.program.machine_states(self.machine).first()?;
            let mut bindings = self.current.clone();
            let mut lengths = self.lengths.clone();
            if entry.symbol != self.state.symbol {
                bindings.clear();
                lengths.retain(|length| {
                    matches!(
                        length.subject,
                        LengthSubject::Result(_) | LengthSubject::ResultField { .. }
                    )
                });
                for (entry_parameter, state_parameter) in origins {
                    if let Some(current) = self
                        .current
                        .iter()
                        .find(|binding| binding.symbol == state_parameter)
                    {
                        let mut binding = current.clone();
                        binding.symbol = entry_parameter;
                        binding.name = self.program.symbols.name(entry_parameter).to_owned();
                        bindings.push(binding);
                    }
                    if let Some(current) = self
                        .lengths
                        .iter()
                        .find(|length| length.subject == LengthSubject::Storage(state_parameter))
                    {
                        let mut length = current.clone();
                        length.subject = LengthSubject::Storage(entry_parameter);
                        length.name = self.program.symbols.name(entry_parameter).to_owned();
                        lengths.push(length);
                    }
                }
            }
            let fields = if entry.symbol == self.state.symbol {
                self.fields.as_slice()
            } else {
                &[]
            };
            let mut occurrence = self.occurrence_at(self.program, self.machine, entry, &lengths);
            occurrence.result_fields = self.result_fields.as_ref();
            if !admitted(occurrence, expression, &bindings, fields, 0) {
                return None;
            }
            let mut goal = scoped(occurrence, expression, &bindings, fields)?;
            if let Some(returned) = returned.and_then(|returned| self.current(returned)) {
                goal.bindings.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Result,
                    value: ScopedArithmeticValue::Term(returned),
                });
            }
            Some(goal)
        }
    }
}
use normal_completion::{CapturedResultFields, result_scalar_field_occurrence};
mod scalar_fields {
    /*
        Executed scalar-field transfers retain exact place and carrier identities.
        A stored value is captured before invalidation. Later reads use that value,
        never the field initializer or a new observation of overwritten storage.
    */
    use super::{
        Binding, CallArgumentProjection, CurrentScalarValues, ScopedOccurrence, integer_carrier,
    };
    mod collection_values {
        /*
            Finite collection observations travel with their captured values. Array
            construction records element coordinates; a formed window translates those
            coordinates, never the backing capacity. Reading an unknown selector can
            join a contiguous equal-value run only after independently proving inclusion.
        */
        use super::super::{CurrentScalarValues, ScopedOccurrence};
        use super::{
            CapturedScalarField, ScalarField, ScalarProjection, integer_carrier, same_projection,
        };
        use symbols::SymbolHandle;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle};
        use typed_trees::types::PrimitiveType;
        use validation::ScopedArithmeticValue;

        fn element(index: u64) -> Option<ScalarProjection> {
            Some(ScalarProjection::Element {
                source: facts::PlaceSegment::FixedIndex {
                    index: usize::try_from(index).ok()?,
                },
                selector: ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(index)),
            })
        }

        impl CurrentScalarValues<'_> {
            pub(super) fn capture_array_elements(
                &self,
                elements: arena::HandleSpan<ExpressionHandle>,
                reference: typed_trees::types::TypeReferenceHandle,
                depth: usize,
            ) -> Vec<CapturedScalarField> {
                let Some((element_type, _)) =
                    validation::readable_fixed_array_elements(self.program, reference)
                else {
                    return Vec::new();
                };
                let mut fields = Vec::new();
                for (index, expression) in self
                    .program
                    .expression_table
                    .expression_handles(elements)
                    .iter()
                    .enumerate()
                {
                    let Some(index) = u64::try_from(index).ok().and_then(element) else {
                        return Vec::new();
                    };
                    for mut field in
                        self.captured_scalar_fields_of_type(*expression, element_type, depth + 1)
                    {
                        field.path.insert(0, index.clone());
                        fields.push(field);
                    }
                    let carrier = integer_carrier(self.program, element_type);
                    if let Some(carrier) = carrier
                        && let Some(value) =
                            self.capture(self.program, self.machine, self.state, *expression)
                    {
                        fields.push(CapturedScalarField {
                            path: vec![index],
                            carrier,
                            value,
                        });
                    }
                }
                fields
            }

            pub(super) fn capture_join_fields(
                &self,
                expression: ExpressionHandle,
                dispatch: &typed_trees::expression::TableMatchExpression,
                depth: usize,
            ) -> Vec<CapturedScalarField> {
                let arms = self.program.expression_table.match_arms(dispatch.arms);
                let alternatives = arms
                    .iter()
                    .map(|arm| self.captured_scalar_fields(arm.value, depth + 1))
                    .collect::<Vec<_>>();
                let Some(first) = alternatives.first() else {
                    return Vec::new();
                };
                first
                    .iter()
                    .filter_map(|field| {
                        let selected = alternatives
                            .iter()
                            .map(|fields| {
                                fields.iter().find(|candidate| {
                                    candidate.carrier == field.carrier
                                        && same_projection(
                                            self.occurrence(),
                                            &candidate.path,
                                            &field.path,
                                        )
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let value = super::super::joins::joined_values(
                            self.program,
                            self.machine,
                            &self.hypotheses,
                            format!(
                                "state:{:?}:join-result:{expression:?}:field:{:?}",
                                self.state.symbol, field.path
                            ),
                            selected.iter().map(|field| field.value.clone()).collect(),
                        )?;
                        Some(CapturedScalarField {
                            path: field.path.clone(),
                            carrier: field.carrier,
                            value,
                        })
                    })
                    .collect()
            }

            pub(super) fn capture_window_elements(
                &self,
                expression: ExpressionHandle,
                indexed: &typed_trees::expression::TableIndexedExpression,
                depth: usize,
            ) -> Option<Vec<CapturedScalarField>> {
                let (start, end) = self.window_bounds(expression, indexed, depth, None, None)?;
                let start = self.exact_length(&start)?.to_u64()?;
                let end = self.exact_length(&end)?.to_u64()?;
                Some(
                    self.captured_scalar_fields(indexed.collection, depth + 1)
                        .into_iter()
                        .filter_map(|mut field| {
                            let ScalarProjection::Element { selector, .. } = field.path.first()?
                            else {
                                return None;
                            };
                            let index = self.exact_length(selector)?.to_u64()?;
                            if index < start || index >= end {
                                return None;
                            }
                            field.path[0] = element(index.checked_sub(start)?)?;
                            Some(field)
                        })
                        .collect(),
                )
            }
        }

        fn proves(
            occurrence: ScopedOccurrence<'_, '_>,
            operator: BinaryOperator,
            left: &ScopedArithmeticValue,
            right: &ScopedArithmeticValue,
        ) -> bool {
            validation::scoped_arithmetic_values_relation(
                occurrence.program,
                occurrence.machine,
                occurrence.hypotheses,
                operator,
                left,
                right,
            ) == validation::StrictArithmeticImplicationJudgment::Proven
        }

        pub(super) fn selected_value(
            occurrence: ScopedOccurrence<'_, '_>,
            owner: SymbolHandle,
            path: &[ScalarProjection],
            carrier: PrimitiveType,
            fields: &[ScalarField],
        ) -> Option<ScopedArithmeticValue> {
            if let Some(field) = fields.iter().find(|field| {
                field.owner == owner
                    && field.carrier == carrier
                    && same_projection(occurrence, &field.path, path)
            }) {
                return field.value.clone();
            }
            let (ScalarProjection::Element { selector, .. }, prefix) = path.split_last()? else {
                return None;
            };
            let mut elements = fields
                .iter()
                .filter_map(|field| {
                    if field.owner != owner
                        || field.carrier != carrier
                        || field.path.len() != path.len()
                        || !same_projection(occurrence, &field.path[..prefix.len()], prefix)
                    {
                        return None;
                    }
                    let ScalarProjection::Element {
                        selector: ScopedArithmeticValue::Integer(index),
                        ..
                    } = field.path.last()?
                    else {
                        return None;
                    };
                    Some((index.to_u64()?, field.value.as_ref()?))
                })
                .collect::<Vec<_>>();
            elements.sort_by_key(|(index, _)| *index);
            if elements.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return None;
            }
            let mut start = 0;
            while start < elements.len() {
                let mut end = start;
                while end + 1 < elements.len()
                    && elements[end].0.checked_add(1) == Some(elements[end + 1].0)
                    && elements[start].1 == elements[end + 1].1
                {
                    end += 1;
                }
                let low = ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(
                    elements[start].0,
                ));
                let high = ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(
                    elements[end].0,
                ));
                if proves(occurrence, BinaryOperator::GreaterOrEqual, selector, &low)
                    && proves(occurrence, BinaryOperator::LessOrEqual, selector, &high)
                {
                    return Some(elements[start].1.clone());
                }
                start = end + 1;
            }
            if let (Some(first), Some(last)) = (elements.first(), elements.last())
                && elements
                    .windows(2)
                    .all(|pair| pair[0].0.checked_add(1) == Some(pair[1].0))
            {
                let low =
                    ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(first.0));
                let high =
                    ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(last.0));
                if proves(occurrence, BinaryOperator::GreaterOrEqual, selector, &low)
                    && proves(occurrence, BinaryOperator::LessOrEqual, selector, &high)
                {
                    return super::super::joins::joined_values(
                        occurrence.program,
                        occurrence.machine,
                        occurrence.hypotheses,
                        format!("selected:{owner:?}:{path:?}:{elements:?}"),
                        elements.iter().map(|(_, value)| (*value).clone()).collect(),
                    );
                }
            }
            None
        }
    }
    pub(super) mod coordinates {
        /*
            A storage projection captures selectors as values, not mutable spellings.
            Source segments remain available for typed observation planning; equality
            of an element uses its completed selector, independently of that syntax.
        */
        use super::super::{Binding, ScalarField, ScopedOccurrence, admitted, scoped};
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;
        use validation::ScopedArithmeticValue;

        #[derive(Clone, Debug)]
        pub(in super::super) enum ScalarProjection {
            Field(SymbolHandle),
            Case(SymbolHandle),
            Element {
                source: facts::PlaceSegment,
                selector: ScopedArithmeticValue,
            },
        }

        impl PartialEq for ScalarProjection {
            fn eq(&self, other: &Self) -> bool {
                match (self, other) {
                    (Self::Field(left), Self::Field(right))
                    | (Self::Case(left), Self::Case(right)) => left == right,
                    (
                        Self::Element { selector: left, .. },
                        Self::Element {
                            selector: right, ..
                        },
                    ) => left == right,
                    _ => false,
                }
            }
        }

        /*
            Captured recipes share a coordinate only when the existing scoped integer
            judgment proves their selectors equal. Equality of source handles or carrier
            types supplies no such proof; structural equality is only the fast path.
        */
        pub(in super::super) fn same_projection(
            occurrence: ScopedOccurrence<'_, '_>,
            left: &[ScalarProjection],
            right: &[ScalarProjection],
        ) -> bool {
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    if left == right {
                        return true;
                    }
                    let (
                        ScalarProjection::Element { selector: left, .. },
                        ScalarProjection::Element {
                            selector: right, ..
                        },
                    ) = (left, right)
                    else {
                        return false;
                    };
                    validation::scoped_arithmetic_values_relation(
                        occurrence.program,
                        occurrence.machine,
                        occurrence.hypotheses,
                        typed_trees::expression::BinaryOperator::Equal,
                        left,
                        right,
                    ) == validation::StrictArithmeticImplicationJudgment::Proven
                })
        }

        pub(in super::super) fn projection_has_prefix(
            occurrence: ScopedOccurrence<'_, '_>,
            path: &[ScalarProjection],
            prefix: &[ScalarProjection],
        ) -> bool {
            path.len() >= prefix.len() && same_projection(occurrence, &path[..prefix.len()], prefix)
        }

        pub(in super::super) fn source_path(path: &[ScalarProjection]) -> Vec<facts::PlaceSegment> {
            path.iter()
                .map(|segment| match segment {
                    ScalarProjection::Field(symbol) => {
                        facts::PlaceSegment::Field { symbol: *symbol }
                    }
                    ScalarProjection::Case(variant) => {
                        facts::PlaceSegment::Case { variant: *variant }
                    }
                    ScalarProjection::Element { source, .. } => *source,
                })
                .collect()
        }

        pub(in super::super) fn capture_path(
            path: &[facts::PlaceSegment],
            mut capture: impl FnMut(ExpressionHandle) -> Option<ScopedArithmeticValue>,
        ) -> Option<Vec<ScalarProjection>> {
            path.iter()
                .map(|segment| {
                    Some(match segment {
                        facts::PlaceSegment::Field { symbol } => ScalarProjection::Field(*symbol),
                        facts::PlaceSegment::Case { variant } => ScalarProjection::Case(*variant),
                        facts::PlaceSegment::FixedIndex { index } => ScalarProjection::Element {
                            source: *segment,
                            selector: ScopedArithmeticValue::Integer(
                                numerics::bignum::BigInt::from_u64(u64::try_from(*index).ok()?),
                            ),
                        },
                        facts::PlaceSegment::Index { expression } => ScalarProjection::Element {
                            source: *segment,
                            selector: capture(*expression)?,
                        },
                        _ => return None,
                    })
                })
                .collect()
        }

        /*
            Entry inference may use values already present in the invocation, never a
            producer that will execute in the body. Check the whole executable tree:
            a folded length can otherwise hide an effectful collection underneath it.
        */
        pub(in super::super) fn entry_selector_value(
            occurrence: ScopedOccurrence<'_, '_>,
            expression: ExpressionHandle,
            bindings: &[Binding],
            fields: &[ScalarField],
        ) -> Option<ScopedArithmeticValue> {
            if !validation::expression_traversal::inert_value_tree_with_operations(
                occurrence.program,
                expression,
                |operation| admitted(occurrence, operation, bindings, fields, 0),
            ) {
                return None;
            }
            let mut expressions = Vec::new();
            validation::expression_traversal::collect_expression_tree(
                occurrence.program,
                expression,
                &mut expressions,
            );
            if expressions.iter().any(|expression| {
                let typed_trees::expression::ExpressionNode::Name(name) =
                    occurrence.program.expression_table.expression(*expression)
                else {
                    return false;
                };
                occurrence
                    .program
                    .state_parameter_by_symbol(occurrence.state, name.symbol)
                    .is_none()
                    && !bindings
                        .iter()
                        .any(|binding| binding.symbol == name.symbol && binding.value.is_some())
                    && occurrence
                        .program
                        .const_declaration_by_symbol(name.symbol)
                        .is_none()
            }) {
                return None;
            }
            selector_value(occurrence, expression, bindings, fields)
        }

        pub(in super::super) fn selector_value(
            occurrence: ScopedOccurrence<'_, '_>,
            expression: ExpressionHandle,
            bindings: &[Binding],
            fields: &[ScalarField],
        ) -> Option<ScopedArithmeticValue> {
            if let typed_trees::expression::ExpressionNode::Name(name) =
                occurrence.program.expression_table.expression(expression)
                && name.symbol == name.head_symbol
                && let Some(binding) = bindings
                    .iter()
                    .find(|binding| binding.symbol == name.symbol)
            {
                return binding.value.clone();
            }
            if !admitted(occurrence, expression, bindings, fields, 0) {
                return None;
            }
            let term = scoped(occurrence, expression, bindings, fields)?;
            Some(
                crate::checks::contracts::prover::call_guarantees::arithmetic::completed_scalar_value(
                    occurrence.program,
                    &crate::checks::contracts::prover::call_guarantees::callable::Callable::Machine {
                        machine: occurrence.machine,
                        state: occurrence.state,
                    },
                    term,
                ),
            )
        }

        pub(in super::super) fn storage_path(
            occurrence: ScopedOccurrence<'_, '_>,
            expression: ExpressionHandle,
            bindings: &[Binding],
            fields: &[ScalarField],
        ) -> Option<(SymbolHandle, Vec<ScalarProjection>)> {
            let place =
                validation::canonical_place_from_expression(occurrence.program, expression)?;
            let facts::PlaceRoot::Symbol(owner) = place.root else {
                return None;
            };
            Some((
                owner,
                capture_path(&place.segments, |selector| {
                    selector_value(occurrence, selector, bindings, fields)
                })?,
            ))
        }
    }
    pub(super) use coordinates::{
        ScalarProjection, capture_path, entry_selector_value, projection_has_prefix,
        same_projection, source_path, storage_path,
    };
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::PrimitiveType;
    use validation::ScopedArithmeticValue;

    #[derive(Clone)]
    pub(super) struct ScalarField {
        pub(super) owner: SymbolHandle,
        pub(super) path: Vec<ScalarProjection>,
        pub(super) carrier: PrimitiveType,
        pub(super) value: Option<ScopedArithmeticValue>,
    }

    #[derive(Clone)]
    pub(super) struct CapturedScalarField {
        pub(super) path: Vec<ScalarProjection>,
        pub(super) carrier: PrimitiveType,
        pub(super) value: ScopedArithmeticValue,
    }

    impl ScalarField {
        pub(super) fn for_owner(owner: SymbolHandle, field: CapturedScalarField) -> Self {
            Self {
                owner,
                path: field.path,
                carrier: field.carrier,
                value: Some(field.value),
            }
        }
    }

    pub(super) fn scalar_field_occurrence(
        occurrence: ScopedOccurrence<'_, '_>,
        expression: ExpressionHandle,
        bindings: &[Binding],
        fields: &[ScalarField],
    ) -> Option<ScopedArithmeticValue> {
        let (owner, path) = storage_path(occurrence, expression, bindings, fields)?;
        let ScopedOccurrence {
            program,
            machine,
            state,
            ..
        } = occurrence;
        if path.is_empty() {
            return None;
        }
        let reference =
            validation::expression_result_type_reference(program, machine, state, expression)?;
        let carrier = integer_carrier(program, reference)?;
        collection_values::selected_value(occurrence, owner, &path, carrier, fields)
    }

    impl CurrentScalarValues<'_> {
        /*
            Read only the live roster at this completed frontier. A missing or
            invalidated cell cannot be reconstructed from its source initializer.
        */
        pub(crate) fn scalar_value_at_place(
            &self,
            place: &validation::CanonicalPlace,
        ) -> Option<ScopedArithmeticValue> {
            let owner = place.root.as_symbol()?;
            if place.segments.is_empty() {
                return self
                    .current
                    .iter()
                    .find(|binding| binding.symbol == owner)?
                    .value
                    .clone();
            }
            let path = capture_path(&place.segments, |_| None)?;
            let mut fields = self.fields.iter().filter(|field| {
                field.owner == owner && same_projection(self.occurrence(), &field.path, &path)
            });
            let value = fields.next()?.value.clone()?;
            fields.next().is_none().then_some(value)
        }

        pub(super) fn scalar_storage_path(
            &self,
            expression: ExpressionHandle,
        ) -> Option<(SymbolHandle, Vec<ScalarProjection>)> {
            storage_path(self.occurrence(), expression, &self.current, &self.fields)
        }

        /*
            Syntax handles identify observations, while captured selectors identify
            the selected entry storage. Normalization may copy a selector into a
            fresh local without changing that storage coordinate.
        */
        pub(crate) fn same_scalar_projection(
            &self,
            left: &[facts::PlaceSegment],
            right: &[facts::PlaceSegment],
        ) -> bool {
            let capture = |path: &[facts::PlaceSegment]| {
                capture_path(path, |selector| {
                    coordinates::selector_value(
                        self.occurrence(),
                        selector,
                        &self.current,
                        &self.fields,
                    )
                })
            };
            match (capture(left), capture(right)) {
                (Some(left), Some(right)) => same_projection(self.occurrence(), &left, &right),
                _ => false,
            }
        }

        /*
            Exact retained inline fields of one formal. The caller selects the
            frame: an entry frame supplies entry values; a later frame supplies
            only associations that survived its executed writes and calls.
        */
        pub(crate) fn entry_scalar_field_observations(
            &self,
            parameter: SymbolHandle,
        ) -> impl Iterator<
            Item = (
                Vec<facts::PlaceSegment>,
                PrimitiveType,
                &ScopedArithmeticValue,
                Vec<checked_trees::CheckedEntryIndexBound>,
            ),
        > {
            self.fields.iter().filter_map(move |field| {
                if field.owner != parameter {
                    return None;
                }
                let value = field.value.as_ref()?;
                let indexes = self.scalar_projection_formation(field)?;
                Some((source_path(&field.path), field.carrier, value, indexes))
            })
        }

        fn scalar_projection_formation(
            &self,
            field: &ScalarField,
        ) -> Option<Vec<checked_trees::CheckedEntryIndexBound>> {
            let geometry = validation::structural_scalar_coordinate(
                self.program,
                self.program.state_parameters(self.state),
                field.owner,
                &source_path(&field.path),
            )?;
            self.projection_indexes(field.owner, &field.path, &geometry.projections)
        }

        /*
            Form an arbitrary typed entry projection without choosing a scalar leaf.
            Selector values are captured once in the entry frame. This supplies
            index/case formation only, never whole-value read or copy authority.
        */
        pub(crate) fn entry_projection_indexes(
            &self,
            owner: SymbolHandle,
            path: &[facts::PlaceSegment],
        ) -> Option<Vec<checked_trees::CheckedEntryIndexBound>> {
            let execution = self
                .program
                .statement_table
                .admitted_execution_order(self.state.statement_nodes);
            let first = execution.indices().first().copied().unwrap_or(0);
            if self.statement_index != first || self.in_transition_target {
                return None;
            }
            let geometry = validation::structural_coordinate(
                self.program,
                self.program.state_parameters(self.state),
                owner,
                path,
            )?;
            let captured = capture_path(path, |selector| {
                entry_selector_value(self.occurrence(), selector, &self.current, &self.fields)
            })?;
            self.projection_indexes(owner, &captured, &geometry.projections)
        }

        /*
            Scalar and structural entry projections share the same case and index
            obligations. The supplied path already contains its selector snapshots;
            no later source spelling is used to prove a bound.
        */
        fn projection_indexes(
            &self,
            owner: SymbolHandle,
            captured: &[ScalarProjection],
            projections: &[validation::ScalarCoordinateProjection],
        ) -> Option<Vec<checked_trees::CheckedEntryIndexBound>> {
            let path = source_path(captured);
            if path
                .iter()
                .any(|step| matches!(step, facts::PlaceSegment::Case { .. }))
            {
                let is_entry = self
                    .program
                    .machine_states(self.machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == self.state.symbol);
                let requires = self
                    .program
                    .state_contracts(self.state)
                    .iter()
                    .chain(
                        self.program
                            .machine_contracts(self.machine)
                            .iter()
                            .filter(|_| is_entry),
                    )
                    .filter(|contract| {
                        contract.kind == typed_trees::signature::SignatureContractKind::Requires
                    })
                    .flat_map(|contract| self.program.proof_facts.span_or_empty(contract.facts))
                    .filter_map(|fact| fact.as_expression().copied())
                    .collect::<Vec<_>>();
                if !validation::declaration_projection_cases_hold(
                    self.program,
                    self.machine.symbol,
                    self.program.state_parameters(self.state),
                    &requires,
                    &validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(owner),
                        segments: path.clone(),
                    },
                ) {
                    return None;
                }
            }
            let mut indexes = Vec::new();
            for (path_step, step) in projections.iter().enumerate() {
                let validation::ScalarCoordinateProjection::Element {
                    selector: expression,
                    collection_type,
                    element_type,
                    collection_path,
                    source_step,
                    extent,
                } = step
                else {
                    continue;
                };
                let ScalarProjection::Element { selector, .. } = captured.get(*source_step)? else {
                    return None;
                };
                let extent = extent.map(u64::try_from).transpose().ok()?;
                let length = if let Some(extent) = extent {
                    ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(extent))
                } else {
                    self.coordinate_collection_length(owner, collection_path)?
                };
                if !self.proves_values_relation(
                    self.program,
                    self.machine,
                    typed_trees::expression::BinaryOperator::GreaterOrEqual,
                    selector,
                    &ScopedArithmeticValue::Integer(numerics::bignum::BigInt::zero()),
                ) || !self.proves_values_relation(
                    self.program,
                    self.machine,
                    typed_trees::expression::BinaryOperator::Less,
                    selector,
                    &length,
                ) {
                    return None;
                }
                let primitive_type = match expression {
                    checked_trees::CheckedEntryIndexSelector::Expression(expression) => {
                        let reference = validation::expression_result_type_reference(
                            self.program,
                            self.machine,
                            self.state,
                            *expression,
                        )?;
                        integer_carrier(self.program, reference)?
                    }
                    checked_trees::CheckedEntryIndexSelector::Constant(_) => PrimitiveType::U64,
                };
                indexes.push(checked_trees::CheckedEntryIndexBound {
                    state: self.state.symbol,
                    parameter: owner,
                    path_step: u32::try_from(path_step).ok()?,
                    selector: expression.clone(),
                    collection_path: collection_path.clone(),
                    collection_type: *collection_type,
                    element_type: *element_type,
                    extent,
                    primitive_type,
                });
            }
            Some(indexes)
        }

        // Bind every selector in a contract path before comparing it with a
        // caller path. The stored rows are inputs, not a selected case or value.
        pub(crate) fn call_argument_projection(
            &self,
            path: &[facts::PlaceSegment],
            callee: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            parameters: &[typed_trees::signature::StateParameter],
            arguments: &[ExpressionHandle],
        ) -> Option<CallArgumentProjection> {
            if parameters.len() != arguments.len() {
                return None;
            }
            let bindings = parameters
                .iter()
                .zip(arguments)
                .filter(|(parameter, _)| {
                    integer_carrier(self.program, parameter.type_reference).is_some()
                })
                .map(|(parameter, argument)| Binding {
                    symbol: parameter.symbol,
                    name: parameter.name.to_string(),
                    value: self.capture(self.program, self.machine, self.state, *argument),
                    alternatives: Vec::new(),
                    immutable_coordinate: true,
                    unwritable: true,
                })
                .collect::<Vec<_>>();
            let fields = parameters
                .iter()
                .zip(arguments)
                .flat_map(|(formal, actual)| {
                    self.captured_scalar_fields(*actual, 0)
                        .into_iter()
                        .map(|field| ScalarField::for_owner(formal.symbol, field))
                })
                .collect::<Vec<_>>();
            let occurrence = self.occurrence_at(self.program, callee, state, &[]);
            let bound = capture_path(path, |selector| {
                coordinates::selector_value(occurrence, selector, &bindings, &fields)
            })?;
            Some(CallArgumentProjection(bound))
        }

        pub(crate) fn matches_call_argument_projection(
            &self,
            path: &[facts::PlaceSegment],
            prefix: &[facts::PlaceSegment],
            required: &CallArgumentProjection,
        ) -> bool {
            let Some(mut required_path) = capture_path(prefix, |selector| {
                coordinates::selector_value(
                    self.occurrence(),
                    selector,
                    &self.current,
                    &self.fields,
                )
            }) else {
                return false;
            };
            required_path.extend_from_slice(&required.0);
            let observed = capture_path(path, |selector| {
                coordinates::selector_value(
                    self.occurrence(),
                    selector,
                    &self.current,
                    &self.fields,
                )
            });
            observed.is_some_and(|observed| {
                same_projection(self.occurrence(), &observed, &required_path)
            })
        }

        /*
            Use the argument values passed to a recursive call before resolving
            a field or element. Its index belongs to this invocation, rather than
            the earlier parameter declaration.
        */
        pub(crate) fn projected_scalar_value_in_call(
            &self,
            actual: ExpressionHandle,
            path: &[facts::PlaceSegment],
            callee: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            parameters: &[typed_trees::signature::StateParameter],
            arguments: &[ExpressionHandle],
        ) -> Option<ScopedArithmeticValue> {
            if path.is_empty() {
                return self.capture(self.program, self.machine, self.state, actual);
            }
            let path = self.call_argument_projection(path, callee, state, parameters, arguments)?;
            self.captured_scalar_fields(actual, 0)
                .into_iter()
                .find(|field| same_projection(self.occurrence(), &field.path, &path.0))
                .map(|field| field.value)
        }

        /*
            Transfer only values from this executed expression or the live roster.
            Unmodeled producers and unequal joins supply no field equality.
        */
        pub(super) fn captured_scalar_fields(
            &self,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Vec<CapturedScalarField> {
            let reference = validation::expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                expression,
            )
            .unwrap_or_default();
            self.captured_scalar_fields_of_type(expression, reference, depth)
        }

        /*
            The producing declaration supplies constructor element/field carriers;
            anonymous literals need no standalone type guess. This context grants
            no qualification: values still come only from completed source capture.
        */
        pub(super) fn captured_scalar_fields_of_type(
            &self,
            expression: ExpressionHandle,
            reference: typed_trees::types::TypeReferenceHandle,
            depth: usize,
        ) -> Vec<CapturedScalarField> {
            if depth >= 64 {
                return Vec::new();
            }
            let mut captured = match self.program.expression_table.expression(expression) {
                ExpressionNode::Borrow(borrow) => {
                    let referee = validation::reference_shell(self.program, reference)
                        .map(|shell| shell.referee);
                    if let Some(referee) = referee {
                        self.captured_scalar_fields_of_type(borrow.target, referee, depth + 1)
                    } else {
                        self.captured_scalar_fields(borrow.target, depth + 1)
                    }
                }
                ExpressionNode::Call(_) => {
                    let mut fields: Vec<CapturedScalarField> = Vec::new();
                    for observation in validation::scoped_call_result_observations(
                        self.program,
                        self.machine,
                        self.state,
                        expression,
                    ) {
                        let Some(path) = capture_path(&observation.path, |_| None) else {
                            continue;
                        };
                        if !fields
                            .iter()
                            .any(|field| same_projection(self.occurrence(), &field.path, &path))
                        {
                            fields.push(CapturedScalarField {
                                path,
                                carrier: observation.primitive_type,
                                value: observation.value,
                            });
                        }
                    }
                    fields
                }
                ExpressionNode::Match(dispatch) => {
                    self.capture_join_fields(expression, dispatch, depth)
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    self.capture_array_elements(*elements, reference, depth)
                }
                ExpressionNode::Indexed(indexed)
                    if validation::has_builtin_subslice_meaning(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    ) =>
                {
                    self.capture_window_elements(expression, indexed, depth)
                        .unwrap_or_default()
                }
                ExpressionNode::StructLiteral(literal) => {
                    let Some(definition) = validation::selected_construction_definition(
                        self.program,
                        expression,
                        literal,
                    ) else {
                        return Vec::new();
                    };
                    let selected_case = literal.case_symbol.and_then(|_| {
                        validation::selected_construction_case(
                            self.program,
                            expression,
                            literal,
                            definition,
                        )
                    });
                    if literal.case_symbol.is_some() && selected_case.is_none() {
                        return Vec::new();
                    }
                    let mut captured = Vec::new();
                    for field in self.program.expression_table.struct_fields(literal.fields) {
                        if !field.field_symbol.is_valid() {
                            continue;
                        }
                        let Some(declared) = validation::selected_construction_field(
                            self.program,
                            expression,
                            literal,
                            field.field_symbol,
                        ) else {
                            continue;
                        };
                        let mut prefix = Vec::new();
                        if self.program.symbols.get(declared.symbol).parent != definition.symbol {
                            let Some(case) = selected_case else { continue };
                            if self.program.symbols.get(declared.symbol).parent != case.symbol {
                                continue;
                            }
                            prefix.push(ScalarProjection::Case(case.symbol));
                        }
                        prefix.push(ScalarProjection::Field(declared.symbol));
                        let declared_field = declared.type_reference;
                        for mut nested in self.captured_scalar_fields_of_type(
                            field.value,
                            declared_field,
                            depth + 1,
                        ) {
                            nested.path.splice(0..0, prefix.iter().cloned());
                            captured.push(nested);
                        }
                        let carrier = integer_carrier(self.program, declared_field);
                        if let Some(carrier) = carrier
                            && let Some(value) =
                                self.capture(self.program, self.machine, self.state, field.value)
                        {
                            captured.push(CapturedScalarField {
                                path: prefix.clone(),
                                carrier,
                                value,
                            });
                        }
                    }
                    captured
                }
                ExpressionNode::Name(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_) => {
                    let Some((owner, prefix)) = self.scalar_storage_path(expression) else {
                        return Vec::new();
                    };
                    self.fields
                        .iter()
                        .filter_map(|field| {
                            if field.owner != owner
                                || !projection_has_prefix(self.occurrence(), &field.path, &prefix)
                                || field.path.len() <= prefix.len()
                            {
                                return None;
                            }
                            Some(CapturedScalarField {
                                path: field.path[prefix.len()..].to_vec(),
                                carrier: field.carrier,
                                value: field.value.clone()?,
                            })
                        })
                        .collect()
                }
                _ => Vec::new(),
            };
            let borrowed = matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::Borrow(_)
            ) || validation::expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                expression,
            )
            .and_then(|reference| validation::reference_shell(self.program, reference))
            .is_some();
            if borrowed {
                let Some(frames) = validation::CallFrameResolver::new(self.program) else {
                    return Vec::new();
                };
                let paths = captured
                    .iter()
                    .map(|field| self.scalar_dependency_path(&field.path))
                    .collect::<Vec<_>>();
                let dependencies = frames.captured_reference_read_paths(
                    self.machine,
                    self.state,
                    expression,
                    &paths,
                );
                captured = captured
                    .into_iter()
                    .zip(dependencies)
                    .filter_map(|(field, paths)| paths.map(|_| field))
                    .collect();
            }
            captured
        }

        /*
            A dependency follows the saved selector, not a later value of its source
            binding. Unknown selectors remain conservative collection demands; only
            an independently exact captured integer can select a stored pointer slot.
        */
        fn scalar_dependency_path(&self, path: &[ScalarProjection]) -> Vec<facts::PlaceSegment> {
            let mut source = source_path(path);
            for (step, projection) in source.iter_mut().zip(path) {
                if let ScalarProjection::Element { selector, .. } = projection
                    && let Some(index) = self
                        .exact_length(selector)
                        .and_then(|value| value.to_u64())
                        .and_then(|value| usize::try_from(value).ok())
                {
                    *step = facts::PlaceSegment::FixedIndex { index };
                }
            }
            source
        }

        pub(super) fn replace_scalar_fields(
            &mut self,
            owner: SymbolHandle,
            prefix: &[ScalarProjection],
            fields: Vec<CapturedScalarField>,
        ) {
            let occurrence = ScopedOccurrence {
                program: self.program,
                machine: self.machine,
                state: self.state,
                lengths: &self.lengths,
                hypotheses: &self.hypotheses,
                result_fields: None,
            };
            self.fields.retain(|field| {
                field.owner != owner
                    || !(projection_has_prefix(occurrence, &field.path, prefix)
                        || projection_has_prefix(occurrence, prefix, &field.path))
            });
            for mut field in fields {
                let mut path = prefix.to_vec();
                path.extend(field.path);
                field.path = path;
                self.fields.push(ScalarField::for_owner(owner, field));
            }
        }
    }
}
mod scalar_storage {
    /*
        One completed scalar store updates its physical storage associations.
        Capture RHS first, authenticate exact locations at the same effect cut,
        retire overwritten observations, then publish the new value.
        Typed origins establish equality; the storage graph expires observations.
    */
    use super::scalar_fields::capture_path;
    use super::scalar_fields::same_projection;
    use super::scalar_fields::{CapturedScalarField, ScalarProjection, source_path};
    use super::{CurrentScalarValues, integer_carrier, unsigned};
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::PrimitiveType;
    use validation::ScopedArithmeticValue;

    /* A current storage association, refreshed at the store's observation cut. */
    pub(super) struct ScalarStoreAlias {
        owner: SymbolHandle,
        path: Vec<ScalarProjection>,
    }

    impl CurrentScalarValues<'_> {
        /*
            Capture a scalar store's completed value before its target write retires
            associations. An opaque result gets one fresh store identity, no bits.
        */
        pub(super) fn captured_scalar_store(
            &self,
            target: ExpressionHandle,
            value: Option<ScopedArithmeticValue>,
            ordinal: usize,
        ) -> Option<CapturedScalarField> {
            let (owner, path) = self.scalar_storage_path(target)?;
            let reference = validation::expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                target,
            )?;
            let reference = validation::reference_shell(self.program, reference)
                .map_or(reference, |shell| shell.referee);
            let carrier = integer_carrier(self.program, reference)?;
            let value = value.unwrap_or_else(|| ScopedArithmeticValue::Atom {
                identity: format!(
                    "state:{:?}:store:{ordinal}:field:{owner:?}:{path:?}:{carrier:?}",
                    self.state.symbol
                ),
                unsigned: unsigned(carrier),
            });
            Some(CapturedScalarField {
                path: Vec::new(),
                carrier,
                value,
            })
        }

        /*
            Capture the current scalar behind a newly formed reference. The source
            owner supplies exact load evidence; no initializer is replayed later.
        */
        pub(super) fn reference_scalar_initializer(
            &self,
            statement: &typed_trees::statement::StatementNode,
            expression: ExpressionHandle,
            primitive: PrimitiveType,
            frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Option<ScopedArithmeticValue> {
            let frames = frames?;
            if !frames
                .expression_write_frame(self.machine, expression)
                .into_complete_paths()
                .is_some_and(|paths| paths.is_empty())
            {
                return None;
            }
            let sources = frames
                .reference_expression_sources_before_statement(self.machine, statement, expression)
                .filter(|sources| !sources.is_empty())?;
            let values = sources
                .iter()
                .map(|source| {
                    if source.collection_coarse
                        || !source
                            .scalar_storage_coordinate(self.program, self.state)
                            .is_some_and(|scalar| {
                                scalar.preserves_scalar_value
                                    && scalar.source_primitive == primitive
                                    && scalar.view_primitive == primitive
                            })
                    {
                        return None;
                    }
                    let root = source.root;
                    let path = &source.segments;
                    if path.is_empty() {
                        return self.current_symbol(root);
                    }
                    let path = capture_path(path, |selector| {
                        self.capture(self.program, self.machine, self.state, selector)
                    })?;
                    self.fields
                        .iter()
                        .find(|field| {
                            field.owner == root
                                && same_projection(self.occurrence(), &field.path, &path)
                        })?
                        .value
                        .clone()
                })
                .collect::<Option<Vec<_>>>()?;
            let first = values.first()?;
            values
                .iter()
                .all(|value| value == first)
                .then(|| first.clone())
        }

        /*
            A store publishes one completed scalar to every proven identical live
            location. Invalidation footprints cannot establish this equality. Use
            exact prefix sources, and keep owned copies at their distinct homes.
            Effectful operands have a later observation cut and cannot use this
            pre-statement correspondence; their ordinary invalidation still applies.
        */
        pub(super) fn scalar_store_aliases(
            &self,
            statement: &typed_trees::statement::StatementNode,
            ordinal: usize,
            carrier: PrimitiveType,
            frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Vec<ScalarStoreAlias> {
            let Some(frames) = frames else {
                return Vec::new();
            };
            let typed_trees::statement::StatementNode::Assignment(assignment) = statement else {
                return Vec::new();
            };
            if [assignment.target, assignment.value]
                .iter()
                .any(|expression| {
                    !frames
                        .expression_write_frame(self.machine, *expression)
                        .into_complete_paths()
                        .is_some_and(|paths| paths.is_empty())
                })
            {
                return Vec::new();
            }
            let Ok(origins) = frames.assignment_storage_origins(self.machine, statement) else {
                return Vec::new();
            };
            let [destination] = origins.destinations.as_slice() else {
                return Vec::new();
            };
            if destination.collection_coarse
                || !destination.root.is_valid()
                || destination.invocation_output.is_some()
                || destination.is_image_storage
            {
                return Vec::new();
            }
            let candidates = self
                .fields
                .iter()
                .filter(|field| field.carrier == carrier)
                .map(|field| (field.owner, field.path.clone()))
                .chain(
                    self.current
                        .iter()
                        .filter(|binding| !binding.immutable_coordinate)
                        .map(|binding| (binding.symbol, Vec::new())),
                );
            candidates
                .filter_map(|(owner, path)| {
                    let coordinate = source_path(&path);
                    let sources = frames.coordinate_sources_before_statement(
                        self.machine,
                        self.state,
                        ordinal,
                        owner,
                        &coordinate,
                    )?;
                    let [source] = sources.as_slice() else {
                        return None;
                    };
                    (!source.collection_coarse
                        && source.root == destination.root
                        && source.segments == destination.segments
                        && source.recasts == destination.recasts
                        && source.representation == destination.representation
                        && source.invocation_output == destination.invocation_output
                        && source.is_image_storage == destination.is_image_storage)
                        .then_some(ScalarStoreAlias { owner, path })
                })
                .collect()
        }

        pub(super) fn publish_scalar_store(
            &mut self,
            aliases: &[ScalarStoreAlias],
            value: &ScopedArithmeticValue,
        ) {
            for binding in &mut self.current {
                if aliases
                    .iter()
                    .any(|alias| alias.owner == binding.symbol && alias.path.is_empty())
                {
                    binding.value = Some(value.clone());
                    binding.alternatives.clear();
                }
            }
            let occurrence = super::ScopedOccurrence {
                program: self.program,
                machine: self.machine,
                state: self.state,
                lengths: &self.lengths,
                hypotheses: &self.hypotheses,
                result_fields: None,
            };
            for field in &mut self.fields {
                if aliases.iter().any(|alias| {
                    alias.owner == field.owner
                        && same_projection(occurrence, &alias.path, &field.path)
                }) {
                    field.value = Some(value.clone());
                }
            }
        }
    }
}
use conditions::ConditionBinding;
use scalar_fields::{ScalarField, ScalarProjection, scalar_field_occurrence};

#[derive(Clone)]
struct Binding {
    symbol: SymbolHandle,
    name: String,
    value: Option<ScopedArithmeticValue>,
    alternatives: Vec<ScopedArithmeticValue>,
    immutable_coordinate: bool,
    /*
        A by-value binding declared without `mut`. No store or call can reach
        it: a write needs a mutable place, and so does a `&mut` loan. Its
        current value therefore survives every effect even when the input has
        no storage graph to ask, as in build-time admission over bare typed
        trees. Unlike `immutable_coordinate`, it remains an ordinary store-
        alias candidate.
    */
    unwritable: bool,
}

#[derive(Clone, PartialEq, Eq)]
enum LengthSubject {
    Storage(SymbolHandle),
    Result(SymbolHandle),
    /*
        One captured descriptor, identified by its complete static storage path.
        The owner is the current storage binding, not the source producer.
    */
    Field {
        owner: SymbolHandle,
        path: Vec<facts::PlaceSegment>,
    },
    /*
        One slice field of the value returned from this machine. The owner is
        the machine, and `result.field.len` reads this observation.
    */
    ResultField {
        owner: SymbolHandle,
        path: Vec<facts::PlaceSegment>,
    },
}

/* A captured descriptor observation, not the capacity of its backing owner. */
#[derive(Clone)]
struct Length {
    subject: LengthSubject,
    name: String,
    stable: bool,
    type_reference: TypeReferenceHandle,
    value: Option<ScopedArithmeticValue>,
}

/*
    The value a tracked entry holds, compared to find the entries a statement
    established or changed.
*/
#[derive(PartialEq)]
enum EntryValue {
    Binding(Option<ScopedArithmeticValue>, Vec<ScopedArithmeticValue>),
    Value(Option<ScopedArithmeticValue>),
    Condition(
        Vec<ScopedArithmeticHypothesis>,
        Vec<ScopedArithmeticHypothesis>,
    ),
}

/* A storage length's graph place. A result length names no storage. */
fn length_place(subject: &LengthSubject) -> Option<validation::CanonicalPlace> {
    match subject {
        LengthSubject::Storage(symbol) => Some(validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(*symbol),
            segments: Vec::new(),
        }),
        LengthSubject::Field { owner, path } => Some(validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(*owner),
            segments: path.clone(),
        }),
        LengthSubject::Result(_) | LengthSubject::ResultField { .. } => None,
    }
}

pub(crate) struct CurrentScalarValues<'program> {
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    statement_index: usize,
    initial: Vec<Binding>,
    current: Vec<Binding>,
    lengths: Vec<Length>,
    initial_lengths: Vec<Length>,
    fields: Vec<ScalarField>,
    result_fields: Option<CapturedResultFields>,
    conditions: Vec<ConditionBinding>,
    hypotheses: Vec<ScopedArithmeticHypothesis>,
    in_transition_target: bool,
    scalar_observations: std::cell::OnceCell<Vec<validation::CapturedScalarObservation>>,
    /*
        The snapshot each tracked place's entry was established at; absent is
        incoming storage.
    */
    anchors: Vec<(
        validation::CanonicalPlace,
        validation::completed_storage::versions::Snapshot,
    )>,
    /* Lent by the state's pass for the replay only; empty otherwise. */
    completed_calls: CompletedCallGuarantees,
}

pub(crate) use call_contracts::CompletedCallGuarantees;

// A declaration path after its selectors have been bound to one caller frame.
// Only the frame's typed projection operations can construct or compare it.
pub(crate) struct CallArgumentProjection(Vec<ScalarProjection>);

impl<'program> CurrentScalarValues<'program> {
    pub(crate) fn machine(&self) -> &Machine {
        self.machine
    }
    pub(crate) fn state(&self) -> &State {
        self.state
    }

    pub(crate) fn program(&self) -> &'program dyn ::validation::CheckingInput {
        self.program
    }

    pub(crate) fn at(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
        statement_index: usize,
        target: TransitionTargetHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Self> {
        Self::at_observing_projections(
            program,
            machine,
            state,
            statement_index,
            target,
            frames,
            &[],
        )
    }

    pub(crate) fn at_observing_projections(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
        statement_index: usize,
        target: TransitionTargetHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
        requested: &[Vec<facts::PlaceSegment>],
    ) -> Option<Self> {
        Self::at_completing(
            program,
            machine,
            state,
            statement_index,
            target,
            frames,
            requested,
            &mut CompletedCallGuarantees::default(),
        )
    }

    /*
        `at_observing_projections`, reusing the completed-call hypotheses
        `completed` already holds for this state; see `CompletedCallGuarantees`.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn at_completing(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
        statement_index: usize,
        target: TransitionTargetHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
        requested: &[Vec<facts::PlaceSegment>],
        completed: &mut CompletedCallGuarantees,
    ) -> Option<Self> {
        let mut initial = program
            .state_parameters(state)
            .iter()
            .filter_map(|parameter| {
                let (primitive, readable) = if let Some(reference) =
                    validation::reference_shell(program, parameter.type_reference)
                {
                    if !validation::state_reference_parameter_binding_is_stable(
                        program,
                        machine,
                        state,
                        parameter.symbol,
                    ) {
                        return None;
                    }
                    (
                        integer_carrier(program, reference.referee)?,
                        reference.access.is_readable(),
                    )
                } else {
                    (integer_carrier(program, parameter.type_reference)?, true)
                };
                Some(Binding {
                    symbol: parameter.symbol,
                    name: parameter.name.to_string(),
                    value: readable.then(|| {
                        entry_fields::entry_value(
                            program,
                            validation::reference_shell(program, parameter.type_reference)
                                .map_or(parameter.type_reference, |shell| shell.referee),
                            None,
                            ScopedArithmeticValue::Atom {
                                identity: format!(
                                    "state:{:?}:parameter:{:?}",
                                    state.symbol, parameter.symbol
                                ),
                                unsigned: unsigned(primitive),
                            },
                        )
                    }),
                    alternatives: Vec::new(),
                    immutable_coordinate: false,
                    unwritable: !parameter.is_mutable
                        && validation::reference_shell(program, parameter.type_reference).is_none(),
                })
            })
            .collect::<Vec<_>>();
        initial.extend(
            program
                .machine_type_parameters(machine)
                .iter()
                .filter_map(|parameter| {
                    let reference = match parameter.kind {
                        typed_trees::data::TypeParameterKind::Value { type_reference }
                        | typed_trees::data::TypeParameterKind::Const { type_reference } => {
                            type_reference
                        }
                        _ => return None,
                    };
                    let primitive = integer_carrier(program, reference)?;
                    Some(Binding {
                        symbol: parameter.symbol,
                        name: parameter.name.to_string(),
                        value: Some(entry_fields::entry_value(
                            program,
                            reference,
                            None,
                            ScopedArithmeticValue::Atom {
                                identity: format!(
                                    "machine:{:?}:value:{:?}",
                                    machine.symbol, parameter.symbol
                                ),
                                unsigned: unsigned(primitive),
                            },
                        )),
                        alternatives: Vec::new(),
                        immutable_coordinate: true,
                        unwritable: true,
                    })
                }),
        );
        let (declaration_scope, _) =
            program.machine_declaration_scope(machine, |left, right| left.symbol == right.symbol);
        for parameter in declaration_scope {
            if initial
                .iter()
                .any(|binding| binding.symbol == parameter.symbol)
            {
                continue;
            }
            let reference = match parameter.kind {
                typed_trees::data::TypeParameterKind::Value { type_reference }
                | typed_trees::data::TypeParameterKind::Const { type_reference } => type_reference,
                _ => continue,
            };
            let Some(primitive) = integer_carrier(program, reference) else {
                continue;
            };
            initial.push(Binding {
                symbol: parameter.symbol,
                name: parameter.name.to_string(),
                value: Some(entry_fields::entry_value(
                    program,
                    reference,
                    None,
                    ScopedArithmeticValue::Atom {
                        identity: format!(
                            "machine:{:?}:value:{:?}",
                            machine.symbol, parameter.symbol
                        ),
                        unsigned: unsigned(primitive),
                    },
                )),
                alternatives: Vec::new(),
                immutable_coordinate: true,
                unwritable: true,
            });
        }
        let lengths = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                validation::state_reference_parameter_binding_is_stable(
                    program,
                    machine,
                    state,
                    parameter.symbol,
                )
            })
            .filter_map(|parameter| {
                Some(Length {
                    subject: LengthSubject::Storage(parameter.symbol),
                    name: parameter.name.to_string(),
                    stable: true,
                    type_reference: parameter.type_reference,
                    value: Some(entry_collection_length(
                        program,
                        parameter.type_reference,
                        || {
                            format!(
                                "state:{:?}:parameter:{:?}:length",
                                state.symbol, parameter.symbol
                            )
                        },
                    )?),
                })
            })
            .collect();
        let mut values = Self {
            program,
            machine,
            state,
            statement_index,
            current: initial.clone(),
            initial,
            lengths,
            initial_lengths: Vec::new(),
            fields: Vec::new(),
            result_fields: None,
            conditions: Vec::new(),
            hypotheses: Vec::new(),
            in_transition_target: target.is_valid(),
            scalar_observations: std::cell::OnceCell::new(),
            anchors: Vec::new(),
            completed_calls: std::mem::take(completed),
        };
        let replayed = Self::replay_prefix(&mut values, statement_index, target, frames, requested);
        *completed = std::mem::take(&mut values.completed_calls);
        replayed.map(|()| values)
    }

    fn replay_prefix(
        values: &mut Self,
        statement_index: usize,
        target: TransitionTargetHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
        requested: &[Vec<facts::PlaceSegment>],
    ) -> Option<()> {
        let (program, machine, state) = (values.program, values.machine, values.state);
        let owned_frames = frames
            .is_none()
            .then(|| validation::CallFrameResolver::new(program))
            .flatten();
        let frames = frames.or(owned_frames.as_ref());
        let mut field_paths = parameter_projection_paths(program, machine, state);
        for path in requested {
            if !field_paths.contains(path) {
                field_paths.push(path.clone());
            }
        }
        values.seed_parameter_fields(&field_paths, frames);
        values.initial_lengths = values.lengths.clone();
        let entry = program.machine_states(machine).first()?;
        let contracts = program.state_contracts(state).iter().chain(
            program
                .machine_contracts(machine)
                .iter()
                .filter(|_| entry.symbol == state.symbol),
        );
        for contract in contracts {
            if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
                continue;
            }
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let Some(expression) = fact.as_expression() {
                    values.assume(program, machine, state, *expression, true);
                }
            }
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        if statement_index > statements.len() {
            return None;
        }
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let position = order.position(statement_index)?;
        for &ordinal in &order.indices()[..position] {
            let statement = &statements[ordinal];
            if validation::is_arm_pattern_marker(statement) {
                continue;
            }
            let before = values
                .program
                .storage_graph()
                .map(|_| values.anchored_entries());
            match statement {
                StatementNode::Transition(transition) => {
                    let TransitionGuardNode::When(guard) = transition.guard else {
                        return None;
                    };
                    values.retire_operand_effects(guard);
                    let holds = match (
                        transition.target.is_valid(),
                        transition.continuation.is_valid(),
                    ) {
                        (true, false) => false,
                        (false, true) => true,
                        _ => return None,
                    };
                    values.assume(program, machine, state, guard, holds);
                }
                StatementNode::LocalData(local) if !local.role.has_ordinary_initializer() => {}
                StatementNode::LocalData(local) => {
                    values.retire_operand_effects(local.initial_value);
                    // An unnamed temporary of the initializer dies inside this
                    // statement, and its cleanup may write a tracked place. The
                    // statement snapshot carries those writes; only a statement
                    // that has any pays for the walk.
                    if frames.is_none_or(|frames| {
                        frames
                            .statement_cleanup_write_frame(machine, statement)
                            .complete_paths()
                            .is_none_or(|paths| !paths.is_empty())
                    }) {
                        let at = values.statement_snapshot(ordinal);
                        values.retire_unpreserved(at);
                    }
                    let actuals = values.capture_call_actuals(local.initial_value);
                    let value = values.capture(program, machine, state, local.initial_value);
                    let conditions = values.capture_conditions(local.initial_value, 0);
                    values.replace_conditions(local.symbol, &[], conditions);
                    values.capture_collection(
                        local.symbol,
                        local.name.as_str(),
                        local.type_reference,
                        local.initial_value,
                        ordinal,
                    );
                    let scalar_fields = values.captured_scalar_fields_of_type(
                        local.initial_value,
                        local.type_reference,
                        0,
                    );
                    values.replace_scalar_fields(local.symbol, &[], scalar_fields);
                    values.capture_record_fields(local.symbol, &[], local.initial_value);
                    if integer_carrier(program, local.type_reference).is_none() {
                        values.assume_aggregate_call_ensures(
                            local.initial_value,
                            &actuals,
                            ordinal,
                        );
                    }
                    let reference_scalar =
                        validation::reference_shell(program, local.type_reference).and_then(
                            |shell| {
                                integer_carrier(program, shell.referee)
                                    .map(|primitive| (primitive, shell.access.is_readable()))
                            },
                        );
                    if let Some(primitive) = reference_scalar
                        .map(|(primitive, _)| primitive)
                        .or_else(|| integer_carrier(program, local.type_reference))
                    {
                        let value = if let Some((_, readable)) = reference_scalar {
                            let value = values.reference_scalar_initializer(
                                statement,
                                local.initial_value,
                                primitive,
                                frames,
                            );
                            readable.then_some(value).flatten()
                        } else {
                            value
                        };
                        let alternatives = values.capture_alternatives(
                            program,
                            machine,
                            state,
                            local.initial_value,
                        );
                        let value = value.or_else(|| {
                            (!reference_scalar.is_some_and(|(_, readable)| !readable))
                                .then(|| observed_scalar(state, local.symbol, ordinal, primitive))
                        });
                        if let Some(result_value) = &value {
                            values.assume_call_ensures(
                                local.initial_value,
                                result_value,
                                &actuals,
                                ordinal,
                            );
                        }
                        values.current.push(Binding {
                            symbol: local.symbol,
                            name: local.name.to_string(),
                            value,
                            alternatives,
                            immutable_coordinate: false,
                            unwritable: !local.is_mutable && reference_scalar.is_none(),
                        });
                    }
                }
                StatementNode::Assignment(assignment) => {
                    values.retire_operand_effects(assignment.value);
                    let stored_at = values.statement_snapshot(ordinal);
                    let target_remains_current = frames.is_some_and(|frames| {
                        frames.assignment_destination_remains_current(machine, statement)
                    });
                    let actuals = values.capture_call_actuals(assignment.value);
                    let value = values.capture(program, machine, state, assignment.value);
                    let conditions = values.capture_conditions(assignment.value, 0);
                    let mut scalar_fields = validation::expression_result_type_reference(
                        program,
                        machine,
                        state,
                        assignment.target,
                    )
                    .map(|reference| {
                        values.captured_scalar_fields_of_type(assignment.value, reference, 0)
                    })
                    .unwrap_or_else(|| values.captured_scalar_fields(assignment.value, 0));
                    let scalar_store =
                        values.captured_scalar_store(assignment.target, value.clone(), ordinal);
                    let store_aliases = scalar_store.as_ref().map_or_else(Vec::new, |field| {
                        values.scalar_store_aliases(statement, ordinal, field.carrier, frames)
                    });
                    let stored_value = scalar_store.as_ref().map(|field| field.value.clone());
                    if let Some(field) = scalar_store
                        && values
                            .scalar_storage_path(assignment.target)
                            .is_some_and(|(_, path)| !path.is_empty())
                    {
                        scalar_fields.push(field);
                    }
                    let alternatives =
                        values.capture_alternatives(program, machine, state, assignment.value);
                    if stored_value.is_some() {
                        values.retire_unpreserved(stored_at);
                    }
                    let mut fields = values.captured_field_lengths(assignment.value, 0);
                    if let Some(value) = values.collection_length(assignment.value, 0)
                        && let Some(type_reference) = validation::expression_result_type_reference(
                            program,
                            machine,
                            state,
                            assignment.value,
                        )
                        && fixed_length(program, type_reference, String::new).is_some()
                    {
                        fields.push(collections::CapturedFieldLength {
                            path: Vec::new(),
                            name: String::new(),
                            type_reference,
                            value,
                        });
                    }
                    if target_remains_current
                        && let ExpressionNode::Name(name) =
                            program.expression_table.expression(assignment.target)
                        && name.symbol.is_valid()
                        && name.symbol == name.head_symbol
                        && values
                            .current
                            .iter()
                            .any(|binding| binding.symbol == name.symbol)
                    {
                        let next_value = value.or_else(|| stored_value.clone()).or_else(|| {
                            let reference = validation::expression_result_type_reference(
                                program,
                                machine,
                                state,
                                assignment.target,
                            )?;
                            let primitive = integer_carrier(program, reference)?;
                            Some(observed_scalar(state, name.symbol, ordinal, primitive))
                        });
                        if let Some(result_value) = &next_value {
                            values.assume_call_ensures(
                                assignment.value,
                                result_value,
                                &actuals,
                                ordinal,
                            );
                        }
                        if let Some(binding) = values
                            .current
                            .iter_mut()
                            .find(|binding| binding.symbol == name.symbol)
                        {
                            binding.value = next_value;
                            binding.alternatives = alternatives;
                        }
                    } else if target_remains_current
                        && let ExpressionNode::Name(name) =
                            program.expression_table.expression(assignment.target)
                        && name.symbol == name.head_symbol
                        && let Some(length) = values
                            .lengths
                            .iter()
                            .find(|length| length.subject == LengthSubject::Storage(name.symbol))
                            .cloned()
                    {
                        values.capture_collection(
                            name.symbol,
                            &length.name,
                            length.type_reference,
                            assignment.value,
                            ordinal,
                        );
                    } else {
                        values.retire_unpreserved(stored_at);
                    }
                    if target_remains_current
                        && let Some((owner, path)) = values.scalar_storage_path(assignment.target)
                    {
                        values.replace_scalar_fields(owner, &path, scalar_fields);
                        if stored_value.is_none() {
                            values.assume_aggregate_call_ensures(
                                assignment.value,
                                &actuals,
                                ordinal,
                            );
                        }
                    }
                    if target_remains_current
                        && let Some((owner, path)) = field_storage_path(program, assignment.target)
                    {
                        values.replace_conditions(owner, &path, conditions);
                    }
                    if target_remains_current
                        && validation::value_has_builtin_coordinates(
                            program,
                            machine,
                            state,
                            assignment.target,
                        )
                        && let Some((owner, path)) =
                            descriptor_storage_path(program, assignment.target)
                    {
                        values.replace_record_fields(owner, &path, fields);
                    }
                    if let Some(value) = stored_value {
                        values.publish_scalar_store(&store_aliases, &value);
                    }
                }
                StatementNode::Call(call) => {
                    let at = values.statement_snapshot(ordinal);
                    values.retire_unpreserved(at);
                    values.assume_statement_ensures(call, ordinal);
                }
                StatementNode::Expression(_) => {
                    let at = values.statement_snapshot(ordinal);
                    values.retire_unpreserved(at)
                }
                StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
            }
            if let Some(before) = before {
                values.anchor_changed_entries(&before, ordinal);
            }
        }
        if target.is_valid() {
            let transition = statements.get(statement_index)?.as_transition()?;
            if transition.target != target && transition.continuation != target {
                return None;
            }
            if let TransitionGuardNode::When(guard) = transition.guard {
                values.retire_operand_effects(guard);
                values.assume(program, machine, state, guard, transition.target == target);
            }
        }
        if target.is_valid()
            && let typed_trees::statement::TransitionTargetNode::Named { arguments, .. } =
                program.statement_table.transition_target(target)
        {
            for argument in program.statement_table.expression_handles(*arguments) {
                values.retire_operand_effects(*argument);
            }
        }
        Some(())
    }

    /*
        Retire the entries the evaluation of `expression` may have written. An
        operand flow recorded no position for runs no call or atomic access, so
        its evaluation wrote nothing.
    */
    pub(crate) fn retire_operand_effects(&mut self, expression: ExpressionHandle) {
        if let Some(at) = self.operand_snapshot(expression) {
            self.retire_unpreserved(Some(at));
        } else if validation::executable_subtree_any(
            self.program,
            expression,
            typed_trees::SubtreeEdges::VALUES,
            |_, node| match node {
                ExpressionNode::Call(_) | ExpressionNode::Atomic(_) => {
                    typed_trees::SubtreeVisit::Found
                }
                _ => typed_trees::SubtreeVisit::Descend,
            },
        ) {
            self.retire_unpreserved(None);
        }
    }

    fn operand_snapshot(
        &self,
        expression: ExpressionHandle,
    ) -> Option<validation::completed_storage::versions::Snapshot> {
        validation::completed_storage::versions::operand_snapshot(
            self.program.storage_graph()?,
            self.state.symbol,
            expression,
        )
    }

    fn statement_snapshot(
        &self,
        ordinal: usize,
    ) -> Option<validation::completed_storage::versions::Snapshot> {
        validation::completed_storage::versions::statement_snapshot(
            self.program,
            self.program.storage_graph()?,
            self.state.symbol,
            ordinal,
        )
    }

    /* Each tracked entry with a graph place, with the value it holds. */
    fn anchored_entries(&self) -> Vec<(validation::CanonicalPlace, EntryValue)> {
        let place =
            |root: SymbolHandle, segments: Vec<facts::PlaceSegment>| validation::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(root),
                segments,
            };
        let field_path = |path: &[SymbolHandle]| {
            path.iter()
                .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                .collect::<Vec<_>>()
        };
        let mut entries = Vec::new();
        for binding in &self.current {
            entries.push((
                place(binding.symbol, Vec::new()),
                EntryValue::Binding(binding.value.clone(), binding.alternatives.clone()),
            ));
        }
        for field in &self.fields {
            entries.push((
                place(field.owner, scalar_fields::source_path(&field.path)),
                EntryValue::Value(field.value.clone()),
            ));
        }
        for condition in &self.conditions {
            entries.push((
                place(condition.owner, field_path(&condition.path)),
                EntryValue::Condition(
                    condition.hypotheses(true).to_vec(),
                    condition.hypotheses(false).to_vec(),
                ),
            ));
        }
        for length in &self.lengths {
            let Some(length_place) = length_place(&length.subject) else {
                continue;
            };
            entries.push((length_place, EntryValue::Value(length.value.clone())));
        }
        entries
    }

    /* Anchor every entry this statement established or changed at its snapshot. */
    fn anchor_changed_entries(
        &mut self,
        before: &[(validation::CanonicalPlace, EntryValue)],
        ordinal: usize,
    ) {
        let Some(snapshot) = self.statement_snapshot(ordinal) else {
            return;
        };
        for (place, value) in self.anchored_entries() {
            if before
                .iter()
                .any(|(earlier, earlier_value)| *earlier == place && *earlier_value == value)
            {
                continue;
            }
            self.anchor_at(place, snapshot);
        }
    }

    fn anchor_at(
        &mut self,
        place: validation::CanonicalPlace,
        snapshot: validation::completed_storage::versions::Snapshot,
    ) {
        match self
            .anchors
            .iter_mut()
            .find(|(anchored, _)| *anchored == place)
        {
            Some((_, anchor)) => *anchor = snapshot,
            None => self.anchors.push((place, snapshot)),
        }
    }

    fn anchor_of(
        &self,
        place: &validation::CanonicalPlace,
    ) -> validation::completed_storage::versions::Snapshot {
        self.anchors
            .iter()
            .find(|(anchored, _)| anchored == place)
            .map_or_else(arena::Handle::invalid, |(_, anchor)| *anchor)
    }

    /*
        Retire, at version `at`, every tracked entry whose place a write
        reached since its anchor. Machine Value/Const coordinates belong to this
        invocation, not addressable storage, and a stable descriptor's length
        survives endpoint stores. Without a graph position the effect is
        unknown: every other entry retires.

        A replay calls this at each write of the state prefix, in order. An
        entry that survives at `at` is re-anchored there, so the next call
        walks only the writes after `at` instead of the whole prefix again;
        a std program spent seconds repeating those walks. When `at` lies on
        every path to the next position, the shorter walk gives the same
        answer. When it does not, the walk runs past `at` to the state's
        start and the entry retires: a less precise answer, never an unsound
        one. An entry that already holds no value has nothing to retire and
        skips its walk.
    */
    pub(crate) fn retire_unpreserved(
        &mut self,
        at: Option<validation::completed_storage::versions::Snapshot>,
    ) {
        let graph = self.program.storage_graph();
        let mut survived = Vec::new();
        let mut preserved = |place: validation::CanonicalPlace| match (at, graph) {
            (Some(at), Some(graph)) => {
                let kept = validation::completed_storage::versions::place_preserved_since(
                    self.program,
                    graph,
                    at,
                    &place,
                    self.anchor_of(&place),
                );
                if kept {
                    survived.push(place);
                }
                kept
            }
            _ => false,
        };
        let bindings = self
            .current
            .iter()
            .map(|binding| {
                binding.immutable_coordinate
                    || binding.unwritable
                    || (binding.value.is_none() && binding.alternatives.is_empty())
                    || preserved(validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(binding.symbol),
                        segments: Vec::new(),
                    })
            })
            .collect::<Vec<_>>();
        let fields = self
            .fields
            .iter()
            .map(|field| {
                field.value.is_none()
                    || preserved(validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(field.owner),
                        segments: scalar_fields::source_path(&field.path),
                    })
            })
            .collect::<Vec<_>>();
        let conditions = self
            .conditions
            .iter()
            .map(|condition| {
                preserved(validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(condition.owner),
                    segments: condition
                        .path
                        .iter()
                        .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                        .collect(),
                })
            })
            .collect::<Vec<_>>();
        let lengths = self
            .lengths
            .iter()
            .map(|length| {
                length.stable
                    || length.value.is_none()
                    || length_place(&length.subject).map_or(at.is_some(), &mut preserved)
            })
            .collect::<Vec<_>>();
        if let Some(at) = at {
            for place in survived {
                self.anchor_at(place, at);
            }
        }
        for (binding, preserved) in self.current.iter_mut().zip(bindings) {
            if !preserved {
                binding.value = None;
                binding.alternatives.clear();
            }
        }
        for (field, preserved) in self.fields.iter_mut().zip(fields) {
            if !preserved {
                field.value = None;
            }
        }
        let mut conditions = conditions.into_iter();
        self.conditions
            .retain(|_| conditions.next().unwrap_or(false));
        for (length, preserved) in self.lengths.iter_mut().zip(lengths) {
            if !preserved {
                length.value = None;
            }
        }
    }

    fn assume(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        holds: bool,
    ) {
        let hypotheses = self.capture_condition(program, machine, state, expression, holds, 0);
        self.hypotheses.extend(hypotheses);
    }

    fn capture_condition(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        holds: bool,
        depth: usize,
    ) -> Vec<ScopedArithmeticHypothesis> {
        if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
            return Vec::new();
        }
        if let Some((owner, path)) = field_storage_path(program, expression)
            && let Some(condition) = self
                .conditions
                .iter()
                .find(|condition| condition.owner == owner && condition.path == path)
        {
            return condition.hypotheses(holds).to_vec();
        }
        if let Some(binary) = program.expression_table.binary_node(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            )
            && validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        {
            let wrapped = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (_, ExpressionNode::Boolean(value)) => Some((binary.left, *value)),
                (ExpressionNode::Boolean(value), _) => Some((binary.right, *value)),
                _ => None,
            };
            if let Some((operand, value)) = wrapped {
                return self.capture_condition(
                    program,
                    machine,
                    state,
                    operand,
                    holds == (value == (binary.operator == BinaryOperator::Equal)),
                    depth + 1,
                );
            }
        }
        if let Some(binary) = program.expression_table.binary_node(expression)
            && ((holds && binary.operator == BinaryOperator::And)
                || (!holds && binary.operator == BinaryOperator::Or))
            && validation::has_builtin_decomposed_guard_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        {
            let mut hypotheses =
                self.capture_condition(program, machine, state, binary.left, holds, depth + 1);
            for hypothesis in
                self.capture_condition(program, machine, state, binary.right, holds, depth + 1)
            {
                crate::shared::collections::push_unique(&mut hypotheses, hypothesis);
            }
            return hypotheses;
        }
        if let ExpressionNode::Unary(unary) = program.expression_table.expression(expression)
            && unary.operator == typed_trees::expression::UnaryOperator::LogicalNot
            && validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        {
            return self.capture_condition(
                program,
                machine,
                state,
                unary.operand,
                !holds,
                depth + 1,
            );
        }
        let occurrence = self.occurrence_at(program, machine, state, &self.lengths);
        if admitted(occurrence, expression, &self.current, &self.fields, 0)
            && let Some(proposition) = scoped(occurrence, expression, &self.current, &self.fields)
        {
            return vec![ScopedArithmeticHypothesis { proposition, holds }];
        }
        Vec::new()
    }

    fn capture(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticValue> {
        self.capture_at_depth(program, machine, state, expression, 0)
    }

    /*
        Capture an executed primitive's normal value, not a specification term.
        Exact and Trapping addition/subtraction/multiplication have their
        mathematical result on this continuation; a trap or an unproved exact
        overflow never reaches the use. Selection and the result carrier are
        checked independently. A completed unsigned remainder records its normal
        denotation on the operation itself, irrespective of policy. Other
        Wrapping and Saturating operations retain the engine's policy checks.
    */
    fn capture_at_depth(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<ScopedArithmeticValue> {
        if depth >= 64 {
            return None;
        }
        if let ExpressionNode::Name(name) = program.expression_table.expression(expression)
            && name.symbol == name.head_symbol
            && let Some(binding) = self
                .current
                .iter()
                .find(|binding| binding.symbol == name.symbol)
        {
            return binding.value.clone();
        }
        if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression)
            && let Some(value) = literal.value_i64()
        {
            return Some(ScopedArithmeticValue::Integer(
                numerics::bignum::BigInt::from_i64(value),
            ));
        }
        if let Some(call) = program.expression_table.call_node(expression)
            && let Some((low, high)) =
                validation::normal_return_integer_bounds(program, machine.symbol, expression, call)
        {
            if let (Some(low), Some(high)) = (low, high)
                && low == high
            {
                return Some(ScopedArithmeticValue::Integer(
                    numerics::bignum::BigInt::from_i64(low),
                ));
            }
            let reference =
                validation::expression_result_type_reference(program, machine, state, expression)?;
            let primitive = integer_carrier(program, reference)?;
            let arguments = program.expression_table.expression_handles(call.arguments);
            let actuals = validation::call_target_parameters(
                program,
                validation::call_target_symbol(program, call),
            )
            .filter(|parameters| parameters.len() == arguments.len())
            .map(|parameters| {
                parameters
                    .iter()
                    .zip(arguments)
                    .filter(|(parameter, _)| {
                        !parameter.is_mutable
                            && validation::reference_shell(program, parameter.type_reference)
                                .is_none()
                            && program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some_and(PrimitiveType::is_fixed_integer)
                    })
                    .filter_map(|(_, argument)| {
                        Some(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Occurrence(*argument),
                            value: self.capture_at_depth(
                                program,
                                machine,
                                state,
                                *argument,
                                depth + 1,
                            )?,
                        })
                    })
                    .collect()
            });
            let value = actuals.map_or_else(
                || ScopedArithmeticValue::Atom {
                    identity: format!("state:{:?}:call-result:{expression:?}", state.symbol),
                    unsigned: unsigned(primitive),
                },
                |bindings| {
                    ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                        owner: state.symbol,
                        expression,
                        bindings,
                    })
                },
            );
            return Some(ScopedArithmeticValue::BoundedInteger {
                value: Box::new(value),
                interval: BigIntInterval {
                    low: low.map(numerics::bignum::BigInt::from_i64),
                    high: high.map(numerics::bignum::BigInt::from_i64),
                },
            });
        }
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) {
            if let Some(value) =
                length_occurrence(program, machine, state, expression, &self.lengths)
                    .and_then(|length| length.value.clone())
            {
                return Some(value);
            }
            if let Some(value) =
                scalar_field_occurrence(self.occurrence(), expression, &self.current, &self.fields)
            {
                return Some(value);
            }
            if let Some(value) = field_occurrence(program, machine, state, expression) {
                return Some(value);
            }
            if let Some(observation) = self
                .scalar_observations
                .get_or_init(|| {
                    validation::captured_numeric_observations(
                        program,
                        machine,
                        state,
                        self.statement_index,
                        self.in_transition_target,
                        validation::NumericObservationDemand {
                            scalar_values: validation::ScalarObservationDemand::StatePrefix,
                            integer_operations: false,
                            scalar_associations: false,
                        },
                    )
                    .scalar_values
                })
                .iter()
                .find(|observation| observation.expression == expression)
            {
                return Some(observation.value.clone());
            }
        }
        if let ExpressionNode::Cast(_) = program.expression_table.expression(expression) {
            let operand = preserving_integer_cast_operand(
                self.occurrence(),
                expression,
                &self.current,
                &self.fields,
            )?;
            return self.capture_at_depth(program, machine, state, operand, depth + 1);
        }
        if let Some(binary) = program.expression_table.binary_node(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            )
            && validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
            && let Some(reference) =
                validation::expression_result_type_reference(program, machine, state, expression)
            && integer_carrier(program, reference).is_some()
            && matches!(
                program.arithmetic_domain_for_type_reference(reference),
                ArithmeticDomain::Exact | ArithmeticDomain::Trapping
            )
        {
            return Some(ScopedArithmeticValue::IntegerOperation {
                operator: binary.operator,
                left: Box::new(self.capture_at_depth(
                    program,
                    machine,
                    state,
                    binary.left,
                    depth + 1,
                )?),
                right: Box::new(self.capture_at_depth(
                    program,
                    machine,
                    state,
                    binary.right,
                    depth + 1,
                )?),
            });
        }
        if let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression)
            && let Some(value) =
                self.capture_join(program, machine, state, expression, dispatch, depth)
        {
            return Some(value);
        }
        let occurrence = self.occurrence();
        admitted(occurrence, expression, &self.current, &self.fields, 0)
            .then(|| scoped(occurrence, expression, &self.current, &self.fields))
            .flatten()
            .map(|term| {
                use crate::checks::contracts::prover::call_guarantees::{
                    arithmetic, callable::Callable,
                };
                arithmetic::completed_scalar_value(
                    occurrence.program,
                    &Callable::Machine {
                        machine: occurrence.machine,
                        state: occurrence.state,
                    },
                    term,
                )
            })
    }

    /*
        Exact value of a runtime index at this occurrence. Its declaration
        symbol remains storage identity; the returned roster value captures
        assignments and unknown write effects at the selected program point.
    */
    pub(crate) fn current_symbol(&self, symbol: SymbolHandle) -> Option<ScopedArithmeticValue> {
        self.current
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
            .clone()
    }

    /*
        Value of a completed operand at this frame, retaining admitted
        operation evidence as well as captured stores and field values.
        Formation and ordered operand effects remain caller obligations.
    */
    pub(crate) fn current_value(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticValue> {
        self.capture(self.program, self.machine, self.state, expression)
    }

    pub(crate) fn initial_symbol(&self, symbol: SymbolHandle) -> Option<ScopedArithmeticValue> {
        self.initial
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
            .clone()
    }
    /*
        Read an authored expression under the captured arrival roster. Later
        scalar replacement cannot reinterpret this input as current storage.
    */
    pub(crate) fn initial(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticExpression> {
        let occurrence = self.occurrence();
        admitted(occurrence, expression, &self.initial, &[], 0)
            .then(|| scoped(occurrence, expression, &self.initial, &[]))
            .flatten()
    }
    pub(crate) fn current(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticExpression> {
        let occurrence = self.occurrence();
        admitted(occurrence, expression, &self.current, &self.fields, 0)
            .then(|| scoped(occurrence, expression, &self.current, &self.fields))
            .flatten()
    }
    fn capture_alternatives(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Vec<ScopedArithmeticValue> {
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression)
        else {
            return Vec::new();
        };
        let arms = program.expression_table.match_arms(dispatch.arms);
        if arms.is_empty() || arms.len() > 8 {
            return Vec::new();
        }
        let mut values = Vec::with_capacity(arms.len());
        for arm in arms {
            let Some(value) = self.capture(program, machine, state, arm.value) else {
                return Vec::new();
            };
            values.push(value);
        }
        values
    }

    /*
        Facts already captured in their establishing rosters. Consumers must
        preserve those rosters rather than reread the source predicates.
    */
    pub(crate) fn captured_hypotheses(&self) -> &[ScopedArithmeticHypothesis] {
        &self.hypotheses
    }

    /*
        Bind a declaration recipe to current snapshots without treating its
        unevaluated calls as completed runtime occurrences. The formation owner
        must still discharge every operation before using the returned roster.
    */
    pub(crate) fn integer_expression(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticExpression> {
        scoped_operations(
            self.occurrence(),
            expression,
            &self.current,
            &self.fields,
            true,
        )
    }

    pub(crate) fn integer_expression_formation(
        &self,
        machine: &Machine,
        expression: &ScopedArithmeticExpression,
    ) -> Option<(
        validation::IntegerApplicationFormation,
        ScopedArithmeticValue,
    )> {
        validation::scoped_integer_expression_formation(
            self.program,
            machine,
            &self.hypotheses,
            expression,
        )
    }

    pub(crate) fn declared_integer_application_formation(
        &self,
        machine: &Machine,
        expression: &ScopedArithmeticExpression,
        input_type: typed_trees::types::TypeReferenceHandle,
        result: PrimitiveType,
    ) -> Option<(
        validation::IntegerApplicationFormation,
        ScopedArithmeticValue,
    )> {
        validation::scoped_declared_integer_application_formation(
            self.program,
            machine,
            &self.hypotheses,
            expression,
            input_type,
            result,
        )
    }

    pub(crate) fn proves_values_relation(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        operator: BinaryOperator,
        left: &ScopedArithmeticValue,
        right: &ScopedArithmeticValue,
    ) -> bool {
        validation::scoped_arithmetic_values_relation(
            program,
            machine,
            &self.hypotheses,
            operator,
            left,
            right,
        ) == StrictArithmeticImplicationJudgment::Proven
    }

    pub(crate) fn proves(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        goal: &ScopedArithmeticExpression,
    ) -> bool {
        if validation::scoped_arithmetic_implication(program, machine, &self.hypotheses, goal)
            == StrictArithmeticImplicationJudgment::Proven
        {
            return true;
        }
        for current in &self.current {
            let Some(expected) = &current.value else {
                continue;
            };
            if current.alternatives.is_empty() {
                continue;
            }
            if substitute_symbol_in_expression(
                goal,
                current.symbol,
                expected,
                &current.alternatives[0],
            )
            .is_none()
            {
                continue;
            }
            if current.alternatives.iter().all(|alternative| {
                substitute_symbol_in_expression(goal, current.symbol, expected, alternative)
                    .is_some_and(|specialized| {
                        validation::scoped_arithmetic_implication(
                            program,
                            machine,
                            &self.hypotheses,
                            &specialized,
                        ) == StrictArithmeticImplicationJudgment::Proven
                    })
            }) {
                return true;
            }
        }
        false
    }
}

fn substitute_symbol_in_expression(
    expression: &ScopedArithmeticExpression,
    symbol: SymbolHandle,
    expected: &ScopedArithmeticValue,
    replacement: &ScopedArithmeticValue,
) -> Option<ScopedArithmeticExpression> {
    let mut updated = expression.clone();
    let mut changed = false;
    for binding in &mut updated.bindings {
        if binding.binder == ScopedArithmeticBinder::Symbol(symbol) && &binding.value == expected {
            binding.value = replacement.clone();
            changed = true;
        } else if let Some(next) =
            substitute_symbol_in_value(&binding.value, symbol, expected, replacement)
        {
            binding.value = next;
            changed = true;
        }
    }
    changed.then_some(updated)
}

fn substitute_symbol_in_value(
    value: &ScopedArithmeticValue,
    symbol: SymbolHandle,
    expected: &ScopedArithmeticValue,
    replacement: &ScopedArithmeticValue,
) -> Option<ScopedArithmeticValue> {
    match value {
        ScopedArithmeticValue::Term(term) => {
            substitute_symbol_in_expression(term, symbol, expected, replacement)
                .map(ScopedArithmeticValue::Term)
        }
        ScopedArithmeticValue::AdmittedIntegerRemainder(term) => {
            substitute_symbol_in_expression(term, symbol, expected, replacement)
                .map(ScopedArithmeticValue::AdmittedIntegerRemainder)
        }
        ScopedArithmeticValue::BoundedInteger {
            value: inner,
            interval,
        } => substitute_symbol_in_value(inner, symbol, expected, replacement).map(|next| {
            ScopedArithmeticValue::BoundedInteger {
                value: Box::new(next),
                interval: interval.clone(),
            }
        }),
        ScopedArithmeticValue::IntegerOperation {
            operator,
            left,
            right,
        } => {
            let next_left = substitute_symbol_in_value(left, symbol, expected, replacement);
            let next_right = substitute_symbol_in_value(right, symbol, expected, replacement);
            if next_left.is_none() && next_right.is_none() {
                return None;
            }
            Some(ScopedArithmeticValue::IntegerOperation {
                operator: *operator,
                left: Box::new(next_left.unwrap_or_else(|| (**left).clone())),
                right: Box::new(next_right.unwrap_or_else(|| (**right).clone())),
            })
        }
        _ => None,
    }
}

fn integer_carrier(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let primitive = program.primitive_type_reference(reference)?;
    if !primitive.is_fixed_integer() {
        return None;
    }
    Some(primitive)
}
/*
    A collection type's length when no store or reseat can change it during
    the activation: a literal fixed array's own length, or one unsigned unknown.
    Entry metadata includes inline fixed arrays as well as borrowed descriptors.
    Only the descriptor owner below may create a live length atom; a declared
    extent supplies its exact constant without treating owned storage as a loan.
*/
fn entry_collection_length(
    program: &dyn validation::CheckingInput,
    reference: TypeReferenceHandle,
    identity: impl FnOnce() -> String,
) -> Option<ScopedArithmeticValue> {
    if let Some(extent) = validation::fixed_array_type_length(program, reference) {
        return Some(ScopedArithmeticValue::Integer(
            numerics::bignum::BigInt::from_u64(u64::try_from(extent).ok()?),
        ));
    }
    fixed_length(program, reference, identity)
}

/*
    A descriptor length is read as a `u64` count, so a private length atom lies
    in the `u64` interval whenever it is transported into a scoped expression.
    Without that interval an exact `items.len - 1` or `items.len + 1` in a
    declaration (such as a rank endpoint) could never prove it fits its carrier,
    even under a `requires items.len > 0` hypothesis. A known length is already
    an exact integer and needs no interval.
*/
fn carrier_bounded_length(length: ScopedArithmeticValue) -> ScopedArithmeticValue {
    let ScopedArithmeticValue::Atom { .. } = length else {
        return length;
    };
    ScopedArithmeticValue::BoundedInteger {
        value: Box::new(length),
        interval: validation::BigIntInterval {
            low: Some(numerics::bignum::BigInt::zero()),
            high: Some(numerics::bignum::BigInt::from_u64(u64::MAX)),
        },
    }
}

fn fixed_length(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    identity: impl FnOnce() -> String,
) -> Option<ScopedArithmeticValue> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let shared = *access == language_core::ReferenceAccess::Shared;
    let mut referee = *referee;
    let mut qualified = false;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(referee)
    {
        qualified = true;
        referee = *base_type;
    }
    match program.type_reference_table.type_reference(referee) {
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(length),
            ..
        } => Some(ScopedArithmeticValue::Integer(
            numerics::bignum::BigInt::from_u64(u64::try_from(*length).ok()?),
        )),
        TypeReferenceNode::Slice { .. } if qualified && !shared => None,
        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => {
            Some(ScopedArithmeticValue::Atom {
                identity: identity(),
                unsigned: true,
            })
        }
        _ => None,
    }
}

/*
    Exact length of `collection[start..end].len` when both bounds are closed
    integer literals. A fixed array's own `.len` stays on the declared extent;
    this is the same `end - start` a plain window comparison already uses.
*/
fn literal_window_length(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
    let member = program.expression_table.member_node(expression)?;
    if member.case_variant.is_some() || member.member_symbol.is_valid() {
        return None;
    }
    let mut receiver = member.receiver;
    while let Some(borrow) = program.expression_table.borrow_expression(receiver) {
        receiver = borrow.target;
    }
    let indexed = program.expression_table.indexed_node(receiver)?;
    if !program.expression_table.indexed_on_range(indexed) {
        return None;
    }
    if validation::collection_operations::collection_measure_member(
        program,
        member,
        validation::collection_operations::MeasureReceiver::Window,
    ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
    {
        return None;
    }
    let range = program.expression_table.range_node(indexed.index)?;
    let start = if range.start.is_valid() {
        closed_integer_literal(program, range.start)?
    } else {
        numerics::bignum::BigInt::zero()
    };
    if !range.end.is_valid() {
        return None;
    }
    let mut end = closed_integer_literal(program, range.end)?;
    if range.end_inclusive {
        end = end.add(&numerics::bignum::BigInt::from_u64(1));
    }
    if start > end {
        return None;
    }
    Some(ScopedArithmeticValue::Integer(end.sub(&start)))
}

fn closed_integer_literal(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<numerics::bignum::BigInt> {
    expressions(program)
        .integer_node(expression)?
        .value_bignum()
}

/* Static declaration fields retain their exact storage root and complete path. */
fn field_storage_path(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Vec<SymbolHandle>)> {
    let place = validation::canonical_place_from_expression(program, expression)?;
    let owner = place.root.as_symbol()?;
    Some((owner, declaration_field_path(&place.segments)?))
}

/*
    Descriptor observations may name fixed array cells as well as fields.
    Runtime selectors still need their own captured-value association and cannot
    become a persistent declaration coordinate through this static adapter.
*/
fn descriptor_storage_path(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    let mut receiver = expression;
    loop {
        match program.expression_table.expression(receiver) {
            ExpressionNode::Name(_) => break,
            ExpressionNode::Member(member) => receiver = member.receiver,
            ExpressionNode::Indexed(indexed) => receiver = indexed.collection,
            _ => return None,
        }
    }
    let place = validation::canonical_place_from_expression(program, expression)?;
    let owner = place.root.as_symbol()?;
    static_descriptor_path(&place.segments).then_some((owner, place.segments))
}

fn static_descriptor_path(path: &[facts::PlaceSegment]) -> bool {
    path.iter().all(|segment| match segment {
        facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
        facts::PlaceSegment::FixedIndex { .. } => true,
        _ => false,
    })
}

fn declaration_field_path(segments: &[facts::PlaceSegment]) -> Option<Vec<SymbolHandle>> {
    segments
        .iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Field { symbol } if symbol.is_valid() => Some(*symbol),
            _ => None,
        })
        .collect()
}

/* The fixed length a `collection.len` occurrence reads, if it reads one. */
fn length_occurrence<'lengths>(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    lengths: &'lengths [Length],
) -> Option<&'lengths Length> {
    let member = program.expression_table.member_node(expression)?;
    if !validation::value_has_builtin_coordinates(program, machine, state, member.receiver) {
        return None;
    }
    if let Some(length) = lengths.iter().find(|length| {
        if length.value.is_none() {
            return false;
        }
        match &length.subject {
            LengthSubject::Field { owner, path } => {
                descriptor_storage_path(program, member.receiver).is_some_and(
                    |(actual_owner, actual_path)| actual_owner == *owner && actual_path == *path,
                )
            }
            LengthSubject::ResultField { owner, path } => {
                validation::reserved_result_place(program, member.receiver)
                    .is_some_and(|place| place.machine_symbol == *owner && place.segments == *path)
            }
            _ => false,
        }
    }) {
        return (validation::collection_operations::collection_measure_member(
            program,
            member,
            validation::collection_operations::MeasureReceiver::Declared(length.type_reference),
        ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length))
        .then_some(length);
    }
    let subject =
        if let Some((owner, _)) = validation::reserved_result_owner(program, member.receiver) {
            LengthSubject::Result(owner)
        } else {
            let name = program.expression_table.name_path_node(member.receiver)?;
            if !program.expression_table.name_path_is_single_member(name) {
                return None;
            }
            LengthSubject::Storage(name.symbol)
        };
    let length = lengths
        .iter()
        .find(|length| length.subject == subject && length.value.is_some())?;
    (validation::collection_operations::collection_measure_member(
        program,
        member,
        validation::collection_operations::MeasureReceiver::Declared(length.type_reference),
    ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length))
    .then_some(length)
}

/*
    Frozen integer fields retain canonical root and complete declaration path.
    Only stable shared parameter loans participate; owned or exclusive storage
    needs an effect-sensitive value transfer before it can retain entry atoms.
*/
fn frozen_field(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(TypeReferenceHandle, String)> {
    let parameters = program.state_parameters(state);
    let (index, path, selected_type) =
        structural_fields::structural_parameter_place(program, parameters, expression)?;
    if path.is_empty()
        || path.iter().any(|segment| {
            !matches!(
                segment,
                checked_trees::CheckedStructuralPredicatePathSegment::Field(_)
            )
        })
    {
        return None;
    }
    let parameter = parameters.get(usize::try_from(index).ok()?)?;
    let shell = validation::reference_shell(program, parameter.type_reference)?;
    if shell.access != language_core::ReferenceAccess::Shared
        || parameter.is_mutable
        || !validation::state_reference_parameter_binding_is_stable(
            program,
            machine,
            state,
            parameter.symbol,
        )
    {
        return None;
    }
    let (_, segments, _, frozen) =
        structural_fields::resolve_structural_parameter_path(program, parameters, index, &path)?;
    if !frozen {
        return None;
    }
    Some((
        selected_type,
        format!(
            "state:{:?}:parameter:{:?}:field:{segments:?}",
            state.symbol, parameter.symbol
        ),
    ))
}

fn field_collection_length(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
    let (reference, identity) = frozen_field(program, machine, state, expression)?;
    fixed_length(program, reference, || format!("{identity}:length"))
}

fn field_occurrence(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
    if let Some(member) = program.expression_table.member_node(expression)
        && let Some((reference, _)) = frozen_field(program, machine, state, member.receiver)
        && validation::collection_operations::collection_measure_member(
            program,
            member,
            validation::collection_operations::MeasureReceiver::Declared(reference),
        ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length)
    {
        return field_collection_length(program, machine, state, member.receiver);
    }
    let (reference, identity) = frozen_field(program, machine, state, expression)?;
    let primitive = integer_carrier(program, reference)?;
    let value = ScopedArithmeticValue::Atom {
        identity,
        unsigned: unsigned(primitive),
    };
    Some(
        match validation::immutable_integer_expression_bounds(program, machine, state, expression) {
            Some((minimum, maximum)) => ScopedArithmeticValue::BoundedInteger {
                value: Box::new(value),
                interval: BigIntInterval {
                    low: Some(numerics::bignum::BigInt::from_i64(minimum)),
                    high: Some(numerics::bignum::BigInt::from_i64(maximum)),
                },
            },
            None => value,
        },
    )
}

/*
    Identity belongs to this executed store on the current state activation.
    Distinct stores never share an unknown merely because they have the same
    destination or initializer; subsequent effects still retire the binding.
*/
fn observed_scalar(
    state: &State,
    symbol: SymbolHandle,
    statement: usize,
    primitive: PrimitiveType,
) -> ScopedArithmeticValue {
    ScopedArithmeticValue::Atom {
        identity: format!(
            "state:{:?}:store:{statement}:scalar:{symbol:?}",
            state.symbol
        ),
        unsigned: unsigned(primitive),
    }
}

/*
    Operand of an exact or trapping fixed-integer value cast. This is formation
    only: wrapping and saturating casts are different values, and representability
    is decided separately from the live hypotheses.
*/
fn integer_cast_operand(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) else {
        return None;
    };
    if cast.form.is_recast() || !cast.semantic_domain.is_empty() {
        return None;
    }
    let target_type = program.primitive_type_reference(cast.target_type)?;
    if target_type.is_fixed_integer()
        && matches!(
            program.expression_table.expression(cast.value),
            ExpressionNode::Integer(_)
        )
    {
        return Some(cast.value);
    }
    let operand_type =
        validation::expression_result_type_reference(program, machine, state, cast.value)
            .and_then(|reference| program.primitive_type_reference(reference))?;
    (operand_type.is_fixed_integer()
        && target_type.is_fixed_integer()
        && (matches!(
            cast.domain,
            ArithmeticDomain::Exact | ArithmeticDomain::Trapping
        ) || operand_type == target_type))
        .then_some(cast.value)
}

/*
    The occurrence a scoped walk reads: the program point plus the roster it
    admits against. A callee's entry and a matched alternative's state are
    occurrences too, so the pair moves while the rest stays the caller's.
*/
#[derive(Clone, Copy)]
struct ScopedOccurrence<'roster, 'tree> {
    program: &'tree dyn ::validation::CheckingInput,
    machine: &'tree Machine,
    state: &'tree State,
    lengths: &'roster [Length],
    hypotheses: &'roster [ScopedArithmeticHypothesis],
    result_fields: Option<&'roster CapturedResultFields>,
}

impl<'program> CurrentScalarValues<'program> {
    // A composed index keeps its meaning only when both its typed operations
    // and every storage read remain valid at this observation point.
    pub(crate) fn selector_has_immutable_denotation(&self, expression: ExpressionHandle) -> bool {
        validation::value_reads_have_immutable_denotation(
            self.program,
            self.machine,
            self.state,
            self.statement_index,
            expression,
        ) && validation::expression_traversal::inert_value_tree_with_operations(
            self.program,
            expression,
            |operation| admitted(self.occurrence(), operation, &self.current, &self.fields, 0),
        ) && scalar_fields::coordinates::selector_value(
            self.occurrence(),
            expression,
            &self.current,
            &self.fields,
        )
        .is_some()
    }

    fn occurrence(&self) -> ScopedOccurrence<'_, 'program> {
        ScopedOccurrence {
            program: self.program,
            machine: self.machine,
            state: self.state,
            lengths: &self.lengths,
            hypotheses: &self.hypotheses,
            result_fields: None,
        }
    }

    /*
        This occurrence read through an explicitly supplied program point: a
        callee's entry or a selected alternative's state, against the roster
        that scope re-derived. The caller's hypotheses and statement still apply.
    */
    fn occurrence_at<'a>(
        &'a self,
        program: &'a dyn ::validation::CheckingInput,
        machine: &'a Machine,
        state: &'a State,
        lengths: &'a [Length],
    ) -> ScopedOccurrence<'a, 'a> {
        ScopedOccurrence {
            program,
            machine,
            state,
            lengths,
            hypotheses: &self.hypotheses,
            result_fields: None,
        }
    }
}

/*
    Operand of an exact or trapping integer cast that denotes that operand.
    A total carrier inclusion always does. A partial conversion does only when
    the same hypotheses prove the operand fits in the target.
*/
fn preserving_integer_cast_operand(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
) -> Option<ExpressionHandle> {
    let ScopedOccurrence {
        program,
        machine,
        state,
        hypotheses,
        ..
    } = occurrence;
    let operand = integer_cast_operand(program, machine, state, expression)?;
    let (target_minimum, target_maximum) =
        integer_carrier_span(program, machine, state, expression)?;
    if let Some(value) = expressions(program)
        .integer_node(operand)
        .and_then(|literal| literal.value_bignum())
    {
        return (value >= target_minimum && value <= target_maximum).then_some(operand);
    }
    let (source_minimum, source_maximum) = integer_carrier_span(program, machine, state, operand)?;
    if source_minimum >= target_minimum && source_maximum <= target_maximum {
        return Some(operand);
    }
    let operand_value = ScopedArithmeticValue::Term(scoped(occurrence, operand, bindings, fields)?);
    let minimum = ScopedArithmeticValue::Integer(target_minimum);
    let maximum = ScopedArithmeticValue::Integer(target_maximum);
    let fits = validation::scoped_arithmetic_values_relation(
        program,
        machine,
        hypotheses,
        BinaryOperator::GreaterOrEqual,
        &operand_value,
        &minimum,
    ) == StrictArithmeticImplicationJudgment::Proven
        && validation::scoped_arithmetic_values_relation(
            program,
            machine,
            hypotheses,
            BinaryOperator::LessOrEqual,
            &operand_value,
            &maximum,
        ) == StrictArithmeticImplicationJudgment::Proven;
    fits.then_some(operand)
}

fn integer_carrier_span(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    let minimum = expression_integer_carrier_minimum(program, machine, state, expression)?;
    let maximum = expression_integer_carrier_maximum(program, machine, state, expression)?;
    Some((minimum, numerics::bignum::BigInt::from_u64(maximum)))
}

/*
    Whether the occurrence's scoped engine admits `expression`: its syntax is
    integer vocabulary, its leaves resolve against the occurrence's own roster.
*/
fn admitted(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
    depth: usize,
) -> bool {
    let ScopedOccurrence {
        program,
        machine,
        state,
        lengths,
        ..
    } = occurrence;
    if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        ExpressionNode::Cast(_) => integer_cast_operand(program, machine, state, expression)
            .is_some_and(|operand| admitted(occurrence, operand, bindings, fields, depth + 1)),
        ExpressionNode::Call(_) => validation::integer_embedding_argument(program, expression)
            .is_some_and(|(_, argument)| {
                admitted(occurrence, argument, bindings, fields, depth + 1)
            }),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            result_scalar_field_occurrence(occurrence, expression).is_some()
                || length_occurrence(program, machine, state, expression, lengths).is_some()
                || scalar_field_occurrence(occurrence, expression, bindings, fields).is_some()
                || field_occurrence(program, machine, state, expression).is_some()
                || literal_window_length(program, expression).is_some()
        }
        ExpressionNode::Name(name) => {
            (name.symbol == name.head_symbol
                && bindings
                    .iter()
                    .any(|binding| binding.symbol == name.symbol && binding.value.is_some()))
                || validation::reserved_result_owner(program, expression)
                    .is_some_and(|(owner, _)| owner == machine.symbol)
        }
        ExpressionNode::Binary(binary) => {
            matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or
            ) && validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) && admitted(occurrence, binary.left, bindings, fields, depth + 1)
                && admitted(occurrence, binary.right, bindings, fields, depth + 1)
        }
        _ => false,
    }
}

/* The scoped projection of `expression` at this occurrence. */
fn scoped(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
) -> Option<ScopedArithmeticExpression> {
    scoped_operations(occurrence, expression, bindings, fields, false)
}

/*
    Bind the same exact read roster for proof consumers and declaration
    formation. Formation preserves operations for the child-first checker;
    completed proof consumers retain their prior preserving-cast projection.
*/
fn scoped_operations(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
    form_operations: bool,
) -> Option<ScopedArithmeticExpression> {
    let ScopedOccurrence {
        program,
        machine,
        state,
        lengths,
        ..
    } = occurrence;
    let mut symbols = Vec::new();
    let mut projections = Vec::new();
    let mut pending = vec![(expression, 0)];
    let mut budget = 4096usize;
    while let Some((expression, depth)) = pending.pop() {
        budget = budget.checked_sub(1)?;
        if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(_)
                if program.expression_table.names_reserved_result(expression) => {}
            ExpressionNode::Name(name) if name.symbol == name.head_symbol => {
                crate::shared::collections::push_unique(&mut symbols, name.symbol);
            }
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
            ExpressionNode::Atomic(atomic) if form_operations => {
                pending.push((atomic.value, depth + 1));
            }
            ExpressionNode::Cast(cast) if form_operations => {
                pending.push((cast.value, depth + 1));
            }
            ExpressionNode::Call(call) if form_operations => {
                if call.receiver.is_valid() {
                    pending.push((call.receiver, depth + 1));
                }
                pending.extend(
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .map(|argument| (*argument, depth + 1)),
                );
            }
            ExpressionNode::Cast(_) => {
                let operand =
                    preserving_integer_cast_operand(occurrence, expression, bindings, fields)?;
                let value = scoped(occurrence, operand, bindings, fields)?;
                projections.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(expression),
                    value: ScopedArithmeticValue::Term(value),
                });
            }
            ExpressionNode::Call(_) => {
                let (_, argument) = validation::integer_embedding_argument(program, expression)?;
                pending.push((argument, depth + 1));
            }
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                let value = result_scalar_field_occurrence(occurrence, expression)
                    .or_else(|| {
                        length_occurrence(program, machine, state, expression, lengths)
                            .and_then(|length| length.value.clone())
                            .map(carrier_bounded_length)
                    })
                    .or_else(|| scalar_field_occurrence(occurrence, expression, bindings, fields))
                    .or_else(|| field_occurrence(program, machine, state, expression))
                    .or_else(|| literal_window_length(program, expression))?;
                projections.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(expression),
                    value,
                });
            }
            ExpressionNode::Binary(binary) => {
                pending.push((binary.left, depth + 1));
                pending.push((binary.right, depth + 1));
            }
            _ => return None,
        }
    }
    let mut selected = Vec::new();
    for symbol in symbols {
        let value = bindings
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
            .as_ref()?;
        charge(value, &mut budget, 0)?;
        selected.push(ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Symbol(symbol),
            value: value.clone(),
        });
    }
    selected.extend(projections);
    Some(ScopedArithmeticExpression {
        owner: state.symbol,
        expression,
        bindings: selected,
    })
}
fn charge(value: &ScopedArithmeticValue, budget: &mut usize, depth: usize) -> Option<()> {
    *budget = budget.checked_sub(1)?;
    if depth >= 64 {
        return None;
    }
    match value {
        ScopedArithmeticValue::Term(term)
        | ScopedArithmeticValue::CollectionLength(term)
        | ScopedArithmeticValue::AdmittedIntegerRemainder(term) => {
            for binding in &term.bindings {
                charge(&binding.value, budget, depth + 1)?;
            }
        }
        ScopedArithmeticValue::BoundedInteger { value, .. } => {
            charge(value, budget, depth + 1)?;
        }
        ScopedArithmeticValue::IntegerOperation { left, right, .. } => {
            charge(left, budget, depth + 1)?;
            charge(right, budget, depth + 1)?;
        }
        ScopedArithmeticValue::TypeReference(_)
        | ScopedArithmeticValue::OwnedSnapshot(_)
        | ScopedArithmeticValue::CapturedInteger(_)
        | ScopedArithmeticValue::Atom { .. }
        | ScopedArithmeticValue::Integer(_) => {}
    }
    Some(())
}

/*
    The current descriptor observation's proven upper bound, after every
    preceding store and call has retired or replaced its captured facts.
*/
pub(crate) fn byte_length_bound(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<u64> {
    let machine = program.machine_by_own_or_state_symbol(state.symbol)?;
    CurrentScalarValues::at(
        program,
        machine,
        state,
        statement_index,
        TransitionTargetHandle::invalid(),
        None,
    )?
    .byte_length_bound(expression)
}
