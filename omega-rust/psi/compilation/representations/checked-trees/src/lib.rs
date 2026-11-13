/*
We pair frozen `TypedTrees` with `CheckFacts` in `CheckedTrees`, holding all
admissibility, borrow, control-flow, operator, proof, value, and Terminal-plan
judgments established by `Psi 05` (`05_typed-trees-to-checked-trees`).
*/

/*
We keep checked facts beside the typed program rather than duplicating source
vocabulary. Downstream lowering (`Psi 06`, `Psi 08`) and realization stages read
exact checked plans—loans, restoration obligations, crash routes, affine
cleanup, ranking witnesses, and provider inventories—from `CheckedTrees` without
re-running borrow, proof, or termination analysis.
*/

/*
- Produced by: `Psi 05` (`05_typed-trees-to-checked-trees`).
- Consumed by: `Psi 06` (`06_checked-trees-to-lowered-psi`), `Psi 08` (`08_lowered-psi-to-terminal-psi`), `checked-interpreter`, and Omega realization/compiler crates.
- Flat module layout:
  - `lib.rs`: `CheckedTrees`, `KeyedRows`, `ShiftedRows`, callback declarations, and source guards.
  - `providers_and_trust.rs`: provider declarations.
  - `admissibility_and_borrow.rs`: acceptance verdicts (`admissibility`) and borrow/loan lifecycle facts (`borrow`).
  - `facts.rs`, `contract_and_obligation_facts.rs`, `termination_and_cleanup_facts.rs`: `CheckFacts` and declaration, contract, crash, carry, content, cleanup, and termination fact tables.
  - `flow.rs`, `flow_ownership_and_roots.rs`, `flow_terminal.rs`, `flow_terminal_units.rs`: `FlowFacts`, ownership/claim transfers, and checked Terminal lowering plans.
  - `operators_and_proof.rs`: checked operator applications (`operators`) and proof obligations/contracts (`proof`).
  - `values_and_views.rs`: checked value computations/structural values (`values`) and read-only fact views (`views`).
*/

pub(crate) use optimization::digest_encoder;
pub use typed_trees;
pub use typed_trees::type_identity::TypeIdentityRequest;

pub(crate) mod callback_declarations {
    use crate::{CheckedNominalMachineUse, CheckedTrees};
    use language_semantics::callback_declarations::{
        CallbackDeclaration, CallbackRegistrationDeclaration,
    };

    impl CheckedTrees {
        pub fn callback_declaration<'usage>(
            &self,
            usage: &'usage CheckedNominalMachineUse,
        ) -> CallbackDeclaration<'usage> {
            CallbackDeclaration {
                registration: self.callback_registration_declaration(
                    usage.registration_operation,
                    usage.registration_requirement_trait,
                ),
                satisfaction_is_boundary: self
                    .typed
                    .trait_definition_by_symbol(usage.satisfaction_trait)
                    .is_some_and(|definition| definition.is_boundary),
                has_checked_entry: usage.callback_entry.is_some(),
                requirement_overload: &usage.canonical_requirement_overload,
            }
        }

        fn callback_registration_declaration(
            &self,
            operation: symbols::SymbolHandle,
            requirement_trait: Option<symbols::SymbolHandle>,
        ) -> Result<CallbackRegistrationDeclaration, String> {
            let typed = &self.typed;
            let identity = if let Some((owner, signature)) =
                typed.trait_machine_signature(operation)
            {
                if !owner.is_boundary {
                    return Ok(CallbackRegistrationDeclaration::CheckedSource);
                }
                typed.trait_requirement_callable_identity_text(owner, signature)
            } else if let Some(machine) = typed.machine_by_own_or_state_symbol(operation) {
                if machine.supplies_checked_body() {
                    return Ok(CallbackRegistrationDeclaration::CheckedSource);
                }
                typed
                    .machine_callable_identity_text(machine)
                    .ok_or_else(|| {
                        "nominal machine registration lost its complete callable identity"
                            .to_owned()
                    })?
            } else if typed.machine_parameter_signature(operation).is_some()
                || requirement_trait
                    .and_then(|owner| typed.trait_definition_by_symbol(owner))
                    .is_some_and(|declaration| !declaration.is_boundary)
            {
                return Ok(CallbackRegistrationDeclaration::CheckedSource);
            } else {
                return Err(format!(
                    "nominal machine use lost its exact registration declaration: {} ({operation:?})",
                    typed.symbols.display_path(operation, "::"),
                ));
            };
            Ok(CallbackRegistrationDeclaration::External(identity))
        }
    }
}

