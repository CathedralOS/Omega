/*
Executable control-flow graphs (`TerminalMachine`, `Block`, `Operation`, `OperationKind`,
`Terminator`), atomic and register access events, integer/float operations, call sites, and
termination ranking witnesses in `TerminalModule`.
*/

pub(crate) mod atomic {

    use language_core::atomic::AtomicOrderingPlan;
    pub use language_core::atomic::{FenceOrdering, MemoryOrdering};
    use semantic_vocabulary::ValueId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum AtomicReadModifyWrite {
        FetchAdd,
        FetchSub,
        FetchAnd,
        FetchOr,
        FetchXor,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum AtomicAccessEvent {
        Load {
            ordering: MemoryOrdering,
        },
        Store {
            value: ValueId,
            ordering: MemoryOrdering,
        },
        ReadModifyWrite {
            operation: AtomicReadModifyWrite,
            operand: ValueId,
            ordering: MemoryOrdering,
        },
        Swap {
            value: ValueId,
            ordering: MemoryOrdering,
        },
        CompareExchange {
            expected: ValueId,
            replacement: ValueId,
            success: MemoryOrdering,
            failure: MemoryOrdering,
        },
        CompareExchangeOnce {
            custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
            expected: ValueId,
            replacement: ValueId,
            success: MemoryOrdering,
            failure: MemoryOrdering,
        },
    }

    impl AtomicAccessEvent {
        pub const fn replacement(self) -> Option<ValueId> {
            match self {
                Self::Store { value, .. } | Self::Swap { value, .. } => Some(value),
                Self::CompareExchange { replacement, .. }
                | Self::CompareExchangeOnce { replacement, .. } => Some(replacement),
                _ => None,
            }
        }

        pub const fn single_attempt_custody(
            self,
        ) -> Option<language_core::atomic::AtomicCompareExchangeOnceResultCustody> {
            match self {
                Self::CompareExchangeOnce { custody, .. } => Some(custody),
                _ => None,
            }
        }
        pub const fn ordering_plan(self) -> AtomicOrderingPlan {
            match self {
                Self::Load { ordering } => AtomicOrderingPlan::Load(ordering),
                Self::Store { ordering, .. } => AtomicOrderingPlan::Store(ordering),
                Self::ReadModifyWrite { ordering, .. } => {
                    AtomicOrderingPlan::ReadModifyWrite(ordering)
                }
                Self::Swap { ordering, .. } => AtomicOrderingPlan::Swap(ordering),
                Self::CompareExchange {
                    success, failure, ..
                } => AtomicOrderingPlan::CompareExchange { success, failure },
                Self::CompareExchangeOnce {
                    success, failure, ..
                } => AtomicOrderingPlan::CompareExchangeOnce { success, failure },
            }
        }

        pub const fn ordering_is_legal(self) -> bool {
            match self {
                Self::Load { ordering } => ordering.valid_for_load(),
                Self::Store { ordering, .. } => ordering.valid_for_store(),
                Self::ReadModifyWrite { .. } | Self::Swap { .. } => true,
                Self::CompareExchange {
                    success, failure, ..
                }
                | Self::CompareExchangeOnce {
                    success, failure, ..
                } => failure.valid_compare_exchange_failure(success),
            }
        }

        pub const fn observes_resident(self) -> bool {
            !matches!(self, Self::Store { .. })
        }

        pub const fn modifies_resident(self) -> bool {
            !matches!(self, Self::Load { .. })
        }

        pub fn operands(self) -> Vec<ValueId> {
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

        pub fn map_operands(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
            match self {
                Self::Load { .. } => {}
                Self::Store { value, .. } | Self::Swap { value, .. } => *value = map(*value),
                Self::ReadModifyWrite { operand, .. } => *operand = map(*operand),
                Self::CompareExchange {
                    expected,
                    replacement,
                    ..
                }
                | Self::CompareExchangeOnce {
                    expected,
                    replacement,
                    ..
                } => {
                    *expected = map(*expected);
                    *replacement = map(*replacement);
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{AtomicAccessEvent, AtomicReadModifyWrite, MemoryOrdering, ValueId};

        fn value(index: u64) -> ValueId {
            ValueId::new(index).expect("nonzero value identity")
        }

        #[test]
        fn legality_replays_the_source_ordering_matrix() {
            assert!(
                AtomicAccessEvent::Load {
                    ordering: MemoryOrdering::Receive
                }
                .ordering_is_legal()
            );
            assert!(
                !AtomicAccessEvent::Load {
                    ordering: MemoryOrdering::Publish
                }
                .ordering_is_legal()
            );
            assert!(
                !AtomicAccessEvent::Store {
                    value: value(1),
                    ordering: MemoryOrdering::ReceivePublish,
                }
                .ordering_is_legal()
            );
            assert!(
                AtomicAccessEvent::CompareExchange {
                    expected: value(1),
                    replacement: value(2),
                    success: MemoryOrdering::ReceivePublish,
                    failure: MemoryOrdering::Receive,
                }
                .ordering_is_legal()
            );
            assert!(
                !AtomicAccessEvent::CompareExchange {
                    expected: value(1),
                    replacement: value(2),
                    success: MemoryOrdering::Receive,
                    failure: MemoryOrdering::GlobalOrder,
                }
                .ordering_is_legal()
            );
        }

        #[test]
        fn operands_and_result_shape_follow_the_event() {
            let fetch = AtomicAccessEvent::ReadModifyWrite {
                operation: AtomicReadModifyWrite::FetchXor,
                operand: value(3),
                ordering: MemoryOrdering::NoOrdering,
            };
            assert_eq!(fetch.operands(), vec![value(3)]);
            assert!(fetch.observes_resident() && fetch.modifies_resident());
            let store = AtomicAccessEvent::Store {
                value: value(4),
                ordering: MemoryOrdering::GlobalOrder,
            };
            assert!(!store.observes_resident() && store.modifies_resident());
            let mut exchange = AtomicAccessEvent::CompareExchange {
                expected: value(5),
                replacement: value(6),
                success: MemoryOrdering::GlobalOrder,
                failure: MemoryOrdering::GlobalOrder,
            };
            exchange.map_operands(&mut |operand| value(operand.get() + 10));
            assert_eq!(exchange.operands(), vec![value(15), value(16)]);
        }
    }
}
pub(crate) mod float_result_policy {

    use semantic_vocabulary::{IeeeFloatFormat, ScalarType, ValueId};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum FloatResultPolicy {
        Trapping {
            raw_result: ValueId,
        },
        Saturating {
            raw_result: ValueId,
            eligible: ValueId,
        },
    }

    impl FloatResultPolicy {
        pub const fn raw_result(self) -> ValueId {
            match self {
                Self::Trapping { raw_result } | Self::Saturating { raw_result, .. } => raw_result,
            }
        }
        pub const fn can_trap(self) -> bool {
            matches!(self, Self::Trapping { .. })
        }
        pub fn operands(self) -> Vec<ValueId> {
            match self {
                Self::Trapping { raw_result } => vec![raw_result],
                Self::Saturating {
                    raw_result,
                    eligible,
                } => vec![eligible, raw_result],
            }
        }
        pub fn accepts_operand_types(
            self,
            format: IeeeFloatFormat,
            mut type_of: impl FnMut(ValueId) -> Option<ScalarType>,
        ) -> bool {
            type_of(self.raw_result()) == Some(ScalarType::IeeeFloat(format))
                && match self {
                    Self::Trapping { .. } => true,
                    Self::Saturating { eligible, .. } => {
                        type_of(eligible) == Some(ScalarType::Boolean)
                    }
                }
        }
        pub fn map_operands(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
            match self {
                Self::Trapping { raw_result } => *raw_result = map(*raw_result),
                Self::Saturating {
                    raw_result,
                    eligible,
                } => {
                    *eligible = map(*eligible);
                    *raw_result = map(*raw_result);
                }
            }
        }
    }
}
pub(crate) mod invocations {

    use super::{OperationKind, Terminator};
    use crate::{ClaimTransfer, CrashRouteBucket, StructuralArgument};
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, ExternalCompletionKind, InvocationSite, MachineId,
        ObligationId, ProofTerm, ScalarTerm, ValueId,
    };

    #[derive(Clone, Copy)]
    pub struct BoundaryInvocation<'a> {
        pub block: BlockId,
        pub site: InvocationSite,
        pub boundary: BoundaryMachineId,
        pub arguments: &'a [ValueId],
        pub structural_arguments: &'a [StructuralArgument],
        pub requirement_obligations: &'a [ObligationId],
        pub completion: BoundaryInvocationCompletion<'a>,
    }

    #[derive(Clone, Copy)]
    pub enum BoundaryInvocationCompletion<'a> {
        Normal {
            result: &'a super::OperationResult,
            receipts: &'a [crate::CompletionReceipt],
        },
        External(ExternalCompletionKind),
    }

    impl super::Block {
        pub fn boundary_invocations(&self) -> impl Iterator<Item = BoundaryInvocation<'_>> {
            self.operations
                .iter()
                .filter_map(|operation| {
                    let OperationKind::BoundaryCall {
                        boundary,
                        arguments,
                        structural_arguments,
                        completion_receipts,
                        requirement_obligations,
                    } = &operation.kind
                    else {
                        return None;
                    };
                    Some(BoundaryInvocation {
                        block: self.id,
                        site: InvocationSite::Operation(operation.id),
                        boundary: *boundary,
                        arguments,
                        structural_arguments,
                        requirement_obligations,
                        completion: BoundaryInvocationCompletion::Normal {
                            result: &operation.result,
                            receipts: completion_receipts,
                        },
                    })
                })
                .chain(match &self.terminator {
                    Terminator::ExternalCompletion {
                        edge,
                        boundary,
                        kind,
                        arguments,
                        structural_arguments,
                        requirement_obligations,
                        ..
                    } => Some(BoundaryInvocation {
                        block: self.id,
                        site: InvocationSite::ExternalCompletion(*edge),
                        boundary: *boundary,
                        arguments,
                        structural_arguments,
                        requirement_obligations,
                        completion: BoundaryInvocationCompletion::External(*kind),
                    }),
                    _ => None,
                })
        }
    }

    impl super::TerminalMachine {
        pub fn boundary_invocations(&self) -> impl Iterator<Item = BoundaryInvocation<'_>> {
            self.blocks
                .iter()
                .flat_map(super::Block::boundary_invocations)
        }
    }

