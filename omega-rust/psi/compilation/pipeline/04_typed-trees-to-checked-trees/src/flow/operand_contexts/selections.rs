//! Reuse an operand's evidence selection across one invocation's obligations.
//!
//! The immutable borrows fix every producer input and the invocation epoch.
//! The sparse key retains both generations of the operand/type pair: one
//! operand can be checked under distinct bound destination types. Context
//! selection and authenticated tag/membership observations are reusable here;
//! proof verdicts and projected query subjects are not. Each observation roster
//! is prepared lazily, only when its consumer needs that kind of evidence.

use crate::flow::CapturedOperand;
use arena::HandleSpan;
use checked_trees::{FlowConstraintRef, FlowFacts};
use facts::{FactContextHandle, FactPlan};
use std::cell::OnceCell;
use std::collections::BTreeMap;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

pub(crate) struct OperandContextSelections<'checking> {
    program: &'checking dyn validation::CheckingInput,
    flow: &'checking FlowFacts,
    semantic: &'checking FactPlan,
    invocation: HandleSpan<FlowConstraintRef>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    // A dense operand-by-type table would reserve the Cartesian product for
    // each call. Keep only queried pairs, without growing linear identity scans.
    selected: BTreeMap<(u32, u32, u32, u32), OperandContextSelection<'checking>>,
    scratch: Option<OperandContextSelection<'checking>>,
}

/// The selected namespace and its authenticated observations share one
/// invocation lifetime. Projected queries and proof verdicts stay per query.
pub(crate) struct OperandContextSelection<'checking> {
    pub contexts: Vec<FactContextHandle>,
    program: &'checking dyn validation::CheckingInput,
    semantic: &'checking FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    cases: OnceCell<Option<crate::flow::CaseEvidence<'checking>>>,
    memberships: OnceCell<crate::flow::MembershipEvidence<'checking>>,
}

impl OperandContextSelection<'_> {
    pub(crate) fn has_membership(
        &self,
        subject: &validation::CanonicalPlace,
        domain: SymbolHandle,
        instance: language_semantics::SemanticDomainId,
    ) -> bool {
        self.memberships
            .get_or_init(|| {
                crate::flow::MembershipEvidence::prepare(
                    self.program,
                    self.semantic,
                    &self.contexts,
                )
            })
            .contains(subject, domain, instance)
    }

    fn cases(&self) -> &Option<crate::flow::CaseEvidence<'_>> {
        self.cases.get_or_init(|| {
            crate::flow::CaseEvidence::prepare(
                self.program,
                self.semantic,
                &self.contexts,
                self.machine_symbol,
                self.state_symbol,
                self.statement_index,
            )
        })
    }

    pub(crate) fn cases_are_selected(&self, subject: &validation::CanonicalPlace) -> bool {
        !subject
            .segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
            || self
                .cases()
                .as_ref()
                .is_some_and(|cases| cases.cases_are_selected(subject))
    }

    pub(crate) fn path_is_inactive(&self, subject: &validation::CanonicalPlace) -> bool {
        subject
            .segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
            && self
                .cases()
                .as_ref()
                .is_some_and(|cases| cases.path_is_inactive(subject))
    }
}

impl<'checking> OperandContextSelections<'checking> {
    pub(crate) fn new(
        program: &'checking dyn validation::CheckingInput,
        flow: &'checking FlowFacts,
        semantic: &'checking FactPlan,
        invocation: HandleSpan<FlowConstraintRef>,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    ) -> Self {
        Self {
            program,
            flow,
            semantic,
            invocation,
            machine_symbol,
            state_symbol,
            statement_index,
            selected: BTreeMap::new(),
            scratch: None,
        }
    }

    pub(crate) fn for_operand(
        &mut self,
        operand: CapturedOperand<'checking>,
        type_reference: TypeReferenceHandle,
    ) -> Option<&OperandContextSelection<'checking>> {
        if !operand.belongs_to(&self.flow.control) {
            return None;
        }
        let select = || {
            let contexts = super::operand_contexts(
                self.program,
                &self.flow.contexts,
                &self.flow.control,
                self.semantic,
                &operand,
                type_reference,
                self.invocation,
            );
            OperandContextSelection {
                contexts,
                program: self.program,
                semantic: self.semantic,
                machine_symbol: self.machine_symbol,
                state_symbol: self.state_symbol,
                statement_index: self.statement_index,
                cases: OnceCell::new(),
                memberships: OnceCell::new(),
            }
        };
        // Scratch operands have no arena identity and cannot enter the table.
        // They still use the exact same selection law without reuse.
        let handle = operand.handle();
        if !handle.is_valid() {
            return Some(self.scratch.insert(select()));
        }
        let key = (
            handle.arena_index(),
            handle.generation(),
            type_reference.arena_index(),
            type_reference.generation(),
        );
        Some(self.selected.entry(key).or_insert_with(select))
    }
}
