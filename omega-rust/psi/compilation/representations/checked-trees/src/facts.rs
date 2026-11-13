/*
We assemble `CheckFacts` (`collection`) and the declaration, call, carry,
content-projection, domain, dynamic-conformance, qualification, service-reach,
and state-local fact families in `CheckedTrees`.
*/

pub(crate) mod collection {
    use crate::{
        BlockingFacts, BorrowFacts, CarryFacts, CheckedOperatorFacts, CheckedValueFacts,
        DomainFacts, DynamicConformanceFacts, FlowFacts, IndexCompatibilityFacts,
        MachineContractPlans, MutationFacts, NominalMachineUseFacts, ProofFacts,
        QualificationFacts, RequirementCallSpecializationFacts, ServiceReachFacts, SuspensionFacts,
        SynchronousInvocationFacts, TerminationFacts,
    };

    use crate::operators::handle_position;
    use crate::{CheckedAccessViewInput, CheckedFactCallProjection};
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId;
    use language_semantics::declaration_selection::AuthoredDeclarationSelections;
    use typed_trees::expression::ExpressionHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckFacts {
        pub callable_contracts: crate::CheckedCallableContracts,
        pub declaration_descriptions: crate::CheckedDeclarationDescriptions,
        pub proof_applications: crate::ProofApplicationPlan,
        pub deferred_layout_applications: Vec<typed_trees::types::TypeReferenceHandle>,
        pub quotient_formations: Vec<super::CheckedQuotientFormation>,
        pub quotient_operations: Vec<super::CheckedQuotientOperation>,
        pub specialization_identities: crate::CheckedSpecializationIdentities,
        pub accepted_template_identities: crate::CheckedAcceptedTemplateIdentities,
        pub finalized_selections: AuthoredDeclarationSelections,
        pub finalized_selection_occurrences: arena::Arena<(
            ExpressionHandle,
            Option<AuthoredDeclarationSelectionOccurrenceId>,
        )>,
        pub resolved_types: crate::ResolvedTypeFacts,
        pub executable_bindings: crate::CheckedExecutableBindings,
        pub stored_integer_writes: arena::Arena<crate::CheckedStoredIntegerWrite>,
        pub proven_scalar_indexes: Vec<ExpressionHandle>,
        pub scalar_index_coordinates: Vec<(ExpressionHandle, u64)>,
        pub scalar_selector_coordinates: Vec<(ExpressionHandle, u64)>,
        pub proven_byte_writes: Vec<ExpressionHandle>,
        pub proven_assignment_indexes: Vec<ProvenAssignmentIndex>,
        pub decided_call_requires: Vec<DecidedCallRequires>,
        pub operational: flow_effects::OperationalPlan,
        pub semantic: facts::FactPlan,
        pub borrow: BorrowFacts,
        pub reference_custody: crate::ReferenceCustodyFacts,
        pub state_locals: crate::StateLocalFacts,
        pub provider_closed_requirements: crate::CheckedProviderClosedRequirements,
        pub provider_instance_publication: crate::CheckedProviderInstancePublication,
        pub content_classes: crate::ContentClassFacts,
        pub contextual_cleanup: crate::ContextualCleanupFacts,
        pub proof: ProofFacts,
        pub values: CheckedValueFacts,
        pub domains: DomainFacts,
        pub dynamic_conformances: DynamicConformanceFacts,
        pub nominal_machine_uses: NominalMachineUseFacts,
        pub installed_interrupt_registrations: Vec<crate::CheckedInstalledInterruptRegistration>,
        pub requirement_call_specializations: RequirementCallSpecializationFacts,
        pub operators: CheckedOperatorFacts,
        pub capabilities: flow_effects::CapabilityFlowPlan,
        pub flow: FlowFacts,
        pub index_compatibility: IndexCompatibilityFacts,
        pub mutation: MutationFacts,
        pub service_reaches: ServiceReachFacts,
        pub synchronous_invocations: SynchronousInvocationFacts,
        pub suspensions: SuspensionFacts,
        pub blocking: BlockingFacts,
        pub termination: TerminationFacts,
        pub qualifications: QualificationFacts,
        pub contract_plans: MachineContractPlans,
        pub carry: CarryFacts,
        pub fact_call_projections: Vec<CheckedFactCallProjection>,
        pub access_view_inputs: Vec<CheckedAccessViewInput>,
        pub constant_recipe_obligations: Vec<crate::ConstantRecipeObligation>,
        pub constant_predicate_obligations: Vec<super::ConstantPredicateObligation>,
        pub integer_range_obligations: Vec<super::IntegerRangeObligation>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProvenAssignmentIndex {
        pub state: symbols::SymbolHandle,
        pub statement_index: u32,
        pub depth: u32,
        pub coordinate: Option<u64>,
    }

    impl ProvenAssignmentIndex {
        pub fn key(&self) -> (u32, u32, u32, u32) {
            (
                self.state.arena_index(),
                self.state.generation(),
                self.statement_index,
                self.depth,
            )
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct DecidedCallRequires {
        pub state: symbols::SymbolHandle,
        pub statement_index: u32,
        pub target: symbols::SymbolHandle,
    }

    impl CheckFacts {
        pub fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
            &self.finalized_selections
        }

        pub(crate) fn contract_fact_refs(
            &self,
            refs: arena::HandleSpan<crate::ContractProofFactRef>,
        ) -> &[crate::ContractProofFactRef] {
            self.proof.contract_fact_refs.span_or_empty(refs)
        }

        pub fn authored_selection_occurrences<'input>(
            &'input self,
            program: &'input typed_trees::TypedTrees,
            expression: ExpressionHandle,
        ) -> impl Iterator<Item = AuthoredDeclarationSelectionOccurrenceId> + 'input {
            program
                .expression_table
                .authored_selection_occurrences(expression)
                .chain(self.finalized_selection_occurrences.iter().filter_map(
                    move |(_, (owner, occurrence))| {
                        (*owner == expression).then_some(*occurrence).flatten()
                    },
                ))
        }

        pub fn byte_write_is_proven(&self, target: ExpressionHandle) -> bool {
            self.proven_byte_writes
                .binary_search_by_key(&handle_position(target), |proven| handle_position(*proven))
                .is_ok()
        }

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

        pub fn selector_coordinate(&self, selector: ExpressionHandle) -> Option<u64> {
            let position = self
                .scalar_selector_coordinates
                .binary_search_by_key(&handle_position(selector), |(expression, _)| {
                    handle_position(*expression)
                })
                .ok()?;
            Some(self.scalar_selector_coordinates[position].1)
        }

        pub fn scalar_index_is_proven(&self, expression: ExpressionHandle) -> bool {
            self.proven_scalar_indexes
                .binary_search_by_key(&handle_position(expression), |proven| {
                    handle_position(*proven)
                })
                .is_ok()
        }
    }
}
pub(crate) use crate::contract_and_obligation_facts::constant_predicate_obligations;
pub(crate) use crate::contract_and_obligation_facts::constant_recipe_obligations;
pub(crate) use crate::contract_and_obligation_facts::integer_range_obligations;
pub(crate) use crate::termination_and_cleanup_facts::contextual_cleanup;
pub use constant_predicate_obligations::ConstantPredicateObligation;
pub use integer_range_obligations::{
    IntegerRangeModel, IntegerRangeObligation, IntegerRangeRecipe,
};
pub mod scalar_predicates {
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
    };

    pub use numerics::scalar_predicates::ScalarValue;
    pub(crate) use numerics::scalar_predicates::{
        PredicateNode, PredicateOperator, evaluate_nodes,
    };

    pub fn closed_boolean_value(
        program: &typed_trees::TypedTrees,
        operators: &crate::CheckedOperatorFacts,
        expression: ExpressionHandle,
    ) -> Option<bool> {
        if !has_builtin_operators(program, operators, expression) {
            return None;
        }
        let ScalarValue::Boolean(value) =
            evaluate(program, expression, &mut |_| None, &|_, _| true)?
        else {
            return None;
        };
        Some(value)
    }

    pub fn has_builtin_operators(
        program: &typed_trees::TypedTrees,
        operators: &crate::CheckedOperatorFacts,
        expression: ExpressionHandle,
    ) -> bool {
        if operators.uses.iter().any(|(_, operator_use)| {
            operator_use.expression == expression
                && operator_use.status != crate::CheckedOperatorResolutionStatus::BuiltinFallback
        }) {
            return false;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                has_builtin_operators(program, operators, binary.left)
                    && has_builtin_operators(program, operators, binary.right)
            }
            ExpressionNode::Unary(unary) => {
                has_builtin_operators(program, operators, unary.operand)
            }
            ExpressionNode::Borrow(borrow) => {
                has_builtin_operators(program, operators, borrow.target)
            }
            _ => true,
        }
    }

    pub fn evaluate(
        program: &typed_trees::TypedTrees,
        expression: ExpressionHandle,
        resolve_leaf: &mut impl FnMut(ExpressionHandle) -> Option<ScalarValue>,
        comparison_is_admitted: &impl Fn(&ScalarValue, &ScalarValue) -> bool,
    ) -> Option<ScalarValue> {
        evaluate_for_atoms(
            program,
            expression,
            resolve_leaf,
            comparison_is_admitted,
            &mut |_| None,
        )
    }

    pub fn evaluate_for_atoms(
        program: &typed_trees::TypedTrees,
        expression: ExpressionHandle,
        resolve_leaf: &mut impl FnMut(ExpressionHandle) -> Option<ScalarValue>,
        comparison_is_admitted: &impl Fn(&ScalarValue, &ScalarValue) -> bool,
        observe_boolean: &mut impl FnMut(ExpressionHandle) -> Option<bool>,
    ) -> Option<ScalarValue> {
        evaluate_nodes(
            expression,
            &mut |expression| source_node(program, expression, observe_boolean),
            resolve_leaf,
            comparison_is_admitted,
        )
    }

    pub(crate) fn source_node(
        program: &typed_trees::TypedTrees,
        expression: ExpressionHandle,
        observe_boolean: &mut impl FnMut(ExpressionHandle) -> Option<bool>,
    ) -> Option<PredicateNode<ExpressionHandle, ExpressionHandle>> {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        if let Some(value) = program.static_boolean_value(expression) {
            return Some(PredicateNode::Value(ScalarValue::Boolean(value)));
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(value) => {
                return value
                    .value_bignum()
                    .map(|value| PredicateNode::Value(ScalarValue::Integer(value)));
            }
            ExpressionNode::Boolean(value) => {
                return Some(PredicateNode::Value(ScalarValue::Boolean(*value)));
            }
            _ => {}
        }
        if let Some(value) = observe_boolean(expression) {
            return Some(PredicateNode::Value(ScalarValue::Boolean(value)));
        }
        Some(match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => PredicateNode::Forward(borrow.target),
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                PredicateNode::Not(unary.operand)
            }
            ExpressionNode::Binary(binary) if predicate_operator(binary.operator).is_some() => {
                PredicateNode::Binary {
                    operator: predicate_operator(binary.operator)?,
                    left: binary.left,
                    right: binary.right,
                }
            }
            _ => PredicateNode::Leaf(expression),
        })
    }

    fn predicate_operator(operator: BinaryOperator) -> Option<PredicateOperator> {
        Some(match operator {
            BinaryOperator::And => PredicateOperator::And,
            BinaryOperator::Or => PredicateOperator::Or,
            BinaryOperator::Equal => PredicateOperator::Equal,
            BinaryOperator::NotEqual => PredicateOperator::NotEqual,
            BinaryOperator::Less => PredicateOperator::Less,
            BinaryOperator::LessOrEqual => PredicateOperator::LessOrEqual,
            BinaryOperator::Greater => PredicateOperator::Greater,
            BinaryOperator::GreaterOrEqual => PredicateOperator::GreaterOrEqual,
            _ => return None,
        })
    }
}

pub(crate) mod blocking {
    use language_semantics::BlockingPlan;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct BlockingFacts {
        pub machines: Vec<MachineBlockingFact>,
    }

    impl BlockingFacts {
        pub fn for_machine(&self, machine: SymbolHandle) -> Option<BlockingPlan> {
            self.machines
                .iter()
                .find(|fact| fact.machine == machine)
                .map(|fact| fact.plan)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineBlockingFact {
        pub machine: SymbolHandle,
        pub plan: BlockingPlan,
    }
}
pub(crate) mod checked_crash_operator_site {
    use arena::Handle;
    use symbols::SymbolHandle;

    use crate::{
        CheckedNamedOperatorUseFact, CheckedOperatorUseFact, CheckedValueOrigin, CrashRouteBucket,
        FlowOperatorInvocationFact,
    };

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCrashOperatorSite {
        pub operator_use: Handle<CheckedOperatorUseFact>,
        pub invocation: Handle<FlowOperatorInvocationFact>,
        pub named_use: Handle<CheckedNamedOperatorUseFact>,
        pub selected_operator: SymbolHandle,
        pub published: Vec<CrashRouteBucket>,
        pub surviving: Vec<CrashRouteBucket>,
    }

    impl CheckedCrashOperatorSite {
        pub fn location(&self, facts: &crate::CheckFacts) -> OperatorSiteLocation {
            let origin = if self.named_use.is_valid() {
                facts.operators.named_uses.get(self.named_use).origin
            } else {
                facts.operators.uses.get(self.operator_use).origin
            };
            let (_machine_symbol, state_symbol, statement_index) = match origin {
                CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    ..
                } => (machine_symbol, state_symbol, statement_index),
                _ => unreachable!("operator crash site must have StateStatement origin"),
            };
            OperatorSiteLocation {
                state: state_symbol,
                statement_ordinal: u32::try_from(statement_index)
                    .expect("statement ordinal fits u32"),
            }
        }

        pub fn expression(
            &self,
            facts: &crate::CheckFacts,
        ) -> typed_trees::expression::ExpressionHandle {
            if self.named_use.is_valid() {
                facts.operators.named_uses.get(self.named_use).expression
            } else {
                facts.operators.uses.get(self.operator_use).expression
            }
        }

        pub fn selected_operator(&self) -> SymbolHandle {
            self.selected_operator
        }

        pub fn published(&self) -> &[CrashRouteBucket] {
            &self.published
        }

