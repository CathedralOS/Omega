//! Descriptor words detach from their former homes; their backing stays held.
//! This is shared compatibility geometry for a pair of staged arguments, not
//! authority to access either backing. Unknown geometry refuses that pair;
//! a single operand has no pair obligation to discharge.
use checked_trees::{
    BorrowArgumentAccessFact, BorrowCallFact, CapturedPlace, CheckFacts, FlowStateFact,
};
use diagnostics::Diagnostic;

#[derive(Default)]
pub(super) enum HeldPlaces {
    #[default]
    Unrequested,
    Known(Vec<CapturedPlace>),
    Unavailable,
}

impl HeldPlaces {
    pub(super) fn resolve(
        &mut self,
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &BorrowCallFact,
        access: &BorrowArgumentAccessFact,
        target: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> &[CapturedPlace] {
        if matches!(self, Self::Unrequested) {
            let forwarded = !matches!(
                program.expression_table.expression(access.expression),
                typed_trees::expression::ExpressionNode::Borrow(_)
            ) && validation::expression_type_reference_in_state(
                program,
                state.state_symbol,
                call.statement_index,
                access.expression,
            )
            .and_then(|reference| validation::reference_shell(program, reference))
            .is_some();
            *self = if forwarded {
                match super::super::captured_descriptors::argument_places(
                    program,
                    facts,
                    state,
                    call.statement_index,
                    access.expression,
                ) {
                    Some(places) => Self::Known(places),
                    None => {
                        diagnostics.push(Diagnostic::error(format!(
                            "state `{target}` receives reference `{}` without a completed descriptor backing for simultaneous access",
                            crate::labels::borrow_access_label(program, &facts.borrow, access))));
                        Self::Unavailable
                    }
                }
            } else {
                Self::Known(vec![CapturedPlace {
                    incoming_reference: Default::default(),
                    root: access.root,
                    segments: facts.borrow.access_segments(access).to_vec(),
                }])
            };
        }
        match self {
            Self::Known(places) => places,
            Self::Unavailable | Self::Unrequested => &[],
        }
    }
}
