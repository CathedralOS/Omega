/*
We define checked Terminal lowering plans (`flow::terminal`), including affine
cleanup, atomic access, boundary machines, calling signatures, place loans,
result bindings, scalar states, and structural types/arguments/control in
`CheckedTrees`.
*/

pub(crate) mod affine_cleanup_plans {

    use super::CheckedUnitEffectOperationPlan::ReleaseReference;
    use crate::CheckedMemberIdentity;
    use crate::flow::terminal::{
        CheckedUnitEffectMachinePlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralPathSegment,
    };
    use language_semantics::SemanticDomainId;
    use symbols::SymbolHandle;
    use typed_trees::type_identity::TypeIdentityId;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCleanupControlPlans {
        pub conditions: arena::Arena<CheckedCleanupCaseCondition>,
        pub paths: arena::Arena<CheckedUnitStructuralPathSegment>,
        pub normal_exit_rows: arena::Arena<CheckedNormalExitCleanupPlan>,
        pub normal_result_completions: arena::Arena<CheckedNormalResultCompletion>,
        pub normal_result_arms: arena::Arena<CheckedNormalResultArm>,
        pub value_completions: arena::Arena<CheckedValueCompletionPlan>,
        pub actions: arena::Arena<CheckedNormalCleanupAction>,
        pub normal_exit_regions: arena::Arena<super::CheckedUnitStructuralArgumentPlan>,
        pub reference_owner_cleanups: arena::Arena<CheckedReferenceOwnerCleanup>,
        pub partitions: arena::Arena<CheckedCleanupPartitionPlan>,
        pub partition_nodes: arena::Arena<CheckedCleanupPartitionNode>,
        pub partition_steps: arena::Arena<CheckedCleanupPartitionStep>,
        pub partition_arms: arena::Arena<CheckedCleanupPartitionArm>,
        pub partition_refs: arena::Arena<CheckedCleanupPartitionRef>,
    }

    /// Keep the proof that these cleanup actions finish this specific value.
    /// The tree records which case-specific and shared actions must complete
    /// before its branches join. The disposition identifies its exact source,
    /// completion, storage version and excluded transfers.
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCleanupPartitionPlan {
        pub disposition: arena::Handle<crate::ContextualCleanupDisposition>,
        pub source: CheckedUnitStructuralArgumentSourcePlan,
        pub path: arena::HandleSpan<CheckedUnitStructuralPathSegment>,
        pub type_identity: TypeIdentityId,
        pub root: arena::Handle<CheckedCleanupPartitionNode>,
        pub actions: arena::HandleSpan<CheckedNormalCleanupAction>,
    }

