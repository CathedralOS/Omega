use facts::{FactContextHandle, FactPlan, ProgramPoint};
use validation::CanonicalPlace;

/// One immutable predicate-query scope. Many field/domain facts share the
/// same enclosing case path; selected-tag evidence depends on that path, not
/// on its trailing scalar field. Reuse only within these exact contexts and
/// this exact program point, never across a write or a different invocation.
pub(super) struct CasePremiseAvailability<'scope> {
    program: &'scope dyn validation::CheckingInput,
    semantic: &'scope FactPlan,
    contexts: &'scope [FactContextHandle],
    point: ProgramPoint,
    observations: Vec<(CanonicalPlace, bool)>,
}

impl<'scope> CasePremiseAvailability<'scope> {
    pub(super) fn new(
        program: &'scope dyn validation::CheckingInput,
        semantic: &'scope FactPlan,
        contexts: &'scope [FactContextHandle],
        point: ProgramPoint,
    ) -> Self {
        Self {
            program,
            semantic,
            contexts,
            point,
            observations: Vec::new(),
        }
    }

    pub(super) fn proves(&mut self, subject: &CanonicalPlace) -> bool {
        let Some(last_case) = subject
            .segments
            .iter()
            .rposition(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
        else {
            return true;
        };
        let case_path = &subject.segments[..=last_case];
        if let Some((_, available)) = self.observations.iter().find(|(observed, _)| {
            observed.root == subject.root && observed.segments.as_slice() == case_path
        }) {
            return *available;
        }
        let selected = CanonicalPlace {
            root: subject.root,
            segments: case_path.to_vec(),
        };
        let available = case_premise_is_available(
            self.program,
            self.semantic,
            self.contexts,
            self.point,
            &selected,
        );
        self.observations.push((selected, available));
        available
    }
}

/// A field membership below a case is a conditional invariant, not an
/// unconditional premise. Expansion needs selected-tag evidence at the
/// consumer's exact scope. A fact's publication point is not that scope:
/// transported contracts can still contain another telescope's expressions.
pub(crate) fn case_premise_is_available(
    program: &dyn validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    point: ProgramPoint,
    subject: &CanonicalPlace,
) -> bool {
    if !subject
        .segments
        .iter()
        .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
    {
        return true;
    }
    let (machine_symbol, state_symbol, statement_index) = match point {
        ProgramPoint::State {
            machine_symbol,
            state_symbol,
        } => (machine_symbol, state_symbol, 0),
        ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index,
        }
        | ProgramPoint::Call {
            machine_symbol,
            state_symbol,
            statement_index,
            ..
        }
        | ProgramPoint::CallRequires {
            machine_symbol,
            state_symbol,
            statement_index,
            ..
        }
        | ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            ..
        }
        | ProgramPoint::TransitionArm {
            machine_symbol,
            state_symbol,
            statement_index,
            ..
        }
        | ProgramPoint::Exit {
            machine_symbol,
            state_symbol,
            statement_index,
            ..
        } => (machine_symbol, state_symbol, statement_index),
        ProgramPoint::Machine { machine_symbol } => {
            let Some(machine) = program.machine_by_symbol(machine_symbol) else {
                return false;
            };
            let Some(state) = program.machine_states(machine).first() else {
                return false;
            };
            (machine_symbol, state.symbol, 0)
        }
        ProgramPoint::Global | ProgramPoint::Definition { .. } => return false,
    };
    crate::flow::place_cases_are_selected(
        program,
        semantic,
        contexts,
        machine_symbol,
        state_symbol,
        statement_index,
        subject,
    )
}
