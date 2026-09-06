//! Closed call operands transported through an exact normal-return guarantee.
//! No source arithmetic, callee body, or current argument storage is replayed.

use super::*;
use checked_trees::{CheckedScalarComputationKind, ContractProofFactKind, ContractProofFactOwner};
use facts::FactPlace;
use typed_trees::expression::{BinaryOperator, ExpressionNode};

impl ExitScalars<'_, '_> {
    pub(super) fn value_at_place(
        &self,
        subject: &crate::flow::CanonicalPlace,
    ) -> Option<ScalarValue> {
        if let Some(value) = scalar_value_at_place(
            self.program,
            &self.facts.semantic,
            self.contexts
                .iter()
                .map(|context| self.facts.semantic.contexts.get(*context)),
            subject,
        ) {
            return Some(value);
        }
        let mut source = None;
        for context in self.contexts {
            for fact in self
                .facts
                .semantic
                .context_view(self.facts.semantic.contexts.get(*context))
                .facts()
            {
                if !matches!(
                    fact.payload,
                    FactPayload::AssignedValue { .. } | FactPayload::AssignedScalarValue { .. }
                ) {
                    continue;
                }
                let FactPlace::Place(place) = fact.place else {
                    continue;
                };
                let Some(candidate) = crate::flow::canonical_place_from_semantic_place(
                    self.program,
                    &self.facts.semantic,
                    self.facts.semantic.places.get(place),
                ) else {
                    continue;
                };
                if candidate.root != subject.root || candidate.segments != subject.segments {
                    continue;
                }
                let FactPayload::AssignedValue { value } = fact.payload else {
                    return None;
                };
                if !matches!(
                    self.program.expression_table.expression(value),
                    ExpressionNode::Call(_)
                ) || source.is_some_and(|prior| prior != value)
                {
                    return None;
                }
                source = Some(value);
            }
        }
        self.closed_call_value(source?)
    }

