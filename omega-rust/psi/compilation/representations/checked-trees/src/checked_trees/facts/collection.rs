use crate::{
    BlockingFacts, BorrowFacts, CarryFacts, CheckedOperatorFacts, CheckedValueFacts, DomainFacts,
    DynamicConformanceFacts, FlowFacts, IndexCompatibilityFacts, MachineContractPlans,
    MutationFacts, NominalMachineUseFacts, ProofFacts, QualificationFacts,
    RequirementCallSpecializationFacts, ServiceReachFacts, SuspensionFacts,
    SynchronousInvocationFacts, TerminationFacts,
};

use crate::{CheckedFactCallProjection, CheckedPlacedViewInput};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckFacts {
    pub quotient_formations: Vec<super::CheckedQuotientFormation>,
    pub quotient_operations: Vec<super::CheckedQuotientOperation>,
    /// Portable specialization identities derived from checked machine contracts.
    pub specialization_identities: crate::CheckedSpecializationIdentities,
    /// One identity row per admission-claim template machine, including
    /// unspecialized ones no specialization row could carry. Consumers read
    /// these instead of rederiving the canonical template commitment.
    pub accepted_template_identities: crate::CheckedAcceptedTemplateIdentities,
    /// Authored declaration choices finalized by checking. The typed ledger
    /// retains resolution data; checked consumers read this ledger exclusively.
    pub finalized_selections:
        language_semantics::declaration_selection::AuthoredDeclarationSelections,
    /// Checking-minted proof-view occurrences and their exact expression handles.
    /// The optional occurrence supplies the arena's empty row, not semantic absence.
    pub finalized_selection_occurrences: arena::Arena<(
        typed_trees::expression::ExpressionHandle,
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
    )>,
    /// Frozen executable place/result types and type-reference carriers.
    /// Lowering consumes these decisions instead of reconstructing them.
    pub resolved_types: crate::ResolvedTypeFacts,
    /// Completed source assignments proved to fit their physical integer encoding.
    pub stored_integer_writes: arena::Arena<crate::CheckedStoredIntegerWrite>,
    /// Source `[]` occurrences whose every bounds check proved a scalar index
    /// within its collection's live length, ordered by handle position.
    /// Lowering marks the read it emits for one of them, so that read's bound
    /// obligation consumes the decision instead of searching for a proof.
    pub proven_scalar_indexes: Vec<typed_trees::expression::ExpressionHandle>,
    /// Exact nonnegative coordinates agreed by every range check of an indexed
    /// occurrence. Evaluation and bounds remain on the executable selector.
    pub scalar_index_coordinates: Vec<(typed_trees::expression::ExpressionHandle, u64)>,
    /// Indexed byte assignments whose live bound and declared byte predicates
    /// were established at the store itself, ordered by target handle position.
    /// A later restoration of an invariant does not establish this decision.
    pub proven_byte_writes: Vec<typed_trees::expression::ExpressionHandle>,
    /// The `AssignmentIndex` selectors whose `[]` occurrence in their
    /// assignment target is in `proven_scalar_indexes`, keyed by the
    /// coordinate a plan path names (state, statement, depth) and ordered.
    /// Lowering reads a runtime element's decision here instead of
    /// rejoining the authored assignment target.
    pub proven_assignment_indexes: Vec<ProvenAssignmentIndex>,
    /// Calls whose `requires` facts checking proved or retained as ground
    /// constant obligations, keyed by calling state, statement and target.
    /// Lowering requires all constant obligations discharged before consuming
    /// these rows, including a state transfer's arrival contract.
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
    /// Every declared state's `let` locals, so lowering never scans a
    /// state's statements for them.
    pub state_locals: crate::StateLocalFacts,
    /// The closed boundary requirement each concrete provider refines through
    /// a conformance naming a generic top-level requirement template.
    pub provider_closed_requirements: crate::CheckedProviderClosedRequirements,
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
    /// Direct state inputs whose opaque placed-view meaning has been joined to
    /// its exact source-derived placement plan. These rows grant no runtime
    /// storage, accessor, provider, or ABI authority.
    pub placed_view_inputs: Vec<CheckedPlacedViewInput>,
    /// Comparisons the judgment proved against retained constant recipes,
    /// discharged by each target closure against its closed values.
    pub constant_recipe_obligations: Vec<crate::ConstantRecipeObligation>,
    /// Ground predicates whose target observations must close before execution.
    pub constant_predicate_obligations: Vec<super::ConstantPredicateObligation>,
    pub integer_range_obligations: Vec<super::IntegerRangeObligation>,
}

/// One `AssignmentIndex { depth }` selector of the assignment at
/// `statement_index` whose bounds checking proved its index in range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvenAssignmentIndex {
    pub state: symbols::SymbolHandle,
    pub statement_index: u32,
    pub depth: u32,
    /// A singleton evaluated coordinate, agreed across every checking context.
    /// Absence retains conservative overlap; it never changes executable paths.
    pub coordinate: Option<u64>,
}

impl ProvenAssignmentIndex {
    /// The order `proven_assignment_indexes` is kept in.
    pub fn key(&self) -> (u32, u32, u32, u32) {
        (
            self.state.arena_index(),
            self.state.generation(),
            self.statement_index,
            self.depth,
        )
    }
}

/// One call site's decision, conditional on its CheckFacts constant obligations
/// until target closure discharges them before lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecidedCallRequires {
    pub state: symbols::SymbolHandle,
    pub statement_index: u32,
    pub target: symbols::SymbolHandle,
}

impl CheckFacts {
    /// The finalized ledger never falls back to unresolved typed selections.
    pub fn authored_declaration_selections(
        &self,
    ) -> &language_semantics::declaration_selection::AuthoredDeclarationSelections {
        &self.finalized_selections
    }

    /// Retain typed occurrence identities and append checking-minted associations.
    /// Targets for both kinds of occurrence belong to the finalized ledger.
    pub fn authored_selection_occurrences<'input>(
        &'input self,
        program: &'input typed_trees::TypedTrees,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> impl Iterator<
        Item = language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
    > + 'input {
        program
            .expression_table
            .authored_selection_occurrences(expression)
            .chain(self.finalized_selection_occurrences.iter().filter_map(
                move |(_, (owner, occurrence))| {
                    (*owner == expression).then_some(*occurrence).flatten()
                },
            ))
    }

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

    /// Whether the `AssignmentIndex { depth }` selector of the assignment at
    /// `statement_index` in `state` is proven in range.
    pub fn assignment_index_is_proven(
        &self,
        state: symbols::SymbolHandle,
        statement_index: u32,
        depth: u32,
    ) -> bool {
        let key = ProvenAssignmentIndex {
            state,
            statement_index,
            depth,
            coordinate: None,
        }
        .key();
        self.proven_assignment_indexes
            .binary_search_by_key(&key, ProvenAssignmentIndex::key)
            .is_ok()
    }

    /// The proven singleton of this exact assignment selector, if every
    /// admission context agrees. This is not permission to skip its evaluation.
    pub fn assignment_index_coordinate(
        &self,
        state: symbols::SymbolHandle,
        statement_index: u32,
        depth: u32,
    ) -> Option<u64> {
        let key = ProvenAssignmentIndex {
            state,
            statement_index,
            depth,
            coordinate: None,
        }
        .key();
        let position = self
            .proven_assignment_indexes
            .binary_search_by_key(&key, ProvenAssignmentIndex::key)
            .ok()?;
        self.proven_assignment_indexes[position].coordinate
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