#[cfg(test)]
pub(crate) mod source_guards {

    use source_audit::violations;

    const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

    const ALLOWED: &[(&str, &str)] = &[
        (
            "facts.rs",
            concat!("pub registrar_application_identity", ": String,"),
        ),
        (
            "flow_terminal.rs",
            concat!("pub requirement_identity", ": String,"),
        ),
        (
            "flow_terminal.rs",
            concat!("pub native_identity", ": Option<String>,"),
        ),
        (
            "flow_terminal.rs",
            concat!("pub native_policy_identity", ": Option<String>,"),
        ),
        (
            "flow_terminal.rs",
            concat!("pub native_semantic_identity", ": Option<String>,"),
        ),
        (
            "operators_and_proof.rs",
            concat!("pub schema_identity", ": String,"),
        ),
    ];

    #[test]
    fn checked_trees_carry_no_string_identity_fields() {
        let violations = violations(
            CRATE_ROOT,
            "",
            &[
                concat!("identity", ": String"),
                concat!("identity", ": Option<String>"),
                concat!("identity", ": Vec<String>"),
                concat!("identities", ": Vec<String>"),
                concat!("identities", ": HashSet<String>"),
                concat!("identities", ": BTreeSet<String>"),
            ],
            ALLOWED,
        );
        assert!(
            violations.is_empty(),
            "checked trees must carry identity handles, not text identities: {violations:?}"
        );
    }
}

pub use typed_trees::ClosedConformanceConstArgument;
pub use typed_trees::byte_predicates;
pub use typed_trees::ranking;
pub use typed_trees::{
    data, domain, expression, finite_family, identity, machine, name, proof_only, proposition,
    signature, state, trait_definition, types, wire,
};

mod providers_and_trust;
pub use providers_and_trust::provider_declarations;
mod admissibility_and_borrow;
pub use admissibility_and_borrow::{admissibility, borrow};
mod contract_and_obligation_facts;
pub mod facts;
pub mod flow;
mod flow_ownership_and_roots;
mod flow_terminal;
mod flow_terminal_units;
mod termination_and_cleanup_facts;
pub use flow::CheckedCallOrigin;
pub mod keyed_rows {
    use arena::{Arena, Handle, OwnedIndex, PagedArenaSet};
    use std::hash::Hash;
    use symbols::SymbolKeyMap;

    pub trait DurableRows {
        type Row;

        fn revision(&self) -> u64;

        fn rows(&self) -> impl Iterator<Item = (Handle<Self::Row>, &Self::Row)>;

        fn append_epoch(&self) -> Option<(u64, usize)> {
            None
        }

        fn for_each_row_from(
            &self,
            start: usize,
            mut visit: impl FnMut(Handle<Self::Row>, &Self::Row),
        ) {
            for (handle, row) in self.rows().skip(start) {
                visit(handle, row);
            }
        }
    }

    impl<T: Default> DurableRows for Arena<T> {
        type Row = T;

        fn revision(&self) -> u64 {
            Arena::revision(self)
        }

        fn rows(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
            Arena::iter(self)
        }

        fn append_epoch(&self) -> Option<(u64, usize)> {
            Arena::append_epoch(self)
        }

        fn for_each_row_from(&self, start: usize, mut visit: impl FnMut(Handle<T>, &T)) {
            for (handle, row) in Arena::iter_from(self, start) {
                visit(handle, row);
            }
        }
    }

    impl<T: Default> DurableRows for PagedArenaSet<T> {
        type Row = T;

        fn revision(&self) -> u64 {
            PagedArenaSet::revision(self)
        }

        fn rows(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
            PagedArenaSet::iter(self)
        }

        fn append_epoch(&self) -> Option<(u64, usize)> {
            PagedArenaSet::append_epoch(self)
        }

        fn for_each_row_from(&self, start: usize, visit: impl FnMut(Handle<T>, &T)) {
            PagedArenaSet::for_each_from(self, start, visit);
        }
    }

    struct KeyedGroupCache<Key, Row> {
        append_epoch: Option<(u64, usize)>,
        groups: SymbolKeyMap<Key, Vec<Handle<Row>>>,
    }

    impl<Key: Clone, Row> Clone for KeyedGroupCache<Key, Row> {
        fn clone(&self) -> Self {
            Self {
                append_epoch: self.append_epoch,
                groups: self.groups.clone(),
            }
        }
    }

