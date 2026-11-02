//! Normal exits consume the completed value, not a replay of the enclosing loop.
//! The flow owner supplies exact invocation-to-header origins. A recurrent
//! header starts from its own contract; only those origins can bind an original
//! owned input in the outgoing guarantee. The guarantee is never a premise.

use super::structural_judgment::StructuralJudgment;
use super::structural_prefix::{PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor};
use language_semantics::content::ContentPlaceVersion;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::{machine::Machine, state::State};

/// Judge one value exit with independently checked, all-arrival formal origins.
/// Mutable/reference parameters need their storage-version contract owner;
/// they are not admitted as immutable value aliases by this query.
pub fn normal_exit_proves(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    returned: ExpressionHandle,
    expression: ExpressionHandle,
    origins: &[(SymbolHandle, SymbolHandle)],
) -> bool {
    if !returned.is_valid() {
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
        expression,
        origins,
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
    expression: ExpressionHandle,
    origins: &'a [(SymbolHandle, SymbolHandle)],
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
        let (program, machine, state, judge, environment) = observation;
        let entry = program.machine_states(machine).first()?;
        let mut parameters = Vec::new();
        for parameter in program.state_parameters(entry) {
            if !crate::value_custody::source_liveness::expression_uses_symbol(
                program,
                self.expression,
                parameter.symbol,
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
            parameters.push((parameter.name.to_string(), value));
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
            let mut bindings = parameters.clone();
            if !program
                .state_parameters(entry)
                .iter()
                .any(|parameter| parameter.name.as_str() == super::RESULT_BINDER)
            {
                bindings.push((super::RESULT_BINDER.to_owned(), path.value));
            }
            if !matches!(
                path.judge.judge_predicate_in_scope(
                    (machine, entry, entry.return_type),
                    self.expression,
                    &bindings,
                    0,
                ),
                StructuralJudgment::Proven
            ) {
                return Some(false);
            }
        }
        Some(true)
    }
}
