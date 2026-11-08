/*
We represent contextual storage cleanup obligations (`contextual_cleanup`) and
recursive/transition termination ranking plans (`termination`) in
`CheckedTrees`.
*/

pub(crate) mod contextual_cleanup {
    use arena::{Arena, Handle, HandleSpan, OwnedIndex};
    use facts::{Place, PlaceSegment};

    use language_semantics::PermissionEventSource;
    use symbols::SymbolHandle;

    use crate::keyed_rows::KeyedRows;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ContextualCleanupExclusionCause {
        #[default]
        Unknown,
        Consumed,
        Unestablished,
        InactiveCase,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContextualCleanupExcludedRegion {
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<PlaceSegment>,
        pub cause: ContextualCleanupExclusionCause,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContextualCleanupFacts {
        pub dispositions: Arena<ContextualCleanupDisposition>,
        pub storage_establishments: Arena<ContextualStorageEstablishment>,
        pub selected_dispositions: Arena<Handle<ContextualCleanupDisposition>>,
        pub applications: Arena<ContextualCleanupApplication>,
        pub exclusions: Arena<ContextualCleanupExcludedRegion>,
        pub segments: Arena<PlaceSegment>,
        pub window_invocations: Arena<StorageWindowInvocation>,
        pub entry_requirements: ContextualCleanupEntryRequirements,
        pub state_index: KeyedRows<(SymbolHandle, SymbolHandle), ContextualCleanupDisposition>,
        selected_roster: OwnedIndex<Vec<Handle<ContextualCleanupDisposition>>>,
    }

    impl ContextualCleanupFacts {
        pub fn append_shifted(&mut self, other: Self) -> bool {
            use crate::shifted_rows::{append_rows, shift, shift_span, shiftable};
            let Self {
                dispositions,
                storage_establishments,
                selected_dispositions,
                applications,
                exclusions,
                segments,
                window_invocations,
                entry_requirements,
                state_index: _,
                selected_roster: _,
            } = other;
            let (
                Some(disposition_rows),
                Some(_),
                Some(_),
                Some(_),
                Some(exclusion_rows),
                Some(segment_rows),
                Some(_),
                Some(_),
            ) = (
                shiftable(&self.dispositions, &dispositions),
                shiftable(&self.storage_establishments, &storage_establishments),
                shiftable(&self.selected_dispositions, &selected_dispositions),
                shiftable(&self.applications, &applications),
                shiftable(&self.exclusions, &exclusions),
                shiftable(&self.segments, &segments),
                shiftable(&self.window_invocations, &window_invocations),
                shiftable(&self.entry_requirements.rows, &entry_requirements.rows),
            )
            else {
                return false;
            };
            let place = |place: Place| Place {
                root: place.root,
                segments: shift_span(place.segments, segment_rows),
            };
            append_rows(&mut self.dispositions, dispositions, |row| {
                let ContextualCleanupDisposition {
                    selection,
                    machine,
                    state,
                    source,
                    exit,
                    completion,
                    completion_constraints,
                    root,
                    disposed_type,
                    path,
                    exclusions,
                    definite_exclusions,
                    inactive_cases,
                    establishment_ordered,
                    zero_image,
                } = row;
                ContextualCleanupDisposition {
                    selection,
                    machine,
                    state,
                    source,
                    exit,
                    completion,
                    completion_constraints,
                    root,
                    disposed_type,
                    path,
                    exclusions: shift_span(exclusions, exclusion_rows),
                    definite_exclusions: shift_span(definite_exclusions, exclusion_rows),
                    inactive_cases: shift_span(inactive_cases, exclusion_rows),
                    establishment_ordered,
                    zero_image,
                }
            });
            append_rows(
                &mut self.storage_establishments,
                storage_establishments,
                |row| {
                    let ContextualStorageEstablishment {
                        prior_value_transferred,
                        machine,
                        state,
                        source,
                        destination,
                    } = row;
                    ContextualStorageEstablishment {
                        prior_value_transferred,
                        machine,
                        state,
                        source,
                        destination: place(destination),
                    }
                },
            );
            append_rows(
                &mut self.selected_dispositions,
                selected_dispositions,
                |row| shift(row, disposition_rows),
            );
            append_rows(&mut self.applications, applications, |row| {
                let ContextualCleanupApplication { template, instance } = row;
                ContextualCleanupApplication {
                    template: shift(template, disposition_rows),
                    instance: shift(instance, disposition_rows),
                }
            });
            append_rows(&mut self.exclusions, exclusions, |row| {
                let ContextualCleanupExcludedRegion {
                    root,
                    segments,
                    cause,
                } = row;
                ContextualCleanupExcludedRegion {
                    root,
                    segments: shift_span(segments, segment_rows),
                    cause,
                }
            });
            append_rows(&mut self.segments, segments, |row| row);
            append_rows(&mut self.window_invocations, window_invocations, |row| {
                let StorageWindowInvocation {
                    machine,
                    state,
                    statement,
                    expression,
                    absent,
                    opened_statement,
                } = row;
                StorageWindowInvocation {
                    machine,
                    state,
                    statement,
                    expression,
                    absent: place(absent),
                    opened_statement,
                }
            });
            let ContextualCleanupEntryRequirements { rows } = entry_requirements;
            append_rows(&mut self.entry_requirements.rows, rows, |row| row);
            true
        }

        pub fn replaces_transferred_storage(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
            source: PermissionEventSource,
            root: facts::PlaceRoot,
            path: &[PlaceSegment],
        ) -> bool {
            let mut matched = false;
            for (_, establishment) in self.storage_establishments.iter() {
                if establishment.machine == machine
                    && establishment.state == state
                    && establishment.source == source
                    && establishment.destination.root == root
                {
                    let Some(destination) = self.segments.span(establishment.destination.segments)
                    else {
                        return false;
                    };
                    if destination != path {
                        continue;
                    }
                    if !establishment.prior_value_transferred {
                        return false;
                    }
                    matched = true;
                }
            }
            matched
        }
        fn with_state_dispositions<Result>(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
            read: impl FnOnce(&[Handle<ContextualCleanupDisposition>]) -> Result,
        ) -> Result {
            self.state_index.with_rows(
                &self.dispositions,
                |disposition| (disposition.machine, disposition.state),
                &(machine, state),
                read,
            )
        }

        pub fn is_selected_disposition(
            &self,
            handle: Handle<ContextualCleanupDisposition>,
        ) -> bool {
            self.selected_roster.with(
                self.selected_dispositions.revision(),
                || {
                    let mut selected: Vec<_> = self
                        .selected_dispositions
                        .iter()
                        .map(|(_, candidate)| *candidate)
                        .collect();
                    selected
                        .sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
                    selected.dedup();
                    selected
                },
                |selected| {
                    selected
                        .binary_search_by_key(
                            &(handle.arena_index(), handle.generation()),
                            |candidate| (candidate.arena_index(), candidate.generation()),
                        )
                        .is_ok()
                },
            )
        }

        pub fn for_each_state_disposition(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
            mut visit: impl FnMut(Handle<ContextualCleanupDisposition>, &ContextualCleanupDisposition),
        ) {
            self.with_state_dispositions(machine, state, |rows| {
                for &handle in rows {
                    visit(handle, self.dispositions.get(handle));
                }
            });
        }

        pub fn any_state_disposition(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
            mut predicate: impl FnMut(
                Handle<ContextualCleanupDisposition>,
                &ContextualCleanupDisposition,
            ) -> bool,
        ) -> bool {
            self.with_state_dispositions(machine, state, |rows| {
                rows.iter()
                    .any(|&handle| predicate(handle, self.dispositions.get(handle)))
            })
        }

        pub fn state_disposition_handles(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
        ) -> Vec<Handle<ContextualCleanupDisposition>> {
            self.with_state_dispositions(
                machine,
                state,
                <[Handle<ContextualCleanupDisposition>]>::to_vec,
            )
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContextualStorageEstablishment {
        pub prior_value_transferred: bool,
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub source: PermissionEventSource,
        pub destination: Place,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct StorageWindowInvocation {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub statement: usize,
        pub expression: typed_trees::expression::ExpressionHandle,
        pub absent: Place,
        pub opened_statement: Option<usize>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContextualCleanupDisposition {
        pub selection: Handle<crate::FlowOwnedSelectionReceipt>,
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub source: PermissionEventSource,
        pub exit: crate::FlowClaimJoinExit,
        pub completion: typed_trees::expression::ExpressionHandle,
        pub completion_constraints: Option<HandleSpan<crate::FlowConstraintRef>>,
        pub root: facts::PlaceRoot,
        pub disposed_type: typed_trees::types::TypeReferenceHandle,
        pub path: HandleSpan<PlaceSegment>,
        pub exclusions: HandleSpan<ContextualCleanupExcludedRegion>,
        pub definite_exclusions: HandleSpan<ContextualCleanupExcludedRegion>,
        pub inactive_cases: HandleSpan<ContextualCleanupExcludedRegion>,
        pub establishment_ordered: bool,
        pub zero_image: bool,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContextualCleanupApplication {
        pub template: Handle<ContextualCleanupDisposition>,
        pub instance: Handle<ContextualCleanupDisposition>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContextualCleanupEntryRequirements {
        pub rows: Arena<ContextualCleanupEntryRequirement>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ContextualCleanupEntryRequirement {
        pub machine: SymbolHandle,
        pub consequence: crate::CheckedBooleanExpression,
        pub antecedent: crate::CheckedBooleanExpression,
        pub hook_machine: SymbolHandle,
        pub hook_clause: typed_trees::expression::ExpressionHandle,
        pub template_machine: SymbolHandle,
    }

    impl Default for ContextualCleanupEntryRequirement {
        fn default() -> Self {
            Self {
                machine: SymbolHandle::invalid(),
                consequence: crate::CheckedBooleanExpression::Constant(false),
                antecedent: crate::CheckedBooleanExpression::Constant(true),
                hook_machine: SymbolHandle::invalid(),
                hook_clause: Default::default(),
                template_machine: SymbolHandle::invalid(),
            }
        }
    }

    impl ContextualCleanupEntryRequirement {
        pub fn predicate(&self) -> crate::CheckedBooleanExpression {
            use crate::CheckedBooleanExpression;
            if self.antecedent == CheckedBooleanExpression::Constant(true) {
                self.consequence.clone()
            } else {
                CheckedBooleanExpression::Or {
                    left: Box::new(CheckedBooleanExpression::Not(Box::new(
                        self.antecedent.clone(),
                    ))),
                    right: Box::new(self.consequence.clone()),
                }
            }
        }

        pub fn direct_boolean_subject(
            &self,
        ) -> Option<(u32, &[crate::CheckedStructuralPredicatePathSegment], bool)> {
            use crate::CheckedBooleanExpression;
            let (subject, expected) = match &self.consequence {
                CheckedBooleanExpression::Not(subject) => (subject.as_ref(), false),
                subject => (subject, true),
            };
            let CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
                ..
            } = subject
            else {
                return None;
            };
            Some((*parameter_position, path, expected))
        }
    }

    #[cfg(test)]
    mod index_tests {
        use super::{ContextualCleanupDisposition, ContextualCleanupFacts, SymbolHandle};

        #[test]
        fn selected_membership_preserves_full_handles_and_sparse_rosters() {
            let mut facts = ContextualCleanupFacts::default();
            let first = facts
                .dispositions
                .insert(ContextualCleanupDisposition::default());
            facts.selected_dispositions.insert(first);
            assert!(facts.is_selected_disposition(first));
            let last = facts
                .dispositions
                .insert(ContextualCleanupDisposition::default());
            facts.selected_dispositions.insert(last);
            assert!(facts.is_selected_disposition(last));
            assert!(facts.dispositions.free(first));
            assert!(facts.is_selected_disposition(last));
            let replacement = facts
                .dispositions
                .insert(ContextualCleanupDisposition::default());
            assert_eq!(first.arena_index(), replacement.arena_index());
            assert_ne!(first.generation(), replacement.generation());
            assert!(!facts.is_selected_disposition(replacement));
            facts.selected_dispositions.insert(replacement);
            assert!(facts.is_selected_disposition(replacement));
            let machine = SymbolHandle::from_parts(4, 1);
            let state = SymbolHandle::from_parts(5, 1);
            let moved = SymbolHandle::from_parts(6, 1);
            assert!(facts.state_disposition_handles(machine, state).is_empty());
            facts.dispositions.get_mut(last).machine = machine;
            facts.dispositions.get_mut(last).state = state;
            assert_eq!(facts.state_disposition_handles(machine, state), [last]);
            assert!(facts.any_state_disposition(machine, state, |_, _| true));
            facts.dispositions.get_mut(last).state = moved;
            assert!(facts.state_disposition_handles(machine, state).is_empty());
            assert_eq!(facts.state_disposition_handles(machine, moved), [last]);
        }
    }
}

pub(crate) mod termination {
    use language_semantics::MachineTerminationPlan;
    use symbols::SymbolHandle;
    use typed_trees::type_identity::TypeIdentityId;

    pub(crate) mod application_formation {
        use arena::{Handle, HandleSpan};
        use symbols::SymbolHandle;
        use typed_trees::{expression::ExpressionHandle, state::State};

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedDeclarationCall {
            pub expression: ExpressionHandle,
            pub authored_target: SymbolHandle,
            pub arguments: HandleSpan<ExpressionHandle>,
            pub checked_arguments: HandleSpan<ExpressionHandle>,
            pub target_machine: SymbolHandle,
            pub target_state: SymbolHandle,
            pub target_contract: Option<crate::MachineContractIdentity>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedDeclarationDispatchArm {
            pub expression: ExpressionHandle,
            pub source_arm: Handle<typed_trees::expression::TableMatchArm>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRankApplicationFacts {
            pub owner: SymbolHandle,
            pub definition_body: ExpressionHandle,
            pub casts: HandleSpan<crate::operators::ExactIntegerCastFact>,
            pub calls: HandleSpan<CheckedDeclarationCall>,
            pub dispatches: HandleSpan<CheckedDeclarationDispatchArm>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRankApplicationFormation {
            pub machine: SymbolHandle,
            pub machine_body: HandleSpan<State>,
            pub state: SymbolHandle,
            pub application: CheckedRankApplicationFacts,
            pub bindings: HandleSpan<super::CheckedRankCoordinateBinding>,
            pub input_home: Option<CheckedRankApplicationInputHome>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRankApplicationInputHome {
            pub formal: SymbolHandle,
            pub actual: super::CheckedRankCoordinateHandle,
            pub actual_type: typed_trees::types::TypeReferenceHandle,
        }

        pub type CheckedRankApplicationFormationHandle = Handle<CheckedRankApplicationFormation>;
    }
    pub use application_formation::{
        CheckedDeclarationCall, CheckedDeclarationDispatchArm, CheckedRankApplicationFacts,
        CheckedRankApplicationFormation, CheckedRankApplicationFormationHandle,
        CheckedRankApplicationInputHome,
    };

    pub(crate) mod arrival_correspondence {
        use arena::{Arena, Handle, HandleSpan};
        use facts::PlaceSegment;
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::state::State;
        use typed_trees::statement::TransitionTargetHandle;
        use typed_trees::types::TypeReferenceHandle;

        pub type CheckedRankCoordinateHandle = Handle<CheckedRankCoordinate>;

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedRankValueDerivation {
            #[default]
            CapturedCoordinate,
            ComputedDependency,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRankCoordinate {
            pub parameter: SymbolHandle,
            pub path: HandleSpan<PlaceSegment>,
            pub entry_indexes: HandleSpan<crate::CheckedEntryIndexBound>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRankLocalCapture {
            pub symbol: SymbolHandle,
            pub statement_ordinal: u32,
            pub initializer: ExpressionHandle,
            pub occurrence: ExpressionHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRankCoordinateTransport {
            pub source_state: SymbolHandle,
            pub source: CheckedRankCoordinateHandle,
            pub selected_suffix: HandleSpan<PlaceSegment>,
            pub statement_ordinal: u32,
            pub target: TransitionTargetHandle,
            pub actual: ExpressionHandle,
            pub completion: ExpressionHandle,
            pub derivation: CheckedRankValueDerivation,
            pub captures: HandleSpan<CheckedRankLocalCapture>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRankReceivingCoordinate {
            pub destination: CheckedRankCoordinateHandle,
            pub type_reference: TypeReferenceHandle,
            pub source_state: SymbolHandle,
            pub statement_ordinal: u32,
            pub target: TransitionTargetHandle,
            pub actual: ExpressionHandle,
            pub completion: ExpressionHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRankCoordinateBinding {
            pub application_context: HandleSpan<ExpressionHandle>,
            pub entry: CheckedRankCoordinateHandle,
            pub destination: CheckedRankCoordinateHandle,
            pub transports: HandleSpan<CheckedRankCoordinateTransport>,
            pub receiving: HandleSpan<CheckedRankReceivingCoordinate>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedStateRankCorrespondence {
            pub application: super::CheckedRankApplicationFacts,
            pub state: SymbolHandle,
            pub bindings: HandleSpan<CheckedRankCoordinateBinding>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedRankPremiseRegime {
            #[default]
            RankInvariant,
            EntryInvariant,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRankArrivalEdge {
            pub source: SymbolHandle,
            pub destination: SymbolHandle,
            pub statement_ordinal: u32,
            pub target: TransitionTargetHandle,
            pub non_increasing: bool,
            pub strictly_decreases: bool,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedMachineRankCorrespondence {
            pub machine: SymbolHandle,
            pub body: HandleSpan<State>,
            pub entry_state: SymbolHandle,
            pub premise_regime: CheckedRankPremiseRegime,
            pub states: HandleSpan<CheckedStateRankCorrespondence>,
            pub edges: HandleSpan<CheckedRankArrivalEdge>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRankBodyJudgment {
            pub machine: SymbolHandle,
            pub body: HandleSpan<State>,
            pub local_terminates: bool,
            pub entry_rank_formed: bool,
            pub observations: HandleSpan<crate::CheckedStateNaturalRank>,
            pub diagnostic: String,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRuntimeRankMember {
            pub machine: SymbolHandle,
            pub body: HandleSpan<State>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedRankObservation {
            Value,
            Length,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedRankOperand {
            Coordinate {
                coordinate: CheckedRankCoordinateHandle,
                primitive_type: typed_trees::types::PrimitiveType,
                observation: CheckedRankObservation,
            },
            Expression {
                state: SymbolHandle,
                expression: typed_trees::expression::ExpressionHandle,
                primitive_type: typed_trees::types::PrimitiveType,
                formation: super::CheckedRankApplicationFormationHandle,
            },
            Literal(u64),
        }

        impl Default for CheckedRankOperand {
            fn default() -> Self {
                Self::Coordinate {
                    coordinate: CheckedRankCoordinateHandle::invalid(),
                    primitive_type: typed_trees::types::PrimitiveType::Bool,
                    observation: CheckedRankObservation::Value,
                }
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedRuntimeRankInput {
            Value(CheckedRankObservation),
            Distance {
                lower_observation: CheckedRankObservation,
                lower_primitive: typed_trees::types::PrimitiveType,
                upper: Handle<CheckedRankOperand>,
            },
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRuntimeNaturalState {
            pub state: SymbolHandle,
            pub values: HandleSpan<CheckedRuntimeIntegerRank>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRuntimeIntegerRank {
            pub application: super::CheckedRankApplicationFacts,
            pub application_bindings: HandleSpan<CheckedRankCoordinateBinding>,
            pub input_type: TypeReferenceHandle,
            pub measure: SymbolHandle,
            pub coordinate: CheckedRankCoordinateHandle,
            pub parameter_position: u32,
            pub primitive_type: typed_trees::types::PrimitiveType,
            pub observation: CheckedRuntimeRankInput,
            pub function: crate::CheckedNaturalRankFunction,
        }

        impl Default for CheckedRuntimeIntegerRank {
            fn default() -> Self {
                Self {
                    application: Default::default(),
                    application_bindings: Default::default(),
                    input_type: TypeReferenceHandle::invalid(),
                    measure: SymbolHandle::invalid(),
                    coordinate: CheckedRankCoordinateHandle::invalid(),
                    parameter_position: 0,
                    primitive_type: typed_trees::types::PrimitiveType::Bool,
                    observation: CheckedRuntimeRankInput::Value(CheckedRankObservation::Value),
                    function: crate::CheckedNaturalRankFunction::Identity,
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRuntimeRankEdge {
            pub source: SymbolHandle,
            pub target: SymbolHandle,
            pub statement_ordinal: u32,
            pub strict: bool,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRuntimeRankComponent {
            pub members: HandleSpan<CheckedRuntimeRankMember>,
            pub states: HandleSpan<CheckedRuntimeNaturalState>,
            pub edges: HandleSpan<CheckedRuntimeRankEdge>,
            pub diagnostic: String,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedSourceNaturalRankMember {
            pub machine: SymbolHandle,
            pub body: HandleSpan<State>,
            pub subject: ExpressionHandle,
            pub carrier: TypeReferenceHandle,
            pub realized_from: Option<SymbolHandle>,
            pub realized_parameters: Vec<(SymbolHandle, SymbolHandle)>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedSourceNaturalRankEdge {
            pub caller: SymbolHandle,
            pub callee: SymbolHandle,
            pub source_callee: SymbolHandle,
            pub state: SymbolHandle,
            pub statement_ordinal: u32,
            pub call_ordinal: u32,
            pub authored_application:
                typed_trees::static_applications::StaticMachineApplicationHandle,
            pub argument_position: Option<u32>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedSourceNaturalRankComponent {
            pub members: HandleSpan<CheckedSourceNaturalRankMember>,
            pub edges: HandleSpan<CheckedSourceNaturalRankEdge>,
            pub view: language_semantics::RankingViewId,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRankArrivalPlan {
            pub application_contexts: Arena<ExpressionHandle>,
            pub application_casts: Arena<crate::operators::ExactIntegerCastFact>,
            pub application_calls: Arena<super::CheckedDeclarationCall>,
            pub application_dispatches: Arena<super::CheckedDeclarationDispatchArm>,
            pub application_call_arguments: Arena<typed_trees::expression::ExpressionHandle>,
            pub application_formations: Arena<super::CheckedRankApplicationFormation>,
            pub judgments: Arena<CheckedRankBodyJudgment>,
            pub machines: Arena<CheckedMachineRankCorrespondence>,
            pub states: Arena<CheckedStateRankCorrespondence>,
            pub bindings: Arena<CheckedRankCoordinateBinding>,
            pub coordinates: Arena<CheckedRankCoordinate>,
            pub rank_operands: Arena<CheckedRankOperand>,
            pub entry_indexes: Arena<crate::CheckedEntryIndexBound>,
            pub path_segments: Arena<PlaceSegment>,
            pub transports: Arena<CheckedRankCoordinateTransport>,
            pub receiving: Arena<CheckedRankReceivingCoordinate>,
            pub captures: Arena<CheckedRankLocalCapture>,
            pub arrival_edges: Arena<CheckedRankArrivalEdge>,
            pub observations: Arena<crate::CheckedStateNaturalRank>,
            pub observation_components: Arena<crate::CheckedIntegerRankObservation>,
            pub runtime_components: Arena<CheckedRuntimeRankComponent>,
            pub runtime_members: Arena<CheckedRuntimeRankMember>,
            pub runtime_states: Arena<CheckedRuntimeNaturalState>,
            pub runtime_values: Arena<CheckedRuntimeIntegerRank>,
            pub runtime_edges: Arena<CheckedRuntimeRankEdge>,
            pub source_natural_components: Arena<CheckedSourceNaturalRankComponent>,
            pub source_natural_members: Arena<CheckedSourceNaturalRankMember>,
            pub source_natural_edges: Arena<CheckedSourceNaturalRankEdge>,
        }

        impl CheckedRankArrivalPlan {
            pub fn judgment_for_body(
                &self,
                machine: SymbolHandle,
                body: HandleSpan<State>,
            ) -> Option<&CheckedRankBodyJudgment> {
                let mut rows = self
                    .judgments
                    .iter()
                    .map(|(_, row)| row)
                    .filter(|row| row.machine == machine && row.body == body);
                let row = rows.next()?;
                rows.next().is_none().then_some(row)
            }

            pub fn for_body(
                &self,
                machine: SymbolHandle,
                body: HandleSpan<State>,
            ) -> Option<&CheckedMachineRankCorrespondence> {
                let mut witnesses = self
                    .machines
                    .iter()
                    .map(|(_, witness)| witness)
                    .filter(|witness| witness.machine == machine && witness.body == body);
                let witness = witnesses.next()?;
                witnesses.next().is_none().then_some(witness)
            }
        }
    }
    pub use arrival_correspondence::{
        CheckedMachineRankCorrespondence, CheckedRankArrivalEdge, CheckedRankArrivalPlan,
        CheckedRankBodyJudgment, CheckedRankCoordinate, CheckedRankCoordinateBinding,
        CheckedRankCoordinateHandle, CheckedRankCoordinateTransport, CheckedRankLocalCapture,
        CheckedRankObservation, CheckedRankOperand, CheckedRankPremiseRegime,
        CheckedRankReceivingCoordinate, CheckedRankValueDerivation, CheckedRuntimeIntegerRank,
        CheckedRuntimeNaturalState, CheckedRuntimeRankComponent, CheckedRuntimeRankEdge,
        CheckedRuntimeRankInput, CheckedRuntimeRankMember, CheckedSourceNaturalRankComponent,
        CheckedSourceNaturalRankEdge, CheckedSourceNaturalRankMember,
        CheckedStateRankCorrespondence,
    };

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminationFacts {
        pub rank_arrivals: CheckedRankArrivalPlan,
        pub machines: Vec<MachineTerminationFact>,
        pub proof_recursive_components: Vec<CheckedProofRecursiveComponent>,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum CheckedProofRankingRelation {
        StructuralSubterm,
        StructuralSubtermCycles,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedProofRecursiveCallSite {
        Statement {
            state: SymbolHandle,
            statement_index: usize,
        },
        Expression {
            state: SymbolHandle,
            statement_index: usize,
            expression_ordinal: usize,
        },
        Transition {
            state: SymbolHandle,
            statement_index: usize,
            lane: CheckedProofRecursiveTransitionLane,
        },
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum CheckedProofRecursiveTransitionLane {
        Target,
        Continuation,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProofRecursiveMember {
        pub machine: SymbolHandle,
        pub rank_parameter: SymbolHandle,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProofRecursiveEdge {
        pub caller: SymbolHandle,
        pub callee: SymbolHandle,
        pub site: CheckedProofRecursiveCallSite,
        pub caller_rank_parameter: SymbolHandle,
        pub callee_rank_parameter: SymbolHandle,
        pub strict_member_path: Vec<SymbolHandle>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProofRecursiveComponent {
        pub members: Vec<CheckedProofRecursiveMember>,
        pub ranking_relation: CheckedProofRankingRelation,
        pub rank_type_identity: TypeIdentityId,
        pub edges: Vec<CheckedProofRecursiveEdge>,
    }

    impl TerminationFacts {
        pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineTerminationPlan> {
            self.machines
                .iter()
                .find(|fact| fact.machine == machine)
                .map(|fact| &fact.plan)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineTerminationFact {
        pub machine: SymbolHandle,
        pub plan: MachineTerminationPlan,
    }
}
