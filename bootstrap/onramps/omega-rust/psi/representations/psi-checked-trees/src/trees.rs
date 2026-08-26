use crate::{
    BlockingFacts, BorrowFacts, CarryFacts, CheckedOperatorFacts, CheckedValueFacts, DomainFacts,
    DynamicConformanceFacts, FlowFacts, IndexCompatibilityFacts, MachineContractPlans,
    MutationFacts, NominalMachineUseFacts, ProofFacts, QualificationFacts, ServiceReachFacts,
    SuspensionFacts, SynchronousInvocationFacts, TerminationFacts,
};

/// Exact checked certificate for the first fact-call projection rung. The
/// expression handles rejoin the retained typed call/member tree; all nominal
/// coordinates are duplicated here so later review cannot accept a merely
/// shape-compatible projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedFactCallProjection {
    pub projection_expression: psi_typed_trees::expression::ExpressionHandle,
    pub call_expression: psi_typed_trees::expression::ExpressionHandle,
    pub target_machine: psi_symbols::SymbolHandle,
    pub target_state: psi_symbols::SymbolHandle,
    pub machine_arguments: Box<[psi_typed_trees::expression::StaticMachineArgument]>,
    pub result_type: psi_typed_trees::types::TypeReferenceHandle,
    pub field: psi_symbols::SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckFacts {
    pub semantic: psi_facts::FactPlan,
    pub borrow: BorrowFacts,
    pub proof: ProofFacts,
    pub values: CheckedValueFacts,
    pub domains: DomainFacts,
    /// Exact complete nominal conformance selected by each admitted local
    /// dynamic coercion.
    pub dynamic_conformances: DynamicConformanceFacts,
    /// ENT4: exact nominal machine satisfaction selected for each admitted
    /// static machine argument, before specialization consumes its syntax.
    pub nominal_machine_uses: NominalMachineUseFacts,
    pub operators: CheckedOperatorFacts,
    pub capabilities: psi_effects::CapabilityFlowPlan,
    pub flow: FlowFacts,
    /// PDI3 named equality verification conditions and their exact discharge
    /// evidence. These rows never participate in semantic type identity.
    pub index_compatibility: IndexCompatibilityFacts,
    /// Body-derived, state-relative may-write frames. Mutation remains an
    /// independent implementation axis outside public machine contracts.
    pub mutation: MutationFacts,
    /// EFX: symbol-resolved boundary-service declarations plus grouped
    /// machine/state/call reach summaries.
    pub service_reaches: ServiceReachFacts,
    /// Exact machine-keyed direct synchronous invocation contracts. This axis
    /// remains independent from service reach and operational possibilities.
    pub synchronous_invocations: SynchronousInvocationFacts,
    /// Exact machine-keyed suspension interface and checked inference. Worker
    /// blocking remains a separate contract axis.
    pub suspensions: SuspensionFacts,
    /// Exact machine-keyed worker-blocking interface and checked inference.
    /// Suspension remains a separate contract axis.
    pub blocking: BlockingFacts,
    /// Exact machine-keyed termination interface, checked summary, and
    /// private implementation witness.
    pub termination: TerminationFacts,
    /// STR4 checked plans, slice 2 (decision 19): the semantic-domain
    /// commitments each machine's body makes (arithmetic-policy casts v1).
    pub qualifications: QualificationFacts,
    /// STR4 checked plans (machine_taxonomy.md): the normalized machine
    /// semantic contracts -- published halves + deterministic fingerprint.
    pub contract_plans: MachineContractPlans,
    /// CRY1: checker-derived four-axis carry policy per transparent data
    /// declaration. Later live-set and runtime-admission passes consume this
    /// plan rather than re-deriving policy from syntax.
    pub carry: CarryFacts,
    /// Denotational, non-executing call-result projections admitted by
    /// validation. Package review must rejoin this row exactly.
    pub fact_call_projections: Vec<CheckedFactCallProjection>,
}

impl CheckFacts {
    pub fn with_roots(
        semantic: psi_facts::FactPlan,
        borrow: BorrowFacts,
        proof: ProofFacts,
        values: CheckedValueFacts,
        domains: DomainFacts,
        dynamic_conformances: DynamicConformanceFacts,
        nominal_machine_uses: NominalMachineUseFacts,
        operators: CheckedOperatorFacts,
        capabilities: psi_effects::CapabilityFlowPlan,
        flow: FlowFacts,
        index_compatibility: IndexCompatibilityFacts,
        mutation: MutationFacts,
        service_reaches: ServiceReachFacts,
        synchronous_invocations: SynchronousInvocationFacts,
        suspensions: SuspensionFacts,
        blocking: BlockingFacts,
        termination: TerminationFacts,
        qualifications: QualificationFacts,
        contract_plans: MachineContractPlans,
        carry: CarryFacts,
        fact_call_projections: Vec<CheckedFactCallProjection>,
    ) -> Self {
        Self {
            semantic,
            borrow,
            proof,
            values,
            domains,
            dynamic_conformances,
            nominal_machine_uses,
            operators,
            capabilities,
            flow,
            index_compatibility,
            mutation,
            service_reaches,
            synchronous_invocations,
            suspensions,
            blocking,
            termination,
            qualifications,
            contract_plans,
            carry,
            fact_call_projections,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTrees {
    pub typed: psi_typed_trees::TypedTrees,
    pub facts: CheckFacts,
}

impl CheckedTrees {
    pub fn with_roots(typed: psi_typed_trees::TypedTrees, facts: CheckFacts) -> Self {
        Self { typed, facts }
    }
}

impl std::ops::Deref for CheckedTrees {
    type Target = psi_typed_trees::TypedTrees;

    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

#[cfg(test)]
mod tests {
    use crate::{CheckFacts, CheckedTrees};

    #[test]
    fn checked_tree_constructor_keeps_typed_tree_and_fact_roots_explicit() {
        let typed = psi_typed_trees::TypedTrees::default();
        let facts = CheckFacts::default();

        let checked = CheckedTrees::with_roots(typed.clone(), facts.clone());

        assert_eq!(checked.typed, typed);
        assert_eq!(checked.facts, facts);
    }
}

impl AsRef<psi_typed_trees::TypedTrees> for CheckedTrees {
    fn as_ref(&self) -> &psi_typed_trees::TypedTrees {
        &self.typed
    }
}