    pub(super) fn closed_call_value(&self, expression: ExpressionHandle) -> Option<ScalarValue> {
        let ExpressionNode::Call(authored) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if authored.receiver.is_valid()
            || !authored.machine_arguments.is_empty()
            || !authored.evidence_arguments.is_empty()
            || authored.static_requirement_dispatch.is_some()
            || authored.quotient_operation.is_some()
            || authored.private_layout_operation.is_some()
        {
            return None;
        }
        let mut occurrences = self
            .facts
            .flow
            .control
            .states
            .iter()
            .filter(|(_, state)| state.machine_symbol == self.machine.symbol)
            .flat_map(|(_, state)| {
                self.facts
                    .flow
                    .control
                    .calls
                    .span_or_empty(state.calls)
                    .iter()
                    .map(move |call| (state, call))
            })
            .filter(|(_, call)| {
                call.authored_expression == expression
                    && call.target_symbol == authored.target_symbol
            });
        let (state, call) = occurrences.next()?;
        if occurrences.next().is_some() {
            return None;
        }
        let Some(crate::CallSite::Expression {
            expression: actual,
            call: selected,
        }) = crate::find_call_site(
            self.program,
            self.machine.symbol,
            state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        )
        else {
            return None;
        };
        if actual != expression || selected.target_symbol != call.target_symbol {
            return None;
        }
        let callee = self.program.machines().iter().find(|machine| {
            self.program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == call.target_symbol)
        })?;
        let entry = self.program.machine_states(callee).first()?;
        let parameters = self.program.state_parameters(entry);
        if parameters
            .iter()
            .any(|parameter| parameter.is_self || parameter.is_const)
        {
            return None;
        }
        let arguments = self
            .program
            .expression_table
            .expression_handles(authored.arguments);
        if arguments.len() != parameters.len() {
            return None;
        }
        let result_type = self.program.primitive_type_reference(entry.return_type)?;
        if !matches!(
            result_type,
            typed_trees::types::PrimitiveType::I8
                | typed_trees::types::PrimitiveType::I16
                | typed_trees::types::PrimitiveType::I32
                | typed_trees::types::PrimitiveType::I64
                | typed_trees::types::PrimitiveType::U8
                | typed_trees::types::PrimitiveType::U16
                | typed_trees::types::PrimitiveType::U32
                | typed_trees::types::PrimitiveType::U64
        ) {
            return None;
        }
        let mut retained = None;
        for reference in self
            .facts
            .proof
            .contract_fact_refs
            .span_or_empty(call.ensures)
        {
            let guarantee = self.facts.proof.contract_facts.get(reference.fact);
            if guarantee.kind != ContractProofFactKind::Ensures
                || !matches!(guarantee.owner,
                    ContractProofFactOwner::Machine { machine_symbol } if machine_symbol == callee.symbol)
                    && !matches!(guarantee.owner,
                        ContractProofFactOwner::MachineState { machine_symbol, state_symbol }
                            if machine_symbol == callee.symbol && state_symbol == entry.symbol)
            {
                continue;
            }
            let typed_trees::domain::ProofFact::Expression(guarantee_expression) =
                self.program.proof_facts.get(guarantee.fact)
            else {
                continue;
            };
            let ExpressionNode::Binary(binary) = self
                .program
                .expression_table
                .expression(*guarantee_expression)
            else {
                continue;
            };
            if binary.operator != BinaryOperator::Equal {
                continue;
            }
            let formal = if is_result_reference(self.program, callee, binary.left) {
                binary.right
            } else if is_result_reference(self.program, callee, binary.right) {
                binary.left
            } else {
                continue;
            };
            let ExpressionNode::Name(path) = self.program.expression_table.expression(formal)
            else {
                continue;
            };
            let Some((position, parameter)) =
                parameters.iter().enumerate().find(|(_, parameter)| {
                    parameter.symbol.is_valid()
                        && path.symbol == parameter.symbol
                        && path.head_symbol == parameter.symbol
                })
            else {
                continue;
            };
            if parameter.is_mutable
                || self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    != Some(result_type)
                || !result_type.accepts_integer_literal()
                || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    self.program,
                    callee.symbol,
                    *guarantee_expression,
                    language_core::OperatorSpelling::Equal,
                    &[Some(entry.return_type), Some(parameter.type_reference)],
                )
            {
                continue;
            }
            let argument = arguments[position];
            let value = self.closed_argument_value(
                state.state_symbol,
                call,
                expression,
                argument,
                position,
                result_type,
            )?;
            if retained.as_ref().is_some_and(|prior| prior != &value) {
                return None;
            }
            retained = Some(value);
        }
        retained
    }

    fn closed_argument_value(
        &self,
        state: symbols::SymbolHandle,
        call: &checked_trees::FlowCallFact,
        call_expression: ExpressionHandle,
        argument: ExpressionHandle,
        position: usize,
        expected_type: typed_trees::types::PrimitiveType,
    ) -> Option<ScalarValue> {
        let statement = u32::try_from(call.statement_index).ok()?;
        let position = u32::try_from(position).ok()?;
        let caller_state = crate::find_state_in_machine(self.program, self.machine.symbol, state)?;
        let statements = self
            .program
            .statement_table
            .statements(caller_state.statement_nodes);
        let direct_binding_ordinal = match statements.get(call.statement_index)? {
            typed_trees::statement::StatementNode::LocalData(local)
                if !local.is_mutable && local.initial_value == call_expression =>
            {
                u32::try_from(statements[..call.statement_index].iter().filter(|statement| {
                    matches!(statement, typed_trees::statement::StatementNode::LocalData(local)
                        if !local.is_mutable && self.program.primitive_type_reference(local.type_reference).is_some())
                }).count()).ok()
            }
            _ => None,
        };
        let plans = &self.facts.values.scalar_expressions;
        let mut values = Vec::new();
        for (_, binding) in plans.source_bindings.iter() {
            let matches_role = match binding.role {
                CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal,
                    argument_ordinal,
                } => {
                    direct_binding_ordinal == Some(binding_ordinal) && argument_ordinal == position
                }
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal,
                    argument_ordinal,
                } => {
                    usize::try_from(call_ordinal).ok() == Some(call.call_ordinal)
                        && argument_ordinal == position
                }
                _ => false,
            };
            if binding.state == state
                && binding.statement_ordinal == statement
                && binding.expression == argument
                && matches_role
            {
                values.push(plans.expression_at(
                    binding.state,
                    binding.statement_ordinal,
                    binding.role,
                )?);
            }
        }
        let computations = &self.facts.values.scalar_computations;
        for (_, node) in computations.nodes.iter() {
            if node.authored_root != call_expression {
                continue;
            }
            let CheckedScalarComputationKind::Call {
                source_call,
                arguments,
                ..
            } = &node.kind
            else {
                continue;
            };
            if self.facts.flow.control.calls.get(*source_call) != call {
                continue;
            }
            let operand = computations
                .operands
                .span_or_empty(*arguments)
                .get(usize::try_from(position).ok()?)?;
            let CheckedScalarComputationKind::Value(value) = &computations.nodes.get(*operand).kind
            else {
                return None;
            };
            values.push(value);
        }
        let mut retained = None;
        for expression in values {
            if crate::values::scalar_expression_type(expression) != Some(expected_type) {
                return None;
            }
            // Closed selected operations only: do not reread mutable storage,
            // replay a call, or invent a snapshot for an earlier argument.
            let value = evaluate_checked_scalar(expression, &mut |_| None)?;
            if retained.as_ref().is_some_and(|prior| prior != &value) {
                return None;
            }
            retained = Some(value);
        }
        retained
    }
}