        pub fn surviving(&self) -> &[CrashRouteBucket] {
            &self.surviving
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OperatorSiteLocation {
        pub state: SymbolHandle,
        pub statement_ordinal: u32,
    }

    impl std::cmp::Ord for OperatorSiteLocation {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            (
                self.state.arena_index(),
                self.state.generation(),
                self.statement_ordinal,
            )
                .cmp(&(
                    other.state.arena_index(),
                    other.state.generation(),
                    other.statement_ordinal,
                ))
        }
    }

    impl std::cmp::PartialOrd for OperatorSiteLocation {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl OperatorSiteLocation {
        pub fn state(&self) -> SymbolHandle {
            self.state
        }

        pub fn statement_ordinal(&self) -> u32 {
            self.statement_ordinal
        }
    }
}
pub(crate) mod content {
    use arena::HandleSpan;
    use language_semantics::content::{ContentConservationPlan, ContentProjectionPlan};
    use language_semantics::{PermissionClaimIdentity, ReferenceAccess, SemanticDomainId};
    use symbols::SymbolHandle;
    use typed_trees::name::Identifier;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentProjectionFacts {
        pub plans: Vec<ContentProjectionPlan>,
        pub conservation_plans: Vec<ContentConservationPlan>,
        pub identity_reshuffles: Vec<ContentIdentityReshuffleFact>,
        pub partition_compositions: Vec<ContentPartitionCompositionFact>,
        pub body_conservations: arena::Arena<ContentBodyConservationFact>,
        /// Exact equations in each completed body's parameter namespace. Source
        /// authorization remains on the separate authored plan table above.
        pub body_conservation_plans: Vec<ContentConservationPlan>,
        pub body_claim_bindings: arena::Arena<ContentBodyClaimBinding>,
        pub body_available_inputs: arena::Arena<ContentBodyAvailableInput>,
        pub body_transformations: arena::Arena<ContentBodyTransformation>,
        pub body_derivations: arena::Arena<ContentBodyDerivation>,
        pub boundary_requirement_plans: Vec<ContentConservationPlan>,
        pub boundary_body_requirements: arena::Arena<ContentBoundaryBodyRequirement>,
        pub backing_addresses: arena::Arena<ContentBackingAddressTransport>,
        pub partition_returns: Vec<ContentPartitionReturnFact>,
        pub claim_transformations: arena::Arena<ContentClaimTransformation>,
        pub retained_borrow_custodies: Vec<RetainedBorrowCustodyFact>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentClaimTransformation {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub statement_index: usize,
        pub expression: typed_trees::expression::ExpressionHandle,
        pub evaluation: crate::FlowPermissionEvaluation,
        pub source: ContentClaimTransformationSource,
        pub inputs: HandleSpan<ContentBodyClaimBinding>,
        pub outputs: HandleSpan<ContentBodyClaimBinding>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ContentClaimTransformationSource {
        #[default]
        Unknown,
        Call {
            call_ordinal: usize,
            plan_index: usize,
        },
        Body {
            conservation: arena::Handle<ContentBodyConservationFact>,
        },
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentBodyTransformation {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub completed_value: arena::Handle<crate::FlowOperandFact>,
        pub evaluation: crate::FlowPermissionEvaluation,
        pub derivations: HandleSpan<ContentBodyDerivation>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentBodyDerivation {
        pub conservation: arena::Handle<ContentBodyConservationFact>,
        pub lineage_roots: HandleSpan<ContentBodyClaimBinding>,
        pub custody: ContentBodyCustody,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ContentBodyCustody {
        #[default]
        Derived,
        Boundary {
            requirements: HandleSpan<ContentBoundaryBodyRequirement>,
            addresses: HandleSpan<ContentBackingAddressTransport>,
        },
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentBoundaryBodyRequirement {
        pub declaring_trait: SymbolHandle,
        pub requirement: SymbolHandle,
        pub realization_state: SymbolHandle,
        pub conservation_plan_index: usize,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentBackingAddressTransport {
        pub input: arena::Handle<ContentBodyAvailableInput>,
        pub output: arena::Handle<ContentBodyClaimBinding>,
        pub input_segments: HandleSpan<facts::PlaceSegment>,
        pub output_segments: HandleSpan<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentBodyConservationFact {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub returned_at: arena::Handle<crate::FlowStateOutgoingFact>,
        pub completed_value: arena::Handle<crate::FlowOperandFact>,
        pub evaluation: crate::FlowPermissionEvaluation,
        /// Original authored demand and authorization in `conservation_plans`.
        /// This source may belong to a terminal caller, so its parameter positions
        /// are not the completed body's coordinates.
        pub plan_index: usize,
        /// Exact proved equation and completion frame in `body_conservation_plans`.
        pub body_plan_index: usize,
        pub inputs: HandleSpan<ContentBodyClaimBinding>,
        pub outputs: HandleSpan<ContentBodyClaimBinding>,
        pub available_inputs: HandleSpan<ContentBodyAvailableInput>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentBodyAvailableInput {
        pub binding: arena::Handle<ContentBodyClaimBinding>,
        pub entry_bindings: HandleSpan<ContentBodyClaimBinding>,
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<facts::PlaceSegment>,
        pub provenance: language_semantics::PermissionProvenance,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentBodyClaimBinding {
        pub claim_identity: PermissionClaimIdentity,
        pub domain: SymbolHandle,
        pub semantic_domain: SemanticDomainId,
        pub projection_machine: SymbolHandle,
        pub parameter_symbol: SymbolHandle,
        pub segments: HandleSpan<facts::PlaceSegment>,
        pub observation: arena::Handle<crate::FlowOperandFact>,
    }

    pub struct ContentClaimFrontier {
        pub bindings: Vec<ContentFrontierBinding>,
        pub replacements: Vec<(
            arena::Handle<ContentClaimTransformation>,
            arena::Handle<ContentBodyClaimBinding>,
            arena::Handle<ContentBodyClaimBinding>,
        )>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ContentFrontierBinding {
        pub binding: arena::Handle<ContentBodyClaimBinding>,
        pub entries: Vec<arena::Handle<ContentBodyClaimBinding>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct RetainedBorrowCustodyFact {
        pub callable: SymbolHandle,
        pub source: language_semantics::content::ContentStructuralPlace,
        pub result: language_semantics::content::ContentStructuralPlace,
        pub access: ReferenceAccess,
        pub lifetime: Identifier,
        pub callable_lifetime_parameter_ordinal: u32,
        pub result_data: SymbolHandle,
        pub result_lifetime_argument_ordinal: u32,
        pub retained_semantic_domain: SemanticDomainId,
        pub source_projection: ContentProjectionPlan,
        pub result_projection: ContentProjectionPlan,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentIdentityReshuffleFact {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub claim_identity: PermissionClaimIdentity,
        pub input_parameter_symbol: SymbolHandle,
        pub input_segments: HandleSpan<facts::PlaceSegment>,
        pub output_segments: HandleSpan<facts::PlaceSegment>,
        pub plan: ContentConservationPlan,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentPartitionReturnFact {
        pub returned_at: arena::Handle<crate::FlowStateOutgoingFact>,
        pub plan: ContentConservationPlan,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentPartitionCompositionFact {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub source_callable: SymbolHandle,
        pub source_report_fingerprint: u64,
        pub source_derivation_depth: u32,
        pub source_plan: ContentConservationPlan,
        pub statement_index: usize,
        pub call_ordinal: usize,
        pub returned_at: arena::Handle<crate::FlowStateOutgoingFact>,
        pub input_claim_identities: Vec<PermissionClaimIdentity>,
        pub input_claim_bindings: Vec<ContentPartitionInputClaimBinding>,
        pub result_rewrites: Vec<ContentPartitionResultRewrite>,
        pub substitutions: Vec<ContentPartitionPlaceSubstitution>,
        pub plan: ContentConservationPlan,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentPartitionInputClaimBinding {
        pub claim_identity: PermissionClaimIdentity,
        pub entry_place: language_semantics::content::ContentStructuralPlace,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentPartitionResultRewrite {
        pub claim_identity: PermissionClaimIdentity,
        pub source: language_semantics::content::ContentStructuralPlace,
        pub target: language_semantics::content::ContentStructuralPlace,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentPartitionPlaceSubstitution {
        pub source: language_semantics::content::ContentStructuralPlace,
        pub target: language_semantics::content::ContentStructuralPlace,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ContentWalkRows {
        pub body_conservations: arena::Arena<ContentBodyConservationFact>,
        pub body_conservation_plans: Vec<ContentConservationPlan>,
        pub body_claim_bindings: arena::Arena<ContentBodyClaimBinding>,
        pub body_available_inputs: arena::Arena<ContentBodyAvailableInput>,
        pub body_transformations: arena::Arena<ContentBodyTransformation>,
        pub body_derivations: arena::Arena<ContentBodyDerivation>,
        pub boundary_requirement_plans: Vec<ContentConservationPlan>,
        pub boundary_body_requirements: arena::Arena<ContentBoundaryBodyRequirement>,
        pub backing_addresses: arena::Arena<ContentBackingAddressTransport>,
        pub claim_transformations: arena::Arena<ContentClaimTransformation>,
    }

    impl ContentProjectionFacts {
        pub fn descendant_frontier(
            &self,
            entries: HandleSpan<ContentBodyClaimBinding>,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement: usize,
            evaluation: crate::FlowPermissionEvaluation,
            before: arena::Handle<ContentBodyConservationFact>,
        ) -> Result<ContentClaimFrontier, &'static str> {
            self.view()
                .descendant_frontier(entries, machine, state, statement, evaluation, before)
        }

        pub fn take_walk_rows(&mut self) -> ContentWalkRows {
            ContentWalkRows {
                body_conservations: std::mem::take(&mut self.body_conservations),
                body_conservation_plans: std::mem::take(&mut self.body_conservation_plans),
                body_claim_bindings: std::mem::take(&mut self.body_claim_bindings),
                body_available_inputs: std::mem::take(&mut self.body_available_inputs),
                body_transformations: std::mem::take(&mut self.body_transformations),
                body_derivations: std::mem::take(&mut self.body_derivations),
                boundary_requirement_plans: std::mem::take(&mut self.boundary_requirement_plans),
                boundary_body_requirements: std::mem::take(&mut self.boundary_body_requirements),
                backing_addresses: std::mem::take(&mut self.backing_addresses),
                claim_transformations: std::mem::take(&mut self.claim_transformations),
            }
        }

        /// Append rows produced by an independent state walk. Adjust their
        /// body and boundary plan indexes, handles and spans for the destination
        /// tables. Authored plan positions, ownership place segments and flow
        /// handles stay unchanged. Return false without changing the destination
        /// when an index cannot be shifted exactly.
        pub fn append_walk_rows(&mut self, rows: ContentWalkRows) -> bool {
            use crate::shifted_rows::{append_rows, shift, shift_span, shiftable};
            let ContentWalkRows {
                body_conservations,
                body_conservation_plans,
                body_claim_bindings,
                body_available_inputs,
                body_transformations,
                body_derivations,
                boundary_requirement_plans,
                boundary_body_requirements,
                backing_addresses,
                claim_transformations,
            } = rows;
            let (
                Some(conservation_rows),
                Some(binding_rows),
                Some(input_rows),
                Some(_),
                Some(derivation_rows),
                Some(requirement_rows),
                Some(address_rows),
                Some(_),
            ) = (
                shiftable(&self.body_conservations, &body_conservations),
                shiftable(&self.body_claim_bindings, &body_claim_bindings),
                shiftable(&self.body_available_inputs, &body_available_inputs),
                shiftable(&self.body_transformations, &body_transformations),
                shiftable(&self.body_derivations, &body_derivations),
                shiftable(
                    &self.boundary_body_requirements,
                    &boundary_body_requirements,
                ),
                shiftable(&self.backing_addresses, &backing_addresses),
                shiftable(&self.claim_transformations, &claim_transformations),
            )
            else {
                return false;
            };
            let plan_rows = self.boundary_requirement_plans.len();
            let body_plan_rows = self.body_conservation_plans.len();
            if body_plan_rows
                .checked_add(body_conservation_plans.len())
                .is_none()
                || body_conservations
                    .iter()
                    .any(|(_, row)| row.body_plan_index >= body_conservation_plans.len())
            {
                return false;
            }
            append_rows(&mut self.body_conservations, body_conservations, |row| {
                let ContentBodyConservationFact {
                    machine_symbol,
                    state_symbol,
                    returned_at,
                    completed_value,
                    evaluation,
                    plan_index,
                    body_plan_index,
                    inputs,
                    outputs,
                    available_inputs,
                } = row;
                ContentBodyConservationFact {
                    machine_symbol,
                    state_symbol,
                    returned_at,
                    completed_value,
                    evaluation,
                    plan_index,
                    body_plan_index: body_plan_index + body_plan_rows,
                    inputs: shift_span(inputs, binding_rows),
                    outputs: shift_span(outputs, binding_rows),
                    available_inputs: shift_span(available_inputs, input_rows),
                }
            });
            self.body_conservation_plans.extend(body_conservation_plans);
            append_rows(&mut self.body_claim_bindings, body_claim_bindings, |row| {
                row
            });
            append_rows(
                &mut self.body_available_inputs,
                body_available_inputs,
                |row| {
                    let ContentBodyAvailableInput {
                        binding,
                        entry_bindings,
                        root,
                        segments,
                        provenance,
                    } = row;
                    ContentBodyAvailableInput {
                        binding: shift(binding, binding_rows),
                        entry_bindings: shift_span(entry_bindings, binding_rows),
                        root,
                        segments,
                        provenance,
                    }
                },
            );
            append_rows(
                &mut self.body_transformations,
                body_transformations,
                |row| {
                    let ContentBodyTransformation {
                        machine_symbol,
                        state_symbol,
                        completed_value,
                        evaluation,
                        derivations,
                    } = row;
                    ContentBodyTransformation {
                        machine_symbol,
                        state_symbol,
                        completed_value,
                        evaluation,
                        derivations: shift_span(derivations, derivation_rows),
                    }
                },
            );
            append_rows(&mut self.body_derivations, body_derivations, |row| {
                let ContentBodyDerivation {
                    conservation,
                    lineage_roots,
                    custody,
                } = row;
                ContentBodyDerivation {
                    conservation: shift(conservation, conservation_rows),
                    lineage_roots: shift_span(lineage_roots, binding_rows),
                    custody: match custody {
                        ContentBodyCustody::Derived => ContentBodyCustody::Derived,
                        ContentBodyCustody::Boundary {
                            requirements,
                            addresses,
                        } => ContentBodyCustody::Boundary {
                            requirements: shift_span(requirements, requirement_rows),
                            addresses: shift_span(addresses, address_rows),
                        },
                    },
                }
            });
            self.boundary_requirement_plans
                .extend(boundary_requirement_plans);
            append_rows(
                &mut self.boundary_body_requirements,
                boundary_body_requirements,
                |row| {
                    let ContentBoundaryBodyRequirement {
                        declaring_trait,
                        requirement,
                        realization_state,
                        conservation_plan_index,
                    } = row;
                    ContentBoundaryBodyRequirement {
                        declaring_trait,
                        requirement,
                        realization_state,
                        conservation_plan_index: conservation_plan_index + plan_rows,
                    }
                },
            );
            append_rows(&mut self.backing_addresses, backing_addresses, |row| {
                let ContentBackingAddressTransport {
                    input,
                    output,
                    input_segments,
                    output_segments,
                } = row;
                ContentBackingAddressTransport {
                    input: shift(input, input_rows),
                    output: shift(output, binding_rows),
                    input_segments,
                    output_segments,
                }
            });
            append_rows(
                &mut self.claim_transformations,
                claim_transformations,
                |row| {
                    let ContentClaimTransformation {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        expression,
                        evaluation,
                        source,
                        inputs,
                        outputs,
                    } = row;
                    ContentClaimTransformation {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        expression,
                        evaluation,
                        source: match source {
                            ContentClaimTransformationSource::Unknown => {
                                ContentClaimTransformationSource::Unknown
                            }
                            ContentClaimTransformationSource::Call {
                                call_ordinal,
                                plan_index,
                            } => ContentClaimTransformationSource::Call {
                                call_ordinal,
                                plan_index,
                            },
                            ContentClaimTransformationSource::Body { conservation } => {
                                ContentClaimTransformationSource::Body {
                                    conservation: shift(conservation, conservation_rows),
                                }
                            }
                        },
                        inputs: shift_span(inputs, binding_rows),
                        outputs: shift_span(outputs, binding_rows),
                    }
                },
            );
            true
        }
    }

    impl<'f> crate::ContentProjectionFactsRef<'f> {
        pub fn descendant_frontier(
            self,
            entries: HandleSpan<ContentBodyClaimBinding>,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement: usize,
            evaluation: crate::FlowPermissionEvaluation,
            before: arena::Handle<ContentBodyConservationFact>,
        ) -> Result<ContentClaimFrontier, &'static str> {
            let same = |left: arena::Handle<ContentBodyClaimBinding>,
                        right: arena::Handle<ContentBodyClaimBinding>| {
                let left = self.body_claim_bindings.get(left);
                let right = self.body_claim_bindings.get(right);
                left.claim_identity == right.claim_identity
                    && left.domain == right.domain
                    && left.semantic_domain == right.semantic_domain
                    && left.projection_machine == right.projection_machine
            };
            let mut frontier = entries
                .handles()
                .map(|entry| ContentFrontierBinding {
                    binding: entry,
                    entries: vec![entry],
                })
                .collect::<Vec<_>>();
            if frontier.is_empty()
                || frontier.iter().enumerate().any(|(position, claim)| {
                    frontier[..position]
                        .iter()
                        .any(|previous| same(previous.binding, claim.binding))
                })
            {
                return Err("content frontier has no complete unique input roster");
            }
            let mut replacements = Vec::new();
            for (handle, transformation) in self.claim_transformations.iter() {
                if before.is_valid()
                    && transformation.source
                        == (ContentClaimTransformationSource::Body {
                            conservation: before,
                        })
                {
                    break;
                }
                if transformation.machine_symbol != machine
                    || transformation.state_symbol != state
                    || transformation.statement_index > statement
                    || (transformation.statement_index == statement
                        && transformation.evaluation != evaluation)
                    || transformation.source == ContentClaimTransformationSource::Unknown
                {
                    continue;
                }
                let parents = transformation.inputs.handles().collect::<Vec<_>>();
                if !parents
                    .iter()
                    .any(|parent| frontier.iter().any(|claim| same(*parent, claim.binding)))
                {
                    continue;
                }
                if parents.is_empty()
                    || transformation.outputs.is_empty()
                    || !parents
                        .iter()
                        .all(|parent| frontier.iter().any(|claim| same(*parent, claim.binding)))
                {
                    return Err(
                        "a completed transformation consumes an input outside the joint frontier",
                    );
                }
                let mut origins = Vec::new();
                for parent in &parents {
                    let expected = frontier
                        .iter()
                        .find(|claim| same(*parent, claim.binding))
                        .ok_or("replacement parent is absent")?;
                    replacements.push((handle, expected.binding, *parent));
                    for origin in &expected.entries {
                        if !origins.contains(origin) {
                            origins.push(*origin);
                        }
                    }
                }
                frontier.retain(|claim| !parents.iter().any(|parent| same(claim.binding, *parent)));
                for output in transformation.outputs.handles() {
                    if frontier.iter().any(|claim| same(claim.binding, output)) {
                        return Err("a completed transformation duplicates a qualified claim");
                    }
                    frontier.push(ContentFrontierBinding {
                        binding: output,
                        entries: origins.clone(),
                    });
                }
            }
            Ok(ContentClaimFrontier {
                bindings: frontier,
                replacements,
            })
        }
    }

    impl ContentProjectionFacts {
        pub fn publishes_formation_membership(
            &self,
            flow: &crate::FlowFacts,
            state: SymbolHandle,
            statement: usize,
            expression: typed_trees::expression::ExpressionHandle,
            domain: SymbolHandle,
            semantic_domain: SemanticDomainId,
        ) -> bool {
            let at_path = |mut value: arena::Handle<crate::FlowOperandFact>,
                           mut path: &[facts::PlaceSegment]| {
                while !path.is_empty() {
                    let capture = flow.control.operands.get(value);
                    let mut children = flow
                        .control
                        .operand_children
                        .span_or_empty(capture.constructed_children())
                        .iter()
                        .filter_map(|child| {
                            let destination =
                                flow.control.operand_child_paths.span_or_empty(child.path);
                            (child.parent == capture.expression
                                && !destination.is_empty()
                                && path.starts_with(destination))
                            .then_some((child.value, destination.len()))
                        });
                    let (selected, length) = children.next()?;
                    if children.next().is_some() || !flow.control.operands.is_valid(selected) {
                        return None;
                    }
                    value = selected;
                    path = &path[length..];
                }
                Some(value)
            };
            let mut matching = self.body_transformations.iter().filter(|(_, transaction)| {
                transaction.state_symbol == state
                    && self
                        .body_derivations
                        .span_or_empty(transaction.derivations)
                        .iter()
                        .any(|derived| {
                            if !matches!(derived.custody, ContentBodyCustody::Derived) {
                                return false;
                            }
                            let equation = self.body_conservations.get(derived.conservation);
                            self.body_claim_bindings
                                .span_or_empty(equation.outputs)
                                .iter()
                                .any(|output| {
                                    if output.domain != domain
                                        || output.semantic_domain != semantic_domain
                                    {
                                        return false;
                                    }
                                    let path =
                                        flow.ownership.segments.span_or_empty(output.segments);
                                    let Some(selected) = at_path(transaction.completed_value, path)
                                    else {
                                        return false;
                                    };
                                    let capture = flow.control.operands.get(selected);
                                    capture.state_symbol == state
                                        && capture.statement_index == statement
                                        && capture.expression == expression
                                })
                        })
            });
            matching.next().is_some() && matching.next().is_none()
        }

        pub fn establishes_completed_membership(
            &self,
            flow: &crate::FlowFacts,
            value: arena::Handle<crate::FlowOperandFact>,
            path: &[facts::PlaceSegment],
            domain: SymbolHandle,
            semantic_domain: SemanticDomainId,
        ) -> bool {
            self.body_transformations.iter().any(|(_, transaction)| {
                transaction.completed_value == value
                    && self
                        .body_derivations
                        .span_or_empty(transaction.derivations)
                        .iter()
                        .any(|derived| {
                            if !matches!(derived.custody, ContentBodyCustody::Derived) {
                                return false;
                            }
                            let equation = self.body_conservations.get(derived.conservation);
                            self.body_claim_bindings
                                .span_or_empty(equation.outputs)
                                .iter()
                                .any(|output| {
                                    output.domain == domain
                                        && output.semantic_domain == semantic_domain
                                        && flow.ownership.segments.span_or_empty(output.segments)
                                            == path
                                })
                        })
            })
        }

        pub fn for_semantic_domain(
            &self,
            domain: SemanticDomainId,
        ) -> Option<&ContentProjectionPlan> {
            domain
                .is_valid()
                .then(|| {
                    self.plans
                        .iter()
                        .find(|plan| plan.semantic_domain == domain)
                })
                .flatten()
        }
    }
}
pub(crate) use crate::contract_and_obligation_facts::contract_plans;
pub(crate) mod declaration_descriptions {
    use arena::{Arena, HandleSpan};
    use symbols::SymbolHandle;
    use typed_trees::type_identity::{CallableIdentityId, TypeIdentityId};

    use super::MachineContractCommitment;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCallableHeader {
        pub machine: SymbolHandle,
        pub callable: CallableIdentityId,
        pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub source_target: Option<String>,
        pub product_eligible: bool,
        pub supplies_checked_body: bool,
        pub type_parameter_count: usize,
        pub compiler_evaluation:
            Result<semantic_vocabulary::compiler_evaluation::CompilerEvaluationDeclaration, String>,
        pub synchronous_invocations: Result<HandleSpan<CheckedSynchronousBoundaryTarget>, String>,
    }

    impl Default for CheckedCallableHeader {
        fn default() -> Self {
            Self {
                machine: SymbolHandle::default(),
                callable: CallableIdentityId::default(),
                package: None,
                source_target: None,
                product_eligible: false,
                supplies_checked_body: false,
                type_parameter_count: 0,
                compiler_evaluation: Err("checked compiler-evaluation facts are absent".into()),
                synchronous_invocations: Err(
                    "checked synchronous invocation facts are absent".into()
                ),
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedSynchronousBoundaryTarget {
        pub declaration: String,
        pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub application: Option<String>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedDeclarationParameter {
        pub normalized: TypeIdentityId,
        pub qualified: TypeIdentityId,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedDeclarationCapture {
        pub template_identity: TypeIdentityId,
        pub template_contract: [u8; 32],
        pub specialization: [u8; 32],
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedDeclarationSpecialization {
        pub template: SymbolHandle,
        pub callable: CallableIdentityId,
        pub commitment: [u8; 32],
        pub capture: Option<CheckedDeclarationCapture>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedMachineDescription {
        pub machine: SymbolHandle,
        pub entry: SymbolHandle,
        pub parameters: HandleSpan<CheckedDeclarationParameter>,
        pub result: TypeIdentityId,
        pub callable: CallableIdentityId,
        pub contract: MachineContractCommitment,
        pub specialization: Option<CheckedDeclarationSpecialization>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedDeclarationDescription {
        #[default]
        Missing,
        Machine(CheckedMachineDescription),
        Unavailable {
            machine: SymbolHandle,
            reason: String,
        },
    }

    impl CheckedDeclarationDescription {
        pub fn machine(&self) -> SymbolHandle {
            match self {
                Self::Missing => SymbolHandle::default(),
                Self::Machine(description) => description.machine,
                Self::Unavailable { machine, .. } => *machine,
            }
        }
    }

    #[derive(Debug, Clone, compiler_storage::Record)]
    pub struct CheckedDeclarationDescriptions {
        pub selections: Arena<CheckedSelectionRequirement>,
        pub evaluation_dependencies: CheckedCompilerEvaluationDependencies,
        pub headers: Arena<CheckedCallableHeader>,
        pub invocation_targets: Arena<CheckedSynchronousBoundaryTarget>,
        machines: Arena<CheckedDeclarationDescription>,
        pub parameters: Arena<CheckedDeclarationParameter>,
        #[record(skip)]
        machine_index: Vec<((u32, u32), arena::Handle<CheckedDeclarationDescription>)>,
        #[record(skip)]
        indexed_at: Option<(u64, usize)>,
    }

    impl PartialEq for CheckedDeclarationDescriptions {
        fn eq(&self, other: &Self) -> bool {
            self.selections == other.selections
                && self.evaluation_dependencies == other.evaluation_dependencies
                && self.headers == other.headers
                && self.invocation_targets == other.invocation_targets
                && self.machines == other.machines
                && self.parameters == other.parameters
        }
    }

    impl Eq for CheckedDeclarationDescriptions {}

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedSelectionRequirement {
        #[default]
        Missing,
        Selection {
            occurrence: typed_trees::AuthoredDeclarationSelectionOccurrenceId,
            source_span: source::SourceSpan,
            excluded: bool,
            request: Result<
                Option<semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest>,
                String,
            >,
        },
    }

    impl CheckedSelectionRequirement {
        pub fn occurrence_ordinal(&self) -> u64 {
            match self {
                Self::Missing => 0,
                Self::Selection { occurrence, .. } => occurrence.ordinal(),
            }
        }
    }

    impl Default for CheckedDeclarationDescriptions {
        fn default() -> Self {
            Self {
                selections: Arena::new(),
                evaluation_dependencies: CheckedCompilerEvaluationDependencies::default(),
                headers: Arena::new(),
                invocation_targets: Arena::new(),
                machines: Arena::new(),
                parameters: Arena::new(),
                machine_index: Vec::new(),
                indexed_at: None,
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCompilerEvaluationDependencies {
        pub recursive_components: Arena<CheckedCompilerEvaluationComponent>,
        pub recursive_members: Arena<super::CheckedRuntimeRankMember>,
        pub recursive_calls: Arena<CheckedCompilerEvaluationCall>,
        pub pending_conditions: Arena<SymbolHandle>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCompilerEvaluationComponent {
        pub members: HandleSpan<super::CheckedRuntimeRankMember>,
        pub calls: HandleSpan<CheckedCompilerEvaluationCall>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCompilerEvaluationCall {
        pub caller: SymbolHandle,
        pub call_ordinal: u32,
        pub callee: SymbolHandle,
    }

    impl CheckedDeclarationDescriptions {
        pub fn description(&self, machine: SymbolHandle) -> Option<&CheckedDeclarationDescription> {
            if !machine.is_valid() {
                return None;
            }
            if self.indexed_at != Some((self.machines.revision(), self.machines.len())) {
                return self
                    .machines
                    .iter()
                    .find_map(|(_, row)| (row.machine() == machine).then_some(row));
            }
            let key = symbol_key(machine);
            let start = self.machine_index.partition_point(|(row, _)| *row < key);
            self.machine_index[start..]
                .iter()
                .take_while(|(row, _)| *row == key)
                .map(|(_, handle)| self.machines.get(*handle))
                .find(|row| row.machine() == machine)
        }

        pub fn with_requirements(
            selections: Arena<CheckedSelectionRequirement>,
            evaluation_dependencies: CheckedCompilerEvaluationDependencies,
        ) -> Self {
            Self {
                selections,
                evaluation_dependencies,
                ..Self::default()
            }
        }

        pub fn machines(&self) -> &Arena<CheckedDeclarationDescription> {
            &self.machines
        }

        pub fn insert_machine(
            &mut self,
            row: CheckedDeclarationDescription,
        ) -> arena::Handle<CheckedDeclarationDescription> {
            self.machines.insert(row)
        }

        pub fn index_machines(&mut self) {
            let mut index = self
                .machines
                .iter()
                .enumerate()
                .map(|(position, (handle, row))| (symbol_key(row.machine()), position, handle))
                .collect::<Vec<_>>();
            index.sort_unstable_by_key(|(key, position, _)| (*key, *position));
            self.machine_index = index
                .into_iter()
                .map(|(key, _, handle)| (key, handle))
                .collect();
            self.indexed_at = Some((self.machines.revision(), self.machines.len()));
        }

        pub fn for_machine(
            &self,
            machine: SymbolHandle,
        ) -> Result<&CheckedMachineDescription, &str> {
            match self.description(machine) {
                Some(CheckedDeclarationDescription::Machine(description)) => Ok(description),
                Some(CheckedDeclarationDescription::Unavailable { reason, .. }) => Err(reason),
                _ => Err("checked machine lost its retained declaration description"),
            }
        }
    }

    fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
        (symbol.arena_index(), symbol.generation())
    }
}
pub(crate) mod proof_applications {
    use flow_effects::OperationalPlan;
    use semantic_vocabulary::proof_applications;
    use symbols::SymbolHandle;

    pub type ProofApplicationMachine = proof_applications::ProofApplicationMachine<SymbolHandle>;
    pub type ProofApplicationTargetMember =
        proof_applications::ProofApplicationTargetMember<SymbolHandle>;
    pub type ProofDeclarationDependencies =
        proof_applications::ProofDeclarationDependencies<SymbolHandle>;
    pub type ProofApplicationPlan = proof_applications::ProofApplicationPlan<SymbolHandle>;
    pub type ProofApplications<'a> =
        proof_applications::ProofApplications<'a, SymbolHandle, OperationalPlan>;

    #[cfg(test)]
    pub(crate) mod tests {
        #[test]
        fn published_graph_preserves_nested_bindings_and_declaration_membership() {
            let (mut plan, mut operational) = recorded_calls();
            let nested =
                operational
                    .static_machine_bindings
                    .insert_many([StaticMachineCallBinding {
                        parameter: symbol(11),
                        selected: symbol(4),
                        ..Default::default()
                    }]);
            operational
                .static_machine_bindings
                .for_each_mut(|_, binding| {
                    if binding.parameter == symbol(10) {
                        binding.arguments = nested;
                    }
                });
            let nested_calls = operational.calls.insert_many([CallOperational {
                static_machine_parameter: symbol(11),
                ..Default::default()
            }]);
            let nested_states = operational.states.insert_many([StateOperational {
                calls: nested_calls,
                ..Default::default()
            }]);
            let mut summaries = operational.machines().to_vec();
            summaries.push(MachineOperational {
                symbol: symbol(3),
                states: nested_states,
                ..Default::default()
            });
            operational.root_machines = operational.machines.insert_many(summaries);
            let dynamic = plan.symbols.insert_many([symbol(8)]);
            plan.machines.for_each_mut(|_, machine| {
                if machine.machine == symbol(3) {
                    machine.dynamic_realizations = dynamic;
                }
            });
            let dependencies = plan.symbols.insert_many([symbol(20)]);
            plan.declaration_dependencies
                .insert(ProofDeclarationDependencies {
                    declaration: symbol(4),
                    dependencies,
                    ..Default::default()
                });
            let map = |symbol: SymbolHandle| -> Result<u32, &'static str> {
                Ok(if symbol.is_valid() {
                    100 + symbol.arena_index()
                } else {
                    0
                })
            };
            let portable_plan = plan.try_map(map).unwrap();
            let portable_calls = ProofCallGraph::capture(
                &operational,
                plan.machines.iter().map(|(_, row)| row.machine),
                &mut { map },
            )
            .unwrap();
            let source = ProofApplications::new(&plan, &operational);
            let portable = semantic_vocabulary::proof_applications::ProofApplications::new(
                &portable_plan,
                &portable_calls,
            );
            let source_members =
                source.membership("windows_x86_64", false, &[(symbol(6), symbol(7))], None);
            let portable_members =
                portable.membership("windows_x86_64", false, &[(106, 107)], None);
            assert_eq!(
                portable_members.0,
                source_members
                    .0
                    .iter()
                    .map(|symbol| map(*symbol).unwrap())
                    .collect::<Vec<_>>()
            );
            assert_eq!(portable_members.1, [120]);
            let mut machines = portable_members.0;
            machines.sort_unstable();
            assert_eq!(machines, [101, 102, 103, 104, 105, 106, 107, 108]);
            assert_eq!(
                portable.membership("macos_arm64", false, &[], None).0,
                [109]
            );
            assert!(
                !portable
                    .membership("windows_x86_64", false, &[], None)
                    .0
                    .contains(&107)
            );
            assert!(
                !portable_calls
                    .machines()
                    .iter()
                    .any(|(_, row)| row.machine == 108)
            );
        }

        use super::*;
        use flow_effects::{
            CallOperational, CleanupDependency, MachineOperational, StateOperational,
            StaticMachineCallBinding,
        };
        use semantic_vocabulary::proof_applications::ProofCallGraph;

        fn symbol(ordinal: u32) -> SymbolHandle {
            SymbolHandle::from_parts(ordinal, 1)
        }

        fn machine(plan: &mut ProofApplicationPlan, ordinal: u32, scope: Option<&str>) {
            plan.machines.insert(ProofApplicationMachine {
                machine: symbol(ordinal),
                authored_target: scope.map(str::to_owned),
                source_target: scope.map(str::to_owned),
                product_selected: true,
                ..Default::default()
            });
        }

        fn recorded_calls() -> (ProofApplicationPlan, OperationalPlan) {
            let mut plan = ProofApplicationPlan::default();
            machine(&mut plan, 1, Some("windows_x86_64"));
            for ordinal in 2..=8 {
                machine(&mut plan, ordinal, None);
            }
            machine(&mut plan, 9, Some("macos_arm64"));
            let mut operational = OperationalPlan::default();
            let bindings =
                operational
                    .static_machine_bindings
                    .insert_many([StaticMachineCallBinding {
                        parameter: symbol(10),
                        selected: symbol(3),
                        ..Default::default()
                    }]);
            let root_calls = operational.calls.insert_many([
                CallOperational {
                    target_machine_symbol: symbol(2),
                    static_machine_bindings: bindings,
                    ..Default::default()
                },
                CallOperational {
                    target_machine_symbol: symbol(6),
                    ..Default::default()
                },
            ]);
            let callback = operational.calls.insert_many([CallOperational {
                static_machine_parameter: symbol(10),
                ..Default::default()
            }]);
            let root_states = operational.states.insert_many([StateOperational {
                calls: root_calls,
                ..Default::default()
            }]);
            let helper_states = operational.states.insert_many([StateOperational {
                calls: callback,
                ..Default::default()
            }]);
            operational.root_machines = operational.machines.insert_many([
                MachineOperational {
                    symbol: symbol(1),
                    states: root_states,
                    ..Default::default()
                },
                MachineOperational {
                    symbol: symbol(2),
                    states: helper_states,
                    ..Default::default()
                },
            ]);
            operational.cleanup_dependencies.insert(CleanupDependency {
                caller_machine: symbol(1),
                target_machine: symbol(5),
                ..Default::default()
            });
            (plan, operational)
        }

        #[test]
        fn supplemental_membership_follows_bindings_cleanup_and_selected_providers() {
            let (plan, operational) = recorded_calls();
            let applications = ProofApplications::new(&plan, &operational);
            let mut members = applications
                .membership("windows_x86_64", false, &[(symbol(6), symbol(7))], None)
                .0;
            members.sort_unstable_by_key(|member| member.arena_index());
            assert_eq!(members, [1, 2, 3, 5, 6, 7].map(symbol));
            assert_eq!(
                applications.membership("macos_arm64", false, &[], None).0,
                vec![symbol(9)]
            );
        }

        #[test]
        fn discovery_keeps_selected_scope_first_and_skips_unrelated_scopes() {
            let (mut plan, operational) = recorded_calls();
            let empty = ProofApplications::new(&plan, &operational);
            assert_eq!(empty.scopes("macos_arm64"), ["macos_arm64"]);
            assert!(!empty.needs_scope("windows_x86_64", "macos_arm64"));
            plan.has_obligations = true;
            plan.obligation_owners = plan.symbols.insert_many([symbol(3)]);
            let demands = ProofApplications::new(&plan, &operational);
            assert_eq!(
                demands.scopes("macos_arm64"),
                ["macos_arm64", "windows_x86_64"]
            );
            assert!(demands.needs_scope("windows_x86_64", "macos_arm64"));
            assert!(demands.needs_scope("macos_arm64", "macos_arm64"));
            assert!(!demands.needs_scope("linux_arm64", "macos_arm64"));
        }

        #[test]
        fn retained_metadata_does_not_restore_pruned_seeds_or_provider_bodies() {
            let (mut plan, operational) = recorded_calls();
            let captured_count = plan.machines.len();
            plan.retain_product_machines(|machine| ![symbol(7), symbol(9)].contains(&machine));
            assert_eq!(plan.machines.len(), captured_count);
            let applications = ProofApplications::new(&plan, &operational);
            let mut members = applications
                .membership("windows_x86_64", false, &[(symbol(6), symbol(7))], None)
                .0;
            members.sort_unstable_by_key(|member| member.arena_index());
            assert_eq!(members, [1, 2, 3, 5, 6].map(symbol));
            assert!(
                applications
                    .membership("macos_arm64", false, &[], None)
                    .0
                    .is_empty()
            );
        }
    }
}
pub use declaration_descriptions::{
    CheckedCallableHeader, CheckedCompilerEvaluationCall, CheckedCompilerEvaluationComponent,
    CheckedCompilerEvaluationDependencies, CheckedDeclarationCapture,
    CheckedDeclarationDescription, CheckedDeclarationDescriptions, CheckedDeclarationParameter,
    CheckedDeclarationSpecialization, CheckedMachineDescription, CheckedSelectionRequirement,
    CheckedSynchronousBoundaryTarget,
};
pub use proof_applications::{
    ProofApplicationMachine, ProofApplicationPlan, ProofApplicationTargetMember, ProofApplications,
    ProofDeclarationDependencies,
};
pub(crate) mod mutation {
    use arena::Arena;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct MutationFacts {
        pub machines: Vec<MachineMutationFact>,
        pub integer_writes: Arena<PerformedIntegerWrite>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct PerformedIntegerWrite {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub statement_index: usize,
        pub destination: typed_trees::expression::ExpressionHandle,
        pub value: typed_trees::expression::ExpressionHandle,
        pub primitive_type: Option<typed_trees::types::PrimitiveType>,
        pub frame: facts::NormalizedWriteFrame,
        pub integer_range: Option<crate::CheckedIntegerRange>,
        pub entry_value: Option<crate::CheckedScalarExpression>,
        pub entry_condition: Option<crate::CheckedBooleanExpression>,
    }

    impl MutationFacts {
        pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineMutationFact> {
            self.machines.iter().find(|fact| fact.machine == machine)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineMutationFact {
        pub machine: SymbolHandle,
        pub state_write_frames: Vec<StateWriteFramePlan>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct StateWriteFramePlan {
        pub state: SymbolHandle,
        pub frame: facts::NormalizedWriteFrame,
    }
}
pub(crate) mod qualifications {
    use language_semantics::SemanticDomainId;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct QualificationFacts {
        pub scalar_returns: Vec<CheckedScalarReturnQualification>,
        pub value_formations: arena::Arena<CheckedValueQualification>,
        pub machines: Vec<MachineQualifications>,
        pub vacuous_uses: Vec<VacuousQualificationUse>,
        pub content: crate::ContentProjectionFacts,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedScalarReturnQualification {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub statement_index: usize,
        pub transition_target: typed_trees::statement::TransitionTargetHandle,
        pub is_continuation: bool,
        pub expression: ExpressionHandle,
        pub semantic_domain: SemanticDomainId,
        pub domain: SymbolHandle,
        pub origin: CheckedScalarReturnOrigin,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedScalarReturnOrigin {
        Forwarded,
        Predicate,
        Established(language_semantics::DomainEstablishmentRoute),
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedValueQualification {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub statement_index: u32,
        pub expression: ExpressionHandle,
        pub semantic_domain: SemanticDomainId,
        pub domain_symbol: SymbolHandle,
        pub structural_value: crate::CheckedStructuralValueHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct VacuousQualificationUse {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub statement_index: u32,
        pub expression: ExpressionHandle,
        pub domain: SymbolHandle,
        pub semantic_domain: SemanticDomainId,
    }

    impl QualificationFacts {
        pub fn formation_domains_at(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement_index: u32,
            expression: ExpressionHandle,
        ) -> Vec<SemanticDomainId> {
            self.value_formations
                .iter()
                .filter_map(|(_, row)| {
                    (!row.structural_value.is_valid()
                        && row.machine == machine
                        && row.state == state
                        && row.statement_index == statement_index
                        && row.expression == expression)
                        .then_some(row.semantic_domain)
                })
                .collect()
        }

        pub fn link_value_formations(
            &mut self,
            values: &mut crate::CheckedStructuralValuePlans,
            flow: &crate::FlowFacts,
        ) {
            let mut decisions = std::mem::take(&mut self.value_formations)
                .into_items()
                .collect::<Vec<_>>();
            decisions.extend(
                self.vacuous_uses
                    .iter()
                    .map(|use_| CheckedValueQualification {
                        machine: use_.machine,
                        state: use_.state,
                        statement_index: use_.statement_index,
                        expression: use_.expression,
                        semantic_domain: use_.semantic_domain,
                        domain_symbol: use_.domain,
                        structural_value: crate::CheckedStructuralValueHandle::invalid(),
                    }),
            );
            decisions.retain(|decision| !self.transformation_publishes(flow, decision));
            let coordinate =
                |machine: SymbolHandle,
                 state: SymbolHandle,
                 statement_index: u32,
                 expression: ExpressionHandle,
                 structural_value: crate::CheckedStructuralValueHandle| {
                    (
                        (machine.arena_index(), machine.generation()),
                        (state.arena_index(), state.generation()),
                        statement_index,
                        (expression.arena_index(), expression.generation()),
                        (
                            structural_value.arena_index(),
                            structural_value.generation(),
                        ),
                    )
                };
            let site = |decision: &CheckedValueQualification| {
                coordinate(
                    decision.machine,
                    decision.state,
                    decision.statement_index,
                    decision.expression,
                    decision.structural_value,
                )
            };
            decisions.sort_by_key(|decision| (site(decision), decision.semantic_domain.0));
            decisions.dedup();
            let mut groups = Vec::new();
            let mut start = 0;
            while start < decisions.len() {
                let key = site(&decisions[start]);
                let mut end = start + 1;
                while end < decisions.len() && site(&decisions[end]) == key {
                    end += 1;
                }
                let span = self
                    .value_formations
                    .insert_many(decisions[start..end].iter().copied());
                groups.push((key, span));
                start = end;
            }
            values.nodes.for_each_mut(|handle, value| {
                let structural_value = if matches!(
                    value.kind,
                    crate::CheckedStructuralValueKind::ZeroedStructural { .. }
                        | crate::CheckedStructuralValueKind::ZeroedScalarArray { .. }
                ) {
                    handle
                } else {
                    crate::CheckedStructuralValueHandle::invalid()
                };
                let key = coordinate(
                    value.machine,
                    value.state,
                    value.statement_index,
                    value.expression,
                    structural_value,
                );
                value.qualification_decisions =
                    match groups.binary_search_by_key(&key, |(key, _)| *key) {
                        Ok(position) => groups[position].1,
                        Err(_) => arena::HandleSpan::empty(),
                    };
            });
        }

        fn transformation_publishes(
            &self,
            flow: &crate::FlowFacts,
            decision: &CheckedValueQualification,
        ) -> bool {
            if decision.structural_value.is_valid() {
                return false;
            }
            self.content.publishes_formation_membership(
                flow,
                decision.state,
                decision.statement_index as usize,
                decision.expression,
                decision.domain_symbol,
                decision.semantic_domain,
            )
        }

        pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineQualifications> {
            self.machines.iter().find(|fact| fact.machine == machine)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineQualifications {
        pub machine: SymbolHandle,
        pub body_committed: Vec<SemanticDomainId>,
    }
}
pub(crate) mod service_reaches {
    use arena::{Arena, HandleSpan};
    use language_semantics::{ServiceReachRowId, ServiceReachRowTable, ServiceReachTable};
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ServiceReachFacts {
        pub services: ServiceReachTable,
        pub rows: ServiceReachRowTable,
        pub dependency_parameters: Arena<SymbolHandle>,
        pub root_machines: HandleSpan<MachineServiceReachRows>,
        pub machines: Arena<MachineServiceReachRows>,
        pub states: Arena<StateServiceReachRows>,
        pub calls: Arena<CallServiceReachRows>,
    }

    impl ServiceReachFacts {
        pub fn machines(&self) -> &[MachineServiceReachRows] {
            self.machines.span_or_empty(self.root_machines)
        }

        pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineServiceReachRows> {
            self.machines().iter().find(|fact| fact.machine == machine)
        }

        pub fn plan_for_machine(
            &self,
            machine: SymbolHandle,
        ) -> Option<language_semantics::ServiceReachPlan> {
            self.for_machine(machine)
                .map(|fact| language_semantics::ServiceReachPlan {
                    interface: fact.interface,
                    checked_inferred: fact.inferred_transitive,
                })
        }

        pub fn states_for(&self, machine: &MachineServiceReachRows) -> &[StateServiceReachRows] {
            self.states.span_or_empty(machine.states)
        }

        pub fn for_state(&self, state: SymbolHandle) -> Option<&StateServiceReachRows> {
            self.states
                .iter()
                .map(|(_, fact)| fact)
                .find(|fact| fact.state == state)
        }

        pub fn calls_for(&self, state: &StateServiceReachRows) -> &[CallServiceReachRows] {
            self.calls.span_or_empty(state.calls)
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineServiceReachRows {
        pub machine: SymbolHandle,
        pub dependency: flow_effects::ServiceReachDependency,
        pub cleanup_contribution: ServiceReachRowId,
        pub interface: language_semantics::ServiceReachInterface,
        pub published_ceiling: ServiceReachRowId,
        pub inferred_direct: ServiceReachRowId,
        pub inferred_transitive: ServiceReachRowId,
        pub concrete_transitive: ServiceReachRowId,
        pub effective: ServiceReachRowId,
        pub concrete_effective: ServiceReachRowId,
        pub unresolved_installation_reaches: Vec<flow_effects::InstallationReachRequirement>,
        pub states: HandleSpan<StateServiceReachRows>,
    }

    pub type StateServiceReachRows = flow_effects::StateServiceReachInference;

    pub type CallServiceReachRows = flow_effects::CallServiceReachInference;
}
pub(crate) mod suspensions {
    use language_semantics::SuspensionPlan;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct SuspensionFacts {
        pub machines: Vec<MachineSuspensionFact>,
    }

    impl SuspensionFacts {
        pub fn for_machine(&self, machine: SymbolHandle) -> Option<SuspensionPlan> {
            self.machines
                .iter()
                .find(|fact| fact.machine == machine)
                .map(|fact| fact.plan)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineSuspensionFact {
        pub machine: SymbolHandle,
        pub plan: SuspensionPlan,
    }
}
pub(crate) mod synchronous_invocations {
    use language_semantics::SynchronousInvocationPlan;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct SynchronousInvocationFacts {
        pub machines: Vec<MachineSynchronousInvocationFact>,
    }

    impl SynchronousInvocationFacts {
        pub fn for_machine(&self, machine: SymbolHandle) -> Option<&SynchronousInvocationPlan> {
            self.machines
                .iter()
                .find(|fact| fact.machine == machine)
                .map(|fact| &fact.plan)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineSynchronousInvocationFact {
        pub machine: SymbolHandle,
        pub published_targets: Vec<flow_effects::InvocationTarget>,
        pub checked_inferred_targets: Vec<flow_effects::InvocationTarget>,
        pub realized_targets: Vec<flow_effects::RealizedInvocationTarget>,
        pub plan: SynchronousInvocationPlan,
    }
}
pub(crate) use crate::termination_and_cleanup_facts::termination;

pub(crate) mod calls {
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedFactCallProjection {
        pub projection_expression: typed_trees::expression::ExpressionHandle,
        pub call_expression: typed_trees::expression::ExpressionHandle,
        pub target_machine: symbols::SymbolHandle,
        pub target_state: symbols::SymbolHandle,
        pub machine_arguments: Box<[typed_trees::expression::StaticMachineArgument]>,
        pub result_type: typed_trees::types::TypeReferenceHandle,
        pub field: symbols::SymbolHandle,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CheckedBoundaryAdapterDispatch {
        pub receiver: symbols::SymbolHandle,
        pub requirement: symbols::SymbolHandle,
        pub realization_state: symbols::SymbolHandle,
        pub family_tuple: Box<[String]>,
    }
}
pub(crate) mod quotient_operations {
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedQuotientOperation {
        pub request: typed_trees::expression::ExpressionHandle,
        pub public_machine: symbols::SymbolHandle,
        pub public_state: symbols::SymbolHandle,
        pub representative_state: symbols::SymbolHandle,
        pub correspondence:
            language_semantics::quotient_correspondence::QuotientCorrespondenceJudgment,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedQuotientFormation {
        pub data_symbol: symbols::SymbolHandle,
        pub carrier: typed_trees::types::TypeReferenceHandle,
        pub relation_symbol: symbols::SymbolHandle,
    }
}
pub use quotient_operations::{CheckedQuotientFormation, CheckedQuotientOperation};
pub(crate) mod dynamic_conformances {
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::type_identity::CallableIdentityId;
    use typed_trees::type_identity::TypeIdentityId;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct DynamicSelectionFact {
        pub occurrence: ExpressionHandle,
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub implementation: SymbolHandle,
        pub target_trait: SymbolHandle,
        pub interface_identity: TypeIdentityId,
        pub rows: Vec<DynamicConformanceRowFact>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct DynamicConformanceRowFact {
        pub declaring_trait: SymbolHandle,
        pub requirement: SymbolHandle,
        pub requirement_identity: CallableIdentityId,
        pub realization_machine: SymbolHandle,
        pub realization_state: SymbolHandle,
        pub realization_identity: CallableIdentityId,
        pub family_tuple: Box<[String]>,
        pub source: DynamicConformanceRowSource,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum DynamicConformanceRowSource {
        Inline,
        Reference,
        TraitDefault,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct DynamicConformanceFacts {
        pub selections: Vec<DynamicSelectionFact>,
    }
}
pub(crate) mod installed_interrupts {
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedInstalledInterruptRegistration {
        pub site: crate::NominalMachineUseSite,
        pub control_service: language_semantics::ServiceReachId,
        pub application: typed_trees::static_applications::StaticMachineApplicationHandle,
        pub target: symbols::SymbolHandle,
        pub registrar_owner: symbols::SymbolHandle,
        pub registrar_application_identity: String,
        pub registrar_declaring_application: String,
        pub registrar_arguments: Vec<String>,
        pub registrar_lifetimes: Vec<String>,
        pub timer_slot: typed_trees::expression::StaticMachineArgument,
        pub slot_application: typed_trees::typed_trees::ClosedConformanceApplication,
        pub callbacks: crate::NominalMachineUseFacts,
        pub specializations: crate::RequirementCallSpecializationFacts,
    }
}
pub(crate) mod nominal_machine_uses {
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::statement::StatementHandle;

    use super::contract_plans::CheckedEntryResourceEnvelope;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum NominalMachineUseSite {
        Statement(StatementHandle),
        Expression(ExpressionHandle),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedMachineContractEnvelopeIdentity {
        pub contract_report_fingerprint: u64,
        pub contract_commitment: crate::MachineContractCommitment,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedMachineContractRefinement {
        pub published_requirement_report_fingerprint: u64,
        pub published_requirement_commitment: crate::MachineContractCommitment,
        pub selected_actual_report_fingerprint: u64,
        pub selected_actual_commitment: crate::MachineContractCommitment,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCallbackResourceReceipt {
        machine: SymbolHandle,
        entry: SymbolHandle,
        contract_report_fingerprint: u64,
        contract_commitment: crate::MachineContractCommitment,
        stack_report_fingerprint: u64,
        logical_structural_work_report_fingerprint: u64,
        machine_state_report_fingerprint: u64,
        envelope_report_fingerprint: u64,
    }

    impl CheckedCallbackResourceReceipt {
        pub fn try_from_entry_envelope(
            envelope: &CheckedEntryResourceEnvelope,
        ) -> Result<Self, &'static str> {
            envelope.validate()?;
            Ok(Self {
                machine: envelope.machine(),
                entry: envelope.entry(),
                contract_report_fingerprint: envelope.contract_report_fingerprint(),
                contract_commitment: envelope.contract_commitment(),
                stack_report_fingerprint: envelope.stack().report_fingerprint(),
                logical_structural_work_report_fingerprint: envelope
                    .logical_structural_work()
                    .report_fingerprint(),
                machine_state_report_fingerprint: envelope.machine_state().report_fingerprint(),
                envelope_report_fingerprint: envelope.report_fingerprint(),
            })
        }

        pub const fn machine(self) -> SymbolHandle {
            self.machine
        }

        pub const fn entry(self) -> SymbolHandle {
            self.entry
        }

        pub const fn contract_report_fingerprint(self) -> u64 {
            self.contract_report_fingerprint
        }

        pub const fn contract_commitment(self) -> crate::MachineContractCommitment {
            self.contract_commitment
        }

        pub const fn stack_report_fingerprint(self) -> u64 {
            self.stack_report_fingerprint
        }

        pub const fn logical_structural_work_report_fingerprint(self) -> u64 {
            self.logical_structural_work_report_fingerprint
        }

        pub const fn machine_state_report_fingerprint(self) -> u64 {
            self.machine_state_report_fingerprint
        }

        pub const fn envelope_report_fingerprint(self) -> u64 {
            self.envelope_report_fingerprint
        }

        pub fn validate(self) -> Result<(), &'static str> {
            let replayed = CheckedEntryResourceEnvelope::from_checked_contract(
                self.machine,
                self.entry,
                self.contract_report_fingerprint,
                self.contract_commitment,
            );
            self.validate_against(&replayed)
        }

        pub fn validate_against(
            self,
            envelope: &CheckedEntryResourceEnvelope,
        ) -> Result<(), &'static str> {
            envelope.validate()?;
            if self.machine != envelope.machine()
                || self.entry != envelope.entry()
                || self.contract_report_fingerprint != envelope.contract_report_fingerprint()
                || self.contract_commitment != envelope.contract_commitment()
                || self.stack_report_fingerprint != envelope.stack().report_fingerprint()
                || self.logical_structural_work_report_fingerprint
                    != envelope.logical_structural_work().report_fingerprint()
                || self.machine_state_report_fingerprint
                    != envelope.machine_state().report_fingerprint()
                || self.envelope_report_fingerprint != envelope.report_fingerprint()
            {
                return Err(
                    "checked callback resource receipt does not bind its exact per-entry envelope",
                );
            }
            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedNominalMachineUse {
        pub application_index: u32,
        pub site: NominalMachineUseSite,
        pub registration_operation: SymbolHandle,
        pub registration_requirement_trait: Option<SymbolHandle>,
        pub static_machine_ordinal: u32,
        pub selected_machine: SymbolHandle,
        pub selected_entry: SymbolHandle,
        pub receiving_trait: SymbolHandle,
        pub receiving_application: Result<String, String>,
        pub satisfaction_trait: SymbolHandle,
        pub satisfaction_requirement: SymbolHandle,
        pub canonical_requirement_overload: String,
        pub published_requirement_envelope: CheckedMachineContractEnvelopeIdentity,
        pub selected_actual_envelope: CheckedMachineContractEnvelopeIdentity,
        pub callback_entry: Option<CheckedCallbackResourceReceipt>,
        pub callback_requirements: Vec<SymbolHandle>,
        pub refinement: CheckedMachineContractRefinement,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct NominalMachineUseFacts {
        pub uses: Vec<CheckedNominalMachineUse>,
    }

    impl NominalMachineUseFacts {
        pub fn try_with_uses(
            uses: impl IntoIterator<Item = CheckedNominalMachineUse>,
        ) -> Result<Self, String> {
            let mut retained = Vec::new();
            for nominal_use in uses {
                if nominal_use
                    .published_requirement_envelope
                    .contract_commitment
                    .is_zero()
                    || nominal_use
                        .selected_actual_envelope
                        .contract_commitment
                        .is_zero()
                {
                    return Err(
                        "nominal machine use retained an empty envelope identity".to_owned()
                    );
                }
                if let Some(receipt) = nominal_use.callback_entry {
                    receipt.validate()?;
                    if receipt.machine() != nominal_use.selected_machine
                        || receipt.entry() != nominal_use.selected_entry
                        || receipt.contract_report_fingerprint()
                            != nominal_use
                                .selected_actual_envelope
                                .contract_report_fingerprint
                        || receipt.contract_commitment()
                            != nominal_use.selected_actual_envelope.contract_commitment
                    {
                        return Err(
                        "nominal callback resource receipt does not bind its selected actual entry envelope"
                            .to_owned(),
                    );
                    }
                }
                if nominal_use
                    .refinement
                    .published_requirement_report_fingerprint
                    != nominal_use
                        .published_requirement_envelope
                        .contract_report_fingerprint
                    || nominal_use.refinement.selected_actual_report_fingerprint
                        != nominal_use
                            .selected_actual_envelope
                            .contract_report_fingerprint
                    || nominal_use.refinement.published_requirement_commitment
                        != nominal_use
                            .published_requirement_envelope
                            .contract_commitment
                    || nominal_use.refinement.selected_actual_commitment
                        != nominal_use.selected_actual_envelope.contract_commitment
                {
                    return Err(
                    "nominal machine use refinement receipt does not bind its envelope identities"
                        .to_owned(),
                );
                }
                if let Some(existing) =
                    retained
                        .iter()
                        .find(|existing: &&CheckedNominalMachineUse| {
                            existing.site == nominal_use.site
                                && existing.static_machine_ordinal
                                    == nominal_use.static_machine_ordinal
                        })
                {
                    if *existing != nominal_use {
                        return Err(format!(
                            "nominal machine use site {:?} ordinal {} has conflicting admitted identities",
                            nominal_use.site, nominal_use.static_machine_ordinal
                        ));
                    }
                    continue;
                }
                retained.push(nominal_use);
            }
            for (application_index, usage) in retained.iter_mut().enumerate() {
                usage.application_index = u32::try_from(application_index)
                    .map_err(|_| "nominal machine application count exceeds u32".to_owned())?;
            }
            Ok(Self { uses: retained })
        }

        pub fn for_site(
            &self,
            site: NominalMachineUseSite,
            static_machine_ordinal: u32,
        ) -> Option<&CheckedNominalMachineUse> {
            self.uses.iter().find(|nominal_use| {
                nominal_use.site == site
                    && nominal_use.static_machine_ordinal == static_machine_ordinal
            })
        }
    }
}
pub use installed_interrupts::CheckedInstalledInterruptRegistration;
pub(crate) mod provider_closed_requirements {
    use arena::{Arena, HandleSpan};
    use symbols::SymbolHandle;
    use typed_trees::type_identity::{LifetimeParameterIdentity, NormalizedTypeIdentity};

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProviderClosedRequirement {
        pub provider: SymbolHandle,
        pub selecting_declaration: SymbolHandle,
        pub template: SymbolHandle,
        pub closed_requirement: SymbolHandle,
        pub declaration_application: NormalizedTypeIdentity,
        pub receiving_application: NormalizedTypeIdentity,
        pub receiving_regions: HandleSpan<LifetimeParameterIdentity>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProviderClosedRequirements {
        rows: Arena<CheckedProviderClosedRequirement>,
        regions: Arena<LifetimeParameterIdentity>,
    }

    impl CheckedProviderClosedRequirements {
        pub fn insert(
            &mut self,
            mut row: CheckedProviderClosedRequirement,
            receiving_regions: impl IntoIterator<Item = LifetimeParameterIdentity>,
        ) {
            row.receiving_regions = self.regions.insert_many(receiving_regions);
            self.rows.insert(row);
        }

        pub fn iter(&self) -> impl Iterator<Item = &CheckedProviderClosedRequirement> {
            self.rows.iter().map(|(_, row)| row)
        }

        pub fn receiving_regions(
            &self,
            row: &CheckedProviderClosedRequirement,
        ) -> Option<&[LifetimeParameterIdentity]> {
            self.regions.span(row.receiving_regions)
        }

        pub fn retain(
            &mut self,
            mut retain: impl FnMut(&CheckedProviderClosedRequirement) -> bool,
        ) {
            let removed = self
                .rows
                .iter()
                .filter_map(|(handle, row)| (!retain(row)).then_some(handle))
                .collect::<Vec<_>>();
            for handle in removed {
                self.rows.free(handle);
            }
        }
    }
}
pub(crate) mod provider_instance_publication {
    use arena::{Arena, Handle, HandleSpan};
    use symbols::SymbolHandle;
    use typed_trees::type_identity::CallableIdentityId;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProviderCandidate {
        pub boundary: SymbolHandle,
        pub candidate: SymbolHandle,
        pub declaration: Result<CheckedProviderCandidateDeclaration, String>,
    }

    impl Default for CheckedProviderCandidate {
        fn default() -> Self {
            Self {
                boundary: SymbolHandle::default(),
                candidate: SymbolHandle::default(),
                declaration: Err("missing checked provider candidate".into()),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProviderCandidateDeclaration {
        pub provider: SymbolHandle,
        pub receiving: Handle<CheckedProviderReceivingApplication>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProviderInstanceRequirement {
        pub template: SymbolHandle,
        pub boundary: SymbolHandle,
        pub provider: SymbolHandle,
        pub declaration: Result<CheckedProviderInstanceDeclaration, String>,
    }

    impl Default for CheckedProviderInstanceRequirement {
        fn default() -> Self {
            Self {
                template: SymbolHandle::default(),
                boundary: SymbolHandle::default(),
                provider: SymbolHandle::default(),
                declaration: Err("missing checked provider instance declaration".into()),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProviderInstanceDeclaration {
        pub template_identity: CallableIdentityId,
        pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub selecting_declaration: SymbolHandle,
        pub source_candidate_identity: CallableIdentityId,
        pub requirement_identity: CallableIdentityId,
        pub candidate_identity: CallableIdentityId,
        pub receiving: Handle<CheckedProviderReceivingApplication>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProviderReceivingApplication {
        pub association: super::CheckedProviderClosedRequirement,
        pub description: Result<CheckedProviderReceivingDescription, String>,
    }

    impl Default for CheckedProviderReceivingApplication {
        fn default() -> Self {
            Self {
                association: Default::default(),
                description: Err("missing checked provider receiving publication".into()),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProviderReceivingDescription {
        pub declaration_application: String,
        pub application: String,
        pub regions: HandleSpan<String>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProviderInstancePublication {
        pub candidates: Arena<CheckedProviderCandidate>,
        pub requirements: Arena<CheckedProviderInstanceRequirement>,
        pub receivers: Arena<CheckedProviderReceivingApplication>,
        pub regions: Arena<String>,
    }

    impl CheckedProviderInstancePublication {
        pub fn receiving(
            &self,
            association: &super::CheckedProviderClosedRequirement,
        ) -> Result<&CheckedProviderReceivingDescription, &str> {
            let mut matches = self
                .receivers
                .iter()
                .filter(|(_, row)| row.association == *association);
            let (_, row) = matches
                .next()
                .ok_or("provider lost its checked receiving publication")?;
            if matches.next().is_some() {
                return Err("provider has multiple checked receiving publications");
            }
            row.description.as_ref().map_err(String::as_str)
        }
    }
}
pub(crate) mod requirement_call_specializations {
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;

    use super::nominal_machine_uses::NominalMachineUseSite;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedRequirementCallTypeBinding {
        pub parameter: SymbolHandle,
        pub actual: TypeReferenceHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedRequirementCallMachineSelection {
        pub static_machine_ordinal: u32,
        pub parameter: SymbolHandle,
        pub selected_machine: SymbolHandle,
        pub selected: SymbolHandle,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedRequirementCallSpecialization {
        pub site: NominalMachineUseSite,
        pub registration_operation: SymbolHandle,
        pub type_bindings: Vec<CheckedRequirementCallTypeBinding>,
        pub machine_selections: Vec<CheckedRequirementCallMachineSelection>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct RequirementCallSpecializationFacts {
        pub specializations: Vec<CheckedRequirementCallSpecialization>,
    }

    impl RequirementCallSpecializationFacts {
        pub fn try_with_specializations(
            specializations: impl IntoIterator<Item = CheckedRequirementCallSpecialization>,
        ) -> Result<Self, String> {
            let mut retained: Vec<CheckedRequirementCallSpecialization> = Vec::new();
            for specialization in specializations {
                if specialization.machine_selections.is_empty() {
                    return Err(
                        "requirement call specialization retained no machine selection".to_owned(),
                    );
                }
                if let Some(existing) = retained.iter().find(|existing| {
                    existing.site == specialization.site
                        && existing.registration_operation == specialization.registration_operation
                }) {
                    if *existing != specialization {
                        return Err(format!(
                            "requirement call specialization site {:?} has conflicting admitted specializations",
                            specialization.site
                        ));
                    }
                    continue;
                }
                retained.push(specialization);
            }
            Ok(Self {
                specializations: retained,
            })
        }

        pub fn for_site(
            &self,
            site: NominalMachineUseSite,
            registration_operation: SymbolHandle,
        ) -> Option<&CheckedRequirementCallSpecialization> {
            self.specializations.iter().find(|specialization| {
                specialization.site == site
                    && specialization.registration_operation == registration_operation
            })
        }
    }
}

pub(crate) mod accepted_template_identities {
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedAcceptedTemplateClassification {
        pub machine: symbols::SymbolHandle,
        pub identity: Option<CheckedAcceptedTemplateIdentity>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedAcceptedTemplateIdentity {
        pub report_fingerprint: u64,
        pub commitment: typed_trees::typed_trees::MachineTemplateCommitment,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedAcceptedTemplateIdentities {
        rows: Box<[CheckedAcceptedTemplateClassification]>,
    }

    impl CheckedAcceptedTemplateIdentities {
        pub fn with_rows(rows: Vec<CheckedAcceptedTemplateClassification>) -> Self {
            Self {
                rows: rows.into_boxed_slice(),
            }
        }

        pub fn for_machine(
            &self,
            machine: symbols::SymbolHandle,
        ) -> Option<&CheckedAcceptedTemplateClassification> {
            self.rows.iter().find(|row| row.machine == machine)
        }

        pub fn rows(&self) -> &[CheckedAcceptedTemplateClassification] {
            &self.rows
        }
    }
}
pub(crate) mod carry {
    use arena::{Arena, Handle, HandleSpan};
    use language_semantics::{
        CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension,
    };
    use std::borrow::Cow;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CarryFacts {
        pub data: Vec<DataCarryFact>,
        pub suspension_crossings: Vec<SuspensionCrossingCarryFact>,
        pub activation_wide_carry: Vec<MachineActivationCarryFact>,
        pub claim_policies: Vec<ClaimCarryPolicyFact>,
        pub suspension_loans: Arena<SuspensionCrossingLoanFact>,
        pub suspension_value_loans: Arena<Handle<SuspensionCrossingLoanFact>>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct ClaimCarryPolicyFact {
        pub claim_identity: language_semantics::PermissionClaimIdentity,
        pub effective: CarryPolicy,
        pub contributing_origins: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineActivationCarryFact {
        pub machine: SymbolHandle,
        pub effective: CarryPolicy,
        pub analysis_complete: bool,
        pub contributing_types: Vec<typed_trees::types::TypeReferenceHandle>,
        pub unnamed_strict_values: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct SuspensionCrossingCarryFact {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub statement_index: usize,
        pub call_ordinal: usize,
        pub target: SymbolHandle,
        pub condition: flow_effects::ContractDependency<bool>,
        pub effective: CarryPolicy,
        pub live_values: Vec<SuspensionCrossingLiveValueFact>,
        pub borrows: SuspensionCrossingBorrowFrontier,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct SuspensionCrossingBorrowFrontier {
        pub loans: HandleSpan<SuspensionCrossingLoanFact>,
        pub operands: HandleSpan<crate::FlowOperandFact>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct SuspensionCrossingLoanFact {
        pub source: SuspensionCrossingLoanSource,
        pub active_at_call: bool,
        pub retained_for_crossing: bool,
        pub backing: SuspensionCrossingLoanBacking,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum SuspensionCrossingLoanSource {
        Incoming {
            identity: Handle<crate::IncomingReferenceIdentity>,
            access: crate::BorrowAccessKind,
        },
        Stored {
            loan: Handle<crate::BorrowLoanFact>,
            resource: crate::CheckedParentBorrowResource,
        },
        CapturedActual {
            resource: Handle<crate::CheckedCallBorrowLoanResource>,
        },
        LifetimeDependency {
            loan: Handle<crate::BorrowLoanFact>,
            resource: Handle<crate::CheckedOutputLifetimeDependencyResource>,
        },
        OutputInput {
            owner: Handle<crate::BorrowLoanFact>,
            input: Handle<crate::CheckedOutputBorrowInput>,
        },
    }

    impl Default for SuspensionCrossingLoanSource {
        fn default() -> Self {
            Self::Stored {
                loan: Handle::invalid(),
                resource: Default::default(),
            }
        }
    }

    impl SuspensionCrossingLoanFact {
        pub fn stored_loan(&self) -> Handle<crate::BorrowLoanFact> {
            match self.source {
                SuspensionCrossingLoanSource::Stored { loan, .. }
                | SuspensionCrossingLoanSource::LifetimeDependency { loan, .. } => loan,
                SuspensionCrossingLoanSource::CapturedActual { .. }
                | SuspensionCrossingLoanSource::OutputInput { .. }
                | SuspensionCrossingLoanSource::Incoming { .. } => Handle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum SuspensionCrossingLoanBacking {
        #[default]
        Unresolved,
        StateValue {
            origin: SuspensionCrossingValueOrigin,
        },
        IncomingReferent {
            identity: Handle<crate::IncomingReferenceIdentity>,
        },
        CompletedOutput {
            resource: Handle<crate::CheckedSuppliedOutputBorrowResource>,
        },
        InputDependencies,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct SuspensionCrossingLiveValueFact {
        pub type_reference: typed_trees::types::TypeReferenceHandle,
        pub storage: SuspensionCrossingStorage,
        pub origin: SuspensionCrossingValueOrigin,
        pub claims: Vec<language_semantics::PermissionArrival<SymbolHandle>>,
        pub effective: CarryPolicy,
        pub borrow_frontier: SuspensionValueBorrowFrontier,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum SuspensionValueBorrowFrontier {
        #[default]
        Unresolved,
        ViewFree,
        RetainedLoans {
            loans: HandleSpan<Handle<SuspensionCrossingLoanFact>>,
        },
        // Known leaves are checked now; the value's exact type and subject
        // retain the symbolic remainder until application checking resolves it.
        TemplateDependent {
            loans: HandleSpan<Handle<SuspensionCrossingLoanFact>>,
        },
    }

    pub type SuspensionCrossingValueOrigin =
        language_semantics::SuspensionValueOrigin<SymbolHandle>;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum SuspensionCrossingStorage {
        Parameter,
        Local,
        CallArgument,
    }

    pub fn canonical_suspension_crossing_id(
        program: &typed_trees::TypedTrees,
        crossing: &SuspensionCrossingCarryFact,
    ) -> Option<semantic_vocabulary::SuspensionCrossingId> {
        let mut hash = StableCrossingHash::new();
        hash.byte(0x73);
        hash.string(&symbol_identity(program, crossing.machine)?);
        hash.string(&symbol_identity(program, crossing.state)?);
        hash.usize(crossing.statement_index);
        hash.usize(crossing.call_ordinal);
        hash.string(&symbol_identity(program, crossing.target)?);
        hash.policy(crossing.effective);
        for live in &crossing.live_values {
            hash.string(&program.normalized_type_identity_text(live.type_reference));
            hash.byte(match live.storage {
                SuspensionCrossingStorage::Parameter => 2,
                SuspensionCrossingStorage::Local => 3,
                SuspensionCrossingStorage::CallArgument => 4,
            });
            hash.policy(live.effective);
        }
        semantic_vocabulary::SuspensionCrossingId::new(hash.finish())
    }

    fn symbol_identity(
        program: &typed_trees::TypedTrees,
        symbol: SymbolHandle,
    ) -> Option<Cow<'_, str>> {
        if let Some(slot) = program.dynamic_slot_by_symbol(symbol) {
            let definition = program.trait_definition_by_symbol(slot.declaring_trait)?;
            let requirement = program
                .closed_requirement_application(slot.call_requirement)
                .map(|application| application.signature)
                .or_else(|| program.dynamic_slot_requirement(slot))?;
            return Some(Cow::Owned(format!(
                "selector-slot:{}:{}",
                program.type_identity_text(slot.interface_identity)?,
                program.trait_requirement_callable_identity_text(definition, requirement),
            )));
        }
        for machine in program.machines() {
            if machine.symbol == symbol {
                return Some(crossing_declaration_name(
                    program,
                    symbol,
                    machine.name.as_str(),
                ));
            }
            if let Some(state) = program.state_in_machine(machine, symbol) {
                return Some(crossing_declaration_name(
                    program,
                    symbol,
                    state.name.as_str(),
                ));
            }
        }
        program
            .trait_machine_signature(symbol)
            .filter(|(definition, _)| definition.is_boundary)
            .map(|_| Cow::Owned(program.symbols.display_path(symbol, "::")))
    }

    fn crossing_declaration_name<'program>(
        program: &'program typed_trees::TypedTrees,
        symbol: SymbolHandle,
        legacy_name: &'program str,
    ) -> Cow<'program, str> {
        if program.symbols.symbol_module(symbol).is_valid() {
            Cow::Owned(program.symbols.display_path(symbol, "::"))
        } else {
            Cow::Borrowed(legacy_name)
        }
    }

    struct StableCrossingHash(semantic_vocabulary::Fnv1a);

    impl StableCrossingHash {
        fn new() -> Self {
            Self(semantic_vocabulary::Fnv1a::new())
        }

        fn byte(&mut self, byte: u8) {
            self.0.byte(byte);
        }

        fn string(&mut self, value: &str) {
            self.0.string_terminated(value);
        }

        fn usize(&mut self, value: usize) {
            self.0.u64(value as u64);
        }

        fn policy(&mut self, policy: CarryPolicy) {
            self.byte(u8::from(policy.suspension == CarrySuspension::Allowed));
            self.byte(u8::from(policy.cpu == CarryCpu::Origin));
            self.byte(u8::from(policy.host_thread == CarryHostThread::Origin));
            self.byte(match policy.address {
                CarryAddress::Movable => 1,
                CarryAddress::Stable => 2,
            });
        }

        fn finish(self) -> u64 {
            self.0.finish().max(1)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct DataCarryFact {
        pub data: SymbolHandle,
        pub declared: Option<CarryPolicy>,
        pub effective: CarryPolicy,
    }
}
pub(crate) mod content_classes {
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentClassRow {
        pub plain_owned_with_numeric_constraints: bool,
        pub plain_owned_with_represented_constraints: bool,
        pub linear_owned_contents: bool,
        pub stored_value_contents: bool,
        pub borrow_free_contents: bool,
        pub owned_or_shared_view_fields: bool,
        pub owned_or_shared_view_contents: bool,
        pub closed_primitive_array: bool,
        pub bounded_byte_capacity: Option<usize>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContentClassFacts {
        rows: Vec<ContentClassRow>,
    }

    impl ContentClassFacts {
        pub fn with_rows(rows: Vec<ContentClassRow>) -> Self {
            Self { rows }
        }

        pub fn classes(&self, reference: TypeReferenceHandle) -> ContentClassRow {
            self.rows
                .get(reference.arena_index() as usize)
                .or_else(|| self.rows.first())
                .copied()
                .unwrap_or_default()
        }

        pub fn len(&self) -> usize {
            self.rows.len()
        }

        pub fn is_empty(&self) -> bool {
            self.rows.is_empty()
        }
    }
}
pub(crate) mod domains {
    use arena::{Arena, HandleSpan};
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct DomainDependencyPathFact {
        pub segments: HandleSpan<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct DomainDependencyFact {
        pub domain_symbol: SymbolHandle,
        pub dependencies: HandleSpan<DomainDependencyPathFact>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct DomainFacts {
        pub segments: Arena<facts::PlaceSegment>,
        pub dependency_paths: Arena<DomainDependencyPathFact>,
        pub dependencies: Arena<DomainDependencyFact>,
    }

    impl DomainFacts {
        pub fn with_roots(
            segments: Arena<facts::PlaceSegment>,
            dependency_paths: Arena<DomainDependencyPathFact>,
            dependencies: Arena<DomainDependencyFact>,
        ) -> Self {
            Self {
                segments,
                dependency_paths,
                dependencies,
            }
        }

        pub fn dependency_fact(
            &self,
            domain_symbol: SymbolHandle,
        ) -> Option<&DomainDependencyFact> {
            self.dependencies
                .iter()
                .find_map(|(_, fact)| (fact.domain_symbol == domain_symbol).then_some(fact))
        }

        pub fn dependency_paths<'a>(
            &'a self,
            dependency: &'a DomainDependencyFact,
        ) -> impl Iterator<Item = &'a [facts::PlaceSegment]> + 'a {
            self.dependency_paths
                .span_or_empty(dependency.dependencies)
                .iter()
                .map(|path| self.segments.span_or_empty(path.segments))
        }
    }
}
pub(crate) mod index_compatibility {
    use facts::{FactHandle, ProgramPoint};
    use language_semantics::SemanticDomainId;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum IndexCompatibilityDischarge {
        ClosedEvaluation,
        LicensedNormalization { operation_count: usize },
        EstablishedLocalFacts { facts: Vec<FactHandle> },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct IndexCompatibilityFact {
        pub name: String,
        pub point: ProgramPoint,
        pub value: ExpressionHandle,
        pub target_type: TypeReferenceHandle,
        pub family: SymbolHandle,
        pub actual_instance: SemanticDomainId,
        pub expected_instance: SemanticDomainId,
        pub actual_label: String,
        pub expected_label: String,
        pub discharge: IndexCompatibilityDischarge,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct IndexCompatibilityFacts {
        pub conditions: Vec<IndexCompatibilityFact>,
    }
}
pub(crate) mod placement {
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedAccessViewInput {
        pub machine: symbols::SymbolHandle,
        pub state: symbols::SymbolHandle,
        pub position: u32,
        pub parameter: symbols::SymbolHandle,
        pub reference_access: language_core::ReferenceAccess,
        pub binding_is_const: bool,
        pub binding_is_mutable: bool,
        pub view: symbols::SymbolHandle,
        pub policy: symbols::SymbolHandle,
        pub policy_plan_machine: symbols::SymbolHandle,
        pub schema: symbols::SymbolHandle,
        pub placement: access_plans::ValidatedAccessPlan,
    }
}
pub(crate) mod reference_custody {
    use crate::CheckedStructuralAccess;
    use language_semantics::Multiplicity;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct ReferenceCustodyRow {
        pub primitive_loan_referent: TypeReferenceHandle,
        pub stored_loan_referent: TypeReferenceHandle,
        pub stored_loan_access: CheckedStructuralAccess,
        pub mutable_borrow_referent: TypeReferenceHandle,
        pub shared_borrow_referent: TypeReferenceHandle,
        pub is_reference_record: bool,
        pub binding_multiplicity: Multiplicity,
        pub result_multiplicity: Multiplicity,
    }

    impl Default for ReferenceCustodyRow {
        fn default() -> Self {
            Self {
                primitive_loan_referent: TypeReferenceHandle::invalid(),
                stored_loan_referent: TypeReferenceHandle::invalid(),
                stored_loan_access: CheckedStructuralAccess::Owned,
                mutable_borrow_referent: TypeReferenceHandle::invalid(),
                shared_borrow_referent: TypeReferenceHandle::invalid(),
                is_reference_record: false,
                binding_multiplicity: Multiplicity::Affine,
                result_multiplicity: Multiplicity::Affine,
            }
        }
    }

    impl ReferenceCustodyRow {
        pub fn stored_loan_parts(&self) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
            self.stored_loan_referent
                .is_valid()
                .then_some((self.stored_loan_referent, self.stored_loan_access))
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ReferenceCustodyFacts {
        rows: Vec<ReferenceCustodyRow>,
    }

    impl ReferenceCustodyFacts {
        pub fn with_rows(rows: Vec<ReferenceCustodyRow>) -> Self {
            Self { rows }
        }

        pub fn custody(&self, reference: TypeReferenceHandle) -> ReferenceCustodyRow {
            self.rows
                .get(reference.arena_index() as usize)
                .or_else(|| self.rows.first())
                .copied()
                .unwrap_or_default()
        }

        pub fn len(&self) -> usize {
            self.rows.len()
        }

        pub fn is_empty(&self) -> bool {
            self.rows.is_empty()
        }
    }
}
pub(crate) mod resolved_types {
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ResolvedExpressionType {
        pub expression: ExpressionHandle,
        pub place: TypeReferenceHandle,
        pub result: TypeReferenceHandle,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ResolvedCarrierType {
        pub reference: TypeReferenceHandle,
        pub carrier: TypeReferenceHandle,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ResolvedTypeFacts {
        expressions: Box<[ResolvedExpressionType]>,
        carriers: Box<[ResolvedCarrierType]>,
    }

    impl ResolvedTypeFacts {
        pub fn with_rows(
            expressions: Vec<ResolvedExpressionType>,
            carriers: Vec<ResolvedCarrierType>,
        ) -> Self {
            Self {
                expressions: expressions.into_boxed_slice(),
                carriers: carriers.into_boxed_slice(),
            }
        }

        pub fn place(&self, expression: ExpressionHandle) -> TypeReferenceHandle {
            self.expression(expression)
                .map_or_else(TypeReferenceHandle::invalid, |row| row.place)
        }

        pub fn result(&self, expression: ExpressionHandle) -> TypeReferenceHandle {
            self.expression(expression)
                .map_or_else(TypeReferenceHandle::invalid, |row| row.result)
        }

        pub fn carrier(&self, reference: TypeReferenceHandle) -> TypeReferenceHandle {
            if !reference.is_valid() {
                return TypeReferenceHandle::invalid();
            }
            match self.carriers.get(reference.arena_index() as usize) {
                Some(row) if row.reference == reference => row.carrier,
                _ => TypeReferenceHandle::invalid(),
            }
        }

        fn expression(&self, expression: ExpressionHandle) -> Option<&ResolvedExpressionType> {
            if !expression.is_valid() {
                return None;
            }
            let row = self.expressions.get(expression.arena_index() as usize)?;
            (row.expression == expression).then_some(row)
        }
    }
}
pub(crate) mod specialization_identities {
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedMachineSpecializationIdentity {
        pub instance: symbols::SymbolHandle,
        pub template_base_commitment: [u8; 32],
        pub template_contract_commitment: typed_trees::typed_trees::MachineTemplateCommitment,
        pub suspension_dependency: flow_effects::ContractDependency<bool, Vec<u32>>,
        pub blocking_dependency: flow_effects::ContractDependency<bool, Vec<u32>>,
        pub conditional_carry: Box<[flow_effects::ConditionalCarryRequirement]>,
        pub machine_argument_contract_commitments: Box<[[u8; 32]]>,
        pub commitment: typed_trees::typed_trees::MachineSpecializationCommitment,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedSpecializationIdentities {
        rows: Box<[CheckedMachineSpecializationIdentity]>,
    }

    impl CheckedSpecializationIdentities {
        pub fn with_rows(rows: Vec<CheckedMachineSpecializationIdentity>) -> Self {
            Self {
                rows: rows.into_boxed_slice(),
            }
        }

        pub fn for_instance(
            &self,
            instance: symbols::SymbolHandle,
        ) -> Option<&CheckedMachineSpecializationIdentity> {
            self.rows
                .iter()
                .find(|row| instance.is_valid() && row.instance == instance)
        }
    }
}
pub use accepted_template_identities::{
    CheckedAcceptedTemplateClassification, CheckedAcceptedTemplateIdentities,
    CheckedAcceptedTemplateIdentity,
};
pub use constant_recipe_obligations::{ConstantRecipeObligation, ConstantRecipeRequirement};
pub use specialization_identities::{
    CheckedMachineSpecializationIdentity, CheckedSpecializationIdentities,
};
pub(crate) mod executable_bindings {
    use arena::Handle;
    use symbols::SymbolHandle;
    use typed_trees::expression::{BuildSelectionRequest, CompletedValueCapture, ExpressionHandle};
    use typed_trees::statement::TransitionTargetHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedExpressionSource {
        #[default]
        Invalid,
        Value,
        Saved {
            capture: Handle<CompletedValueCapture>,
            source: ExpressionHandle,
        },
        ProviderReadiness {
            selection: Handle<BuildSelectionRequest>,
            requirement: TypeReferenceHandle,
        },
    }
    impl CheckedExpressionSource {
        pub fn is_saved(self) -> bool {
            matches!(self, Self::Saved { .. })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedCaptureArgumentValue {
        Type(typed_trees::type_identity::TypeIdentityId),
        Const(typed_trees::type_identity::TypeIdentityId),
        Machine,
        RuntimeValue,
        Conformance,
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCaptureArgument {
        pub parameter: SymbolHandle,
        pub value: CheckedCaptureArgumentValue,
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCaptureInputs {
        pub instance: SymbolHandle,
        pub arguments: Vec<CheckedCaptureArgument>,
        pub machines: Vec<SymbolHandle>,
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedSelectedMachine {
        pub symbol: SymbolHandle,
        pub diagnostic_path: String,
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedRequirementMachines {
        pub requirement: SymbolHandle,
        pub machines: Vec<CheckedSelectedMachine>,
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedSourceCallTarget {
        pub target: SymbolHandle,
        pub diagnostic_name: String,
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedSourceTransition {
        pub state: SymbolHandle,
        pub statement_index: usize,
        pub primary: TransitionTargetHandle,
        pub named_targets: Vec<SymbolHandle>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedExecutableBindings {
        expressions: Vec<(ExpressionHandle, CheckedExpressionSource)>,
        pub captures: Vec<CheckedCaptureInputs>,
        pub requirements: Vec<CheckedRequirementMachines>,
        pub calls: Vec<CheckedSourceCallTarget>,
        pub transitions: Vec<CheckedSourceTransition>,
    }
    impl CheckedExecutableBindings {
        pub fn insert_expression(
            &mut self,
            expression: ExpressionHandle,
            source: CheckedExpressionSource,
        ) {
            let index = expression.arena_index() as usize;
            if self.expressions.len() <= index {
                self.expressions.resize(
                    index + 1,
                    (
                        ExpressionHandle::invalid(),
                        CheckedExpressionSource::Invalid,
                    ),
                );
            }
            self.expressions[index] = (expression, source);
        }
        pub fn expression(&self, expression: ExpressionHandle) -> CheckedExpressionSource {
            self.expressions
                .get(expression.arena_index() as usize)
                .filter(|(handle, _)| expression.is_valid() && *handle == expression)
                .map_or(CheckedExpressionSource::Invalid, |(_, source)| *source)
        }
        pub fn transition(
            &self,
            state: SymbolHandle,
            statement_index: usize,
        ) -> Option<&CheckedSourceTransition> {
            self.transitions
                .iter()
                .find(|row| row.state == state && row.statement_index == statement_index)
        }
    }
}
pub(crate) mod state_locals {
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStateLocal {
        pub statement_index: u32,
        pub symbol: SymbolHandle,
        pub declared_type: TypeReferenceHandle,
        pub type_is_inferred: bool,
        pub is_mutable: bool,
        pub is_erased: bool,
        pub is_scalar_value: bool,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct StateLocalFacts {
        spans: Vec<(u32, u32)>,
        locals: Vec<CheckedStateLocal>,
    }

    impl StateLocalFacts {
        pub fn with_states(
            states: impl IntoIterator<Item = (SymbolHandle, Vec<CheckedStateLocal>)>,
        ) -> Self {
            let mut facts = Self::default();
            for (state, locals) in states {
                let index = state.arena_index() as usize;
                if facts.spans.len() <= index {
                    facts.spans.resize(index + 1, (0, 0));
                }
                let start = facts.locals.len() as u32;
                facts.locals.extend(locals);
                facts.spans[index] = (start, facts.locals.len() as u32);
            }
            facts
        }

        pub fn locals(&self, state: SymbolHandle) -> &[CheckedStateLocal] {
            let (start, end) = self
                .spans
                .get(state.arena_index() as usize)
                .copied()
                .unwrap_or_default();
            &self.locals[start as usize..end as usize]
        }

        pub fn at_statement(
            &self,
            state: SymbolHandle,
            statement_index: u32,
        ) -> Option<&CheckedStateLocal> {
            self.locals(state)
                .iter()
                .find(|local| local.statement_index == statement_index)
        }

        pub fn scalar_locals_before(&self, state: SymbolHandle, statement_index: u32) -> u32 {
            self.locals(state)
                .iter()
                .take_while(|local| local.statement_index < statement_index)
                .filter(|local| local.is_scalar_value)
                .count() as u32
        }

        pub fn named(
            &self,
            state: SymbolHandle,
            symbol: SymbolHandle,
        ) -> Option<&CheckedStateLocal> {
            let mut named = self
                .locals(state)
                .iter()
                .filter(|local| local.symbol == symbol && symbol.is_valid());
            let local = named.next()?;
            named.next().is_none().then_some(local)
        }
    }
}
pub use executable_bindings::{
    CheckedCaptureArgument, CheckedCaptureArgumentValue, CheckedCaptureInputs,
    CheckedExecutableBindings, CheckedExpressionSource, CheckedRequirementMachines,
    CheckedSelectedMachine, CheckedSourceCallTarget, CheckedSourceTransition,
};
pub(crate) mod stored_integer_writes {
    use semantic_vocabulary::IntegerType;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum StoredIntegerWriteSite {
        #[default]
        Missing,
        Assignment {
            destination: ExpressionHandle,
        },
        Construction {
            expression: ExpressionHandle,
            statement_index: usize,
        },
        Call {
            statement_index: usize,
            call_ordinal: usize,
            write: arena::Handle<super::mutation::PerformedIntegerWrite>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStoredIntegerWrite {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub site: StoredIntegerWriteSite,
        pub value: ExpressionHandle,
        pub field: SymbolHandle,
        pub stored_type: IntegerType,
    }

    impl Default for CheckedStoredIntegerWrite {
        fn default() -> Self {
            Self {
                machine: SymbolHandle::invalid(),
                state: SymbolHandle::invalid(),
                site: StoredIntegerWriteSite::Missing,
                value: ExpressionHandle::invalid(),
                field: SymbolHandle::invalid(),
                stored_type: IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                    .expect("u8 has a valid fixed width"),
            }
        }
    }
}
pub use stored_integer_writes::{CheckedStoredIntegerWrite, StoredIntegerWriteSite};

pub use blocking::{BlockingFacts, MachineBlockingFact};
pub use calls::{CheckedBoundaryAdapterDispatch, CheckedFactCallProjection};
pub use carry::{
    CarryFacts, ClaimCarryPolicyFact, DataCarryFact, MachineActivationCarryFact,
    SuspensionCrossingBorrowFrontier, SuspensionCrossingCarryFact, SuspensionCrossingLiveValueFact,
    SuspensionCrossingLoanBacking, SuspensionCrossingLoanFact, SuspensionCrossingLoanSource,
    SuspensionCrossingStorage, SuspensionCrossingValueOrigin, SuspensionValueBorrowFrontier,
    canonical_suspension_crossing_id,
};
pub use checked_crash_operator_site::{CheckedCrashOperatorSite, OperatorSiteLocation};
pub use collection::{CheckFacts, DecidedCallRequires, ProvenAssignmentIndex};
pub use content::{
    ContentBackingAddressTransport, ContentBodyAvailableInput, ContentBodyClaimBinding,
    ContentBodyConservationFact, ContentBodyCustody, ContentBodyDerivation,
    ContentBodyTransformation, ContentBoundaryBodyRequirement, ContentClaimTransformation,
    ContentClaimTransformationSource, ContentIdentityReshuffleFact,
    ContentPartitionCompositionFact, ContentPartitionInputClaimBinding,
    ContentPartitionPlaceSubstitution, ContentPartitionResultRewrite, ContentPartitionReturnFact,
    ContentProjectionFacts, ContentWalkRows, RetainedBorrowCustodyFact,
};
pub use content_classes::{ContentClassFacts, ContentClassRow};
pub use contextual_cleanup::{
    ContextualCleanupApplication, ContextualCleanupDisposition, ContextualCleanupEntryRequirement,
    ContextualCleanupEntryRequirements, ContextualCleanupExcludedRegion,
    ContextualCleanupExclusionCause, ContextualCleanupFacts, ContextualStorageEstablishment,
    StorageWindowInvocation,
};
pub use contract_plans::{
    CheckedBodyCrashSummary, CheckedCrashCallSite, CheckedCrashSite, CheckedEntryResourceEnvelope,
    CheckedMachineResourceEnvelopes, CheckedResourceAxisAnchor,
    CheckedResourceDerivationObligation, CheckedRuntimeMembershipSubject, CheckedTrappingSite,
    ClosedFloatRangeRequirement, ClosedIntegerRangeRequirement, ClosedScalarContractValue,
    ClosedScalarValueContractPlan, CrashCallSiteLocation, CrashCause, CrashContractCapsule,
    CrashInterface, CrashPlan, CrashPredicateExpression, CrashPredicateIdentity, CrashRouteBucket,
    CrashRouteBucketId, CrashRouteGuard, CrashSiteLocation, DeclaredCallableContract,
    DeclaredContractClause, DeclaredContractFact, DeclaredContractKind, DeclaredContractParameter,
    DeclaredContractStructure, DeclaredProgressPremise, DeclaredTermination,
    MachineContractCommitment, MachineContractIdentity, MachineContractPlan, MachineContractPlans,
    RealizedMachineContractEnvelope, contract_declarations_commitment, contract_identity,
    contract_plan_by_machine, declared_termination_commitment,
};
pub use domains::{DomainDependencyFact, DomainDependencyPathFact, DomainFacts};
pub use dynamic_conformances::{
    DynamicConformanceFacts, DynamicConformanceRowFact, DynamicConformanceRowSource,
    DynamicSelectionFact,
};
pub use index_compatibility::{
    IndexCompatibilityDischarge, IndexCompatibilityFact, IndexCompatibilityFacts,
};
pub use mutation::{
    MachineMutationFact, MutationFacts, PerformedIntegerWrite, StateWriteFramePlan,
};
pub use nominal_machine_uses::{
    CheckedCallbackResourceReceipt, CheckedMachineContractEnvelopeIdentity,
    CheckedMachineContractRefinement, CheckedNominalMachineUse, NominalMachineUseFacts,
    NominalMachineUseSite,
};
pub use placement::CheckedAccessViewInput;
pub use provider_closed_requirements::{
    CheckedProviderClosedRequirement, CheckedProviderClosedRequirements,
};
pub use provider_instance_publication::{
    CheckedProviderCandidate, CheckedProviderCandidateDeclaration,
    CheckedProviderInstanceDeclaration, CheckedProviderInstancePublication,
    CheckedProviderInstanceRequirement, CheckedProviderReceivingApplication,
    CheckedProviderReceivingDescription,
};
pub use qualifications::{
    CheckedScalarReturnOrigin, CheckedScalarReturnQualification, CheckedValueQualification,
    MachineQualifications, QualificationFacts, VacuousQualificationUse,
};
pub use reference_custody::{ReferenceCustodyFacts, ReferenceCustodyRow};
pub use requirement_call_specializations::{
    CheckedRequirementCallMachineSelection, CheckedRequirementCallSpecialization,
    CheckedRequirementCallTypeBinding, RequirementCallSpecializationFacts,
};
pub use resolved_types::{ResolvedCarrierType, ResolvedExpressionType, ResolvedTypeFacts};
pub use service_reaches::{
    CallServiceReachRows, MachineServiceReachRows, ServiceReachFacts, StateServiceReachRows,
};
pub use state_locals::{CheckedStateLocal, StateLocalFacts};
pub use suspensions::{MachineSuspensionFact, SuspensionFacts};
pub use synchronous_invocations::{MachineSynchronousInvocationFact, SynchronousInvocationFacts};
pub use termination::{
    CheckedDeclarationCall, CheckedDeclarationDispatchArm, CheckedMachineRankCorrespondence,
    CheckedProofRankingRelation, CheckedProofRecursiveCallSite, CheckedProofRecursiveComponent,
    CheckedProofRecursiveEdge, CheckedProofRecursiveMember, CheckedProofRecursiveTransitionLane,
    CheckedRankApplicationFacts, CheckedRankApplicationFormation,
    CheckedRankApplicationFormationHandle, CheckedRankApplicationInputHome, CheckedRankArrivalEdge,
    CheckedRankArrivalPlan, CheckedRankBodyJudgment, CheckedRankCoordinate,
    CheckedRankCoordinateBinding, CheckedRankCoordinateHandle, CheckedRankCoordinateTransport,
    CheckedRankLocalCapture, CheckedRankObservation, CheckedRankOperand, CheckedRankPremiseRegime,
    CheckedRankReceivingCoordinate, CheckedRankValueDerivation, CheckedRuntimeIntegerRank,
    CheckedRuntimeNaturalState, CheckedRuntimeRankComponent, CheckedRuntimeRankEdge,
    CheckedRuntimeRankInput, CheckedRuntimeRankMember, CheckedSourceNaturalRankComponent,
    CheckedSourceNaturalRankEdge, CheckedSourceNaturalRankMember, CheckedStateRankCorrespondence,
    MachineTerminationFact, TerminationFacts,
};
