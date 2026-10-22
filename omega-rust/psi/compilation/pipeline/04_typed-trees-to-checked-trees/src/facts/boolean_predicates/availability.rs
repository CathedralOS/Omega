use facts::{FactContextHandle, FactPlan, ProgramPoint};
use std::cell::OnceCell;
use validation::CanonicalPlace;

/// A payload membership is a conditional invariant, not an unconditional
/// Boolean premise. Expansion requires selected tags in the consumer scope;
/// a fact's publication point may still name another contract's telescope.
/// A batch is valid only for these immutable consumer contexts and scope.
/// Templates may be shared across subjects; selected-case verdicts may not.
pub(crate) struct CasePremiseAvailability<'program> {
    program: &'program dyn validation::CheckingInput,
    semantic: &'program FactPlan,
    contexts: &'program [FactContextHandle],
    point: ProgramPoint,
    evidence: OnceCell<Option<crate::flow::CaseEvidence<'program>>>,
}

impl<'program> CasePremiseAvailability<'program> {
    pub(crate) fn new(
        program: &'program dyn validation::CheckingInput,
        semantic: &'program FactPlan,
        contexts: &'program [FactContextHandle],
        point: ProgramPoint,
    ) -> Self {
        Self {
            program,
            semantic,
            contexts,
            point,
            evidence: OnceCell::new(),
        }
    }

    pub(crate) fn allows(&self, subject: &CanonicalPlace) -> bool {
        !subject
            .segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
            || self
                .evidence
                .get_or_init(|| {
                    consumer_scope(self.program, self.point).and_then(
                        |(machine, state, statement)| {
                            crate::flow::CaseEvidence::prepare(
                                self.program,
                                self.semantic,
                                self.contexts,
                                machine,
                                state,
                                statement,
                            )
                        },
                    )
                })
                .as_ref()
                .is_some_and(|evidence| evidence.cases_are_selected(subject))
    }
}

fn consumer_scope(
    program: &dyn validation::CheckingInput,
    point: ProgramPoint,
) -> Option<(symbols::SymbolHandle, symbols::SymbolHandle, usize)> {
    Some(match point {
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
            let machine = program.machine_by_symbol(machine_symbol)?;
            let state = program.machine_states(machine).first()?;
            (machine_symbol, state.symbol, 0)
        }
        ProgramPoint::Global | ProgramPoint::Definition { .. } => return None,
    })
}
