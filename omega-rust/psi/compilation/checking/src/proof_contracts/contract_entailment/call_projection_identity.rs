//! Snapshot identities for owned formal subjects in erased contracts.
//!
//! A selected outcome describes operands captured before the call, never the
//! storage spelling at a later arm. Reuse the normal-value path owner for
//! ordering, copies, branch agreement and mutation epochs; this adapter only
//! authenticates declared subjects for checking-only value comparison.

use super::citations::CitationFacts;
use super::structural_case_arms::structural_arm_judge;
use super::structural_judgment::{StructuralJudge, StructuralTerm};
use super::structural_prefix::{
    StructuralPrefixSite, StructuralPrefixVisitor, observe_runtime_state_prefix,
};
use crate::machine_calls::calls::CallSite;
use facts::PlaceSegment;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;

/// Identify a resolved formal or its projection. Reference formals retain their live
/// occurrence rules and cannot acquire owned snapshot semantics here.
pub fn owned_contract_subject(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(usize, Vec<PlaceSegment>)> {
    let mut root = expression;
    let mut fields = Vec::new();
    while let ExpressionNode::Member(member) = program.expression_table.expression(root) {
        if !member.member_symbol.is_valid() || member.case_variant.is_some() {
            return None;
        }
        fields.push(PlaceSegment::Field {
            symbol: member.member_symbol,
        });
        root = member.receiver;
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(root) else {
        return None;
    };
    let position = parameters
        .iter()
        .position(|parameter| parameter.symbol == name.symbol)?;
    if crate::reference_shell(program, parameters[position].type_reference).is_some() {
        return None;
    }
    fields.reverse();
    Some((position, fields))
}

/// All normal entry paths must agree on the exact completed operand projection.
/// Unknown completion supplies no reusable identity. State symbols scope entry
/// variables; completed call/event identities already retain their occurrences.
pub fn captured_call_subject_identity(
    program: &dyn crate::CheckingInput,
    caller_state: SymbolHandle,
    statement_index: usize,
    call: CallSite<'_>,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<String> {
    let (position, path) = owned_contract_subject(program, parameters, expression)?;
    match capture_projection(
        program,
        caller_state,
        statement_index,
        call,
        parameters,
        ProjectionRequest::Identity { position, path },
    )? {
        ProjectionValue::Identity(identity) => Some(identity),
        _ => None,
    }
}

/// Interpret an admitted result-index recipe under the call's completed
/// operands, including earlier sibling writes. The normal-value owner supplies
/// the evaluation cut; this adapter never replays a supplying expression.
pub fn captured_call_index_value(
    program: &dyn crate::CheckingInput,
    caller_state: SymbolHandle,
    statement_index: usize,
    call: CallSite<'_>,
    parameters: &[StateParameter],
    owner: SymbolHandle,
    argument: typed_trees::types::TypeReferenceHandle,
) -> Option<super::ScopedArithmeticValue> {
    match capture_projection(
        program,
        caller_state,
        statement_index,
        call,
        parameters,
        ProjectionRequest::Value { owner, argument },
    )? {
        ProjectionValue::Scalar(value) => Some(value),
        _ => None,
    }
}

/// The source descriptor and receiving runtime index are compared at one
/// invocation cut. Both terms come from completed operands; no foreign result
/// recipe or later read of the supplying home participates.
pub fn captured_call_argument_index_matches(
    program: &dyn crate::CheckingInput,
    caller_state: SymbolHandle,
    statement_index: usize,
    call: CallSite<'_>,
    parameters: &[StateParameter],
    position: usize,
    source_type: typed_trees::types::TypeReferenceHandle,
    descriptor: SymbolHandle,
    required: typed_trees::types::TypeReferenceHandle,
) -> bool {
    matches!(
        capture_projection(
            program,
            caller_state,
            statement_index,
            call,
            parameters,
            ProjectionRequest::IndexEquality {
                position,
                source_type,
                descriptor,
                required,
                owner: call.target_symbol(program)
            }
        ),
        Some(ProjectionValue::Equality(true))
    )
}

enum ProjectionRequest {
    IndexEquality {
        position: usize,
        source_type: typed_trees::types::TypeReferenceHandle,
        descriptor: SymbolHandle,
        required: typed_trees::types::TypeReferenceHandle,
        owner: SymbolHandle,
    },
    Identity {
        position: usize,
        path: Vec<PlaceSegment>,
    },
    Value {
        owner: SymbolHandle,
        argument: typed_trees::types::TypeReferenceHandle,
    },
}

#[derive(PartialEq)]
enum ProjectionValue {
    Equality(bool),
    Identity(String),
    Scalar(super::ScopedArithmeticValue),
}

fn capture_projection(
    program: &dyn crate::CheckingInput,
    caller_state: SymbolHandle,
    statement_index: usize,
    call: CallSite<'_>,
    parameters: &[StateParameter],
    request: ProjectionRequest,
) -> Option<ProjectionValue> {
    let state = program.state_by_symbol(caller_state)?;
    let machine = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == caller_state)
    })?;
    let in_transition_target = match call {
        CallSite::Expression { expression, .. } => {
            super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                expression,
            )
        }
        CallSite::TransitionNamed {
            path: selected,
            arguments: actuals,
            ..
        } => {
            let typed_trees::statement::StatementNode::Transition(transition) = program
                .statement_table
                .statement_at(state, statement_index)?
            else {
                return None;
            };
            let matches = |target: typed_trees::statement::TransitionTargetHandle| {
                target.is_valid()
                    && matches!(program.statement_table.transition_target(target), typed_trees::statement::TransitionTargetNode::Named { path, arguments, .. } if std::ptr::eq(path, selected) && *arguments == actuals)
            };
            // Continuations belong to their separate path owner. Neither an
            // unknown occurrence nor a shared sibling borrows the primary guard.
            if !matches(transition.target) || matches(transition.continuation) {
                return None;
            }
            true
        }
        CallSite::Statement(_) => false,
    };
    let mut visitor = ProjectionIdentity {
        state: caller_state,
        statement_index,
        in_transition_target,
        call,
        parameters,
        request,
        seen: false,
        value: None,
        valid: true,
    };
    observe_runtime_state_prefix(program, machine, state, &mut visitor)?;
    (visitor.seen && visitor.valid)
        .then_some(visitor.value)
        .flatten()
}