    #[derive(compiler_storage::Record)]
    pub struct KeyedRows<Key, Row> {
        groups: OwnedIndex<KeyedGroupCache<Key, Row>>,
    }

    impl<Key: Eq + Hash, Row> KeyedRows<Key, Row> {
        pub fn with_groups<A, R>(
            &self,
            arena: &A,
            key_of: impl FnMut(&Row) -> Key,
            run: impl FnOnce(&SymbolKeyMap<Key, Vec<Handle<Row>>>) -> R,
        ) -> R
        where
            A: DurableRows<Row = Row>,
        {
            let mut key_of = key_of;
            self.groups.with_updated(
                arena.revision(),
                |existing| {
                    let next_epoch = arena.append_epoch();
                    if let Some(existing) = existing {
                        if let (Some((prev_rewrite, prev_len)), Some((next_rewrite, next_len))) =
                            (existing.append_epoch, next_epoch)
                            && prev_rewrite == next_rewrite
                            && prev_len <= next_len
                        {
                            arena.for_each_row_from(prev_len, |handle, row| {
                                existing.groups.entry(key_of(row)).or_default().push(handle);
                            });
                            existing.append_epoch = next_epoch;
                            return None;
                        }
                        existing.groups.clear();
                        for (handle, row) in arena.rows() {
                            existing.groups.entry(key_of(row)).or_default().push(handle);
                        }
                        existing.append_epoch = next_epoch;
                        return None;
                    }
                    let mut groups: SymbolKeyMap<Key, Vec<Handle<Row>>> = SymbolKeyMap::default();
                    for (handle, row) in arena.rows() {
                        groups.entry(key_of(row)).or_default().push(handle);
                    }
                    Some(KeyedGroupCache {
                        append_epoch: next_epoch,
                        groups,
                    })
                },
                |cached| run(&cached.groups),
            )
        }

        pub fn seal<A>(&mut self, arena: &A, key_of: impl FnMut(&Row) -> Key)
        where
            A: DurableRows<Row = Row>,
        {
            self.with_groups(arena, key_of, |_| {});
            self.groups.seal();
        }

        pub fn with_rows<A, R>(
            &self,
            arena: &A,
            key_of: impl FnMut(&Row) -> Key,
            key: &Key,
            run: impl FnOnce(&[Handle<Row>]) -> R,
        ) -> R
        where
            A: DurableRows<Row = Row>,
        {
            self.with_groups(arena, key_of, |groups| {
                run(groups.get(key).map_or(&[], Vec::as_slice))
            })
        }
    }

    impl<Key, Row> KeyedRows<Key, Row> {
        pub fn clear(&mut self) {
            self.groups.clear();
        }
    }

    impl<Key, Row> Default for KeyedRows<Key, Row> {
        fn default() -> Self {
            Self {
                groups: OwnedIndex::default(),
            }
        }
    }

    impl<Key: Clone, Row> Clone for KeyedRows<Key, Row> {
        fn clone(&self) -> Self {
            Self {
                groups: self.groups.clone(),
            }
        }
    }

    impl<Key, Row> std::fmt::Debug for KeyedRows<Key, Row> {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("KeyedRows")
        }
    }

    impl<Key, Row> PartialEq for KeyedRows<Key, Row> {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    impl<Key, Row> Eq for KeyedRows<Key, Row> {}

    #[cfg(test)]
    mod tests {
        use super::{Arena, Handle, KeyedRows};
        use symbols::SymbolHandle;

        #[derive(Clone, Default, PartialEq, Eq)]
        struct Row {
            owner: SymbolHandle,
            marker: u32,
        }

        fn rows(
            keyed: &KeyedRows<SymbolHandle, Row>,
            arena: &Arena<Row>,
            owner: SymbolHandle,
        ) -> Vec<Handle<Row>> {
            keyed.with_rows(arena, |row| row.owner, &owner, |rows| rows.to_vec())
        }

        #[test]
        fn groups_stay_fresh_across_mutation_and_batch_append() {
            let keyed = KeyedRows::<SymbolHandle, Row>::default();
            let original = SymbolHandle::from_parts(1, 1);
            let moved = SymbolHandle::from_parts(2, 1);
            let mut arena = Arena::<Row>::default();
            assert!(rows(&keyed, &arena, original).is_empty());
            let first = arena.insert(Row {
                owner: original,
                marker: 1,
            });
            assert_eq!(rows(&keyed, &arena, original), [first]);
            arena.get_mut(first).owner = moved;
            assert_eq!(rows(&keyed, &arena, moved), [first]);
            assert!(rows(&keyed, &arena, original).is_empty());
            let batch = arena.insert_many([
                Row {
                    owner: original,
                    marker: 2,
                },
                Row {
                    owner: original,
                    marker: 3,
                },
            ]);
            assert_eq!(rows(&keyed, &arena, moved), [first]);
            assert_eq!(
                rows(&keyed, &arena, original),
                batch.handles().collect::<Vec<_>>()
            );
            assert!(arena.free(first));
            assert!(rows(&keyed, &arena, moved).is_empty());
            assert_eq!(rows(&keyed, &arena, original).len(), 2);
        }
    }
}
mod operators_and_proof;
pub use operators_and_proof::{operators, proof};
pub(crate) mod shifted_rows {
    use arena::{Arena, Handle, HandleSpan};