    #[derive(Clone, Copy)]
    pub struct ModuleInvocationInputs<'a, Callable = MachineId> {
        pub callee: Callable,
        pub arguments: &'a [ValueId],
        pub erased_arguments: &'a [ScalarTerm],
        pub erased_proof_arguments: &'a [ProofTerm],
        pub structural_arguments: &'a [StructuralArgument],
        pub claim_transfers: &'a [ClaimTransfer],
        pub completion_receipts: &'a [crate::CompletionReceipt],
        pub requirement_obligations: &'a [ObligationId],
        pub crash_continuations: &'a [CrashRouteBucket],
    }

    impl<'a> ModuleInvocationInputs<'a> {
        pub fn declared(self) -> ModuleInvocationInputs<'a, super::TargetMachineCallable> {
            ModuleInvocationInputs {
                callee: super::TargetMachineCallable::Machine(self.callee),
                arguments: self.arguments,
                erased_arguments: self.erased_arguments,
                erased_proof_arguments: self.erased_proof_arguments,
                structural_arguments: self.structural_arguments,
                claim_transfers: self.claim_transfers,
                completion_receipts: self.completion_receipts,
                requirement_obligations: self.requirement_obligations,
                crash_continuations: self.crash_continuations,
            }
        }
    }

    impl OperationKind {
        /// The two declaration namespaces share call operands and obligation
        /// custody. Machine-only consumers keep `module_invocation`; callers
        /// judging interface contracts use the exact declared target here.
        pub fn declared_invocation(
            &self,
        ) -> Option<ModuleInvocationInputs<'_, super::TargetMachineCallable>> {
            if let Self::BoundaryCall {
                boundary,
                arguments,
                structural_arguments,
                completion_receipts,
                requirement_obligations,
            } = self
            {
                return Some(ModuleInvocationInputs {
                    callee: super::TargetMachineCallable::Boundary(*boundary),
                    arguments,
                    erased_arguments: &[],
                    erased_proof_arguments: &[],
                    structural_arguments,
                    claim_transfers: &[],
                    completion_receipts,
                    requirement_obligations,
                    crash_continuations: &[],
                });
            }
            self.module_invocation()
                .map(ModuleInvocationInputs::declared)
        }

        pub fn module_invocation(&self) -> Option<ModuleInvocationInputs<'_>> {
            let (
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            ) = match self {
                Self::Call {
                    callee,
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    requirement_obligations,
                    crash_continuations,
                } => (
                    *callee,
                    arguments.as_slice(),
                    erased_arguments.as_slice(),
                    erased_proof_arguments.as_slice(),
                    &[][..],
                    &[][..],
                    requirement_obligations.as_slice(),
                    crash_continuations.as_slice(),
                ),
                Self::CallUnit {
                    callee,
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                }
                | Self::CallStructuralScalar {
                    callee,
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                }
                | Self::CallStructuralWithScalarArguments {
                    callee,
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    ..
                } => (
                    *callee,
                    arguments.as_slice(),
                    erased_arguments.as_slice(),
                    erased_proof_arguments.as_slice(),
                    structural_arguments.as_slice(),
                    claim_transfers.as_slice(),
                    requirement_obligations.as_slice(),
                    crash_continuations.as_slice(),
                ),
                Self::CallStructural {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    ..
                } => (
                    *callee,
                    &[][..],
                    &[][..],
                    &[][..],
                    structural_arguments.as_slice(),
                    claim_transfers.as_slice(),
                    requirement_obligations.as_slice(),
                    crash_continuations.as_slice(),
                ),
                _ => return None,
            };
            Some(ModuleInvocationInputs {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                completion_receipts: &[],
                requirement_obligations,
                crash_continuations,
            })
        }
    }

    impl Terminator {
        pub fn module_invocation(&self) -> Option<ModuleInvocationInputs<'_>> {
            let Self::InvokeNoNormal {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            } = self
            else {
                return None;
            };
            Some(ModuleInvocationInputs {
                callee: *callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                completion_receipts: &[],
                requirement_obligations,
                crash_continuations,
            })
        }
    }

    #[derive(Clone, Copy)]
    pub struct ModuleInvocation<'a> {
        pub site: semantic_vocabulary::InvocationSite,
        pub static_reach_binding: Option<u32>,
        pub inputs: ModuleInvocationInputs<'a>,
    }

    impl super::Block {
        pub fn module_invocations(&self) -> impl Iterator<Item = ModuleInvocation<'_>> {
            self.operations
                .iter()
                .filter_map(|operation| {
                    operation
                        .kind
                        .module_invocation()
                        .map(|inputs| ModuleInvocation {
                            site: semantic_vocabulary::InvocationSite::Operation(operation.id),
                            static_reach_binding: operation.static_reach_binding,
                            inputs,
                        })
                })
                .chain(
                    self.terminator
                        .module_invocation()
                        .map(|inputs| ModuleInvocation {
                            site: semantic_vocabulary::InvocationSite::InvokeNoNormal(
                                self.terminator.edge(),
                            ),
                            static_reach_binding: match self.terminator {
                                Terminator::InvokeNoNormal {
                                    static_reach_binding,
                                    ..
                                } => static_reach_binding,
                                _ => None,
                            },
                            inputs,
                        }),
                )
        }
    }

    impl super::TerminalMachine {
        pub fn module_invocations(&self) -> impl Iterator<Item = ModuleInvocation<'_>> {
            self.blocks
                .iter()
                .flat_map(super::Block::module_invocations)
        }
    }

    #[cfg(test)]
    mod boundary_tests {
        use super::*;
        use crate::{Block, Operation, OperationResult};
        use semantic_vocabulary::{EdgeId, OperationId};

        #[test]
        fn repeated_boundary_keeps_distinct_operands_and_completions() {
            let boundary = BoundaryMachineId::new(1).unwrap();
            let normal_value = ValueId::new(1).unwrap();
            let external_value = ValueId::new(2).unwrap();
            let operation = OperationId::new(1).unwrap();
            let edge = EdgeId::new(1).unwrap();
            let mut block = Block::plain(
                BlockId::new(1).unwrap(),
                Vec::new(),
                Terminator::ExternalCompletion {
                    edge,
                    boundary,
                    kind: ExternalCompletionKind::ProcessExit,
                    arguments: vec![external_value],
                    structural_arguments: Vec::new(),
                    requirement_obligations: Vec::new(),
                    frontier_lower_bound: Vec::new(),
                },
            );
            block.operations.push(Operation::unsuspended(
                operation,
                OperationResult::Unit,
                OperationKind::BoundaryCall {
                    boundary,
                    arguments: vec![normal_value],
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                    requirement_obligations: Vec::new(),
                },
            ));
            let occurrences: Vec<_> = block.boundary_invocations().collect();
            assert_eq!(occurrences.len(), 2);
            assert_eq!(occurrences[0].boundary, occurrences[1].boundary);
            assert_eq!(occurrences[0].site, InvocationSite::Operation(operation));
            assert_eq!(occurrences[0].arguments, [normal_value]);
            assert!(matches!(
                occurrences[0].completion,
                BoundaryInvocationCompletion::Normal {
                    result: OperationResult::Unit,
                    receipts: [],
                }
            ));
            assert_eq!(
                occurrences[1].site,
                InvocationSite::ExternalCompletion(edge)
            );
            assert_eq!(occurrences[1].arguments, [external_value]);
            assert!(matches!(
                occurrences[1].completion,
                BoundaryInvocationCompletion::External(ExternalCompletionKind::ProcessExit)
            ));
        }
    }
}
pub(crate) mod machines {
    use crate::{
        ContentEntryClaim, ContentIdentityReshuffle, ContentPartitionComposition, EntryClaim,
        ErasedProofFormal, MachineContract, Operation, StructuralParameterDeclaration,
        StructuralPlaceDeclaration, TerminalMachineResult, TerminalRankedScc, Terminator,
        ValueDeclaration,
    };
    use semantic_vocabulary::{
        BlockId, MachineId, OperationId, PlaceId, ServiceId, StructuralTypeId,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalMachine {
        pub assembly_target_requirement: crate::RegisterTargetApplicability,
        pub id: MachineId,
        pub declaration: Option<crate::CheckedMachineDeclaration>,
        pub attachment: Option<StructuralTypeId>,
        pub nominal_cleanup_subject: Option<StructuralTypeId>,
        pub parameters: Vec<ValueDeclaration>,
        pub parameter_order: Vec<crate::BoundaryParameterKind>,
        pub structural_parameters: Vec<StructuralParameterDeclaration>,
        pub ranked_scc: Option<TerminalRankedScc>,
        pub result: TerminalMachineResult,
        pub structural_places: Vec<StructuralPlaceDeclaration>,
        pub entry_claims: Vec<EntryClaim>,
        pub block_entry_claims: Vec<EntryClaim>,
        pub loan_completions: Vec<crate::LoanCompletion>,
        pub declared_service_reach: Vec<ServiceId>,
        pub closed_reach_application: Option<crate::ClosedReachApplication>,
        pub published_service_ceiling: Vec<ServiceId>,
        pub content_entry_claims: Vec<ContentEntryClaim>,
        pub content_identity_reshuffles: Vec<ContentIdentityReshuffle>,
        pub content_partition_compositions: Vec<ContentPartitionComposition>,
        pub entry: BlockId,
        pub blocks: Vec<Block>,
        pub contract: MachineContract,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Block {
        pub id: BlockId,
        pub parameters: Vec<ValueDeclaration>,
        pub erased_scalar_formals: Vec<ValueDeclaration>,
        pub erased_proof_formals: Vec<ErasedProofFormal>,
        pub structural_parameters: Vec<StructuralParameterDeclaration>,
        pub operations: Vec<Operation>,
        pub terminator: Terminator,
    }

    impl Block {
        pub fn plain(
            id: BlockId,
            parameters: Vec<ValueDeclaration>,
            terminator: Terminator,
        ) -> Self {
            Self {
                id,
                parameters,
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                operations: Vec::new(),
                terminator,
            }
        }
    }

    impl TerminalMachine {
        pub fn entry_proposition_context(
            &self,
        ) -> Result<semantic_vocabulary::PropositionContext, semantic_vocabulary::PropositionError>
        {
            semantic_vocabulary::PropositionContext::from_value_types_and_places(
                self.parameters
                    .iter()
                    .chain(&self.contract.erased_scalar_formals)
                    .map(|value| (value.id, value.scalar_type)),
                self.structural_places
                    .iter()
                    .filter(|place| {
                        matches!(
                            place.kind,
                            semantic_vocabulary::StructuralPlaceKind::Parameter { .. }
                        )
                    })
                    .map(|place| (place.id, place.kind)),
            )
        }
        pub fn structural_place_type(&self, place: PlaceId) -> Option<StructuralTypeId> {
            self.structural_parameter(place)
                .map(|parameter| parameter.structural_type)
                .or_else(|| {
                    self.structural_places.iter().find_map(|row| {
                        if row.id != place {
                            return None;
                        }
                        match row.kind {
                            semantic_vocabulary::StructuralPlaceKind::Result => self
                                .result
                                .structural()
                                .filter(|result| result.place == place)
                                .map(|result| result.structural_type),
                            _ => row.kind.produced_structural_type(),
                        }
                    })
                })
        }

        pub fn validate_parameter_order(&self) -> Result<(), &'static str> {
            let scalar_count = self
                .parameter_order
                .iter()
                .filter(|kind| **kind == crate::BoundaryParameterKind::Scalar)
                .count();
            let structural_count = self.parameter_order.len() - scalar_count;
            if scalar_count != self.parameters.len()
                || structural_count != self.structural_parameters.len()
            {
                return Err("machine runtime parameter order does not cover its signature");
            }
            Ok(())
        }

        pub fn runtime_parameter_formals(&self) -> Result<Vec<u32>, &'static str> {
            self.validate_parameter_order()?;
            let positions =
                |kind| {
                    self.parameter_order.iter().enumerate().filter_map(
                        move |(position, candidate)| (*candidate == kind).then_some(position),
                    )
                };
            positions(crate::BoundaryParameterKind::Scalar)
                .chain(positions(crate::BoundaryParameterKind::Structural))
                .map(|position| {
                    u32::try_from(position)
                        .map_err(|_| "machine runtime formal ordinal cannot be represented")
                })
                .collect()
        }

        pub fn block(&self, id: &BlockId) -> Option<&Block> {
            self.blocks.iter().find(|block| block.id == *id)
        }

        pub fn operations(&self) -> impl Iterator<Item = &Operation> + '_ {
            self.blocks.iter().flat_map(|block| &block.operations)
        }

