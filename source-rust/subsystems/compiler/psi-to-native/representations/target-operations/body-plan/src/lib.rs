//! Stage 04 publishes the prepared instructions, their emission facts and the
//! private functions they reference together for instruction selection.
#![forbid(unsafe_code)]

use verified_optimization_input::publication::{
    SemanticEmissionFacts, target_replay::BodyPublication,
};

/// Only target lowering issues the instructions and obligations for a body together.
///
/// ```compile_fail,E0616
/// fn replace_references(plan: &mut native_body_plan::NativeBodyPlan) {
///     plan.private_references.clear();
/// }
/// ```
pub struct NativeBodyPlan {
    selection: legalized_operations::PreparedInstructionSelection,
    facts: BodyPublication<SemanticEmissionFacts>,
    private_references: Vec<function_identity::MachineFunctionIdentity>,
}

impl NativeBodyPlan {
    pub fn from_lowering(
        _issuer: native_body_plan_issuer::LoweringIssuer,
        selection: legalized_operations::PreparedInstructionSelection,
        facts: BodyPublication<SemanticEmissionFacts>,
        private_references: Vec<function_identity::MachineFunctionIdentity>,
    ) -> Self {
        Self {
            selection,
            facts,
            private_references,
        }
    }

    pub fn semantic_facts(&self) -> &SemanticEmissionFacts {
        &self.facts.facts
    }

    pub fn original_for_audit(
        &self,
    ) -> Option<&verified_optimization_input::publication::target_replay::RetainedTargetOperations>
    {
        self.facts.original_for_audit()
    }

    pub fn release_original_for_audit(&mut self) {
        self.facts.release_original();
    }

    pub fn private_references(&self) -> &[function_identity::MachineFunctionIdentity] {
        &self.private_references
    }

    pub fn selections(&self) -> &optimization_core::OptimizationSelections {
        &self.selection.selections
    }

    pub fn into_selection(
        self,
    ) -> (
        legalized_operations::PreparedInstructionSelection,
        BodyPublication<SemanticEmissionFacts>,
        Vec<function_identity::MachineFunctionIdentity>,
    ) {
        (self.selection, self.facts, self.private_references)
    }

    pub fn execution_plan(&self) -> &legalized_operations::PreparedExecutionPlan {
        &self.selection.plan
    }
}
