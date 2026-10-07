// Callers select one statement before entering traversal. Ordinals belong to
// that walk; source statement coordinates remain on the authored call facts.
pub(in crate::semantic::calls) struct CallSiteTraversal<'program, 'ordinal> {
    pub(super) program: &'program typed_trees::TypedTrees,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) target_call_ordinal: usize,
    pub(super) current_ordinal: usize,
    /// When set, every reached call site is pushed in ordinal order instead
    /// of stopping at `target_call_ordinal` — one statement walk binds all of
    /// a statement's call sites at once.
    /// Selection ancestry does not change call ordinals or lookup membership.
    conditional: bool,
    pub(in crate::semantic::calls) matched_conditionally: bool,
    pub(super) collected: Option<&'ordinal mut Vec<super::super::CallSite<'program>>>,
    /// Normalization preserves a DAG's authored expression handles. A second
    /// edge to one call is not a second authored invocation or ordinal.
    expressions: Vec<typed_trees::expression::ExpressionHandle>,
}

impl<'program> CallSiteTraversal<'program, '_> {
    pub(in crate::semantic::calls) fn new<'ordinal>(
        program: &'program typed_trees::TypedTrees,
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
            conditional: false,
            matched_conditionally: false,
            expressions: Vec::new(),
        }
    }

    /// Traversal that records every call site of one statement in ordinal
    /// order instead of stopping at the first ordinal match.
    pub(in crate::semantic::calls) fn collecting<'ordinal>(
        program: &'program typed_trees::TypedTrees,
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
            collected: Some(collected),
            conditional: false,
            matched_conditionally: false,
            expressions: Vec::new(),
        }
    }

    /// Site admission shared by every traversal arm: collect mode records the
    /// site and keeps walking; match mode returns it once the ordinal lines up.
    pub(super) fn visit(
        &mut self,
        site: super::super::CallSite<'program>,
    ) -> Option<super::super::CallSite<'program>> {
        if let super::super::CallSite::Expression { expression, .. } = site {
            if self.expressions.contains(&expression) {
                return None;
            }
            self.expressions.push(expression);
        }
        if let Some(collected) = self.collected.as_deref_mut() {
            collected.push(site);
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

    pub(super) fn is_target_call_site(&self) -> bool {
        self.current_ordinal == self.target_call_ordinal
    }

    pub(super) fn advance_call_ordinal(&mut self) {
        self.current_ordinal = self.current_ordinal.saturating_add(1);
    }
}