        pub fn block_parameters(&self) -> impl Iterator<Item = &ValueDeclaration> + '_ {
            self.blocks.iter().flat_map(|block| &block.parameters)
        }

        pub fn block_structural_parameters(
            &self,
        ) -> impl Iterator<Item = &StructuralParameterDeclaration> + '_ {
            self.blocks
                .iter()
                .flat_map(|block| &block.structural_parameters)
        }

        pub fn operation(&self, id: OperationId) -> Option<&Operation> {
            self.operations().find(|operation| operation.id == id)
        }

        pub fn structural_place(&self, place: PlaceId) -> Option<&StructuralPlaceDeclaration> {
            self.structural_places
                .iter()
                .find(|entry| entry.id == place)
        }

        pub fn producing_operation(
            &self,
            place: PlaceId,
        ) -> Option<(&Operation, StructuralTypeId)> {
            let declaration = self.structural_place(place)?;
            let (producer, structural_type) = declaration.kind.as_operation_result()?;
            let operation = self
                .operations()
                .find(|operation| operation.id == producer)?;
            Some((operation, structural_type))
        }

        pub fn structural_parameter(
            &self,
            place: PlaceId,
        ) -> Option<&StructuralParameterDeclaration> {
            self.structural_parameters
                .iter()
                .chain(self.block_structural_parameters())
                .find(|parameter| parameter.place == place)
        }
        pub fn parameter(&self, place: PlaceId) -> Option<&StructuralParameterDeclaration> {
            self.structural_parameter(place)
        }
        pub fn signature_structural_parameter(
            &self,
            place: PlaceId,
        ) -> Option<&StructuralParameterDeclaration> {
            self.structural_parameters
                .iter()
                .find(|parameter| parameter.place == place)
        }
    }

    pub fn structural_parameter_position(
        parameters: &[StructuralParameterDeclaration],
        place: PlaceId,
    ) -> Option<usize> {
        parameters
            .iter()
            .position(|parameter| parameter.place == place)
    }
}
pub use invocations::{
    BoundaryInvocation, BoundaryInvocationCompletion, ModuleInvocation, ModuleInvocationInputs,
};
pub(crate) mod operations {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralStoreDisposition {
        Vacant,
        ReplaceDiscardable,
    }

    impl StructuralStoreDisposition {
        pub fn required_access(self) -> crate::StructuralAccess {
            match self {
                Self::Vacant => crate::StructuralAccess::MutableBorrow,
                Self::ReplaceDiscardable => crate::StructuralAccess::WriteOnlyBorrow,
            }
        }
    }