    pub(crate) fn shiftable<T: Default>(into: &Arena<T>, from: &Arena<T>) -> Option<u32> {
        let (_, count) = into.append_epoch()?;
        from.append_epoch()?;
        (into.storage_view().fresh_generation == from.storage_view().fresh_generation)
            .then(|| u32::try_from(count).ok())
            .flatten()
    }

    pub(crate) fn shift<T>(handle: Handle<T>, rows: u32) -> Handle<T> {
        if !handle.is_valid() {
            return handle;
        }
        Handle::from_parts(
            handle
                .arena_index()
                .checked_add(rows)
                .expect("appended rows exceed the handle range"),
            handle.generation(),
        )
    }

    pub(crate) fn shift_span<T>(span: HandleSpan<T>, rows: u32) -> HandleSpan<T> {
        if !span.start().is_valid() {
            return span;
        }
        HandleSpan::from_parts(shift(span.start(), rows), span.count())
    }

    pub(crate) fn append_rows<T: Default>(
        into: &mut Arena<T>,
        from: Arena<T>,
        mut rewrite: impl FnMut(T) -> T,
    ) {
        for row in from.into_items() {
            into.append(rewrite(row));
        }
    }
}
pub mod statement {
    pub use typed_trees::statement::{
        AssemblyFactKind, OutcomeProofSelector, StatementHandle, StatementNode, StatementTable,
        TableAssemblyFact, TableAssignment, TableCall, TableLocalData, TableNamePath,
        TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetHandle,
        TransitionTargetNode,
    };

