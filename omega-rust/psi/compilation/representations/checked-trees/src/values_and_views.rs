/*
We represent checked value computations, structural values, and proof terms
(`values`) alongside borrowed traversal views (`views`) over `CheckFacts` in
`CheckedTrees`.
*/

pub mod values {

    use arena::{Arena, Handle, HandleSpan};
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::TypeReferenceHandle;

    use crate::keyed_rows::KeyedRows;

    pub use facts::{
        CheckedBooleanExpression, CheckedByteSequenceOperand, CheckedEntryIndexBound,
        CheckedEntryIndexSelector, CheckedIeeeFloatComparisonKind, CheckedIntegerBinaryKind,
        CheckedIntegerComparisonKind, CheckedIntegerMathExpression, CheckedIntegerMathTerm,
        CheckedScalarExpression, CheckedScalarReadProjection, CheckedScalarSelector,
        CheckedStorageRoot, CheckedStructuralCaseRoot, CheckedStructuralCaseSubject,
        CheckedStructuralParameterField, CheckedStructuralPredicatePathSegment,
    };

    pub(crate) mod computations {

        use super::{
            Arena, CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
            Handle, HandleSpan, SymbolHandle,
        };
        use crate::CheckedMemberIdentity;
        use crate::CheckedUnitStructuralArgumentPlan;
        use crate::keyed_rows::KeyedRows;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::type_identity::TypeIdentityId;
        use typed_trees::types::PrimitiveType;

        pub type CheckedScalarComputationHandle = Handle<CheckedScalarComputation>;

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedDispatchSubject {
            Scalar(CheckedScalarComputationHandle),
            Structural {
                /// Exact subject occurrence whose ordered place selectors completed.
                expression: ExpressionHandle,
                value: crate::CheckedStructuralValueHandle,
                place: crate::CheckedUnitStructuralArgumentPlan,
                /// Completed-home name for lexical SavedSubject reads or, for a
                /// tag-only place observation, its exact private subject occurrence.
                /// A produced value needs no alias when no payload binding reads it.
                alias: ExpressionHandle,
            },
        }

        impl CheckedDispatchSubject {
            pub fn scalar_operands<'a>(
                &'a self,
                operands: &'a arena::Arena<CheckedScalarComputationHandle>,
            ) -> impl Iterator<Item = CheckedScalarComputationHandle> + 'a {
                let (scalar, place) = match self {
                    Self::Scalar(value) => (Some(*value), None),
                    Self::Structural { place, .. } => (None, Some(place)),
                };
                scalar.into_iter().chain(
                    place
                        .into_iter()
                        .flat_map(move |place| place.computation_operands(operands)),
                )
            }

