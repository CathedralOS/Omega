/*
  Module, machine, terminator, call, and effect lowering from `TerminalModule`
  into `AbstractOperationPlan` (`lower_decoded_module`, `lower_selected_module`,
  `LoweringError`).

  Consumes the selected native execution scope, scalar applications, and target
  layouts, traverses each reachable machine's blocks in order, and dispatches
  every operation and terminator into source-free abstract operations while
  retaining canonical Terminal identity.
*/
use abstract_operations::AbstractOperationPlan;
use terminal_codec::terminal_psi_identity;
mod error {
    use semantic_vocabulary::{BlockId, MachineId};
    use terminal_codec::CodecError;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum LoweringError {
        InvalidDescriptorTableOperation(semantic_vocabulary::OperationId),
        UnclosedScalarConditions,
        UnclosedScalarApplication(semantic_vocabulary::ScalarApplicationId),
        ScalarApplicationCarrierMismatch(semantic_vocabulary::OperationId),
        UnadmittedCallableScalarApplication {
            operation: semantic_vocabulary::OperationId,
            application: semantic_vocabulary::ScalarApplicationId,
        },
        InvalidActivationStorage(semantic_vocabulary::OperationId),
        UnsupportedReferenceCustody(semantic_vocabulary::OperationId),
        UnsupportedDynamicDispatch(semantic_vocabulary::OperationId),
        InvalidPrimitiveLocalEstablishment(semantic_vocabulary::OperationId),
        InvalidPrimitiveScalarRead(semantic_vocabulary::OperationId),
        InvalidStructuralCaseMembership(semantic_vocabulary::OperationId),
        SemanticIdentity(CodecError),
        UnsupportedPayloadlessCase(semantic_vocabulary::OperationId),
        InvalidStructuralLeafCopy {
            machine: MachineId,
            operation: semantic_vocabulary::OperationId,
            reason: &'static str,
        },
        InvalidOwnedValueTransfer {
            machine: MachineId,
            operation: semantic_vocabulary::OperationId,
            reason: &'static str,
        },
        UnsupportedScalarArray(semantic_vocabulary::OperationId),
        UnsupportedStructuralCase(semantic_vocabulary::OperationId),
        UnsupportedByteSequenceLiteral(semantic_vocabulary::OperationId),
        InvalidByteSequenceConcatenation(semantic_vocabulary::OperationId),
        InvalidStructuralByteSequenceFieldStore(semantic_vocabulary::OperationId),
        InvalidStructuralByteViewFieldStore(semantic_vocabulary::OperationId),
        UnsupportedStructuralByteSequenceFieldRead(semantic_vocabulary::OperationId),
        VerifiedTrappingIntegerMalformed(semantic_vocabulary::OperationId),
        UnsupportedAtomicAccess(semantic_vocabulary::OperationId),
        VerifiedAtomicAccessMalformed(semantic_vocabulary::OperationId),
        UnsupportedAtomicCoherence(semantic_vocabulary::OperationId),
        InvalidByteSequenceLength(semantic_vocabulary::OperationId),
        InvalidByteSequencePredicate(semantic_vocabulary::OperationId),
        InvalidByteSequenceEqual(semantic_vocabulary::OperationId),
        InvalidByteSequenceRead(semantic_vocabulary::OperationId),
        InvalidByteSequenceWrite(semantic_vocabulary::OperationId),
        InvalidByteSequenceSubslice(semantic_vocabulary::OperationId),
        InvalidElementViewEstablishment(semantic_vocabulary::OperationId),
        InvalidElementViewLength(semantic_vocabulary::OperationId),
        InvalidElementViewRead(semantic_vocabulary::OperationId),
        InvalidElementViewSubslice(semantic_vocabulary::OperationId),
        InvalidWriteOnlyPrimitiveStore(semantic_vocabulary::OperationId),
        InvalidStructuralScalarFieldStore {
            machine: MachineId,
            operation: semantic_vocabulary::OperationId,
            reason: &'static str,
        },
        InvalidScalarStructuralField(semantic_vocabulary::OperationId),
        InvalidBorrowedStorageWindow {
            machine: MachineId,
            operation: semantic_vocabulary::OperationId,
            place: semantic_vocabulary::PlaceId,
            reason: String,
        },
        ScalarReturnFromUnitMachine(MachineId),
        UnitReturnFromScalarMachine(MachineId),
        UnsupportedStructuralResult(MachineId),
        UnsupportedStructuralReturn {
            machine: MachineId,
            edge: semantic_vocabulary::EdgeId,
        },
        VerifiedEntryMachineMissing(MachineId),
        VerifiedBlockMissing {
            machine: MachineId,
            block: BlockId,
        },
        VerifiedJumpArityMismatch {
            edge: semantic_vocabulary::EdgeId,
        },
        VerifiedWrappingAddMalformed(semantic_vocabulary::OperationId),
        VerifiedSaturatingAddMalformed(semantic_vocabulary::OperationId),
        VerifiedWrappingSubtractMalformed(semantic_vocabulary::OperationId),
        VerifiedSaturatingSubtractMalformed(semantic_vocabulary::OperationId),
        VerifiedWrappingMultiplyMalformed(semantic_vocabulary::OperationId),
        VerifiedExactDivideMalformed(semantic_vocabulary::OperationId),
        VerifiedExactRemainderMalformed(semantic_vocabulary::OperationId),
        VerifiedWrappingDivideMalformed(semantic_vocabulary::OperationId),
        VerifiedWrappingRemainderMalformed(semantic_vocabulary::OperationId),
        VerifiedSaturatingDivideMalformed(semantic_vocabulary::OperationId),
        VerifiedSaturatingRemainderMalformed(semantic_vocabulary::OperationId),
        VerifiedSaturatingMultiplyMalformed(semantic_vocabulary::OperationId),
        VerifiedIntegerBitwiseMalformed(semantic_vocabulary::OperationId),
        VerifiedIntegerWidenMalformed(semantic_vocabulary::OperationId),
        VerifiedScalarRepresentationViewMalformed(semantic_vocabulary::OperationId),
        VerifiedIntegerExactCastMalformed(semantic_vocabulary::OperationId),
        VerifiedWrappingShiftMalformed(semantic_vocabulary::OperationId),
        VerifiedExactShiftMalformed(semantic_vocabulary::OperationId),
        VerifiedIeeeFloatMalformed(semantic_vocabulary::OperationId),
    }

    diagnostics::debug_display!(LoweringError);

    impl std::error::Error for LoweringError {}
}
#[path = "."]
mod machine {
    use crate::lowering::LoweringError;
    use semantic_vocabulary::OperationId;
    #[path = "."]
    mod operation {
        #[path = "lowering_scalars.rs"]
        mod scalars;
        pub(super) use scalars::{
            arithmetic, boolean, ieee_float, integer_bitwise, integer_constants_and_relations,
            integer_conversion, scalar_applications, shifts, trapping_integer,
        };
        #[path = "lowering_storage.rs"]
        mod storage;
        pub(super) use storage::{
            atomic_events, borrowed_windows, primitive_projection, primitive_storage,
            structural_establishment, structural_leaf_copy, structural_scalar_fields,
        };
        mod calls {
            use abstract_operations::{
                AbstractCallTarget, AbstractOperation, AbstractStructuralCallResult,
                CompletionClaimSource,
            };
            use terminal_psi::{Operation, OperationKind, TerminalMachine};

            use crate::lowering::LoweringError;

