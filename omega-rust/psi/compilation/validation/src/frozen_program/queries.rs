//! Borrowed ancestry for unfinished checking queries. Descendants carry the
//! same immutable input view and conservative recurrence cuts. They read the
//! memos their input completed but write only their own
//! (`CheckingMemos::query_scratch`), so nothing computed under a cut is reused
//! outside it.
//! An unfinished machine population keeps the ordinary occurrence path; a repeated
//! numeric state contributes unknown evidence. Neither publishes its own premise.
//!
//! A cut makes an answer depend on where it was asked. A query that meets an
//! enclosing query's cut records that, so its owner keeps that answer out of
//! the completed record; a cut on the query's own identity is the same at
//! every call site and does not.

use crate::CheckingInput;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use symbols::SymbolHandle;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
enum QueryKind {
    Numeric {
        machine: SymbolHandle,
        state: SymbolHandle,
    },
    OperandFits {
        machine: SymbolHandle,
    },
    CompletedCases {
        machine: SymbolHandle,
    },
    RecastFormation {
        expression: typed_trees::expression::ExpressionHandle,
    },
}

pub(crate) struct CheckingQuery<'input> {
    input: &'input dyn CheckingInput,
    query: QueryKind,
    /// This query's own memos: the typed-tree memos shared from its input,
    /// and empty context-sensitive memos dropped with the query.
    memos: Option<crate::CheckingMemos>,
    /// A descendant met the cut of a query enclosing this one, so this
    /// query's answer is specific to its ancestry.
    met_enclosing_cut: AtomicBool,
    /// How many `has_active_*` answers through this query were `true`: each
    /// is an answer that holds only while this query or an enclosing one is
    /// live (`CutGuard`).
    cut_reads: AtomicU64,
}

impl std::fmt::Debug for CheckingQuery<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckingQuery")
            .field("input", &self.input)
            .field("query", &self.query)
            .finish_non_exhaustive()
    }
}

impl<'input> CheckingQuery<'input> {
    pub(crate) fn recast_formation(
        input: &'input dyn CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<Self> {
        if input.has_active_recast_formation(expression, 0) {
            return None;
        }
        Some(Self::new(input, QueryKind::RecastFormation { expression }))
    }

    pub(crate) fn numeric_observation(
        input: &'input dyn CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<Self> {
        if input.has_active_numeric_observation(machine, state) {
            return None;
        }
        Some(Self::new(input, QueryKind::Numeric { machine, state }))
    }

    pub(crate) fn operand_fit_population(
        input: &'input dyn CheckingInput,
        machine: SymbolHandle,
    ) -> Option<Self> {
        if input.has_active_operand_fit_population(machine) {
            return None;
        }
        Some(Self::new(input, QueryKind::OperandFits { machine }))
    }

    pub(crate) fn completed_case_population(
        input: &'input dyn CheckingInput,
        machine: SymbolHandle,
    ) -> Option<Self> {
        if input.has_active_completed_case_population(machine) {
            return None;
        }
        Some(Self::new(input, QueryKind::CompletedCases { machine }))
    }

    fn new(input: &'input dyn CheckingInput, query: QueryKind) -> Self {
        Self {
            input,
            query,
            memos: input
                .checking_memos()
                .map(crate::CheckingMemos::query_scratch),
            met_enclosing_cut: AtomicBool::new(false),
            cut_reads: AtomicU64::new(0),
        }
    }

    /// Whether this query's answer depends on an enclosing query being live.
    pub(crate) fn met_enclosing_cut(&self) -> bool {
        self.met_enclosing_cut.load(Ordering::Relaxed)
    }

    /// Ancestry is searched from the innermost query outward, so every query
    /// passed before the match lies inside the cut query.
    fn enclosing_cut(&self, enclosing: bool) -> bool {
        if enclosing {
            self.met_enclosing_cut.store(true, Ordering::Relaxed);
        }
        enclosing
    }

