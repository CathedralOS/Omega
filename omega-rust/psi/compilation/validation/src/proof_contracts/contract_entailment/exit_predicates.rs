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
    returned: ExpressionHandle,
    contract: NormalExitContract<'_>,
    origins: &[(SymbolHandle, SymbolHandle)],
    ranks: &checked_trees::CheckedRankArrivalPlan,
) -> bool {
    if !returned.is_valid() || contract.parameters.len() != contract.supplying_parameters.len() {
        return false;
    }
    let mut query = ExitPredicate {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                returned,
            ),
        },
        returned,
        contract,
        origins,
        ranks,
        seen: false,
        proven: true,
    };
    super::structural_prefix::observe_runtime_state_prefix(program, machine, state, &mut query)
        .is_some()
        && query.seen
        && query.proven
}

struct ExitPredicate<'a> {
    site: StructuralPrefixSite,
    returned: ExpressionHandle,
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

    fn unresolved(&mut self, _machine: &Machine) {
        self.proven = false;
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        self.proven &= self.proves_observation(observation).unwrap_or(false);
    }
}

impl ExitPredicate<'_> {
    fn proves_observation(&self, observation: PrefixObservation<'_, '_>) -> Option<bool> {
        let (program, machine, state, mut judge, environment) = observation;
        judge.retain_runtime_induction(self.ranks);
        let entry = program.machine_states(machine).first()?;
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
                return None;
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
                .find(|(original, _)| *original == parameter.symbol)?;
            let current = program.state_parameter_by_symbol(state, *current)?;
            if parameter.is_mutable
                || current.is_mutable
                || parameter.is_const
                || current.is_const
                || crate::reference_shell(program, parameter.type_reference).is_some()
                || current.type_reference != parameter.type_reference
            {
                return None;
            }
            // Capture before evaluating the return operand. Moving the value
            // into the result does not destroy the observation used by a proof.
            let value = judge.captured_parameter_projection(
                current.symbol,
                &[],
                ContentPlaceVersion::Current,
            )?;
            parameters.push((formal.symbol, value));
        }
        let paths = judge.normal_return_paths(
            self.returned,
            environment,
            (machine, state, state.return_type),
        )?;
        if paths.is_empty() {
            return None;
        }
        for path in paths {
            // Substitute exact formal identities and the completed result. A
            // source-style reread could replace its captured field equations
            // with a weaker denotational application before judging the exit.
            let predicate = path.judge.bound_contract_term(
                self.contract.expression,
                self.contract.callable,
                self.contract.parameters,
                self.contract.callable,
                &parameters,
                self.contract.type_bindings,
                Some(&path.value),
                0,
            )?;
            let trace_predicate = super::struct_trace().then(|| predicate.clone());
            let result = path.judge.judge_observed_predicate(predicate, 0);
            if !matches!(result, StructuralJudgment::Proven) {
                if let Some(predicate) = trace_predicate {
                    eprintln!(
                        "STRUCT exit-predicate-refused machine={} state={} statement={} returned={:?} predicate={predicate:?} judgment={result:?}",
                        machine.name, state.name, self.site.statement_index, path.value,
                    );
                }
                return Some(false);
            }
        }
        Some(true)
    }
}
