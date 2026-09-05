pub(in crate::semantic_calls) struct CallSiteTraversal<'program, 'ordinal> {
    pub(super) program: &'program typed_trees::TypedTrees,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) current_statement_index: usize,
    pub(super) target_statement_index: usize,
    pub(super) target_call_ordinal: usize,
    pub(super) current_ordinal: &'ordinal mut usize,
}

impl CallSiteTraversal<'_, '_> {
    pub(in crate::semantic_calls) fn new<'program, 'ordinal>(
        program: &'program typed_trees::TypedTrees,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
        current_statement_index: usize,
        target_statement_index: usize,
        target_call_ordinal: usize,
        current_ordinal: &'ordinal mut usize,
    ) -> CallSiteTraversal<'program, 'ordinal> {
        CallSiteTraversal {
            program,
            machine,
            state,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        }
    }

    pub(super) fn is_target_call_site(&self) -> bool {
        self.current_statement_index == self.target_statement_index
            && *self.current_ordinal == self.target_call_ordinal
    }

    pub(super) fn advance_call_ordinal(&mut self) {
        *self.current_ordinal = self.current_ordinal.saturating_add(1);
    }
}