    use crate::{
        ClaimTransfer, CompletionReceipt, CrashRouteBucket, OutcomeSpecificCallEvidence,
        StructuralArgument, StructuralMultiplicity, StructuralPathQualification,
        StructuralPathSegment, ValueDeclaration,
    };
    use semantic_vocabulary::{
        BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, IeeeFloatValue, IntegerValue,
        MachineId, ObligationId, OperationId, PlaceId, ProofTerm, ScalarTerm, ScalarType,
        ServiceId, StructuralCaseId, StructuralDomainId, StructuralFieldId, StructuralTypeId,
        SuspensionCrossingId, ValueId,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ByteSequenceOperand {
        Whole {
            source: StructuralArgument,
            length: ValueId,
        },
        Field {
            source: StructuralArgument,
            field: StructuralFieldId,
            length: ValueId,
        },
    }

    impl ByteSequenceOperand {
        pub const fn length(&self) -> ValueId {
            match self {
                Self::Whole { length, .. } | Self::Field { length, .. } => *length,
            }
        }

        pub const fn source(&self) -> PlaceId {
            match self {
                Self::Whole { source, .. } | Self::Field { source, .. } => source.place,
            }
        }

        pub const fn argument(&self) -> &StructuralArgument {
            match self {
                Self::Whole { source, .. } | Self::Field { source, .. } => source,
            }
        }

        pub fn argument_mut(&mut self) -> &mut StructuralArgument {
            match self {
                Self::Whole { source, .. } | Self::Field { source, .. } => source,
            }
        }

        pub fn remap_length(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
            match self {
                Self::Whole { length, .. } | Self::Field { length, .. } => *length = map(*length),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Operation {
        pub static_reach_binding: Option<u32>,
        pub suspension_crossing: Option<SuspensionCrossingId>,
        pub id: OperationId,
        pub result: OperationResult,
        pub kind: OperationKind,
    }

    impl Operation {
        pub const fn unsuspended(
            id: OperationId,
            result: OperationResult,
            kind: OperationKind,
        ) -> Self {
            Self {
                static_reach_binding: None,
                suspension_crossing: None,
                id,
                result,
                kind,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum OperationResult {
        Unit,
        Scalar(ValueDeclaration),
        Structural(StructuralOperationResult),
    }

    impl OperationResult {
        pub fn as_structural(&self) -> Option<&StructuralOperationResult> {
            let OperationResult::Structural(structural) = self else {
                return None;
            };
            Some(structural)
        }

        pub fn as_scalar(&self) -> Option<ValueDeclaration> {
            let OperationResult::Scalar(scalar) = self else {
                return None;
            };
            Some(*scalar)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StructuralOperationResult {
        pub place: PlaceId,
        pub structural_type: StructuralTypeId,
        pub multiplicity: StructuralMultiplicity,
        pub qualifications: Vec<StructuralDomainId>,
        pub projected_qualifications: Vec<StructuralPathQualification>,
        pub additional_input_qualifications: Vec<StructuralPathQualification>,
        pub qualification_establishments: Vec<crate::ResultQualificationEstablishment>,
        pub claims: Vec<StructuralResultClaimBinding>,
    }

    impl StructuralOperationResult {
        pub fn is_unrestricted_plain(&self) -> bool {
            self.multiplicity == StructuralMultiplicity::Unrestricted
                && self.qualifications.is_empty()
                && self.projected_qualifications.is_empty()
                && self.claims.is_empty()
        }

        pub fn is_affine_plain(&self) -> bool {
            self.multiplicity == StructuralMultiplicity::Affine
                && self.qualifications.is_empty()
                && self.projected_qualifications.is_empty()
                && self.claims.is_empty()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StructuralResultClaimBinding {
        pub claim: ClaimId,
        pub path: Vec<StructuralPathSegment>,
    }

    impl OperationResult {
        pub const fn scalar(&self) -> Option<ValueDeclaration> {
            match self {
                Self::Unit | Self::Structural(_) => None,
                Self::Scalar(value) => Some(*value),
            }
        }

        pub const fn scalar_ref(&self) -> Option<&ValueDeclaration> {
            match self {
                Self::Unit | Self::Structural(_) => None,
                Self::Scalar(value) => Some(value),
            }
        }

        optimization::structural_variant_accessor!(StructuralOperationResult; Self::Unit | Self::Scalar(_));

        pub const fn expect_scalar(&self) -> ValueDeclaration {
            match self {
                Self::Scalar(value) => *value,
                Self::Unit | Self::Structural(_) => panic!("operation has no scalar result"),
            }
        }
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ElementViewRegion {
        WholeCollection,
        WholeRepresentation {
            source_length: ValueId,
            obligation: ObligationId,
        },
        RepresentationBytes {
            start: ValueId,
            end: ValueId,
            obligation: ObligationId,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ReferenceStorageView {
        pub byte_offset: ValueId,
        pub stored_scalar: Option<ScalarType>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OperationKind {
        ActivateTaskRegister {
            service: ServiceId,
            entries: u16,
            gdt: StructuralArgument,
            task_state: StructuralArgument,
            selector: ValueId,
            scratch: StructuralArgument,
        },
        ReadTaskRegister {
            service: ServiceId,
        },
        ReloadSegmentDescriptors {
            service: ServiceId,
            entries: u16,
            kind: language_core::descriptor_tables::SegmentReload,
            gdt: StructuralArgument,
            selector: ValueId,
            scratch: StructuralArgument,
        },
        ReadSegmentSelector {
            service: ServiceId,
            register: language_core::descriptor_tables::SegmentRegister,
        },
        ReadScalarApplication {
            application: semantic_vocabulary::ScalarApplicationId,
        },
        EstablishNativeCodeLayout {
            machine: MachineId,
        },
        StorageAddress {
            source: StructuralArgument,
        },
        GlobalDescriptorTable {
            service: ServiceId,
            operation: language_core::inline_assembly::AsmGlobalDescriptorTableOperation,
            source: StructuralArgument,
        },
        EstablishAccessReference {
            source: StructuralArgument,
            resident: Option<crate::ResidentReferenceSource>,
        },
        EstablishReference {
            source: StructuralArgument,
            storage: Option<ReferenceStorageView>,
        },
        ReleaseReference {
            source: PlaceId,
            path: Vec<StructuralPathSegment>,
        },
        EstablishScalarArray {
            elements: Vec<ValueId>,
        },
        EstablishActivationClaim {
            bound: u64,
            committed: ValueId,
            backing_type: StructuralTypeId,
            established_case: StructuralCaseId,
            rejected_case: StructuralCaseId,
        },
        ReleaseActivationClaim {
            source: StructuralArgument,
            claim: ClaimId,
        },
        SplitActivationClaim {
            source: StructuralArgument,
            claim: ClaimId,
            bound_at: u64,
            committed_at: ValueId,
            covered_case: StructuralCaseId,
            rejected_case: StructuralCaseId,
        },
        MergeActivationClaim {
            lower: StructuralArgument,
            upper: StructuralArgument,
            lower_claim: ClaimId,
            upper_claim: ClaimId,
            covered_case: StructuralCaseId,
            rejected_case: StructuralCaseId,
        },
        EstablishStructuralArray {
            elements: Vec<StructuralArgument>,
        },
        EstablishZeroedStructural {},
        EstablishStructuralLocal {},
        EstablishPrimitiveLocal {
            value: ValueId,
            range_obligation: Option<ObligationId>,
        },
        PrimitiveScalarRead {
            source: PlaceId,
            path: Vec<StructuralPathSegment>,
        },
        StructuralByteSequenceFieldLength {
            source: PlaceId,
            path: Vec<StructuralPathSegment>,
            field: StructuralFieldId,
        },
        StructuralByteSequenceFieldByteStore {
            destination: PlaceId,
            path: Vec<StructuralPathSegment>,
            field: StructuralFieldId,
            index: ValueId,
            value: ValueId,
            length: ValueId,
            obligation: ObligationId,
        },
        WriteOnlyPrimitiveStore {
            destination: PlaceId,
            path: Vec<StructuralPathSegment>,
            value: ValueId,
            range_obligation: Option<ObligationId>,
        },
        ByteSequenceStore {
            destination: PlaceId,
            source: PlaceId,
            length: ValueId,
            obligation: ObligationId,
        },
        StructuralByteSequenceFieldStore {
            destination: PlaceId,
            path: Vec<StructuralPathSegment>,
            field: StructuralFieldId,
            source: PlaceId,
            length: ValueId,
            obligation: ObligationId,
        },
        StructuralByteViewFieldStore {
            destination: PlaceId,
            path: Vec<StructuralPathSegment>,
            field: Option<StructuralFieldId>,
            source: PlaceId,
            predicate_obligation: Option<ObligationId>,
        },
        StructuralByteSequenceFieldRead {
            source: PlaceId,
            path: Vec<StructuralPathSegment>,
            field: StructuralFieldId,
            index: ValueId,
            length: ValueId,
            obligation: ObligationId,
        },
        StructuralScalarFieldStore {
            destination: PlaceId,
            path: Vec<StructuralPathSegment>,
            field: StructuralFieldId,
            value: ValueId,
            range_obligation: Option<ObligationId>,
        },
        MoveStructuralField {
            source: PlaceId,
            path: Vec<StructuralPathSegment>,
            field: Option<StructuralFieldId>,
        },
        StoreStructuralField {
            disposition: StructuralStoreDisposition,
            destination: PlaceId,
            path: Vec<StructuralPathSegment>,
            field: Option<StructuralFieldId>,
            value: StructuralArgument,
        },
        EstablishScalarCase {
            result_case: StructuralCaseId,
            fields: Vec<crate::ScalarCaseField>,
        },
        EstablishStructuralCase {
            result_case: StructuralCaseId,
            fields: Vec<crate::RecordFieldInitializer>,
        },
        StructuralCaseMembership {
            source: PlaceId,
            path: Vec<StructuralPathSegment>,
            case: StructuralCaseId,
        },
        MoveStructuralValue {
            source: StructuralArgument,
        },
        TransformContent {
            source: StructuralArgument,
            parents: Vec<StructuralArgument>,
            derivations: Vec<crate::ContentTransformationDerivation>,
        },
        StructuralLeafCopy {
            source: PlaceId,
            path: Vec<StructuralPathSegment>,
        },
        StructuralCaseLeafCopy {
            source: PlaceId,
            path: Vec<CanonicalStructuralPathSegment>,
        },
        EstablishByteSequenceConcatenation {
            left: ByteSequenceOperand,
            right: ByteSequenceOperand,
            capacity_obligation: ObligationId,
        },
        EstablishByteSequenceLiteral {
            destination: PlaceId,
            bytes: Vec<u8>,
            qualifications: Vec<StructuralDomainId>,
        },
        ByteSequenceLength {
            source: StructuralArgument,
        },
        ByteSequenceRead {
            source: PlaceId,
            index: ValueId,
            length: ValueId,
            obligation: ObligationId,
        },
        ByteSequenceWrite {
            destination: PlaceId,
            index: ValueId,
            value: ValueId,
            length: ValueId,
            obligation: ObligationId,
        },
        ByteSequenceSubslice {
            source: StructuralArgument,
            start: ValueId,
            end: ValueId,
            length: ValueId,
            obligation: ObligationId,
        },
        ByteSequencePredicate {
            operand: ByteSequenceOperand,
            predicate: crate::ByteSequencePredicate,
        },
        ByteSequenceEqual {
            left: ByteSequenceOperand,
            right: ByteSequenceOperand,
        },
        EstablishElementView {
            destination: PlaceId,
            source: StructuralArgument,
            element: StructuralTypeId,
            region: ElementViewRegion,
        },
        EstablishMutableElementView {
            destination: PlaceId,
            source: StructuralArgument,
            element: StructuralTypeId,
            region: ElementViewRegion,
        },
        ElementViewLength {
            source: StructuralArgument,
        },
        ElementViewRead {
            source: PlaceId,
            index: ValueId,
            length: ValueId,
            obligation: ObligationId,
            path: Vec<StructuralPathSegment>,
        },
        ElementViewSubslice {
            source: StructuralArgument,
            start: ValueId,
            end: ValueId,
            length: ValueId,
            obligation: ObligationId,
        },
        ElementViewWrite {
            destination: StructuralArgument,
            path: Vec<StructuralPathSegment>,
            index: ValueId,
            value: ValueId,
            length: ValueId,
            obligation: ObligationId,
        },
        EstablishTrivialAffineLocal {
            destination: PlaceId,
        },
        EstablishRecord {
            fields: Vec<crate::RecordFieldInitializer>,
        },
        SelectImplementation {
            table: u32,
        },
        CallDynamic {
            selector: ValueId,
            interface: u32,
            slot: u32,
            arguments: Vec<ValueId>,
            family_actuals: Vec<ValueId>,
            erased_arguments: Vec<ScalarTerm>,
            erased_proof_arguments: Vec<ProofTerm>,
            structural_arguments: Vec<StructuralArgument>,
            claim_transfers: Vec<ClaimTransfer>,
            requirement_obligations: Vec<ObligationId>,
            crash_continuations: Vec<CrashRouteBucket>,
        },
        Call {
            callee: MachineId,
            arguments: Vec<ValueId>,
            erased_arguments: Vec<ScalarTerm>,
            erased_proof_arguments: Vec<ProofTerm>,
            requirement_obligations: Vec<ObligationId>,
            crash_continuations: Vec<CrashRouteBucket>,
        },
        CallUnit {
            callee: MachineId,
            arguments: Vec<ValueId>,
            erased_arguments: Vec<ScalarTerm>,
            erased_proof_arguments: Vec<ProofTerm>,
            structural_arguments: Vec<StructuralArgument>,
            claim_transfers: Vec<ClaimTransfer>,
            requirement_obligations: Vec<ObligationId>,
            crash_continuations: Vec<CrashRouteBucket>,
        },
        CallStructuralScalar {
            callee: MachineId,
            arguments: Vec<ValueId>,
            erased_arguments: Vec<ScalarTerm>,
            erased_proof_arguments: Vec<ProofTerm>,
            structural_arguments: Vec<StructuralArgument>,
            claim_transfers: Vec<ClaimTransfer>,
            requirement_obligations: Vec<ObligationId>,
            crash_continuations: Vec<CrashRouteBucket>,
        },
        CallStructural {
            callee: MachineId,
            structural_arguments: Vec<StructuralArgument>,
            claim_transfers: Vec<ClaimTransfer>,
            requirement_obligations: Vec<ObligationId>,
            crash_continuations: Vec<CrashRouteBucket>,
            selected_evidence: Vec<OutcomeSpecificCallEvidence>,
        },
        CallStructuralWithScalarArguments {
            callee: MachineId,
            arguments: Vec<ValueId>,
            erased_arguments: Vec<ScalarTerm>,
            erased_proof_arguments: Vec<ProofTerm>,
            structural_arguments: Vec<StructuralArgument>,
            claim_transfers: Vec<ClaimTransfer>,
            requirement_obligations: Vec<ObligationId>,
            crash_continuations: Vec<CrashRouteBucket>,
            selected_evidence: Vec<OutcomeSpecificCallEvidence>,
        },
        BoundaryCall {
            boundary: BoundaryMachineId,
            arguments: Vec<ValueId>,
            structural_arguments: Vec<StructuralArgument>,
            completion_receipts: Vec<CompletionReceipt>,
            requirement_obligations: Vec<ObligationId>,
        },
        PortWrite {
            service: ServiceId,
            port: u16,
            value: u8,
        },
        RegisterAccess {
            service: Option<ServiceId>,
            event: super::RegisterAccessEvent,
        },
        IntegerConstant {
            value: IntegerValue,
        },
        BooleanConstant {
            value: bool,
        },
        IeeeFloatConstant {
            value: IeeeFloatValue,
        },
        FloatResultPolicy {
            policy: crate::FloatResultPolicy,
        },
        IeeeFloatCompare {
            comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
            left: ValueId,
            right: ValueId,
        },
        ScalarRepresentationView {
            operand: ValueId,
            landing_obligation: Option<ObligationId>,
        },
        NearestIeeeFloatFusedMultiplyAdd {
            left: ValueId,
            right: ValueId,
            addend: ValueId,
        },
        BooleanStructuralField {
            source: PlaceId,
            path: Vec<CanonicalStructuralPathSegment>,
            field: StructuralFieldId,
        },
        ScalarStructuralField {
            source: PlaceId,
            path: Vec<CanonicalStructuralPathSegment>,
            field: StructuralFieldId,
        },
        BooleanNot {
            operand: ValueId,
        },
        BooleanEqual {
            left: ValueId,
            right: ValueId,
        },
        IntegerEqual {
            left: ValueId,
            right: ValueId,
        },
        IntegerLessThan {
            left: ValueId,
            right: ValueId,
        },
        IntegerLessOrEqual {
            left: ValueId,
            right: ValueId,
        },
        IntegerBitwiseNot {
            operand: ValueId,
        },
        IntegerWiden {
            operand: ValueId,
        },
        IntegerExactCast {
            operand: ValueId,
            obligation: ObligationId,
        },
        IntegerBitwiseAnd {
            left: ValueId,
            right: ValueId,
        },
        IntegerBitwiseOr {
            left: ValueId,
            right: ValueId,
        },
        IntegerBitwiseXor {
            left: ValueId,
            right: ValueId,
        },
        WrappingIntegerShiftLeft {
            value: ValueId,
            count: ValueId,
        },
        WrappingIntegerShiftRight {
            value: ValueId,
            count: ValueId,
        },
        ExactIntegerShiftLeft {
            value: ValueId,
            count: ValueId,
            obligation: ObligationId,
        },
        ExactIntegerShiftRight {
            value: ValueId,
            count: ValueId,
            obligation: ObligationId,
        },
        ExactIntegerAdd {
            left: ValueId,
            right: ValueId,
            obligation: ObligationId,
        },
        ExactIntegerSubtract {
            left: ValueId,
            right: ValueId,
            obligation: ObligationId,
        },
        ExactIntegerMultiply {
            left: ValueId,
            right: ValueId,
            obligation: ObligationId,
        },
        ExactIntegerDivide {
            left: ValueId,
            right: ValueId,
            obligation: ObligationId,
        },
        ExactIntegerRemainder {
            left: ValueId,
            right: ValueId,
            obligation: ObligationId,
        },
        WrappingIntegerDivide {
            left: ValueId,
            right: ValueId,
            obligation: ObligationId,
        },
        WrappingIntegerRemainder {
            left: ValueId,
            right: ValueId,
            obligation: ObligationId,
        },
        SaturatingIntegerDivide {
            left: ValueId,
            right: ValueId,
            obligation: ObligationId,
        },
        SaturatingIntegerRemainder {
            left: ValueId,
            right: ValueId,
            obligation: ObligationId,
        },
        WrappingIntegerAdd {
            left: ValueId,
            right: ValueId,
        },
        SaturatingIntegerAdd {
            left: ValueId,
            right: ValueId,
        },
        WrappingIntegerSubtract {
            left: ValueId,
            right: ValueId,
        },
        SaturatingIntegerSubtract {
            left: ValueId,
            right: ValueId,
        },
        WrappingIntegerMultiply {
            left: ValueId,
            right: ValueId,
        },
        SaturatingIntegerMultiply {
            left: ValueId,
            right: ValueId,
        },
        TrappingInteger {
            operation: crate::TrappingIntegerOperation,
        },
        AtomicAccess {
            place: PlaceId,
            path: Vec<StructuralPathSegment>,
            event: crate::AtomicAccessEvent,
            resident_obligation: Option<semantic_vocabulary::ObligationId>,
        },
        AtomicFence {
            ordering: language_core::atomic::FenceOrdering,
        },
    }

    impl OperationKind {
        pub fn direct_callee(&self) -> Option<MachineId> {
            match self {
                Self::Call { callee, .. }
                | Self::CallUnit { callee, .. }
                | Self::CallStructuralScalar { callee, .. }
                | Self::CallStructural { callee, .. }
                | Self::CallStructuralWithScalarArguments { callee, .. } => Some(*callee),
                _ => None,
            }
        }

        pub fn constructor_arguments(&self) -> impl Iterator<Item = &StructuralArgument> {
            let (elements, fields): (&[StructuralArgument], &[crate::RecordFieldInitializer]) =
                match self {
                    Self::EstablishStructuralArray { elements } => (elements, &[]),
                    Self::EstablishRecord { fields }
                    | Self::EstablishStructuralCase { fields, .. } => (&[], fields),
                    _ => (&[], &[]),
                };
            elements
                .iter()
                .chain(fields.iter().filter_map(|field| match &field.value {
                    crate::RecordFieldValue::Structural(argument) => Some(argument),
                    crate::RecordFieldValue::Scalar { .. } => None,
                }))
        }

        pub fn map_scalar_uses(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
            for path in self.structural_projection_paths_mut() {
                for segment in path {
                    if let crate::StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                        *index = map(*index);
                    }
                }
            }
            match self {
                Self::EstablishNativeCodeLayout { .. }
                | Self::ReleaseActivationClaim { .. }
                | Self::MergeActivationClaim { .. } => {}
                Self::EstablishActivationClaim { committed, .. }
                | Self::SplitActivationClaim {
                    committed_at: committed,
                    ..
                } => *committed = map(*committed),
                Self::EstablishReference { storage, .. } => {
                    if let Some(storage) = storage {
                        storage.byte_offset = map(storage.byte_offset);
                    }
                }
                Self::EstablishAccessReference { .. }
                | Self::StorageAddress { .. }
                | Self::GlobalDescriptorTable { .. } => {}
                Self::EstablishByteSequenceConcatenation { left, right, .. } => {
                    left.remap_length(map);
                    right.remap_length(map);
                }
                Self::EstablishScalarArray { elements } => {
                    for element in elements {
                        *element = map(*element);
                    }
                }
                Self::EstablishScalarCase { fields, .. } => {
                    for field in fields {
                        field.value = map(field.value);
                    }
                }
                Self::EstablishStructuralCase { fields, .. } => {
                    for field in fields {
                        if let crate::RecordFieldValue::Scalar { value, .. } = &mut field.value {
                            *value = map(*value);
                        }
                    }
                }
                Self::EstablishRecord { fields } => {
                    for field in fields {
                        if let crate::RecordFieldValue::Scalar { value, .. } = &mut field.value {
                            *value = map(*value);
                        }
                    }
                }
                Self::EstablishPrimitiveLocal { value, .. }
                | Self::WriteOnlyPrimitiveStore { value, .. }
                | Self::StructuralScalarFieldStore { value, .. }
                | Self::BooleanNot { operand: value }
                | Self::IntegerBitwiseNot { operand: value }
                | Self::IntegerWiden { operand: value }
                | Self::IntegerExactCast { operand: value, .. }
                | Self::ScalarRepresentationView { operand: value, .. } => *value = map(*value),
                Self::StructuralByteSequenceFieldStore { length, .. }
                | Self::ByteSequenceStore { length, .. } => {
                    *length = map(*length);
                }
                Self::StructuralByteSequenceFieldByteStore {
                    index,
                    value,
                    length,
                    ..
                }
                | Self::ByteSequenceWrite {
                    index,
                    value,
                    length,
                    ..
                } => {
                    *index = map(*index);
                    *value = map(*value);
                    *length = map(*length);
                }
                Self::ByteSequenceRead { index, length, .. } => {
                    *index = map(*index);
                    *length = map(*length);
                }
                Self::StructuralByteSequenceFieldRead { index, length, .. } => {
                    *index = map(*index);
                    *length = map(*length);
                }
                Self::ByteSequencePredicate { operand, .. } => operand.remap_length(map),
                Self::ByteSequenceEqual { left, right } => {
                    left.remap_length(map);
                    right.remap_length(map);
                }
                Self::EstablishElementView { region, .. }
                | Self::EstablishMutableElementView { region, .. } => {
                    if let ElementViewRegion::RepresentationBytes { start, end, .. } = region {
                        *start = map(*start);
                        *end = map(*end);
                    } else if let ElementViewRegion::WholeRepresentation { source_length, .. } =
                        region
                    {
                        *source_length = map(*source_length);
                    }
                }
                Self::ByteSequenceSubslice {
                    start, end, length, ..
                }
                | Self::ElementViewSubslice {
                    start, end, length, ..
                } => {
                    *start = map(*start);
                    *end = map(*end);
                    *length = map(*length);
                }
                Self::ElementViewRead { index, length, .. } => {
                    *index = map(*index);
                    *length = map(*length);
                }
                Self::ElementViewWrite {
                    index,
                    value,
                    length,
                    ..
                } => {
                    *index = map(*index);
                    *value = map(*value);
                    *length = map(*length);
                }
                Self::BooleanEqual { left, right }
                | Self::IeeeFloatCompare { left, right, .. }
                | Self::IntegerEqual { left, right }
                | Self::IntegerLessThan { left, right }
                | Self::IntegerLessOrEqual { left, right }
                | Self::IntegerBitwiseAnd { left, right }
                | Self::IntegerBitwiseOr { left, right }
                | Self::IntegerBitwiseXor { left, right }
                | Self::WrappingIntegerAdd { left, right }
                | Self::SaturatingIntegerAdd { left, right }
                | Self::WrappingIntegerSubtract { left, right }
                | Self::SaturatingIntegerSubtract { left, right }
                | Self::WrappingIntegerMultiply { left, right }
                | Self::SaturatingIntegerMultiply { left, right }
                | Self::ExactIntegerAdd { left, right, .. }
                | Self::ExactIntegerSubtract { left, right, .. }
                | Self::ExactIntegerMultiply { left, right, .. }
                | Self::ExactIntegerDivide { left, right, .. }
                | Self::ExactIntegerRemainder { left, right, .. }
                | Self::WrappingIntegerDivide { left, right, .. }
                | Self::WrappingIntegerRemainder { left, right, .. }
                | Self::SaturatingIntegerDivide { left, right, .. }
                | Self::SaturatingIntegerRemainder { left, right, .. } => {
                    *left = map(*left);
                    *right = map(*right);
                }
                Self::WrappingIntegerShiftLeft { value, count }
                | Self::WrappingIntegerShiftRight { value, count }
                | Self::ExactIntegerShiftLeft { value, count, .. }
                | Self::ExactIntegerShiftRight { value, count, .. } => {
                    *value = map(*value);
                    *count = map(*count);
                }
                Self::TrappingInteger { operation } => operation.map_operands(map),
                Self::FloatResultPolicy { policy } => policy.map_operands(map),
                Self::AtomicAccess { event, .. } => event.map_operands(map),
                Self::ActivateTaskRegister { selector, .. }
                | Self::ReloadSegmentDescriptors { selector, .. } => *selector = map(*selector),
                Self::ReadTaskRegister { .. } | Self::ReadSegmentSelector { .. } => {}
                Self::RegisterAccess { event, .. } => event.map_operands(map),
                Self::NearestIeeeFloatFusedMultiplyAdd {
                    left,
                    right,
                    addend,
                } => {
                    *left = map(*left);
                    *right = map(*right);
                    *addend = map(*addend);
                }
                Self::Call { arguments, .. }
                | Self::CallUnit { arguments, .. }
                | Self::CallStructuralScalar { arguments, .. }
                | Self::CallStructuralWithScalarArguments { arguments, .. }
                | Self::BoundaryCall { arguments, .. } => {
                    for argument in arguments {
                        *argument = map(*argument);
                    }
                }
                Self::CallDynamic {
                    selector,
                    arguments,
                    family_actuals,
                    ..
                } => {
                    *selector = map(*selector);
                    for argument in arguments {
                        *argument = map(*argument);
                    }
                    for actual in family_actuals {
                        *actual = map(*actual);
                    }
                }
                Self::SelectImplementation { .. }
                | Self::ReleaseReference { .. }
                | Self::PrimitiveScalarRead { .. }
                | Self::MoveStructuralField { .. }
                | Self::MoveStructuralValue { .. }
                | Self::TransformContent { .. }
                | Self::StoreStructuralField { .. }
                | Self::StructuralCaseMembership { .. }
                | Self::StructuralLeafCopy { .. }
                | Self::StructuralCaseLeafCopy { .. }
                | Self::EstablishByteSequenceLiteral { .. }
                | Self::ByteSequenceLength { .. }
                | Self::ElementViewLength { .. }
                | Self::StructuralByteSequenceFieldLength { .. }
                | Self::StructuralByteViewFieldStore { .. }
                | Self::EstablishTrivialAffineLocal { .. }
                | Self::CallStructural { .. }
                | Self::PortWrite { .. }
                | Self::AtomicFence { .. }
                | Self::IntegerConstant { .. }
                | Self::ReadScalarApplication { .. }
                | Self::BooleanConstant { .. }
                | Self::IeeeFloatConstant { .. }
                | Self::EstablishStructuralArray { .. }
                | Self::EstablishZeroedStructural { .. }
                | Self::EstablishStructuralLocal { .. }
                | Self::BooleanStructuralField { .. }
                | Self::ScalarStructuralField { .. } => {}
            }
        }

        pub fn structural_arguments(&self) -> &[StructuralArgument] {
            match self {
                crate::static_call_structural_argument_variants!(structural_arguments) => {
                    structural_arguments
                }
                _ => &[],
            }
        }

        pub fn claim_transfers(&self) -> &[ClaimTransfer] {
            match self {
                Self::CallUnit {
                    claim_transfers, ..
                }
                | Self::CallStructuralScalar {
                    claim_transfers, ..
                }
                | Self::CallStructural {
                    claim_transfers, ..
                }
                | Self::CallStructuralWithScalarArguments {
                    claim_transfers, ..
                }
                | Self::CallDynamic {
                    claim_transfers, ..
                } => claim_transfers,
                _ => &[],
            }
        }

        pub fn structural_arguments_mut(&mut self) -> Option<&mut Vec<StructuralArgument>> {
            match self {
                crate::static_call_structural_argument_variants!(structural_arguments) => {
                    Some(structural_arguments)
                }
                _ => None,
            }
        }
    }

    #[macro_export]
    macro_rules! static_call_structural_argument_variants {
        ($arguments:ident) => {
            $crate::OperationKind::CallUnit {
                structural_arguments: $arguments,
                ..
            } | $crate::OperationKind::CallStructuralScalar {
                structural_arguments: $arguments,
                ..
            } | $crate::OperationKind::CallStructural {
                structural_arguments: $arguments,
                ..
            } | $crate::OperationKind::CallStructuralWithScalarArguments {
                structural_arguments: $arguments,
                ..
            } | $crate::OperationKind::BoundaryCall {
                structural_arguments: $arguments,
                ..
            }
        };
    }

    #[macro_export]
    macro_rules! call_structural_argument_variants {
        ($arguments:ident) => {
            $crate::static_call_structural_argument_variants!($arguments)
                | $crate::OperationKind::CallDynamic {
                    structural_arguments: $arguments,
                    ..
                }
        };
    }

    #[macro_export]
    macro_rules! integer_arithmetic_operations {
        () => {
            $crate::OperationKind::ExactIntegerAdd { .. }
                | $crate::OperationKind::WrappingIntegerAdd { .. }
                | $crate::OperationKind::ExactIntegerSubtract { .. }
                | $crate::OperationKind::WrappingIntegerSubtract { .. }
                | $crate::OperationKind::ExactIntegerMultiply { .. }
                | $crate::OperationKind::WrappingIntegerMultiply { .. }
                | $crate::OperationKind::ExactIntegerDivide { .. }
                | $crate::OperationKind::ExactIntegerRemainder { .. }
                | $crate::OperationKind::WrappingIntegerDivide { .. }
                | $crate::OperationKind::WrappingIntegerRemainder { .. }
                | $crate::OperationKind::SaturatingIntegerDivide { .. }
                | $crate::OperationKind::SaturatingIntegerRemainder { .. }
                | $crate::OperationKind::SaturatingIntegerAdd { .. }
                | $crate::OperationKind::SaturatingIntegerSubtract { .. }
                | $crate::OperationKind::SaturatingIntegerMultiply { .. }
        };
    }

    #[macro_export]
    macro_rules! move_or_store_structural_field_pattern {
        () => {
            $crate::OperationKind::MoveStructuralField { .. }
                | $crate::OperationKind::StoreStructuralField { .. }
        };
    }

    #[macro_export]
    macro_rules! structural_leaf_copy_pair_pattern {
        () => {
            $crate::OperationKind::StructuralLeafCopy { .. }
                | $crate::OperationKind::StructuralCaseLeafCopy { .. }
        };
    }

    #[macro_export]
    macro_rules! call_structural_pair_pattern {
        ($($fields:tt)*) => {
            $crate::OperationKind::CallStructural { $($fields)* }
                | $crate::OperationKind::CallStructuralWithScalarArguments { $($fields)* }
        };
    }

    #[macro_export]
    macro_rules! integer_less_or_equal_pair_pattern {
        ($($fields:tt)*) => {
            $crate::OperationKind::IntegerLessThan { $($fields)* }
                | $crate::OperationKind::IntegerLessOrEqual { $($fields)* }
        };
    }

    #[macro_export]
    macro_rules! element_view_establish_pair_pattern {
        ($($fields:tt)*) => {
            $crate::OperationKind::EstablishElementView { $($fields)* }
                | $crate::OperationKind::EstablishMutableElementView { $($fields)* }
        };
    }
}
pub(crate) mod ranking {
    use arena::{Arena, HandleSpan};
    use semantic_vocabulary::{BlockId, EdgeId, IntegerType, ValueId};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TerminalRankedScc {
        Natural(Vec<TerminalNaturalCycle>),
    }

    #[derive(Debug, Clone)]
    pub struct TerminalNaturalCycle {
        pub observations: Arena<TerminalIntegerRankValue>,
        pub ranks: Vec<TerminalBlockNaturalRank>,
        pub edges: Vec<TerminalNaturalRankEdge>,
    }

    impl TerminalNaturalCycle {
        pub fn rank_values(
            &self,
            values: HandleSpan<TerminalIntegerRankValue>,
        ) -> Option<&[TerminalIntegerRankValue]> {
            let signature = self.observations.span(self.ranks.first()?.values)?;
            let values = self.observations.span(values)?;
            (!signature.is_empty()
                && values.len() == signature.len()
                && values.iter().zip(signature).all(|(value, expected)| {
                    !value.integer_type.is_address() && value.integer_type == expected.integer_type
                }))
            .then_some(values)
        }

        pub fn scalar_rank_value(
            &self,
            values: HandleSpan<TerminalIntegerRankValue>,
        ) -> Option<&TerminalIntegerRankValue> {
            let [value] = self.rank_values(values)? else {
                return None;
            };
            Some(value)
        }
    }

    impl PartialEq for TerminalNaturalCycle {
        fn eq(&self, other: &Self) -> bool {
            self.ranks.len() == other.ranks.len()
                && self.edges.len() == other.edges.len()
                && self.ranks.iter().zip(&other.ranks).all(|(left, right)| {
                    left.block == right.block
                        && self.observations.span(left.values)
                            == other.observations.span(right.values)
                })
                && self.edges.iter().zip(&other.edges).all(|(left, right)| {
                    left.edge == right.edge
                        && left.source == right.source
                        && left.target == right.target
                        && left.comparison == right.comparison
                        && self.observations.span(left.successor_ranks)
                            == other.observations.span(right.successor_ranks)
                })
        }
    }
    impl Eq for TerminalNaturalCycle {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalIntegerRankValue {
        pub integer_type: IntegerType,
        pub value: ValueId,
    }

    impl Default for TerminalIntegerRankValue {
        fn default() -> Self {
            Self {
                integer_type: IntegerType::supported_address_representation(),
                value: ValueId::FIRST,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TerminalBlockNaturalRank {
        pub block: BlockId,
        pub values: HandleSpan<TerminalIntegerRankValue>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TerminalNaturalRankEdge {
        pub edge: EdgeId,
        pub source: BlockId,
        pub target: BlockId,
        pub successor_ranks: HandleSpan<TerminalIntegerRankValue>,
        pub comparison: TerminalNaturalRankComparison,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalNaturalRankComparison {
        Preserving,
        Strict,
    }
}
pub(crate) mod record_field {
    use semantic_vocabulary::{ObligationId, StructuralFieldId, ValueId};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RecordFieldInitializer {
        pub field: StructuralFieldId,
        pub value: RecordFieldValue,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum RecordFieldValue {
        Scalar {
            value: ValueId,
            range_obligation: Option<ObligationId>,
        },
        Structural(crate::StructuralArgument),
    }

    impl RecordFieldValue {
        pub fn as_structural(&self) -> Option<&crate::StructuralArgument> {
            let RecordFieldValue::Structural(structural) = self else {
                return None;
            };
            Some(structural)
        }
    }
}
pub(crate) mod register_access {

    use language_core::inline_assembly::{
        AsmInterruptControlKind, AsmRegisterOperation, AsmSystemRegister,
    };
    use semantic_vocabulary::ValueId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RegisterAccessEvent<Operand = ValueId> {
        ProcessorHalt,
        MemoryFence {
            kind: language_core::inline_assembly::AsmFenceKind,
        },
        InterruptControl {
            kind: AsmInterruptControlKind,
        },
        Read {
            register: AsmSystemRegister,
        },
        Write {
            register: AsmSystemRegister,
            value: Operand,
        },
        PortRead {
            port: Operand,
        },
        PortWrite {
            port: Operand,
            value: Operand,
        },
        ModelSpecificRead {
            index: Operand,
        },
        ModelSpecificWrite {
            index: Operand,
            value: Operand,
        },
    }

    impl<Operand> RegisterAccessEvent<Operand> {
        pub const fn operation(&self) -> AsmRegisterOperation {
            match self {
                Self::ProcessorHalt => AsmRegisterOperation::ProcessorHalt,
                Self::MemoryFence { kind } => AsmRegisterOperation::MemoryFence(*kind),
                Self::InterruptControl { kind } => AsmRegisterOperation::InterruptControl(*kind),
                Self::Read { register } => AsmRegisterOperation::SystemRead(*register),
                Self::Write { register, .. } => AsmRegisterOperation::SystemWrite(*register),
                Self::PortRead { .. } => AsmRegisterOperation::PortRead,
                Self::PortWrite { .. } => AsmRegisterOperation::PortWrite,
                Self::ModelSpecificRead { .. } => AsmRegisterOperation::ModelSpecificRead,
                Self::ModelSpecificWrite { .. } => AsmRegisterOperation::ModelSpecificWrite,
            }
        }

        pub fn inputs(&self) -> impl Iterator<Item = &Operand> {
            match self {
                Self::Read { .. }
                | Self::ProcessorHalt
                | Self::InterruptControl { .. }
                | Self::MemoryFence { .. } => [None, None],
                Self::Write { value, .. } => [Some(value), None],
                Self::PortRead { port } => [Some(port), None],
                Self::PortWrite { port, value } => [Some(port), Some(value)],
                Self::ModelSpecificRead { index } => [Some(index), None],
                Self::ModelSpecificWrite { index, value } => [Some(index), Some(value)],
            }
            .into_iter()
            .flatten()
        }

        pub fn try_map_inputs<Other, Error>(
            &self,
            mut map: impl FnMut(&Operand) -> Result<Other, Error>,
        ) -> Result<RegisterAccessEvent<Other>, Error> {
            Ok(match self {
                Self::ProcessorHalt => RegisterAccessEvent::ProcessorHalt,
                Self::MemoryFence { kind } => RegisterAccessEvent::MemoryFence { kind: *kind },
                Self::InterruptControl { kind } => {
                    RegisterAccessEvent::InterruptControl { kind: *kind }
                }
                Self::Read { register } => RegisterAccessEvent::Read {
                    register: *register,
                },
                Self::Write { register, value } => RegisterAccessEvent::Write {
                    register: *register,
                    value: map(value)?,
                },
                Self::PortRead { port } => RegisterAccessEvent::PortRead { port: map(port)? },
                Self::PortWrite { port, value } => RegisterAccessEvent::PortWrite {
                    port: map(port)?,
                    value: map(value)?,
                },
                Self::ModelSpecificRead { index } => {
                    RegisterAccessEvent::ModelSpecificRead { index: map(index)? }
                }
                Self::ModelSpecificWrite { index, value } => {
                    RegisterAccessEvent::ModelSpecificWrite {
                        index: map(index)?,
                        value: map(value)?,
                    }
                }
            })
        }
    }

    impl<Operand: Copy> RegisterAccessEvent<Operand> {
        pub fn map_operands(&mut self, mut map: impl FnMut(Operand) -> Operand) {
            *self = self
                .try_map_inputs::<Operand, std::convert::Infallible>(|value| Ok(map(*value)))
                .unwrap();
        }
    }
}
pub(crate) mod scalar_case_field {
    use semantic_vocabulary::{ObligationId, StructuralFieldId, ValueId};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ScalarCaseField {
        pub field: StructuralFieldId,
        pub value: ValueId,
        pub range_obligation: Option<ObligationId>,
    }
}
pub(crate) mod target_families {

    pub use semantic_vocabulary::TargetMachineCallable;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TargetMachineSupply {
        CheckedBody,
        Requirement,
        TopLevelRequirement,
        Boundary,
        AdmissionClaim,
        ExternalRealization,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetMachineFamilyMember {
        pub target: String,
        pub callable: TargetMachineCallable,
        pub supply: TargetMachineSupply,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetMachineFamily {
        pub canonical: TargetMachineCallable,
        pub head_contract: [u8; 32],
        pub members: Vec<TargetMachineFamilyMember>,
    }
}
pub(crate) mod termination {
    use crate::{
        CrashPredicateTerm, StructuralArgument, StructuralResultClaimBinding,
        TerminalAffineCleanupAction,
    };
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, ClaimId, EdgeId, ExternalCompletionKind, MachineId,
        ObligationId, PlaceId, ProofTerm, ScalarTerm, StructuralCaseId, StructuralFieldId, ValueId,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Terminator {
        InvokeNoNormal {
            edge: EdgeId,
            static_reach_binding: Option<u32>,
            callee: MachineId,
            arguments: Vec<ValueId>,
            erased_arguments: Vec<ScalarTerm>,
            erased_proof_arguments: Vec<ProofTerm>,
            structural_arguments: Vec<StructuralArgument>,
            claim_transfers: Vec<crate::ClaimTransfer>,
            requirement_obligations: Vec<ObligationId>,
            crash_continuations: Vec<crate::CrashRouteBucket>,
        },
        ExternalCompletion {
            edge: EdgeId,
            boundary: BoundaryMachineId,
            kind: ExternalCompletionKind,
            arguments: Vec<ValueId>,
            structural_arguments: Vec<StructuralArgument>,
            requirement_obligations: Vec<ObligationId>,
            frontier_lower_bound: Vec<ClaimId>,
        },
        Jump {
            edge: EdgeId,
            target: BlockId,
            arguments: Vec<ValueId>,
            erased_arguments: Vec<ScalarTerm>,
            erased_proof_arguments: Vec<ProofTerm>,
            structural_arguments: Vec<StructuralArgument>,
            cleanup_actions: Vec<TerminalAffineCleanupAction>,
        },
        Conditional {
            condition: ValueId,
            when_true: SuccessorEdge,
            when_false: SuccessorEdge,
        },
        StructuralCase {
            source: PlaceId,
            path: Vec<crate::StructuralPathSegment>,
            cases: Vec<StructuralCaseSuccessorEdge>,
        },
        Return {
            edge: EdgeId,
            value: ValueId,
            cleanup_actions: Vec<TerminalAffineCleanupAction>,
        },
        ReturnUnit {
            edge: EdgeId,
            cleanup_actions: Vec<TerminalAffineCleanupAction>,
        },
        ReturnStructural {
            edge: EdgeId,
            source: PlaceId,
            returned_claims: Vec<StructuralResultClaimBinding>,
            cleanup_actions: Vec<TerminalAffineCleanupAction>,
        },
        Crash {
            edge: EdgeId,
            cause: CrashCause,
            site_guard: Vec<CrashPredicateTerm>,
            frontier_lower_bound: Vec<ClaimId>,
        },
    }

    pub use semantic_vocabulary::CrashCause;

    impl Terminator {
        pub fn storage_uses(&self) -> Vec<crate::OperationProjection<'_>> {
            let mut projections = Vec::new();
            fn argument(argument: &StructuralArgument) -> crate::OperationProjection<'_> {
                crate::OperationProjection {
                    root: argument.place,
                    path: &argument.path,
                }
            }
            match self {
                Self::Jump {
                    structural_arguments,
                    ..
                }
                | Self::InvokeNoNormal {
                    structural_arguments,
                    ..
                }
                | Self::ExternalCompletion {
                    structural_arguments,
                    ..
                } => {
                    projections.extend(structural_arguments.iter().map(argument));
                }
                Self::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        projections.extend(edge.structural_arguments.iter().map(argument));
                    }
                }
                Self::StructuralCase { source, path, .. } => {
                    projections.push(crate::OperationProjection {
                        root: *source,
                        path,
                    });
                }
                Self::ReturnStructural { source, .. } => {
                    projections.push(crate::OperationProjection {
                        root: *source,
                        path: &[],
                    });
                }
                Self::Return { .. } | Self::ReturnUnit { .. } | Self::Crash { .. } => {}
            }
            for (_, actions) in self.cleanup_rosters() {
                for action in actions {
                    projections.push(crate::OperationProjection {
                        root: action.place(),
                        path: action.path(),
                    });
                }
            }
            projections
        }

        pub fn map_structural_uses(&mut self, map: &mut impl FnMut(PlaceId) -> PlaceId) {
            match self {
                Self::Jump {
                    structural_arguments,
                    ..
                }
                | Self::InvokeNoNormal {
                    structural_arguments,
                    ..
                }
                | Self::ExternalCompletion {
                    structural_arguments,
                    ..
                } => {
                    for argument in structural_arguments {
                        argument.place = map(argument.place);
                    }
                }
                Self::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        for argument in &mut edge.structural_arguments {
                            argument.place = map(argument.place);
                        }
                    }
                }
                Self::StructuralCase { source, cases, .. } => {
                    *source = map(*source);
                    for case in cases {
                        for argument in &mut case.structural_arguments {
                            argument.place = map(argument.place);
                        }
                    }
                }
                Self::ReturnStructural { source, .. } => *source = map(*source),
                Self::Return { .. } | Self::ReturnUnit { .. } | Self::Crash { .. } => {}
            }
        }

        pub fn cleanup_rosters(
            &self,
        ) -> impl Iterator<Item = (EdgeId, &[TerminalAffineCleanupAction])> {
            let (first, second, cases): (_, _, &[StructuralCaseSuccessorEdge]) = match self {
                Self::Jump {
                    edge,
                    cleanup_actions,
                    ..
                }
                | Self::Return {
                    edge,
                    cleanup_actions,
                    ..
                }
                | Self::ReturnUnit {
                    edge,
                    cleanup_actions,
                }
                | Self::ReturnStructural {
                    edge,
                    cleanup_actions,
                    ..
                } => (Some((*edge, cleanup_actions.as_slice())), None, &[]),
                Self::Conditional {
                    when_true,
                    when_false,
                    ..
                } => (
                    Some((when_true.edge, when_true.cleanup_actions.as_slice())),
                    Some((when_false.edge, when_false.cleanup_actions.as_slice())),
                    &[],
                ),
                Self::StructuralCase { cases, .. } => (None, None, cases),
                Self::Crash { .. }
                | Self::ExternalCompletion { .. }
                | Self::InvokeNoNormal { .. } => (None, None, &[]),
            };
            first.into_iter().chain(second).chain(
                cases
                    .iter()
                    .map(|case| (case.edge, case.cleanup_actions.as_slice())),
            )
        }

        pub const fn edge(&self) -> EdgeId {
            match self {
                Self::InvokeNoNormal { edge, .. }
                | Self::Jump { edge, .. }
                | Self::Return { edge, .. }
                | Self::ReturnUnit { edge, .. }
                | Self::ReturnStructural { edge, .. }
                | Self::Crash { edge, .. }
                | Self::ExternalCompletion { edge, .. } => *edge,
                Self::Conditional { .. } | Self::StructuralCase { .. } => {
                    panic!("a branching terminator has multiple successor edges")
                }
            }
        }

        pub fn edges(&self) -> impl Iterator<Item = EdgeId> + '_ {
            let edges = match self {
                Self::InvokeNoNormal { edge, .. }
                | Self::Jump { edge, .. }
                | Self::Return { edge, .. }
                | Self::ReturnUnit { edge, .. }
                | Self::ReturnStructural { edge, .. }
                | Self::Crash { edge, .. }
                | Self::ExternalCompletion { edge, .. } => vec![*edge],
                Self::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![when_true.edge, when_false.edge],
                Self::StructuralCase { cases, .. } => cases.iter().map(|case| case.edge).collect(),
            };
            edges.into_iter()
        }

        pub fn map_scalar_uses(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
            match self {
                Self::Jump { arguments, .. }
                | Self::ExternalCompletion { arguments, .. }
                | Self::InvokeNoNormal { arguments, .. } => {
                    for argument in arguments {
                        *argument = map(*argument);
                    }
                }
                Self::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    *condition = map(*condition);
                    for edge in [when_true, when_false] {
                        for argument in &mut edge.arguments {
                            *argument = map(*argument);
                        }
                    }
                }
                Self::Return { value, .. } => {
                    *value = map(*value);
                }
                Self::StructuralCase { path, .. } => {
                    for segment in path {
                        if let crate::StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                            *index = map(*index);
                        }
                    }
                }
                Self::ReturnUnit { .. } | Self::ReturnStructural { .. } | Self::Crash { .. } => {}
            }
        }
        pub fn successor_block_arguments(&self) -> Vec<&[StructuralArgument]> {
            match self {
                Self::Jump {
                    structural_arguments,
                    ..
                } => vec![structural_arguments.as_slice()],
                Self::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![
                    when_true.structural_arguments.as_slice(),
                    when_false.structural_arguments.as_slice(),
                ],
                Self::StructuralCase { cases, .. } => cases
                    .iter()
                    .map(|case| case.structural_arguments.as_slice())
                    .collect(),
                _ => Vec::new(),
            }
        }

        pub fn successor_edges(&self) -> Vec<(EdgeId, BlockId)> {
            match self {
                Self::Jump { edge, target, .. } => vec![(*edge, *target)],
                Self::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![
                    (when_true.edge, when_true.target),
                    (when_false.edge, when_false.target),
                ],
                Self::StructuralCase { cases, .. } => cases
                    .iter()
                    .map(|successor| (successor.edge, successor.target))
                    .collect(),
                Self::Return { .. }
                | Self::ReturnUnit { .. }
                | Self::ReturnStructural { .. }
                | Self::Crash { .. }
                | Self::ExternalCompletion { .. }
                | Self::InvokeNoNormal { .. } => Vec::new(),
            }
        }

        pub fn successor_targets(&self) -> Vec<BlockId> {
            self.successor_edges()
                .into_iter()
                .map(|(_, target)| target)
                .collect()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SuccessorEdge {
        pub edge: EdgeId,
        pub target: BlockId,
        pub arguments: Vec<ValueId>,
        pub erased_arguments: Vec<ScalarTerm>,
        pub erased_proof_arguments: Vec<ProofTerm>,
        pub structural_arguments: Vec<StructuralArgument>,
        pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StructuralCaseSuccessorEdge {
        pub edge: EdgeId,
        pub target: BlockId,
        pub case: StructuralCaseId,
        pub payload_fields: Vec<StructuralFieldId>,
        pub structural_arguments: Vec<StructuralArgument>,
        pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
    }

    #[macro_export]
    macro_rules! terminator_return_pair_pattern {
        ($($fields:tt)*) => {
            $crate::Terminator::Return { $($fields)* }
                | $crate::Terminator::ReturnUnit { $($fields)* }
        };
    }

    #[macro_export]
    macro_rules! return_or_return_structural_pattern {
        ($($fields:tt)*) => {
            $crate::Terminator::Return { $($fields)* }
                | $crate::Terminator::ReturnStructural { $($fields)* }
        };
    }
}
pub(crate) mod trapping_integer {

    use semantic_vocabulary::ValueId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TrappingIntegerOperation {
        Add { left: ValueId, right: ValueId },
        Subtract { left: ValueId, right: ValueId },
        Multiply { left: ValueId, right: ValueId },
        Divide { left: ValueId, right: ValueId },
        Remainder { left: ValueId, right: ValueId },
        ShiftLeft { value: ValueId, count: ValueId },
        ShiftRight { value: ValueId, count: ValueId },
        Convert { operand: ValueId },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TrappingIntegerPrimitive {
        Add,
        Subtract,
        Multiply,
        Divide,
        Remainder,
        ShiftLeft,
        ShiftRight,
        Convert,
    }

    impl TrappingIntegerOperation {
        pub const fn primitive(self) -> TrappingIntegerPrimitive {
            match self {
                Self::Add { .. } => TrappingIntegerPrimitive::Add,
                Self::Subtract { .. } => TrappingIntegerPrimitive::Subtract,
                Self::Multiply { .. } => TrappingIntegerPrimitive::Multiply,
                Self::Divide { .. } => TrappingIntegerPrimitive::Divide,
                Self::Remainder { .. } => TrappingIntegerPrimitive::Remainder,
                Self::ShiftLeft { .. } => TrappingIntegerPrimitive::ShiftLeft,
                Self::ShiftRight { .. } => TrappingIntegerPrimitive::ShiftRight,
                Self::Convert { .. } => TrappingIntegerPrimitive::Convert,
            }
        }

        pub fn operands(self) -> Vec<ValueId> {
            match self {
                Self::Add { left, right }
                | Self::Subtract { left, right }
                | Self::Multiply { left, right }
                | Self::Divide { left, right }
                | Self::Remainder { left, right } => vec![left, right],
                Self::ShiftLeft { value, count } | Self::ShiftRight { value, count } => {
                    vec![value, count]
                }
                Self::Convert { operand } => vec![operand],
            }
        }

        pub fn map_operands(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
            match self {
                Self::Add { left, right }
                | Self::Subtract { left, right }
                | Self::Multiply { left, right }
                | Self::Divide { left, right }
                | Self::Remainder { left, right } => {
                    *left = map(*left);
                    *right = map(*right);
                }
                Self::ShiftLeft { value, count } | Self::ShiftRight { value, count } => {
                    *value = map(*value);
                    *count = map(*count);
                }
                Self::Convert { operand } => *operand = map(*operand),
            }
        }
    }

    #[macro_export]
    macro_rules! trapping_arith_set_pattern {
        ($($fields:tt)*) => {
            $crate::TrappingIntegerOperation::Add { $($fields)* }
                | $crate::TrappingIntegerOperation::Divide { $($fields)* }
                | $crate::TrappingIntegerOperation::Multiply { $($fields)* }
                | $crate::TrappingIntegerOperation::Remainder { $($fields)* }
                | $crate::TrappingIntegerOperation::Subtract { $($fields)* }
        };
    }
}

pub use atomic::{AtomicAccessEvent, AtomicReadModifyWrite, FenceOrdering, MemoryOrdering};
pub use float_result_policy::FloatResultPolicy;
pub use machines::{Block, TerminalMachine, structural_parameter_position};
pub use operations::{
    ByteSequenceOperand, ElementViewRegion, Operation, OperationKind, OperationResult,
    ReferenceStorageView, StructuralOperationResult, StructuralResultClaimBinding,
    StructuralStoreDisposition,
};
pub use ranking::{
    TerminalBlockNaturalRank, TerminalIntegerRankValue, TerminalNaturalCycle,
    TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc,
};
pub use record_field::{RecordFieldInitializer, RecordFieldValue};
pub use register_access::RegisterAccessEvent;
pub use scalar_case_field::ScalarCaseField;
pub use target_families::{
    TargetMachineCallable, TargetMachineFamily, TargetMachineFamilyMember, TargetMachineSupply,
};
pub use termination::{CrashCause, StructuralCaseSuccessorEdge, SuccessorEdge, Terminator};
pub use trapping_integer::{TrappingIntegerOperation, TrappingIntegerPrimitive};
