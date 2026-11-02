//! Borrowed ancestry for unfinished checking queries. Descendants carry the
//! same immutable input view, completed memos, and conservative recurrence cuts.
//! An unfinished machine population keeps the ordinary occurrence path; a repeated
//! numeric state contributes unknown evidence. Neither publishes its own premise.

use crate::CheckingInput;
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
}

#[derive(Debug)]
pub(crate) struct CheckingQuery<'input> {
    input: &'input dyn CheckingInput,
    query: QueryKind,
}

impl<'input> CheckingQuery<'input> {
    pub(crate) fn numeric_observation(
        input: &'input dyn CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<Self> {
        if input.has_active_numeric_observation(machine, state) {
            return None;
        }
        Some(Self {
            input,
            query: QueryKind::Numeric { machine, state },
        })
    }

    pub(crate) fn operand_fit_population(
        input: &'input dyn CheckingInput,
        machine: SymbolHandle,
    ) -> Option<Self> {
        if input.has_active_operand_fit_population(machine) {
            return None;
        }
        Some(Self {
            input,
            query: QueryKind::OperandFits { machine },
        })
    }

    pub(crate) fn completed_case_population(
        input: &'input dyn CheckingInput,
        machine: SymbolHandle,
    ) -> Option<Self> {
        if input.has_active_completed_case_population(machine) {
            return None;
        }
        Some(Self {
            input,
            query: QueryKind::CompletedCases { machine },
        })
    }
}

// Query ancestry changes no checking authority, geometry, selected meaning or
// completed memo. Forward every semantic input to its original owner.
impl CheckingInput for CheckingQuery<'_> {
    fn typed_program(&self) -> &typed_trees::TypedTrees {
        self.input.typed_program()
    }

    fn has_active_numeric_observation(&self, machine: SymbolHandle, state: SymbolHandle) -> bool {
        matches!(self.query, QueryKind::Numeric { machine: active, state: active_state }
            if active == machine && active_state == state)
            || self.input.has_active_numeric_observation(machine, state)
    }

    fn has_active_operand_fit_population(&self, machine: SymbolHandle) -> bool {
        matches!(self.query, QueryKind::OperandFits { machine: active } if active == machine)
            || self.input.has_active_operand_fit_population(machine)
    }

    fn has_active_completed_case_population(&self, machine: SymbolHandle) -> bool {
        matches!(self.query, QueryKind::CompletedCases { machine: active } if active == machine)
            || self.input.has_active_completed_case_population(machine)
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
        self.input.checking_memos()
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

    fn prepared_mathematics(
        &self,
    ) -> Option<crate::mathematical_applications::PreparedMathematicalView<'_>> {
        self.input.prepared_mathematics()
    }
}
