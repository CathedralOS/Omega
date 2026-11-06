//! Normal exits consume the completed value, not a replay of the enclosing loop.
//! The flow owner supplies exact invocation-to-header origins. A recurrent
//! header starts from its own contract; only those origins can bind an original
//! owned input in the outgoing guarantee. The guarantee is never a premise.

use super::structural_judgment::StructuralJudgment;
use super::structural_prefix::{PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor};
use language_semantics::content::ContentPlaceVersion;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::{machine::Machine, state::State};

/// The authored operation supplying one normal exit. Named transitions retain
/// their own occurrence; they do not have a source expression result.
#[derive(Clone, Copy)]
pub enum NormalExitCompletion {
    Expression(ExpressionHandle),
    Transition(typed_trees::statement::TransitionTargetHandle),
}

/// The declaration whose predicate an implementation must prove, with the exact
/// parameter correspondence retained by its checked signature. These are only
/// subjects to substitute; the completed return and all-arrival origins must
/// independently establish the predicate.
pub struct NormalExitContract<'a> {
    pub expression: ExpressionHandle,
    pub callable: SymbolHandle,
    pub parameters: &'a [StateParameter],
    pub supplying_parameters: &'a [StateParameter],
    pub type_bindings: &'a [(SymbolHandle, TypeReferenceHandle)],
}

/// Judge one value exit with independently checked, all-arrival formal origins.
/// Mutable/reference parameters need their storage-version contract owner;
/// they are not admitted as immutable value aliases by this query.
pub fn normal_exit_proves(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    returned: NormalExitCompletion,
    contract: NormalExitContract<'_>,
    origins: &[(SymbolHandle, SymbolHandle)],
    ranks: &checked_trees::CheckedRankArrivalPlan,
) -> bool {
    if contract.parameters.len() != contract.supplying_parameters.len() {
        return false;
    }
    let mut query = ExitPredicate {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: match returned {
                NormalExitCompletion::Expression(expression) => {
                    if !expression.is_valid() {
                        return false;
                    }
                    super::structural_prefix::expression_is_transition_target(
                        program,
                        state,
                        statement_index,
                        expression,
                    )
                }
                NormalExitCompletion::Transition(target) => {
                    match program.statement_table.statement_at(state, statement_index) {
                        Some(typed_trees::statement::StatementNode::Transition(transition))
                            if transition.exit
                                == typed_trees::statement::TransitionExit::Ordinary
                                && target == transition.target
                                && program.statement_table.transition_target_is_valid(target) =>
                        {
                            true
                        }
                        _ => return false,
                    }
                }
            },
        },
        returned,
        contract,
        origins,
        ranks,
        seen: false,
        proven: true,
    };
    let observed =
        super::structural_prefix::observe_runtime_state_prefix(program, machine, state, &mut query)
            .is_some();
    if super::struct_trace() && (!observed || !query.seen) {
        eprintln!(
            "STRUCT exit-observation-refused machine={} state={} statement={statement_index} prefix_complete={observed} reached_exit={}",
            machine.name, state.name, query.seen
        );
    }
    observed && query.seen && query.proven
}

struct ExitPredicate<'a> {
    site: StructuralPrefixSite,
    returned: NormalExitCompletion,
    contract: NormalExitContract<'a>,
    origins: &'a [(SymbolHandle, SymbolHandle)],
    ranks: &'a checked_trees::CheckedRankArrivalPlan,
    seen: bool,
    proven: bool,
}

impl StructuralPrefixVisitor for ExitPredicate<'_> {
    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn entered(&mut self) {
        self.seen = true;
    }

    fn excluded_transition(
        &mut self,
        state: SymbolHandle,
        statement: usize,
        target: typed_trees::statement::TransitionTargetHandle,
    ) {
        if state == self.site.state
            && statement == self.site.statement_index
            && matches!(self.returned, NormalExitCompletion::Transition(own) if own == target)
        {
            // This arrival has no normal result. Other arrivals still have to
            // establish the whole predicate; exclusion cannot repair a refusal.
            self.seen = true;
        }
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.proven = false;
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let machine = observation.1;
        let state = observation.2;
        let result = self.proves_observation(observation);
        if let Err(reason) = result
            && super::struct_trace()
        {
            eprintln!(
                "STRUCT exit-observation-refused machine={} state={} statement={} reason={reason}",
                machine.name, state.name, self.site.statement_index
            );
        }
        self.proven &= result.unwrap_or(false);
    }
}

