use facts::{FactContextHandle, FactPlan, ProgramPoint};
use validation::CanonicalPlace;

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
