//! Call contracts bind both machine value coordinates and ordinary operands.
//! Each actual is a current snapshot after the caller's ordered prefix; unknown
//! writes withdraw addressable captures, while invocation indices remain exact.
//! Requires use entry substitution; completed invocations transport guarantees
//! through the shared public-contract owner and its post-state formal roster.

use super::{
    Binding, CurrentScalarValues, Length, LengthSubject, ScalarField, admitted, fixed_length,
    integer_carrier, length_occurrence, literal_window_length, scalar_field_occurrence, scoped,
    unsigned,
};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, StaticMachineArgument};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceNode;
use validation::{
    CallSite, ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue,
};

struct CapturedParameterGoal {
    goal: ScopedArithmeticExpression,
    /// Exact actuals aligned with goal bindings; non-parameter rows are absent.
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
        )?;
        Some(self.enrich_parameter_goal(captured))
    }

    /// Prove a simultaneous substitution from its independently captured actuals.
    /// Current facts may suffice before discovering extra occurrence bounds;
    /// every unsuccessful attempt retains the original enriched proof.
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
        ) else {
            return false;
        };
        if self.proves(self.program, self.machine, &captured.goal) {
            return true;
        }
        let goal = self.enrich_parameter_goal(captured);
        self.proves(self.program, self.machine, &goal)
    }

    fn enrich_parameter_goal(&self, captured: CapturedParameterGoal) -> ScopedArithmeticExpression {
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
                // Arrival knowledge belongs to this actual's exact observation,
                // never a destination requirement or a later storage version.
                if let Some((minimum, maximum)) = bounds.get(argument) {
                    binding.value = ScopedArithmeticValue::BoundedInteger {
                        value: Box::new(binding.value),
                        minimum: Some(numerics::bignum::BigInt::from_i64(minimum)),
                        maximum: Some(numerics::bignum::BigInt::from_i64(maximum)),
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
                self.program
                    .expression_table
                    .static_machine_application_arguments(application),
            )
        } else {
            (callee, explicit)
        };
        let parameters = self.program.machine_type_parameters(source);
        let internal = site.is_internal_state_transfer(self.program, self.machine.symbol);
        if !internal && parameters.len() != arguments.len() {
            // A type-only telescope carries no integer actual. An empty static
            // roster then leaves the arithmetic requires on the value
            // parameters; it is not an unknown static binding.
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
                | typed_trees::data::TypeParameterKind::Const { type_reference } => type_reference,
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
        use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
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
                _ => {}
            }
        }
        if symbol.is_valid() {
            if let Some(value) = self.current_symbol(symbol) {
                return Some(value);
            }
            let declaration = self.program.const_declaration_by_symbol(symbol)?;
            let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
            return match CanonicalConstValue::new("", encoding.to_owned(), "").decode_encoding()? {
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
        // The caller has applied the ordered operand-write barrier. It erases
        // overlapping knowledge and installs no post-call values; an affected
        // source read cannot be recaptured as the later value.
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
        let machine_values = if let Some((callee, _)) = validation::find_state(self.program, target)
            .or_else(|| validation::find_machine_head(self.program, target))
        {
            let Some(values) = self.machine_value_bindings(callee, site) else {
                return;
            };
            values
        } else {
            // Signature/operator contracts without a concrete machine retain
            // their ordinary operand roster. An unsupported static telescope
            // is unavailable rather than guessed from a realization.
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
        for hypothesis in validation::scoped_completed_call_guarantees(
            self.program,
            self.machine,
            validation::ScopedCallCompletion {
                state: self.state,
                statement_index,
                site,
                actuals,
                machine_values: &machine_values,
                result,
            },
        ) {
            if !self.hypotheses.contains(&hypothesis) {
                self.hypotheses.push(hypothesis);
            }
        }
    }

    /// Simultaneous parameter substitution with independently retained actual
    /// snapshots. The caller owns selection and exact formal/actual roster
    /// identity; this owner checks arithmetic meaning before exposing a term.
    fn capture_parameter_goal(
        &self,
        callee: &Machine,
        state: &State,
        expression: ExpressionHandle,
        parameters: &[typed_trees::signature::StateParameter],
        arguments: &[ExpressionHandle],
        machine_values: &[(SymbolHandle, ScopedArithmeticValue)],
    ) -> Option<CapturedParameterGoal> {
        if parameters.len() != arguments.len() {
            return None;
        }
        let mut formals = Vec::with_capacity(parameters.len());
        let mut lengths = Vec::new();
        let mut fields = Vec::new();
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let Some(primitive) = integer_carrier(self.program, parameter.type_reference) else {
                let measured = self.collection_length(*argument, 0);
                if let Some(value) = measured
                    && fixed_length(self.program, parameter.type_reference, String::new).is_some()
                    && (validation::state_reference_parameter_binding_is_stable(
                        self.program,
                        callee,
                        state,
                        parameter.symbol,
                    ) || matches!(
                        value,
                        ScopedArithmeticValue::Integer(_)
                            | ScopedArithmeticValue::BoundedInteger { .. }
                            | ScopedArithmeticValue::IntegerOperation { .. }
                    ))
                {
                    // A closed length is an entry observation. A mutable slice
                    // parameter may later reseat its descriptor; the requires
                    // clause still sees the length the caller passed.
                    lengths.push(Length {
                        subject: LengthSubject::Storage(parameter.symbol),
                        name: parameter.name.to_string(),
                        stable: true,
                        type_reference: parameter.type_reference,
                        value: Some(value),
                    });
                }
                for field in self.captured_scalar_fields(*argument, 0) {
                    fields.push(ScalarField::for_owner(
                        self.program,
                        parameter.symbol,
                        field,
                    ));
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
                continue;
            };
            formals.push(Binding {
                symbol: parameter.symbol,
                name: parameter.name.to_string(),
                value: Some(ScopedArithmeticValue::Atom {
                    identity: format!("callee:{:?}", parameter.symbol),
                    unsigned: unsigned(primitive),
                }),
                alternatives: Vec::new(),
                immutable_coordinate: false,
            });
        }
        formals.extend(machine_values.iter().map(|(symbol, value)| Binding {
            symbol: *symbol,
            name: self.program.symbols.name(*symbol).to_owned(),
            value: Some(value.clone()),
            alternatives: Vec::new(),
            immutable_coordinate: true,
        }));
        if !admitted(
            self.program,
            callee,
            state,
            self.statement_index,
            expression,
            &formals,
            &lengths,
            &fields,
            0,
        ) {
            return None;
        }
        let mut goal = scoped(
            callee,
            state,
            self.statement_index,
            expression,
            &formals,
            &lengths,
            &fields,
            &self.hypotheses,
            self.program,
        )?;
        let mut captured_actuals = Vec::new();
        for (binding_index, binding) in goal.bindings.iter_mut().enumerate() {
            // A callee field needs an actual-place substitution, including on
            // recursion. Its entry atom is never the caller's current field
            // merely because the declaration and formal symbol are equal.
            if let ScopedArithmeticBinder::Occurrence(occurrence) = binding.binder
                && length_occurrence(self.program, occurrence, &lengths).is_none()
                && scalar_field_occurrence(self.program, callee, state, occurrence, &fields)
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
                // `scoped` selects the formal occurrences this goal actually
                // observes. An unrelated operand need not have arithmetic
                // meaning: it still evaluates and contributes its writes in
                // the caller's prefix before we bind any observed actual.
                let position = parameters
                    .iter()
                    .position(|parameter| parameter.symbol == symbol)?;
                let captured =
                    self.capture(self.program, self.machine, self.state, arguments[position])?;
                captured_actuals.push((binding_index, arguments[position], captured));
            }
        }
        // Every actual identity is established before optional bounds or proof.
        // Machine coordinates already retain their exact invocation binding.
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