impl ExitPredicate<'_> {
    fn proves_observation(
        &self,
        observation: PrefixObservation<'_, '_>,
    ) -> Result<bool, &'static str> {
        let (program, machine, state, mut judge, environment) = observation;
        judge.retain_runtime_induction(self.ranks);
        let entry = program
            .machine_states(machine)
            .first()
            .ok_or("the machine has no entry state")?;
        let mut parameters = Vec::new();
        for (formal, parameter) in self
            .contract
            .parameters
            .iter()
            .zip(self.contract.supplying_parameters)
        {
            // A signature supplies its exact entry parameter, even when its
            // declaration uses a different formal symbol or an adapter prefix.
            // An arbitrary state-local value cannot fill this correspondence.
            if program
                .state_parameter_by_symbol(entry, parameter.symbol)
                .is_none_or(|entry| entry != parameter)
                || !formal.type_reference.is_valid()
                || !parameter.type_reference.is_valid()
                || formal.is_const != parameter.is_const
                || formal.is_mutable != parameter.is_mutable
                || formal.relevance != parameter.relevance
                || (formal.type_reference != parameter.type_reference
                    && program.type_identity(TypeIdentityRequest::ordinary(formal.type_reference))
                        != program
                            .type_identity(TypeIdentityRequest::ordinary(parameter.type_reference)))
            {
                return Err("the contract input does not match the checked entry parameter");
            }
            if !crate::value_custody::source_liveness::expression_uses_symbol(
                program,
                self.contract.expression,
                formal.symbol,
            ) {
                continue;
            }
            let (_, current) = self
                .origins
                .iter()
                .find(|(original, _)| *original == parameter.symbol)
                .ok_or("the input has no exact origin on every arrival")?;
            let current = program
                .state_parameter_by_symbol(state, *current)
                .ok_or("the input origin has no parameter in this state")?;
            if parameter.is_mutable
                || current.is_mutable
                || parameter.is_const
                || current.is_const
                || crate::reference_shell(program, parameter.type_reference).is_some()
                || current.type_reference != parameter.type_reference
            {
                return Err(
                    "the input needs a storage-version proof or has a different receiving type",
                );
            }
            // Capture before evaluating the return operand. Moving the value
            // into the result does not destroy the observation used by a proof.
            let value = judge
                .captured_parameter_projection(current.symbol, &[], ContentPlaceVersion::Current)
                .ok_or("the input has no completed value observation")?;
            parameters.push((formal.symbol, value));
        }
        let scope = (machine, state, state.return_type);
        let paths = match self.returned {
            NormalExitCompletion::Expression(expression) => {
                judge.normal_return_paths(expression, environment, scope)
            }
            NormalExitCompletion::Transition(target) => judge.normal_transition_return_paths(
                target,
                self.site.statement_index,
                environment,
                scope,
            ),
        }
        .ok_or("the return operation has no complete normal observations")?;
        if paths.is_empty() {
            return Err("the return operation has no observed normal path");
        }
        for path in paths {
            // Substitute exact formal identities and the completed result. A
            // source-style reread could replace its captured field equations
            // with a weaker denotational application before judging the exit.
            let predicate = path
                .judge
                .bound_contract_term(
                    self.contract.expression,
                    self.contract.callable,
                    self.contract.parameters,
                    self.contract.callable,
                    &parameters,
                    self.contract.type_bindings,
                    Some(&path.value),
                    0,
                )
                .ok_or("the completed result cannot substitute the contract's subjects")?;
            let trace_predicate = super::struct_trace().then(|| predicate.clone());
            let result = path.judge.judge_observed_predicate(predicate, 0);
            if !matches!(result, StructuralJudgment::Proven) {
                if let Some(predicate) = trace_predicate {
                    eprintln!(
                        "STRUCT exit-predicate-refused machine={} state={} statement={} returned={:?} predicate={predicate:?} judgment={result:?}",
                        machine.name, state.name, self.site.statement_index, path.value,
                    );
                }
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use typed_trees::statement::TransitionTargetHandle;

    fn query(ranks: &checked_trees::CheckedRankArrivalPlan) -> ExitPredicate<'_> {
        ExitPredicate {
            site: StructuralPrefixSite {
                state: SymbolHandle::from_parts(1, 1),
                statement_index: 2,
                in_transition_target: true,
            },
            returned: NormalExitCompletion::Transition(TransitionTargetHandle::from_parts(3, 1)),
            contract: NormalExitContract {
                expression: ExpressionHandle::invalid(),
                callable: SymbolHandle::invalid(),
                parameters: &[],
                supplying_parameters: &[],
                type_bindings: &[],
            },
            origins: &[],
            ranks,
            seen: false,
            proven: true,
        }
    }

    #[test]
    fn a_refuted_guard_closes_its_exact_exit_without_a_result() {
        let ranks = checked_trees::CheckedRankArrivalPlan::default();
        let mut query = query(&ranks);
        query.excluded_transition(
            SymbolHandle::from_parts(1, 1),
            2,
            TransitionTargetHandle::from_parts(3, 1),
        );
        assert!(query.seen && query.proven);
    }

    #[test]
    fn another_arm_or_arrival_does_not_close_the_exit() {
        let ranks = checked_trees::CheckedRankArrivalPlan::default();
        let mut query = query(&ranks);
        for (state, statement, target) in [
            (
                SymbolHandle::from_parts(4, 1),
                2,
                TransitionTargetHandle::from_parts(3, 1),
            ),
            (
                SymbolHandle::from_parts(1, 1),
                4,
                TransitionTargetHandle::from_parts(3, 1),
            ),
            (
                SymbolHandle::from_parts(1, 1),
                2,
                TransitionTargetHandle::from_parts(3, 2),
            ),
        ] {
            query.excluded_transition(state, statement, target);
            assert!(!query.seen);
        }
    }

    #[test]
    fn an_excluded_arrival_does_not_erase_a_refused_arrival() {
        let ranks = checked_trees::CheckedRankArrivalPlan::default();
        let mut query = query(&ranks);
        query.proven = false;
        query.excluded_transition(
            SymbolHandle::from_parts(1, 1),
            2,
            TransitionTargetHandle::from_parts(3, 1),
        );
        assert!(query.seen);
        assert!(!query.proven);
    }
}