            pub(super) fn lower(
                operation: &Operation,
                machine: &TerminalMachine,
            ) -> Result<AbstractOperation, LoweringError> {
                Ok(match operation.kind.clone() {
                    OperationKind::SelectMachine { descriptor } => {
                        let result = operation
                            .result
                            .scalar()
                            .filter(|result| {
                                matches!(
                                    result.scalar_type,
                                    semantic_vocabulary::ScalarType::Callable(_)
                                )
                            })
                            .ok_or(LoweringError::UnsupportedDynamicDispatch(operation.id))?;
                        AbstractOperation::SelectMachine {
                            psi_operation: operation.id,
                            result: result.into(),
                            descriptor,
                        }
                    }
                    OperationKind::SelectImplementation { table } => {
                        let result = operation.result.scalar()
                .filter(|result| matches!(result.scalar_type, semantic_vocabulary::ScalarType::Integer(integer)
                    if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64))
                .ok_or(LoweringError::UnsupportedDynamicDispatch(operation.id))?;
                        AbstractOperation::SelectImplementation {
                            psi_operation: operation.id,
                            result: result.into(),
                            table,
                        }
                    }
                    OperationKind::CallMachine {
                        callee,
                        contract,
                        arguments,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        ..
                    } => AbstractOperation::CallStructural {
                        psi_operation: operation.id,
                        callee: AbstractCallTarget::Callable { callee, contract },
                        arguments,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        result: match &operation.result {
                            terminal_psi::OperationResult::Unit => {
                                AbstractStructuralCallResult::Unit
                            }
                            terminal_psi::OperationResult::Scalar(result) => {
                                AbstractStructuralCallResult::Scalar((*result).into())
                            }
                            terminal_psi::OperationResult::Structural(result) => {
                                AbstractStructuralCallResult::Structural {
                                    result: result.clone(),
                                    selected_evidence: Vec::new(),
                                }
                            }
                        },
                    },
                    OperationKind::CallDynamic {
                        selector,
                        interface,
                        slot,
                        arguments,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        ..
                    } => AbstractOperation::CallStructural {
                        psi_operation: operation.id,
                        callee: AbstractCallTarget::Dynamic {
                            selector,
                            interface,
                            slot,
                        },
                        arguments,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        result: match &operation.result {
                            terminal_psi::OperationResult::Unit => {
                                AbstractStructuralCallResult::Unit
                            }
                            terminal_psi::OperationResult::Scalar(result) => {
                                AbstractStructuralCallResult::Scalar((*result).into())
                            }
                            terminal_psi::OperationResult::Structural(result) => {
                                AbstractStructuralCallResult::Structural {
                                    result: result.clone(),
                                    selected_evidence: Vec::new(),
                                }
                            }
                        },
                    },
                    OperationKind::CallUnit {
                        callee,
                        arguments,
                        erased_arguments: _,
                        erased_proof_arguments: _,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                    } => AbstractOperation::CallStructural {
                        psi_operation: operation.id,
                        callee: callee.into(),
                        arguments,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        result: AbstractStructuralCallResult::Unit,
                    },
                    OperationKind::CallStructuralScalar {
                        callee,
                        arguments,
                        erased_arguments: _,
                        erased_proof_arguments: _,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                    } => {
                        let result = operation.result.expect_scalar();
                        AbstractOperation::CallStructural {
                            psi_operation: operation.id,
                            result: AbstractStructuralCallResult::Scalar(result.into()),
                            callee: callee.into(),
                            arguments,
                            structural_arguments,
                            claim_transfers,
                            requirement_obligations,
                            crash_continuations,
                        }
                    }
                    OperationKind::CallStructural {
                        callee,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        selected_evidence,
                    }
                    | OperationKind::CallStructuralWithScalarArguments {
                        callee,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        selected_evidence,
                        ..
                    } => {
                        let Some(result) = operation.result.structural().cloned() else {
                            return Err(LoweringError::UnsupportedStructuralResult(machine.id));
                        };
                        let arguments = match &operation.kind {
                            OperationKind::CallStructuralWithScalarArguments {
                                arguments, ..
                            } => arguments.clone(),
                            _ => Vec::new(),
                        };
                        AbstractOperation::CallStructural {
                            psi_operation: operation.id,
                            result: AbstractStructuralCallResult::Structural {
                                result,
                                selected_evidence,
                            },
                            callee: callee.into(),
                            arguments,
                            structural_arguments,
                            claim_transfers,
                            requirement_obligations,
                            crash_continuations,
                        }
                    }
                    OperationKind::BoundaryCall {
                        boundary,
                        arguments,
                        structural_arguments,
                        completion_receipts,
                        ..
                    } => {
                        let mut completion_claim_sources = machine
                            .entry_claims
                            .iter()
                            .cloned()
                            .map(|entry| CompletionClaimSource {
                                claim: entry.claim,
                                entry: Some(entry),
                                content: None,
                            })
                            .collect::<Vec<_>>();
                        for content in &machine.content_entry_claims {
                            if let Some(source) = completion_claim_sources
                                .iter_mut()
                                .find(|source| source.claim == content.claim)
                            {
                                source.content = Some(content.clone());
                            } else {
                                completion_claim_sources.push(CompletionClaimSource {
                                    claim: content.claim,
                                    entry: None,
                                    content: Some(content.clone()),
                                });
                            }
                        }
                        completion_claim_sources.sort();
                        AbstractOperation::BoundaryCall {
                            psi_operation: operation.id,
                            result: match &operation.result {
                                terminal_psi::OperationResult::Unit => {
                                    abstract_operations::AbstractBoundaryResult::Unit
                                }
                                terminal_psi::OperationResult::Scalar(result) => {
                                    abstract_operations::AbstractBoundaryResult::Scalar(
                                        (*result).into(),
                                    )
                                }
                                terminal_psi::OperationResult::Structural(result) => {
                                    abstract_operations::AbstractBoundaryResult::Structural(
                                        result.clone(),
                                    )
                                }
                            },
                            boundary,
                            arguments,
                            structural_arguments,
                            completion_claim_sources,
                            completion_receipts,
                        }
                    }
                    OperationKind::Call {
                        callee,
                        arguments,
                        erased_arguments: _,
                        erased_proof_arguments: _,
                        requirement_obligations,
                        crash_continuations,
                    } => AbstractOperation::Call {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type: operation.result.expect_scalar().scalar_type,
                        callee,
                        arguments,
                        requirement_obligations,
                        crash_continuations,
                    },
                    _ => unreachable!("call router is exhaustive"),
                })
            }
        }
        mod effects {
            use std::collections::BTreeMap;

            use abstract_operations::{AbstractOperation, AbstractResult};
            use semantic_vocabulary::ScalarType;
            use terminal_psi::{
                Operation, OperationKind, StructuralTypeDeclaration, TerminalMachine,
            };

            use crate::lowering::LoweringError;

