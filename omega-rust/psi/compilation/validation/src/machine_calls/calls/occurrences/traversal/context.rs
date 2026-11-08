//! Call ordinals and their owning target lanes come from one traversal.

enum CollectedCalls<'program, 'query> {
    Sites(&'query mut Vec<super::super::CallSite<'program>>),
    Occurrences(&'query mut Vec<super::super::StatementCallOccurrence<'program>>),
}

// Callers select one statement before entering traversal. Ordinals belong to
// that walk; source statement coordinates remain on the authored call facts.
pub(in crate::machine_calls::calls::occurrences) struct CallSiteTraversal<'program, 'ordinal> {
    pub(super) program: &'program dyn crate::CheckingInput,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) target_call_ordinal: usize,
    pub(super) current_ordinal: usize,
    /// When set, every reached call site is pushed in ordinal order instead
    /// of stopping at `target_call_ordinal` — one statement walk binds all of
    /// a statement's call sites at once.
    /// Selection ancestry does not change call ordinals or lookup membership.
    conditional: bool,
    pub(in crate::machine_calls::calls::occurrences) matched_conditionally: bool,
    collected: Option<CollectedCalls<'program, 'ordinal>>,
    transition_target: typed_trees::statement::TransitionTargetHandle,
    /// Normalization preserves a DAG's authored expression handles. A second
    /// edge to one call is not a second authored invocation or ordinal.
    expressions: std::collections::BTreeSet<(u32, u32)>,
}

impl<'program> CallSiteTraversal<'program, '_> {
    pub(in crate::machine_calls::calls::occurrences) fn new<'ordinal>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
        target_call_ordinal: usize,
    ) -> CallSiteTraversal<'program, 'ordinal> {
        CallSiteTraversal {
            program,
            machine,
            state,
            target_call_ordinal,
            current_ordinal: 0,
            collected: None,
            transition_target: Default::default(),
            conditional: false,
            matched_conditionally: false,
            expressions: Default::default(),
        }
    }

    /// Traversal that records every call site of one statement in ordinal
    /// order instead of stopping at the first ordinal match.
    pub(in crate::machine_calls::calls::occurrences) fn collecting<'ordinal>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
        collected: &'ordinal mut Vec<super::super::CallSite<'program>>,
    ) -> CallSiteTraversal<'program, 'ordinal> {
        CallSiteTraversal {
            program,
            machine,
            state,
            target_call_ordinal: usize::MAX,
            current_ordinal: 0,
            collected: Some(CollectedCalls::Sites(collected)),
            transition_target: Default::default(),
            conditional: false,
            matched_conditionally: false,
            expressions: Default::default(),
        }
    }

    pub(in crate::machine_calls::calls::occurrences) fn collecting_occurrences<'ordinal>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
        collected: &'ordinal mut Vec<super::super::StatementCallOccurrence<'program>>,
    ) -> CallSiteTraversal<'program, 'ordinal> {
        let mut traversal = Self::new(program, machine, state, usize::MAX);
        traversal.collected = Some(CollectedCalls::Occurrences(collected));
        traversal
    }

    /// Site admission shared by every traversal arm: collect mode records the
    /// site and keeps walking; match mode returns it once the ordinal lines up.
    pub(super) fn visit(
        &mut self,
        site: super::super::CallSite<'program>,
    ) -> Option<super::super::CallSite<'program>> {
        if let super::super::CallSite::Expression { expression, .. } = site
            && !self
                .expressions
                .insert((expression.arena_index(), expression.generation()))
        {
            return None;
        }
        if let Some(collected) = self.collected.as_mut() {
            match collected {
                CollectedCalls::Sites(collected) => collected.push(site),
                CollectedCalls::Occurrences(collected) => {
                    collected.push(super::super::StatementCallOccurrence {
                        site,
                        transition_target: self.transition_target,
                    })
                }
            }
            self.advance_call_ordinal();
            return None;
        }
        if self.is_target_call_site() {
            self.matched_conditionally = self.conditional;
            return Some(site);
        }
        self.advance_call_ordinal();
        None
    }

    /// Visit an alternative with the same occurrence numbering. The selected
    /// scope belongs to custody; it never prunes discovery of an authored call.
    pub(super) fn selected(
        &mut self,
        visit: impl FnOnce(&mut Self) -> Option<super::super::CallSite<'program>>,
    ) -> Option<super::super::CallSite<'program>> {
        let enclosing = self.conditional;
        self.conditional = true;
        let result = visit(self);
        self.conditional = enclosing;
        result
    }

    pub(super) fn selected_target(
        &mut self,
        target: typed_trees::statement::TransitionTargetHandle,
        visit: impl FnOnce(&mut Self) -> Option<super::super::CallSite<'program>>,
    ) -> Option<super::super::CallSite<'program>> {
        let enclosing = self.transition_target;
        self.transition_target = target;
        let result = self.selected(visit);
        self.transition_target = enclosing;
        result
    }

    pub(super) fn is_target_call_site(&self) -> bool {
        self.current_ordinal == self.target_call_ordinal
    }

    pub(super) fn advance_call_ordinal(&mut self) {
        self.current_ordinal = self.current_ordinal.saturating_add(1);
    }
}
