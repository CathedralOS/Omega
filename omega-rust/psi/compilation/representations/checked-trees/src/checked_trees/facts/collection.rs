use crate::{
    BlockingFacts, BorrowFacts, CarryFacts, CheckedOperatorFacts, CheckedValueFacts, DomainFacts,
    DynamicConformanceFacts, FlowFacts, IndexCompatibilityFacts, MachineContractPlans,
    MutationFacts, NominalMachineUseFacts, ProofFacts, QualificationFacts,
    RequirementCallSpecializationFacts, ServiceReachFacts, SuspensionFacts,
    SynchronousInvocationFacts, TerminationFacts,
};

use crate::{CheckedFactCallProjection, CheckedIntrinsicCallFact, CheckedPlacedViewInput};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckFacts {
    /// Completed source assignments proved to fit their physical integer encoding.
    pub stored_integer_writes: arena::Arena<crate::CheckedStoredIntegerWrite>,
    /// Source `[]` occurrences whose every bounds check proved a scalar index
    /// within its collection's live length, ordered by handle position.
    /// Lowering marks the read it emits for one of them, so that read's bound
    /// obligation consumes the decision instead of searching for a proof.
    pub proven_scalar_indexes: Vec<typed_trees::expression::ExpressionHandle>,
    /// Indexed byte assignments whose live bound and declared byte predicates
    /// were established at the store itself, ordered by target handle position.
    /// A later restoration of an invariant does not establish this decision.
    pub proven_byte_writes: Vec<typed_trees::expression::ExpressionHandle>,
    /// Calls whose every `requires` fact checking proved at the call site,
    /// keyed by calling state, statement and resolved target. For a state
    /// transfer this is the target state's arrival contract, so lowering
    /// can publish the matching arrival obligation as decided.
    pub decided_call_requires: Vec<DecidedCallRequires>,
    /// Final source execution dependencies, including selected cleanup edges.
    /// The memo scope accelerates queries; it is not their semantic owner.
    pub operational: flow_effects::OperationalPlan,
    pub semantic: facts::FactPlan,
    pub borrow: BorrowFacts,
    /// Every type reference's borrow custody, classified once over the
    /// frozen type table. Lowering reads these rows instead of re-entering
    /// checking to reclassify a carrier at each of its call sites.
    pub reference_custody: crate::ReferenceCustodyFacts,
    /// Every declared state's reference-result custody, classified once over
    /// the frozen typed program. Lowering reads these rows instead of
    /// re-entering checking to reclassify a state's completion at each of
    /// its call sites.
    pub state_reference_custody: crate::StateReferenceCustodyFacts,
    /// Every type reference's ownership and content classification,
    /// classified once over the frozen type table. Lowering reads these rows
    /// instead of re-running checking's recursive contents walk at each of
    /// its call sites.
    pub content_classes: crate::ContentClassFacts,
    /// Selected source death edges and their remaining custody, retained for
    /// symbolic templates as well as concrete execution.
    pub contextual_cleanup: crate::ContextualCleanupFacts,
    pub proof: ProofFacts,
    pub values: CheckedValueFacts,
    pub domains: DomainFacts,
    /// Exact complete nominal conformance selected by each admitted local
    /// dynamic coercion.
    pub dynamic_conformances: DynamicConformanceFacts,
    /// ENT4: exact nominal machine satisfaction selected for each admitted
    /// static machine argument, before specialization consumes its syntax.
    pub nominal_machine_uses: NominalMachineUseFacts,
    /// The specialization each generic call's admitted static machine
    /// arguments derived: `Type` substitutions plus each `machine` binder's
    /// selected provider entry. A structural contract emits no nominal use
    /// row, so this is the only checked record of what such a call
    /// instantiated.
    pub requirement_call_specializations: RequirementCallSpecializationFacts,
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
    /// STR4 checked plans (wiki/spec/language/machines.md): the normalized machine
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
    /// Exact selected call associations; never substitutes for source call facts.
    pub boundary_adapter_dispatch: Vec<crate::CheckedBoundaryAdapterDispatch>,
    /// Direct state inputs whose opaque placed-view meaning has been joined to
    /// its exact source-derived placement plan. These rows grant no runtime
    /// storage, accessor, provider, or ABI authority.
    pub placed_view_inputs: Vec<CheckedPlacedViewInput>,
}

/// One call site whose `requires` checking discharged in full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecidedCallRequires {
    pub state: symbols::SymbolHandle,
    pub statement_index: u32,
    pub target: symbols::SymbolHandle,
}

impl CheckFacts {
    pub fn byte_write_is_proven(&self, target: typed_trees::expression::ExpressionHandle) -> bool {
        self.proven_byte_writes
            .binary_search_by_key(&crate::operators::handle_position(target), |proven| {
                crate::operators::handle_position(*proven)
            })
            .is_ok()
    }

    /// Whether checking proved every `requires` fact of the call `state`
    /// makes to `target` at `statement_index`.
    pub fn call_requires_decided(
        &self,
        state: symbols::SymbolHandle,
        statement_index: u32,
        target: symbols::SymbolHandle,
    ) -> bool {
        self.decided_call_requires.iter().any(|decided| {
            decided.state == state
                && decided.statement_index == statement_index
                && decided.target == target
        })
    }

    /// Whether every bounds check of this `[]` occurrence proved its scalar
    /// index in range.
    pub fn scalar_index_is_proven(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        self.proven_scalar_indexes
            .binary_search_by_key(&crate::operators::handle_position(expression), |proven| {
                crate::operators::handle_position(*proven)
            })
            .is_ok()
    }
}