            pub(super) fn lower(
                operation: &Operation,
                machine: &TerminalMachine,
                structural_types: &[StructuralTypeDeclaration],
                value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
            ) -> Result<AbstractOperation, LoweringError> {
                Ok(match operation.kind.clone() {
                    OperationKind::PortWrite {
                        service,
                        port,
                        value,
                    } => AbstractOperation::PortWrite {
                        psi_operation: operation.id,
                        service,
                        port,
                        value,
                    },
                    OperationKind::ActivateTaskRegister {
                        service,
                        entries,
                        gdt,
                        task_state,
                        selector,
                        scratch,
                    } => {
                        let Some(result) = operation.result.scalar() else {
                            return Err(LoweringError::InvalidDescriptorTableOperation(
                                operation.id,
                            ));
                        };
                        AbstractOperation::ActivateTaskRegister {
                            psi_operation: operation.id,
                            service,
                            result: result.into(),
                            entries,
                            gdt,
                            task_state,
                            selector,
                            scratch,
                        }
                    }
                    OperationKind::ReadTaskRegister { service } => {
                        let Some(result) = operation.result.scalar() else {
                            return Err(LoweringError::InvalidDescriptorTableOperation(
                                operation.id,
                            ));
                        };
                        AbstractOperation::ReadTaskRegister {
                            psi_operation: operation.id,
                            service,
                            result: result.into(),
                        }
                    }
                    OperationKind::ReloadSegmentDescriptors {
                        service,
                        entries,
                        gdt,
                        kind,
                        selector,
                        scratch,
                    } => {
                        let Some(result) = operation.result.scalar() else {
                            return Err(LoweringError::InvalidDescriptorTableOperation(
                                operation.id,
                            ));
                        };
                        AbstractOperation::ReloadSegmentDescriptors {
                            psi_operation: operation.id,
                            service,
                            result: result.into(),
                            entries,
                            gdt,
                            kind,
                            selector,
                            scratch,
                        }
                    }
                    OperationKind::ReadSegmentSelector { service, register } => {
                        let Some(result) = operation.result.scalar() else {
                            return Err(LoweringError::InvalidDescriptorTableOperation(
                                operation.id,
                            ));
                        };
                        AbstractOperation::ReadSegmentSelector {
                            psi_operation: operation.id,
                            service,
                            register,
                            result: result.into(),
                        }
                    }
                    OperationKind::GlobalDescriptorTable {
                        service,
                        operation: direction,
                        source,
                    } => AbstractOperation::GlobalDescriptorTable {
                        psi_operation: operation.id,
                        service,
                        operation: direction,
                        source,
                    },
                    OperationKind::RegisterAccess { service, event } => {
                        AbstractOperation::RegisterAccess {
                            psi_operation: operation.id,
                            service,
                            event,
                            result: operation.result.scalar().map(Into::into),
                        }
                    }
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination,
                        path,
                        value,
                        range_obligation,
                    } => {
                        let invalid =
                            || LoweringError::InvalidWriteOnlyPrimitiveStore(operation.id);
                        let destination = super::structural_scalar_fields::store_destination(
                            machine,
                            destination,
                        )
                        .or_else(|| {
                            super::structural_scalar_fields::owned_storage_destination(
                                machine,
                                destination,
                            )
                        })
                        .ok_or_else(invalid)?;
                        let Some(scalar_type) = value_types.get(&value).copied() else {
                            return Err(invalid());
                        };
                        let valid_destination = destination.parameter().is_none_or(|parameter| {
                            matches!(
                                parameter.access,
                                terminal_psi::StructuralAccess::Owned
                                    | terminal_psi::StructuralAccess::MutableBorrow
                                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
                            )
                        }) && (destination.multiplicity()
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                            || (!path.is_empty()
                                && destination.multiplicity()
                                    == terminal_psi::StructuralMultiplicity::Affine))
                            && machine
                                .entry_claims
                                .iter()
                                .all(|claim| claim.input != destination.place())
                            && machine
                                .content_entry_claims
                                .iter()
                                .all(|claim| claim.input.root != destination.place());
                        if !valid_destination {
                            return Err(invalid());
                        }
                        if super::primitive_projection::leaf_type(
                            structural_types,
                            destination.structural_type(),
                            &path,
                            |index| value_types.get(&index).copied(),
                        ) != Some(scalar_type)
                        {
                            return Err(invalid());
                        }
                        AbstractOperation::WriteOnlyPrimitiveStore {
                            psi_operation: operation.id,
                            destination,
                            path,
                            value: AbstractResult { value, scalar_type },
                            range_obligation,
                        }
                    }
                    _ => unreachable!("effect router is exhaustive"),
                })
            }
        }
        mod routing {
            use std::collections::BTreeMap;

            use abstract_operations::AbstractOperation;
            use terminal_psi::{OperationKind, TerminalMachine};

            use super::{
                LoweredAffineLocal, ScalarType, StructuralLiteral, arithmetic, atomic_events,
                boolean, borrowed_windows, calls, effects, ieee_float, integer_bitwise,
                integer_constants_and_relations, integer_conversion, shifts,
                structural_establishment, structural_leaf_copy, structural_result,
                structural_scalar_fields, trapping_integer, unsigned_scalar,
            };
            use crate::lowering::LoweringError;

