use crate::{
    BlockingFacts, BorrowFacts, CarryFacts, CheckedOperatorFacts, CheckedValueFacts, DomainFacts,
    DynamicConformanceFacts, FlowFacts, IndexCompatibilityFacts, MachineContractPlans,
    MutationFacts, NominalMachineUseFacts, ProofFacts, QualificationFacts, ServiceReachFacts,
    SuspensionFacts, SynchronousInvocationFacts, TerminationFacts,
};

use crate::{CheckedFactCallProjection, CheckedIntrinsicCallFact, CheckedPlacedViewInput};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckFacts {
    pub semantic: facts::FactPlan,
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
    pub capabilities: flow_effects::CapabilityFlowPlan,
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
    /// Compiler-owned call meanings selected only after exact receiver and
    /// owner checking.
    pub intrinsic_calls: Vec<CheckedIntrinsicCallFact>,
    /// Direct state inputs whose opaque placed-view meaning has been joined to
    /// its exact source-derived placement plan. These rows grant no runtime
    /// storage, accessor, provider, or ABI authority.
    pub placed_view_inputs: Vec<CheckedPlacedViewInput>,
}

impl CheckFacts {
    pub fn with_roots(
        semantic: facts::FactPlan,
        borrow: BorrowFacts,
        proof: ProofFacts,
        values: CheckedValueFacts,
        domains: DomainFacts,
        dynamic_conformances: DynamicConformanceFacts,
        nominal_machine_uses: NominalMachineUseFacts,
        operators: CheckedOperatorFacts,
        capabilities: flow_effects::CapabilityFlowPlan,
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
            intrinsic_calls: Vec::new(),
            placed_view_inputs: Vec::new(),
        }
    }
}