            pub fn structural_value(&self) -> crate::CheckedStructuralValueHandle {
                match self {
                    Self::Structural { value, .. } => *value,
                    Self::Scalar(_) => Handle::invalid(),
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedScalarCaseConstruction {
            pub expression: ExpressionHandle,
            pub type_reference: typed_trees::types::TypeReferenceHandle,
            pub case: SymbolHandle,
            pub fields: HandleSpan<CheckedScalarCaseComputationField>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedScalarCaseComputationField {
            pub symbol: SymbolHandle,
            pub value: CheckedScalarComputationHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedScalarComputationStructuralArgument {
            Place(CheckedUnitStructuralArgumentPlan),
            Case(CheckedScalarCaseConstruction),
        }

        impl Default for CheckedScalarComputationStructuralArgument {
            fn default() -> Self {
                Self::Place(CheckedUnitStructuralArgumentPlan::default())
            }
        }

        impl CheckedScalarComputationStructuralArgument {
            pub fn as_place(&self) -> Option<&CheckedUnitStructuralArgumentPlan> {
                match self {
                    Self::Place(place) => Some(place),
                    Self::Case(_) => None,
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedScalarComputationTemporaryCleanup {
            pub partitions: HandleSpan<crate::CheckedCleanupPartitionRef>,
            pub argument_ordinal: Option<u32>,
            pub operand: Handle<crate::FlowOperandFact>,
            pub owner: crate::CheckedStructuralValueHandle,
            pub actions: HandleSpan<crate::CheckedNormalCleanupAction>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedScalarEvaluationOperand {
            Scalar(CheckedScalarComputationHandle),
            Structural(super::structural_values::CheckedStructuralValueHandle),
        }

        impl Default for CheckedScalarEvaluationOperand {
            fn default() -> Self {
                Self::Scalar(CheckedScalarComputationHandle::invalid())
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedScalarComputationPlans {
            pub roots: Arena<CheckedScalarComputationRoot>,
            pub projection_selectors: Arena<CheckedProjectionSelectorRoot>,
            pub nodes: Arena<CheckedScalarComputation>,
            pub operands: Arena<CheckedScalarComputationHandle>,
            pub evaluation_operands: Arena<CheckedScalarEvaluationOperand>,
            pub observation_collections: Arena<typed_trees::types::TypeReferenceHandle>,
            pub structural_arguments: Arena<CheckedScalarComputationStructuralArgument>,
            pub temporary_cleanups: Arena<CheckedScalarComputationTemporaryCleanup>,
            pub temporary_cleanup_actions: Arena<crate::CheckedNormalCleanupAction>,
            pub claim_transfers: Arena<crate::CheckedUnitClaimTransferPlan>,
            pub dispatch_arms: Arena<CheckedScalarDispatchArm>,
            pub case_fields: Arena<CheckedScalarCaseComputationField>,
            #[doc(hidden)]
            pub roots_by_state: KeyedRows<SymbolHandle, CheckedScalarComputationRoot>,
            #[doc(hidden)]
            pub roots_by_machine: KeyedRows<SymbolHandle, CheckedScalarComputationRoot>,
            #[doc(hidden)]
            pub projection_selectors_by_state:
                KeyedRows<SymbolHandle, CheckedProjectionSelectorRoot>,
        }

        impl CheckedScalarComputationPlans {
            pub fn seal(&mut self) {
                self.roots_by_state.seal(&self.roots, |root| root.state);
                self.roots_by_machine.seal(&self.roots, |root| root.machine);
                self.projection_selectors_by_state
                    .seal(&self.projection_selectors, |root| root.state);
            }

            pub fn projection_selectors_at(
                &self,
                machine: SymbolHandle,
                state: SymbolHandle,
                statement: u32,
                expression: ExpressionHandle,
            ) -> Option<HandleSpan<CheckedScalarComputationHandle>> {
                self.projection_selectors_by_state.with_rows(
                    &self.projection_selectors,
                    |root| root.state,
                    &state,
                    |handles| {
                        let mut roots = handles
                            .iter()
                            .map(|handle| self.projection_selectors.get(*handle))
                            .filter(|root| {
                                root.machine == machine
                                    && root.state == state
                                    && root.statement_ordinal == statement
                                    && root.expression == expression
                            });
                        let root = roots.next()?;
                        roots.next().is_none().then_some(root.evaluations)
                    },
                )
            }

            fn with_state_roots<R>(
                &self,
                state: SymbolHandle,
                run: impl FnOnce(&[Handle<CheckedScalarComputationRoot>]) -> R,
            ) -> R {
                self.roots_by_state
                    .with_rows(&self.roots, |root| root.state, &state, run)
            }

            fn with_machine_roots<R>(
                &self,
                machine: SymbolHandle,
                run: impl FnOnce(&[Handle<CheckedScalarComputationRoot>]) -> R,
            ) -> R {
                self.roots_by_machine
                    .with_rows(&self.roots, |root| root.machine, &machine, run)
            }

            pub fn root_at(
                &self,
                state: SymbolHandle,
                statement_ordinal: u32,
                role: CheckedScalarExpressionRole,
            ) -> Option<&CheckedScalarComputationRoot> {
                self.with_state_roots(state, |handles| {
                    let mut roots =
                        handles
                            .iter()
                            .map(|handle| self.roots.get(*handle))
                            .filter(|root| {
                                root.state == state
                                    && root.statement_ordinal == statement_ordinal
                                    && root.role == role
                            });
                    let root = roots.next()?;
                    roots.next().is_none().then_some(root)
                })
            }

            pub fn roots_for_statement_role(
                &self,
                state: SymbolHandle,
                statement_ordinal: u32,
                role: CheckedScalarExpressionRole,
            ) -> impl Iterator<Item = &CheckedScalarComputationRoot> {
                let mut first_two = [None, None];
                let mut overflow = Vec::new();
                self.with_state_roots(state, |handles| {
                    for handle in handles {
                        let root = self.roots.get(*handle);
                        if root.state == state
                            && root.statement_ordinal == statement_ordinal
                            && root.role == role
                        {
                            if first_two[0].is_none() {
                                first_two[0] = Some(root);
                            } else if first_two[1].is_none() {
                                first_two[1] = Some(root);
                            } else {
                                overflow.push(root);
                            }
                        }
                    }
                });
                first_two.into_iter().flatten().chain(overflow)
            }

            pub fn roots_for_statement(
                &self,
                state: SymbolHandle,
                statement_ordinal: u32,
            ) -> Vec<&CheckedScalarComputationRoot> {
                self.with_state_roots(state, |handles| {
                    handles
                        .iter()
                        .map(|handle| self.roots.get(*handle))
                        .filter(|root| {
                            root.state == state && root.statement_ordinal == statement_ordinal
                        })
                        .collect()
                })
            }

            pub fn root_records_for_state(
                &self,
                state: SymbolHandle,
            ) -> Vec<&CheckedScalarComputationRoot> {
                self.with_state_roots(state, |handles| {
                    handles
                        .iter()
                        .map(|handle| self.roots.get(*handle))
                        .filter(|root| root.state == state)
                        .collect()
                })
            }

            pub fn roots_for_state(
                &self,
                state: SymbolHandle,
            ) -> Vec<CheckedScalarComputationHandle> {
                self.with_state_roots(state, |handles| {
                    handles
                        .iter()
                        .map(|handle| self.roots.get(*handle))
                        .filter_map(|root| (root.state == state).then_some(root.root))
                        .collect()
                })
            }

            pub fn roots_for_machine(
                &self,
                machine: SymbolHandle,
            ) -> Vec<CheckedScalarComputationHandle> {
                self.with_machine_roots(machine, |handles| {
                    handles
                        .iter()
                        .map(|handle| self.roots.get(*handle))
                        .filter_map(|root| (root.machine == machine).then_some(root.root))
                        .collect()
                })
            }

            fn machine_root_handles(
                &self,
                machines: &[SymbolHandle],
            ) -> Vec<Handle<CheckedScalarComputationRoot>> {
                match machines {
                    [] => Vec::new(),
                    [machine] => self.with_machine_roots(*machine, |handles| handles.to_vec()),
                    _ => {
                        let mut seen_machines = Vec::with_capacity(machines.len());
                        let mut all_handles = Vec::new();
                        for &machine in machines {
                            if !seen_machines.contains(&machine) {
                                seen_machines.push(machine);
                                self.with_machine_roots(machine, |handles| {
                                    all_handles.extend_from_slice(handles);
                                });
                            }
                        }
                        if seen_machines.len() > 1 {
                            all_handles.sort_unstable_by_key(|handle| handle.arena_index());
                            all_handles.dedup();
                        }
                        all_handles
                    }
                }
            }

            pub fn is_authored_root(
                &self,
                root: &CheckedScalarComputationRoot,
                machine: SymbolHandle,
                authored: ExpressionHandle,
                scalar_type: impl Into<crate::CheckedScalarType>,
            ) -> bool {
                root.machine == machine
                    && self.nodes.is_valid(root.root)
                    && self.nodes.get(root.root).authored_root == authored
                    && self.nodes.get(root.root).scalar_type == scalar_type.into()
            }

            pub fn roots_in_machines(
                &self,
                machines: &[SymbolHandle],
            ) -> Vec<CheckedScalarComputationHandle> {
                self.machine_root_handles(machines)
                    .into_iter()
                    .map(|handle| self.roots.get(handle).root)
                    .collect()
            }

            pub fn machine_roots<'plans>(
                &'plans self,
                machines: &'plans [SymbolHandle],
            ) -> impl Iterator<Item = &'plans CheckedScalarComputationRoot> {
                self.machine_root_handles(machines)
                    .into_iter()
                    .map(move |handle| self.roots.get(handle))
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedProjectionSelectorRoot {
            pub machine: SymbolHandle,
            pub state: SymbolHandle,
            pub statement_ordinal: u32,
            pub expression: ExpressionHandle,
            pub evaluations: HandleSpan<CheckedScalarComputationHandle>,
        }

        impl Default for CheckedProjectionSelectorRoot {
            fn default() -> Self {
                Self {
                    machine: SymbolHandle::invalid(),
                    state: SymbolHandle::invalid(),
                    statement_ordinal: 0,
                    expression: ExpressionHandle::invalid(),
                    evaluations: HandleSpan::default(),
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedScalarComputationRoot {
            pub machine: SymbolHandle,
            pub state: SymbolHandle,
            pub statement_ordinal: u32,
            pub role: CheckedScalarExpressionRole,
            pub root: CheckedScalarComputationHandle,
        }

        impl Default for CheckedScalarComputationRoot {
            fn default() -> Self {
                Self {
                    machine: SymbolHandle::invalid(),
                    state: SymbolHandle::invalid(),
                    statement_ordinal: 0,
                    role: CheckedScalarExpressionRole::Return,
                    root: Handle::invalid(),
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedScalarComputation {
            pub authored_root: ExpressionHandle,
            pub value_source: ExpressionHandle,
            pub scalar_type: crate::CheckedScalarType,
            pub normal_cleanup_actions: HandleSpan<crate::CheckedNormalCleanupAction>,
            pub kind: CheckedScalarComputationKind,
        }

        impl Default for CheckedScalarComputation {
            fn default() -> Self {
                Self {
                    authored_root: Handle::invalid(),
                    value_source: Handle::invalid(),
                    scalar_type: PrimitiveType::Bool.into(),
                    normal_cleanup_actions: HandleSpan::empty(),
                    kind: CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(
                        Box::new(CheckedBooleanExpression::Constant(false)),
                    )),
                }
            }
        }

        impl CheckedScalarComputation {
            pub fn evaluated_expression(&self) -> ExpressionHandle {
                match &self.kind {
                    CheckedScalarComputationKind::MachineValue {
                        source_expression, ..
                    }
                    | CheckedScalarComputationKind::StorageAddress {
                        source_expression, ..
                    }
                    | CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                        source_expression,
                        ..
                    }
                    | CheckedScalarComputationKind::ByteSequencePredicate {
                        source_expression,
                        ..
                    }
                    | CheckedScalarComputationKind::StructuralField {
                        source_expression, ..
                    }
                    | CheckedScalarComputationKind::Qualification {
                        source_expression, ..
                    }
                    | CheckedScalarComputationKind::AuthoredCast {
                        source_expression, ..
                    }
                    | CheckedScalarComputationKind::BooleanToInteger {
                        source_expression, ..
                    }
                    | CheckedScalarComputationKind::Dispatch {
                        source_expression, ..
                    }
                    | CheckedScalarComputationKind::Select {
                        source_expression, ..
                    }
                    | CheckedScalarComputationKind::StorageObservation {
                        source_expression, ..
                    }
                    | CheckedScalarComputationKind::Apply {
                        source_expression, ..
                    }
                    | CheckedScalarComputationKind::WithOperands {
                        source_expression, ..
                    }
                    | CheckedScalarComputationKind::Call {
                        source_expression, ..
                    } => *source_expression,
                    CheckedScalarComputationKind::CaseMembership {
                        completion_expression,
                        ..
                    } => *completion_expression,
                    CheckedScalarComputationKind::RetainedConstant { expression } => *expression,
                    CheckedScalarComputationKind::Value(_) => self.value_source,
                    CheckedScalarComputationKind::SelectedComparison { .. } => {
                        ExpressionHandle::invalid()
                    }
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedScalarComputationKind {
            MachineValue {
                source_expression: ExpressionHandle,
                value: typed_trees::callable_values::MachineValue,
                machine: SymbolHandle,
                state: SymbolHandle,
            },
            StorageAddress {
                operand_type: typed_trees::types::TypeReferenceHandle,
                source_expression: ExpressionHandle,
                subject: CheckedUnitStructuralArgumentPlan,
            },
            ByteSequenceLiteralComparison {
                operand_type: typed_trees::types::TypeReferenceHandle,
                source_expression: ExpressionHandle,
                literal: source::SourceBytes,
                maximum_length: Option<u64>,
                negated: bool,
                subject: CheckedUnitStructuralArgumentPlan,
            },
            ByteSequencePredicate {
                predicate: language_semantics::byte_predicates::ByteSequencePredicate,
                operand_type: typed_trees::types::TypeReferenceHandle,
                source_expression: ExpressionHandle,
                subject: CheckedUnitStructuralArgumentPlan,
            },
            StructuralField {
                atomic_ordering: Option<language_core::atomic::MemoryOrdering>,
                result_type: typed_trees::types::TypeReferenceHandle,
                source_expression: ExpressionHandle,
                receiver_expression: ExpressionHandle,
                source_type: typed_trees::types::TypeReferenceHandle,
                subject: CheckedUnitStructuralArgumentPlan,
                receiver_value: super::structural_values::CheckedStructuralValueHandle,
                field: SymbolHandle,
                field_identity: CheckedMemberIdentity,
            },
            CaseMembership {
                source_expression: ExpressionHandle,
                completion_expression: ExpressionHandle,
                receiver_value: super::structural_values::CheckedStructuralValueHandle,
                subject: CheckedScalarComputationStructuralArgument,
                case: SymbolHandle,
            },
            SelectedComparison {
                operator_use: Handle<crate::CheckedOperatorUseFact>,
                left: CheckedScalarComputationHandle,
                right: CheckedScalarComputationHandle,
            },
            Qualification {
                source_expression: ExpressionHandle,
                operand: CheckedScalarComputationHandle,
                result_type: typed_trees::types::TypeReferenceHandle,
            },
            RetainedConstant {
                expression: ExpressionHandle,
            },
            Value(CheckedScalarExpression),
            AuthoredCast {
                source_expression: ExpressionHandle,
                operand: CheckedScalarComputationHandle,
                result_type: typed_trees::types::TypeReferenceHandle,
                formation_decided: bool,
            },
            BooleanToInteger {
                source_expression: ExpressionHandle,
                operand: CheckedScalarComputationHandle,
            },
            Dispatch {
                source_expression: ExpressionHandle,
                subject: CheckedDispatchSubject,
                arms: HandleSpan<CheckedScalarDispatchArm>,
            },
            Call {
                source_expression: ExpressionHandle,
                source_call: crate::CheckedCallOrigin,
                callee: CheckedScalarCallee,
                call_ordinal: u32,
                arguments: HandleSpan<CheckedScalarComputationHandle>,
                structural_arguments: HandleSpan<CheckedScalarComputationStructuralArgument>,
                temporary_cleanups: HandleSpan<CheckedScalarComputationTemporaryCleanup>,
                claim_transfers: HandleSpan<crate::CheckedUnitClaimTransferPlan>,
            },
            Select {
                source_expression: ExpressionHandle,
                condition: CheckedScalarComputationHandle,
                when_true: CheckedScalarComputationHandle,
                when_false: CheckedScalarComputationHandle,
            },
            StorageObservation {
                root_type: typed_trees::types::TypeReferenceHandle,
                source_expression: ExpressionHandle,
                expression: CheckedScalarExpression,
                operands: HandleSpan<CheckedScalarComputationHandle>,
                collections: HandleSpan<typed_trees::types::TypeReferenceHandle>,
            },
            WithOperands {
                source_expression: ExpressionHandle,
                operands: HandleSpan<CheckedScalarEvaluationOperand>,
                operand: CheckedScalarComputationHandle,
            },
            Apply {
                source_expression: ExpressionHandle,
                expression: CheckedScalarExpression,
                operands: HandleSpan<CheckedScalarComputationHandle>,
            },
        }

        impl CheckedScalarComputationKind {
            pub fn observed_receiver(
                &self,
            ) -> super::structural_values::CheckedStructuralValueHandle {
                match self {
                    Self::StructuralField { receiver_value, .. }
                    | Self::CaseMembership { receiver_value, .. } => *receiver_value,
                    _ => super::structural_values::CheckedStructuralValueHandle::invalid(),
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedScalarCallee {
            Runtime {
                callee: CheckedScalarComputationHandle,
                contract: typed_trees::callable_values::RuntimeCallableContractHandle,
            },
            Body {
                machine: SymbolHandle,
                state: SymbolHandle,
            },
            Boundary {
                machine: SymbolHandle,
                state: SymbolHandle,
                authority_value: super::structural_values::CheckedStructuralValueHandle,
            },
            DescriptorTable {
                machine: SymbolHandle,
                state: SymbolHandle,
                operation: language_core::descriptor_tables::DescriptorTableOperation,
                service: language_semantics::ServiceReachId,
            },
            Dynamic {
                selector: CheckedScalarComputationHandle,
                slot: SymbolHandle,
                requirement: SymbolHandle,
                interface_identity: TypeIdentityId,
            },
        }

        impl CheckedScalarCallee {
            pub fn authority_value(
                &self,
            ) -> super::structural_values::CheckedStructuralValueHandle {
                match self {
                    Self::Boundary {
                        authority_value, ..
                    } => *authority_value,
                    Self::Body { .. }
                    | Self::DescriptorTable { .. }
                    | Self::Dynamic { .. }
                    | Self::Runtime { .. } => Default::default(),
                }
            }

            pub fn static_target(&self) -> Option<(SymbolHandle, SymbolHandle)> {
                match self {
                    Self::Body { machine, state }
                    | Self::Boundary { machine, state, .. }
                    | Self::DescriptorTable { machine, state, .. } => Some((*machine, *state)),
                    Self::Dynamic { .. } | Self::Runtime { .. } => None,
                }
            }

            pub fn called_signature(&self) -> Option<SymbolHandle> {
                match self {
                    Self::Body { state, .. }
                    | Self::Boundary { state, .. }
                    | Self::DescriptorTable { state, .. } => Some(*state),
                    Self::Dynamic { slot, .. } => Some(*slot),
                    Self::Runtime { .. } => None,
                }
            }

            pub fn runtime_contract(
                &self,
            ) -> Option<typed_trees::callable_values::RuntimeCallableContractHandle> {
                match self {
                    Self::Runtime { contract, .. } => Some(*contract),
                    _ => None,
                }
            }

            pub fn matches_flow_call(
                &self,
                call: &crate::FlowCallFact,
                plans: &CheckedScalarComputationPlans,
            ) -> bool {
                match self {
                    Self::Runtime { callee, contract } => {
                        plans.nodes.is_valid(*callee)
                            && call.runtime_call.is_some_and(|binding| {
                                binding.contract == *contract
                                    && binding.callee == plans.nodes.get(*callee).authored_root
                            })
                    }
                    _ => {
                        call.runtime_call.is_none()
                            && self.called_signature() == Some(call.target_symbol)
                    }
                }
            }

            pub fn evaluated_callee(&self) -> Option<CheckedScalarComputationHandle> {
                match self {
                    Self::Dynamic { selector, .. } => Some(*selector),
                    Self::Runtime { callee, .. } => Some(*callee),
                    Self::Body { .. } | Self::Boundary { .. } | Self::DescriptorTable { .. } => {
                        None
                    }
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedScalarDispatchArm {
            pub equality_use: Handle<crate::CheckedOperatorUseFact>,
            pub source_arm: Handle<typed_trees::expression::TableMatchArm>,
            pub pattern: CheckedScalarDispatchPattern,
            pub value: CheckedScalarComputationHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedScalarDispatchPattern {
            Value(CheckedScalarComputationHandle),
            Predicate(CheckedScalarComputationHandle),
            #[default]
            Wildcard,
        }

        impl CheckedScalarDispatchPattern {
            pub fn computation(&self) -> Option<CheckedScalarComputationHandle> {
                match self {
                    Self::Value(value) | Self::Predicate(value) => Some(*value),
                    Self::Wildcard => None,
                }
            }
        }

        #[macro_export]
        macro_rules! cast_or_qualification_triple_pattern {
    ($($fields:tt)*) => {
        $crate::CheckedScalarComputationKind::AuthoredCast { $($fields)* }
            | $crate::CheckedScalarComputationKind::BooleanToInteger { $($fields)* }
            | $crate::CheckedScalarComputationKind::Qualification { $($fields)* }
    };
}
    }
    pub use computations::{
        CheckedDispatchSubject, CheckedProjectionSelectorRoot, CheckedScalarCallee,
        CheckedScalarCaseComputationField, CheckedScalarCaseConstruction, CheckedScalarComputation,
        CheckedScalarComputationHandle, CheckedScalarComputationKind,
        CheckedScalarComputationPlans, CheckedScalarComputationRoot,
        CheckedScalarComputationStructuralArgument, CheckedScalarComputationTemporaryCleanup,
        CheckedScalarDispatchArm, CheckedScalarDispatchPattern, CheckedScalarEvaluationOperand,
    };
    pub(crate) mod proof_terms {
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;

        use super::CheckedScalarExpression;
        use typed_trees::type_identity::TypeIdentityId;

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedErasedProofParameterPlan {
            pub source_position: u32,
            pub proof_position: u32,
            pub parameter_symbol: SymbolHandle,
            pub type_identity: TypeIdentityId,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
        pub enum CheckedProofTermRole {
            ErasedCallArgument {
                binding_ordinal: u32,
                erased_ordinal: u32,
            },
            ErasedUnitCallArgument {
                call_ordinal: u32,
                erased_ordinal: u32,
            },
            TransitionArgument {
                argument_ordinal: u32,
            },
            TransitionContinuationArgument {
                argument_ordinal: u32,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedMathematicalArgument {
            Formal {
                parameter_symbol: SymbolHandle,
            },
            Scalar {
                type_identity: TypeIdentityId,
                expression: CheckedScalarExpression,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedProofTerm {
            Mathematical {
                type_identity: TypeIdentityId,
                term: proof_admission::TermHandle,
                ty: proof_admission::TermHandle,
            },
            MathematicalApplication {
                type_identity: TypeIdentityId,
                term: proof_admission::TermHandle,
                ty: proof_admission::TermHandle,
                arguments: Vec<CheckedMathematicalArgument>,
            },
            Array {
                type_identity: TypeIdentityId,
                elements: Vec<CheckedProofTerm>,
            },
            Construction {
                data_symbol: SymbolHandle,
                type_identity: TypeIdentityId,
                case_symbol: Option<SymbolHandle>,
                fields: Vec<CheckedProofTermField>,
            },
            Formal {
                parameter_symbol: SymbolHandle,
            },
            Projection {
                parameter_symbol: SymbolHandle,
                field_symbols: Vec<SymbolHandle>,
                type_identity: TypeIdentityId,
            },
            Scalar(CheckedScalarExpression),
        }

        impl Default for CheckedProofTerm {
            fn default() -> Self {
                Self::Formal {
                    parameter_symbol: SymbolHandle::invalid(),
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedProofTermField {
            pub field_symbol: SymbolHandle,
            pub term: CheckedProofTerm,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedLocatedProofTerm {
            pub state: SymbolHandle,
            pub statement_ordinal: u32,
            pub role: CheckedProofTermRole,
            pub expression: ExpressionHandle,
            pub term: CheckedProofTerm,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedProofTerms {
            pub terms: Vec<CheckedLocatedProofTerm>,
        }

        impl CheckedProofTerms {
            pub fn term_at(
                &self,
                state: SymbolHandle,
                statement_ordinal: u32,
                role: CheckedProofTermRole,
            ) -> Option<&CheckedProofTerm> {
                let mut matches = self.terms.iter().filter(|term| {
                    term.state == state
                        && term.statement_ordinal == statement_ordinal
                        && term.role == role
                });
                let term = matches.next()?;
                if matches.next().is_some() {
                    return None;
                }
                Some(&term.term)
            }
        }
    }
    pub use proof_terms::{
        CheckedErasedProofParameterPlan, CheckedLocatedProofTerm, CheckedMathematicalArgument,
        CheckedProofTerm, CheckedProofTermField, CheckedProofTermRole, CheckedProofTerms,
    };
    mod completion_expressions {
        //! Result-less completion expressions retain selection and invocation custody.
        //! Unit creates no scalar slot or structural home. Subjects and patterns use
        //! the same evaluated dispatch operands as value-producing selections.

        use arena::{Arena, Handle, HandleSpan};
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, TableMatchArm};

        pub type CheckedCompletionExpressionHandle = Handle<CheckedCompletionExpression>;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedCompletionPlans {
            pub nodes: Arena<CheckedCompletionExpression>,
            pub arms: Arena<CheckedCompletionArm>,
            pub roots: Vec<CheckedCompletionRoot>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedCompletionRoot {
            pub machine: SymbolHandle,
            pub state: SymbolHandle,
            pub statement_ordinal: u32,
            pub expression: ExpressionHandle,
            pub root: CheckedCompletionExpressionHandle,
            pub refusal: Option<CheckedCompletionRefusal>,
        }

        /// Why the exact selected completion could not be produced. This is diagnostic
        /// data, not a partial executable plan or permission to omit a selected call.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedCompletionRefusal {
            pub expression: ExpressionHandle,
            pub reason: CheckedCompletionRefusalReason,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedCompletionRefusalReason {
            Cycle,
            CallResult,
            CallOccurrence,
            DispatchOperands,
            Comparison,
            ExpressionKind,
            NormalCompletion,
        }

        impl CheckedCompletionRefusalReason {
            pub fn phase(self) -> &'static str {
                match self {
                    Self::Cycle => "Unit completion: cyclic expression dependency",
                    Self::CallResult => "Unit completion: selected call has no Unit result",
                    Self::CallOccurrence => {
                        "Unit completion: selected call has no exact flow occurrence"
                    }
                    Self::DispatchOperands => "Unit completion: dispatch operands unavailable",
                    Self::Comparison => "Unit completion: pattern comparison unavailable",
                    Self::ExpressionKind => "Unit completion: result is not a selected invocation",
                    Self::NormalCompletion => {
                        "Unit completion: normal-return observation unavailable"
                    }
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedCompletionExpression {
            pub expression: ExpressionHandle,
            pub normal_continuation: bool,
            pub kind: CheckedCompletionExpressionKind,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedCompletionExpressionKind {
            #[default]
            Missing,
            Invocation {
                source_call: Handle<crate::FlowCallFact>,
            },
            Dispatch {
                subject: super::CheckedDispatchSubject,
                arms: HandleSpan<CheckedCompletionArm>,
            },
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedCompletionArm {
            pub source_arm: Handle<TableMatchArm>,
            pub equality_use: Handle<crate::CheckedOperatorUseFact>,
            pub pattern: super::CheckedScalarDispatchPattern,
            pub completion: CheckedCompletionExpressionHandle,
            pub comparison: CheckedCompletionComparison,
        }

        impl CheckedCompletionPlans {
            pub fn seal(&mut self) {
                self.roots
                    .sort_unstable_by_key(CheckedCompletionRoot::coordinate);
            }

            pub fn root_at(
                &self,
                machine: SymbolHandle,
                state: SymbolHandle,
                statement: u32,
                expression: ExpressionHandle,
            ) -> Option<&CheckedCompletionRoot> {
                let key = (
                    machine.arena_index(),
                    machine.generation(),
                    state.arena_index(),
                    state.generation(),
                    statement,
                    expression.arena_index(),
                    expression.generation(),
                );
                let first = self.roots.partition_point(|root| root.coordinate() < key);
                let root = self
                    .roots
                    .get(first)
                    .filter(|root| root.coordinate() == key)?;
                (!self
                    .roots
                    .get(first + 1)
                    .is_some_and(|next| next.coordinate() == key))
                .then_some(root)
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedCompletionComparison {
            #[default]
            Predicate,
            Boolean,
            Integer,
        }

        impl CheckedCompletionRoot {
            fn coordinate(&self) -> (u32, u32, u32, u32, u32, u32, u32) {
                (
                    self.machine.arena_index(),
                    self.machine.generation(),
                    self.state.arena_index(),
                    self.state.generation(),
                    self.statement_ordinal,
                    self.expression.arena_index(),
                    self.expression.generation(),
                )
            }
        }
    }
    pub use completion_expressions::{
        CheckedCompletionArm, CheckedCompletionComparison, CheckedCompletionExpression,
        CheckedCompletionExpressionHandle, CheckedCompletionExpressionKind, CheckedCompletionPlans,
        CheckedCompletionRefusal, CheckedCompletionRefusalReason, CheckedCompletionRoot,
    };
    pub(crate) mod structural_values {

        use crate::{
            CheckedOperatorUseFact, CheckedScalarComputationHandle, CheckedScalarDispatchPattern,
        };
        use arena::Arena;
        use arena::{Handle, HandleSpan};
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, TableMatchArm};
        use typed_trees::type_identity::TypeIdentityId;
        use typed_trees::types::TypeReferenceHandle;

        use crate::keyed_rows::KeyedRows;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedStructuralValuePlans {
            pub roots: Arena<CheckedStructuralValueRoot>,
            pub nodes: Arena<CheckedStructuralValue>,
            pub dispatch_arms: Arena<CheckedStructuralDispatchArm>,
            pub record_fields: Arena<CheckedStructuralRecordField>,
            pub content_inputs: Arena<crate::CheckedUnitStructuralArgumentPlan>,
            call_schedules: Arena<StructuralCallSchedule>,
            schedules_by_value_slot: Vec<Vec<Handle<StructuralCallSchedule>>>,
            calls: Arena<crate::CheckedStructuralValueCall>,
            roots_by_state: KeyedRows<SymbolHandle, CheckedStructuralValueRoot>,
            roots_by_machine: KeyedRows<SymbolHandle, CheckedStructuralValueRoot>,
            nodes_by_state: KeyedRows<SymbolHandle, CheckedStructuralValue>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        struct StructuralCallSchedule {
            value: CheckedStructuralValueHandle,
            preceding_bindings: u32,
            calls: HandleSpan<crate::CheckedStructuralValueCall>,
        }

        pub type CheckedStructuralValueHandle = Handle<CheckedStructuralValue>;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum CheckedValueCompletion {
            Normal,
            NoNormal,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedStructuralValueRoot {
            pub captured_copy: Handle<crate::FlowCopiedValue>,
            pub machine: SymbolHandle,
            pub state: SymbolHandle,
            pub statement_ordinal: u32,
            pub expression: ExpressionHandle,
            pub type_reference: TypeReferenceHandle,
            pub root: CheckedStructuralValueHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedStructuralValue {
            pub machine: SymbolHandle,
            pub state: SymbolHandle,
            pub statement_index: u32,
            pub expression: ExpressionHandle,
            pub source_span: source::SourceSpan,
            pub type_reference: TypeReferenceHandle,
            pub qualification_decisions: HandleSpan<crate::CheckedValueQualification>,
            pub content_transformation: Handle<crate::facts::ContentBodyTransformation>,
            pub content_inputs: HandleSpan<crate::CheckedUnitStructuralArgumentPlan>,
            pub kind: CheckedStructuralValueKind,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedElementViewRegion {
            WholeCollection,
            WholeRepresentation,
            RepresentationBytes {
                start: CheckedScalarComputationHandle,
                end: u64,
            },
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedQuotientAdaptationLicense {
            Mint,
            Input {
                request: ExpressionHandle,
                position: u32,
            },
            Result {
                request: ExpressionHandle,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedStructuralValueKind {
            NativeCodeLayout {
                selected_state: SymbolHandle,
            },
            QuotientCall {
                request: ExpressionHandle,
                representative_state: SymbolHandle,
                arguments: Vec<CheckedStructuralValueHandle>,
            },
            QuotientAdaptation {
                source: CheckedStructuralValueHandle,
                quotient: TypeReferenceHandle,
                license: CheckedQuotientAdaptationLicense,
            },

            Scalar {
                value: CheckedScalarComputationHandle,
            },
            Reference {
                source: crate::CheckedUnitStructuralArgumentPlan,
                selectors: HandleSpan<CheckedScalarComputationHandle>,
                storage: crate::CheckedReferenceStorage,
                backing_type: TypeReferenceHandle,
                source_contract: crate::CheckedReferenceSourceContract,
                source_value: CheckedStructuralValueHandle,
            },
            Case(crate::CheckedScalarCaseConstruction),
            StructuralCase {
                data_symbol: SymbolHandle,
                case: SymbolHandle,
                fields: HandleSpan<CheckedStructuralRecordField>,
            },
            Call {
                source_call: Handle<crate::FlowCallFact>,
            },
            Record {
                data_symbol: SymbolHandle,
                fields: HandleSpan<CheckedStructuralRecordField>,
            },
            Place(crate::CheckedUnitStructuralArgumentPlan),
            ByteLiteralView {
                bytes: Vec<u8>,
            },
            ViewSubslice {
                source: CheckedStructuralValueHandle,
                start: Option<crate::CheckedCallScalarArgument>,
                end: Option<crate::CheckedCallScalarArgument>,
                end_inclusive: bool,
                site: crate::CheckedSubsliceSite,
            },
            BorrowedSliceView {
                source: crate::CheckedUnitStructuralArgumentPlan,
                region: CheckedElementViewRegion,
            },
            MutableSliceView {
                source: crate::CheckedUnitStructuralArgumentPlan,
                region: CheckedElementViewRegion,
            },
            CopiedStructuralPlace {
                source: crate::CheckedUnitStructuralArgumentPlan,
                leaf: TypeReferenceHandle,
            },
            Projection {
                source: CheckedStructuralValueHandle,
                logical_root_type_identity: TypeIdentityId,
                path: Vec<crate::CheckedUnitStructuralPathSegment>,
                projection_evaluations: arena::HandleSpan<crate::CheckedScalarComputationHandle>,
                type_identity: TypeIdentityId,
                copies_selected_value: bool,
            },
            Dispatch {
                subject: crate::CheckedDispatchSubject,
                arms: HandleSpan<CheckedStructuralDispatchArm>,
            },
            FixedArray {
                elements: Vec<CheckedStructuralValueHandle>,
            },
            ScalarArray {
                elements: Vec<CheckedScalarComputationHandle>,
            },
            ZeroedScalarArray {
                element: CheckedScalarComputationHandle,
                element_count: u64,
            },
            ZeroedStructural {
                projected_qualifications: Vec<crate::CheckedStructuralPathQualification>,
            },
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedStructuralRecordField {
            pub field: SymbolHandle,
            pub expression: ExpressionHandle,
            pub type_reference: TypeReferenceHandle,
            pub value: CheckedStructuralRecordFieldValue,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedStructuralRecordFieldValue {
            Scalar(CheckedScalarComputationHandle),
            Structural(CheckedStructuralValueHandle),
        }

        impl Default for CheckedStructuralRecordFieldValue {
            fn default() -> Self {
                Self::Scalar(Handle::invalid())
            }
        }

        impl Default for CheckedStructuralValueKind {
            fn default() -> Self {
                Self::Case(crate::CheckedScalarCaseConstruction::default())
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedStructuralDispatchArm {
            pub source_arm: Handle<TableMatchArm>,
            pub equality_use: Handle<CheckedOperatorUseFact>,
            pub pattern: CheckedScalarDispatchPattern,
            pub value: CheckedStructuralValueHandle,
        }

        impl CheckedStructuralValuePlans {
            pub fn completion(
                &self,
                root: CheckedStructuralValueHandle,
                calls: &[crate::CheckedStructuralValueCall],
            ) -> Option<CheckedValueCompletion> {
                self.completion_with(root, calls, &|_| None)
            }

            pub fn completion_with(
                &self,
                root: CheckedStructuralValueHandle,
                calls: &[crate::CheckedStructuralValueCall],
                prepared_root: &dyn Fn(
                    CheckedStructuralValueHandle,
                ) -> Option<CheckedValueCompletion>,
            ) -> Option<CheckedValueCompletion> {
                fn visit(
                    plans: &CheckedStructuralValuePlans,
                    value: CheckedStructuralValueHandle,
                    calls: &[crate::CheckedStructuralValueCall],
                    active: &mut Vec<CheckedStructuralValueHandle>,
                    prepared_root: &dyn Fn(
                        CheckedStructuralValueHandle,
                    ) -> Option<CheckedValueCompletion>,
                ) -> Option<CheckedValueCompletion> {
                    use CheckedValueCompletion::{NoNormal, Normal};
                    if !plans.nodes.is_valid(value) {
                        return None;
                    }
                    if let Some(completion) = prepared_root(value) {
                        return Some(completion);
                    }
                    if active.contains(&value) {
                        return Some(Normal);
                    }
                    active.push(value);
                    let mut children = Vec::new();
                    let completion = match &plans.nodes.get(value).kind {
                        CheckedStructuralValueKind::Call { .. } => {
                            let mut matching = calls.iter().filter(|call| call.value == value);
                            let call = matching.next();
                            if matching.next().is_some() {
                                return None;
                            }
                            if call.is_some_and(|call| !call.has_normal_continuation()) {
                                NoNormal
                            } else {
                                Normal
                            }
                        }
                        CheckedStructuralValueKind::Dispatch { subject, arms } => {
                            let subject = subject.structural_value();
                            let subject_stops = if subject.is_valid() {
                                let subject_calls = plans
                                    .call_schedule(subject)
                                    .map_or(calls, |(_, calls)| calls);
                                visit(plans, subject, subject_calls, active, prepared_root)?
                                    == NoNormal
                            } else {
                                false
                            };
                            let arms = plans.dispatch_arms.span(*arms)?;
                            let mut all_stop = !arms.is_empty();
                            for arm in arms {
                                all_stop &= visit(plans, arm.value, calls, active, prepared_root)?
                                    == NoNormal;
                            }
                            if subject_stops || all_stop {
                                NoNormal
                            } else {
                                Normal
                            }
                        }
                        CheckedStructuralValueKind::Record { fields, .. }
                        | CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                            for field in plans.record_fields.span(*fields)? {
                                if let CheckedStructuralRecordFieldValue::Structural(child) =
                                    field.value
                                {
                                    children.push(child);
                                }
                            }
                            Normal
                        }
                        CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                            children.extend(arguments);
                            Normal
                        }
                        CheckedStructuralValueKind::FixedArray { elements } => {
                            children.extend(elements);
                            Normal
                        }
                        CheckedStructuralValueKind::Projection { source, .. }
                        | CheckedStructuralValueKind::ViewSubslice { source, .. }
                        | CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                            children.push(*source);
                            Normal
                        }
                        CheckedStructuralValueKind::Reference { source_value, .. } => {
                            if source_value.is_valid() {
                                children.push(*source_value);
                            }
                            Normal
                        }
                        _ => Normal,
                    };
                    let mut result = completion;
                    for child in children {
                        if visit(plans, child, calls, active, prepared_root)? == NoNormal {
                            result = NoNormal;
                            break;
                        }
                    }
                    active.pop();
                    Some(result)
                }
                visit(self, root, calls, &mut Vec::new(), prepared_root)
            }

            pub fn value_dependencies(
                &self,
                root: CheckedStructuralValueHandle,
            ) -> Option<Vec<CheckedStructuralValueHandle>> {
                let mut pending = vec![root];
                let mut values = Vec::new();
                while let Some(value) = pending.pop() {
                    if !self.nodes.is_valid(value) {
                        return None;
                    }
                    if values.contains(&value) {
                        continue;
                    }
                    values.push(value);
                    match &self.nodes.get(value).kind {
                        CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                            pending.extend(arguments)
                        }
                        CheckedStructuralValueKind::Record { fields, .. }
                        | CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                            for field in self.record_fields.span(*fields)? {
                                if let CheckedStructuralRecordFieldValue::Structural(child) =
                                    field.value
                                {
                                    pending.push(child);
                                }
                            }
                        }
                        CheckedStructuralValueKind::Dispatch { subject, arms } => {
                            let captured = subject.structural_value();
                            if captured.is_valid() {
                                pending.push(captured);
                            }
                            pending.extend(
                                self.dispatch_arms.span(*arms)?.iter().map(|arm| arm.value),
                            );
                        }
                        CheckedStructuralValueKind::Projection { source, .. }
                        | CheckedStructuralValueKind::ViewSubslice { source, .. }
                        | CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                            pending.push(*source)
                        }
                        CheckedStructuralValueKind::FixedArray { elements } => {
                            pending.extend(elements)
                        }
                        CheckedStructuralValueKind::Reference { source_value, .. }
                            if source_value.is_valid() =>
                        {
                            pending.push(*source_value);
                        }
                        _ => {}
                    }
                }
                Some(values)
            }

            pub fn call_schedule(
                &self,
                value: CheckedStructuralValueHandle,
            ) -> Option<(u32, &[crate::CheckedStructuralValueCall])> {
                let mut schedules = self
                    .schedules_by_value_slot
                    .get(value.arena_index() as usize)
                    .into_iter()
                    .flatten()
                    .map(|handle| self.call_schedules.get(*handle))
                    .filter(|row| row.value == value);
                let schedule = schedules.next()?;
                if schedules.next().is_some() || !self.nodes.is_valid(value) {
                    return None;
                }
                Some((
                    schedule.preceding_bindings,
                    self.calls.span_or_empty(schedule.calls),
                ))
            }

            pub fn retain_calls_for_root(
                &mut self,
                value: CheckedStructuralValueHandle,
                preceding_bindings: u32,
                calls: Vec<crate::CheckedStructuralValueCall>,
            ) {
                assert!(self.nodes.is_valid(value));
                let slot = value.arena_index() as usize;
                if self.schedules_by_value_slot.len() <= slot {
                    self.schedules_by_value_slot.resize_with(slot + 1, Vec::new);
                }
                assert!(
                    !self.schedules_by_value_slot[slot].iter().any(|handle| self
                        .call_schedules
                        .get(*handle)
                        .value
                        == value)
                );
                let calls = self.calls.insert_many(calls);
                let schedule = self.call_schedules.insert(StructuralCallSchedule {
                    value,
                    preceding_bindings,
                    calls,
                });
                self.schedules_by_value_slot[slot].push(schedule);
            }

            pub fn scheduled_calls(
                &self,
            ) -> impl Iterator<
                Item = (
                    Handle<crate::CheckedStructuralValueCall>,
                    &crate::CheckedStructuralValueCall,
                ),
            > {
                self.calls.iter()
            }

            pub fn scheduled_call_mut(
                &mut self,
                call: Handle<crate::CheckedStructuralValueCall>,
            ) -> Option<&mut crate::CheckedStructuralValueCall> {
                if !self.calls.is_valid(call) {
                    return None;
                }
                Some(self.calls.get_mut(call))
            }

            pub fn seal(&mut self) {
                self.roots_by_state.seal(&self.roots, |root| root.state);
                self.roots_by_machine.seal(&self.roots, |root| root.machine);
                self.nodes_by_state.seal(&self.nodes, |node| node.state);
            }

            fn with_state_roots<R>(
                &self,
                state: SymbolHandle,
                run: impl FnOnce(&[Handle<CheckedStructuralValueRoot>]) -> R,
            ) -> R {
                self.roots_by_state
                    .with_rows(&self.roots, |root| root.state, &state, run)
            }

            fn with_machine_roots<R>(
                &self,
                machine: SymbolHandle,
                run: impl FnOnce(&[Handle<CheckedStructuralValueRoot>]) -> R,
            ) -> R {
                self.roots_by_machine
                    .with_rows(&self.roots, |root| root.machine, &machine, run)
            }

            fn unique_state_root(
                &self,
                state: SymbolHandle,
                keep: impl Fn(&CheckedStructuralValueRoot) -> bool,
            ) -> Option<&CheckedStructuralValueRoot> {
                self.with_state_roots(state, |handles| {
                    let mut roots = handles
                        .iter()
                        .map(|handle| self.roots.get(*handle))
                        .filter(|root| keep(root));
                    let root = roots.next()?;
                    roots.next().is_none().then_some(root)
                })
            }

            pub fn with_state_nodes<R>(
                &self,
                state: SymbolHandle,
                run: impl FnOnce(&[Handle<CheckedStructuralValue>]) -> R,
            ) -> R {
                self.nodes_by_state
                    .with_rows(&self.nodes, |node| node.state, &state, run)
            }

            pub fn roots_for_machine(
                &self,
                machine: SymbolHandle,
            ) -> Vec<&CheckedStructuralValueRoot> {
                self.with_machine_roots(machine, |handles| {
                    handles
                        .iter()
                        .map(|handle| self.roots.get(*handle))
                        .filter(|root| root.machine == machine)
                        .collect()
                })
            }

            pub fn roots_for_state(&self, state: SymbolHandle) -> Vec<&CheckedStructuralValueRoot> {
                self.with_state_roots(state, |handles| {
                    handles
                        .iter()
                        .map(|handle| self.roots.get(*handle))
                        .filter(|root| root.state == state)
                        .collect()
                })
            }

            pub fn roots_for_statement(
                &self,
                state: SymbolHandle,
                statement_ordinal: u32,
            ) -> Vec<&CheckedStructuralValueRoot> {
                self.with_state_roots(state, |handles| {
                    handles
                        .iter()
                        .map(|handle| self.roots.get(*handle))
                        .filter(|root| {
                            root.state == state && root.statement_ordinal == statement_ordinal
                        })
                        .collect()
                })
            }

            pub fn root_for_expression(
                &self,
                state: SymbolHandle,
                statement_ordinal: u32,
                expression: ExpressionHandle,
            ) -> Option<&CheckedStructuralValueRoot> {
                self.unique_state_root(state, |root| {
                    !root.captured_copy.is_valid()
                        && root.state == state
                        && root.statement_ordinal == statement_ordinal
                        && root.expression == expression
                })
            }

            pub fn completed_expression_dependency(
                &self,
                expressions: &typed_trees::expression::ExpressionTable,
                state: SymbolHandle,
                statement: Option<u32>,
                expression: ExpressionHandle,
            ) -> Option<&CheckedStructuralValueRoot> {
                self.completed_dependency_from(state, statement, expression, |expression| {
                    if !expressions.expression_is_valid(expression) {
                        return crate::CheckedExpressionSource::Invalid;
                    }
                    match expressions.expression(expression) {
                        typed_trees::expression::ExpressionNode::SavedSubject(saved) => {
                            crate::CheckedExpressionSource::Saved {
                                capture: saved.capture,
                                source: expressions
                                    .saved_subject_source(saved)
                                    .unwrap_or_else(ExpressionHandle::invalid),
                            }
                        }
                        _ => crate::CheckedExpressionSource::Value,
                    }
                })
            }

            pub fn completed_checked_expression_dependency(
                &self,
                bindings: &crate::CheckedExecutableBindings,
                state: SymbolHandle,
                statement: Option<u32>,
                expression: ExpressionHandle,
            ) -> Option<&CheckedStructuralValueRoot> {
                self.completed_dependency_from(state, statement, expression, |expression| {
                    bindings.expression(expression)
                })
            }

            fn completed_dependency_from(
                &self,
                state: SymbolHandle,
                statement: Option<u32>,
                mut expression: ExpressionHandle,
                source_kind: impl Fn(ExpressionHandle) -> crate::CheckedExpressionSource,
            ) -> Option<&CheckedStructuralValueRoot> {
                let mut active_captures = Vec::new();
                if matches!(
                    source_kind(expression),
                    crate::CheckedExpressionSource::Invalid
                ) {
                    return None;
                }
                if let crate::CheckedExpressionSource::Saved { capture, source } =
                    source_kind(expression)
                {
                    active_captures.push(capture);
                    expression = source;
                }
                loop {
                    if matches!(
                        source_kind(expression),
                        crate::CheckedExpressionSource::Invalid
                    ) {
                        return None;
                    }
                    if let crate::CheckedExpressionSource::Saved { capture, .. } =
                        source_kind(expression)
                        && active_captures.contains(&capture)
                    {
                        return None;
                    }
                    let root = self
                        .with_state_roots(state, |handles| {
                            let mut matching = handles
                                .iter()
                                .map(|handle| self.roots.get(*handle))
                                .filter(|root| {
                                    !root.captured_copy.is_valid()
                                        && root.state == state
                                        && root.expression == expression
                                        && statement.is_none_or(|statement| {
                                            root.statement_ordinal == statement
                                        })
                                });
                            let root = matching.next();
                            if matching.next().is_some() {
                                Err(())
                            } else {
                                Ok(root)
                            }
                        })
                        .ok()?;
                    if let Some(root) = root {
                        return (self.nodes.is_valid(root.root)
                            && self.nodes.get(root.root).expression == expression)
                            .then_some(root);
                    }
                    let crate::CheckedExpressionSource::Saved { capture, source } =
                        source_kind(expression)
                    else {
                        return None;
                    };
                    if active_captures.contains(&capture) {
                        return None;
                    }
                    active_captures.push(capture);
                    expression = source;
                }
            }

            pub fn root_for_captured_copy(
                &self,
                state: SymbolHandle,
                statement: u32,
                copy: Handle<crate::FlowCopiedValue>,
            ) -> Option<&CheckedStructuralValueRoot> {
                if !copy.is_valid() {
                    return None;
                }
                self.unique_state_root(state, |root| {
                    root.state == state
                        && root.statement_ordinal == statement
                        && root.captured_copy == copy
                })
            }

            pub fn root_for_value(
                &self,
                value: CheckedStructuralValueHandle,
            ) -> Option<&CheckedStructuralValueRoot> {
                if !self.nodes.is_valid(value) {
                    return None;
                }
                let node = self.nodes.get(value);
                self.unique_state_root(node.state, |root| root.root == value)
            }

            /// The exact completed producer and the expression home it establishes.
            /// A saved alias has its own enclosing capture association; it cannot
            /// relabel a different value as this expression's producer.
            pub fn root_for_value_home(
                &self,
                value: CheckedStructuralValueHandle,
                expression: ExpressionHandle,
            ) -> Option<&CheckedStructuralValueRoot> {
                let root = self.root_for_value(value)?;
                let node = self.nodes.get(value);
                (root.expression == expression
                    && node.expression == expression
                    && root.machine == node.machine
                    && root.state == node.state
                    && root.statement_ordinal == node.statement_index
                    && root.type_reference == node.type_reference)
                    .then_some(root)
            }

            pub fn root_at(
                &self,
                state: SymbolHandle,
                statement_ordinal: u32,
            ) -> Option<&CheckedStructuralValueRoot> {
                self.unique_state_root(state, |root| {
                    !root.captured_copy.is_valid()
                        && root.state == state
                        && root.statement_ordinal == statement_ordinal
                })
            }
        }

        #[macro_export]
        macro_rules! record_or_structural_case_pattern {
    ($($fields:tt)*) => {
        $crate::CheckedStructuralValueKind::Record { $($fields)* }
            | $crate::CheckedStructuralValueKind::StructuralCase { $($fields)* }
    };
}
        #[cfg(test)]
        mod index_tests {

            use super::{CheckedStructuralValuePlans, CheckedStructuralValueRoot, SymbolHandle};
            use typed_trees::expression::{
                ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern,
                TableDestructurePattern, TableMatchArm, TableMatchExpression, TableSavedSubject,
            };

            fn saved_subject(
                expressions: &mut ExpressionTable,
                source: ExpressionHandle,
            ) -> ExpressionHandle {
                let capture = expressions.insert_match_subject(source);
                let head = expressions.insert(ExpressionNode::Boolean(true));
                let alias = expressions.insert(ExpressionNode::SavedSubject(TableSavedSubject {
                    component: Default::default(),
                    capture,
                    pattern_head: head,
                }));
                let arms = expressions.insert_match_arms([TableMatchArm {
                    pattern: MatchPattern::Destructure(TableDestructurePattern {
                        head,
                        ..Default::default()
                    }),
                    value: alias,
                    ..Default::default()
                }]);
                expressions.insert(ExpressionNode::Match(TableMatchExpression {
                    subject: source,
                    capture,
                    arms,
                }));
                alias
            }

            fn retain_root(
                plans: &mut CheckedStructuralValuePlans,
                expression: ExpressionHandle,
            ) -> super::CheckedStructuralValueHandle {
                let state = SymbolHandle::from_parts(1, 1);
                let value = plans.nodes.insert(super::CheckedStructuralValue {
                    machine: state,
                    state,
                    statement_index: 7,
                    expression,
                    source_span: Default::default(),
                    type_reference: Default::default(),
                    qualification_decisions: Default::default(),
                    content_transformation: Default::default(),
                    content_inputs: Default::default(),
                    kind: super::CheckedStructuralValueKind::Scalar {
                        value: Default::default(),
                    },
                });
                plans.roots.insert(CheckedStructuralValueRoot {
                    machine: state,
                    state,
                    statement_ordinal: 7,
                    expression,
                    root: value,
                    ..Default::default()
                });
                value
            }

            #[test]
            fn saved_dependency_uses_defining_subject_and_preserves_a_nested_capture() {
                let mut expressions = ExpressionTable::default();
                let source = expressions.insert(ExpressionNode::Boolean(true));
                let first_alias = saved_subject(&mut expressions, source);
                let second_alias = saved_subject(&mut expressions, first_alias);
                let mut plans = CheckedStructuralValuePlans::default();
                let original = retain_root(&mut plans, source);
                let nested = retain_root(&mut plans, first_alias);
                retain_root(&mut plans, second_alias);
                let state = SymbolHandle::from_parts(1, 1);
                assert_eq!(
                    plans
                        .completed_expression_dependency(&expressions, state, Some(7), first_alias)
                        .map(|root| root.root),
                    Some(original)
                );
                assert_eq!(
                    plans
                        .completed_expression_dependency(&expressions, state, Some(7), second_alias)
                        .map(|root| root.root),
                    Some(nested)
                );
                assert!(
                    plans
                        .completed_expression_dependency(&expressions, state, Some(8), first_alias)
                        .is_none()
                );
                assert!(
                    plans
                        .completed_expression_dependency(
                            &expressions,
                            SymbolHandle::from_parts(2, 1),
                            Some(7),
                            first_alias
                        )
                        .is_none()
                );
                retain_root(&mut plans, source);
                assert!(
                    plans
                        .completed_expression_dependency(&expressions, state, Some(7), first_alias)
                        .is_none()
                );
            }

            #[test]
            fn saved_dependency_rejects_a_cycle_even_with_a_retained_source_root() {
                let mut expressions = ExpressionTable::default();
                let source = expressions.insert(ExpressionNode::Boolean(true));
                let alias = saved_subject(&mut expressions, source);
                *expressions.expression_mut(source) = expressions.expression(alias).clone();
                let mut plans = CheckedStructuralValuePlans::default();
                retain_root(&mut plans, source);
                assert!(
                    plans
                        .completed_expression_dependency(
                            &expressions,
                            SymbolHandle::from_parts(1, 1),
                            Some(7),
                            alias
                        )
                        .is_none()
                );
            }
        }
    }
    pub use structural_values::{
        CheckedElementViewRegion, CheckedQuotientAdaptationLicense, CheckedStructuralDispatchArm,
        CheckedStructuralRecordField, CheckedStructuralRecordFieldValue, CheckedStructuralValue,
        CheckedStructuralValueHandle, CheckedStructuralValueKind, CheckedStructuralValuePlans,
        CheckedStructuralValueRoot, CheckedValueCompletion,
    };
    pub(crate) mod array_construction_source {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedArrayConstructionSource {
            Statement,
            CallArgument {
                call_ordinal: u32,
                parameter_position: u32,
            },
        }
    }
    pub use array_construction_source::CheckedArrayConstructionSource;
    pub mod guard_complement {
        use super::{
            CheckedBooleanExpression, CheckedIeeeFloatComparisonKind,
            CheckedStructuralParameterField,
        };

        pub fn exact_complement(
            first: &CheckedBooleanExpression,
            second: &CheckedBooleanExpression,
            declared_cases: impl FnOnce(
                &CheckedStructuralParameterField,
            ) -> Option<Vec<crate::CheckedMemberIdentity>>,
        ) -> bool {
            use CheckedBooleanExpression as Boolean;
            let (first, first_polarity) = polarity(first);
            let (second, second_polarity) = polarity(second);
            if first == second {
                return first_polarity != second_polarity;
            }
            if first_polarity != second_polarity {
                return false;
            }
            match (first, second) {
                (
                    Boolean::ScalarIeeeFloatComparison {
                        kind: first_kind,
                        left: first_left,
                        right: first_right,
                    },
                    Boolean::ScalarIeeeFloatComparison {
                        kind: second_kind,
                        left: second_left,
                        right: second_right,
                    },
                ) => {
                    ieee_equality_pair(*first_kind, *second_kind)
                        && first_left == second_left
                        && first_right == second_right
                }
                (
                    Boolean::IeeeFloatComparison {
                        kind: first_kind,
                        primitive_type: first_type,
                        left: first_left,
                        right: first_right,
                    },
                    Boolean::IeeeFloatComparison {
                        kind: second_kind,
                        primitive_type: second_type,
                        left: second_left,
                        right: second_right,
                    },
                ) => {
                    ieee_equality_pair(*first_kind, *second_kind)
                        && first_type == second_type
                        && first_left == second_left
                        && first_right == second_right
                }
                (
                    Boolean::StructuralCaseMembership {
                        subject: first_subject,
                        case: first_case,
                    },
                    Boolean::StructuralCaseMembership {
                        subject: second_subject,
                        case: second_case,
                    },
                ) => {
                    first_subject == second_subject
                        && first_case != second_case
                        && first_subject
                            .parameter_field()
                            .and_then(|subject| declared_cases(&subject))
                            .is_some_and(|cases| {
                                cases.len() == 2
                                    && cases.contains(first_case)
                                    && cases.contains(second_case)
                            })
                }
                _ => false,
            }
        }

        fn ieee_equality_pair(
            first: CheckedIeeeFloatComparisonKind,
            second: CheckedIeeeFloatComparisonKind,
        ) -> bool {
            use CheckedIeeeFloatComparisonKind::{Equal, NotEqual};
            matches!((first, second), (Equal, NotEqual) | (NotEqual, Equal))
        }

        fn polarity(
            mut expression: &CheckedBooleanExpression,
        ) -> (&CheckedBooleanExpression, bool) {
            use CheckedBooleanExpression as Boolean;
            let mut polarity = true;
            loop {
                match expression {
                    Boolean::Not(operand) => {
                        expression = operand;
                        polarity = !polarity;
                    }
                    Boolean::Equal { left, right } => match (left.as_ref(), right.as_ref()) {
                        (Boolean::Constant(value), operand)
                        | (operand, Boolean::Constant(value)) => {
                            expression = operand;
                            polarity = polarity == *value;
                        }
                        _ => return (expression, polarity),
                    },
                    _ => return (expression, polarity),
                }
            }
        }
    }

    pub type CheckedValueHandle = Handle<CheckedValueFact>;

    pub use facts::IntegerRange as CheckedIntegerRange;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum CheckedValueOrigin {
        MachineDecrease {
            machine_symbol: SymbolHandle,
            ordinal: usize,
        },
        MachineOwnedDataInitializer {
            machine_symbol: SymbolHandle,
            data_symbol: SymbolHandle,
        },
        StateStatement {
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
            role: CheckedValueStatementRole,
        },
        NestedExpression {
            parent: ExpressionHandle,
        },
    }

    impl Default for CheckedValueOrigin {
        fn default() -> Self {
            Self::NestedExpression {
                parent: ExpressionHandle::invalid(),
            }
        }
    }

    impl CheckedValueOrigin {
        pub const fn machine_symbol(self) -> Option<SymbolHandle> {
            match self {
                Self::MachineDecrease { machine_symbol, .. }
                | Self::MachineOwnedDataInitializer { machine_symbol, .. }
                | Self::StateStatement { machine_symbol, .. } => Some(machine_symbol),
                Self::NestedExpression { .. } => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum CheckedValueStatementRole {
        #[default]
        Expression,
        AssignmentTargetSubexpression,
        AssignmentValue,
        CallArgument,
        LocalInitializer,
        TransitionGuard,
        TransitionTargetArgument,
        TransitionTargetValue,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedValueFact {
        pub expression: ExpressionHandle,
        pub origin: CheckedValueOrigin,
        pub type_reference: TypeReferenceHandle,
        pub primitive_type: Option<typed_trees::types::PrimitiveType>,
        pub integer_range: Option<CheckedIntegerRange>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedValueFacts {
        pub values: Arena<CheckedValueFact>,
        scalar_type_refinements: Arena<CheckedScalarTypeRefinement>,
        scalar_type_predicates: Arena<CheckedBooleanExpression>,
        pub scalar_expressions: CheckedScalarExpressionPlans,
        pub scalar_computations: CheckedScalarComputationPlans,
        pub structural_values: CheckedStructuralValuePlans,
        pub completions: CheckedCompletionPlans,
        /// Proof-only erased actuals recorded under `CheckedProofTermRole`
        /// coordinates. They own no runtime operand rows.
        pub proof_terms: CheckedProofTerms,
    }

    impl CheckedValueFacts {
        pub fn with_roots(values: Arena<CheckedValueFact>) -> Self {
            Self {
                values,
                scalar_expressions: CheckedScalarExpressionPlans::default(),
                scalar_computations: CheckedScalarComputationPlans::default(),
                structural_values: CheckedStructuralValuePlans::default(),
                completions: CheckedCompletionPlans::default(),
                proof_terms: CheckedProofTerms::default(),
                scalar_type_refinements: Arena::new(),
                scalar_type_predicates: Arena::new(),
            }
        }

        pub fn retain_scalar_type_refinement(
            &mut self,
            type_reference: TypeReferenceHandle,
            predicates: impl IntoIterator<Item = CheckedBooleanExpression>,
        ) {
            let predicates = self.scalar_type_predicates.insert_many(predicates);
            self.scalar_type_refinements
                .insert(CheckedScalarTypeRefinement {
                    type_reference,
                    predicates,
                });
        }

        pub fn scalar_type_refinement_predicates(
            &self,
            type_reference: TypeReferenceHandle,
        ) -> Option<&[CheckedBooleanExpression]> {
            if !type_reference.is_valid() {
                return None;
            }
            let row = self
                .scalar_type_refinements
                .iter()
                .find_map(|(_, row)| (row.type_reference == type_reference).then_some(row))?;
            self.scalar_type_predicates.span(row.predicates)
        }

        pub fn expression_values(
            &self,
            expression: ExpressionHandle,
        ) -> impl Iterator<Item = (CheckedValueHandle, &CheckedValueFact)> + '_ {
            self.values
                .iter()
                .filter(move |(_, value)| value.expression == expression)
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    struct CheckedScalarTypeRefinement {
        type_reference: TypeReferenceHandle,
        predicates: HandleSpan<CheckedBooleanExpression>,
    }

    #[derive(Clone, Default)]
    pub struct SealedExpressionStateIndex {
        sealed: Option<(usize, symbols::SymbolKeyMap<SymbolHandle, Vec<usize>>)>,
    }

    impl std::fmt::Debug for SealedExpressionStateIndex {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("SealedExpressionStateIndex")
        }
    }

    impl PartialEq for SealedExpressionStateIndex {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    impl Eq for SealedExpressionStateIndex {}

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedScalarExpressionPlans {
        pub expressions: Vec<CheckedLocatedScalarExpression>,
        pub source_bindings: Arena<CheckedScalarExpressionBindings>,
        pub binding_symbols: Arena<SymbolHandle>,
        #[doc(hidden)]
        pub state_index: KeyedRows<SymbolHandle, CheckedScalarExpressionBindings>,
        #[doc(hidden)]
        pub sealed_expression_index: SealedExpressionStateIndex,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedScalarExpressionBindings {
        pub destination: SymbolHandle,
        pub state: SymbolHandle,
        pub statement_ordinal: u32,
        pub role: CheckedScalarExpressionRole,
        pub expression: ExpressionHandle,
        pub symbols: HandleSpan<SymbolHandle>,
    }

    impl Default for CheckedScalarExpressionBindings {
        fn default() -> Self {
            Self {
                destination: SymbolHandle::invalid(),
                state: SymbolHandle::invalid(),
                statement_ordinal: 0,
                role: CheckedScalarExpressionRole::Return,
                expression: ExpressionHandle::invalid(),
                symbols: HandleSpan::empty(),
            }
        }
    }

    impl CheckedScalarExpressionPlans {
        pub fn seal_expressions(&mut self) {
            let mut by_state: symbols::SymbolKeyMap<SymbolHandle, Vec<usize>> =
                symbols::SymbolKeyMap::default();
            for (index, expression) in self.expressions.iter().enumerate() {
                by_state.entry(expression.state).or_default().push(index);
            }
            self.sealed_expression_index.sealed = Some((self.expressions.len(), by_state));
            self.state_index
                .seal(&self.source_bindings, |binding| binding.state);
        }

        fn with_state_index<R>(
            &self,
            run: impl FnOnce(
                &symbols::SymbolKeyMap<SymbolHandle, Vec<Handle<CheckedScalarExpressionBindings>>>,
            ) -> R,
        ) -> R {
            self.state_index
                .with_groups(&self.source_bindings, |binding| binding.state, run)
        }

        pub fn bound_expression_at(
            &self,
            state: SymbolHandle,
            statement_ordinal: u32,
            role: CheckedScalarExpressionRole,
        ) -> Option<(&CheckedScalarExpressionBindings, &CheckedScalarExpression)> {
            self.with_state_index(|bindings_by_state| {
                let mut bindings = bindings_by_state
                    .get(&state)?
                    .iter()
                    .map(|handle| self.source_bindings.get(*handle))
                    .filter(|binding| {
                        binding.state == state
                            && binding.statement_ordinal == statement_ordinal
                            && binding.role == role
                    });
                let binding = bindings.next()?;
                if bindings.next().is_some() {
                    return None;
                }
                if let Some((sealed_len, by_state)) = &self.sealed_expression_index.sealed
                    && *sealed_len == self.expressions.len()
                {
                    let mut expressions = by_state
                        .get(&state)?
                        .iter()
                        .filter_map(|index| self.expressions.get(*index))
                        .filter(|expression| {
                            expression.state == state
                                && expression.statement_ordinal == statement_ordinal
                                && expression.role == role
                        });
                    let expression = &expressions.next()?.expression;
                    if expressions.next().is_some() {
                        return None;
                    }
                    return Some((binding, expression));
                }
                let mut expressions = self.expressions.iter().filter(|expression| {
                    expression.state == state
                        && expression.statement_ordinal == statement_ordinal
                        && expression.role == role
                });
                let expression = &expressions.next()?.expression;
                if expressions.next().is_some() {
                    return None;
                }
                Some((binding, expression))
            })
        }

        pub fn expression_at(
            &self,
            state: SymbolHandle,
            statement_ordinal: u32,
            role: CheckedScalarExpressionRole,
        ) -> Option<&CheckedScalarExpression> {
            if let Some((sealed_len, by_state)) = &self.sealed_expression_index.sealed
                && *sealed_len == self.expressions.len()
            {
                return by_state
                    .get(&state)?
                    .iter()
                    .filter_map(|index| self.expressions.get(*index))
                    .find(|expression| {
                        expression.state == state
                            && expression.statement_ordinal == statement_ordinal
                            && expression.role == role
                    })
                    .map(|expression| &expression.expression);
            }
            self.expressions
                .iter()
                .find(|expression| {
                    expression.state == state
                        && expression.statement_ordinal == statement_ordinal
                        && expression.role == role
                })
                .map(|expression| &expression.expression)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedLocatedScalarExpression {
        pub state: SymbolHandle,
        pub statement_ordinal: u32,
        pub role: CheckedScalarExpressionRole,
        pub expression: CheckedScalarExpression,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedScalarExpressionRole {
        RankObservation {
            component: u32,
        },
        RuntimeCallCallee {
            call_ordinal: u32,
        },
        CallSelector {
            call_ordinal: u32,
            argument_ordinal: u32,
            expression: ExpressionHandle,
        },
        TransitionSelector {
            argument_ordinal: u32,
            expression: ExpressionHandle,
        },
        RepresentationViewOffset {
            expression: ExpressionHandle,
        },
        StructuralValueSelector {
            expression: ExpressionHandle,
            selector_ordinal: u32,
        },
        StructuralValueField {
            expression: ExpressionHandle,
            field_ordinal: u32,
        },
        RecordField {
            expression: ExpressionHandle,
            field_ordinal: u32,
        },
        StructuralValueSubject {
            expression: ExpressionHandle,
        },
        StructuralValuePattern {
            source_arm: Handle<typed_trees::expression::TableMatchArm>,
        },
        ArrayElement {
            source: CheckedArrayConstructionSource,
            element_ordinal: u32,
        },
        StructuralArrayElement {
            expression: ExpressionHandle,
            element_ordinal: u32,
        },
        StructuralScalarValue {
            expression: ExpressionHandle,
        },
        StorageInitializer,
        AtomicOperand {
            operand_ordinal: u32,
        },
        LocalInitializer {
            binding_ordinal: u32,
        },
        CallArgument {
            binding_ordinal: u32,
            argument_ordinal: u32,
        },
        ErasedCallArgument {
            binding_ordinal: u32,
            erased_ordinal: u32,
        },
        BoundaryCallArgument {
            call_ordinal: u32,
            argument_ordinal: u32,
        },
        UnitCallArgument {
            call_ordinal: u32,
            argument_ordinal: u32,
        },
        SelectedOperatorOperand {
            operand_ordinal: u32,
        },
        ErasedUnitCallArgument {
            call_ordinal: u32,
            erased_ordinal: u32,
        },
        DynamicFamilyArgument {
            call_ordinal: u32,
            family_ordinal: u32,
        },
        SubsliceStart {
            site: CheckedSubsliceSite,
        },
        SubsliceEnd {
            site: CheckedSubsliceSite,
        },
        AssignmentValue,
        AssignmentIndex {
            depth: u32,
        },
        Return,
        ContinuationReturn,
        Guard,
        TransitionArgument {
            argument_ordinal: u32,
        },
        TransitionContinuationArgument {
            argument_ordinal: u32,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedSubsliceSite {
        Expression {
            expression: ExpressionHandle,
        },
        CallArgument {
            call_ordinal: u32,
            argument_ordinal: u32,
        },
        TransitionArgument {
            argument_ordinal: u32,
        },
        LocalBinding,
    }

    #[macro_export]
    macro_rules! integer_widen_triple_pattern {
    ($($fields:tt)*) => {
        $crate::CheckedScalarExpression::IntegerBitwiseNot { $($fields)* }
            | $crate::CheckedScalarExpression::IntegerExactCast { $($fields)* }
            | $crate::CheckedScalarExpression::IntegerWiden { $($fields)* }
    };
}

    impl compiler_storage::Record for SealedExpressionStateIndex {
        fn write(
            &self,
            _: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            Ok(())
        }
        fn read<Context>(
            _: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            Ok(Self::default())
        }
    }

    #[cfg(test)]
    mod index_tests {
        use super::{
            CheckedLocatedScalarExpression, CheckedScalarExpression,
            CheckedScalarExpressionBindings, CheckedScalarExpressionPlans,
            CheckedScalarExpressionRole, SymbolHandle,
        };

        #[test]
        fn expression_lookup_observes_same_length_state_mutation() {
            let mut plans = CheckedScalarExpressionPlans::default();
            let original = SymbolHandle::from_parts(1, 1);
            let moved = SymbolHandle::from_parts(2, 1);
            let role = CheckedScalarExpressionRole::Return;
            plans.expressions.push(CheckedLocatedScalarExpression {
                state: original,
                statement_ordinal: 0,
                role,
                expression: CheckedScalarExpression::IntegerLiteral {
                    literal: numerics::literals::IntegerLiteral::from_value(23),
                },
            });
            let binding = plans
                .source_bindings
                .insert(CheckedScalarExpressionBindings {
                    state: original,
                    ..CheckedScalarExpressionBindings::default()
                });
            assert!(plans.expression_at(original, 0, role).is_some());
            assert!(plans.bound_expression_at(original, 0, role).is_some());
            plans.expressions[0].state = moved;
            assert!(plans.expression_at(moved, 0, role).is_some());
            plans.source_bindings.get_mut(binding).state = moved;
            assert!(plans.bound_expression_at(moved, 0, role).is_some());
            assert!(plans.expression_at(original, 0, role).is_none());
        }
    }

    pub(crate) mod qualification_contracts {
        use crate::CheckedStructuralPathQualification;
        use language_semantics::SemanticDomainId;
        use typed_trees::types::TypeReferenceHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedStructuralQualificationContract {
            pub qualifications: Vec<SemanticDomainId>,
            pub projected_qualifications: Vec<CheckedStructuralPathQualification>,
            pub domains: Vec<(SemanticDomainId, TypeReferenceHandle)>,
            pub projection_types: Vec<TypeReferenceHandle>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedReferenceSourceContract {
            SeparateStorage,
            Standing {
                contract: CheckedStructuralQualificationContract,
                source_requirements: Vec<CheckedSourceQualification>,
            },
            Unavailable(String),
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedSourceQualification {
            pub path: Vec<facts::PlaceSegment>,
            pub semantic_domain: SemanticDomainId,
        }
    }
    pub use qualification_contracts::{
        CheckedReferenceSourceContract, CheckedSourceQualification,
        CheckedStructuralQualificationContract,
    };
}

pub mod views {
    use crate::{CheckFacts, ContentProjectionFacts, FlowFacts, QualificationFacts};

    #[derive(Debug, Clone, Copy)]
    pub struct CheckFactsRef<'f> {
        pub semantic: &'f facts::FactPlan,
        pub borrow: &'f crate::BorrowFacts,
        pub contextual_cleanup: &'f crate::ContextualCleanupFacts,
        pub proof: &'f crate::ProofFacts,
        pub operators: &'f crate::CheckedOperatorFacts,
        pub flow: FlowFactsRef<'f>,
        pub termination: &'f crate::TerminationFacts,
        pub qualifications: QualificationFactsRef<'f>,
    }

    #[derive(Debug)]
    pub struct CheckFactsWalk<'f> {
        pub semantic: &'f facts::FactPlan,
        pub borrow: &'f crate::BorrowFacts,
        pub contextual_cleanup: &'f crate::ContextualCleanupFacts,
        pub proof: &'f crate::ProofFacts,
        pub operators: &'f crate::CheckedOperatorFacts,
        pub flow: FlowFactsWalk<'f>,
        pub termination: &'f crate::TerminationFacts,
        pub qualifications: QualificationFactsWalk<'f>,
    }

    impl CheckFacts {
        pub fn view(&self) -> CheckFactsRef<'_> {
            CheckFactsRef {
                semantic: &self.semantic,
                borrow: &self.borrow,
                contextual_cleanup: &self.contextual_cleanup,
                proof: &self.proof,
                operators: &self.operators,
                flow: self.flow.view(),
                termination: &self.termination,
                qualifications: self.qualifications.view(),
            }
        }

        pub fn walk(&mut self) -> CheckFactsWalk<'_> {
            let Self {
                semantic,
                borrow,
                contextual_cleanup,
                proof,
                operators,
                flow,
                termination,
                qualifications,
                ..
            } = self;
            CheckFactsWalk {
                semantic,
                borrow,
                contextual_cleanup,
                proof,
                operators,
                flow: flow.walk(),
                termination,
                qualifications: qualifications.walk(),
            }
        }
    }

    impl CheckFactsWalk<'_> {
        pub fn view(&self) -> CheckFactsRef<'_> {
            CheckFactsRef {
                semantic: self.semantic,
                borrow: self.borrow,
                contextual_cleanup: self.contextual_cleanup,
                proof: self.proof,
                operators: self.operators,
                flow: self.flow.view(),
                termination: self.termination,
                qualifications: self.qualifications.view(),
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct FlowFactsRef<'f> {
        pub contexts: &'f crate::FlowContextFacts,
        pub invalidations: &'f crate::FlowInvalidationFacts,
        pub borrow_lifetimes: &'f crate::FlowBorrowLifetimeFacts,
        pub ownership: &'f crate::FlowOwnershipFacts,
        pub control: &'f crate::FlowControlFacts,
    }

    #[derive(Debug)]
    pub struct FlowFactsWalk<'f> {
        pub contexts: &'f crate::FlowContextFacts,
        pub invalidations: &'f crate::FlowInvalidationFacts,
        pub borrow_lifetimes: &'f crate::FlowBorrowLifetimeFacts,
        pub ownership: &'f mut crate::FlowOwnershipFacts,
        pub control: &'f crate::FlowControlFacts,
    }

    impl FlowFacts {
        pub fn view(&self) -> FlowFactsRef<'_> {
            FlowFactsRef {
                contexts: &self.contexts,
                invalidations: &self.invalidations,
                borrow_lifetimes: &self.borrow_lifetimes,
                ownership: &self.ownership,
                control: &self.control,
            }
        }

        pub fn walk(&mut self) -> FlowFactsWalk<'_> {
            let Self {
                contexts,
                invalidations,
                borrow_lifetimes,
                ownership,
                control,
                ..
            } = self;
            FlowFactsWalk {
                contexts,
                invalidations,
                borrow_lifetimes,
                ownership,
                control,
            }
        }
    }

    impl FlowFactsWalk<'_> {
        pub fn view(&self) -> FlowFactsRef<'_> {
            FlowFactsRef {
                contexts: self.contexts,
                invalidations: self.invalidations,
                borrow_lifetimes: self.borrow_lifetimes,
                ownership: &*self.ownership,
                control: self.control,
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct QualificationFactsRef<'f> {
        pub content: ContentProjectionFactsRef<'f>,
    }

    #[derive(Debug)]
    pub struct QualificationFactsWalk<'f> {
        pub content: ContentProjectionFactsWalk<'f>,
    }

    impl QualificationFacts {
        pub fn view(&self) -> QualificationFactsRef<'_> {
            QualificationFactsRef {
                content: self.content.view(),
            }
        }

        pub fn walk(&mut self) -> QualificationFactsWalk<'_> {
            let Self { content, .. } = self;
            QualificationFactsWalk {
                content: content.walk(),
            }
        }
    }

    impl QualificationFactsWalk<'_> {
        pub fn view(&self) -> QualificationFactsRef<'_> {
            QualificationFactsRef {
                content: self.content.view(),
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct ContentProjectionFactsRef<'f> {
        pub plans: &'f Vec<language_semantics::content::ContentProjectionPlan>,
        pub conservation_plans: &'f Vec<language_semantics::content::ContentConservationPlan>,
        pub body_conservations: &'f arena::Arena<crate::facts::ContentBodyConservationFact>,
        pub body_conservation_plans: &'f Vec<language_semantics::content::ContentConservationPlan>,
        pub body_claim_bindings: &'f arena::Arena<crate::facts::ContentBodyClaimBinding>,
        pub body_available_inputs: &'f arena::Arena<crate::facts::ContentBodyAvailableInput>,
        pub body_transformations: &'f arena::Arena<crate::facts::ContentBodyTransformation>,
        pub body_derivations: &'f arena::Arena<crate::facts::ContentBodyDerivation>,
        pub boundary_requirement_plans:
            &'f Vec<language_semantics::content::ContentConservationPlan>,
        pub boundary_body_requirements:
            &'f arena::Arena<crate::facts::ContentBoundaryBodyRequirement>,
        pub backing_addresses: &'f arena::Arena<crate::facts::ContentBackingAddressTransport>,
        pub claim_transformations: &'f arena::Arena<crate::facts::ContentClaimTransformation>,
    }

    #[derive(Debug)]
    pub struct ContentProjectionFactsWalk<'f> {
        pub plans: &'f Vec<language_semantics::content::ContentProjectionPlan>,
        pub conservation_plans: &'f Vec<language_semantics::content::ContentConservationPlan>,
        pub body_conservations: &'f mut arena::Arena<crate::facts::ContentBodyConservationFact>,
        pub body_conservation_plans:
            &'f mut Vec<language_semantics::content::ContentConservationPlan>,
        pub body_claim_bindings: &'f mut arena::Arena<crate::facts::ContentBodyClaimBinding>,
        pub body_available_inputs: &'f mut arena::Arena<crate::facts::ContentBodyAvailableInput>,
        pub body_transformations: &'f mut arena::Arena<crate::facts::ContentBodyTransformation>,
        pub body_derivations: &'f mut arena::Arena<crate::facts::ContentBodyDerivation>,
        pub boundary_requirement_plans:
            &'f mut Vec<language_semantics::content::ContentConservationPlan>,
        pub boundary_body_requirements:
            &'f mut arena::Arena<crate::facts::ContentBoundaryBodyRequirement>,
        pub backing_addresses: &'f mut arena::Arena<crate::facts::ContentBackingAddressTransport>,
        pub claim_transformations: &'f mut arena::Arena<crate::facts::ContentClaimTransformation>,
    }

    impl ContentProjectionFacts {
        pub fn view(&self) -> ContentProjectionFactsRef<'_> {
            ContentProjectionFactsRef {
                plans: &self.plans,
                conservation_plans: &self.conservation_plans,
                body_conservations: &self.body_conservations,
                body_conservation_plans: &self.body_conservation_plans,
                body_claim_bindings: &self.body_claim_bindings,
                body_available_inputs: &self.body_available_inputs,
                body_transformations: &self.body_transformations,
                body_derivations: &self.body_derivations,
                boundary_requirement_plans: &self.boundary_requirement_plans,
                boundary_body_requirements: &self.boundary_body_requirements,
                backing_addresses: &self.backing_addresses,
                claim_transformations: &self.claim_transformations,
            }
        }

        pub fn walk(&mut self) -> ContentProjectionFactsWalk<'_> {
            let Self {
                plans,
                conservation_plans,
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
                ..
            } = self;
            ContentProjectionFactsWalk {
                plans,
                conservation_plans,
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
            }
        }
    }

    impl ContentProjectionFactsWalk<'_> {
        pub fn view(&self) -> ContentProjectionFactsRef<'_> {
            ContentProjectionFactsRef {
                plans: self.plans,
                conservation_plans: self.conservation_plans,
                body_conservations: &*self.body_conservations,
                body_conservation_plans: &*self.body_conservation_plans,
                body_claim_bindings: &*self.body_claim_bindings,
                body_available_inputs: &*self.body_available_inputs,
                body_transformations: &*self.body_transformations,
                body_derivations: &*self.body_derivations,
                boundary_requirement_plans: &*self.boundary_requirement_plans,
                boundary_body_requirements: &*self.boundary_body_requirements,
                backing_addresses: &*self.backing_addresses,
                claim_transformations: &*self.claim_transformations,
            }
        }
    }
}
