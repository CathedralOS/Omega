//! Call contracts bind both machine value coordinates and ordinary operands.
//! Each actual is a current snapshot after the caller's ordered prefix; unknown
//! writes withdraw addressable captures, while invocation indices remain exact. Completed statement and expression invocations share
//! guarantee import, while results and recursive self-contracts retain their
//! separate availability obligations.

use super::{
    Binding, CurrentScalarValues, Length, LengthSubject, ScalarField, admitted, fixed_length,
    integer_carrier, length_occurrence, literal_window_length, scalar_field_occurrence, scoped,
    unsigned,
};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, StaticMachineArgument};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceNode;
use validation::{
    CallSite, ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue,
};

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
        self.bind_parameters(
            callee,
            state,
            expression,
            parameters,
            arguments,
            &machine_values,
        )
    }

    fn machine_value_bindings(
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
            return None;
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

    fn static_argument_value(
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

    pub(super) fn assume_call_ensures(
        &mut self,
        expression: ExpressionHandle,
        result: &ScopedArithmeticValue,
    ) {
        let Some(call) = self.program.expression_table.call_node(expression) else {
            return;
        };
        if validation::integer_embedding_argument(self.program, expression).is_some() {
            return;
        }
        let Some((callee, state)) = validation::find_state(self.program, call.target_symbol)
            .or_else(|| validation::find_machine_head(self.program, call.target_symbol))
        else {
            return;
        };
        let Some(parameters) = validation::call_target_parameters(self.program, call.target_symbol)
        else {
            return;
        };
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        self.assume_invocation_ensures(
            callee,
            state,
            parameters,
            arguments,
            CallSite::Expression { expression, call },
            Some(result),
        );
    }

    pub(super) fn assume_statement_ensures(&mut self, call: &typed_trees::statement::TableCall) {
        let Some((callee, state)) = validation::find_state(self.program, call.target_symbol)
            .or_else(|| validation::find_machine_head(self.program, call.target_symbol))
        else {
            return;
        };
        let Some(parameters) = validation::call_target_parameters(self.program, call.target_symbol)
        else {
            return;
        };
        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments);
        self.assume_invocation_ensures(
            callee,
            state,
            parameters,
            arguments,
            CallSite::Statement(call),
            None,
        );
    }

    fn assume_invocation_ensures(
        &mut self,
        callee: &Machine,
        state: &State,
        parameters: &[typed_trees::signature::StateParameter],
        arguments: &[ExpressionHandle],
        site: CallSite<'_>,
        result: Option<&ScopedArithmeticValue>,
    ) {
        if parameters.len() != arguments.len() || state.symbol == self.state.symbol {
            return;
        }
        let is_entry = self
            .program
            .machine_states(callee)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol);
        // Guarantee availability is bounded by source contract formation and
        // common call-cycle admission: state arrivals carry requires only;
        // proof-only recursive occurrences require strict edge descent; runtime
        // components admit only classified tail occurrences. In particular an
        // ordinary statement invocation cannot borrow a runtime component's
        // eventual termination as an induction hypothesis. The occurrence
        // inventory in runtime_ranking is checked before prefix frame shortcuts.
        let contracts = self.program.state_contracts(state).iter().chain(
            self.program
                .machine_contracts(callee)
                .iter()
                .filter(|_| is_entry && callee.symbol != self.machine.symbol),
        );
        let mut guarantees = Vec::new();
        for contract in contracts {
            if contract.kind != typed_trees::signature::SignatureContractKind::Ensures
                || contract.binding.is_some()
            {
                continue;
            }
            for fact in self.program.proof_facts.span_or_empty(contract.facts) {
                if let Some(guarantee) = fact.as_expression() {
                    Self::collect_ensures_conjuncts(
                        self.program,
                        callee,
                        state,
                        *guarantee,
                        &mut guarantees,
                        0,
                    );
                }
            }
        }
        for guarantee in guarantees {
            if result.is_none() {
                let mut contains_result = false;
                validation::expression_traversal::visit_post_order(
                    self.program,
                    guarantee,
                    &mut Vec::new(),
                    |expression, _| {
                        contains_result |= self
                            .program
                            .expression_table
                            .names_reserved_result(expression);
                    },
                );
                if contains_result {
                    continue;
                }
            }
            if let Some(mut proposition) =
                self.bind_call_parameters(callee, state, guarantee, parameters, arguments, site)
            {
                if let Some(result) = result {
                    proposition.bindings.push(ScopedArithmeticBinding {
                        binder: ScopedArithmeticBinder::Result,
                        value: result.clone(),
                    });
                }
                if !self
                    .hypotheses
                    .iter()
                    .any(|existing| existing.holds && existing.proposition == proposition)
                {
                    self.hypotheses.push(ScopedArithmeticHypothesis {
                        proposition,
                        holds: true,
                    });
                }
            }
        }
    }

    /// Simultaneous parameter substitution with independently retained actual
    /// snapshots. The caller owns selection and exact formal/actual roster
    /// identity; this owner checks arithmetic meaning before exposing a term.
    fn bind_parameters(
        &self,
        callee: &Machine,
        state: &State,
        expression: ExpressionHandle,
        parameters: &[typed_trees::signature::StateParameter],
        arguments: &[ExpressionHandle],
        machine_values: &[(SymbolHandle, ScopedArithmeticValue)],
    ) -> Option<ScopedArithmeticExpression> {
        if parameters.len() != arguments.len() {
            return None;
        }
        let mut formals = Vec::with_capacity(parameters.len());
        let mut lengths = Vec::new();
        let mut fields = Vec::new();
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let Some(primitive) = integer_carrier(self.program, parameter.type_reference) else {
                if let Some(value) = self.collection_length(*argument, 0)
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
        for binding in &mut goal.bindings {
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
                // The argument's captured identity remains mandatory. Bounds
                // from its exact caller observation add arrival knowledge
                // (including acyclic predecessors), never a destination
                // requirement or permission to revive an invalidated value.
                binding.value = if let Some((minimum, maximum)) =
                    validation::occurrence_integer_expression_bounds(
                        self.program,
                        self.machine.symbol,
                        self.state.symbol,
                        self.statement_index,
                        arguments[position],
                    ) {
                    ScopedArithmeticValue::BoundedInteger {
                        value: Box::new(captured),
                        minimum: Some(numerics::bignum::BigInt::from_i64(minimum)),
                        maximum: Some(numerics::bignum::BigInt::from_i64(maximum)),
                    }
                } else {
                    captured
                };
            }
        }
        Some(goal)
    }

    fn collect_ensures_conjuncts(
        program: &dyn ::validation::CheckingInput,
        callee: &Machine,
        callee_state: &State,
        expression: ExpressionHandle,
        out: &mut Vec<ExpressionHandle>,
        depth: usize,
    ) {
        if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
            return;
        }
        if let Some(binary) = program.expression_table.binary_node(expression)
            && binary.operator == BinaryOperator::And
            && validation::has_builtin_decomposed_guard_meaning(
                program,
                callee,
                Some(callee_state),
                expression,
            )
        {
            Self::collect_ensures_conjuncts(
                program,
                callee,
                callee_state,
                binary.left,
                out,
                depth + 1,
            );
            Self::collect_ensures_conjuncts(
                program,
                callee,
                callee_state,
                binary.right,
                out,
                depth + 1,
            );
            return;
        }
        out.push(expression);
    }
}
