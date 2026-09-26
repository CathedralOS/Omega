pub(in crate::semantic::calls) struct CallSiteTraversal<'program, 'ordinal> {
    pub(super) program: &'program typed_trees::TypedTrees,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) current_statement_index: usize,
    pub(super) target_statement_index: usize,
    pub(super) target_call_ordinal: usize,
    pub(super) current_ordinal: &'ordinal mut usize,
    /// When set, every reached call site is pushed in ordinal order instead
    /// of stopping at `target_call_ordinal` — one statement walk binds all of
    /// a statement's call sites at once.
    pub(super) collected: Option<&'ordinal mut Vec<super::super::CallSite<'program>>>,
}

impl<'program> CallSiteTraversal<'program, '_> {
    pub(in crate::semantic::calls) fn new<'ordinal>(
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
            collected: None,
        }
    }

    /// Traversal that records every call site of one statement in ordinal
    /// order instead of stopping at the first ordinal match.
    pub(in crate::semantic::calls) fn collecting<'ordinal>(
        program: &'program typed_trees::TypedTrees,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
        statement_index: usize,
        current_ordinal: &'ordinal mut usize,
        collected: &'ordinal mut Vec<super::super::CallSite<'program>>,
    ) -> CallSiteTraversal<'program, 'ordinal> {
        CallSiteTraversal {
            program,
            machine,
            state,
            current_statement_index: statement_index,
            target_statement_index: statement_index,
            target_call_ordinal: usize::MAX,
            current_ordinal,
            collected: Some(collected),
        }
    }

    /// Site admission shared by every traversal arm: collect mode records the
    /// site and keeps walking; match mode returns it once the ordinal lines up.
    pub(super) fn visit(
        &mut self,
        site: super::super::CallSite<'program>,
    ) -> Option<super::super::CallSite<'program>> {
        if let Some(collected) = self.collected.as_deref_mut() {
            collected.push(site);
            self.advance_call_ordinal();
            return None;
        }
        if self.is_target_call_site() {
            return Some(site);
        }
        self.advance_call_ordinal();
        None
    }

    pub(super) fn is_target_call_site(&self) -> bool {
        self.current_statement_index == self.target_statement_index
            && *self.current_ordinal == self.target_call_ordinal
    }

    pub(super) fn advance_call_ordinal(&mut self) {
        *self.current_ordinal = self.current_ordinal.saturating_add(1);
    }
}