struct ProjectionIdentity<'a> {
    state: SymbolHandle,
    statement_index: usize,
    in_transition_target: bool,
    call: CallSite<'a>,
    parameters: &'a [StateParameter],
    request: ProjectionRequest,
    seen: bool,
    value: Option<ProjectionValue>,
    valid: bool,
}

impl StructuralPrefixVisitor for ProjectionIdentity<'_> {
    fn site(&self) -> StructuralPrefixSite {
        StructuralPrefixSite {
            state: self.state,
            statement_index: self.statement_index,
            in_transition_target: self.in_transition_target,
        }
    }
    fn visit<'program>(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        judge: &StructuralJudge<'program>,
        environment: &[(String, StructuralTerm)],
        hypotheses: &[(String, StructuralTerm)],
        equations: &[(StructuralTerm, StructuralTerm)],
        guarantees: &CitationFacts,
    ) {
        let mut judge = structural_arm_judge(program, judge, hypotheses, equations);
        if judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut judge);
        self.seen = true;
        let Some(state) = program.state_in_machine(machine, self.state) else {
            self.valid = false;
            return;
        };
        let (entries, leading) = match self.call {
            CallSite::Expression { expression, call } => {
                let Some(owner) = super::specification_calls::selected_call_contract(
                    program,
                    machine.symbol,
                    call,
                ) else {
                    self.valid = false;
                    return;
                };
                (
                    judge.normal_operation_entries(
                        self.statement_index,
                        expression,
                        super::structural_judgment::normal_returns::OperationOperandDemand::All,
                        environment,
                        (machine, state, state.return_type),
                    ),
                    super::specification_calls::owner_leading_operands(&owner),
                )
            }
            _ => (
                judge.normal_argument_entries(
                    crate::call_site_argument_expressions(program, &self.call),
                    self.parameters,
                    environment,
                    (machine, state, state.return_type),
                    Some(self.call),
                ),
                0,
            ),
        };
        let Some(entries) = entries else {
            self.valid = false;
            return;
        };
        if entries.is_empty() {
            self.valid = false;
        }
        for entry in entries {
            let Some(arguments) = entry.observed_arguments() else {
                self.valid = false;
                continue;
            };
            let value = match &self.request {
                ProjectionRequest::IndexEquality {
                    position,
                    source_type,
                    descriptor,
                    required,
                    owner,
                } => {
                    let mut bindings = self
                        .parameters
                        .iter()
                        .zip(&arguments)
                        .map(|(parameter, value)| (parameter.symbol, value.clone()))
                        .collect::<Vec<_>>();
                    if let Some(selected) = super::specification_calls::selected_call_site_contract(
                        program,
                        machine.symbol,
                        self.call,
                    ) {
                        let Some(values) = super::specification_calls::call_scalar_substitution(
                            program,
                            machine.symbol,
                            &selected,
                            self.call,
                            |subject| {
                                entry.judge.completed_scalar_subject(
                                    subject, self.call, &arguments, machine, state,
                                )
                            },
                        ) else {
                            self.valid = false;
                            continue;
                        };
                        bindings.extend(
                            values
                                .into_iter()
                                .map(|(parameter, value)| (parameter.symbol, value)),
                        );
                    }
                    let actual = arguments.get(*position).cloned().and_then(|value| {
                        super::observed_projections::project_observed_value(
                            program,
                            *source_type,
                            value,
                            &[PlaceSegment::Field {
                                symbol: *descriptor,
                            }],
                        )
                    });
                    let expected =
                        bound_index(program, *owner, self.parameters, &bindings, *required);
                    actual.zip(expected).map(|(actual, expected)| {
                        // Equality consumes the normal observer's established equations,
                        // just as ordinary contract judgment does. Distinct completed
                        // operands can denote equal indices without sharing identity.
                        let actual = entry
                            .judge
                            .resolve(entry.judge.completed_value_identity(actual));
                        let expected = entry
                            .judge
                            .resolve(entry.judge.completed_value_identity(expected));
                        ProjectionValue::Equality(matches!(
                            entry.judge.judge_equation(actual, expected, 0),
                            super::structural_judgment::StructuralJudgment::Proven
                        ))
                    })
                }
                ProjectionRequest::Identity { position, path } => arguments
                    .get(leading + position)
                    .and_then(|value| {
                        super::observed_projections::project_observed_value(
                            program,
                            self.parameters[*position].type_reference,
                            entry.judge.completed_value_identity(value.clone()),
                            path,
                        )
                    })
                    .map(|value| entry.judge.completed_value_identity(value))
                    .and_then(|value| value.observation_identity_key())
                    .map(|key| {
                        ProjectionValue::Identity(format!(
                            "state:{:?}:completed-contract:{key}",
                            self.state
                        ))
                    }),
                ProjectionRequest::Value { owner, argument } => {
                    if arguments.len() != self.parameters.len() {
                        self.valid = false;
                        continue;
                    }
                    let bindings = self
                        .parameters
                        .iter()
                        .zip(&arguments)
                        .map(|(parameter, value)| (parameter.symbol, value.clone()))
                        .collect::<Vec<_>>();
                    let (value, primitive) =
                        match program.type_reference_table.type_reference(*argument) {
                            typed_trees::types::TypeReferenceNode::ConstExpression(expression) => (
                                super::structural_judgment::selected_contracts::bound_term(
                                    program,
                                    *expression,
                                    *owner,
                                    self.parameters,
                                    *owner,
                                    &bindings,
                                    None,
                                    0,
                                ),
                                crate::parameter_expression_result_type_reference(
                                    program,
                                    *owner,
                                    self.parameters,
                                    *expression,
                                )
                                .and_then(|reference| program.primitive_type_reference(reference)),
                            ),
                            typed_trees::types::TypeReferenceNode::Named { symbol, .. } => (
                                bindings
                                    .iter()
                                    .find(|(formal, _)| formal == symbol)
                                    .map(|(_, value)| value.clone()),
                                self.parameters
                                    .iter()
                                    .find(|parameter| parameter.symbol == *symbol)
                                    .and_then(|parameter| {
                                        program.primitive_type_reference(parameter.type_reference)
                                    }),
                            ),
                            _ => (None, None),
                        };
                    value
                        .map(|value| entry.judge.completed_value_identity(value))
                        .and_then(|value| {
                            let value = match value.semantic_value() {
                                StructuralTerm::Integer(integer) => {
                                    super::ScopedArithmeticValue::Integer(integer.clone())
                                }
                                _ => super::ScopedArithmeticValue::Atom {
                                    identity: format!(
                                        "state:{:?}:completed-index:{}",
                                        self.state,
                                        value.observation_identity_key()?
                                    ),
                                    unsigned: !primitive
                                        .filter(|primitive| primitive.is_fixed_integer())?
                                        .is_signed_integer(),
                                },
                            };
                            Some(ProjectionValue::Scalar(value))
                        })
                }
            };
            let Some(value) = value else {
                self.valid = false;
                continue;
            };
            if let Some(previous) = &self.value {
                if previous != &value {
                    self.valid = false;
                }
            } else {
                self.value = Some(value);
            }
        }
    }
}

fn bound_index(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    bindings: &[(SymbolHandle, StructuralTerm)],
    argument: typed_trees::types::TypeReferenceHandle,
) -> Option<StructuralTerm> {
    if let Some(literal) = program.static_const_literal_from_type_reference(argument) {
        return literal.value_bignum().map(StructuralTerm::Integer);
    }
    match program.type_reference_table.type_reference(argument) {
        typed_trees::types::TypeReferenceNode::ConstExpression(expression) => {
            super::structural_judgment::selected_contracts::bound_term(
                program,
                *expression,
                owner,
                parameters,
                owner,
                bindings,
                None,
                0,
            )
        }
        typed_trees::types::TypeReferenceNode::Named { symbol, .. } => bindings
            .iter()
            .find(|(formal, _)| formal == symbol)
            .map(|(_, value)| value.clone()),
        _ => None,
    }
}