    /// Every `has_active_*` answer passes through here, so a `true` one is
    /// always counted.
    fn counted(&self, active: bool) -> bool {
        if active {
            self.cut_reads.fetch_add(1, Ordering::Relaxed);
        }
        active
    }
}

// Query ancestry changes no checking authority, geometry or selected meaning.
// Forward every semantic input to its original owner; memos are the query's own.
impl CheckingInput for CheckingQuery<'_> {
    fn has_active_recast_formation(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
        depth: usize,
    ) -> bool {
        let formation = matches!(self.query, QueryKind::RecastFormation { .. });
        self.counted(
            matches!(self.query, QueryKind::RecastFormation { expression: active }
                if active == expression)
                || self.enclosing_cut(
                    self.input
                        .has_active_recast_formation(expression, depth + usize::from(formation)),
                ),
        )
    }

    fn external_invocation_roots(&self) -> Option<&[symbols::SymbolHandle]> {
        self.input.external_invocation_roots()
    }

    fn typed_program(&self) -> &typed_trees::TypedTrees {
        self.input.typed_program()
    }

    fn has_active_numeric_observation(&self, machine: SymbolHandle, state: SymbolHandle) -> bool {
        self.counted(
            matches!(self.query, QueryKind::Numeric { machine: active, state: active_state }
            if active == machine && active_state == state)
                || self.enclosing_cut(self.input.has_active_numeric_observation(machine, state)),
        )
    }

    fn has_active_operand_fit_population(&self, machine: SymbolHandle) -> bool {
        self.counted(
            matches!(self.query, QueryKind::OperandFits { machine: active } if active == machine)
                || self.enclosing_cut(self.input.has_active_operand_fit_population(machine)),
        )
    }

    fn has_active_completed_case_population(&self, machine: SymbolHandle) -> bool {
        self.counted(
            matches!(self.query, QueryKind::CompletedCases { machine: active } if active == machine)
                || self.enclosing_cut(self.input.has_active_completed_case_population(machine)),
        )
    }

    fn defers_pending_layout_geometry(&self) -> bool {
        self.input.defers_pending_layout_geometry()
    }

    fn retained_borrow_custodies(&self) -> &[checked_trees::RetainedBorrowCustodyFact] {
        self.input.retained_borrow_custodies()
    }

    fn layout_target(&self) -> Option<&str> {
        self.input.layout_target()
    }

    fn opaque_representation_selections(
        &self,
    ) -> &[representation_selections::OpaqueRepresentationSelection] {
        self.input.opaque_representation_selections()
    }

    fn authored_declaration_selections(
        &self,
    ) -> &language_semantics::declaration_selection::AuthoredDeclarationSelections {
        self.input.authored_declaration_selections()
    }

    fn checking_memos(&self) -> Option<&crate::CheckingMemos> {
        self.memos.as_ref()
    }

    /// This query's count plus every enclosing query's: a cut answer read
    /// anywhere in the ancestry changes it.
    fn cut_reads(&self) -> u64 {
        self.cut_reads.load(Ordering::Relaxed) + self.input.cut_reads()
    }

    fn shared_memos(&self) -> Option<&crate::CheckingMemos> {
        self.input.shared_memos()
    }

    fn proof_only_classification(
        &self,
    ) -> std::sync::Arc<typed_trees::proof_only::ProofOnlyClassification> {
        self.input.proof_only_classification()
    }

    fn call_frame_tables(
        &self,
    ) -> Option<std::sync::Arc<crate::machine_calls::calls::CallFrameTables<'_>>> {
        self.input.call_frame_tables()
    }

    fn prepared_effect_plans(&self) -> Option<&crate::PreparedEffectPlans> {
        self.input.prepared_effect_plans()
    }

    fn storage_graph(&self) -> Option<&checked_trees::FlowControlFacts> {
        self.input.storage_graph()
    }

    fn expression_interval(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<crate::ExpressionInterval> {
        self.input.expression_interval(expression)
    }

    fn prepared_mathematics(
        &self,
    ) -> Option<crate::mathematical_applications::PreparedMathematicalView<'_>> {
        self.input.prepared_mathematics()
    }
}