            #[allow(clippy::too_many_arguments)]
            pub(super) fn lower(
                operation: &terminal_psi::Operation,
                block: &terminal_psi::Block,
                machine: &TerminalMachine,
                module: &terminal_psi::TerminalModule,
                structural_types: &[terminal_psi::StructuralTypeDeclaration],
                scalar_applications: Option<&terminal_installation::AdmittedScalarApplications>,
                access_view_plans: &[terminal_psi::TerminalAccessViewPlan],
                value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
                byte_sequence_literals: &[StructuralLiteral<'_>],
                unit_affine_locals: &[StructuralLiteral<'_>],
                lowered_unit_affine_locals: &mut Vec<LoweredAffineLocal>,
            ) -> Result<AbstractOperation, LoweringError> {
                match &operation.kind {
                    OperationKind::ReadScalarApplication { application } => {
                        super::scalar_applications::lower(
                            operation,
                            *application,
                            scalar_applications,
                        )
                    }
                    OperationKind::EstablishActivationClaim {
                        bound,
                        committed,
                        backing_type,
                        established_case,
                        rejected_case,
                    } => {
                        terminal_semantics::activation_storage::activation_claim_shape(
                            module, operation,
                        )
                        .map_err(|_| LoweringError::InvalidActivationStorage(operation.id))?;
                        let result =
                            structural_result(operation, LoweringError::InvalidActivationStorage)?;
                        Ok(AbstractOperation::EstablishActivationClaim {
                            psi_operation: operation.id,
                            result,
                            bound: *bound,
                            committed: *committed,
                            backing_type: *backing_type,
                            established_case: *established_case,
                            rejected_case: *rejected_case,
                        })
                    }
                    OperationKind::ReleaseActivationClaim { source, claim } => {
                        Ok(AbstractOperation::ReleaseActivationClaim {
                            psi_operation: operation.id,
                            source: source.clone(),
                            claim: *claim,
                        })
                    }
                    OperationKind::SplitActivationClaim {
                        source,
                        claim,
                        bound_at,
                        committed_at,
                        covered_case,
                        rejected_case,
                    } => {
                        let result =
                            structural_result(operation, LoweringError::InvalidActivationStorage)?;
                        Ok(AbstractOperation::SplitActivationClaim {
                            psi_operation: operation.id,
                            result,
                            source: source.clone(),
                            claim: *claim,
                            bound_at: *bound_at,
                            committed_at: *committed_at,
                            covered_case: *covered_case,
                            rejected_case: *rejected_case,
                        })
                    }
                    OperationKind::MergeActivationClaim {
                        lower,
                        upper,
                        lower_claim,
                        upper_claim,
                        covered_case,
                        rejected_case,
                    } => {
                        let result =
                            structural_result(operation, LoweringError::InvalidActivationStorage)?;
                        Ok(AbstractOperation::MergeActivationClaim {
                            psi_operation: operation.id,
                            result,
                            lower: lower.clone(),
                            upper: upper.clone(),
                            lower_claim: *lower_claim,
                            upper_claim: *upper_claim,
                            covered_case: *covered_case,
                            rejected_case: *rejected_case,
                        })
                    }
                    OperationKind::EstablishAccessReference { source, resident } => {
                        let Some(result) = operation.result.structural().cloned() else {
                            return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
                        };
                        let Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) =
                            structural_types
                                .iter()
                                .find(|row| row.id == result.structural_type)
                                .map(|row| &row.shape)
                        else {
                            return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
                        };
                        let mut selected = access_view_plans
                            .iter()
                            .filter(|plan| plan.carrier == *referent);
                        let (Some(plan), None) = (selected.next(), selected.next()) else {
                            return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
                        };
                        let correspondence = if let Some(roles) = resident {
                            let root = super::primitive_storage::parameter(machine, source.place)
                                .map(|parameter| parameter.structural_type)
                                .or_else(|| {
                                    machine
                                        .structural_place(source.place)?
                                        .kind
                                        .produced_structural_type()
                                })
                                .ok_or(LoweringError::UnsupportedReferenceCustody(operation.id))?;
                            let owner = terminal_semantics::runtime_structural_path_tip(
                                structural_types.iter(),
                                root,
                                &source.path,
                            )
                            .ok_or(LoweringError::UnsupportedReferenceCustody(operation.id))?;
                            abstract_operations::AccessReferenceBacking::Resident(
                                abstract_operations::ResidentReferenceBacking {
                                    owner: owner.id,
                                    schema: plan.schema,
                                    carrier: plan.carrier,
                                    roles: *roles,
                                    placement_commitment: plan
                                        .placement
                                        .content_interpretation()
                                        .commitment(),
                                },
                            )
                        } else {
                            abstract_operations::AccessReferenceBacking::OrdinaryRam(
                                terminal_semantics::access_ram_plan_correspondence(
                                    structural_types,
                                    access_view_plans,
                                    plan,
                                    plan.schema,
                                )
                                .map_err(|_| {
                                    LoweringError::UnsupportedReferenceCustody(operation.id)
                                })?,
                            )
                        };
                        Ok(AbstractOperation::EstablishAccessReference {
                            psi_operation: operation.id,
                            result,
                            source: source.clone(),
                            correspondence,
                        })
                    }
                    OperationKind::SelectImplementation { .. }
                    | OperationKind::SelectMachine { .. }
                    | OperationKind::CallDynamic { .. }
                    | OperationKind::CallMachine { .. } => calls::lower(operation, machine),
                    OperationKind::StorageAddress { source } => {
                        let result = operation.result.scalar()
                .filter(|result| matches!(result.scalar_type, ScalarType::Integer(integer) if integer.is_address()))
                .ok_or(LoweringError::UnsupportedReferenceCustody(operation.id))?;
                        if source.access != terminal_psi::StructuralAccess::SharedBorrow {
                            return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
                        }
                        let root = super::primitive_storage::parameter(machine, source.place)
                            .map(|parameter| parameter.structural_type)
                            .or_else(|| {
                                machine
                                    .structural_place(source.place)?
                                    .kind
                                    .produced_structural_type()
                            })
                            .ok_or(LoweringError::UnsupportedReferenceCustody(operation.id))?;
                        if terminal_semantics::primitive_read_projection_type(
                            structural_types.iter(),
                            root,
                            &source.path,
                        )
                        .is_none()
                            && terminal_semantics::runtime_structural_path_tip(
                                structural_types.iter(),
                                root,
                                &source.path,
                            )
                            .is_none()
                        {
                            return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
                        }
                        Ok(AbstractOperation::StorageAddress {
                            psi_operation: operation.id,
                            result: result.into(),
                            source: source.place,
                            path: source.path.clone(),
                        })
                    }
                    OperationKind::EstablishReference { source, storage } => {
                        let Some(result) = operation.result.structural().cloned() else {
                            return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
                        };
                        Ok(AbstractOperation::EstablishReference {
                            psi_operation: operation.id,
                            result,
                            source: source.clone(),
                            storage: *storage,
                        })
                    }
                    OperationKind::ReleaseReference { source, path } => {
                        Ok(AbstractOperation::ReleaseReference {
                            psi_operation: operation.id,
                            source: *source,
                            path: path.clone(),
                        })
                    }
                    OperationKind::StructuralCaseMembership { source, path, case } => {
                        let result = operation
                            .result
                            .scalar()
                            .filter(|result| result.scalar_type == ScalarType::Boolean)
                            .ok_or(LoweringError::InvalidStructuralCaseMembership(operation.id))?;
                        Ok(AbstractOperation::StructuralCaseMembership {
                            psi_operation: operation.id,
                            result: result.into(),
                            source: *source,
                            path: path.clone(),
                            case: *case,
                        })
                    }
                    OperationKind::EstablishPrimitiveLocal { .. }
                    | OperationKind::PrimitiveScalarRead { .. } => super::primitive_storage::lower(
                        operation,
                        machine,
                        structural_types,
                        value_types,
                    ),
                    OperationKind::StructuralByteSequenceFieldLength {
                        source,
                        path,
                        field,
                    } => {
                        let result = unsigned_scalar(
                            operation,
                            64,
                            LoweringError::InvalidByteSequenceLength,
                        )?;
                        Ok(AbstractOperation::StructuralByteSequenceFieldLength {
                            psi_operation: operation.id,
                            result: result.into(),
                            source: *source,
                            path: path.clone(),
                            field: *field,
                        })
                    }
                    OperationKind::StructuralByteSequenceFieldRead {
                        source,
                        path,
                        field,
                        index,
                        length,
                        obligation,
                    } => {
                        let result = unsigned_scalar(
                            operation,
                            8,
                            LoweringError::UnsupportedStructuralByteSequenceFieldRead,
                        )?;
                        Ok(AbstractOperation::StructuralByteSequenceFieldRead {
                            psi_operation: operation.id,
                            result: result.into(),
                            source: *source,
                            path: path.clone(),
                            field: *field,
                            index: *index,
                            length: *length,
                            obligation: *obligation,
                        })
                    }
                    OperationKind::ByteSequencePredicate { operand, predicate } => {
                        let result = operation
                            .result
                            .scalar()
                            .filter(|result| result.scalar_type == ScalarType::Boolean)
                            .ok_or(LoweringError::InvalidByteSequencePredicate(operation.id))?;
                        Ok(AbstractOperation::ByteSequencePredicate {
                            psi_operation: operation.id,
                            result: result.into(),
                            operand: operand.clone(),
                            predicate: *predicate,
                        })
                    }
                    OperationKind::ByteSequenceEqual { left, right } => {
                        let result = operation
                            .result
                            .scalar()
                            .filter(|result| result.scalar_type == ScalarType::Boolean)
                            .ok_or(LoweringError::InvalidByteSequenceEqual(operation.id))?;
                        Ok(AbstractOperation::ByteSequenceEqual {
                            psi_operation: operation.id,
                            result: result.into(),
                            left: left.clone(),
                            right: right.clone(),
                        })
                    }
                    OperationKind::StructuralByteSequenceFieldByteStore {
                        destination,
                        path,
                        field,
                        index,
                        value,
                        length,
                        obligation,
                    } => {
                        if operation.result != terminal_psi::OperationResult::Unit {
                            return Err(LoweringError::InvalidStructuralByteSequenceFieldStore(
                                operation.id,
                            ));
                        }
                        Ok(AbstractOperation::StructuralByteSequenceFieldByteStore {
                            psi_operation: operation.id,
                            destination: *destination,
                            path: path.clone(),
                            field: *field,
                            index: *index,
                            value: *value,
                            length: *length,
                            obligation: *obligation,
                        })
                    }
                    OperationKind::ByteSequenceWrite {
                        destination,
                        index,
                        value,
                        length,
                        obligation,
                    } => {
                        if operation.result != terminal_psi::OperationResult::Unit {
                            return Err(LoweringError::InvalidByteSequenceWrite(operation.id));
                        }
                        Ok(AbstractOperation::ByteSequenceWrite {
                            psi_operation: operation.id,
                            destination: *destination,
                            index: *index,
                            value: *value,
                            length: *length,
                            obligation: *obligation,
                        })
                    }
                    OperationKind::ByteSequenceStore {
                        destination,
                        source,
                        length,
                        obligation,
                    } => {
                        if operation.result != terminal_psi::OperationResult::Unit {
                            return Err(LoweringError::InvalidStructuralByteSequenceFieldStore(
                                operation.id,
                            ));
                        }
                        Ok(AbstractOperation::ByteSequenceStore {
                            psi_operation: operation.id,
                            destination: *destination,
                            source: *source,
                            length: *length,
                            obligation: *obligation,
                        })
                    }
                    OperationKind::StructuralByteViewFieldStore {
                        destination,
                        path,
                        field,
                        source,
                        predicate_obligation,
                    } => {
                        if operation.result != terminal_psi::OperationResult::Unit {
                            return Err(LoweringError::InvalidStructuralByteViewFieldStore(
                                operation.id,
                            ));
                        }
                        Ok(AbstractOperation::StructuralByteViewFieldStore {
                            psi_operation: operation.id,
                            destination: *destination,
                            path: path.clone(),
                            field: *field,
                            source: *source,
                            predicate_obligation: *predicate_obligation,
                        })
                    }
                    OperationKind::StructuralByteSequenceFieldStore {
                        destination,
                        path,
                        field,
                        source,
                        length,
                        obligation,
                    } => {
                        if operation.result != terminal_psi::OperationResult::Unit {
                            return Err(LoweringError::InvalidStructuralByteSequenceFieldStore(
                                operation.id,
                            ));
                        }
                        Ok(AbstractOperation::StructuralByteSequenceFieldStore {
                            psi_operation: operation.id,
                            destination: *destination,
                            path: path.clone(),
                            field: *field,
                            source: *source,
                            length: *length,
                            obligation: *obligation,
                        })
                    }
                    OperationKind::ByteSequenceRead {
                        source,
                        index,
                        length,
                        obligation,
                    } => {
                        let result =
                            unsigned_scalar(operation, 8, LoweringError::InvalidByteSequenceRead)?;
                        Ok(AbstractOperation::ByteSequenceRead {
                            psi_operation: operation.id,
                            result: result.into(),
                            source: *source,
                            index: *index,
                            length: *length,
                            obligation: *obligation,
                        })
                    }
                    OperationKind::ByteSequenceSubslice {
                        source,
                        start,
                        end,
                        length,
                        obligation,
                    } => {
                        let result = operation
                            .result
                            .structural()
                            .ok_or(LoweringError::InvalidByteSequenceSubslice(operation.id))?;
                        Ok(AbstractOperation::ByteSequenceSubslice {
                            psi_operation: operation.id,
                            result: result.clone(),
                            source: source.clone(),
                            start: *start,
                            end: *end,
                            length: *length,
                            obligation: *obligation,
                        })
                    }
                    OperationKind::ByteSequenceLength { source } => {
                        let result = unsigned_scalar(
                            operation,
                            64,
                            LoweringError::InvalidByteSequenceLength,
                        )?;
                        Ok(AbstractOperation::ByteSequenceLength {
                            psi_operation: operation.id,
                            result: result.into(),
                            source: source.clone(),
                        })
                    }
                    terminal_psi::element_view_establish_pair_pattern!(
                        destination,
                        source,
                        element,
                        region,
                    ) => {
                        let result = structural_result(
                            operation,
                            LoweringError::InvalidElementViewEstablishment,
                        )?;
                        if result.place != *destination {
                            return Err(LoweringError::InvalidElementViewEstablishment(
                                operation.id,
                            ));
                        }
                        Ok(AbstractOperation::EstablishElementView {
                            psi_operation: operation.id,
                            result,
                            destination: *destination,
                            source: source.clone(),
                            element: *element,
                            region: *region,
                        })
                    }
                    OperationKind::ElementViewLength { source } => {
                        let result = unsigned_scalar(
                            operation,
                            64,
                            LoweringError::InvalidElementViewLength,
                        )?;
                        Ok(AbstractOperation::ElementViewLength {
                            psi_operation: operation.id,
                            result: result.into(),
                            source: source.clone(),
                        })
                    }
                    OperationKind::ElementViewRead {
                        source,
                        index,
                        length,
                        obligation,
                        path,
                    } => {
                        let result = operation
                            .result
                            .scalar()
                            .ok_or(LoweringError::InvalidElementViewRead(operation.id))?;
                        Ok(AbstractOperation::ElementViewRead {
                            psi_operation: operation.id,
                            result: result.into(),
                            source: *source,
                            index: *index,
                            length: *length,
                            obligation: *obligation,
                            path: path.clone(),
                        })
                    }
                    OperationKind::ElementViewWrite {
                        path,
                        destination,
                        index,
                        value,
                        length,
                        obligation,
                    } => Ok(AbstractOperation::ElementViewWrite {
                        path: path.clone(),
                        psi_operation: operation.id,
                        destination: destination.clone(),
                        index: *index,
                        value: *value,
                        length: *length,
                        obligation: *obligation,
                    }),
                    OperationKind::ElementViewSubslice {
                        source,
                        start,
                        end,
                        length,
                        obligation,
                    } => {
                        let result = operation
                            .result
                            .structural()
                            .ok_or(LoweringError::InvalidElementViewSubslice(operation.id))?;
                        Ok(AbstractOperation::ElementViewSubslice {
                            psi_operation: operation.id,
                            result: result.clone(),
                            source: source.clone(),
                            start: *start,
                            end: *end,
                            length: *length,
                            obligation: *obligation,
                        })
                    }
                    OperationKind::EstablishScalarArray { .. }
                    | OperationKind::EstablishStructuralArray { .. }
                    | OperationKind::EstablishScalarCase { .. }
                    | OperationKind::EstablishStructuralCase { .. }
                    | OperationKind::EstablishNativeCodeLayout { .. }
                    | OperationKind::EstablishByteSequenceLiteral { .. }
                    | OperationKind::EstablishByteSequenceConcatenation { .. }
                    | OperationKind::EstablishTrivialAffineLocal { .. }
                    | OperationKind::EstablishZeroedStructural { .. }
                    | OperationKind::EstablishStructuralLocal { .. }
                    | OperationKind::EstablishRecord { .. } => structural_establishment::lower(
                        operation,
                        block,
                        machine,
                        structural_types,
                        byte_sequence_literals,
                        unit_affine_locals,
                        lowered_unit_affine_locals,
                    ),
                    OperationKind::CallUnit { .. }
                    | OperationKind::CallStructuralScalar { .. }
                    | terminal_psi::call_structural_pair_pattern!(..)
                    | OperationKind::BoundaryCall { .. }
                    | OperationKind::Call { .. } => calls::lower(operation, machine),
                    OperationKind::PortWrite { .. }
                    | OperationKind::RegisterAccess { .. }
                    | OperationKind::ActivateTaskRegister { .. }
                    | OperationKind::ReadTaskRegister { .. }
                    | OperationKind::ReloadSegmentDescriptors { .. }
                    | OperationKind::ReadSegmentSelector { .. }
                    | OperationKind::GlobalDescriptorTable { .. }
                    | OperationKind::WriteOnlyPrimitiveStore { .. } => {
                        effects::lower(operation, machine, structural_types, value_types)
                    }
                    OperationKind::StructuralScalarFieldStore { .. }
                    | OperationKind::ScalarStructuralField { .. } => {
                        structural_scalar_fields::lower(
                            operation,
                            machine,
                            structural_types,
                            module,
                            value_types,
                        )
                    }
                    terminal_psi::move_or_store_structural_field_pattern!() => {
                        borrowed_windows::lower(operation, machine, structural_types)
                    }
                    OperationKind::TransformContent {
                        source,
                        parents,
                        derivations,
                    } => {
                        let result = operation.result.structural().ok_or(
                            LoweringError::InvalidOwnedValueTransfer {
                                machine: machine.id,
                                operation: operation.id,
                                reason: "content transformation has no structural result",
                            },
                        )?;
                        Ok(AbstractOperation::TransformContent {
                            psi_operation: operation.id,
                            result: result.clone(),
                            source: source.clone(),
                            parents: parents.clone(),
                            derivations: derivations.clone(),
                        })
                    }
                    OperationKind::MoveStructuralValue { source } => {
                        let result = operation.result.structural().ok_or(
                            LoweringError::InvalidOwnedValueTransfer {
                                machine: machine.id,
                                operation: operation.id,
                                reason: "owned transfer has no structural result",
                            },
                        )?;
                        Ok(AbstractOperation::MoveStructuralValue {
                            psi_operation: operation.id,
                            result: result.clone(),
                            source: source.clone(),
                        })
                    }
                    OperationKind::StructuralLeafCopy { source, path } => {
                        structural_leaf_copy::lower(
                            operation,
                            machine,
                            module,
                            structural_types,
                            *source,
                            abstract_operations::StructuralCopyPath::Ordinary(path.clone()),
                        )
                    }
                    OperationKind::StructuralCaseLeafCopy { source, path } => {
                        structural_leaf_copy::lower(
                            operation,
                            machine,
                            module,
                            structural_types,
                            *source,
                            abstract_operations::StructuralCopyPath::Case(path.clone()),
                        )
                    }
                    OperationKind::IntegerConstant { .. } => {
                        integer_constants_and_relations::lower(operation)
                    }
                    OperationKind::IeeeFloatCompare { .. }
                    | OperationKind::FloatResultPolicy { .. }
                    | OperationKind::IeeeFloatConstant { .. }
                    | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. } => {
                        ieee_float::lower(operation, value_types)
                    }
                    OperationKind::BooleanConstant { .. }
                    | OperationKind::BooleanStructuralField { .. }
                    | OperationKind::BooleanNot { .. }
                    | OperationKind::BooleanEqual { .. } => boolean::lower(operation),
                    OperationKind::IntegerEqual { .. }
                    | terminal_psi::integer_less_or_equal_pair_pattern!(..) => {
                        integer_constants_and_relations::lower(operation)
                    }
                    OperationKind::IntegerBitwiseNot { .. } => integer_bitwise::lower(operation),
                    OperationKind::IntegerWiden { .. }
                    | OperationKind::IntegerExactCast { .. }
                    | OperationKind::ScalarRepresentationView { .. } => {
                        integer_conversion::lower(operation, value_types)
                    }
                    OperationKind::IntegerBitwiseAnd { .. }
                    | OperationKind::IntegerBitwiseOr { .. }
                    | OperationKind::IntegerBitwiseXor { .. } => integer_bitwise::lower(operation),
                    OperationKind::WrappingIntegerShiftLeft { .. }
                    | OperationKind::WrappingIntegerShiftRight { .. }
                    | OperationKind::ExactIntegerShiftLeft { .. }
                    | OperationKind::ExactIntegerShiftRight { .. } => {
                        shifts::lower(operation, value_types)
                    }
                    terminal_psi::integer_arithmetic_operations!() => arithmetic::lower(operation),
                    OperationKind::TrappingInteger { .. } => {
                        trapping_integer::lower(operation, value_types)
                    }
                    OperationKind::AtomicFence { ordering } => {
                        Ok(abstract_operations::AbstractOperation::AtomicEvent {
                            psi_operation: operation.id,
                            event: abstract_operations::AbstractAtomicEvent::Fence {
                                ordering: *ordering,
                            },
                            resident_obligation: None,
                            reads_from: None,
                            modification_after: None,
                        })
                    }
                    OperationKind::AtomicAccess { .. } => {
                        atomic_events::lower(operation, machine, structural_types)
                    }
                }
            }
        }

