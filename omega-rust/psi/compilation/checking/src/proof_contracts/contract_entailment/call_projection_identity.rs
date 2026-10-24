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
        position,
        path,
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
    position: usize,
    path: Vec<PlaceSegment>,
    seen: bool,
    value: Option<String>,
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
                let Some(owner) = super::specification_calls::selected_call_contract(program, call)
                else {
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
            let Some(value) = entry.arguments.get(leading + self.position) else {
                self.valid = false;
                continue;
            };
            let value = entry.judge.completed_value_identity(value.clone());
            let Some(value) = super::observed_projections::project_observed_value(
                program,
                self.parameters[self.position].type_reference,
                value,
                &self.path,
            ) else {
                self.valid = false;
                continue;
            };
            let value = entry.judge.completed_value_identity(value);
            let Some(key) = value.observation_identity_key() else {
                self.valid = false;
                continue;
            };
            let key = format!("state:{:?}:completed-contract:{key}", self.state);
            if let Some(previous) = &self.value {
                if previous != &key {
                    self.valid = false;
                }
            } else {
                self.value = Some(key);
            }
        }
    }
}