    use arena::HandleSpan;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TransitionGuard {
        Always,
        When(crate::expression::Expression),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TransitionTarget {
        Named {
            path: crate::expression::NamePath,
            head_symbol: SymbolHandle,
            symbol: SymbolHandle,
            arguments: HandleSpan<crate::expression::Expression>,
        },
        Value(crate::expression::Expression),
        Terminal,
    }
}
mod values_and_views;
pub use values_and_views::{values, views};
pub use views::{
    CheckFactsRef, CheckFactsWalk, ContentProjectionFactsRef, ContentProjectionFactsWalk,
    FlowFactsRef, FlowFactsWalk, QualificationFactsRef, QualificationFactsWalk,
};

pub use keyed_rows::{DurableRows, KeyedRows};

pub use admissibility::{
    AcceptanceCheck, AcceptanceCheckProvenance, AcceptanceCheckVerdict, AcceptanceDimension,
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, CallAcceptance, ExitAcceptance,
    OperatorAcceptance, StateAcceptance, StateOperationAcceptance, StateOperationAcceptanceKind,
    StatementAcceptance,
};
pub use borrow::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallFact, BorrowCallReferenceSlotEffect,
    BorrowFacts, BorrowIncomingCompatibilityRequirement, BorrowLoanFact, BorrowLoanFormation,
    BorrowLoanLineage, BorrowLoanOwnerSegment, BorrowRetainedOperand, BorrowReturnedLoanFact,
    BorrowRootKind, BorrowWritableRootFact, CapturedPlace, CapturedPlaceCompatibility,
    CapturedPlaceContainment, CheckedBorrowParentEnd, CheckedBorrowResourceDispositionTarget,
    CheckedBorrowResourceLifecyclePhase, CheckedBorrowResourceRetention,
    CheckedCallBorrowLoanResource, CheckedCallBorrowLoanSource, CheckedDirectBorrowLoanResource,
    CheckedDirectBorrowParentLifetime, CheckedDirectBorrowRestorationObligation,
    CheckedOutputBorrowInput, CheckedOutputBorrowInputPermission,
    CheckedOutputLifetimeDependencyResource, CheckedOutputLifetimeDependencySource,
    CheckedParentBorrowResource, CheckedReborrowAccessEffect, CheckedReborrowLoanResource,
    CheckedReborrowParentEndStatus, CheckedReborrowParentSuspensionBoundary,
    CheckedReborrowResourceDisposition, CheckedReborrowResourceDispositionEvent,
    CheckedReborrowRestorationObligation, CheckedRetiredParentResourceDispositionStep,
    CheckedStoredCallBorrowInputs, CheckedSuppliedOutputBorrowResource, IncomingReferenceIdentity,
    IncomingReferenceOrigin, ParentLexicalStatusAtChildEnd, StateBorrowFact,
};
pub use facts::CheckedValueQualification;
pub use facts::{
    CheckedCaptureArgument, CheckedCaptureArgumentValue, CheckedCaptureInputs,
    CheckedDeclarationCall, CheckedDeclarationDispatchArm, CheckedExecutableBindings,
    CheckedExpressionSource, CheckedMachineRankCorrespondence, CheckedRankApplicationFacts,
    CheckedRankApplicationFormation, CheckedRankApplicationFormationHandle,
    CheckedRankApplicationInputHome, CheckedRankArrivalEdge, CheckedRankArrivalPlan,
    CheckedRankBodyJudgment, CheckedRankCoordinate, CheckedRankCoordinateBinding,
    CheckedRankCoordinateHandle, CheckedRankCoordinateTransport, CheckedRankLocalCapture,
    CheckedRankObservation, CheckedRankOperand, CheckedRankPremiseRegime,
    CheckedRankReceivingCoordinate, CheckedRankValueDerivation, CheckedRequirementMachines,
    CheckedRuntimeIntegerRank, CheckedRuntimeNaturalState, CheckedRuntimeRankComponent,
    CheckedRuntimeRankEdge, CheckedRuntimeRankInput, CheckedRuntimeRankMember,
    CheckedSelectedMachine, CheckedSourceCallTarget, CheckedSourceNaturalRankComponent,
    CheckedSourceNaturalRankEdge, CheckedSourceNaturalRankMember, CheckedSourceTransition,
    CheckedStateRankCorrespondence,
};
pub use facts::{
    CheckedCompilerEvaluationCall, CheckedCompilerEvaluationComponent,
    CheckedCompilerEvaluationDependencies, CheckedDeclarationCapture,
    CheckedDeclarationDescription, CheckedDeclarationDescriptions, CheckedDeclarationParameter,
    CheckedDeclarationSpecialization, CheckedMachineDescription, CheckedSelectionRequirement,
};
pub use facts::{
    ProofApplicationMachine, ProofApplicationPlan, ProofApplicationTargetMember, ProofApplications,
    ProofDeclarationDependencies,
};

pub use facts::{
    CheckedStoredIntegerWrite, ConstantRecipeObligation, ConstantRecipeRequirement,
    ContextualCleanupApplication, ContextualCleanupDisposition, ContextualCleanupEntryRequirement,
    ContextualCleanupEntryRequirements, ContextualCleanupExcludedRegion,
    ContextualCleanupExclusionCause, ContextualCleanupFacts, ContextualStorageEstablishment,
    DecidedCallRequires, ProvenAssignmentIndex, StoredIntegerWriteSite,
};