        use std::collections::BTreeMap;

        use abstract_operations::AbstractOperation;
        use terminal_psi::TerminalMachine;

        use super::{LoweredAffineLocal, StructuralLiteral};
        use crate::lowering::LoweringError;
        use semantic_vocabulary::{IntegerSign, IntegerType, OperationId, ScalarType};

        pub(super) fn unique_row<T>(
            rows: impl IntoIterator<Item = T>,
            mut accepts: impl FnMut(&T) -> bool,
        ) -> Option<T> {
            let mut matches = rows.into_iter().filter(|row| accepts(row));
            let row = matches.next()?;
            matches.next().is_none().then_some(row)
        }

        fn unsigned_scalar(
            operation: &terminal_psi::Operation,
            bits: u16,
            invalid: fn(OperationId) -> LoweringError,
        ) -> Result<terminal_psi::ValueDeclaration, LoweringError> {
            let result = operation.result.scalar().ok_or(invalid(operation.id))?;
            if result.scalar_type.as_integer() != IntegerType::new(IntegerSign::Unsigned, bits).ok()
            {
                return Err(invalid(operation.id));
            }
            Ok(result)
        }

        fn structural_result(
            operation: &terminal_psi::Operation,
            invalid: fn(OperationId) -> LoweringError,
        ) -> Result<terminal_psi::StructuralOperationResult, LoweringError> {
            let result = operation.result.structural().cloned();
            result.ok_or(invalid(operation.id))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn lower_operation(
            operation: &terminal_psi::Operation,
            block: &terminal_psi::Block,
            machine: &TerminalMachine,
            module: &terminal_psi::TerminalModule,
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
            scalar_applications: Option<&terminal_installation::AdmittedScalarApplications>,
            access_view_plans: &[terminal_psi::TerminalAccessViewPlan],
            value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
            byte_sequence_literals: &[StructuralLiteral<'_>],
            unit_affine_locals: &[StructuralLiteral<'_>],
            lowered_unit_affine_locals: &mut Vec<LoweredAffineLocal>,
            operations: &mut Vec<AbstractOperation>,
        ) -> Result<(), LoweringError> {
            let lowered = routing::lower(
                operation,
                block,
                machine,
                module,
                structural_types,
                scalar_applications,
                access_view_plans,
                value_types,
                byte_sequence_literals,
                unit_affine_locals,
                lowered_unit_affine_locals,
            )?;
            operations.push(lowered);
            Ok(())
        }
    }
    mod terminator {
        use super::LoweringError;
        use abstract_operations::{AbstractOperation, AbstractSuccessor, ValueBinding};
        use semantic_vocabulary::{BlockId, ValueId};
        use std::collections::BTreeMap;
        use terminal_psi::{
            TerminalAffineCleanupAction, TerminalMachine, Terminator, ValueDeclaration,
        };

        fn structural_bindings(
            parameters: &[terminal_psi::StructuralParameterDeclaration],
            arguments: &[terminal_psi::StructuralArgument],
        ) -> Vec<abstract_operations::AbstractStructuralBinding> {
            parameters
                .iter()
                .zip(arguments)
                .map(
                    |(parameter, argument)| abstract_operations::AbstractStructuralBinding {
                        parameter: parameter.place,
                        argument: argument.clone(),
                    },
                )
                .collect()
        }

        fn value_bindings(
            parameters: &[ValueDeclaration],
            arguments: &[ValueId],
        ) -> Vec<ValueBinding> {
            parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| ValueBinding {
                    parameter: parameter.id,
                    argument: *argument,
                    scalar_type: parameter.scalar_type,
                })
                .collect()
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn lower_terminator(
            module: &terminal_psi::TerminalModule,
            block: &terminal_psi::Block,
            machine: &TerminalMachine,
            blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
            result: Option<ValueDeclaration>,
            operations: &mut Vec<AbstractOperation>,
        ) -> Result<(), LoweringError> {
            match &block.terminator {
                Terminator::InvokeNoNormal {
                    edge,
                    callee,
                    arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    ..
                } => operations.push(AbstractOperation::InvokeNoNormal {
                    psi_edge: *edge,
                    callee: *callee,
                    arguments: arguments.clone(),
                    structural_arguments: structural_arguments.clone(),
                    claim_transfers: claim_transfers.clone(),
                    requirement_obligations: requirement_obligations.clone(),
                    crash_continuations: crash_continuations.clone(),
                }),
                Terminator::Jump {
                    edge,
                    target,
                    arguments,
                    cleanup_actions,
                    structural_arguments,
                    erased_arguments: _,
                    erased_proof_arguments: _,
                } => {
                    let target_block =
                        blocks
                            .get(target)
                            .copied()
                            .ok_or(LoweringError::VerifiedBlockMissing {
                                machine: machine.id,
                                block: *target,
                            })?;
                    if target_block.parameters.len() != arguments.len()
                        || target_block.structural_parameters.len() != structural_arguments.len()
                    {
                        return Err(LoweringError::VerifiedJumpArityMismatch { edge: *edge });
                    }
                    operations.push(AbstractOperation::Jump {
                        psi_edge: *edge,
                        target: *target,
                        structural_bindings: structural_bindings(
                            &target_block.structural_parameters,
                            structural_arguments,
                        ),
                        bindings: value_bindings(&target_block.parameters, arguments),
                        cleanup_actions: runtime_cleanup_actions(module, cleanup_actions),
                    });
                }
                Terminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    let lower_successor = |successor: &terminal_psi::SuccessorEdge| {
                        let target_block = blocks.get(&successor.target).copied().ok_or(
                            LoweringError::VerifiedBlockMissing {
                                machine: machine.id,
                                block: successor.target,
                            },
                        )?;
                        if target_block.parameters.len() != successor.arguments.len()
                            || target_block.structural_parameters.len()
                                != successor.structural_arguments.len()
                        {
                            return Err(LoweringError::VerifiedJumpArityMismatch {
                                edge: successor.edge,
                            });
                        }
                        Ok(AbstractSuccessor {
                            psi_edge: successor.edge,
                            target: successor.target,
                            structural_bindings: structural_bindings(
                                &target_block.structural_parameters,
                                &successor.structural_arguments,
                            ),
                            bindings: value_bindings(
                                &target_block.parameters,
                                &successor.arguments,
                            ),
                            cleanup_actions: runtime_cleanup_actions(
                                module,
                                &successor.cleanup_actions,
                            ),
                        })
                    };
                    operations.push(AbstractOperation::Conditional {
                        condition: *condition,
                        when_true: lower_successor(when_true)?,
                        when_false: lower_successor(when_false)?,
                    });
                }
                Terminator::StructuralCase {
                    source,
                    path,
                    cases,
                } => {
                    let cases = cases
                        .iter()
                        .map(|successor| {
                            let target_block = blocks.get(&successor.target).copied().ok_or(
                                LoweringError::VerifiedBlockMissing {
                                    machine: machine.id,
                                    block: successor.target,
                                },
                            )?;
                            if target_block.parameters.len() != successor.payload_fields.len()
                                || target_block.structural_parameters.len()
                                    != successor.structural_arguments.len()
                            {
                                return Err(LoweringError::VerifiedJumpArityMismatch {
                                    edge: successor.edge,
                                });
                            }
                            Ok(abstract_operations::AbstractStructuralCaseSuccessor {
                                psi_edge: successor.edge,
                                target: successor.target,
                                case: successor.case,
                                structural_bindings: structural_bindings(
                                    &target_block.structural_parameters,
                                    &successor.structural_arguments,
                                ),
                                payloads: target_block
                                    .parameters
                                    .iter()
                                    .zip(&successor.payload_fields)
                                    .map(|(parameter, field)| {
                                        abstract_operations::AbstractStructuralCasePayloadBinding {
                                            parameter: parameter.id,
                                            field: *field,
                                            scalar_type: parameter.scalar_type,
                                        }
                                    })
                                    .collect(),
                                cleanup_actions: runtime_cleanup_actions(
                                    module,
                                    &successor.cleanup_actions,
                                ),
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    operations.push(AbstractOperation::StructuralCase {
                        source: *source,
                        path: path.clone(),
                        cases,
                    });
                }
                Terminator::Return {
                    edge,
                    value,
                    cleanup_actions,
                } => {
                    let result =
                        result.ok_or(LoweringError::ScalarReturnFromUnitMachine(machine.id))?;
                    operations.push(AbstractOperation::Return {
                        psi_edge: *edge,
                        result: result.id,
                        value: *value,
                        scalar_type: result.scalar_type,
                        cleanup_actions: runtime_cleanup_actions(module, cleanup_actions),
                    });
                }
                Terminator::ReturnUnit {
                    edge,
                    cleanup_actions,
                } => {
                    if result.is_some() {
                        return Err(LoweringError::UnitReturnFromScalarMachine(machine.id));
                    }
                    operations.push(AbstractOperation::ReturnUnit {
                        psi_edge: *edge,
                        cleanup_actions: runtime_cleanup_actions(module, cleanup_actions),
                    });
                }
                Terminator::ReturnStructural {
                    edge,
                    source,
                    returned_claims,
                    cleanup_actions,
                } if machine.result.structural().is_some() => {
                    operations.push(AbstractOperation::ReturnStructural {
                        psi_edge: *edge,
                        source: *source,
                        returned_claims: returned_claims.clone(),
                        trivial_affine_locals: Vec::new(),
                        cleanup_actions: runtime_cleanup_actions(module, cleanup_actions),
                    });
                }
                Terminator::ReturnStructural { edge, .. } => {
                    return Err(LoweringError::UnsupportedStructuralReturn {
                        machine: machine.id,
                        edge: *edge,
                    });
                }
                Terminator::ExternalCompletion {
                    edge,
                    boundary,
                    kind,
                    arguments,
                    structural_arguments,
                    requirement_obligations,
                    frontier_lower_bound,
                } => operations.push(AbstractOperation::ExternalCompletion {
                    psi_edge: *edge,
                    boundary: *boundary,
                    kind: *kind,
                    arguments: arguments.clone(),
                    structural_arguments: structural_arguments.clone(),
                    requirement_obligations: requirement_obligations.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                }),
                Terminator::Crash {
                    edge,
                    cause,
                    site_guard,
                    frontier_lower_bound,
                } => {
                    operations.push(AbstractOperation::Crash {
                        psi_edge: *edge,
                        cause: *cause,
                        site_guard: site_guard.clone(),
                        frontier_lower_bound: frontier_lower_bound.clone(),
                    });
                }
            }
            Ok(())
        }

        fn runtime_cleanup_actions(
            module: &terminal_psi::TerminalModule,
            actions: &[TerminalAffineCleanupAction],
        ) -> Vec<TerminalAffineCleanupAction> {
            actions
                .iter()
                .cloned()
                .map(|action| match action {
                    TerminalAffineCleanupAction::InvokeNominal(mut cleanup) => {
                        cleanup.cleanup_receiver = runtime_cleanup_receiver(module, &cleanup);
                        cleanup.requirement_obligations.clear();
                        TerminalAffineCleanupAction::InvokeNominal(cleanup)
                    }
                    action => action,
                })
                .collect()
        }

        fn runtime_cleanup_receiver(
            module: &terminal_psi::TerminalModule,
            cleanup: &terminal_psi::NominalAffineCleanup,
        ) -> Option<semantic_vocabulary::PlaceId> {
            module
                .machine(cleanup.cleanup_machine)
                .and_then(|machine| machine.structural_parameters.first())
                .map(|parameter| parameter.place)
        }
    }
    use abstract_operations::{
        AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractParameter,
    };
    use operation::lower_operation;
    use semantic_vocabulary::{StructuralPlaceKind, StructuralTypeId};
    use std::collections::BTreeMap;
    use terminal_psi::{StructuralPlaceDeclaration, StructuralTypeDeclaration, TerminalMachine};
    use terminator::lower_terminator;

    type StructuralLiteral<'a> = (&'a StructuralPlaceDeclaration, u32, StructuralTypeId);
    type LoweredAffineLocal = (
        OperationId,
        StructuralPlaceDeclaration,
        StructuralTypeDeclaration,
    );

    pub(super) fn lower_machine(
        module: &terminal_psi::TerminalModule,
        machine: &TerminalMachine,
        structural_types: &[StructuralTypeDeclaration],
        scalar_applications: Option<&terminal_installation::AdmittedScalarApplications>,
    ) -> Result<AbstractFunction, LoweringError> {
        let result = machine.result.scalar();
        let blocks = machine
            .blocks
            .iter()
            .map(|block| (block.id, block))
            .collect::<BTreeMap<_, _>>();
        let mut operations = Vec::new();
        let mut block_entries = Vec::with_capacity(machine.blocks.len());
        let value_types = machine
            .parameters
            .iter()
            .chain(result.iter())
            .chain(machine.block_parameters())
            .chain(machine.blocks.iter().flat_map(|block| {
                block
                    .operations
                    .iter()
                    .filter_map(|operation| operation.result.scalar_ref())
            }))
            .map(|value| (value.id, value.scalar_type))
            .collect::<BTreeMap<_, _>>();
        let byte_sequence_literals = machine
            .structural_places
            .iter()
            .filter_map(|place| match place.kind {
                StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal,
                    structural_type,
                } => Some((place, declaration_ordinal, structural_type)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let unit_affine_locals = machine
            .structural_places
            .iter()
            .filter_map(|place| match place.kind {
                semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    structural_type,
                    ..
                } => Some((place, declaration_ordinal, structural_type)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut lowered_unit_affine_locals = Vec::new();

        for block in &machine.blocks {
            block_entries.push(AbstractBlockEntry {
                block: block.id,
                structural_parameters: block.structural_parameters.clone(),
                parameters: block
                    .parameters
                    .iter()
                    .map(|parameter| AbstractParameter {
                        value: parameter.id,
                        scalar_type: parameter.scalar_type,
                    })
                    .collect(),
                operation_offset: operations.len(),
            });
            for operation in &block.operations {
                lower_operation(
                    operation,
                    block,
                    machine,
                    module,
                    structural_types,
                    scalar_applications,
                    &module.access_view_plans,
                    &value_types,
                    &byte_sequence_literals,
                    &unit_affine_locals,
                    &mut lowered_unit_affine_locals,
                    &mut operations,
                )?;
            }
            lower_terminator(module, block, machine, &blocks, result, &mut operations)?;
        }

        Ok(AbstractFunction {
            machine: machine.id,
            attachment: machine.attachment,
            nominal_cleanup_subject: machine.nominal_cleanup_subject,
            entry: machine.entry,
            parameters: machine
                .parameters
                .iter()
                .map(|parameter| AbstractParameter {
                    value: parameter.id,
                    scalar_type: parameter.scalar_type,
                })
                .collect(),
            structural_parameters: machine.structural_parameters.clone(),
            result: match &machine.result {
                terminal_psi::TerminalMachineResult::Unit => AbstractFunctionResult::Unit,
                terminal_psi::TerminalMachineResult::Scalar(result) => {
                    AbstractFunctionResult::Scalar((*result).into())
                }
                terminal_psi::TerminalMachineResult::Structural(result) => {
                    AbstractFunctionResult::Structural(result.clone())
                }
            },
            entry_claims: machine.entry_claims.clone(),
            block_entry_claims: machine.block_entry_claims.clone(),
            published_service_ceiling: machine.published_service_ceiling.clone(),
            block_entries,
            operations,
        })
    }
}

pub use error::LoweringError;

use machine::lower_machine;
use terminal_psi::TerminalModule;

pub(crate) fn lower_decoded_module(
    module: &TerminalModule,
    entry: semantic_vocabulary::MachineId,
) -> Result<AbstractOperationPlan, LoweringError> {
    lower_selected_module(
        module,
        entry,
        module.structural_types.clone(),
        None,
        None,
        None,
    )
}

pub(crate) fn lower_selected_module(
    module: &TerminalModule,
    entry: semantic_vocabulary::MachineId,
    structural_types: Vec<terminal_psi::StructuralTypeDeclaration>,
    scope: Option<&crate::native_execution_scope::NativeExecutionScope>,
    scalar_applications: Option<&terminal_installation::AdmittedScalarApplications>,
    layouts: Option<&terminal_installation::AdmittedTargetLayouts>,
) -> Result<AbstractOperationPlan, LoweringError> {
    if !module.scalar_applications.conditions.is_empty() && scalar_applications.is_none() {
        return Err(LoweringError::UnclosedScalarConditions);
    }
    if !module.machine(entry).is_some() {
        return Err(LoweringError::VerifiedEntryMachineMissing(entry));
    }
    if layouts.is_none()
        && module
            .access_lifecycle_plans
            .iter()
            .any(|plan| plan.target.is_some())
    {
        return Err(LoweringError::SemanticIdentity(
            terminal_codec::CodecError::MalformedStructuralFoundation(
                "native lifecycle requires an admitted target placement selection",
            ),
        ));
    }
    let lifecycles = layouts.map_or(module.access_lifecycle_plans.as_slice(), |layouts| {
        layouts.resolved_lifecycles()
    });
    for boundary in module
        .boundary_machines
        .iter()
        .filter(|boundary| scope.is_some_and(|scope| scope.contains_boundary(boundary.id)))
    {
        if boundary
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.execution.placement_operation())
            .is_some()
            && !lifecycles.iter().any(|plan| plan.boundary == boundary.id)
        {
            return Err(LoweringError::SemanticIdentity(
                terminal_codec::CodecError::MalformedStructuralFoundation(
                    "native lifecycle has no admitted target placement",
                ),
            ));
        }
    }
    let functions = module
        .machines
        .iter()
        .filter(|machine| scope.is_none_or(|scope| scope.contains_machine(machine.id)))
        .map(|machine| lower_machine(module, machine, &structural_types, scalar_applications))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AbstractOperationPlan {
        psi: terminal_psi_identity(module).map_err(LoweringError::SemanticIdentity)?,
        entry,
        structural_types: abstract_operations::StructuralTypeCatalog::new(
            structural_types,
            module.structural_referents.clone(),
        )
        .with_access_views(module.access_view_plans.clone())
        .with_access_view_inputs(module.access_view_inputs.clone())
        .with_access_lifecycles(lifecycles.to_vec())
        .map_err(LoweringError::SemanticIdentity)?
        .with_quotients(module.quotients.clone())
        .map_err(LoweringError::SemanticIdentity)?,
        boundary_machines: module
            .boundary_machines
            .iter()
            .filter(|boundary| scope.is_none_or(|scope| scope.contains_boundary(boundary.id)))
            .cloned()
            .collect(),
        provider_candidates: module
            .provider_candidates
            .iter()
            .enumerate()
            .filter(|(position, _)| {
                scope.is_none_or(|scope| scope.contains_candidate_position(*position))
            })
            .map(|(_, candidate)| candidate.clone())
            .collect(),
        callables: module.callables.clone(),
        dynamic_dispatch: module.dynamic_dispatch.clone(),
        identities: module.identities.clone(),
        functions,
    })
}