    impl Default for CheckedCleanupPartitionPlan {
        fn default() -> Self {
            Self {
                disposition: Default::default(),
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    value_dependency: None,
                    expression: arena::Handle::invalid(),
                    logical_root_type_identity: None,
                },
                path: Default::default(),
                type_identity: Default::default(),
                root: Default::default(),
                actions: Default::default(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCleanupPartitionRef {
        pub partition: arena::Handle<CheckedCleanupPartitionPlan>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCleanupPartitionLeaf {
        pub partition: arena::Handle<CheckedCleanupPartitionPlan>,
        pub ordinal: u32,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCleanupPartitionStep {
        pub node: arena::Handle<CheckedCleanupPartitionNode>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedCleanupPartitionNode {
        Scope {
            path: arena::HandleSpan<CheckedUnitStructuralPathSegment>,
            type_identity: TypeIdentityId,
            conditions: arena::HandleSpan<CheckedCleanupCaseCondition>,
            steps: arena::HandleSpan<CheckedCleanupPartitionStep>,
        },
        Leaf {
            ordinal: u32,
        },
        Cases {
            path: arena::HandleSpan<CheckedUnitStructuralPathSegment>,
            type_identity: TypeIdentityId,
            arms: arena::HandleSpan<CheckedCleanupPartitionArm>,
            common: arena::HandleSpan<CheckedCleanupPartitionStep>,
        },
    }

    impl Default for CheckedCleanupPartitionNode {
        fn default() -> Self {
            Self::Leaf { ordinal: 0 }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCleanupPartitionArm {
        pub case_identity: CheckedMemberIdentity,
        pub reachability: CheckedCleanupCaseReachability,
        pub steps: arena::HandleSpan<CheckedCleanupPartitionStep>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedCleanupCaseReachability {
        #[default]
        Runtime,
        Selected,
        Inactive,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedReferenceOwnerCleanup {
        pub partitions: arena::HandleSpan<CheckedCleanupPartitionRef>,
        pub disposition: arena::Handle<crate::ContextualCleanupDisposition>,
        pub owner: super::CheckedUnitStructuralArgumentPlan,
        pub actions: arena::HandleSpan<CheckedNormalCleanupAction>,
    }

    impl CheckedCleanupControlPlans {
        pub fn operation_actions<'plan>(
            &'plan self,
            operation: &'plan super::CheckedUnitEffectOperationPlan,
        ) -> impl Iterator<Item = &'plan CheckedNormalCleanupAction> {
            let direct = match operation {
                super::CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    cleanup_actions,
                    ..
                } => cleanup_actions.as_slice(),
                _ => &[],
            };
            let owners = match operation {
                ReleaseReference { owner_cleanups, .. } => *owner_cleanups,
                _ => arena::HandleSpan::empty(),
            };
            direct.iter().chain(
                self.reference_owner_cleanups
                    .span_or_empty(owners)
                    .iter()
                    .flat_map(|owner| self.actions.span_or_empty(owner.actions)),
            )
        }

        pub fn normal_exits(
            &self,
            rows: CheckedNormalExitCleanups,
        ) -> &[CheckedNormalExitCleanupPlan] {
            self.normal_exit_rows.span_or_empty(rows.rows)
        }

        pub fn normal_actions(
            &self,
            rows: CheckedNormalExitCleanups,
        ) -> impl Iterator<Item = &CheckedNormalCleanupAction> {
            self.normal_exits(rows).iter().flat_map(|row| {
                self.actions
                    .span_or_empty(row.actions)
                    .iter()
                    .chain(self.result_actions(row.result_completion))
            })
        }

        pub fn result_actions(
            &self,
            result: arena::Handle<CheckedNormalResultCompletion>,
        ) -> impl Iterator<Item = &CheckedNormalCleanupAction> {
            let mut pending = if result.is_valid() {
                vec![result]
            } else {
                Vec::new()
            };
            let mut actions = self
                .actions
                .span_or_empty(arena::HandleSpan::empty())
                .iter();
            std::iter::from_fn(move || {
                loop {
                    if let Some(action) = actions.next() {
                        return Some(action);
                    }
                    match self.normal_result_completions.get(pending.pop()?) {
                        CheckedNormalResultCompletion::NoNormal { .. } => {}
                        CheckedNormalResultCompletion::Value {
                            actions: selected, ..
                        } => {
                            actions = self.actions.span_or_empty(*selected).iter();
                        }
                        CheckedNormalResultCompletion::Dispatch { arms, .. } => {
                            pending.extend(
                                self.normal_result_arms
                                    .span_or_empty(*arms)
                                    .iter()
                                    .rev()
                                    .map(|arm| arm.result),
                            );
                        }
                    }
                }
            })
        }

        pub fn value_completion(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement_index: u32,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<&CheckedValueCompletionPlan> {
            self.value_completions.iter().find_map(|(_, row)| {
                (row.machine == machine
                    && row.state == state
                    && row.statement_index == statement_index
                    && row.expression == expression)
                    .then_some(row)
            })
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedValueCompletionPlan {
        pub partitions: arena::HandleSpan<CheckedCleanupPartitionRef>,
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub statement_index: u32,
        pub expression: typed_trees::expression::ExpressionHandle,
        pub actions: arena::HandleSpan<CheckedNormalCleanupAction>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCleanupCaseCondition {
        pub path: arena::HandleSpan<CheckedUnitStructuralPathSegment>,
        pub type_identity: TypeIdentityId,
        pub case_identity: CheckedMemberIdentity,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitPartialAffineDiscardPlan {
        pub partition_leaf: CheckedCleanupPartitionLeaf,
        pub source: CheckedUnitStructuralArgumentSourcePlan,
        pub path: Vec<CheckedUnitStructuralPathSegment>,
        pub type_identity: TypeIdentityId,
        pub case_conditions: arena::HandleSpan<CheckedCleanupCaseCondition>,
    }

    impl CheckedUnitPartialAffineDiscardPlan {
        fn same_effect(&self, other: &Self) -> bool {
            self.source == other.source
                && self.path == other.path
                && self.type_identity == other.type_identity
                && self.case_conditions == other.case_conditions
        }
    }

    impl Default for CheckedUnitPartialAffineDiscardPlan {
        fn default() -> Self {
            Self {
                partition_leaf: Default::default(),
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    value_dependency: None,
                    expression: arena::Handle::invalid(),
                    logical_root_type_identity: None,
                },
                path: Vec::new(),
                type_identity: TypeIdentityId::default(),
                case_conditions: Default::default(),
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedPartialAffineUnitCleanupPlans {
        pub machines: Vec<CheckedPartialAffineUnitCleanupMachinePlan>,
    }

    impl CheckedPartialAffineUnitCleanupPlans {
        pub fn for_machine(
            &self,
            machine: SymbolHandle,
        ) -> Option<&CheckedPartialAffineUnitCleanupMachinePlan> {
            self.machines
                .iter()
                .find(|plan| plan.machine.machine == machine)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedPartialAffineUnitCleanupMachinePlan {
        pub machine: CheckedUnitEffectMachinePlan,
        pub residual_affine_discards: Vec<CheckedUnitPartialAffineDiscardPlan>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedNominalAffineUnitCleanupPlans {
        pub machines: Vec<CheckedNominalAffineUnitCleanupMachinePlan>,
    }

    impl CheckedNominalAffineUnitCleanupPlans {
        pub fn for_machine(
            &self,
            machine: SymbolHandle,
        ) -> Option<&CheckedNominalAffineUnitCleanupMachinePlan> {
            self.machines.iter().find(|plan| plan.machine == machine)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedNominalAffineUnitCleanupMachinePlan {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub caller_requirements: Vec<CheckedUnitNominalAffineCallerRequirementPlan>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitNominalAffineCallerRequirementPlan {
        pub source_parameter_index: u32,
        pub field_identity: CheckedMemberIdentity,
        pub expected: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitNominalAffineCleanupPlan {
        pub partition_leaf: CheckedCleanupPartitionLeaf,
        pub consumed_claims: Vec<language_semantics::PermissionClaimIdentity>,
        pub source: CheckedUnitStructuralArgumentSourcePlan,
        pub path: Vec<CheckedUnitStructuralPathSegment>,
        pub type_identity: TypeIdentityId,
        pub cleanup_machine: SymbolHandle,
        pub cleanup_state: SymbolHandle,
        pub cleanup_contract_report_fingerprint: u64,
        pub case_conditions: arena::HandleSpan<CheckedCleanupCaseCondition>,
    }

    impl CheckedUnitNominalAffineCleanupPlan {
        fn same_effect(&self, other: &Self) -> bool {
            self.consumed_claims == other.consumed_claims
                && self.source == other.source
                && self.path == other.path
                && self.type_identity == other.type_identity
                && self.cleanup_machine == other.cleanup_machine
                && self.cleanup_state == other.cleanup_state
                && self.cleanup_contract_report_fingerprint
                    == other.cleanup_contract_report_fingerprint
                && self.case_conditions == other.case_conditions
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CheckedUnitNominalAffineCleanupRequirementPlan {
        pub field_identity: CheckedMemberIdentity,
        pub expected: bool,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitClaimTransferPlan {
        pub claim_identity: language_semantics::PermissionClaimIdentity,
        pub argument_index: u32,
        pub path: Vec<CheckedUnitStructuralPathSegment>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralCallCustodyPlan {
        pub reference_loans: arena::HandleSpan<crate::BorrowLoanFact>,
        pub retained_reference_inputs: Vec<u32>,
        pub forwarded_reference_sources: Vec<crate::CheckedReturnedReferencePlan>,
        pub result_qualifications: Vec<SemanticDomainId>,
        pub claim_transfers: Vec<CheckedUnitClaimTransferPlan>,
        pub result_claims: Vec<CheckedStructuralResultClaimPlan>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralResultClaimPlan {
        pub claim: language_semantics::PermissionClaimIdentity,
        pub path: Vec<CheckedUnitStructuralPathSegment>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedNormalCleanupAction {
        Discard(CheckedUnitPartialAffineDiscardPlan),
        InvokeNominal(CheckedUnitNominalAffineCleanupPlan),
        RetireHome {
            source: CheckedUnitStructuralArgumentSourcePlan,
            type_identity: TypeIdentityId,
        },
    }

    impl Default for CheckedNormalCleanupAction {
        fn default() -> Self {
            Self::Discard(CheckedUnitPartialAffineDiscardPlan::default())
        }
    }

    impl CheckedNormalCleanupAction {
        /// Compare the original action when locating its existing call schedule.
        /// Full record equality also compares the retained partition association.
        pub fn same_effect(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::Discard(left), Self::Discard(right)) => left.same_effect(right),
                (Self::InvokeNominal(left), Self::InvokeNominal(right)) => left.same_effect(right),
                _ => self == other,
            }
        }

        pub fn partition_leaf(&self) -> CheckedCleanupPartitionLeaf {
            match self {
                Self::Discard(discard) => discard.partition_leaf,
                Self::InvokeNominal(cleanup) => cleanup.partition_leaf,
                Self::RetireHome { .. } => Default::default(),
            }
        }

        pub fn case_conditions(&self) -> arena::HandleSpan<CheckedCleanupCaseCondition> {
            match self {
                Self::Discard(discard) => discard.case_conditions,
                Self::InvokeNominal(cleanup) => cleanup.case_conditions,
                Self::RetireHome { .. } => Default::default(),
            }
        }

        pub fn nominal(&self) -> Option<&CheckedUnitNominalAffineCleanupPlan> {
            match self {
                Self::Discard(_) | Self::RetireHome { .. } => None,
                Self::InvokeNominal(cleanup) => Some(cleanup),
            }
        }
        pub fn path(&self) -> &[CheckedUnitStructuralPathSegment] {
            match self {
                Self::Discard(discard) => &discard.path,
                Self::InvokeNominal(cleanup) => &cleanup.path,
                Self::RetireHome { .. } => &[],
            }
        }

        pub fn type_identity(&self) -> TypeIdentityId {
            match self {
                Self::Discard(discard) => discard.type_identity,
                Self::InvokeNominal(cleanup) => cleanup.type_identity,
                Self::RetireHome { type_identity, .. } => *type_identity,
            }
        }
        pub fn source(&self) -> &CheckedUnitStructuralArgumentSourcePlan {
            match self {
                Self::Discard(discard) => &discard.source,
                Self::InvokeNominal(cleanup) => &cleanup.source,
                Self::RetireHome { source, .. } => source,
            }
        }
    }

    impl CheckedUnitNominalAffineCleanupPlan {
        pub fn source_parameter_index(&self) -> Option<u32> {
            match self.source {
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                    Some(parameter_index)
                }
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedNormalCompletion {
        #[default]
        Possible,
        NoNormal,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedNormalExitCleanups {
        pub completion: CheckedNormalCompletion,
        pub rows: arena::HandleSpan<CheckedNormalExitCleanupPlan>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedNormalExitCleanupPlan {
        pub partitions: arena::HandleSpan<CheckedCleanupPartitionRef>,
        pub selected_regions: arena::HandleSpan<super::CheckedUnitStructuralArgumentPlan>,
        pub exit: crate::FlowClaimJoinExit,
        pub actions: arena::HandleSpan<CheckedNormalCleanupAction>,
        /// A selected scalar result completes its admitted transaction before the
        /// private result join. Zero retains the ordinary single-edge actions.
        pub result_completion: arena::Handle<CheckedNormalResultCompletion>,
    }

    /// Source-owned scalar result control with normal disposal at completed leaves.
    /// Subjects, patterns and operand computations remain in their ordinary plans.
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedNormalResultCompletion {
        /// The evaluated prefix remains executable but has no normal disposal edge.
        NoNormal {
            computation: crate::CheckedScalarComputationHandle,
        },
        Value {
            computation: crate::CheckedScalarComputationHandle,
            actions: arena::HandleSpan<CheckedNormalCleanupAction>,
            partitions: arena::HandleSpan<CheckedCleanupPartitionRef>,
        },
        Dispatch {
            computation: crate::CheckedScalarComputationHandle,
            arms: arena::HandleSpan<CheckedNormalResultArm>,
        },
    }

    impl Default for CheckedNormalResultCompletion {
        fn default() -> Self {
            Self::Value {
                computation: Default::default(),
                actions: Default::default(),
                partitions: Default::default(),
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedNormalResultArm {
        pub arm: arena::Handle<crate::CheckedScalarDispatchArm>,
        pub result: arena::Handle<CheckedNormalResultCompletion>,
    }
}
pub(crate) mod atomic_plans {
    use crate::CheckedMemberIdentity;
    use crate::flow::terminal::{
        CheckedCallScalarArgument, CheckedUnitScalarResultBindingPlan,
        CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralResultBindingPlan,
    };
    use language_core::atomic::MemoryOrdering;
    use typed_trees::types::PrimitiveType;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum CheckedAtomicReadModifyWrite {
        FetchAdd,
        FetchSub,
        FetchAnd,
        FetchOr,
        FetchXor,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedAtomicEvent {
        Load {
            ordering: MemoryOrdering,
        },
        Store {
            ordering: MemoryOrdering,
            value: CheckedCallScalarArgument,
        },
        ReadModifyWrite {
            operation: CheckedAtomicReadModifyWrite,
            ordering: MemoryOrdering,
            operand: CheckedCallScalarArgument,
        },
        Swap {
            ordering: MemoryOrdering,
            value: CheckedCallScalarArgument,
        },
        CompareExchange {
            success: MemoryOrdering,
            failure: MemoryOrdering,
            expected: CheckedCallScalarArgument,
            replacement: CheckedCallScalarArgument,
        },
        CompareExchangeOnce {
            custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
            success: MemoryOrdering,
            failure: MemoryOrdering,
            expected: CheckedCallScalarArgument,
            replacement: CheckedCallScalarArgument,
        },
    }

    impl CheckedAtomicEvent {
        pub const fn observes_resident(&self) -> bool {
            !matches!(self, Self::Store { .. })
        }

        pub const fn modifies_resident(&self) -> bool {
            !matches!(self, Self::Load { .. })
        }

        pub fn operands(&self) -> Vec<&CheckedCallScalarArgument> {
            match self {
                Self::Load { .. } => Vec::new(),
                Self::Store { value, .. } | Self::Swap { value, .. } => vec![value],
                Self::ReadModifyWrite { operand, .. } => vec![operand],
                Self::CompareExchange {
                    expected,
                    replacement,
                    ..
                }
                | Self::CompareExchangeOnce {
                    expected,
                    replacement,
                    ..
                } => vec![expected, replacement],
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedAtomicAccessPlan {
        pub statement_index: u32,
        pub resident_range_decision: arena::Handle<crate::CheckedIntegerAssignmentDecision>,
        pub receiver: CheckedUnitStructuralArgumentPlan,
        pub receiver_root_type_identity: typed_trees::type_identity::TypeIdentityId,
        pub field_case: Option<CheckedMemberIdentity>,
        pub field_identity: CheckedMemberIdentity,
        pub primitive_type: PrimitiveType,
        pub event: CheckedAtomicEvent,
        pub result: Option<CheckedUnitScalarResultBindingPlan>,
        pub structural_result: Option<CheckedUnitStructuralResultBindingPlan>,
        pub discard_result_on_return: bool,
    }
}
pub(crate) mod boundary_machine_plans {
    use crate::flow::terminal::{
        CheckedStructuralScalarParameterPlan, CheckedUnitStructuralDomainRequirementPlan,
        CheckedUnitStructuralParameterPlan,
    };
    use language_semantics::{
        Multiplicity, SemanticDomainId, ServiceReachPlan, ServiceReachSummary,
    };
    use symbols::SymbolHandle;
    use typed_trees::type_identity::TypeIdentityId;
    use typed_trees::types::PrimitiveType;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedBoundaryMachineResultPlan {
        Unit,
        Scalar(PrimitiveType),
        Structural {
            type_identity: TypeIdentityId,
            multiplicity: Multiplicity,
            qualifications: Vec<SemanticDomainId>,
            projected_qualifications: Vec<super::CheckedStructuralPathQualification>,
        },
    }

    impl CheckedBoundaryMachineResultPlan {
        pub const fn scalar(&self) -> Option<PrimitiveType> {
            match self {
                Self::Scalar(scalar) => Some(*scalar),
                Self::Unit | Self::Structural { .. } => None,
            }
        }

        pub const fn is_unit(&self) -> bool {
            matches!(self, Self::Unit)
        }

        pub fn structural_identity(&self) -> Option<TypeIdentityId> {
            match self {
                Self::Structural { type_identity, .. } => Some(*type_identity),
                Self::Unit | Self::Scalar(_) => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedBoundaryScalarGuarantee {
        Predicate(crate::CheckedBooleanExpression),
        Implication {
            premise: crate::CheckedBooleanExpression,
            conclusion: crate::CheckedBooleanExpression,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedBoundaryReferenceOutput {
        pub destination: CheckedBoundaryReferenceDestination,
        pub path: Vec<super::CheckedUnitStructuralPathSegment>,
        pub access: language_semantics::ReferenceAccess,
        pub dependencies: Vec<CheckedBoundaryLifetimeDependency>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedBoundaryReferenceDestination {
        Parameter(u32),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedBoundaryLifetimeDependency {
        Referent {
            parameter_index: u32,
            access: language_semantics::ReferenceAccess,
        },
        Stored {
            parameter_index: u32,
            path: Vec<super::CheckedUnitStructuralPathSegment>,
            access: language_semantics::ReferenceAccess,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedBoundaryScalarDomainRequirementPlan {
        pub argument_index: u32,
        pub domain_symbol: SymbolHandle,
        pub domain: SemanticDomainId,
        pub primitive_type: PrimitiveType,
        pub predicate: crate::CheckedBooleanExpression,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedBoundaryExecutionPlan {
        Provider,
        AccessInitialize,
        AccessMoveOut,
        AccessView,
        AccessRelocate,
        EmptyRunRelease,
        StaticInterrupt(language_core::static_interrupts::StaticInterruptOperation),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedBoundaryMachinePlan {
        pub execution: CheckedBoundaryExecutionPlan,
        pub kind: semantic_vocabulary::BoundaryRequirementKind,
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub contract_owner: SymbolHandle,
        pub owning_service: Option<language_semantics::ServiceReachId>,
        pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
        pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
        pub result: CheckedBoundaryMachineResultPlan,
        pub result_reference_sources: Vec<crate::CheckedReturnedReferencePlan>,
        pub domain_requirements: Vec<CheckedUnitStructuralDomainRequirementPlan>,
        pub normal_qualifications: Vec<CheckedUnitStructuralDomainRequirementPlan>,
        pub scalar_domain_requirements: Vec<CheckedBoundaryScalarDomainRequirementPlan>,
        pub scalar_requires: Vec<crate::ClosedScalarContractValue>,
        pub pending_scalar_requires: Vec<typed_trees::signature::SignatureContract>,
        pub pending_scalar_recipes: Vec<typed_trees::constant_recipes::ConstantRecipeHandle>,
        pub scalar_ensures: Vec<CheckedBoundaryScalarGuarantee>,
        pub reference_outputs: Vec<CheckedBoundaryReferenceOutput>,
        pub contract_report_fingerprint: u64,
        pub contract_commitment: crate::MachineContractCommitment,
        pub contract_service_reach: ServiceReachPlan,
        pub service_reach: ServiceReachSummary,
    }

    impl CheckedBoundaryMachinePlan {
        pub fn callable_entry(&self) -> super::CheckedUnitCallableEntry<'_> {
            super::CheckedUnitCallableEntry {
                machine: self.machine,
                state: self.state,
                returns_unit: self.result.is_unit(),
                structural_parameters: &self.structural_parameters,
                scalar_parameters: &self.scalar_parameters,
                erased_scalar_parameters: &[],
                erased_proof_parameters: &[],
                entry_claims: &[],
                contract_report_fingerprint: self.contract_report_fingerprint,
                contract_service_reach: self.contract_service_reach,
            }
        }
    }
}
pub(crate) mod calling_signatures {
    use language_semantics::callback_declarations::BoundaryTelescope;
    use semantic_vocabulary::{CallingSignatureId, CallingStaticOperand};
    use typed_trees::type_identity::TypeIdentityId;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCallingSignaturePlan {
        pub id: CallingSignatureId,
        pub source_template: symbols::SymbolHandle,
        pub requirement_identity: String,
        pub declaring_application: String,
        pub operands: Vec<CallingStaticOperand>,
        pub formals: Vec<semantic_vocabulary::CallingGenericFormal>,
        pub default_eligible: bool,
        pub physical_owner: Result<semantic_vocabulary::CallingDeclarationOwner, String>,
        pub storage: Result<CheckedCallingSignatureStorage, String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCallingSignatureStorage {
        pub parameters: Vec<CheckedCallingParameter>,
        pub result: Option<TypeIdentityId>,
        pub telescope: BoundaryTelescope,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCallingParameter {
        pub type_identity: TypeIdentityId,
        pub callback_backing: Option<TypeIdentityId>,
        pub reference_custody: Option<TypeIdentityId>,
        pub access: super::CheckedStructuralAccess,
    }

    impl CheckedCallingSignatureStorage {
        pub fn type_roots(&self) -> impl Iterator<Item = TypeIdentityId> + '_ {
            self.parameters
                .iter()
                .flat_map(|parameter| {
                    std::iter::once(
                        parameter
                            .reference_custody
                            .unwrap_or(parameter.type_identity),
                    )
                    .chain(parameter.callback_backing)
                })
                .chain(self.result)
        }
    }
}
pub(crate) mod opaque_representations {
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedOpaqueRepresentation {
        pub opaque: typed_trees::type_identity::TypeIdentityId,
        pub carrier: typed_trees::type_identity::TypeIdentityId,
        pub application: language_semantics::opaque_representations::SelectedOpaqueApplication,
    }
}
pub(crate) use crate::flow_terminal_units::composed_unit_control_plans;
pub use calling_signatures::{
    CheckedCallingParameter, CheckedCallingSignaturePlan, CheckedCallingSignatureStorage,
};
pub use opaque_representations::CheckedOpaqueRepresentation;
pub(crate) mod dynamic_slot_plans {
    use crate::flow::terminal::{
        CheckedBoundaryMachineResultPlan, CheckedStructuralScalarParameterPlan,
        CheckedUnitStructuralParameterPlan,
    };
    use symbols::SymbolHandle;
    use typed_trees::type_identity::CallableIdentityId;
    use typed_trees::type_identity::TypeIdentityId;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedDynamicSlotPlan {
        pub slot: SymbolHandle,
        pub interface_identity: TypeIdentityId,
        pub declaring_trait: SymbolHandle,
        pub requirement: SymbolHandle,
        pub requirement_identity: CallableIdentityId,
        pub family_tuple: Vec<String>,
        pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
        pub entry_claim_paths: Vec<CheckedDynamicClaimPath>,
        pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
        pub result: CheckedBoundaryMachineResultPlan,
        pub result_claim_paths: Vec<Vec<crate::CheckedUnitStructuralPathSegment>>,
        pub result_reference_sources: Vec<crate::CheckedReturnedReferencePlan>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedDynamicClaimPath {
        pub argument_index: u32,
        pub path: Vec<crate::CheckedUnitStructuralPathSegment>,
    }
}
pub(crate) mod place_loan_plans {
    use super::{CheckedStructuralAccess, CheckedUnitStructuralPathSegment};
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedPlaceLoanPlan {
        pub statement_index: u32,
        pub symbol: SymbolHandle,
        pub access: CheckedStructuralAccess,
        pub referee: TypeReferenceHandle,
        pub root: CheckedPlaceLoanRoot,
        pub root_type: TypeReferenceHandle,
        pub path: Vec<CheckedUnitStructuralPathSegment>,
        pub recast_source: TypeReferenceHandle,
        pub decided_byte_view_store: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedPlaceLoanRoot {
        StructuralParameter { parameter_index: u32 },
        ScalarParameter { position: u32 },
        Local { symbol: SymbolHandle },
    }
}
pub(crate) mod result_binding_plans {
    use crate::CheckedMemberIdentity;
    use crate::flow::terminal::{CheckedStructuralAccess, CheckedUnitStructuralPathSegment};
    use language_semantics::Multiplicity;
    use symbols::SymbolHandle;
    use typed_trees::type_identity::TypeIdentityId;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitScalarResultBindingPlan {
        pub statement_index: u32,
        pub binding_ordinal: u32,
        pub primitive_type: PrimitiveType,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitStructuralResultBindingPlan {
        pub statement_index: u32,
        pub binding_ordinal: u32,
        pub type_identity: TypeIdentityId,
        pub multiplicity: Multiplicity,
        pub local: CheckedInitializedLocal,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedInitializedLocal {
        pub symbol: SymbolHandle,
        pub declared_type: TypeReferenceHandle,
        pub access: CheckedStructuralAccess,
    }

    impl CheckedInitializedLocal {
        pub fn is_named(&self) -> bool {
            self.symbol.is_valid()
        }
    }

    impl Default for CheckedInitializedLocal {
        fn default() -> Self {
            Self {
                symbol: SymbolHandle::invalid(),
                declared_type: TypeReferenceHandle::invalid(),
                access: CheckedStructuralAccess::Owned,
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedByteSequenceValuePlan {
        pub nodes: arena::Arena<CheckedByteSequenceValue>,
        pub root: arena::Handle<CheckedByteSequenceValue>,
    }

    impl CheckedByteSequenceValuePlan {
        pub fn literal(bytes: Vec<u8>) -> Self {
            let mut plan = Self::default();
            plan.root = plan.nodes.insert(CheckedByteSequenceValue::Literal(bytes));
            plan
        }

        pub fn completed_call_binding_ordinal(&self) -> Option<u32> {
            let value = self
                .nodes
                .iter()
                .find_map(|(identity, value)| (identity == self.root).then_some(value))?;
            match value {
                CheckedByteSequenceValue::CallResult {
                    binding_ordinal, ..
                } => Some(*binding_ordinal),
                CheckedByteSequenceValue::Descriptor { source, .. } => {
                    source.source_structural_result_binding_ordinal()
                }
                _ => None,
            }
        }

        pub fn literal_bytes(&self) -> Option<&[u8]> {
            self.nodes.iter().find_map(|(identity, value)| match value {
                CheckedByteSequenceValue::Literal(bytes) if identity == self.root => {
                    Some(bytes.as_slice())
                }
                _ => None,
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedByteSequenceValue {
        Literal(Vec<u8>),
        Descriptor {
            expression: typed_trees::expression::ExpressionHandle,
            source: crate::CheckedUnitStructuralArgumentPlan,
        },
        CallResult {
            state: symbols::SymbolHandle,
            statement_index: u32,
            binding_ordinal: u32,
            capacity: u64,
        },
        Whole {
            root: crate::CheckedStoreRoot,
            capacity: u64,
        },
        Field {
            root: crate::CheckedStoreRoot,
            carrier_path: Vec<CheckedUnitStructuralPathSegment>,
            field_identity: CheckedMemberIdentity,
            capacity: u64,
        },
        Concatenation {
            left: arena::Handle<CheckedByteSequenceValue>,
            right: arena::Handle<CheckedByteSequenceValue>,
            capacity: u64,
        },
    }

    impl Default for CheckedByteSequenceValue {
        fn default() -> Self {
            Self::Literal(Vec::new())
        }
    }
}
pub(crate) mod scalar_state_plans {
    use crate::flow::terminal::CheckedStructuralControlSuccessorPlan;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedGuardedExitPlans {
        pub exits: arena::Arena<CheckedScalarGuardedExit>,
        pub tails: Vec<CheckedScalarGuardedTail>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedScalarGuardedTail {
        pub state: SymbolHandle,
        pub arms: arena::HandleSpan<CheckedScalarGuardedExit>,
        pub fallback: Option<CheckedScalarBranchDestination>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedScalarGuardedExit {
        pub guard_statement_ordinal: u32,
        pub destination: CheckedScalarBranchDestination,
    }

    impl Default for CheckedScalarGuardedExit {
        fn default() -> Self {
            Self {
                guard_statement_ordinal: 0,
                destination: CheckedScalarBranchDestination::Return {
                    statement_ordinal: 0,
                    is_continuation: false,
                },
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedScalarStateTerminator {
        Guarded {
            arms: arena::HandleSpan<CheckedScalarGuardedExit>,
            fallback: Option<CheckedScalarBranchDestination>,
        },
        Return {
            statement_ordinal: u32,
        },
        Crash {
            statement_ordinal: u32,
        },
        Jump(CheckedStructuralControlSuccessorPlan),
        Conditional {
            guard_statement_ordinal: u32,
            when_true: CheckedScalarBranchDestination,
            when_false: CheckedScalarBranchDestination,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedScalarBranchDestination {
        InvokeNoNormal {
            statement_ordinal: u32,
            is_continuation: bool,
        },
        Jump(CheckedStructuralControlSuccessorPlan),
        Crash {
            statement_ordinal: u32,
        },
        Return {
            statement_ordinal: u32,
            is_continuation: bool,
        },
    }

    pub struct ScalarReturnPath<'a> {
        pub destination: CheckedScalarBranchDestination,
        guarded_prefix: &'a [CheckedScalarGuardedExit],
        conditional_guard: Option<u32>,
    }

    impl ScalarReturnPath<'_> {
        pub fn guard_ordinals(&self) -> impl Iterator<Item = u32> + '_ {
            self.conditional_guard.into_iter().chain(
                self.guarded_prefix
                    .iter()
                    .map(|arm| arm.guard_statement_ordinal),
            )
        }
    }

    impl CheckedGuardedExitPlans {
        pub fn returning_exits(
            &self,
            arms: arena::HandleSpan<CheckedScalarGuardedExit>,
            fallback: Option<&CheckedScalarBranchDestination>,
        ) -> Result<&[CheckedScalarGuardedExit], &'static str> {
            let arms = self
                .exits
                .span(arms)
                .ok_or("ordered scalar guard roster is stale")?;
            if arms.is_empty() {
                return Err("ordered scalar guard roster is empty");
            }
            if arms
                .iter()
                .map(|arm| &arm.destination)
                .chain(fallback)
                .any(|destination| {
                    !matches!(
                        destination,
                        CheckedScalarBranchDestination::Return {
                            is_continuation: false,
                            ..
                        }
                    )
                })
            {
                return Err("ordered scalar exit transfers or crashes instead of returning");
            }
            Ok(arms)
        }
    }

    impl CheckedScalarStateTerminator {
        pub fn returning_paths<'a>(
            &self,
            guarded: &'a CheckedGuardedExitPlans,
        ) -> Result<Vec<ScalarReturnPath<'a>>, &'static str> {
            let paths = match self {
                Self::Return { statement_ordinal } => vec![ScalarReturnPath {
                    destination: CheckedScalarBranchDestination::Return {
                        statement_ordinal: *statement_ordinal,
                        is_continuation: false,
                    },
                    guarded_prefix: &[],
                    conditional_guard: None,
                }],
                Self::Conditional {
                    guard_statement_ordinal,
                    when_true,
                    when_false,
                } => [when_true, when_false]
                    .into_iter()
                    .map(|destination| ScalarReturnPath {
                        destination: destination.clone(),
                        guarded_prefix: &[],
                        conditional_guard: Some(*guard_statement_ordinal),
                    })
                    .collect(),
                Self::Guarded { arms, fallback } => {
                    let arms = guarded.returning_exits(*arms, fallback.as_ref())?;
                    arms.iter()
                        .enumerate()
                        .map(|(index, arm)| ScalarReturnPath {
                            destination: arm.destination.clone(),
                            guarded_prefix: &arms[..=index],
                            conditional_guard: None,
                        })
                        .chain(fallback.iter().map(|destination| ScalarReturnPath {
                            destination: destination.clone(),
                            guarded_prefix: arms,
                            conditional_guard: None,
                        }))
                        .collect()
                }
                _ => return Err("scalar completion has no normal return destination"),
            };
            if paths.iter().any(|path| {
                !matches!(
                    path.destination,
                    CheckedScalarBranchDestination::Return { .. }
                )
            }) {
                return Err("scalar completion has a nonreturning destination");
            }
            Ok(paths)
        }
    }
}
pub(crate) mod structural_argument_plans {

    use crate::flow::terminal::{CheckedStructuralAccess, CheckedUnitStructuralPathSegment};
    use crate::{CheckedScalarExpression, CheckedStorageRoot};
    use language_semantics::CarryPolicy;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::type_identity::TypeIdentityId;
    use typed_trees::types::PrimitiveType;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, compiler_storage::Record)]
    pub enum CheckedReferenceStorage {
        #[default]
        Declared,
        Access,
        ResidentAccess {
            extent: SymbolHandle,
            granted: language_semantics::SemanticDomainId,
            resident: language_semantics::SemanticDomainId,
        },
        View {
            stored_scalar: Option<semantic_vocabulary::ScalarType>,
        },
    }

    impl Ord for CheckedReferenceStorage {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            let key = |storage: &Self| match storage {
                Self::Declared => (0, 0, 0, 0, 0, None),
                Self::Access => (1, 0, 0, 0, 0, None),
                Self::ResidentAccess {
                    extent,
                    granted,
                    resident,
                } => (
                    2,
                    extent.arena_index(),
                    extent.generation(),
                    granted.0,
                    resident.0,
                    None,
                ),
                Self::View { stored_scalar } => (3, 0, 0, 0, 0, *stored_scalar),
            };
            key(self).cmp(&key(other))
        }
    }

    impl PartialOrd for CheckedReferenceStorage {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitEntryClaimPlan {
        pub claim_identity: language_semantics::PermissionClaimIdentity,
        pub parameter_index: u32,
        pub path: Vec<CheckedUnitStructuralPathSegment>,
        pub carry: CarryPolicy,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitCallCoordinate {
        pub statement_index: u32,
        pub call_ordinal: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedCallScalarArgument {
        Pure(CheckedScalarExpression),
        Computation(crate::CheckedScalarComputationHandle),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralScalarParameterPlan {
        pub source_position: u32,
        pub primitive_type: PrimitiveType,
    }

    impl CheckedCallScalarArgument {
        pub fn as_pure(&self) -> Option<&CheckedScalarExpression> {
            match self {
                Self::Pure(expression) => Some(expression),
                Self::Computation(_) => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitStructuralArgumentSourcePlan {
        CapturedStorage {
            selection: arena::Handle<crate::FlowOwnedSelectionReceipt>,
            source_ordinal: u32,
        },
        Parameter {
            parameter_index: u32,
        },
        ScalarParameter {
            parameter_index: u32,
        },
        PrimitiveLocal {
            symbol: SymbolHandle,
        },
        StructuralLocal {
            symbol: SymbolHandle,
        },
        StructuralLocalBorrow {
            symbol: SymbolHandle,
            root_type_identity: TypeIdentityId,
            root_reference_hops: u32,
        },
        /// An already evaluated structural expression. The enclosing operation
        /// sequence establishes its storage before a consumer observes it; this
        /// identity never authorizes evaluating the expression again.
        StructuralExpression {
            /// Exact completed value retained by the producing owner. `None`
            /// names already established storage, not a request to discover or
            /// evaluate a producer from the expression home.
            value_dependency: Option<crate::CheckedStructuralValueHandle>,
            expression: ExpressionHandle,
            /// A projection of this completed owner can start at its checked
            /// logical referent rather than the stored descriptor. This identity
            /// comes from the value producer; it never authorizes an owned move.
            logical_root_type_identity: Option<TypeIdentityId>,
        },
        /// Dense declaration ordinal in the caller's checked trivial-affine-local
        /// table. This source is always the exact whole local.
        TrivialAffineLocal {
            declaration_ordinal: u32,
        },
        /// Exact binding ordinal of an earlier whole structural call result.
        StructuralResult {
            binding_ordinal: u32,
        },
        /// Exact byte sequence passed directly to a bodyless boundary.
        ByteSequenceLiteral {
            bytes: Vec<u8>,
        },
        /// Exclusive range over one established immutable byte view: a whole
        /// byte-view parameter (dense, state-local structural index) or an
        /// earlier byte-view local. Missing endpoints mean zero and the source
        /// length, not absent runtime values. Present endpoints retain the unique
        /// scalar operand under `SubsliceStart`/`SubsliceEnd` at the enclosing
        /// statement and this range's `CheckedSubsliceSite`. Calls remain computed
        /// operands, evaluated once in start/end order before narrowing the view.
        ByteSequenceSubslice {
            root: CheckedStorageRoot,
            root_type_identity: TypeIdentityId,
            expression: ExpressionHandle,
            start: Option<CheckedCallScalarArgument>,
            end: Option<CheckedCallScalarArgument>,
            end_inclusive: bool,
            site: crate::CheckedSubsliceSite,
            storage_type_identity: TypeIdentityId,
        },
        ElementViewSubslice {
            root: CheckedStorageRoot,
            root_type_identity: TypeIdentityId,
            expression: ExpressionHandle,
            start: Option<CheckedCallScalarArgument>,
            end: Option<CheckedCallScalarArgument>,
            end_inclusive: bool,
            site: crate::CheckedSubsliceSite,
            storage_type_identity: TypeIdentityId,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitStructuralArgumentPlan {
        pub source: CheckedUnitStructuralArgumentSourcePlan,
        pub path: Vec<CheckedUnitStructuralPathSegment>,
        pub projection_evaluations: arena::HandleSpan<crate::CheckedScalarComputationHandle>,
        pub type_identity: TypeIdentityId,
        pub access: CheckedStructuralAccess,
    }

    impl Default for CheckedUnitStructuralArgumentPlan {
        fn default() -> Self {
            Self {
                source: CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                    symbol: SymbolHandle::invalid(),
                },
                path: Vec::new(),
                projection_evaluations: Default::default(),
                type_identity: TypeIdentityId::default(),
                access: CheckedStructuralAccess::Owned,
            }
        }
    }

    impl CheckedUnitStructuralArgumentPlan {
        pub fn call_projection(&self) -> &[super::CheckedUnitStructuralPathSegment] {
            match self.source {
                CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
                | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. } => &[],
                _ => &self.path,
            }
        }

        pub fn call_selector_positions(&self) -> impl Iterator<Item = usize> + '_ {
            let path = self.call_projection();
            let index_count = path
                .iter()
                .filter(|segment| Self::is_index(segment))
                .count();
            let inherited = index_count.saturating_sub(self.projection_evaluations.len());
            let mut ordinal = 0;
            path.iter()
                .enumerate()
                .filter_map(move |(position, segment)| {
                    let index = Self::is_index(segment);
                    let authored =
                        index && ordinal >= inherited && !self.projection_evaluations.is_empty();
                    ordinal += usize::from(index);
                    (authored
                        || matches!(
                            segment,
                            CheckedUnitStructuralPathSegment::ComputedIndex(_)
                                | CheckedUnitStructuralPathSegment::RuntimeIndex(
                                    super::CheckedRuntimeIndex::StructuralField(_)
                                        | super::CheckedRuntimeIndex::StorageRead { .. }
                                )
                        ))
                    .then_some(position)
                })
        }

        fn is_index(segment: &CheckedUnitStructuralPathSegment) -> bool {
            matches!(
                segment,
                CheckedUnitStructuralPathSegment::FixedIndex(_)
                    | CheckedUnitStructuralPathSegment::ComputedIndex(_)
                    | CheckedUnitStructuralPathSegment::RuntimeIndex(_)
            )
        }

        pub fn retain_projection_evaluations(
            &mut self,
            evaluations: arena::HandleSpan<crate::CheckedScalarComputationHandle>,
            operands: &arena::Arena<crate::CheckedScalarComputationHandle>,
        ) -> Option<()> {
            if !Self::projection_evaluations_match(&self.path, evaluations, operands) {
                return None;
            }
            self.projection_evaluations = evaluations;
            Some(())
        }

        pub fn projection_evaluations_match(
            path: &[CheckedUnitStructuralPathSegment],
            evaluations: arena::HandleSpan<crate::CheckedScalarComputationHandle>,
            operands: &arena::Arena<crate::CheckedScalarComputationHandle>,
        ) -> bool {
            let Some(children) = (if evaluations.is_empty() {
                Some(&[][..])
            } else {
                operands.span(evaluations)
            }) else {
                return false;
            };
            let indexes = path
                .iter()
                .filter(|segment| Self::is_index(segment))
                .count();
            let Some(inherited) = indexes.checked_sub(children.len()) else {
                return false;
            };
            path.iter()
                .filter(|segment| Self::is_index(segment))
                .skip(inherited)
                .zip(children)
                .all(|(segment, child)| {
                    child.is_valid()
                        && match segment {
                            CheckedUnitStructuralPathSegment::FixedIndex(_) => true,
                            CheckedUnitStructuralPathSegment::ComputedIndex(expected) => {
                                expected == child
                            }
                            _ => false,
                        }
                })
        }

        pub fn projection_computation_operands<'a>(
            path: &'a [CheckedUnitStructuralPathSegment],
            evaluations: arena::HandleSpan<crate::CheckedScalarComputationHandle>,
            operands: &'a arena::Arena<crate::CheckedScalarComputationHandle>,
        ) -> impl Iterator<Item = crate::CheckedScalarComputationHandle> + 'a {
            path.iter()
                .filter_map(move |segment| match segment {
                    CheckedUnitStructuralPathSegment::ComputedIndex(root)
                        if evaluations.is_empty() =>
                    {
                        Some(*root)
                    }
                    _ => None,
                })
                .chain(
                    evaluations
                        .handles()
                        .map(move |handle| *operands.get(handle)),
                )
        }

        pub fn selector_computation(
            &self,
            position: usize,
            operands: &arena::Arena<crate::CheckedScalarComputationHandle>,
        ) -> Option<crate::CheckedScalarComputationHandle> {
            let indexes = self
                .path
                .iter()
                .enumerate()
                .filter(|(_, segment)| Self::is_index(segment))
                .map(|(position, _)| position)
                .collect::<Vec<_>>();
            let inherited = indexes
                .len()
                .checked_sub(self.projection_evaluations.len())?;
            let ordinal = indexes
                .iter()
                .position(|candidate| *candidate == position)?;
            let ordinal = ordinal.checked_sub(inherited)?;
            operands
                .span(self.projection_evaluations)?
                .get(ordinal)
                .copied()
        }

        pub fn structural_result(
            binding_ordinal: u32,
            type_identity: TypeIdentityId,
            access: CheckedStructuralAccess,
        ) -> Self {
            Self {
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                },
                path: Vec::new(),
                projection_evaluations: Default::default(),
                type_identity,
                access,
            }
        }

        pub fn scalar_operands(&self) -> impl Iterator<Item = &CheckedCallScalarArgument> {
            let endpoints = match &self.source {
                CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    start,
                    end,
                    ..
                }
                | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                    start, end, ..
                } => [start.as_ref(), end.as_ref()],
                _ => [None, None],
            };
            endpoints.into_iter().flatten()
        }

        pub fn computation_operands<'a>(
            &'a self,
            operands: &'a arena::Arena<crate::CheckedScalarComputationHandle>,
        ) -> impl Iterator<Item = crate::CheckedScalarComputationHandle> + 'a {
            Self::projection_computation_operands(&self.path, self.projection_evaluations, operands)
                .chain(self.scalar_operands().filter_map(|operand| match operand {
                    CheckedCallScalarArgument::Computation(root) => Some(*root),
                    CheckedCallScalarArgument::Pure(_) => None,
                }))
        }

        pub fn structural_storage_root(&self) -> Option<facts::PlaceRoot> {
            match self.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
                | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
                    symbol, ..
                } => Some(facts::PlaceRoot::Symbol(symbol)),
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    expression,
                    ..
                } => Some(facts::PlaceRoot::Expression(expression)),
                _ => None,
            }
        }

        pub fn source_parameter_index(&self) -> Option<u32> {
            match self.source {
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                    Some(parameter_index)
                }
                _ => None,
            }
        }

        pub fn source_local_declaration_ordinal(&self) -> Option<u32> {
            match self.source {
                CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal {
                    declaration_ordinal,
                } => Some(declaration_ordinal),
                _ => None,
            }
        }

        pub fn source_structural_result_binding_ordinal(&self) -> Option<u32> {
            match self.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                    Some(binding_ordinal)
                }
                _ => None,
            }
        }

        pub fn subslice_storage_type_identity(&self) -> Option<TypeIdentityId> {
            match &self.source {
                CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    storage_type_identity,
                    ..
                }
                | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                    storage_type_identity,
                    ..
                } => storage_type_identity
                    .is_valid()
                    .then_some(*storage_type_identity),
                _ => None,
            }
        }

        pub fn borrowed_root_reference_hops(&self) -> u32 {
            match &self.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
                    root_reference_hops,
                    ..
                } => *root_reference_hops,
                _ => 0,
            }
        }

        pub fn subslice_root_type_identity(&self) -> Option<TypeIdentityId> {
            match &self.source {
                CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    root_type_identity,
                    ..
                }
                | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                    root_type_identity,
                    ..
                } => Some(*root_type_identity),
                _ => None,
            }
        }

        /// The logical root before a borrowed projection of a local or completed
        /// expression, distinct from its final type and published descriptor.
        pub fn borrowed_root_type_identity(&self) -> Option<TypeIdentityId> {
            match &self.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
                    root_type_identity,
                    ..
                } => Some(*root_type_identity),
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    logical_root_type_identity,
                    ..
                } => *logical_root_type_identity,
                _ => None,
            }
        }

        pub fn byte_sequence_literal(&self) -> Option<&[u8]> {
            match &self.source {
                CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } => {
                    Some(bytes)
                }
                _ => None,
            }
        }
    }

    impl CheckedUnitStructuralArgumentSourcePlan {
        /// The completed producer selected by this argument's checking owner.
        /// An existing storage capture has no new producer dependency.
        pub fn structural_value_dependency(&self) -> Option<crate::CheckedStructuralValueHandle> {
            match self {
                Self::StructuralExpression {
                    value_dependency, ..
                } => *value_dependency,
                _ => None,
            }
        }

        /// Exact completed-expression namespace, independent of projection presentation.
        pub fn structural_expression(&self) -> Option<ExpressionHandle> {
            match self {
                Self::StructuralExpression { expression, .. } => Some(*expression),
                _ => None,
            }
        }
    }
}
pub(crate) mod structural_control_plans {
    use crate::CheckedMemberIdentity;
    use crate::flow::terminal::{
        CheckedOperandSequence, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralPathSegment,
    };
    use crate::values::CheckedProofTerm;
    use symbols::SymbolHandle;
    use typed_trees::types::PrimitiveType;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralControlCleanupPlans {
        pub states: Vec<CheckedStructuralControlStateCleanupPlan>,
    }

    impl CheckedStructuralControlCleanupPlans {
        pub fn for_state(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
        ) -> Option<&CheckedStructuralControlStateCleanupPlan> {
            self.states
                .iter()
                .find(|plan| plan.machine == machine && plan.state == state)
        }

        pub fn for_edge(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement_ordinal: u32,
        ) -> Option<&CheckedStructuralControlEdgeCleanupPlan> {
            self.for_state(machine, state)?
                .edges
                .iter()
                .find(|edge| edge.statement_ordinal == statement_ordinal)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralControlStateCleanupPlan {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub edges: Vec<CheckedStructuralControlEdgeCleanupPlan>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralControlEdgeCleanupPlan {
        pub statement_ordinal: u32,
        pub target_state: SymbolHandle,
        pub trivial_affine_discard_parameter_positions: Vec<u32>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedSuccessorCompletion {
        StateArrival,
        ReturningInvocation,
        NoNormalInvocation,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralControlSuccessorPlan {
        pub statement_ordinal: u32,
        pub target_state: SymbolHandle,
        pub target_occurrence: Option<u32>,
        pub completion: CheckedSuccessorCompletion,
        pub is_continuation: bool,
        pub transfers: Vec<CheckedStructuralControlTransferPlan>,
        pub scalar_arguments: Vec<CheckedStructuralScalarArgumentPlan>,
        pub erased_arguments: Vec<CheckedStructuralScalarArgumentPlan>,
        pub erased_proof_arguments: Vec<CheckedProofTerm>,
        pub trivial_affine_discard_parameter_positions: Vec<u32>,
    }

    impl CheckedStructuralControlSuccessorPlan {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            statement_ordinal: u32,
            target_state: SymbolHandle,
            completion: CheckedSuccessorCompletion,
            is_continuation: bool,
            transfers: Vec<CheckedStructuralControlTransferPlan>,
            scalar_arguments: Vec<CheckedStructuralScalarArgumentPlan>,
            erased_arguments: Vec<CheckedStructuralScalarArgumentPlan>,
            erased_proof_arguments: Vec<CheckedProofTerm>,
            trivial_affine_discard_parameter_positions: Vec<u32>,
        ) -> Self {
            CheckedStructuralControlSuccessorPlan {
                statement_ordinal,
                target_state,
                target_occurrence: None,
                completion,
                is_continuation,
                transfers,
                scalar_arguments,
                erased_arguments,
                erased_proof_arguments,
                trivial_affine_discard_parameter_positions,
            }
        }

        pub fn operand_operations(
            &self,
        ) -> impl Iterator<Item = &crate::CheckedUnitEffectOperationPlan> {
            self.transfers
                .iter()
                .filter_map(|transfer| match &transfer.source {
                    CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                        operands,
                        establishment,
                        ..
                    } => Some((operands, Some(establishment.as_ref()))),
                    CheckedStructuralControlTransferSourcePlan::EvaluateNoNormal { operations } => {
                        Some((operations, None))
                    }
                    _ => None,
                })
                .flat_map(|(operands, establishment)| operands.iter().chain(establishment))
        }

        pub fn operand_calls(
            &self,
        ) -> impl Iterator<Item = &crate::CheckedUnitEffectOperationPlan> {
            self.operand_operations()
                .flat_map(crate::CheckedUnitEffectOperationPlan::with_value_calls)
                .filter(|operation| operation.call().is_some())
        }

        pub fn has_normal_caller_completion(&self) -> bool {
            self.completion != CheckedSuccessorCompletion::NoNormalInvocation
                && self.has_normal_continuation()
        }

        pub fn has_normal_continuation(&self) -> bool {
            self.operand_operations()
                .all(crate::CheckedUnitEffectOperationPlan::has_normal_continuation)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralScalarArgumentPlan {
        pub argument_ordinal: u32,
        pub source: CheckedStructuralScalarArgumentSourcePlan,
        pub target_scalar_parameter_index: u32,
        pub primitive_type: PrimitiveType,
    }

    impl Default for CheckedStructuralScalarArgumentPlan {
        fn default() -> Self {
            Self {
                argument_ordinal: 0,
                source: CheckedStructuralScalarArgumentSourcePlan::Expression,
                target_scalar_parameter_index: 0,
                primitive_type: PrimitiveType::Bool,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedStructuralScalarArgumentSourcePlan {
        Parameter { index: u32 },
        Expression,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralControlTransferPlan {
        pub source: CheckedStructuralControlTransferSourcePlan,
        pub target_parameter_index: u32,
        pub retained_loan_backing: Option<u32>,
        pub incoming_loan_resources: Vec<arena::Handle<crate::CheckedCallBorrowLoanResource>>,
        pub retained_loan_parents: Vec<(
            crate::CheckedUnitStructuralArgumentSourcePlan,
            crate::CheckedUnitStructuralArgumentSourcePlan,
        )>,
        pub retained_operand_parents: Vec<crate::CheckedUnitStructuralArgumentSourcePlan>,
    }

    impl Default for CheckedStructuralControlTransferPlan {
        fn default() -> Self {
            Self {
                source: CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 },
                target_parameter_index: 0,
                retained_loan_backing: None,
                incoming_loan_resources: Vec::new(),
                retained_loan_parents: Vec::new(),
                retained_operand_parents: Vec::new(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedStructuralControlTransferSourcePlan {
        StructuralResult {
            binding_ordinal: u32,
        },
        Parameter {
            index: u32,
        },
        CasePayload {
            subject: CheckedUnitStructuralArgumentPlan,
            case_identity: CheckedMemberIdentity,
            field_identity: CheckedMemberIdentity,
            path: Vec<CheckedUnitStructuralPathSegment>,
        },
        ViewSubslice(CheckedUnitStructuralArgumentPlan),
        ByteSequenceLiteral {
            bytes: Vec<u8>,
        },
        ProjectedValue {
            subject: CheckedUnitStructuralArgumentPlan,
        },
        EstablishedValue {
            operands: CheckedOperandSequence,
            establishment: Box<crate::CheckedUnitEffectOperationPlan>,
            presentation: CheckedUnitStructuralArgumentPlan,
        },
        EvaluateNoNormal {
            operations: CheckedOperandSequence,
        },
        ProjectedLoan {
            subject: CheckedUnitStructuralArgumentPlan,
            backing_type: typed_trees::types::TypeReferenceHandle,
            source_contract: crate::CheckedReferenceSourceContract,
            retired_carrier_backing: Option<u32>,
            formation_expression: typed_trees::expression::ExpressionHandle,
            carrier_type_identity: typed_trees::type_identity::TypeIdentityId,
        },
    }
}
pub(crate) mod structural_return {
    use super::{CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan};
    use language_semantics::Multiplicity;
    use typed_trees::type_identity::TypeIdentityId;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitStructuralReturnPlan {
        pub source: CheckedUnitStructuralArgumentSourcePlan,
        pub completion: CheckedStructuralReturnCompletion,
        pub type_identity: TypeIdentityId,
        pub multiplicity: Multiplicity,
        pub reference_sources: Vec<CheckedReturnedReferencePlan>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedStructuralReturnCompletion {
        Value {
            statement_index: u32,
            expression: typed_trees::expression::ExpressionHandle,
            evaluation: crate::FlowPermissionEvaluation,
        },
        Call {
            coordinate: super::CheckedUnitCallCoordinate,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CheckedReferenceResultSourcePlan {
        pub path: Vec<super::CheckedUnitStructuralPathSegment>,
        pub source: super::CheckedUnitStructuralArgumentPlan,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedReturnedReferencePlan {
        pub path: Vec<super::CheckedUnitStructuralPathSegment>,
        pub source: CheckedReferenceOriginSourcePlan,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedReferenceOriginSourcePlan {
        pub root: CheckedReferenceOriginRoot,
        pub entry_reference_hops: u32,
        pub path: Vec<CheckedReferenceOriginProjection>,
        pub extent: semantic_vocabulary::ReferenceOriginExtent,
        pub type_identity: TypeIdentityId,
        pub access: super::CheckedStructuralAccess,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedReferenceOriginRoot {
        Parameter { index: u32 },
        Invocation { carrier_type: TypeIdentityId },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedReferenceOriginProjection {
        Exact(super::CheckedUnitStructuralPathSegment),
        Element,
    }

    impl CheckedUnitStructuralReturnPlan {
        pub fn from_binding(
            binding: CheckedUnitStructuralResultBindingPlan,
            completion: CheckedStructuralReturnCompletion,
        ) -> Self {
            Self {
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: binding.binding_ordinal,
                },
                completion,
                type_identity: binding.type_identity,
                multiplicity: binding.multiplicity,
                reference_sources: Vec::new(),
            }
        }
    }
}
pub(crate) mod structural_type_plans {
    use language_core::BindingRelevance;
    use language_semantics::{Multiplicity, SemanticDomainId};
    use symbols::SymbolHandle;
    use typed_trees::type_identity::TypeIdentityId;
    use typed_trees::types::PrimitiveType;

    pub use typed_trees::data::DataMemberIdentity as CheckedMemberIdentity;

    pub use typed_trees::constant_recipes::DeclaredArrayExtent as CheckedExtent;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedTrivialAffineStructuralLocalPlan {
        pub declaration_ordinal: u32,
        pub type_identity: TypeIdentityId,
        pub construction: Option<CheckedAffineConstructionElementPlan>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedAffineConstructionElementPlan {
        pub root_type_identity: TypeIdentityId,
        pub index: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralResultPlan {
        pub type_identity: TypeIdentityId,
        pub multiplicity: Multiplicity,
        pub qualifications: Vec<SemanticDomainId>,
        pub projected_qualifications: Vec<CheckedStructuralPathQualification>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub struct CheckedStructuralPathQualification {
        pub path: Vec<CheckedUnitStructuralPathSegment>,
        pub domain: SemanticDomainId,
    }

    #[derive(Debug, Clone, compiler_storage::Record)]
    pub struct CheckedUnitStructuralTypePlan {
        pub source_declaration: SymbolHandle,
        pub source_application: typed_trees::SourceTypeApplication,
        pub identity: TypeIdentityId,
        pub descriptor_presentation: TypeIdentityId,
        pub native_identity: Option<String>,
        pub native_policy_identity: Option<String>,
        pub native_semantic_identity: Option<String>,
        pub owns_linear_claim: bool,
        pub copyable: bool,
        pub no_code_affine_cleanup: bool,
        pub descriptor_cleanup: bool,
        pub shape: CheckedUnitStructuralTypeShape,
        pub scalar_restriction: Option<CheckedUnitStructuralFieldType>,
    }

    impl PartialEq for CheckedUnitStructuralTypePlan {
        fn eq(&self, other: &Self) -> bool {
            self.identity == other.identity
                && self.descriptor_presentation == other.descriptor_presentation
                && self.native_identity == other.native_identity
                && self.native_policy_identity == other.native_policy_identity
                && self.native_semantic_identity == other.native_semantic_identity
                && self.owns_linear_claim == other.owns_linear_claim
                && self.copyable == other.copyable
                && self.no_code_affine_cleanup == other.no_code_affine_cleanup
                && self.descriptor_cleanup == other.descriptor_cleanup
                && self.shape == other.shape
                && self.scalar_restriction == other.scalar_restriction
        }
    }

    impl Eq for CheckedUnitStructuralTypePlan {}

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitRecordStorage {
        Packed,
        AccessView {
            schema_type_identity: TypeIdentityId,
            storage: semantic_vocabulary::AuthoredRecordStorage,
        },
        Accessor(semantic_vocabulary::AuthoredRecordStorage),
        Authored(semantic_vocabulary::AuthoredRecordStorage),
        TargetLayout {
            application: typed_trees::types::TypeReferenceHandle,
            application_identity: TypeIdentityId,
            schema_type_identity: TypeIdentityId,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitStructuralTypeShape {
        Reference {
            referent_identity: TypeIdentityId,
            access: CheckedStructuralAccess,
            foreign_descriptor: bool,
            abstract_referent: bool,
        },
        PrimitiveScalar(typed_trees::types::PrimitiveType),
        ByteSequence(CheckedByteSequenceCarrier),
        Record {
            fields: Vec<CheckedUnitStructuralFieldPlan>,
            storage: CheckedUnitRecordStorage,
        },
        FixedArray {
            element_type_identity: TypeIdentityId,
            length: CheckedExtent,
        },
        BorrowedSliceView {
            element_type_identity: TypeIdentityId,
            access: Option<super::CheckedStructuralAccess>,
        },
        Sum {
            cases: Vec<CheckedUnitStructuralCasePlan>,
        },
        Mixed {
            fields: Vec<CheckedUnitStructuralFieldPlan>,
            cases: Vec<CheckedUnitStructuralCasePlan>,
        },
        AtomicCell(semantic_vocabulary::AtomicCellType),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitStructuralCasePlan {
        pub identity: CheckedMemberIdentity,
        pub fields: Vec<CheckedUnitStructuralFieldPlan>,
    }

    #[derive(Debug, Clone, compiler_storage::Record)]
    pub struct CheckedUnitStructuralFieldPlan {
        pub source_declaration: SymbolHandle,
        pub scalar_carrier_identity: TypeIdentityId,
        pub type_reference: crate::types::TypeReferenceHandle,
        pub receiving_type_identity: TypeIdentityId,
        pub identity: CheckedMemberIdentity,
        pub relevance: BindingRelevance,
        pub native_identity: Option<String>,
        pub field_type: CheckedUnitStructuralFieldType,
        pub byte_sequence_type_identity: TypeIdentityId,
        pub storage: Option<semantic_vocabulary::AuthoredFieldStorage>,
        pub qualification_carriers: Vec<TypeIdentityId>,
    }

    impl PartialEq for CheckedUnitStructuralFieldPlan {
        fn eq(&self, other: &Self) -> bool {
            self.receiving_type_identity == other.receiving_type_identity
                && self.identity == other.identity
                && self.scalar_carrier_identity == other.scalar_carrier_identity
                && self.native_identity == other.native_identity
                && self.relevance == other.relevance
                && self.field_type == other.field_type
                && self.byte_sequence_type_identity == other.byte_sequence_type_identity
                && self.storage == other.storage
                && self.qualification_carriers == other.qualification_carriers
        }
    }

    impl Eq for CheckedUnitStructuralFieldPlan {}

    impl CheckedUnitStructuralFieldPlan {
        pub fn retained_type_identities(&self) -> impl Iterator<Item = TypeIdentityId> + '_ {
            self.field_type
                .structural_type_identity()
                .into_iter()
                .chain(
                    self.byte_sequence_type_identity
                        .is_valid()
                        .then_some(self.byte_sequence_type_identity),
                )
                .chain(self.qualification_carriers.iter().copied())
                .chain(
                    self.scalar_carrier_identity
                        .is_valid()
                        .then_some(self.scalar_carrier_identity),
                )
        }

        pub fn cleanup_type_identity(&self) -> TypeIdentityId {
            match &self.field_type {
                CheckedUnitStructuralFieldType::Structural { type_identity } => *type_identity,
                field
                    if field
                        .byte_sequence_carrier()
                        .is_some_and(|carrier| carrier.is_borrowed()) =>
                {
                    self.byte_sequence_type_identity
                }
                _ => TypeIdentityId::default(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitStructuralFieldType {
        Scalar(PrimitiveType),
        Atomic(PrimitiveType),
        AtomicBoundedInteger(semantic_vocabulary::BoundedIntegerType),
        BoundedInteger(semantic_vocabulary::BoundedIntegerType),
        RefinedScalar {
            primitive: PrimitiveType,
            predicate: crate::CheckedBooleanExpression,
        },
        ByteSequence(CheckedByteSequenceCarrier),
        PredicateByteSequence {
            carrier: CheckedByteSequenceCarrier,
            predicates: language_semantics::byte_predicates::ByteSequencePredicateSet,
        },
        Structural {
            type_identity: TypeIdentityId,
        },
        Erased {
            type_identity: TypeIdentityId,
        },
    }

    impl CheckedUnitStructuralFieldType {
        pub fn structural_type_identity(&self) -> Option<TypeIdentityId> {
            match self {
                Self::Structural { type_identity } => Some(*type_identity),
                _ => None,
            }
        }

        pub fn byte_sequence_carrier(&self) -> Option<CheckedByteSequenceCarrier> {
            match self {
                Self::ByteSequence(carrier) | Self::PredicateByteSequence { carrier, .. } => {
                    Some(*carrier)
                }
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedByteSequenceCarrier {
        BorrowedView {
            access: Option<CheckedStructuralAccess>,
        },
        BoundedView {
            access: Option<CheckedStructuralAccess>,
            capacity: u64,
        },
        BoundedOwned {
            capacity: u64,
        },
    }

    impl From<language_core::ReferenceAccess> for CheckedStructuralAccess {
        fn from(access: language_core::ReferenceAccess) -> Self {
            match access {
                language_core::ReferenceAccess::Shared => Self::SharedBorrow,
                language_core::ReferenceAccess::Mutable => Self::MutableBorrow,
                language_core::ReferenceAccess::WriteOnly => Self::WriteOnlyBorrow,
            }
        }
    }

    impl CheckedByteSequenceCarrier {
        pub const fn is_borrowed_view(self) -> bool {
            matches!(self, Self::BorrowedView { .. })
        }

        pub const fn is_borrowed(self) -> bool {
            matches!(self, Self::BorrowedView { .. } | Self::BoundedView { .. })
        }
    }

    pub fn is_borrowed_view(carrier: Option<CheckedByteSequenceCarrier>) -> bool {
        carrier.is_some_and(CheckedByteSequenceCarrier::is_borrowed_view)
    }

    impl CheckedByteSequenceCarrier {
        pub const fn without_access(self) -> Self {
            match self {
                Self::BorrowedView { .. } => Self::BorrowedView { access: None },
                Self::BoundedView { capacity, .. } => Self::BoundedView {
                    access: None,
                    capacity,
                },
                other => other,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitStructuralDomainPlan {
        pub required_memberships: Option<Vec<SemanticDomainId>>,
        pub complete_predicate_theory: bool,
        pub predicate: Option<crate::CheckedBooleanExpression>,
        pub byte_predicate: Option<language_semantics::byte_predicates::ByteSequencePredicate>,
        pub domain: SemanticDomainId,
        pub name: String,
        pub carrier_type_identity: TypeIdentityId,
        pub establishment_routes: Vec<language_semantics::DomainEstablishmentRoute>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitStructuralDomainRequirementPlan {
        pub argument_index: u32,
        pub path: Vec<CheckedUnitStructuralPathSegment>,
        pub domain: SemanticDomainId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedStructuralAccess {
        Owned,
        SharedBorrow,
        MutableBorrow,
        WriteOnlyBorrow,
    }

    #[macro_export]
    macro_rules! checked_borrowed_access_pair_pattern {
        () => {
            $crate::CheckedStructuralAccess::SharedBorrow
                | $crate::CheckedStructuralAccess::MutableBorrow
        };
    }
    impl CheckedStructuralAccess {
        pub fn borrow_access_kind(self) -> Option<crate::borrow::BorrowAccessKind> {
            use crate::borrow::BorrowAccessKind;
            Some(match self {
                Self::SharedBorrow => BorrowAccessKind::Read,
                Self::MutableBorrow => BorrowAccessKind::Mutable,
                Self::WriteOnlyBorrow => BorrowAccessKind::WriteOnly,
                Self::Owned => return None,
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitStructuralParameterPlan {
        pub position: u32,
        pub type_identity: TypeIdentityId,
        pub multiplicity: Multiplicity,
        pub access: CheckedStructuralAccess,
        pub primitive_referent: Option<PrimitiveType>,
        pub qualifications: Vec<SemanticDomainId>,
        pub projected_qualifications: Vec<CheckedStructuralPathQualification>,
        pub additional_input_qualifications: Vec<CheckedStructuralPathQualification>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum CheckedUnitStructuralPathSegment {
        AllElements,
        Field(CheckedMemberIdentity),
        Case(CheckedMemberIdentity),
        FixedIndex(u64),
        FixedRange {
            start: u64,
            end: u64,
        },
        RuntimeIndex(CheckedRuntimeIndex),
        ComputedIndex(crate::CheckedScalarComputationHandle),
        RepresentationView {
            type_identity: TypeIdentityId,
            storage: super::CheckedReferenceStorage,
        },
        Referent,
    }

    impl Default for CheckedUnitStructuralPathSegment {
        fn default() -> Self {
            Self::Field(CheckedMemberIdentity::default())
        }
    }

    impl Ord for CheckedUnitStructuralPathSegment {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            use CheckedUnitStructuralPathSegment::*;
            fn kind(segment: &CheckedUnitStructuralPathSegment) -> u8 {
                match segment {
                    Field(_) => 0,
                    Case(_) => 1,
                    FixedIndex(_) => 2,
                    FixedRange { .. } => 3,
                    RuntimeIndex(_) => 4,
                    Referent => 5,
                    ComputedIndex(_) => 6,
                    RepresentationView { .. } => 7,
                    AllElements => 8,
                }
            }
            match (self, other) {
                (Field(left), Field(right)) | (Case(left), Case(right)) => left.cmp(right),
                (FixedIndex(left), FixedIndex(right)) => left.cmp(right),
                (
                    FixedRange {
                        start: left_start,
                        end: left_end,
                    },
                    FixedRange {
                        start: right_start,
                        end: right_end,
                    },
                ) => (left_start, left_end).cmp(&(right_start, right_end)),
                (RuntimeIndex(left), RuntimeIndex(right)) => left.cmp(right),
                (ComputedIndex(left), ComputedIndex(right)) => {
                    (left.arena_index(), left.generation())
                        .cmp(&(right.arena_index(), right.generation()))
                }
                (
                    RepresentationView {
                        type_identity: left_type,
                        storage: left_storage,
                    },
                    RepresentationView {
                        type_identity: right_type,
                        storage: right_storage,
                    },
                ) => (left_type, left_storage).cmp(&(right_type, right_storage)),
                _ => kind(self).cmp(&kind(other)),
            }
        }
    }

    impl PartialOrd for CheckedUnitStructuralPathSegment {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    pub use facts::CheckedRuntimeIndex;

    pub fn is_bounded_nominal_cleanup_record(shape: &CheckedUnitStructuralTypeShape) -> bool {
        match shape {
            CheckedUnitStructuralTypeShape::Record { fields, .. } => fields.iter().all(|field| {
                !field.relevance.is_erased()
                    && matches!(
                        &field.field_type,
                        CheckedUnitStructuralFieldType::Scalar(
                            PrimitiveType::Bool
                                | PrimitiveType::I8
                                | PrimitiveType::I16
                                | PrimitiveType::I32
                                | PrimitiveType::I64
                                | PrimitiveType::U8
                                | PrimitiveType::U16
                                | PrimitiveType::U32
                                | PrimitiveType::U64
                                | PrimitiveType::Addr
                        )
                    )
            }),
            CheckedUnitStructuralTypeShape::Reference { .. }
            | CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
            | CheckedUnitStructuralTypeShape::AtomicCell(_)
            | CheckedUnitStructuralTypeShape::ByteSequence(_)
            | CheckedUnitStructuralTypeShape::FixedArray { .. }
            | CheckedUnitStructuralTypeShape::BorrowedSliceView { .. }
            | CheckedUnitStructuralTypeShape::Sum { .. }
            | CheckedUnitStructuralTypeShape::Mixed { .. } => false,
        }
    }

    pub fn is_partial_affine_field_type(field_type: &CheckedUnitStructuralFieldType) -> bool {
        if let CheckedUnitStructuralFieldType::Structural { type_identity } = field_type {
            return type_identity.is_valid();
        }
        matches!(
            field_type,
            CheckedUnitStructuralFieldType::BoundedInteger(_)
                | CheckedUnitStructuralFieldType::RefinedScalar { .. }
                | CheckedUnitStructuralFieldType::ByteSequence(
                    CheckedByteSequenceCarrier::BoundedOwned { .. }
                )
                | CheckedUnitStructuralFieldType::PredicateByteSequence {
                    carrier: CheckedByteSequenceCarrier::BoundedOwned { .. },
                    ..
                }
                | CheckedUnitStructuralFieldType::Scalar(
                    PrimitiveType::Bool
                        | PrimitiveType::I8
                        | PrimitiveType::I16
                        | PrimitiveType::I32
                        | PrimitiveType::I64
                        | PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                        | PrimitiveType::Addr
                        | PrimitiveType::F32
                        | PrimitiveType::F64
                )
        )
    }

    pub fn checked_structural_type_by_identity(
        plans: &[CheckedUnitStructuralTypePlan],
        identity: TypeIdentityId,
    ) -> Option<&CheckedUnitStructuralTypePlan> {
        plans.iter().find(|plan| plan.identity == identity)
    }

    #[cfg(test)]
    mod field_identity_tests {
        use super::*;

        #[test]
        fn receiving_type_identity_not_source_allocation_controls_field_equality() {
            let identities = typed_trees::type_identity::TypeIdentityTable::default();
            let counter_reader = identities.intern("dyn Reader<Counter>");
            let other_reader = identities.intern("dyn Reader<Other>");
            let field = CheckedUnitStructuralFieldPlan {
                source_declaration: Default::default(),
                scalar_carrier_identity: Default::default(),
                native_identity: None,
                type_reference: crate::types::TypeReferenceHandle::from_parts(1, 1),
                receiving_type_identity: counter_reader,
                identity: CheckedMemberIdentity::of_spelling("operations"),
                relevance: BindingRelevance::Relevant,
                field_type: CheckedUnitStructuralFieldType::Scalar(PrimitiveType::Selector),
                byte_sequence_type_identity: TypeIdentityId::default(),
                storage: None,
                qualification_carriers: Vec::new(),
            };
            let mut reallocated = field.clone();
            reallocated.type_reference = crate::types::TypeReferenceHandle::from_parts(2, 1);
            assert_eq!(field, reallocated);

            reallocated.receiving_type_identity = other_reader;
            assert_ne!(field, reallocated);
            reallocated.receiving_type_identity = counter_reader;
            reallocated.qualification_carriers.push(other_reader);
            assert_ne!(field, reallocated);
        }
    }
}
pub(crate) mod terminal_selections {
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedTerminalMachineSelections {
        pub machines: Vec<CheckedTerminalMachineSelection>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedTerminalMachineSelection {
        pub machine: SymbolHandle,
        pub name: String,
        pub signature: CheckedTerminalSignatureEligibility,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedTerminalSignatureEligibility {
        Eligible,
        FreeUnitEffect,
        Unsupported,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedTerminalDebugPlans {
        pub machines: Vec<CheckedTerminalMachineDebugPlan>,
    }

    impl CheckedTerminalDebugPlans {
        pub fn for_machine(
            &self,
            machine: SymbolHandle,
        ) -> Option<&CheckedTerminalMachineDebugPlan> {
            self.machines.iter().find(|plan| plan.machine == machine)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedTerminalMachineDebugPlan {
        pub machine: SymbolHandle,
        pub machine_span: Option<source::SourceSpan>,
        pub contract_span: Option<source::SourceSpan>,
        pub states: Vec<CheckedTerminalStateDebugPlan>,
        pub source_files: Vec<source::SourceFile>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedTerminalStateDebugPlan {
        pub state: SymbolHandle,
        pub state_span: Option<source::SourceSpan>,
        pub parameter_spans: Vec<Option<source::SourceSpan>>,
        pub transition_spans: Vec<source::SourceSpan>,
        pub operation_spans: Vec<source::SourceSpan>,
    }
}
pub(crate) use crate::flow_terminal_units::unit_call_plans;
pub(crate) use crate::flow_terminal_units::unit_effect_plans;
pub(crate) use crate::flow_terminal_units::unit_plans;

pub use affine_cleanup_plans::{
    CheckedCleanupCaseCondition, CheckedCleanupCaseReachability, CheckedCleanupControlPlans,
    CheckedCleanupPartitionArm, CheckedCleanupPartitionLeaf, CheckedCleanupPartitionNode,
    CheckedCleanupPartitionPlan, CheckedCleanupPartitionRef, CheckedCleanupPartitionStep,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedNominalAffineUnitCleanupPlans,
    CheckedNormalCleanupAction, CheckedNormalCompletion, CheckedNormalExitCleanupPlan,
    CheckedNormalExitCleanups, CheckedNormalResultArm, CheckedNormalResultCompletion,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupPlans,
    CheckedReferenceOwnerCleanup, CheckedStructuralCallCustodyPlan,
    CheckedStructuralResultClaimPlan, CheckedUnitClaimTransferPlan,
    CheckedUnitNominalAffineCallerRequirementPlan, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitNominalAffineCleanupRequirementPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedValueCompletionPlan,
};
pub use atomic_plans::{CheckedAtomicAccessPlan, CheckedAtomicEvent, CheckedAtomicReadModifyWrite};
pub use boundary_machine_plans::{
    CheckedBoundaryExecutionPlan, CheckedBoundaryLifetimeDependency, CheckedBoundaryMachinePlan,
    CheckedBoundaryMachineResultPlan, CheckedBoundaryReferenceDestination,
    CheckedBoundaryReferenceOutput, CheckedBoundaryScalarDomainRequirementPlan,
    CheckedBoundaryScalarGuarantee,
};
pub use composed_unit_control_plans::{
    CheckedClosedSumCaseOutcomePlan, CheckedClosedSumCasePlan, CheckedClosedSumPayloadTransferPlan,
    CheckedClosedSumSubjectKind, CheckedComposedUnitControlMachinePlan,
    CheckedComposedUnitControlStatePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedConditionalReturnArm, CheckedControlResultPlan, CheckedDeclarationRequirementSet,
    CheckedGuardedJumpArmPlan, CheckedGuardedJumpFallbackPlan, CheckedGuardedJumpPlan,
    CheckedGuardedJumpReturnPlan, CheckedGuardedReturnValue, CheckedIntegerRankObservation,
    CheckedNaturalRankFunction, CheckedNaturalRankMeasure, CheckedRankEdgeCoordinate,
    CheckedScalarCaseFieldPlan, CheckedScalarReturnPlan, CheckedStateContractRequirement,
    CheckedStateNaturalRank, CheckedUnitCompletionExpression, CheckedUnitCompletionKind,
    CheckedUnitCompletionPlan, CheckedUnitCompletionSelection, CheckedUnitScalarCompletionArm,
    CheckedUnitScalarCompletionCondition, CheckedUnitStructuralCompletionArm,
};
pub use dynamic_slot_plans::{CheckedDynamicClaimPath, CheckedDynamicSlotPlan};
pub use place_loan_plans::{CheckedPlaceLoanPlan, CheckedPlaceLoanRoot};
pub use result_binding_plans::{
    CheckedByteSequenceValue, CheckedByteSequenceValuePlan, CheckedInitializedLocal,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralResultBindingPlan,
};
pub use scalar_state_plans::{
    CheckedGuardedExitPlans, CheckedScalarBranchDestination, CheckedScalarGuardedExit,
    CheckedScalarGuardedTail, CheckedScalarStateTerminator, ScalarReturnPath,
};
pub use structural_argument_plans::{
    CheckedCallScalarArgument, CheckedReferenceStorage, CheckedStructuralScalarParameterPlan,
    CheckedUnitCallCoordinate, CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
pub use structural_control_plans::{
    CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
    CheckedStructuralControlStateCleanupPlan, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralControlTransferSourcePlan,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarArgumentSourcePlan,
    CheckedSuccessorCompletion,
};
pub use structural_return::{
    CheckedReferenceOriginProjection, CheckedReferenceOriginRoot, CheckedReferenceOriginSourcePlan,
    CheckedReferenceResultSourcePlan, CheckedReturnedReferencePlan,
    CheckedStructuralReturnCompletion, CheckedUnitStructuralReturnPlan,
};
pub use structural_type_plans::{
    CheckedAffineConstructionElementPlan, CheckedByteSequenceCarrier, CheckedExtent,
    CheckedMemberIdentity, CheckedRuntimeIndex, CheckedStructuralAccess,
    CheckedStructuralPathQualification, CheckedStructuralResultPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitRecordStorage,
    CheckedUnitStructuralCasePlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, checked_structural_type_by_identity, is_borrowed_view,
    is_bounded_nominal_cleanup_record, is_partial_affine_field_type,
};
pub use terminal_selections::{
    CheckedTerminalDebugPlans, CheckedTerminalMachineDebugPlan, CheckedTerminalMachineSelection,
    CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
    CheckedTerminalStateDebugPlan,
};
pub use unit_call_plans::{
    CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
    CheckedUnitCallTarget, CheckedUnitCallTransfers,
};
pub use unit_effect_plans::{
    CheckedLayoutObservation, CheckedOperandSequence, CheckedScalarReturnCopies,
    CheckedSelectedPayloadCopies, CheckedSelectedPayloadCopy, CheckedStructuralInvocation,
    CheckedStructuralLocalInitialization, CheckedStructuralValueCall, CheckedUnitCallableEntry,
    CheckedUnitCleanupBoundary, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEffectPlans, CheckedUnitLocalBindingPlan, CheckedUnitParameterStoragePlan,
    CheckedUnitPlanGap, CheckedUnitPlanGapReason, CheckedUnitScalarControlPlan,
    CheckedUnitSelectedReferenceRetirementPlan,
};
pub use unit_plans::CheckedUnitPlan;