pub use facts::{
    BlockingFacts, CallServiceReachRows, CarryFacts, CheckFacts,
    CheckedAcceptedTemplateClassification, CheckedAcceptedTemplateIdentities,
    CheckedAcceptedTemplateIdentity, CheckedAccessViewInput, CheckedBodyCrashSummary,
    CheckedBoundaryAdapterDispatch, CheckedCallableHeader, CheckedCallbackResourceReceipt,
    CheckedCrashCallSite, CheckedCrashOperatorSite, CheckedCrashSite, CheckedEntryResourceEnvelope,
    CheckedFactCallProjection, CheckedInstalledInterruptRegistration,
    CheckedMachineContractEnvelopeIdentity, CheckedMachineContractRefinement,
    CheckedMachineResourceEnvelopes, CheckedMachineSpecializationIdentity,
    CheckedNominalMachineUse, CheckedProofRankingRelation, CheckedProofRecursiveCallSite,
    CheckedProofRecursiveComponent, CheckedProofRecursiveEdge, CheckedProofRecursiveMember,
    CheckedProofRecursiveTransitionLane, CheckedProviderCandidate,
    CheckedProviderCandidateDeclaration, CheckedProviderClosedRequirement,
    CheckedProviderClosedRequirements, CheckedProviderInstanceDeclaration,
    CheckedProviderInstancePublication, CheckedProviderInstanceRequirement,
    CheckedProviderReceivingApplication, CheckedProviderReceivingDescription,
    CheckedQuotientFormation, CheckedQuotientOperation, CheckedRequirementCallMachineSelection,
    CheckedRequirementCallSpecialization, CheckedRequirementCallTypeBinding,
    CheckedResourceAxisAnchor, CheckedResourceDerivationObligation,
    CheckedRuntimeMembershipSubject, CheckedSpecializationIdentities, CheckedStateLocal,
    CheckedSynchronousBoundaryTarget, CheckedTrappingSite, ClaimCarryPolicyFact,
    ClosedFloatRangeRequirement, ClosedIntegerRangeRequirement, ClosedScalarContractValue,
    ClosedScalarValueContractPlan, ContentClassFacts, ContentClassRow,
    ContentIdentityReshuffleFact, ContentPartitionCompositionFact,
    ContentPartitionInputClaimBinding, ContentPartitionPlaceSubstitution,
    ContentPartitionResultRewrite, ContentPartitionReturnFact, ContentProjectionFacts,
    ContentWalkRows, CrashCallSiteLocation, CrashCause, CrashContractCapsule, CrashInterface,
    CrashPlan, CrashPredicateExpression, CrashPredicateIdentity, CrashRouteBucket,
    CrashRouteBucketId, CrashRouteGuard, CrashSiteLocation, DataCarryFact,
    DeclaredCallableContract, DeclaredContractClause, DeclaredContractFact, DeclaredContractKind,
    DeclaredContractParameter, DeclaredContractStructure, DeclaredProgressPremise,
    DeclaredTermination, DomainDependencyFact, DomainDependencyPathFact, DomainFacts,
    DynamicConformanceFacts, DynamicConformanceRowFact, DynamicConformanceRowSource,
    DynamicSelectionFact, IndexCompatibilityDischarge, IndexCompatibilityFact,
    IndexCompatibilityFacts, MachineActivationCarryFact, MachineBlockingFact,
    MachineContractCommitment, MachineContractIdentity, MachineContractPlan, MachineContractPlans,
    MachineMutationFact, MachineQualifications, MachineServiceReachRows, MachineSuspensionFact,
    MachineSynchronousInvocationFact, MachineTerminationFact, MutationFacts,
    NominalMachineUseFacts, NominalMachineUseSite, OperatorSiteLocation, PerformedIntegerWrite,
    QualificationFacts, RealizedMachineContractEnvelope, ReferenceCustodyFacts,
    ReferenceCustodyRow, RequirementCallSpecializationFacts, ResolvedCarrierType,
    ResolvedExpressionType, ResolvedTypeFacts, RetainedBorrowCustodyFact, ServiceReachFacts,
    StateLocalFacts, StateServiceReachRows, StateWriteFramePlan, SuspensionCrossingBorrowFrontier,
    SuspensionCrossingCarryFact, SuspensionCrossingLiveValueFact, SuspensionCrossingLoanBacking,
    SuspensionCrossingLoanFact, SuspensionCrossingLoanSource, SuspensionCrossingStorage,
    SuspensionCrossingValueOrigin, SuspensionFacts, SuspensionValueBorrowFrontier,
    SynchronousInvocationFacts, TerminationFacts, VacuousQualificationUse,
    canonical_suspension_crossing_id, contract_declarations_commitment, contract_identity,
    contract_plan_by_machine, declared_termination_commitment,
};
pub use flow::*;
pub use operators::{
    CheckedArithmeticPolicyAdapter, CheckedBoundaryApplicationOperands,
    CheckedBoundaryBodyCandidate, CheckedBoundaryOperatorApplicationArgument,
    CheckedBoundaryOperatorApplicationDemand, CheckedBoundaryOperatorApplicationUseSite,
    CheckedBoundaryOperatorDeclaration, CheckedCrashQualifiedUse, CheckedNamedOperatorUseFact,
    CheckedNamedRequirementUseFact, CheckedOperatorCandidateFact, CheckedOperatorContractUse,
    CheckedOperatorCrashBucket, CheckedOperatorCrashContract, CheckedOperatorFacts,
    CheckedOperatorOccurrence, CheckedOperatorRealizationContract, CheckedOperatorResolutionIssue,
    CheckedOperatorResolutionStatus, CheckedOperatorUseFact, CheckedOperatorUseHandle,
    CheckedProviderPlanCommitment, CheckedProviderRequirement, CheckedProviderRequirementKind,
    CheckedSelectedFloatComparisonExecution, CheckedSymbolicBoundaryOperatorApplicationArgument,
    CheckedSymbolicBoundaryOperatorApplicationDemand,
};
pub use proof::{
    BoundaryQualificationAuthorization, CallContractSignature, CheckedDirectBlockFloatParameter,
    CheckedDirectCallFloatResult, CheckedDirectMachineFloatParameter,
    CheckedDirectMachineFloatResult, CheckedDirectOperationFloatResult,
    CheckedDirectStructuralFloatLeaf, CheckedEvidenceInterfaceIdentity, CheckedEvidenceProjection,
    CheckedEvidenceRequirementIdentity, CheckedEvidenceTerm,
    CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
    CheckedFloatMeaningProjectionError, CheckedFloatMeaningProjectionOccurrence,
    CheckedFloatMeaningProjectionOccurrenceId, CheckedFloatProjectionContractIdentity,
    CheckedFloatProjectionInput, CheckedFloatProjectionInputId, CheckedFloatProjectionSource,
    CheckedFloatSemanticApplication, CheckedFloatSemanticApplicationError,
    CheckedFloatSemanticApplicationOperand, CheckedFloatUseSite, CheckedIdentityFormation,
    CheckedIdentityFormationPlan, CheckedIdentityOwner, CheckedIdentityPublication,
    CheckedIndexedCitation, CheckedIndexedTheorem, CheckedIntegerAssignmentDecision,
    CheckedMathematicalBinder, CheckedMathematicalBinderKind, CheckedMathematicalBody,
    CheckedMathematicalDeclaration, CheckedMathematicalParameter, CheckedMathematicalSignature,
    CheckedProofOnlyValueType, CheckedProofPropositionId, CheckedProofValueDeclaration,
    CheckedProofValueId, CheckedPropositionApplication, CheckedPropositionBinder,
    CheckedPropositionBinderArgument, CheckedPropositionBinderArgumentKind,
    CheckedPropositionBinderKind, CheckedPropositionDeclaration, CheckedPropositionEvidence,
    CheckedPropositionVocabulary, CheckedSourceEvidenceCarrier, ContractCallFact,
    ContractEvidenceArgument, ContractExitFact, ContractExpressionEvidenceArgumentFact,
    ContractExpressionEvidenceCallFact, ContractExpressionStaticConformanceApplicationFact,
    ContractOperatorUseFact, ContractProofFact, ContractProofFactKind, ContractProofFactOwner,
    ContractProofFactRef, EvidenceAssignmentSource, EvidenceForwardingFact, InheritedContractScope,
    LemmaFacts, OutcomeOccurrenceRoot, OutcomeReferencedOccurrence, OutcomeSpecificArmFact,
    OutcomeSpecificArmRowFact, OutcomeSpecificEvidenceInterfaceScopeFact,
    OutcomeSpecificGuaranteeFact, OutcomeSpecificValidityFact, ProofFactKind, ProofFacts,
    ProofLemmaFact, ProofLemmaKind, ProofObligationFact, ProofObligationOwner, ProofOutputCallFact,
    ProofOutputEvidenceArgumentFact, ProofOutputFact, ProofOutputRuntimeCallFact,
    QuantifiedBoundFact, QuantifiedRangeFact, StaticRequirementDispatchFact,
};
pub use values::{
    CheckedArrayConstructionSource, CheckedBooleanExpression, CheckedByteSequenceOperand,
    CheckedCompletionArm, CheckedCompletionComparison, CheckedCompletionExpression,
    CheckedCompletionExpressionHandle, CheckedCompletionExpressionKind, CheckedCompletionPlans,
    CheckedCompletionRefusal, CheckedCompletionRefusalReason, CheckedCompletionRoot,
    CheckedDispatchSubject, CheckedElementViewRegion, CheckedEntryIndexBound,
    CheckedEntryIndexSelector, CheckedErasedProofParameterPlan, CheckedIeeeFloatComparisonKind,
    CheckedIntegerBinaryKind, CheckedIntegerComparisonKind, CheckedIntegerMathExpression,
    CheckedIntegerMathTerm, CheckedIntegerRange, CheckedLocatedProofTerm,
    CheckedLocatedScalarExpression, CheckedMathematicalArgument, CheckedProjectionSelectorRoot,
    CheckedProofTerm, CheckedProofTermField, CheckedProofTermRole, CheckedProofTerms,
    CheckedQuotientAdaptationLicense, CheckedReferenceSourceContract, CheckedScalarCallee,
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction, CheckedScalarComputation,
    CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarComputationPlans,
    CheckedScalarComputationRoot, CheckedScalarComputationStructuralArgument,
    CheckedScalarComputationTemporaryCleanup, CheckedScalarDispatchArm,
    CheckedScalarDispatchPattern, CheckedScalarEvaluationOperand, CheckedScalarExpression,
    CheckedScalarExpressionBindings, CheckedScalarExpressionPlans, CheckedScalarExpressionRole,
    CheckedScalarReadProjection, CheckedScalarSelector, CheckedSourceQualification,
    CheckedStorageRoot, CheckedStructuralCaseRoot, CheckedStructuralCaseSubject,
    CheckedStructuralDispatchArm, CheckedStructuralParameterField,
    CheckedStructuralPredicatePathSegment, CheckedStructuralQualificationContract,
    CheckedStructuralRecordField, CheckedStructuralRecordFieldValue, CheckedStructuralValue,
    CheckedStructuralValueHandle, CheckedStructuralValueKind, CheckedStructuralValuePlans,
    CheckedStructuralValueRoot, CheckedSubsliceSite, CheckedValueCompletion, CheckedValueFact,
    CheckedValueFacts, CheckedValueHandle, CheckedValueOrigin, CheckedValueStatementRole,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedTrees {
    pub typed: typed_trees::FrozenTypedTrees,
    pub facts: CheckFacts,
}

impl CheckedTrees {
    pub fn with_roots(typed: impl Into<typed_trees::FrozenTypedTrees>, facts: CheckFacts) -> Self {
        Self {
            typed: typed.into(),
            facts,
        }
    }

    pub fn body_crash_summary(
        &self,
        machine: symbols::SymbolHandle,
    ) -> Option<&CheckedBodyCrashSummary> {
        let mut bodies = self
            .typed
            .machines()
            .iter()
            .filter(|body| body.symbol == machine);
        let body = bodies.next()?;
        if bodies.next().is_some() {
            return None;
        }
        let mut plans = self
            .facts
            .contract_plans
            .machines()
            .iter()
            .filter(|plan| plan.machine == machine);
        let plan = plans.next()?;
        if plans.next().is_some() {
            return None;
        }
        plan.crash.checked_body_summary(machine, body.states)
    }

    pub fn landed_integer_constant(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<&numerics::literals::IntegerLiteral> {
        match self.typed.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Integer(literal) => Some(literal),
            _ => None,
        }
    }

    pub fn authored_declaration_selections(
        &self,
    ) -> &language_semantics::declaration_selection::AuthoredDeclarationSelections {
        self.facts.authored_declaration_selections()
    }

    pub fn consistent_terminal_origin(
        &self,
        mut origin: CheckedValueOrigin,
    ) -> Option<CheckedValueOrigin> {
        let mut parents = Vec::new();
        while let CheckedValueOrigin::NestedExpression { parent } = origin {
            if parents.contains(&parent) {
                return None;
            }
            parents.push(parent);
            let mut origins = self
                .facts
                .values
                .expression_values(parent)
                .map(|(_, value)| value.origin);
            let parent_origin = origins.next()?;
            if origins.any(|candidate| candidate != parent_origin) {
                return None;
            }
            origin = parent_origin;
        }
        Some(origin)
    }

    pub fn authored_selection_occurrences(
        &self,
        expression: expression::ExpressionHandle,
    ) -> impl Iterator<
        Item = language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
    > + '_ {
        self.facts
            .authored_selection_occurrences(&self.typed, expression)
    }
}

impl std::ops::Deref for CheckedTrees {
    type Target = typed_trees::TypedTrees;

    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

impl AsRef<typed_trees::TypedTrees> for CheckedTrees {
    fn as_ref(&self) -> &typed_trees::TypedTrees {
        &self.typed
    }
}

pub use facts::ConstantPredicateObligation;
pub use facts::{CheckedScalarReturnOrigin, CheckedScalarReturnQualification};
pub use facts::{IntegerRangeModel, IntegerRangeObligation, IntegerRangeRecipe};
// Compatibility while product consumers stop retaining the old representation.
pub type PreparedProviderInventory =
    typed_trees::provider_declarations::PreparedProviderInventory<CheckedTrees>;

pub mod checked_trees {
    pub use crate::*;
}
